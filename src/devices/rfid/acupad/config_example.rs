use serde_json::{json, Value};

use super::config::ParamMap;

fn json_to_map(v: Value) -> ParamMap {
    match v {
        Value::Object(m) => m.into_iter().collect(),
        _ => Default::default(),
    }
}

pub fn acupad_default_map() -> ParamMap {
    json_to_map(json!({
        "reader": "ACUPAD", "port": "AUTO", "baudrate": 115200,
        "vid": 260, "pid": 24656, "beep": false, "session": 1,
        "start_reading": false, "active_ant": [1], "read_power": 22
    }))
}

pub fn acupad_basic_map() -> ParamMap {
    json_to_map(json!({
        "reader": "ACUPAD", "start_reading": true,
        "active_ant": [1], "session": 1, "read_power": 22
    }))
}
