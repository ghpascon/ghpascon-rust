use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use super::config::{ConnectionType, ParamMap, X714Config};
use super::parser::parse_line_to_events;
use super::transport::{SharedEventHandler, default_event_handler, dispatch_event};
use super::types::X714Event;

/// Shared mutable runtime state. Lives inside an `Arc` so background tasks
/// (read loop, monitor, ping) can access the same state as the user-facing `X714`.
pub(crate) struct X714Shared {
    pub is_connected: AtomicBool,
    pub is_reading: AtomicBool,
    pub serial_number: Mutex<Option<String>>,
    /// Generic async writer – holds OwnedWriteHalf (TCP) or WriteHalf<SerialStream> (Serial).
    pub writer: tokio::sync::Mutex<Option<Box<dyn tokio::io::AsyncWrite + Send + Unpin>>>,
    /// BLE write channel – set by run_ble_loop, None for Serial/TCP.
    pub ble_write_tx: tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedSender<Vec<u8>>>>,
    /// Set to `false` by `close()` to break reconnection loops.
    pub running: AtomicBool,
}

impl X714Shared {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            is_connected: AtomicBool::new(false),
            is_reading: AtomicBool::new(false),
            serial_number: Mutex::new(None),
            writer: tokio::sync::Mutex::new(None),
            ble_write_tx: tokio::sync::Mutex::new(None),
            running: AtomicBool::new(true),
        })
    }
}

/// X714 RFID reader.
///
/// `clone()` is cheap – all runtime state is behind an `Arc` so clones share
/// the same connection.  Typical pattern:
///
/// ```rust,no_run
/// # use ghpascon_rust::devices::rfid::x714::X714;
/// # #[tokio::main] async fn main() {
/// let reader = X714::default();
/// let bg = reader.clone();
/// tokio::spawn(async move { bg.connect().await; }); // reconnects automatically
/// reader.write("#READ:ON").await.ok();
/// # }
/// ```
pub struct X714 {
    pub config: X714Config,
    pub on_event: SharedEventHandler,
    pub(crate) shared: Arc<X714Shared>,
}

impl Clone for X714 {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            on_event: Arc::clone(&self.on_event),
            shared: Arc::clone(&self.shared),
        }
    }
}

impl Default for X714 {
    fn default() -> Self {
        Self::new(X714Config::default())
    }
}

impl X714 {
    pub fn new(config: X714Config) -> Self {
        Self {
            config,
            on_event: default_event_handler(),
            shared: X714Shared::new(),
        }
    }

    pub fn from_map(params: ParamMap) -> Result<Self, String> {
        Ok(Self::new(X714Config::from_map(params)?))
    }

    pub fn with_event_handler(mut self, handler: SharedEventHandler) -> Self {
        self.on_event = handler;
        self
    }

    pub fn set_event_handler(&mut self, handler: SharedEventHandler) {
        self.on_event = handler;
    }

    // ─── Runtime state ────────────────────────────────────────────────────────

    pub fn is_connected(&self) -> bool {
        self.shared.is_connected.load(Ordering::Relaxed)
    }

    pub fn is_reading(&self) -> bool {
        self.shared.is_reading.load(Ordering::Relaxed)
    }

    pub fn serial_number(&self) -> Option<String> {
        self.shared.serial_number.lock().unwrap().clone()
    }

    // ─── Config helpers ───────────────────────────────────────────────────────

    pub fn to_map(&self) -> ParamMap {
        self.config.to_map()
    }

    pub fn connect_instruction(&self) -> String {
        match self.config.connection_type {
            ConnectionType::Serial => format!(
                "SERIAL {} @ {} (VID={:#06x}, PID={:#06x})",
                self.config.serial.port,
                self.config.serial.baudrate,
                self.config.serial.vid,
                self.config.serial.pid
            ),
            ConnectionType::Tcp => format!("TCP {}:{}", self.config.tcp.ip, self.config.tcp.port),
            ConnectionType::Ble => {
                if let Some(address) = &self.config.ble.address {
                    format!("BLE {} ({})", self.config.ble.name, address)
                } else {
                    format!("BLE {} (auto-discovery)", self.config.ble.name)
                }
            }
        }
    }

    // ─── Connection ───────────────────────────────────────────────────────────

    /// Run the connection + reconnection loop forever (until `close()` is called).
    ///
    /// Spawn this as a background task:
    /// ```rust,no_run
    /// # use ghpascon_rust::devices::rfid::x714::X714;
    /// # #[tokio::main] async fn main() {
    /// let reader = X714::default();
    /// let bg = reader.clone();
    /// tokio::spawn(async move { bg.connect().await; });
    /// # }
    /// ```
    pub async fn connect(&self) {
        self.shared.running.store(true, Ordering::Relaxed);
        match self.config.connection_type {
            ConnectionType::Serial => self.run_serial_loop().await,
            ConnectionType::Tcp => self.run_tcp_loop().await,
            ConnectionType::Ble => self.run_ble_loop().await,
        }
    }

    /// Stop all connection loops and clean up resources.
    pub async fn close(&self) {
        self.shared.running.store(false, Ordering::Relaxed);
        self.shared.is_connected.store(false, Ordering::Relaxed);
        self.shared.is_reading.store(false, Ordering::Relaxed);
        *self.shared.serial_number.lock().unwrap() = None;
        *self.shared.writer.lock().await = None;
        *self.shared.ble_write_tx.lock().await = None;
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &X714Event::Connection(false),
        );
    }

    // ─── Write ────────────────────────────────────────────────────────────────

    /// Send a command string over the current transport (appends `\n`).
    /// Handles Serial/TCP (stream writer) and BLE (unbounded channel) transparently.
    /// BLE operations are serialized to avoid concurrent write conflicts.
    pub async fn write(&self, command: &str) -> Result<(), String> {
        let payload_ble = command.trim().as_bytes().to_vec();
        let frame_stream = format!("{}\n", command.trim()).into_bytes();

        // BLE path: send through the mpsc channel to the BLE write task with small delay after send.
        {
            let guard = self.shared.ble_write_tx.lock().await;
            if let Some(sender) = guard.as_ref() {
                // Throttling is handled by the 150 ms delay inside the BLE write task.
                return sender
                    .send(payload_ble)
                    .map_err(|e| format!("BLE write channel closed: {e}"));
            }
        }

        // Serial / TCP path: write directly to the async stream.
        use tokio::io::AsyncWriteExt;
        let mut guard = self.shared.writer.lock().await;
        if let Some(writer) = guard.as_mut() {
            writer
                .write_all(&frame_stream)
                .await
                .map_err(|e| format!("write error: {e}"))
        } else {
            Err("not connected".to_string())
        }
    }

    // ─── Receive / parse ──────────────────────────────────────────────────────

    /// Process one raw line received from the reader.
    /// Updates internal state and dispatches events through the registered handler.
    pub fn on_receive(&self, data: &str) {
        let events = parse_line_to_events(data);
        for event in &events {
            self.apply_event_to_state(event);
            dispatch_event(&self.on_event, &self.config.name, event);
        }
    }

    /// Same as `on_receive` but also returns the parsed events (useful for tests/examples).
    pub fn parse_line(&self, input: &str) -> Vec<X714Event> {
        let events = parse_line_to_events(input);
        for event in &events {
            self.apply_event_to_state(event);
            dispatch_event(&self.on_event, &self.config.name, event);
        }
        events
    }

    pub(crate) fn apply_event_to_state(&self, event: &X714Event) {
        match event {
            X714Event::Reading(v) => self.shared.is_reading.store(*v, Ordering::Relaxed),
            X714Event::SerialNumber(s) => {
                *self.shared.serial_number.lock().unwrap() = Some(s.clone());
            }
            _ => {}
        }
    }

    // ─── Connection hooks ─────────────────────────────────────────────────────

    /// Called by each transport loop right after a successful connection.
    /// Mirrors Python's `on_connected()`.
    pub(crate) async fn on_connected(&self) {
        self.shared.is_connected.store(true, Ordering::Relaxed);
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &X714Event::Connection(true),
        );
        self.config_reader().await.ok();
        if self.config.start_reading && !self.config.gpi_start {
            self.start_inventory().await.ok();
        }
    }

    pub(crate) fn on_disconnected(&self) {
        self.shared.is_connected.store(false, Ordering::Relaxed);
        self.shared.is_reading.store(false, Ordering::Relaxed);
        *self.shared.serial_number.lock().unwrap() = None;
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &X714Event::Connection(false),
        );
    }

    pub fn on_start(&self) {
        self.shared.is_reading.store(true, Ordering::Relaxed);
        dispatch_event(&self.on_event, &self.config.name, &X714Event::Reading(true));
    }

    pub fn on_stop(&self) {
        self.shared.is_reading.store(false, Ordering::Relaxed);
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &X714Event::Reading(false),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::{Number, Value};

    use super::*;

    #[test]
    fn from_map_builds_reader_and_antennas() {
        let params = HashMap::from([
            (
                "connection_type".to_string(),
                Value::String("tcp".to_string()),
            ),
            ("ip".to_string(), Value::String("10.0.0.40".to_string())),
            ("tcp_port".to_string(), Value::Number(Number::from(23))),
            (
                "active_ant".to_string(),
                Value::Array(vec![
                    Value::Number(Number::from(1)),
                    Value::Number(Number::from(3)),
                ]),
            ),
            ("read_power".to_string(), Value::Number(Number::from(25))),
        ]);

        let reader = X714::from_map(params).expect("valid map");
        assert_eq!(reader.config.connection_type, ConnectionType::Tcp);
        assert_eq!(reader.config.tcp.ip, "10.0.0.40");
        assert!(reader.config.ant_dict.get("1").expect("ant1").active);
        assert!(!reader.config.ant_dict.get("2").expect("ant2").active);
        assert!(reader.config.ant_dict.get("3").expect("ant3").active);
        assert_eq!(reader.config.ant_dict.get("4").expect("ant4").power, 25);
    }

    #[test]
    fn parse_tag_line() {
        let reader = X714::default();
        let events = reader.parse_line("#T+@E28011|E20033|2|58|LOCKED");
        assert_eq!(events.len(), 1);

        match &events[0] {
            X714Event::Tag(tag) => {
                assert_eq!(tag.epc.as_deref(), Some("e28011"));
                assert_eq!(tag.tid.as_deref(), Some("e20033"));
                assert_eq!(tag.ant, 2);
                assert_eq!(tag.rssi, -58);
                assert_eq!(tag.protected.as_deref(), Some("locked"));
            }
            _ => panic!("unexpected event"),
        }
    }

    #[test]
    fn reading_state_updated_by_parse_line() {
        let reader = X714::default();
        assert!(!reader.is_reading());
        reader.parse_line("#read:on");
        assert!(reader.is_reading());
        reader.parse_line("#read:off");
        assert!(!reader.is_reading());
    }

    #[test]
    fn serial_number_updated_by_parse_line() {
        let reader = X714::default();
        reader.parse_line("#name:X714-ABC123");
        assert_eq!(reader.serial_number().as_deref(), Some("x714-abc123"));
    }

    #[test]
    fn clone_shares_runtime_state() {
        let reader = X714::default();
        let clone = reader.clone();
        reader.parse_line("#read:on");
        assert!(clone.is_reading()); // same AtomicBool via Arc
    }

    #[test]
    fn config_commands_include_core_flags() {
        let mut reader = X714::default();
        reader.config.session = 2;
        reader.config.start_reading = true;

        let cmds = reader.config_commands();
        assert!(cmds.iter().any(|c| c == "#session:2"));
        assert!(cmds.iter().any(|c| c == "#start_reading:on"));
        assert!(cmds.iter().any(|c| c == "#setup_reader"));
    }
}
