/// Example: connect to an X714 via Serial, store tags in a TagList, print every
/// event received live, and dump the full collection every 10 s.
///
/// Run:
///   cargo run --example x714_custom_event -- <port> <baudrate>
/// or just:
///   cargo run --example x714_custom_event
/// (AUTO port detection via VID=0x0001 PID=0x0001, 115200 bps)
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ghpascon_rust::devices::rfid::x714::{SharedEventHandler, X714};
use ghpascon_rust::utils::tag_list::{TagList, make_tag};
use serde_json::{Number, Value};

fn print_handler(tags: Arc<TagList>) -> SharedEventHandler {
    Arc::new(Mutex::new(Box::new(
        move |name: &str, event_type: &str, event_data: Option<Value>| match event_type {
            "tag" => {
                if let Some(obj) = event_data.as_ref().and_then(|v| v.as_object()) {
                    let epc = obj.get("epc").and_then(|v| v.as_str()).unwrap_or_default();
                    let tid = obj.get("tid").and_then(|v| v.as_str());
                    let rssi = obj.get("rssi").and_then(|v| v.as_i64()).unwrap_or(0);
                    let ant = obj.get("ant").and_then(|v| v.as_u64()).unwrap_or(0);

                    let record = make_tag(epc, tid, rssi, ant);
                    let (is_new, _tag) = tags.add(record, name);
                    let marker = if is_new { "➕ NEW" } else { "♻️  UPD" };
                    println!(
                        "[{}] 🏷  {} epc={} tid={} ant={} rssi={}",
                        name,
                        marker,
                        epc,
                        tid.unwrap_or("-"),
                        ant,
                        rssi
                    );
                }
            }
            "connection" => {
                let connected = event_data.and_then(|v| v.as_bool()).unwrap_or(false);
                if connected {
                    println!("[{}] ✅ Connected", name);
                } else {
                    println!("[{}] 🔌 Disconnected", name);
                }
            }
            "reading" => {
                let on = event_data.and_then(|v| v.as_bool()).unwrap_or(false);
                println!("[{}] 📡 Reading: {}", name, if on { "ON" } else { "OFF" });
            }
            "serial_number" => {
                let sn = event_data
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default();
                println!("[{}] 🔢 Serial: {}", name, sn);
            }
            other => {
                println!("[{}] ℹ  {} => {:?}", name, other, event_data);
            }
        },
    )))
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port = args.get(1).cloned().unwrap_or_else(|| "AUTO".to_string());
    let baudrate: u32 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(115_200);

    // Shared tag collection
    let tags = Arc::new(TagList::builder().build());

    let mut params = HashMap::new();
    params.insert("name".to_string(), Value::String("x714-custom".to_string()));
    params.insert(
        "connection_type".to_string(),
        Value::String("SERIAL".to_string()),
    );
    params.insert("port".to_string(), Value::String(port.clone()));
    params.insert(
        "baudrate".to_string(),
        Value::Number(Number::from(baudrate)),
    );
    params.insert("vid".to_string(), Value::Number(Number::from(1_u32)));
    params.insert("pid".to_string(), Value::Number(Number::from(1_u32)));
    params.insert("start_reading".to_string(), Value::Bool(true));
    params.insert(
        "active_ant".to_string(),
        Value::Array(vec![Value::Number(Number::from(1))]),
    );
    params.insert("session".to_string(), Value::Number(Number::from(0)));

    let reader = X714::from_map(params)
        .expect("valid config")
        .with_event_handler(print_handler(Arc::clone(&tags)));

    println!("X714 custom event example (SERIAL)");
    println!("Connecting via Serial {}  (Ctrl+C to stop)\n", port);

    // Background: reconnection loop
    let bg = reader.clone();
    let connect_task = tokio::spawn(async move { bg.connect().await });

    // Background: print full tag collection every 10 s
    let tags_timer = Arc::clone(&tags);
    let dump_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        interval.tick().await; // skip the immediate first tick
        loop {
            interval.tick().await;
            let all = tags_timer.get_all_sorted();
            println!("\n─── Tag collection ({} tags) ───", all.len());
            for tag in &all {
                println!(
                    "  epc={} tid={} ant={} rssi={} count={}",
                    tag.get("epc").and_then(|v| v.as_str()).unwrap_or("-"),
                    tag.get("tid").and_then(|v| v.as_str()).unwrap_or("-"),
                    tag.get("ant").and_then(|v| v.as_i64()).unwrap_or(0),
                    tag.get("rssi").and_then(|v| v.as_i64()).unwrap_or(0),
                    tag.get("count").and_then(|v| v.as_i64()).unwrap_or(1),
                );
            }
            println!("────────────────────────────────\n");
        }
    });

    // Wait until interrupted (Ctrl+C)
    tokio::signal::ctrl_c().await.ok();

    println!("\nStopping…");
    reader.stop_inventory().await.ok();
    reader.close().await;
    connect_task.abort();
    dump_task.abort();
}
