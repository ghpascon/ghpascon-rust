use serde_json::{Value, json};

use super::config::ParamMap;

fn json_to_map(v: Value) -> ParamMap {
    match v {
        Value::Object(m) => m.into_iter().collect(),
        _ => Default::default(),
    }
}

/// X714 via serial — auto-start reading, antenna 1, session 0, power 22 dBm.
pub fn x714_default_map() -> ParamMap {
    json_to_map(json!({
        "reader":           "X714",
        "start_reading":    true,
        "active_ant":       [1],
        "session":          0,
        "read_power":       22,
    }))
}

/// X714 via serial — GPI-triggered, all 4 antennas, session 1, power 27 dBm.
pub fn x714_map() -> ParamMap {
    json_to_map(json!({
        "reader":           "X714",
        "connection_type":  "SERIAL",
        "buzzer":           true,
        "start_reading":    false,
        "gpi_start":        true,
        "active_ant":       [1, 2, 3, 4],
        "session":          1,
        "read_power":       27,
    }))
}

/// X714 via TCP.
pub fn x714_tcp_map() -> ParamMap {
    json_to_map(json!({
        "reader":           "X714",
        "connection_type":  "TCP",
        "buzzer":           true,
        "ip":               "192.168.1.101",
        "tcp_port":         23,
        "start_reading":    true,
        "active_ant":       [1],
        "session":          1,
        "read_power":       22,
    }))
}

/// X714 via BLE.
pub fn x714_ble_map() -> ParamMap {
    json_to_map(json!({
        "reader":           "X714",
        "connection_type":  "BLE",
        "ble_name":         "SMTX",
        // "ble_address":   "AA:BB:CC:DD:EE:FF",
        "buzzer":           true,
        "start_reading":    false,
        "gpi_start":        true,
        "active_ant":       [1],
        "session":          1,
        "read_power":       22,
    }))
}

/// X714 with all available fields explicitly set.
/// Uses `ant_dict` for per-antenna configuration (takes priority over `active_ant`/`read_power`/`read_rssi`).
pub fn x714_all_map() -> ParamMap {
    json_to_map(json!({
        // Identity
        "reader":           "X714",
        "name":             "X714",
        "connection_type":  "SERIAL",

        // Serial
        "port":             "AUTO",
        "baudrate":         115200,
        "vid":              1,
        "pid":              1,

        // TCP
        "ip":               "192.168.1.100",
        "tcp_port":         23,

        // BLE
        "ble_name":         "SMTX",
        "ble_address":      null,
        "ble_service_uuid": "6E400001-B5A3-F393-E0A9-E50E24DCCA9E",
        "ble_rx_uuid":      "6E400002-B5A3-F393-E0A9-E50E24DCCA9E",
        "ble_tx_uuid":      "6E400003-B5A3-F393-E0A9-E50E24DCCA9E",

        // Behaviour
        "buzzer":           false,
        "session":          1,
        "start_reading":    false,
        "gpi_start":        false,
        "always_send":      true,
        "simple_send":      false,
        "keyboard":         false,
        "decode_gtin":      false,
        "hotspot":          true,

        // Reconnection & filters
        "reconnection_time":            3,
        "prefix":                       "",
        "protected_inventory_active":   false,
        "protected_inventory_password": "12345678",

        // Per-antenna config (takes priority over active_ant / read_power / read_rssi shortcuts)
        "ant_dict": {
            "1": { "active": true,  "power": 26, "rssi": -70  },
            "2": { "active": true,  "power": 22, "rssi": -80  },
            "3": { "active": false, "power": 22, "rssi": -120 },
            "4": { "active": false, "power": 22, "rssi": -120 },
        },
    }))
}
