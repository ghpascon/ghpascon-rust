/// Example: SATO WS4 printer — variant with default IP 192.168.1.102.
///
/// Run:
///   cargo run --example sato_ws4 -- <ip>
/// or:
///   cargo run --example sato_ws4
/// (defaults to 192.168.1.102)
use std::collections::HashMap;
use std::time::Duration;

use ghpascon_rust::devices::printer::sato::SatoWs4Printer;
use serde_json::Value;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    let printer = if let Some(ip) = args.get(1) {
        let mut params: HashMap<String, Value> = HashMap::new();
        params.insert("name".to_string(), Value::String("sato-ws4".to_string()));
        params.insert("ip".to_string(), Value::String(ip.clone()));
        SatoWs4Printer::from_map(params)
    } else {
        SatoWs4Printer::default()
    };

    println!("SATO WS4 example");
    println!("  connect via: {}", printer.connect_instruction());

    let bg = printer.clone();
    let connect_task = tokio::spawn(async move { bg.0.connect().await });
    tokio::time::sleep(Duration::from_secs(2)).await;

    if printer.is_connected() {
        let zpl = "^XA^FO50,50^ADN,36,20^FDSATO WS4 ready^FS^XZ";
        match printer.print(zpl).await {
            Ok(id) => println!("Printed, job id: {}", id),
            Err(e) => eprintln!("Print error: {}", e),
        }
    } else {
        println!("Not connected");
    }

    tokio::time::sleep(Duration::from_secs(2)).await;
    printer.0.close().await;
    connect_task.abort();
}
