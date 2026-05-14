use serde_json::{json, Value};

use super::config::ParamMap;

fn json_to_map(v: Value) -> ParamMap {
    match v {
        Value::Object(m) => m.into_iter().collect(),
        _ => Default::default(),
    }
}

pub fn sato_default_map() -> ParamMap {
    json_to_map(json!({
        "name": "SATO", "ip": "192.168.1.112", "port": 9100, "reconnection_time": 3
    }))
}

pub fn sato_ws4_map() -> ParamMap {
    json_to_map(json!({
        "name": "SATO_WS4", "ip": "192.168.1.102", "port": 9100, "reconnection_time": 3
    }))
}
