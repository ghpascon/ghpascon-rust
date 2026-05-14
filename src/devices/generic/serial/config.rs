use std::collections::HashMap;

use serde_json::{Number, Value};

pub type ParamMap = HashMap<String, Value>;

pub const DEFAULT_CONFIG_JSON: &str =
    r#"{"name":"SERIAL","port":"AUTO","baudrate":9600,"vid":259,"pid":24673,"reconnection_time":3}"#;

#[derive(Debug, Clone)]
pub struct SerialDeviceConfig {
    pub name: String,
    pub port: String,
    pub baudrate: u32,
    pub vid: u16,
    pub pid: u16,
    pub reconnection_time: u64,
}

impl Default for SerialDeviceConfig {
    fn default() -> Self {
        let params: ParamMap =
            serde_json::from_str(DEFAULT_CONFIG_JSON).expect("DEFAULT_CONFIG_JSON is valid");
        Self::from_map(params)
    }
}

impl SerialDeviceConfig {
    fn base() -> Self {
        Self {
            name: "SERIAL".to_string(),
            port: "AUTO".to_string(),
            baudrate: 9600,
            vid: 259,
            pid: 24673,
            reconnection_time: 3,
        }
    }

    pub fn from_map(params: ParamMap) -> Self {
        let mut config = Self::base();
        if let Some(v) = params.get("name").and_then(Value::as_str) {
            config.name = v.to_string();
        }
        if let Some(v) = params.get("port").and_then(Value::as_str) {
            config.port = v.to_string();
        }
        if let Some(v) = params
            .get("baudrate")
            .and_then(Value::as_u64)
            .and_then(|v| u32::try_from(v).ok())
        {
            config.baudrate = v;
        }
        if let Some(v) = params
            .get("vid")
            .and_then(Value::as_u64)
            .and_then(|v| u16::try_from(v).ok())
        {
            config.vid = v;
        }
        if let Some(v) = params
            .get("pid")
            .and_then(Value::as_u64)
            .and_then(|v| u16::try_from(v).ok())
        {
            config.pid = v;
        }
        if let Some(v) = params.get("reconnection_time").and_then(Value::as_u64) {
            config.reconnection_time = v;
        }
        config
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
        out.insert(
            "reconnection_time".to_string(),
            Value::Number(Number::from(self.reconnection_time)),
        );
        out
    }
}
