/// Example: Write a GPO (General Purpose Output) using the R700 reader.
///
/// Run:
///   cargo run --example r700_gpo -- <ip>
/// or:
///   cargo run --example r700_gpo
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
    params.insert("name".to_string(), Value::String("r700-gpo".to_string()));
    params.insert("ip".to_string(), Value::String(ip.clone()));
    params.insert("username".to_string(), Value::String("root".to_string()));
    params.insert("password".to_string(), Value::String("impinj".to_string()));
    params.insert("start_reading".to_string(), Value::Bool(false));
    params.insert("session".to_string(), Value::Number(Number::from(1)));

    let reader = R700::from_map(params).expect("valid config");
    println!("R700 GPO example");

    let bg = reader.clone();
    let connect_task = tokio::spawn(async move { bg.connect().await });

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    if reader.is_connected() {
        // Set GPO port 1 high (static), then low
        let r1 = reader.write_gpo(1, true, "static", 0).await;
        println!("GPO 1 HIGH: {:?}", r1);
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let r2 = reader.write_gpo(1, false, "static", 0).await;
        println!("GPO 1 LOW : {:?}", r2);
    } else {
        println!("Not connected — is the reader at {}?", ip);
    }

    reader.close().await;
    connect_task.abort();
}
