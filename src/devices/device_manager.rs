//! `DeviceManager` — loads and manages multiple RFID and generic devices.
//!
//! Each device is configured via a `.json` file in a directory.
//! The `"reader"` field determines the type.
//! All other fields are optional — defaults are applied automatically.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use serde_json::Value;
use tokio::task::JoinHandle;

use super::rfid::r700::R700;
use super::rfid::x714::X714;
use super::rfid::acupad::Acupad;
use super::generic::serial::SerialDevice;
use super::generic::tcp::TcpDevice;
use super::printer::sato::{SatoPrinter, SatoWs4Printer};

pub type EventHandler = dyn FnMut(&str, &str, Option<Value>) + Send + 'static;
pub type SharedEventHandler = Arc<Mutex<Box<EventHandler>>>;

static CONFIG_EXAMPLES: &[(&str, fn() -> HashMap<String, Value>)] = &[
    ("XPAD", || super::rfid::x714::config_example::xpad_map()),
    ("X714_SERIAL", || super::rfid::x714::config_example::x714_map()),
    ("X714_TCP", || super::rfid::x714::config_example::x714_tcp_map()),
    ("X714_BLE", || super::rfid::x714::config_example::x714_ble_map()),
    ("X714_ALL", || super::rfid::x714::config_example::x714_all_map()),
    ("R700_IOT", || super::rfid::r700::config_example::r700_iot_map()),
    ("R700_IOT_DICT", || super::rfid::r700::config_example::r700_iot_dict_map()),
    ("R700_IOT_GPI", || super::rfid::r700::config_example::r700_iot_gpi_map()),
    ("R700_PROTECTED_INVENTORY", || super::rfid::r700::config_example::r700_protected_inventory_map()),
    ("ACUPAD", || super::rfid::acupad::config_example::acupad_default_map()),
    ("ACUPAD_BASIC", || super::rfid::acupad::config_example::acupad_basic_map()),
    ("SERIAL", || super::generic::serial::config_example::serial_default_map()),
    ("SERIAL_CUSTOM", || super::generic::serial::config_example::serial_custom_map()),
    ("TCP", || super::generic::tcp::config_example::tcp_default_map()),
    ("TCP_CUSTOM", || super::generic::tcp::config_example::tcp_custom_map()),
    ("SATO", || super::printer::sato::config_example::sato_default_map()),
    ("SATO_WS4", || super::printer::sato::config_example::sato_ws4_map()),
];

pub enum Device {
    X714(X714),
    R700(R700),
    Acupad(Acupad),
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
            Self::Acupad(d) => Self::Acupad(d.clone()),
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
            Self::Acupad(d) => &d.config.name,
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
            Self::Acupad(_) => "ACUPAD",
            Self::Serial(_) => "SERIAL",
            Self::Tcp(_) => "TCP",
            Self::Sato(_) => "SATO",
            Self::SatoWs4(_) => "SATO_WS4",
        }
    }

    pub fn is_connected(&self) -> bool {
        match self {
            Self::X714(d) => d.is_connected(),
            Self::R700(d) => d.is_connected(),
            Self::Acupad(d) => d.is_connected(),
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
            Self::Acupad(d) => d.is_reading(),
            Self::Serial(_) => false,
            Self::Tcp(_) => false,
            Self::Sato(_) => false,
            Self::SatoWs4(_) => false,
        }
    }

    pub fn serial_number(&self) -> Option<String> {
        match self {
            Self::X714(d) => d.serial_number(),
            Self::R700(d) => d.serial_number(),
            Self::Acupad(d) => d.serial_number(),
            Self::Serial(_) => None,
            Self::Tcp(_) => None,
            Self::Sato(_) => None,
            Self::SatoWs4(_) => None,
        }
    }

    pub fn connect_instruction(&self) -> String {
        match self {
            Self::X714(d) => d.connect_instruction(),
            Self::R700(d) => d.connect_instruction(),
            Self::Acupad(d) => d.connect_instruction(),
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
            Self::Acupad(d) => d.set_event_handler(handler),
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
            Self::Acupad(d) => d.connect().await,
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
            Self::Acupad(d) => d.close().await,
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
            Self::Acupad(d) => d.start_inventory().await,
            _ => Err("device type does not support this operation".to_string()),
        }
    }

    pub async fn stop_inventory(&self) -> Result<(), String> {
        match self {
            Self::X714(d) => d.stop_inventory().await,
            Self::R700(d) => d.stop_inventory().await,
            Self::Acupad(d) => d.stop_inventory().await,
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
            Self::X714(d) => d.write_epc(target_identifier, target_value, new_epc, password).await,
            Self::R700(d) => d.write_epc(target_identifier, target_value, new_epc, password).await,
            Self::Acupad(d) => d.write_epc(target_identifier, target_value, new_epc, password).await,
            _ => Err("device type does not support this operation".to_string()),
        }
    }

    pub async fn write_gpo(&self, pin: u8, state: bool, control: &str, time_ms: u64) -> Result<(), String> {
        match self {
            Self::X714(d) => d.write_gpo(pin, state, control, time_ms).await,
            Self::R700(d) => d.write_gpo(pin, state, control, time_ms as u32).await,
            Self::Acupad(d) => d.write_gpo(pin, state, control, time_ms).await,
            _ => Err("device type does not support this operation".to_string()),
        }
    }
}

/// Snapshot of device state (does not hold a reference to the device).
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub device_type: String,
    pub is_connected: bool,
    pub is_reading: bool,
    pub serial_number: Option<String>,
    pub connect_instruction: String,
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
                    eprintln!("❌ Could not create directory '{}': {e}", self.devices_path.display());
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

            let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let name = filename.trim_end_matches(".json").to_string();

            eprintln!("📄 Reading '{}'…", filename);

            let content = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => { eprintln!("❌ Error reading '{}': {e}", filename); continue; }
            };

            let raw: HashMap<String, Value> = match serde_json::from_str(&content) {
                Ok(d) => d,
                Err(e) => { eprintln!("❌ Invalid JSON in '{}': {e}", filename); continue; }
            };

            let data: HashMap<String, Value> = raw.into_iter().map(|(k, v)| (k.to_lowercase(), v)).collect();

            let reader_type = match data.get("reader").and_then(|v| v.as_str()) {
                Some(t) => t.to_string(),
                None => { eprintln!("⚠️  '{}' has no 'reader' field — skipped", filename); continue; }
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
                Ok(d) => { eprintln!("  ✅ X714 '{}' → {}", name, d.connect_instruction()); self.devices.push(Device::X714(d)); }
                Err(e) => eprintln!("  ❌ X714 '{}' config error: {e}", name),
            },
            "R700_IOT" | "R700" => match R700::from_map(data) {
                Ok(d) => { eprintln!("  ✅ R700 '{}' → {}", name, d.connect_instruction()); self.devices.push(Device::R700(d)); }
                Err(e) => eprintln!("  ❌ R700 '{}' config error: {e}", name),
            },
            "ACUPAD" => match Acupad::from_map(data) {
                Ok(d) => { eprintln!("  ✅ ACUPAD '{}' → {}", name, d.connect_instruction()); self.devices.push(Device::Acupad(d)); }
                Err(e) => eprintln!("  ❌ ACUPAD '{}' config error: {e}", name),
            },
            "SERIAL" => {
                let d = SerialDevice::from_map(data);
                eprintln!("  ✅ SERIAL '{}' → {}", name, d.connect_instruction());
                self.devices.push(Device::Serial(d));
            },
            "TCP" => {
                let d = TcpDevice::from_map(data);
                eprintln!("  ✅ TCP '{}' → {}", name, d.connect_instruction());
                self.devices.push(Device::Tcp(d));
            },
            "SATO" => {
                let d = SatoPrinter::from_map(data);
                eprintln!("  ✅ SATO '{}' → {}", name, d.connect_instruction());
                self.devices.push(Device::Sato(d));
            },
            "SATO_WS4" => {
                let d = SatoWs4Printer::from_map(data);
                eprintln!("  ✅ SATO_WS4 '{}' → {}", name, d.connect_instruction());
                self.devices.push(Device::SatoWs4(d));
            },
            other => eprintln!("  ⚠️  Unknown type '{}' for '{}' — skipped", other, name),
        }
    }

    pub fn assign_event_handler(&mut self) {
        let Some(handler) = &self.event_handler else { return; };
        for device in &mut self.devices {
            device.set_event_handler(Arc::clone(handler));
        }
    }

    pub async fn connect_devices(&mut self, force: bool) {
        let active = self.connect_tasks.iter().filter(|t| !t.is_finished()).count();

        if active > 0 && !force {
            eprintln!("ℹ️  {} active connection task(s) — use force=true to restart", active);
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

    pub fn len(&self) -> usize { self.devices.len() }
    pub fn is_empty(&self) -> bool { self.devices.is_empty() }

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
            Some(n) => self.get_device(n).map(|d| vec![Self::build_info(d)]).unwrap_or_default(),
            None => self.devices.iter().map(Self::build_info).collect(),
        }
    }

    fn build_info(d: &Device) -> DeviceInfo {
        DeviceInfo {
            name: d.name().to_string(),
            device_type: d.device_type().to_string(),
            is_connected: d.is_connected(),
            is_reading: d.is_reading(),
            serial_number: d.serial_number(),
            connect_instruction: d.connect_instruction(),
        }
    }

    pub fn any_device_reading(&self) -> bool {
        self.devices.iter().any(|d| d.is_connected() && d.is_reading())
    }

    pub fn get_serial_number(&self, name: &str) -> Option<String> {
        let d = self.get_device(name)?;
        if !d.is_connected() { return None; }
        d.serial_number()
    }

    pub async fn start_inventory(&self, name: &str) -> Result<(), String> {
        let d = self.get_device(name).ok_or_else(|| format!("device '{}' not found", name))?;
        if !d.is_connected() { return Err(format!("device '{}' is not connected", name)); }
        d.start_inventory().await.map_err(|e| { eprintln!("❌ start_inventory '{}': {e}", name); e })
    }

    pub async fn stop_inventory(&self, name: &str) -> Result<(), String> {
        let d = self.get_device(name).ok_or_else(|| format!("device '{}' not found", name))?;
        if !d.is_connected() { return Err(format!("device '{}' is not connected", name)); }
        d.stop_inventory().await.map_err(|e| { eprintln!("❌ stop_inventory '{}': {e}", name); e })
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

    pub async fn write_epc(&self, name: &str, target_identifier: Option<&str>, target_value: Option<&str>, new_epc: &str, password: &str) -> Result<(), String> {
        let d = self.get_device(name).ok_or_else(|| format!("device '{}' not found", name))?;
        if !d.is_connected() { return Err(format!("device '{}' is not connected", name)); }
        d.write_epc(target_identifier, target_value, new_epc, password).await
    }

    pub async fn write_gpo(&self, name: &str, pin: u8, state: bool, control: &str, time_ms: u64) -> Result<(), String> {
        let d = self.get_device(name).ok_or_else(|| format!("device '{}' not found", name))?;
        if !d.is_connected() { return Err(format!("device '{}' is not connected", name)); }
        d.write_gpo(pin, state, control, time_ms).await
    }

    pub fn get_config_examples() -> Vec<&'static str> {
        CONFIG_EXAMPLES.iter().map(|(name, _)| *name).collect()
    }

    pub fn get_config_example(name: &str) -> Option<HashMap<String, Value>> {
        CONFIG_EXAMPLES.iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, f)| f())
    }
}
