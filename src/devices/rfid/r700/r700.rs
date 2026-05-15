use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use super::config::{ParamMap, R700Config};
use super::transport::{SharedEventHandler, default_event_handler, dispatch_event};
use super::types::{R700Event, R700Tag};

/// Runtime state shared between the main `R700` handle and background tasks.
pub(crate) struct R700Shared {
    pub is_connected: AtomicBool,
    pub is_reading: AtomicBool,
    pub serial_number: Mutex<Option<String>>,
    /// Set to `false` by `close()` to exit the reconnection loop.
    pub running: AtomicBool,
}

impl R700Shared {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            is_connected: AtomicBool::new(false),
            is_reading: AtomicBool::new(false),
            serial_number: Mutex::new(None),
            running: AtomicBool::new(true),
        })
    }
}

/// Impinj R700 RFID reader using the HTTP REST API.
///
/// `clone()` is cheap – all runtime state lives behind an `Arc`.
///
/// ```rust,no_run
/// # use ghpascon_rust::devices::rfid::r700::R700;
/// # #[tokio::main] async fn main() {
/// let reader = R700::default();
/// let bg = reader.clone();
/// tokio::spawn(async move { bg.connect().await; });
/// # }
/// ```
pub struct R700 {
    pub config: R700Config,
    pub on_event: SharedEventHandler,
    pub(crate) shared: Arc<R700Shared>,
}

impl Clone for R700 {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            on_event: Arc::clone(&self.on_event),
            shared: Arc::clone(&self.shared),
        }
    }
}

impl Default for R700 {
    fn default() -> Self {
        Self::new(R700Config::default())
    }
}

impl R700 {
    pub fn new(config: R700Config) -> Self {
        Self {
            config,
            on_event: default_event_handler(),
            shared: R700Shared::new(),
        }
    }

    pub fn from_map(params: ParamMap) -> Result<Self, String> {
        Ok(Self::new(R700Config::from_map(params)?))
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
            "HTTPS {}@{} (gpi_start={}, start_reading={})",
            self.config.username, self.config.ip, self.config.gpi_start, self.config.start_reading
        )
    }

    // ─── Connection ───────────────────────────────────────────────────────────

    /// Run the connection + reconnection loop forever (until `close()` is called).
    ///
    /// Spawn as a background task:
    /// ```rust,no_run
    /// # use ghpascon_rust::devices::rfid::r700::R700;
    /// # #[tokio::main] async fn main() {
    /// let reader = R700::default();
    /// let bg = reader.clone();
    /// tokio::spawn(async move { bg.connect().await; });
    /// # }
    /// ```
    pub async fn connect(&self) {
        self.shared.running.store(true, Ordering::Relaxed);
        self.run_http_loop().await;
    }

    /// Stop the reconnection loop and release resources.
    pub async fn close(&self) {
        self.shared.running.store(false, Ordering::Relaxed);
        self.shared.is_connected.store(false, Ordering::Relaxed);
        self.shared.is_reading.store(false, Ordering::Relaxed);
        *self.shared.serial_number.lock().unwrap() = None;
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &R700Event::Reading(false),
        );
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &R700Event::Connection(false),
        );
    }

    // ─── Inventory ───────────────────────────────────────────────────────────

    pub async fn start_inventory(&self) -> Result<(), String> {
        if !self.is_connected() {
            return Err("not connected".to_string());
        }
        if self.config.gpi_start {
            return Ok(());
        }
        let client = self.make_client()?;
        let url = format!("{}/profiles/inventory/start", self.config.base_url());
        let body = self.config.build_reading_config();
        self.post_to_reader(&client, &url, Some(&body)).await?;
        self.shared.is_reading.store(true, Ordering::Relaxed);
        dispatch_event(&self.on_event, &self.config.name, &R700Event::Reading(true));
        Ok(())
    }

    pub async fn stop_inventory(&self) -> Result<(), String> {
        if !self.is_connected() {
            return Err("not connected".to_string());
        }
        if self.config.gpi_start {
            return Ok(());
        }
        let client = self.make_client()?;
        let url = format!("{}/profiles/stop", self.config.base_url());
        self.post_to_reader(&client, &url, None).await?;
        self.shared.is_reading.store(false, Ordering::Relaxed);
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &R700Event::Reading(false),
        );
        Ok(())
    }

    // ─── GPO ─────────────────────────────────────────────────────────────────

    pub async fn write_gpo(
        &self,
        pin: u8,
        state: bool,
        control: &str,
        time_ms: u32,
    ) -> Result<(), String> {
        let client = self.make_client()?;
        let url = format!("{}/device/gpos", self.config.base_url());
        let gpo_state = if state { "high" } else { "low" };
        let payload = if control == "pulsed" {
            serde_json::json!({
                "gpoConfigurations": [{
                    "gpo": pin,
                    "state": gpo_state,
                    "pulseDurationMilliseconds": time_ms,
                    "control": "pulsed"
                }]
            })
        } else {
            serde_json::json!({
                "gpoConfigurations": [{
                    "gpo": pin,
                    "state": gpo_state,
                    "control": "static"
                }]
            })
        };
        self.put_to_reader(&client, &url, &payload).await
    }

    // ─── EPC write ───────────────────────────────────────────────────────────

    pub async fn write_epc(
        &self,
        target_identifier: Option<&str>,
        target_value: Option<&str>,
        new_epc: &str,
        password: &str,
    ) -> Result<(), String> {
        if new_epc.len() < 24 {
            return Err("EPC must be at least 24 hex characters".to_string());
        }
        let client = self.make_client()?;
        let url = format!("{}/profiles/inventory/tag-access", self.config.base_url());

        let identifier_str = target_identifier.unwrap_or("epc");
        let bit_offset = if target_identifier == Some("tid") {
            0
        } else {
            32
        };
        let mask_length = if target_identifier.is_none() { 1 } else { 96 };

        let payload = serde_json::json!({
            "accessConfigurations": [{
                "accessCommands": [
                    {"identifier": "1", "blockWrite": {"memoryBank": "epc", "wordOffset": 2, "dataHex": &new_epc[0..8]}},
                    {"identifier": "2", "blockWrite": {"memoryBank": "epc", "wordOffset": 4, "dataHex": &new_epc[8..16]}},
                    {"identifier": "3", "blockWrite": {"memoryBank": "epc", "wordOffset": 6, "dataHex": &new_epc[16..24]}}
                ],
                "tagAccessPasswordHex": password,
                "tagSelectors": [{
                    "action": "include",
                    "tagMemoryBank": identifier_str,
                    "bitOffset": bit_offset,
                    "mask": target_value.unwrap_or("0"),
                    "maskLength": mask_length
                }]
            }]
        });

        self.post_to_reader(&client, &url, Some(&payload)).await
    }

    // ─── Protected inventory ──────────────────────────────────────────────────

    pub async fn protected_inventory(
        &mut self,
        active: bool,
        password: Option<&str>,
    ) -> Result<(), String> {
        let pwd = password
            .unwrap_or(&self.config.protected_inventory_password)
            .to_string();
        self.config.protected_inventory_active = active;
        self.config.protected_inventory_password = pwd;
        // Restart inventory to apply new config
        if self.is_reading() {
            self.stop_inventory().await.ok();
            self.start_inventory().await.ok();
        }
        Ok(())
    }

    // ─── Internal helpers ─────────────────────────────────────────────────────

    pub(crate) fn on_tag(&self, tag: R700Tag) {
        let mut tagged = tag;
        tagged.protected = self.config.protected_inventory_active;
        dispatch_event(&self.on_event, &self.config.name, &R700Event::Tag(tagged));
    }

    pub(crate) fn on_connected(&self) {
        self.shared.is_connected.store(true, Ordering::Relaxed);
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &R700Event::Connection(true),
        );
    }

    pub(crate) fn on_disconnected(&self) {
        self.shared.is_connected.store(false, Ordering::Relaxed);
        self.shared.is_reading.store(false, Ordering::Relaxed);
        *self.shared.serial_number.lock().unwrap() = None;
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &R700Event::Connection(false),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Number, Value};
    use std::collections::HashMap;

    #[test]
    fn default_reader_is_not_connected() {
        let reader = R700::default();
        assert!(!reader.is_connected());
        assert!(!reader.is_reading());
        assert!(reader.serial_number().is_none());
    }

    #[test]
    fn from_map_applies_all_fields() {
        let params = HashMap::from([
            ("name".to_string(), Value::String("test-r700".to_string())),
            ("ip".to_string(), Value::String("10.0.0.99".to_string())),
            ("session".to_string(), Value::Number(Number::from(2))),
            ("read_power".to_string(), Value::Number(Number::from(2500))),
            ("gpi_start".to_string(), Value::Bool(true)),
            (
                "active_ant".to_string(),
                Value::Array(vec![
                    Value::Number(Number::from(1)),
                    Value::Number(Number::from(2)),
                ]),
            ),
        ]);

        let reader = R700::from_map(params).expect("valid");
        assert_eq!(reader.config.name, "test-r700");
        assert_eq!(reader.config.ip, "10.0.0.99");
        assert_eq!(reader.config.session, 2);
        assert_eq!(reader.config.read_power, 2500);
        assert!(reader.config.gpi_start);
        assert!(!reader.config.start_reading); // forced off by gpi_start
        assert_eq!(reader.config.active_ant, vec![1, 2]);
    }

    #[test]
    fn clone_shares_runtime_state() {
        let reader = R700::default();
        let clone = reader.clone();
        reader
            .shared
            .is_connected
            .store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(clone.is_connected());
    }

    #[test]
    fn default_search_mode_matches_python() {
        let cfg = R700Config::default();
        assert_eq!(cfg.search_mode, "single-target");
    }

    #[test]
    fn build_reading_config_has_antennas() {
        let mut cfg = R700Config::default();
        cfg.active_ant = vec![1, 2];
        cfg.session = 2;
        cfg.read_power = 2800;

        let json = cfg.build_reading_config();
        let ants = json["antennaConfigs"].as_array().unwrap();
        assert_eq!(ants.len(), 2);
        assert_eq!(ants[0]["antennaPort"], 1);
        assert_eq!(ants[0]["inventorySession"], 2);
        assert_eq!(ants[0]["transmitPowerCdbm"], 2800);
    }

    #[test]
    fn build_reading_config_gpi_triggers() {
        let mut cfg = R700Config::default();
        cfg.gpi_start = true;
        let json = cfg.build_reading_config();
        assert!(json.get("startTriggers").is_some());
        assert!(json.get("stopTriggers").is_some());
    }

    #[test]
    fn connect_instruction_format() {
        let reader = R700::default();
        let instr = reader.connect_instruction();
        assert!(instr.contains("192.168.1.101"));
        assert!(instr.contains("root"));
    }
}
