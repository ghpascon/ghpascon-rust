use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};

pub type ParamMap = HashMap<String, Value>;

pub const DEFAULT_CONFIG_JSON: &str = r#"{
    "name":             "ACUPAD",
    "port":     "AUTO",
    "baudrate":  115200,
    "vid":       260,
    "pid":       24656,
    "beep":      false,
    "session":   1,
    "start_reading": false,
    "gpi_start":     false,
    "reconnection_time": 3,
    "protected_inventory_active":   false,
    "protected_inventory_password": "12345678",
    "active_ant": [1],
    "read_power":  22,
    "read_rssi":   -120
}"#;

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
pub struct AcupadConfig {
    pub name: String,
    pub port: String,
    pub baudrate: u32,
    pub vid: u16,
    pub pid: u16,
    pub beep: bool,
    pub session: u8,
    pub start_reading: bool,
    pub gpi_start: bool,
    pub reconnection_time: u64,
    pub protected_inventory_active: bool,
    pub protected_inventory_password: String,
    pub ant_dict: BTreeMap<String, AntennaConfig>,
}

impl Default for AcupadConfig {
    fn default() -> Self {
        let params: ParamMap =
            serde_json::from_str(DEFAULT_CONFIG_JSON).expect("DEFAULT_CONFIG_JSON is valid JSON");
        Self::from_map(params).expect("DEFAULT_CONFIG_JSON produces a valid config")
    }
}

impl AcupadConfig {
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
            name: "ACUPAD".to_string(),
            port: "AUTO".to_string(),
            baudrate: 115_200,
            vid: 260,
            pid: 24656,
            beep: false,
            session: 1,
            start_reading: false,
            gpi_start: false,
            reconnection_time: 3,
            protected_inventory_active: false,
            protected_inventory_password: "12345678".to_string(),
            ant_dict,
        }
    }

    pub fn from_map(params: ParamMap) -> Result<Self, String> {
        let mut config = Self::base();

        if let Some(v) = get_string(&params, "name") {
            config.name = v;
        }
        if let Some(v) = get_string(&params, "port") {
            config.port = v;
        }
        if let Some(v) = get_u32(&params, "baudrate") {
            config.baudrate = v;
        }
        if let Some(v) = get_u16(&params, "vid") {
            config.vid = v;
        }
        if let Some(v) = get_u16(&params, "pid") {
            config.pid = v;
        }
        set_bool(&params, "beep", &mut config.beep);
        if let Some(v) = get_u8(&params, "session") {
            config.session = v.clamp(0, 3);
        }
        set_bool(&params, "start_reading", &mut config.start_reading);
        set_bool(&params, "gpi_start", &mut config.gpi_start);
        if let Some(v) = get_u64(&params, "reconnection_time") {
            config.reconnection_time = v;
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
        out.insert("port".to_string(), Value::String(self.port.clone()));
        out.insert(
            "baudrate".to_string(),
            Value::Number(Number::from(self.baudrate)),
        );
        out.insert("vid".to_string(), Value::Number(Number::from(self.vid)));
        out.insert("pid".to_string(), Value::Number(Number::from(self.pid)));
        out.insert("beep".to_string(), Value::Bool(self.beep));
        out.insert(
            "session".to_string(),
            Value::Number(Number::from(self.session)),
        );
        out.insert("start_reading".to_string(), Value::Bool(self.start_reading));
        out.insert("gpi_start".to_string(), Value::Bool(self.gpi_start));
        out.insert(
            "reconnection_time".to_string(),
            Value::Number(Number::from(self.reconnection_time)),
        );
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
        out.insert(k.clone(), AntennaConfig { active, power, rssi });
    }

    if out.is_empty() {
        return Err("ant_dict must not be empty".to_string());
    }

    Ok(out)
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
