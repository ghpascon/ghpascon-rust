/// Example: R700 protected inventory (access password required for read/write).
///
/// Run:
///   cargo run --example r700_protected_inventory -- <ip>
/// or:
///   cargo run --example r700_protected_inventory
/// (defaults to 192.168.1.101)
use ghpascon_rust::devices::rfid::r700::config_example::r700_protected_inventory_map;
use ghpascon_rust::devices::rfid::r700::R700;
use serde_json::Value;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let ip = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "192.168.1.101".to_string());

    let mut params = r700_protected_inventory_map();
    params.insert("ip".to_string(), Value::String(ip.clone()));

    let reader = R700::from_map(params).expect("valid config");

    println!("R700 protected inventory example");
    println!("  connect via: {}", reader.connect_instruction());
    println!("  inventory uses access password for protected tags");
    println!("\nConnecting to {}…  (Ctrl+C to stop)\n", ip);

    let bg = reader.clone();
    let connect_task = tokio::spawn(async move { bg.connect().await });

    tokio::signal::ctrl_c().await.ok();

    println!("\nStopping…");
    reader.stop_inventory().await.ok();
    reader.close().await;
    connect_task.abort();
}
