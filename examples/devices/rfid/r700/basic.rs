/// Example: connect to an R700 via HTTPS REST API and run the inventory.
///
/// Run:
///   cargo run --example r700_basic -- <ip>
/// or:
///   cargo run --example r700_basic
/// (defaults to 192.168.1.101)
use std::collections::HashMap;

use ghpascon_rust::devices::rfid::r700::R700;
use serde_json::{Number, Value};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let ip = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "192.168.1.101".to_string());

    let mut params = HashMap::new();
    params.insert("name".to_string(), Value::String("r700-basic".to_string()));
    params.insert("ip".to_string(), Value::String(ip.clone()));
    params.insert("username".to_string(), Value::String("root".to_string()));
    params.insert("password".to_string(), Value::String("impinj".to_string()));
    params.insert("start_reading".to_string(), Value::Bool(true));
    params.insert("session".to_string(), Value::Number(Number::from(1)));
    params.insert("read_power".to_string(), Value::Number(Number::from(3000)));
    params.insert(
        "read_rssi".to_string(),
        Value::Number(Number::from(-80_i64)),
    );
    params.insert(
        "active_ant".to_string(),
        Value::Array(vec![Value::Number(Number::from(1))]),
    );

    let reader = R700::from_map(params).expect("valid config");

    println!("R700 example");
    println!("  connect via: {}", reader.connect_instruction());
    println!("  reading config:");
    println!(
        "  {}",
        serde_json::to_string_pretty(&reader.config.build_reading_config()).unwrap()
    );
    println!("\nConnecting to {}…  (Ctrl+C to stop)\n", ip);

    // connect() runs the reconnection loop forever — spawn as background task
    let bg = reader.clone();
    let connect_task = tokio::spawn(async move { bg.connect().await });

    // Give the reader time to connect and stream tags
    tokio::signal::ctrl_c().await.ok();

    println!("\nStopping…");
    reader.stop_inventory().await.ok();
    reader.close().await;
    connect_task.abort();
}
