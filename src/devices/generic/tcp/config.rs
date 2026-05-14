use std::collections::HashMap;

use serde_json::{Number, Value};

pub type ParamMap = HashMap<String, Value>;

pub const DEFAULT_CONFIG_JSON: &str =
    r#"{"name":"TCP","ip":"192.168.99.202","port":23,"reconnection_time":3}"#;

#[derive(Debug, Clone)]
pub struct TcpDeviceConfig {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub reconnection_time: u64,
}

impl Default for TcpDeviceConfig {
    fn default() -> Self {
        let params: ParamMap =
            serde_json::from_str(DEFAULT_CONFIG_JSON).expect("DEFAULT_CONFIG_JSON is valid");
        Self::from_map(params)
    }
}

impl TcpDeviceConfig {
    fn base() -> Self {
        Self {
            name: "TCP".to_string(),
            ip: "192.168.99.202".to_string(),
            port: 23,
            reconnection_time: 3,
        }
    }

    pub fn from_map(params: ParamMap) -> Self {
        let mut config = Self::base();
        if let Some(v) = params.get("name").and_then(Value::as_str) {
            config.name = v.to_string();
        }
        if let Some(v) = params.get("ip").and_then(Value::as_str) {
            config.ip = v.to_string();
        }
        if let Some(v) = params.get("host").and_then(Value::as_str) {
            config.ip = v.to_string();
        }
        if let Some(v) = params
            .get("port")
            .and_then(Value::as_u64)
            .and_then(|v| u16::try_from(v).ok())
        {
            config.port = v;
        }
        if let Some(v) = params.get("reconnection_time").and_then(Value::as_u64) {
            config.reconnection_time = v;
        }
        config
    }

    pub fn to_map(&self) -> ParamMap {
        let mut out = HashMap::new();
        out.insert("name".to_string(), Value::String(self.name.clone()));
        out.insert("ip".to_string(), Value::String(self.ip.clone()));
        out.insert("port".to_string(), Value::Number(Number::from(self.port)));
        out.insert(
            "reconnection_time".to_string(),
            Value::Number(Number::from(self.reconnection_time)),
        );
        out
    }
}
