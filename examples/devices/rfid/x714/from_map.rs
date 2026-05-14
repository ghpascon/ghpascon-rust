use std::collections::HashMap;

use ghpascon_rust::devices::rfid::x714::X714;
use serde_json::{Number, Value, json};

fn main() {
    let mut ant_dict = serde_json::Map::new();
    ant_dict.insert(
        "1".to_string(),
        json!({"active": true, "power": 26, "rssi": -70}),
    );
    ant_dict.insert(
        "2".to_string(),
        json!({"active": true, "power": 20, "rssi": -65}),
    );

    let params = HashMap::from([
        ("name".to_string(), Value::String("mapped-x714".to_string())),
        (
            "connection_type".to_string(),
            Value::String("BLE".to_string()),
        ),
        (
            "ble_name".to_string(),
            Value::String("SMTX-RFID".to_string()),
        ),
        ("session".to_string(), Value::Number(Number::from(2))),
        ("buzzer".to_string(), Value::Bool(true)),
        ("ant_dict".to_string(), Value::Object(ant_dict)),
    ]);

    let reader = X714::from_map(params).expect("config from map must work");
    println!("X714 from_map example (BLE)");
    println!("{}", reader.connect_instruction());
    println!("{:#?}", reader.to_map());
}
