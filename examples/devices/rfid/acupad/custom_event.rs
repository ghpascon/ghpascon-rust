/// Example: ACUPAD with custom event handler.
///
/// Run:
///   cargo run --example acupad_custom_event
use std::sync::{Arc, Mutex};
use ghpascon_rust::devices::rfid::acupad::{Acupad, SharedEventHandler};
use serde_json::Value;

fn build_handler() -> SharedEventHandler {
    Arc::new(Mutex::new(Box::new(|name: &str, event_type: &str, data: Option<Value>| {
        match event_type {
            "tag" => {
                let obj = data.as_ref().and_then(|v| v.as_object());
                if let Some(obj) = obj {
                    let epc = obj.get("epc").and_then(|v| v.as_str()).unwrap_or("-");
                    let rssi = obj.get("rssi").and_then(|v| v.as_i64()).unwrap_or(0);
                    println!("[{}] Tag: epc={} rssi={}", name, epc, rssi);
                }
            }
            "connection" => {
                let ok = data.and_then(|v| v.as_bool()).unwrap_or(false);
                println!("[{}] Connection: {}", name, ok);
            }
            "reading" => {
                let on = data.and_then(|v| v.as_bool()).unwrap_or(false);
                println!("[{}] Reading: {}", name, if on { "ON" } else { "OFF" });
            }
            _ => println!("[{}] {}: {:?}", name, event_type, data),
        }
    })))
}

#[tokio::main]
async fn main() {
    let reader = Acupad::default().with_event_handler(build_handler());
    println!("ACUPAD custom event example");
    println!("  connect via: {}", reader.connect_instruction());

    let bg = reader.clone();
    let connect_task = tokio::spawn(async move { bg.connect().await });

    tokio::signal::ctrl_c().await.ok();
    reader.close().await;
    connect_task.abort();
}
