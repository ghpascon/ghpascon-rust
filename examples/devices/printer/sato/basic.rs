/// Example: SATO printer — connect and print a ZPL label.
///
/// Run:
///   cargo run --example sato_basic -- <ip>
/// or:
///   cargo run --example sato_basic
/// (defaults to 192.168.1.100)
use std::collections::HashMap;
use std::time::Duration;

use ghpascon_rust::devices::printer::sato::SatoPrinter;
use serde_json::Value;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let ip = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "192.168.1.100".to_string());

    let mut params: HashMap<String, Value> = HashMap::new();
    params.insert("name".to_string(), Value::String("sato-basic".to_string()));
    params.insert("ip".to_string(), Value::String(ip.clone()));

    let printer = SatoPrinter::from_map(params);

    println!("SATO printer basic example");
    println!("  connect via: {}", printer.connect_instruction());

    let bg = printer.clone();
    let connect_task = tokio::spawn(async move { bg.connect().await });

    tokio::time::sleep(Duration::from_secs(2)).await;

    if printer.is_connected() {
        let zpl = "^XA^FO50,50^ADN,36,20^FDHello SATO^FS^XZ";
        match printer.print(zpl).await {
            Ok(id) => println!("Printed, job id: {}", id),
            Err(e) => eprintln!("Print error: {}", e),
        }
    } else {
        println!("Not connected (is the printer at {}?)", ip);
    }

    tokio::time::sleep(Duration::from_secs(2)).await;
    printer.close().await;
    connect_task.abort();
}
