/// Example: ACUPAD RFID reader via serial.
///
/// Run:
///   cargo run --example acupad_basic
use std::time::Duration;
use ghpascon_rust::devices::rfid::acupad::Acupad;

#[tokio::main]
async fn main() {
    let reader = Acupad::default();
    println!("ACUPAD example");
    println!("  connect via: {}", reader.connect_instruction());

    let bg = reader.clone();
    let connect_task = tokio::spawn(async move { bg.connect().await });

    tokio::time::sleep(Duration::from_secs(15)).await;
    println!("status: connected={}, reading={}", reader.is_connected(), reader.is_reading());

    reader.stop_inventory().await.ok();
    reader.close().await;
    connect_task.abort();

    println!("\nparsed frames (offline demo):");
    let demo = Acupad::default();
    for frame in ["#READ:ON", "#T+@E28011|E20033|1|54|LOCKED", "#name:ACUPAD-ABC123"] {
        for event in demo.parse_line(frame) {
            println!("  {:?}", event);
        }
    }
}
