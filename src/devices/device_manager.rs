//! `DeviceManager` — loads and manages multiple RFID and generic devices.
//!
//! Each device is configured via a `.json` file in a directory.
//! The `"reader"` field determines the type.
//! All other fields are optional — defaults are applied automatically.

use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

use super::generic::serial::SerialDevice;
use super::generic::tcp::TcpDevice;
use super::printer::sato::{SatoPrinter, SatoWs4Printer};
use super::rfid::r700::R700;
use super::rfid::x714::X714;

pub type EventHandler = dyn FnMut(&str, &str, Option<Value>) + Send + 'static;
pub type SharedEventHandler = Arc<Mutex<Box<EventHandler>>>;

static CONFIG_EXAMPLES: &[(&str, fn() -> HashMap<String, Value>)] = &[
    ("X714_DEFAULT", || {
        super::rfid::x714::config_example::x714_default_map()
    }),
    ("X714_SERIAL", || {
        super::rfid::x714::config_example::x714_map()
    }),
    ("X714_TCP", || {
        super::rfid::x714::config_example::x714_tcp_map()
    }),
    ("X714_BLE", || {
        super::rfid::x714::config_example::x714_ble_map()
    }),
    ("X714_ALL", || {
        super::rfid::x714::config_example::x714_all_map()
    }),
    ("R700_IOT", || {
        super::rfid::r700::config_example::r700_iot_map()
    }),
    ("R700_IOT_DICT", || {
        super::rfid::r700::config_example::r700_iot_dict_map()
    }),
    ("R700_IOT_GPI", || {
        super::rfid::r700::config_example::r700_iot_gpi_map()
    }),
    ("R700_PROTECTED_INVENTORY", || {
        super::rfid::r700::config_example::r700_protected_inventory_map()
    }),
    ("SERIAL", || {
        super::generic::serial::config_example::serial_default_map()
    }),
    ("SERIAL_CUSTOM", || {
        super::generic::serial::config_example::serial_custom_map()
    }),
    ("TCP", || {
        super::generic::tcp::config_example::tcp_default_map()
    }),
    ("TCP_CUSTOM", || {
        super::generic::tcp::config_example::tcp_custom_map()
    }),
    ("SATO", || {
        super::printer::sato::config_example::sato_default_map()
    }),
    ("SATO_WS4", || {
        super::printer::sato::config_example::sato_ws4_map()
    }),
];

pub enum Device {
    X714(X714),
    R700(R700),
    Serial(SerialDevice),
    Tcp(TcpDevice),
    Sato(SatoPrinter),
    SatoWs4(SatoWs4Printer),
}

impl Clone for Device {
    fn clone(&self) -> Self {
        match self {
            Self::X714(d) => Self::X714(d.clone()),
            Self::R700(d) => Self::R700(d.clone()),
            Self::Serial(d) => Self::Serial(d.clone()),
            Self::Tcp(d) => Self::Tcp(d.clone()),
            Self::Sato(d) => Self::Sato(d.clone()),
            Self::SatoWs4(d) => Self::SatoWs4(d.clone()),
        }
    }
}

impl Device {
    pub fn name(&self) -> &str {
        match self {
            Self::X714(d) => &d.config.name,
            Self::R700(d) => &d.config.name,
            Self::Serial(d) => &d.config.name,
            Self::Tcp(d) => &d.config.name,
            Self::Sato(d) => &d.config.name,
            Self::SatoWs4(d) => &d.config.name,
        }
    }

    pub fn device_type(&self) -> &'static str {
        match self {
            Self::X714(_) => "X714",
            Self::R700(_) => "R700_IOT",
            Self::Serial(_) => "SERIAL",
            Self::Tcp(_) => "TCP",
            Self::Sato(_) => "SATO",
            Self::SatoWs4(_) => "SATO_WS4",
        }
    }

    pub fn device_class(&self) -> &'static str {
        match self {
            Self::X714(_) => "X714",
            Self::R700(_) => "R700",
            Self::Serial(_) => "SerialDevice",
            Self::Tcp(_) => "TcpDevice",
            Self::Sato(_) => "SatoPrinter",
            Self::SatoWs4(_) => "SatoWs4Printer",
        }
    }

    pub fn is_connected(&self) -> bool {
        match self {
            Self::X714(d) => d.is_connected(),
            Self::R700(d) => d.is_connected(),
            Self::Serial(d) => d.is_connected(),
            Self::Tcp(d) => d.is_connected(),
            Self::Sato(d) => d.is_connected(),
            Self::SatoWs4(d) => d.is_connected(),
        }
    }

    pub fn is_reading(&self) -> bool {
        match self {
            Self::X714(d) => d.is_reading(),
            Self::R700(d) => d.is_reading(),
            Self::Serial(_) => false,
            Self::Tcp(_) => false,
            Self::Sato(_) => false,
            Self::SatoWs4(_) => false,
        }
    }

    pub fn is_gpi_trigger_on(&self) -> bool {
        match self {
            Self::X714(d) => d.config.gpi_start,
            Self::R700(d) => d.config.gpi_start,
            Self::Serial(_) | Self::Tcp(_) | Self::Sato(_) | Self::SatoWs4(_) => false,
        }
    }

    pub fn serial_number(&self) -> Option<String> {
        match self {
            Self::X714(d) => d.serial_number(),
            Self::R700(d) => d.serial_number(),
            Self::Serial(_) => None,
            Self::Tcp(_) => None,
            Self::Sato(_) => None,
            Self::SatoWs4(_) => None,
        }
    }

    pub fn can_print(&self) -> bool {
        match self {
            Self::Sato(d) => d.can_print(),
            Self::SatoWs4(d) => d.can_print(),
            _ => false,
        }
    }

    pub fn pending_print_jobs(&self) -> usize {
        match self {
            Self::Sato(d) => d.pending_print_jobs(),
            Self::SatoWs4(d) => d.pending_print_jobs(),
            _ => 0,
        }
    }

    pub fn to_map(&self) -> HashMap<String, Value> {
        match self {
            Self::X714(d) => d.to_map(),
            Self::R700(d) => d.to_map(),
            Self::Serial(d) => d.to_map(),
            Self::Tcp(d) => d.to_map(),
            Self::Sato(d) => d.to_map(),
            Self::SatoWs4(d) => d.to_map(),
        }
    }

    pub fn connect_instruction(&self) -> String {
        match self {
            Self::X714(d) => d.connect_instruction(),
            Self::R700(d) => d.connect_instruction(),
            Self::Serial(d) => d.connect_instruction(),
            Self::Tcp(d) => d.connect_instruction(),
            Self::Sato(d) => d.connect_instruction(),
            Self::SatoWs4(d) => d.connect_instruction(),
        }
    }

    pub fn set_event_handler(&mut self, handler: SharedEventHandler) {
        match self {
            Self::X714(d) => d.set_event_handler(handler),
            Self::R700(d) => d.set_event_handler(handler),
            Self::Serial(d) => d.set_event_handler(handler),
            Self::Tcp(d) => d.set_event_handler(handler),
            Self::Sato(d) => d.set_event_handler(handler),
            Self::SatoWs4(d) => d.set_event_handler(handler),
        }
    }

    pub async fn connect(&self) {
        match self {
            Self::X714(d) => d.connect().await,
            Self::R700(d) => d.connect().await,
            Self::Serial(d) => d.connect().await,
            Self::Tcp(d) => d.connect().await,
            Self::Sato(d) => d.connect().await,
            Self::SatoWs4(d) => d.0.connect().await,
        }
    }

    pub async fn close(&self) {
        match self {
            Self::X714(d) => d.close().await,
            Self::R700(d) => d.close().await,
            Self::Serial(d) => d.close().await,
            Self::Tcp(d) => d.close().await,
            Self::Sato(d) => d.close().await,
            Self::SatoWs4(d) => d.0.close().await,
        }
    }

    pub async fn start_inventory(&self) -> Result<(), String> {
        match self {
            Self::X714(d) => d.start_inventory().await,
            Self::R700(d) => d.start_inventory().await,
            _ => Err("device type does not support this operation".to_string()),
        }
    }

    pub async fn stop_inventory(&self) -> Result<(), String> {
        match self {
            Self::X714(d) => d.stop_inventory().await,
            Self::R700(d) => d.stop_inventory().await,
            _ => Err("device type does not support this operation".to_string()),
        }
    }

    pub async fn write_epc(
        &self,
        target_identifier: Option<&str>,
        target_value: Option<&str>,
        new_epc: &str,
        password: &str,
    ) -> Result<(), String> {
        match self {
            Self::X714(d) => {
                d.write_epc(target_identifier, target_value, new_epc, password)
                    .await
            }
            Self::R700(d) => {
                d.write_epc(target_identifier, target_value, new_epc, password)
                    .await
            }
            _ => Err("device type does not support this operation".to_string()),
        }
    }

    pub async fn write_gpo(
        &self,
        pin: u8,
        state: bool,
        control: &str,
        time_ms: u64,
    ) -> Result<(), String> {
        match self {
            Self::X714(d) => d.write_gpo(pin, state, control, time_ms).await,
            Self::R700(d) => d.write_gpo(pin, state, control, time_ms as u32).await,
            _ => Err("device type does not support this operation".to_string()),
        }
    }
}

/// Snapshot of device state (does not hold a reference to the device).
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceInfo {
    pub name: String,
    pub device_type: String,
    pub device_class: String,
    pub is_connected: bool,
    pub is_reading: bool,
    pub is_gpi_trigger_on: bool,
    pub can_print: bool,
    pub to_print: usize,
    pub has_serial_number: bool,
    pub serial_number: String,
    pub connect_instruction: String,
    pub current_config: HashMap<String, Value>,
}

/// Manages multiple devices: loads JSON configs, connects, dispatches events.
pub struct DeviceManager {
    pub devices: Vec<Device>,
    devices_path: PathBuf,
    event_handler: Option<SharedEventHandler>,
    connect_tasks: Vec<JoinHandle<()>>,
}

impl DeviceManager {
    pub fn new<P: AsRef<Path>>(devices_path: P) -> Self {
        Self {
            devices: Vec::new(),
            devices_path: devices_path.as_ref().to_path_buf(),
            event_handler: None,
            connect_tasks: Vec::new(),
        }
    }

    pub fn with_event_handler(mut self, handler: SharedEventHandler) -> Self {
        self.event_handler = Some(handler);
        self
    }

    pub fn set_event_handler(&mut self, handler: SharedEventHandler) {
        self.event_handler = Some(handler);
    }

    pub fn load_devices(&mut self) {
        self.devices.clear();

        if !self.devices_path.exists() {
            match std::fs::create_dir_all(&self.devices_path) {
                Ok(_) => eprintln!("📁 Directory created: {}", self.devices_path.display()),
                Err(e) => {
                    eprintln!(
                        "❌ Could not create directory '{}': {e}",
                        self.devices_path.display()
                    );
                    return;
                }
            }
        }

        let entries = match std::fs::read_dir(&self.devices_path) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("❌ Error listing '{}': {e}", self.devices_path.display());
                return;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            let filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let name = filename.trim_end_matches(".json").to_string();

            eprintln!("📄 Reading '{}'…", filename);

            let content = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("❌ Error reading '{}': {e}", filename);
                    continue;
                }
            };

            let raw: HashMap<String, Value> = match serde_json::from_str(&content) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("❌ Invalid JSON in '{}': {e}", filename);
                    continue;
                }
            };

            let data: HashMap<String, Value> = raw
                .into_iter()
                .map(|(k, v)| (k.to_lowercase(), v))
                .collect();

            let reader_type = match data.get("reader").and_then(|v| v.as_str()) {
                Some(t) => t.to_string(),
                None => {
                    eprintln!("⚠️  '{}' has no 'reader' field — skipped", filename);
                    continue;
                }
            };

            self.add_device(&name, &reader_type, data);
        }

        self.assign_event_handler();
        eprintln!("✅ {} device(s) loaded", self.devices.len());
    }

    pub fn add_device(&mut self, name: &str, device_type: &str, mut data: HashMap<String, Value>) {
        data.insert("name".to_string(), Value::String(name.to_string()));

        match device_type.to_uppercase().as_str() {
            "X714" => match X714::from_map(data) {
                Ok(d) => {
                    eprintln!("  ✅ X714 '{}' → {}", name, d.connect_instruction());
                    self.devices.push(Device::X714(d));
                }
                Err(e) => eprintln!("  ❌ X714 '{}' config error: {e}", name),
            },
            "R700_IOT" | "R700" => match R700::from_map(data) {
                Ok(d) => {
                    eprintln!("  ✅ R700 '{}' → {}", name, d.connect_instruction());
                    self.devices.push(Device::R700(d));
                }
                Err(e) => eprintln!("  ❌ R700 '{}' config error: {e}", name),
            },
            "SERIAL" => {
                let d = SerialDevice::from_map(data);
                eprintln!("  ✅ SERIAL '{}' → {}", name, d.connect_instruction());
                self.devices.push(Device::Serial(d));
            }
            "TCP" => {
                let d = TcpDevice::from_map(data);
                eprintln!("  ✅ TCP '{}' → {}", name, d.connect_instruction());
                self.devices.push(Device::Tcp(d));
            }
            "SATO" => {
                let d = SatoPrinter::from_map(data);
                eprintln!("  ✅ SATO '{}' → {}", name, d.connect_instruction());
                self.devices.push(Device::Sato(d));
            }
            "SATO_WS4" => {
                let d = SatoWs4Printer::from_map(data);
                eprintln!("  ✅ SATO_WS4 '{}' → {}", name, d.connect_instruction());
                self.devices.push(Device::SatoWs4(d));
            }
            other => eprintln!("  ⚠️  Unknown type '{}' for '{}' — skipped", other, name),
        }
    }

    pub fn assign_event_handler(&mut self) {
        let Some(handler) = &self.event_handler else {
            return;
        };
        for device in &mut self.devices {
            device.set_event_handler(Arc::clone(handler));
        }
    }

    pub async fn connect_devices(&mut self, force: bool) {
        let active = self
            .connect_tasks
            .iter()
            .filter(|t| !t.is_finished())
            .count();

        if active > 0 && !force {
            eprintln!(
                "ℹ️  {} active connection task(s) — use force=true to restart",
                active
            );
            return;
        }

        self.cancel_connect_tasks().await;
        self.disconnect_devices().await;
        self.load_devices();

        let mut tasks = Vec::new();
        for device in &self.devices {
            let d = device.clone();
            let name_display = d.connect_instruction();
            eprintln!("🚀 Connecting '{}'…  ({})", d.name(), name_display);
            let handle = tokio::spawn(async move { d.connect().await });
            tasks.push(handle);
        }

        eprintln!("ℹ️  {} connection task(s) started", tasks.len());
        self.connect_tasks = tasks;
    }

    pub async fn cancel_connect_tasks(&mut self) {
        let n = self.connect_tasks.len();
        for task in self.connect_tasks.drain(..) {
            task.abort();
        }
        if n > 0 {
            eprintln!("🛑 {} task(s) cancelled", n);
        }
    }

    pub async fn disconnect_devices(&mut self) {
        for device in &self.devices {
            device.close().await;
        }
        self.devices.clear();
    }

    pub fn len(&self) -> usize {
        self.devices.len()
    }
    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    pub fn get_device_names(&self) -> Vec<String> {
        self.devices.iter().map(|d| d.name().to_string()).collect()
    }

    pub fn get_device(&self, name: &str) -> Option<&Device> {
        self.devices.iter().find(|d| d.name() == name)
    }

    pub fn get_device_mut(&mut self, name: &str) -> Option<&mut Device> {
        self.devices.iter_mut().find(|d| d.name() == name)
    }

    pub fn get_device_info(&self, name: Option<&str>) -> Vec<DeviceInfo> {
        match name {
            Some(n) => self
                .get_device(n)
                .map(|d| vec![Self::build_info(d)])
                .unwrap_or_default(),
            None => self.devices.iter().map(Self::build_info).collect(),
        }
    }

    fn build_info(d: &Device) -> DeviceInfo {
        let serial_number = d.serial_number();
        let has_serial_number = d.is_connected() && serial_number.is_some();
        DeviceInfo {
            name: d.name().to_string(),
            device_type: d.device_type().to_string(),
            device_class: d.device_class().to_string(),
            is_connected: d.is_connected(),
            is_reading: d.is_connected() && d.is_reading(),
            is_gpi_trigger_on: d.is_gpi_trigger_on(),
            can_print: d.can_print(),
            to_print: d.pending_print_jobs(),
            has_serial_number,
            serial_number: serial_number.unwrap_or_else(|| "Unknown".to_string()),
            connect_instruction: d.connect_instruction(),
            current_config: d.to_map(),
        }
    }

    pub fn any_device_reading(&self) -> bool {
        self.devices
            .iter()
            .any(|d| d.is_connected() && d.is_reading())
    }

    pub fn get_serial_number(&self, name: &str) -> Option<String> {
        let d = self.get_device(name)?;
        if !d.is_connected() {
            return None;
        }
        d.serial_number()
    }

    pub fn get_device_config(&self, name: &str) -> Option<HashMap<String, Value>> {
        self.get_device(name).map(Device::to_map)
    }

    pub fn get_device_configs(&self) -> HashMap<String, HashMap<String, Value>> {
        self.devices
            .iter()
            .map(|d| (d.name().to_string(), d.to_map()))
            .collect()
    }

    pub async fn start_inventory(&self, name: &str) -> Result<(), String> {
        let d = self
            .get_device(name)
            .ok_or_else(|| format!("device '{}' not found", name))?;
        if !d.is_connected() {
            return Err(format!("device '{}' is not connected", name));
        }
        d.start_inventory().await.map_err(|e| {
            eprintln!("❌ start_inventory '{}': {e}", name);
            e
        })
    }

    pub async fn stop_inventory(&self, name: &str) -> Result<(), String> {
        let d = self
            .get_device(name)
            .ok_or_else(|| format!("device '{}' not found", name))?;
        if !d.is_connected() {
            return Err(format!("device '{}' is not connected", name));
        }
        d.stop_inventory().await.map_err(|e| {
            eprintln!("❌ stop_inventory '{}': {e}", name);
            e
        })
    }

    pub async fn start_inventory_all(&self) -> HashMap<String, bool> {
        let mut results = HashMap::new();
        for d in &self.devices {
            if d.is_connected() {
                let ok = d.start_inventory().await.is_ok();
                results.insert(d.name().to_string(), ok);
            }
        }
        results
    }

    pub async fn stop_inventory_all(&self) -> HashMap<String, bool> {
        let mut results = HashMap::new();
        for d in &self.devices {
            if d.is_connected() {
                let ok = d.stop_inventory().await.is_ok();
                results.insert(d.name().to_string(), ok);
            }
        }
        results
    }

    pub async fn write_epc(
        &self,
        name: &str,
        target_identifier: Option<&str>,
        target_value: Option<&str>,
        new_epc: &str,
        password: &str,
    ) -> Result<(), String> {
        let d = self
            .get_device(name)
            .ok_or_else(|| format!("device '{}' not found", name))?;
        if !d.is_connected() {
            return Err(format!("device '{}' is not connected", name));
        }
        d.write_epc(target_identifier, target_value, new_epc, password)
            .await
    }

    pub async fn write_gpo(
        &self,
        name: &str,
        pin: u8,
        state: bool,
        control: &str,
        time_ms: u64,
    ) -> Result<(), String> {
        let d = self
            .get_device(name)
            .ok_or_else(|| format!("device '{}' not found", name))?;
        if !d.is_connected() {
            return Err(format!("device '{}' is not connected", name));
        }
        d.write_gpo(pin, state, control, time_ms).await
    }

    pub fn get_config_examples() -> Vec<&'static str> {
        CONFIG_EXAMPLES.iter().map(|(name, _)| *name).collect()
    }

    pub fn get_config_example(name: &str) -> Option<HashMap<String, Value>> {
        CONFIG_EXAMPLES
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, f)| f())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn device_info_includes_runtime_and_config_fields() {
        let mut manager = DeviceManager::new("/tmp/unused");
        manager.add_device(
            "dock-reader",
            "X714",
            HashMap::from([
                ("reader".to_string(), Value::String("X714".to_string())),
                (
                    "connection_type".to_string(),
                    Value::String("TCP".to_string()),
                ),
                ("ip".to_string(), Value::String("192.168.1.50".to_string())),
                ("tcp_port".to_string(), json!(23)),
                ("gpi_start".to_string(), Value::Bool(true)),
            ]),
        );

        let info = manager
            .get_device_info(Some("dock-reader"))
            .into_iter()
            .next()
            .expect("device info");

        assert_eq!(info.name, "dock-reader");
        assert_eq!(info.device_type, "X714");
        assert_eq!(info.device_class, "X714");
        assert!(!info.is_connected);
        assert!(!info.is_reading);
        assert!(info.is_gpi_trigger_on);
        assert!(!info.can_print);
        assert_eq!(info.to_print, 0);
        assert!(!info.has_serial_number);
        assert_eq!(info.serial_number, "Unknown");
        assert_eq!(
            info.current_config
                .get("connection_type")
                .and_then(Value::as_str),
            Some("TCP")
        );
    }

    #[test]
    fn config_examples_list_only_supported_devices() {
        let examples = DeviceManager::get_config_examples();
        assert!(examples.contains(&"X714_DEFAULT"));
        assert_eq!(examples.len(), 15);
    }
}
