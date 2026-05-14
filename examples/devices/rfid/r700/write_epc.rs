/// Example: Write an EPC using the R700 reader.
///
/// Run:
///   cargo run --example r700_write_epc -- <ip>
/// or:
///   cargo run --example r700_write_epc
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
    params.insert("name".to_string(), Value::String("r700-write".to_string()));
    params.insert("ip".to_string(), Value::String(ip.clone()));
    params.insert("username".to_string(), Value::String("root".to_string()));
    params.insert("password".to_string(), Value::String("impinj".to_string()));
    params.insert("start_reading".to_string(), Value::Bool(false));
    params.insert("session".to_string(), Value::Number(Number::from(1)));

    let reader = R700::from_map(params).expect("valid config");
    println!("R700 write EPC example");

    let bg = reader.clone();
    let connect_task = tokio::spawn(async move { bg.connect().await });

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    if reader.is_connected() {
        let result = reader
            .write_epc(
                Some("epc"),
                Some("E280110000000000"),
                "E280110000000001",
                "00000000",
            )
            .await;
        println!("write_epc result: {:?}", result);
    } else {
        println!("Not connected — is the reader at {}?", ip);
    }

    reader.close().await;
    connect_task.abort();
}
