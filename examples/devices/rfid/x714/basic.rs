use std::collections::HashMap;
use std::time::Duration;

use ghpascon_rust::devices::rfid::x714::X714;
use serde_json::{Number, Value};

#[tokio::main]
async fn main() {
    let mut params = HashMap::new();
    params.insert("name".to_string(), Value::String("dock-x714".to_string()));
    params.insert(
        "connection_type".to_string(),
        Value::String("TCP".to_string()),
    );
    params.insert("ip".to_string(), Value::String("192.168.1.50".to_string()));
    params.insert("tcp_port".to_string(), Value::Number(Number::from(23)));
    params.insert(
        "active_ant".to_string(),
        Value::Array(vec![
            Value::Number(Number::from(1)),
            Value::Number(Number::from(2)),
        ]),
    );

    let reader = X714::from_map(params).expect("valid config");

    println!("X714 basic example (TCP)");
    println!("connect via: {}", reader.connect_instruction());
    println!("config commands:");
    for cmd in reader.config_commands() {
        println!("  {}", cmd);
    }

    // `connect()` runs the reconnection loop forever – spawn it as a background task.
    let bg = reader.clone();
    let connect_task = tokio::spawn(async move { bg.connect().await });

    // Give the reader some time to connect and send tags.
    tokio::time::sleep(Duration::from_secs(15)).await;

    println!(
        "\nstatus: connected={}, reading={}",
        reader.is_connected(),
        reader.is_reading()
    );
    if let Some(sn) = reader.serial_number() {
        println!("serial number: {}", sn);
    }

    // Gracefully stop.
    reader.stop_inventory().await.ok();
    reader.close().await;
    connect_task.abort();

    // Offline parser demo (no hardware needed).
    println!("\nparsed frames (offline demo):");
    let demo = X714::default();
    for frame in [
        "#READ:ON",
        "#T+@E28011|E20033|1|54|LOCKED",
        "#name:X714-ABC123",
    ] {
        for event in demo.parse_line(frame) {
            println!("  {:?}", event);
        }
    }
}
