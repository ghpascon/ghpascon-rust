use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};

pub type ParamMap = HashMap<String, Value>;

// ── Default configuration ─────────────────────────────────────────────────────
//
// All default values for X714Config live here.
// Edit this JSON to change defaults used by X714Config::default() and
// as the base for X714Config::from_map().
//
pub const DEFAULT_CONFIG_JSON: &str = r#"{
    "name":             "X714",
    "connection_type":  "SERIAL",

    "port":     "AUTO",
    "baudrate":  115200,
    "vid":       1,
    "pid":       1,

    "ip":       "192.168.1.100",
    "tcp_port":  23,

    "ble_name":         "SMTX",
    "ble_service_uuid": "6E400001-B5A3-F393-E0A9-E50E24DCCA9E",
    "ble_rx_uuid":      "6E400002-B5A3-F393-E0A9-E50E24DCCA9E",
    "ble_tx_uuid":      "6E400003-B5A3-F393-E0A9-E50E24DCCA9E",

    "buzzer":        false,
    "session":       1,
    "start_reading": false,
    "gpi_start":     false,
    "always_send":   true,
    "simple_send":   false,
    "keyboard":      false,
    "decode_gtin":   false,
    "hotspot":       true,

    "reconnection_time":            3,
    "prefix":                       "",
    "protected_inventory_active":   false,
    "protected_inventory_password": "12345678",

    "active_ant": [1],
    "read_power":  22,
    "read_rssi":   -120
}"#;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConnectionType {
    Serial,
    Tcp,
    Ble,
}

impl ConnectionType {
    pub fn from_str(value: &str) -> Self {
        match value.trim().to_uppercase().as_str() {
            "TCP" => Self::Tcp,
            "BLE" => Self::Ble,
            _ => Self::Serial,
        }
    }

    pub fn as_wire_str(&self) -> &'static str {
        match self {
            Self::Serial => "SERIAL",
            Self::Tcp => "TCP",
            Self::Ble => "BLE",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SerialConfig {
    pub port: String,
    pub baudrate: u32,
    pub vid: u16,
    pub pid: u16,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            port: "AUTO".to_string(),
            baudrate: 115_200,
            vid: 1,
            pid: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TcpConfig {
    pub ip: String,
    pub port: u16,
}

impl Default for TcpConfig {
    fn default() -> Self {
        Self {
            ip: "192.168.1.100".to_string(),
            port: 23,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BleConfig {
    pub name: String,
    pub address: Option<String>,
    pub service_uuid: String,
    pub rx_uuid: String,
    pub tx_uuid: String,
}

impl Default for BleConfig {
    fn default() -> Self {
        Self {
            name: "SMTX".to_string(),
            address: None,
            service_uuid: "6E400001-B5A3-F393-E0A9-E50E24DCCA9E".to_string(),
            rx_uuid: "6E400002-B5A3-F393-E0A9-E50E24DCCA9E".to_string(),
            tx_uuid: "6E400003-B5A3-F393-E0A9-E50E24DCCA9E".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AntennaConfig {
    pub active: bool,
    pub power: i32,
    pub rssi: i32,
}

impl Default for AntennaConfig {
    fn default() -> Self {
        Self {
            active: true,
            power: 22,
            rssi: -120,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X714Config {
    pub name: String,
    pub connection_type: ConnectionType,
    pub serial: SerialConfig,
    pub tcp: TcpConfig,
    pub ble: BleConfig,
    pub buzzer: bool,
    pub session: u8,
    pub start_reading: bool,
    pub gpi_start: bool,
    pub always_send: bool,
    pub simple_send: bool,
    pub keyboard: bool,
    pub decode_gtin: bool,
    pub hotspot: bool,
    pub reconnection_time: u64,
    pub prefix: String,
    pub protected_inventory_active: bool,
    pub protected_inventory_password: String,
    pub ant_dict: BTreeMap<String, AntennaConfig>,
}

impl Default for X714Config {
    fn default() -> Self {
        let params: ParamMap =
            serde_json::from_str(DEFAULT_CONFIG_JSON).expect("DEFAULT_CONFIG_JSON is valid JSON");
        Self::from_map(params).expect("DEFAULT_CONFIG_JSON produces a valid config")
    }
}

impl X714Config {
    /// Hard-coded base config used as the starting point for `from_map`.
    /// These values are the fallback for any key absent from the caller's map.
    /// Should match `DEFAULT_CONFIG_JSON` – edit both together.
    fn base() -> Self {
        let mut ant_dict = BTreeMap::new();
        for ant in [1_u8, 2, 3, 4] {
            ant_dict.insert(
                ant.to_string(),
                AntennaConfig {
                    active: ant == 1,
                    power: 22,
                    rssi: -120,
                },
            );
        }
        Self {
            name: "X714".to_string(),
            connection_type: ConnectionType::Serial,
            serial: SerialConfig::default(),
            tcp: TcpConfig::default(),
            ble: BleConfig::default(),
            buzzer: false,
            session: 1,
            start_reading: false,
            gpi_start: false,
            always_send: true,
            simple_send: false,
            keyboard: false,
            decode_gtin: false,
            hotspot: true,
            reconnection_time: 3,
            prefix: String::new(),
            protected_inventory_active: false,
            protected_inventory_password: "12345678".to_string(),
            ant_dict,
        }
    }

    pub fn from_map(params: ParamMap) -> Result<Self, String> {
        let mut config = Self::base(); // use base() to avoid Default<->from_map recursion

        if let Some(v) = get_string(&params, "name") {
            config.name = v;
        }
        if let Some(v) = get_string(&params, "connection_type") {
            config.connection_type = ConnectionType::from_str(&v);
        }

        if let Some(v) = get_string(&params, "port") {
            config.serial.port = v;
        }
        if let Some(v) = get_u32(&params, "baudrate") {
            config.serial.baudrate = v;
        }
        if let Some(v) = get_u16(&params, "vid") {
            config.serial.vid = v;
        }
        if let Some(v) = get_u16(&params, "pid") {
            config.serial.pid = v;
        }

        if let Some(v) = get_string(&params, "ip") {
            config.tcp.ip = v;
        }
        if let Some(v) = get_u16(&params, "tcp_port") {
            config.tcp.port = v;
        }

        if let Some(v) = get_string(&params, "ble_name") {
            config.ble.name = v;
        }
        if let Some(v) = get_string(&params, "ble_address") {
            config.ble.address = Some(v);
        }
        if let Some(v) = get_string(&params, "ble_service_uuid") {
            config.ble.service_uuid = v;
        }
        if let Some(v) = get_string(&params, "ble_rx_uuid") {
            config.ble.rx_uuid = v;
        }
        if let Some(v) = get_string(&params, "ble_tx_uuid") {
            config.ble.tx_uuid = v;
        }

        set_bool(&params, "buzzer", &mut config.buzzer);
        if let Some(v) = get_u8(&params, "session") {
            config.session = v.clamp(0, 3);
        }
        set_bool(&params, "start_reading", &mut config.start_reading);
        set_bool(&params, "gpi_start", &mut config.gpi_start);
        set_bool(&params, "always_send", &mut config.always_send);
        set_bool(&params, "simple_send", &mut config.simple_send);
        set_bool(&params, "keyboard", &mut config.keyboard);
        set_bool(&params, "decode_gtin", &mut config.decode_gtin);
        set_bool(&params, "hotspot", &mut config.hotspot);

        if let Some(v) = get_u64(&params, "reconnection_time") {
            config.reconnection_time = v;
        }
        if let Some(v) = get_string(&params, "prefix") {
            config.prefix = normalize_hex_prefix(&v);
        }

        set_bool(
            &params,
            "protected_inventory_active",
            &mut config.protected_inventory_active,
        );
        if let Some(v) = get_string(&params, "protected_inventory_password") {
            config.protected_inventory_password = v;
        }

        if let Some(raw_ant) = params.get("ant_dict") {
            config.ant_dict = parse_ant_dict(raw_ant)?;
        } else {
            let active_ant = get_u8_vec(&params, "active_ant").unwrap_or_else(|| vec![1]);
            let read_power = get_i32(&params, "read_power").unwrap_or(22);
            let read_rssi = get_i32(&params, "read_rssi").unwrap_or(-120);
            for ant in [1_u8, 2, 3, 4] {
                config.ant_dict.insert(
                    ant.to_string(),
                    AntennaConfig {
                        active: active_ant.contains(&ant),
                        power: read_power,
                        rssi: read_rssi,
                    },
                );
            }
        }

        if config.gpi_start {
            config.start_reading = false;
        }

        Ok(config)
    }

    pub fn to_map(&self) -> ParamMap {
        let mut out = HashMap::new();
        out.insert("name".to_string(), Value::String(self.name.clone()));
        out.insert(
            "connection_type".to_string(),
            Value::String(self.connection_type.as_wire_str().to_string()),
        );
        out.insert("port".to_string(), Value::String(self.serial.port.clone()));
        out.insert(
            "baudrate".to_string(),
            Value::Number(Number::from(self.serial.baudrate)),
        );
        out.insert(
            "vid".to_string(),
            Value::Number(Number::from(self.serial.vid)),
        );
        out.insert(
            "pid".to_string(),
            Value::Number(Number::from(self.serial.pid)),
        );
        out.insert("ip".to_string(), Value::String(self.tcp.ip.clone()));
        out.insert(
            "tcp_port".to_string(),
            Value::Number(Number::from(self.tcp.port)),
        );
        out.insert("ble_name".to_string(), Value::String(self.ble.name.clone()));
        out.insert(
            "ble_address".to_string(),
            self.ble
                .address
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        out.insert(
            "ble_service_uuid".to_string(),
            Value::String(self.ble.service_uuid.clone()),
        );
        out.insert(
            "ble_rx_uuid".to_string(),
            Value::String(self.ble.rx_uuid.clone()),
        );
        out.insert(
            "ble_tx_uuid".to_string(),
            Value::String(self.ble.tx_uuid.clone()),
        );
        out.insert("buzzer".to_string(), Value::Bool(self.buzzer));
        out.insert(
            "session".to_string(),
            Value::Number(Number::from(self.session)),
        );
        out.insert("start_reading".to_string(), Value::Bool(self.start_reading));
        out.insert("gpi_start".to_string(), Value::Bool(self.gpi_start));
        out.insert("always_send".to_string(), Value::Bool(self.always_send));
        out.insert("simple_send".to_string(), Value::Bool(self.simple_send));
        out.insert("keyboard".to_string(), Value::Bool(self.keyboard));
        out.insert("decode_gtin".to_string(), Value::Bool(self.decode_gtin));
        out.insert("hotspot".to_string(), Value::Bool(self.hotspot));
        out.insert(
            "reconnection_time".to_string(),
            Value::Number(Number::from(self.reconnection_time)),
        );
        out.insert("prefix".to_string(), Value::String(self.prefix.clone()));
        out.insert(
            "protected_inventory_active".to_string(),
            Value::Bool(self.protected_inventory_active),
        );
        out.insert(
            "protected_inventory_password".to_string(),
            Value::String(self.protected_inventory_password.clone()),
        );
        out.insert("ant_dict".to_string(), ant_dict_to_value(&self.ant_dict));
        out
    }
}

pub fn ant_dict_to_value(ant_dict: &BTreeMap<String, AntennaConfig>) -> Value {
    let mut outer = Map::new();
    for (k, v) in ant_dict {
        let mut ant = Map::new();
        ant.insert("active".to_string(), Value::Bool(v.active));
        ant.insert("power".to_string(), Value::Number(Number::from(v.power)));
        ant.insert("rssi".to_string(), Value::Number(Number::from(v.rssi)));
        outer.insert(k.clone(), Value::Object(ant));
    }
    Value::Object(outer)
}

pub fn parse_ant_dict(value: &Value) -> Result<BTreeMap<String, AntennaConfig>, String> {
    let Some(obj) = value.as_object() else {
        return Err("ant_dict must be an object".to_string());
    };

    let mut out = BTreeMap::new();
    for (k, v) in obj {
        let Some(ant) = v.as_object() else {
            return Err(format!("ant_dict['{k}'] must be an object"));
        };
        let active = ant.get("active").and_then(Value::as_bool).unwrap_or(false);
        let power = ant
            .get("power")
            .and_then(Value::as_i64)
            .and_then(|v| i32::try_from(v).ok())
            .unwrap_or(22);
        let rssi = ant
            .get("rssi")
            .and_then(Value::as_i64)
            .and_then(|v| i32::try_from(v).ok())
            .unwrap_or(-120);
        out.insert(
            k.clone(),
            AntennaConfig {
                active,
                power,
                rssi,
            },
        );
    }

    if out.is_empty() {
        return Err("ant_dict must not be empty".to_string());
    }

    Ok(out)
}

pub fn normalize_hex_prefix(value: &str) -> String {
    let trimmed = value.trim().to_lowercase();
    if trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        trimmed
    } else {
        String::new()
    }
}

fn get_string(params: &ParamMap, key: &str) -> Option<String> {
    params.get(key).and_then(Value::as_str).map(str::to_string)
}

fn get_bool(params: &ParamMap, key: &str) -> Option<bool> {
    params.get(key).and_then(Value::as_bool)
}

fn set_bool(params: &ParamMap, key: &str, target: &mut bool) {
    if let Some(v) = get_bool(params, key) {
        *target = v;
    }
}

fn get_u8(params: &ParamMap, key: &str) -> Option<u8> {
    params
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|v| u8::try_from(v).ok())
}

fn get_u16(params: &ParamMap, key: &str) -> Option<u16> {
    params
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|v| u16::try_from(v).ok())
}

fn get_u32(params: &ParamMap, key: &str) -> Option<u32> {
    params
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
}

fn get_u64(params: &ParamMap, key: &str) -> Option<u64> {
    params.get(key).and_then(Value::as_u64)
}

fn get_i32(params: &ParamMap, key: &str) -> Option<i32> {
    params
        .get(key)
        .and_then(Value::as_i64)
        .and_then(|v| i32::try_from(v).ok())
}

fn get_u8_vec(params: &ParamMap, key: &str) -> Option<Vec<u8>> {
    let values = params.get(key)?.as_array()?;
    Some(
        values
            .iter()
            .filter_map(|v| v.as_u64().and_then(|n| u8::try_from(n).ok()))
            .collect::<Vec<_>>(),
    )
}
