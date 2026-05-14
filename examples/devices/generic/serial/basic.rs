/// Example: Generic serial device — connect, write, and receive data.
///
/// Run:
///   cargo run --example serial_device_basic -- <port>
/// or:
///   cargo run --example serial_device_basic
/// (auto-detects the first available serial port)
use std::collections::HashMap;
use std::time::Duration;

use ghpascon_rust::devices::generic::serial::SerialDevice;
use serde_json::Value;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut params: HashMap<String, Value> = HashMap::new();
    params.insert("name".to_string(), Value::String("serial-demo".to_string()));
    if let Some(port) = args.get(1) {
        params.insert("port".to_string(), Value::String(port.clone()));
    }

    let device = SerialDevice::from_map(params);

    println!("Serial device example");
    println!("  connect via: {}", device.connect_instruction());

    let bg = device.clone();
    let connect_task = tokio::spawn(async move { bg.connect().await });

    tokio::time::sleep(Duration::from_secs(2)).await;

    if device.is_connected() {
        println!("Connected — sending test message");
        if let Err(e) = device.write("hello\n").await {
            eprintln!("write error: {}", e);
        }
    } else {
        println!("Not connected (no port available?)");
    }

    tokio::time::sleep(Duration::from_secs(5)).await;
    device.close().await;
    connect_task.abort();
}
