/// Example: connect to an R700, store tags in a TagList, print every event
/// received live, and dump the full collection every 10 s.
///
/// Run:
///   cargo run --example r700_custom_event -- <ip>
/// or:
///   cargo run --example r700_custom_event
/// (defaults to 192.168.1.101)
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ghpascon_rust::devices::rfid::r700::{R700, SharedEventHandler};
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
                    let (is_new, _) = tags.add(record, name);
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
    let ip = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "192.168.1.101".to_string());

    // Shared tag collection
    let tags = Arc::new(TagList::builder().build());

    let mut params = HashMap::new();
    params.insert("name".to_string(), Value::String("r700-custom".to_string()));
    params.insert("ip".to_string(), Value::String(ip.clone()));
    params.insert("username".to_string(), Value::String("root".to_string()));
    params.insert("password".to_string(), Value::String("impinj".to_string()));
    params.insert("start_reading".to_string(), Value::Bool(true));
    params.insert("session".to_string(), Value::Number(Number::from(1)));
    params.insert("read_power".to_string(), Value::Number(Number::from(3000)));
    params.insert(
        "read_rssi".to_string(),
        Value::Number(Number::from(-80_i64)),
    );
    params.insert(
        "active_ant".to_string(),
        Value::Array(vec![Value::Number(Number::from(1))]),
    );

    let reader = R700::from_map(params)
        .expect("valid config")
        .with_event_handler(print_handler(Arc::clone(&tags)));

    println!("Connecting to {}…  (Ctrl+C to stop)\n", ip);

    // Background: reconnection loop
    let bg = reader.clone();
    let connect_task = tokio::spawn(async move { bg.connect().await });

    // Background: dump tag collection every 10 s
    let tags_timer = Arc::clone(&tags);
    let dump_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        interval.tick().await; // skip immediate first tick
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

    // Wait until Ctrl+C
    tokio::signal::ctrl_c().await.ok();

    println!("\nStopping…");
    reader.stop_inventory().await.ok();
    reader.close().await;
    connect_task.abort();
    dump_task.abort();
}
