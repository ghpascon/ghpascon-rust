/// Example: SATO printer — queue and print a list of labels.
///
/// Run:
///   cargo run --example sato_print_list -- <ip>
/// or:
///   cargo run --example sato_print_list
/// (defaults to 192.168.1.100)
use std::collections::HashMap;
use std::time::Duration;

use ghpascon_rust::devices::printer::sato::SatoPrinter;
use serde_json::Value;

fn make_label(index: u32) -> String {
    format!(
        "^XA^FO50,50^ADN,36,20^FDLabel {:03}^FS^FO50,100^ADN,28,15^FDEPC:E2801100000000{:02X}^FS^XZ",
        index, index
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
    params.insert("name".to_string(), Value::String("sato-list".to_string()));
    params.insert("ip".to_string(), Value::String(ip.clone()));

    let printer = SatoPrinter::from_map(params);

    let labels: Vec<String> = (1..=5).map(make_label).collect();
    println!("Queuing {} labels…", labels.len());
    printer.add_to_print_queue(labels).await;

    let bg = printer.clone();
    let connect_task = tokio::spawn(async move { bg.connect().await });
    tokio::time::sleep(Duration::from_secs(2)).await;

    if printer.is_connected() {
        println!("Processing print queue…");
        printer.process_queue().await;
        println!("Done.");
    } else {
        println!("Not connected (is the printer at {}?)", ip);
    }

    tokio::time::sleep(Duration::from_secs(2)).await;
    printer.close().await;
    connect_task.abort();
}
