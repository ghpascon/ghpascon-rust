use serde_json::{Value, json};

use super::config::ParamMap;

fn json_to_map(v: Value) -> ParamMap {
    match v {
        Value::Object(m) => m.into_iter().collect(),
        _ => Default::default(),
    }
}

pub fn tcp_default_map() -> ParamMap {
    json_to_map(json!({
        "reader": "TCP", "ip": "192.168.99.202", "port": 23, "reconnection_time": 3
    }))
}

pub fn tcp_custom_map() -> ParamMap {
    json_to_map(json!({
        "reader": "TCP", "ip": "192.168.99.100", "port": 9000, "reconnection_time": 5
    }))
}
