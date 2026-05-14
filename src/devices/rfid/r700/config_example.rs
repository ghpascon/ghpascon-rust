use serde_json::{Value, json};

use super::config::ParamMap;

fn json_to_map(v: Value) -> ParamMap {
    match v {
        Value::Object(m) => m.into_iter().collect(),
        _ => Default::default(),
    }
}

pub fn r700_iot_map() -> ParamMap {
    json_to_map(json!({
        "reader": "R700_IOT", "ip": "impinj-14-46-36", "start_reading": true,
        "session": 1, "active_ant": [1], "read_power": 30, "read_rssi": -80,
        "search_mode": "single-target", "rf_mode": 4, "gpi_start": false
    }))
}

pub fn r700_iot_dict_map() -> ParamMap {
    json_to_map(json!({
        "reader": "R700_IOT", "active_ant": [1,2,3,4], "session": 1,
        "read_power": 3300, "start_reading": true
    }))
}

pub fn r700_iot_gpi_map() -> ParamMap {
    json_to_map(json!({
        "reader": "R700_IOT", "gpi_start": true, "start_reading": false,
        "active_ant": [1,2,3,4]
    }))
}

pub fn r700_protected_inventory_map() -> ParamMap {
    json_to_map(json!({
        "reader": "R700_IOT", "protected_inventory_active": true,
        "protected_inventory_password": "12345678", "start_reading": true
    }))
}
