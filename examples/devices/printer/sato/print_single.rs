/// Example: SATO printer — print a single ZPL label with parameter substitution.
///
/// Run:
///   cargo run --example sato_print_single -- <ip>
/// or:
///   cargo run --example sato_print_single
/// (defaults to 192.168.1.100)
use std::collections::HashMap;
use std::time::Duration;

use ghpascon_rust::devices::printer::sato::{SatoPrinter, zpl_utils::generate_zpl_with_params};
use serde_json::Value;

const ZPL_TEMPLATE: &str = "^XA\
    ^FO50,30^ADN,36,20^FD{name}^FS\
    ^FO50,80^ADN,28,15^FDEPC: {epc}^FS\
    ^FO50,130^B3N,N,60,Y,N^FD{barcode}^FS\
    ^XZ";

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let ip = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "192.168.1.100".to_string());

    let mut params: HashMap<String, Value> = HashMap::new();
    params.insert("name".to_string(), Value::String("sato-single".to_string()));
    params.insert("ip".to_string(), Value::String(ip.clone()));

    let printer = SatoPrinter::from_map(params);

    let zpl = generate_zpl_with_params(
        ZPL_TEMPLATE,
        &[
            ("name", "Product A"),
            ("epc", "E280110000000001"),
            ("barcode", "1234567890"),
        ],
    );
    println!("ZPL to send:\n{}\n", zpl);

    let bg = printer.clone();
    let connect_task = tokio::spawn(async move { bg.connect().await });
    tokio::time::sleep(Duration::from_secs(2)).await;

    if printer.is_connected() {
        match printer.print(&zpl).await {
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
