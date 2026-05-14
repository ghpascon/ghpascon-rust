/// Example: SATO printer — sequential (one-by-one) label printing.
///
/// Demonstrates printing labels one at a time and waiting for each result.
///
/// Run:
///   cargo run --example sato_sequential -- <ip>
/// or:
///   cargo run --example sato_sequential
/// (defaults to 192.168.1.100)
use std::collections::HashMap;
use std::time::Duration;

use ghpascon_rust::devices::printer::sato::SatoPrinter;
use serde_json::Value;

fn make_label(index: u32, text: &str) -> String {
    format!(
        "^XA^FO50,50^ADN,36,20^FD{}^FS^FO50,100^ADN,28,15^FDItem {:03}^FS^XZ",
        text, index
    )
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let ip = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "192.168.1.100".to_string());

    let mut params: HashMap<String, Value> = HashMap::new();
    params.insert(
        "name".to_string(),
        Value::String("sato-sequential".to_string()),
    );
    params.insert("ip".to_string(), Value::String(ip.clone()));

    let printer = SatoPrinter::from_map(params);

    let bg = printer.clone();
    let connect_task = tokio::spawn(async move { bg.connect().await });
    tokio::time::sleep(Duration::from_secs(2)).await;

    if !printer.is_connected() {
        println!("Not connected (is the printer at {}?)", ip);
        connect_task.abort();
        return;
    }

    let items = vec![("Apple", "fruit"), ("Banana", "fruit"), ("Carrot", "veg")];
    for (i, (name, category)) in items.iter().enumerate() {
        let zpl = make_label(i as u32 + 1, name);
        println!("Printing #{} — {} ({})", i + 1, name, category);
        match printer.print(&zpl).await {
            Ok(id) => println!("  ✓ job id: {}", id),
            Err(e) => eprintln!("  ✗ error: {}", e),
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    printer.close().await;
    connect_task.abort();
}
