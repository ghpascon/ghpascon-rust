/// Example: R700 with GPI triggers for inventory start/stop.
///
/// Run:
///   cargo run --example r700_gpi -- <ip>
/// or:
///   cargo run --example r700_gpi
/// (defaults to 192.168.1.101)
use ghpascon_rust::devices::rfid::r700::config_example::r700_iot_gpi_map;
use ghpascon_rust::devices::rfid::r700::R700;
use serde_json::Value;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let ip = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "192.168.1.101".to_string());

    let mut params = r700_iot_gpi_map();
    params.insert("ip".to_string(), Value::String(ip.clone()));

    let reader = R700::from_map(params).expect("valid config");

    println!("R700 GPI trigger example");
    println!("  connect via: {}", reader.connect_instruction());
    println!("  GPI is configured — toggle GPIO to start/stop inventory");
    println!("\nConnecting to {}…  (Ctrl+C to stop)\n", ip);

    let bg = reader.clone();
    let connect_task = tokio::spawn(async move { bg.connect().await });

    tokio::signal::ctrl_c().await.ok();

    println!("\nStopping…");
    reader.close().await;
    connect_task.abort();
}
