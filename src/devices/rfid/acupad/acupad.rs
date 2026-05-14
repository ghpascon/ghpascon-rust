use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use super::config::{AcupadConfig, ParamMap};
use super::parser::parse_line_to_events;
use super::transport::{SharedEventHandler, default_event_handler, dispatch_event};
use super::types::AcupadEvent;

/// Shared runtime state for Acupad. Lives inside an `Arc` so background
/// tasks share the same connection state as the user-facing `Acupad`.
pub(crate) struct AcupadShared {
    pub is_connected: AtomicBool,
    pub is_reading: AtomicBool,
    pub serial_number: Mutex<Option<String>>,
    pub writer: tokio::sync::Mutex<Option<Box<dyn tokio::io::AsyncWrite + Send + Unpin>>>,
    pub running: AtomicBool,
}

impl AcupadShared {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            is_connected: AtomicBool::new(false),
            is_reading: AtomicBool::new(false),
            serial_number: Mutex::new(None),
            writer: tokio::sync::Mutex::new(None),
            running: AtomicBool::new(true),
        })
    }
}

/// ACUPAD RFID reader (serial-only).
///
/// `clone()` is cheap – all runtime state is behind an `Arc`.
///
/// ```rust,no_run
/// # use ghpascon_rust::devices::rfid::acupad::Acupad;
/// # #[tokio::main] async fn main() {
/// let reader = Acupad::default();
/// let bg = reader.clone();
/// tokio::spawn(async move { bg.connect().await; });
/// # }
/// ```
pub struct Acupad {
    pub config: AcupadConfig,
    pub on_event: SharedEventHandler,
    pub(crate) shared: Arc<AcupadShared>,
}

impl Clone for Acupad {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            on_event: Arc::clone(&self.on_event),
            shared: Arc::clone(&self.shared),
        }
    }
}

impl Default for Acupad {
    fn default() -> Self {
        Self::new(AcupadConfig::default())
    }
}

impl Acupad {
    pub fn new(config: AcupadConfig) -> Self {
        Self {
            config,
            on_event: default_event_handler(),
            shared: AcupadShared::new(),
        }
    }

    pub fn from_map(params: ParamMap) -> Result<Self, String> {
        Ok(Self::new(AcupadConfig::from_map(params)?))
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
        format!(
            "SERIAL {} @ {} (VID={:#06x}, PID={:#06x})",
            self.config.port,
            self.config.baudrate,
            self.config.vid,
            self.config.pid
        )
    }

    // ─── Connection ───────────────────────────────────────────────────────────

    pub async fn connect(&self) {
        self.shared.running.store(true, Ordering::Relaxed);
        self.run_serial_loop().await;
    }

    pub async fn close(&self) {
        self.shared.running.store(false, Ordering::Relaxed);
        self.shared.is_connected.store(false, Ordering::Relaxed);
        self.shared.is_reading.store(false, Ordering::Relaxed);
        *self.shared.serial_number.lock().unwrap() = None;
        *self.shared.writer.lock().await = None;
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &AcupadEvent::Connection(false),
        );
    }

    // ─── Write ────────────────────────────────────────────────────────────────

    pub async fn write(&self, command: &str) -> Result<(), String> {
        use tokio::io::AsyncWriteExt;
        let frame = format!("{}\n", command.trim()).into_bytes();
        let mut guard = self.shared.writer.lock().await;
        if let Some(writer) = guard.as_mut() {
            writer
                .write_all(&frame)
                .await
                .map_err(|e| format!("write error: {e}"))
        } else {
            Err("not connected".to_string())
        }
    }

    // ─── Receive / parse ──────────────────────────────────────────────────────

    pub fn on_receive(&self, data: &str) {
        let events = parse_line_to_events(data);
        for event in &events {
            self.apply_event_to_state(event);
            dispatch_event(&self.on_event, &self.config.name, event);
        }
    }

    pub fn parse_line(&self, input: &str) -> Vec<AcupadEvent> {
        let events = parse_line_to_events(input);
        for event in &events {
            self.apply_event_to_state(event);
            dispatch_event(&self.on_event, &self.config.name, event);
        }
        events
    }

    pub(crate) fn apply_event_to_state(&self, event: &AcupadEvent) {
        match event {
            AcupadEvent::Reading(v) => self.shared.is_reading.store(*v, Ordering::Relaxed),
            AcupadEvent::SerialNumber(s) => {
                *self.shared.serial_number.lock().unwrap() = Some(s.clone());
            }
            _ => {}
        }
    }

    // ─── Connection hooks ─────────────────────────────────────────────────────

    pub(crate) async fn on_connected(&self) {
        self.shared.is_connected.store(true, Ordering::Relaxed);
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &AcupadEvent::Connection(true),
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
            &AcupadEvent::Connection(false),
        );
    }

    pub fn on_start(&self) {
        self.shared.is_reading.store(true, Ordering::Relaxed);
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &AcupadEvent::Reading(true),
        );
    }

    pub fn on_stop(&self) {
        self.shared.is_reading.store(false, Ordering::Relaxed);
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &AcupadEvent::Reading(false),
        );
    }
}
