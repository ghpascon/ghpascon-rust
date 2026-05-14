/// Example: Generic TCP device — connect, write, and receive data.
///
/// Run:
///   cargo run --example tcp_device_basic -- <ip> <port>
/// or:
///   cargo run --example tcp_device_basic
/// (defaults to 127.0.0.1:9000)
use std::collections::HashMap;
use std::time::Duration;

use ghpascon_rust::devices::generic::tcp::TcpDevice;
use serde_json::{Number, Value};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let ip = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let port: u16 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(9000);

    let mut params: HashMap<String, Value> = HashMap::new();
    params.insert("name".to_string(), Value::String("tcp-demo".to_string()));
    params.insert("ip".to_string(), Value::String(ip.clone()));
    params.insert("port".to_string(), Value::Number(Number::from(port)));

    let device = TcpDevice::from_map(params);

    println!("TCP device example");
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
        println!("Not connected (is something listening on {}:{}?)", ip, port);
    }

    tokio::time::sleep(Duration::from_secs(5)).await;
    device.close().await;
    connect_task.abort();
}
