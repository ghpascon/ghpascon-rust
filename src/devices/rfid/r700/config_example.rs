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
        "reader": "R700_IOT", "ip": "192.168.1.101", "start_reading": true,
        "session": 1, "active_ant": [1], "read_power": 33, "read_rssi": -80,
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

pub fn r700_iot_full_map() -> ParamMap {
    json_to_map(json!({
        "reader": "R700_IOT",
        "ip": "192.168.1.101",
        "username": "root",
        "password": "impinj",
        "start_reading": true,
        "reconnection_time": 2,
        "reading_config": {
            "antennaConfigs": [
                {
                    "antennaPort": 1,
                    "estimatedTagPopulation": 16,
                    "fastId": "enabled",
                    "inventorySearchMode": "dual-target",
                    "inventorySession": 1,
                    "receiveSensitivityDbm": -80,
                    "rfMode": 4,
                    "transmitPowerCdbm": 3300
                },
                {
                    "antennaPort": 2,
                    "estimatedTagPopulation": 16,
                    "fastId": "enabled",
                    "inventorySearchMode": "dual-target",
                    "inventorySession": 1,
                    "receiveSensitivityDbm": -80,
                    "rfMode": 4,
                    "transmitPowerCdbm": 3300
                },
                {
                    "antennaPort": 3,
                    "estimatedTagPopulation": 16,
                    "fastId": "enabled",
                    "inventorySearchMode": "dual-target",
                    "inventorySession": 1,
                    "receiveSensitivityDbm": -80,
                    "rfMode": 4,
                    "transmitPowerCdbm": 3300
                },
                {
                    "antennaPort": 4,
                    "estimatedTagPopulation": 16,
                    "fastId": "enabled",
                    "inventorySearchMode": "dual-target",
                    "inventorySession": 1,
                    "receiveSensitivityDbm": -80,
                    "rfMode": 4,
                    "transmitPowerCdbm": 3300
                }
            ],
            "startTriggers": [{"gpiTransitionEvent": {"gpi": 1, "transition": "high-to-low"}}],
            "stopTriggers": [{"gpiTransitionEvent": {"gpi": 1, "transition": "low-to-high"}}],
            "eventConfig": {
                "common": {"hostname": "disabled"},
                "tagInventory": {
                    "epc": "disabled",
                    "epcHex": "enabled",
                    "xpcHex": "disabled",
                    "tid": "disabled",
                    "tidHex": "enabled",
                    "antennaPort": "enabled",
                    "transmitPowerCdbm": "disabled",
                    "peakRssiCdbm": "enabled",
                    "frequency": "disabled",
                    "pc": "disabled",
                    "lastSeenTime": "disabled",
                    "phaseAngle": "disabled",
                    "tagReporting": {
                        "reportingIntervalSeconds": 1,
                        "tagCacheSize": 2048,
                        "antennaIdentifier": "antennaPort",
                        "tagIdentifier": "epc"
                    }
                }
            }
        }
    }))
}
