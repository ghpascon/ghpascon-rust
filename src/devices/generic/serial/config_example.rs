use serde_json::{Value, json};

use super::config::ParamMap;

fn json_to_map(v: Value) -> ParamMap {
    match v {
        Value::Object(m) => m.into_iter().collect(),
        _ => Default::default(),
    }
}

pub fn serial_default_map() -> ParamMap {
    json_to_map(json!({
        "reader": "SERIAL", "port": "AUTO", "baudrate": 9600,
        "vid": 259, "pid": 24673, "reconnection_time": 3
    }))
}

pub fn serial_custom_map() -> ParamMap {
    json_to_map(json!({
        "reader": "SERIAL", "port": "AUTO", "baudrate": 9600,
        "vid": 259, "pid": 24673, "reconnection_time": 3
    }))
}
