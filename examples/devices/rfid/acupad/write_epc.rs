/// Example: Write EPC to a tag using ACUPAD.
///
/// Run:
///   cargo run --example acupad_write_epc
use std::time::Duration;
use ghpascon_rust::devices::rfid::acupad::Acupad;

#[tokio::main]
async fn main() {
    let reader = Acupad::default();
    let bg = reader.clone();
    let connect_task = tokio::spawn(async move { bg.connect().await });

    tokio::time::sleep(Duration::from_secs(3)).await;

    if reader.is_connected() {
        let result = reader
            .write_epc(Some("epc"), Some("E280110000000000"), "E280110000000001", "00000000")
            .await;
        println!("write_epc result: {:?}", result);
    } else {
        println!("Not connected");
    }

    reader.close().await;
    connect_task.abort();
}
