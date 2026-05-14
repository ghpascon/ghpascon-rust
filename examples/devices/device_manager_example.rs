/// Example: DeviceManager with TagList and a custom event handler.
///
/// Reads all `.json` config files from `examples/devices/configs/` and connects
/// every device. The handler populates a shared TagList and prints each event.
/// A periodic timer dumps all tags every 10 s; another logs device status every
/// 30 s. Press Ctrl+C to stop.
///
/// Run:
///   cargo run --example device_manager_example
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ghpascon_rust::device_manager::{DeviceManager, SharedEventHandler};
use ghpascon_rust::utils::tag_list::{TagList, make_tag};
use serde_json::Value;

fn build_handler(tags: Arc<TagList>) -> SharedEventHandler {
    Arc::new(Mutex::new(Box::new(
        move |name: &str, event_type: &str, data: Option<Value>| match event_type {
            "tag" => {
                let Some(obj) = data.as_ref().and_then(|v| v.as_object()) else {
                    return;
                };
                let epc = obj.get("epc").and_then(|v| v.as_str()).unwrap_or_default();
                let tid = obj.get("tid").and_then(|v| v.as_str());
                let rssi = obj.get("rssi").and_then(|v| v.as_i64()).unwrap_or(0);
                let ant = obj.get("ant").and_then(|v| v.as_u64()).unwrap_or(0);

                let record = make_tag(epc, tid, rssi, ant);
                let (is_new, _) = tags.add(record, name);
                let marker = if is_new { "NEW" } else { "UPD" };
                println!(
                    "[{}] {} epc={} tid={} ant={} rssi={}",
                    name,
                    marker,
                    epc,
                    tid.unwrap_or("-"),
                    ant,
                    rssi
                );
            }
            "connection" => {
                let ok = data.and_then(|v| v.as_bool()).unwrap_or(false);
                println!(
                    "[{}] Connection: {}",
                    name,
                    if ok { "connected" } else { "disconnected" }
                );
            }
            "reading" => {
                let on = data.and_then(|v| v.as_bool()).unwrap_or(false);
                println!("[{}] Reading: {}", name, if on { "ON" } else { "OFF" });
            }
            "serial_number" => {
                let sn = data
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default();
                println!("[{}] Serial: {}", name, sn);
            }
            other => {
                println!("[{}] {} -> {:?}", name, other, data);
            }
        },
    )))
}

#[tokio::main]
async fn main() {
    let tags = Arc::new(TagList::builder().build());

    let mut manager = DeviceManager::new("examples/devices/configs")
        .with_event_handler(build_handler(Arc::clone(&tags)));

    manager.connect_devices(false).await;
    let device_names = manager.get_device_names();

    println!(
        "\n{} device(s) loaded: {:?}",
        manager.len(),
        device_names
    );
    println!(
        "available config examples: {:?}",
        DeviceManager::get_config_examples()
    );
    println!(
        "current device info: {}",
        serde_json::to_string_pretty(&manager.get_device_info(None)).unwrap()
    );
    if let Some(first_name) = device_names.first() {
        if let Some(config) = manager.get_device_config(first_name) {
            println!(
                "current config for '{}': {}",
                first_name,
                serde_json::to_string_pretty(&config).unwrap()
            );
        }
    }
    if let Some(example) = DeviceManager::get_config_example("X714_DEFAULT") {
        println!(
            "example config (X714_DEFAULT): {}",
            serde_json::to_string_pretty(&example).unwrap()
        );
    }
    println!("Press Ctrl+C to stop\n");

    // Dump all tags every 10 s
    let tags_timer = Arc::clone(&tags);
    let dump_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        interval.tick().await;
        loop {
            interval.tick().await;
            let all = tags_timer.get_all_sorted();
            println!("\n--- Tag collection ({} tags) ---", all.len());
            for tag in &all {
                println!(
                    "  epc={} tid={} ant={} rssi={} count={} device={}",
                    tag.get("epc").and_then(|v| v.as_str()).unwrap_or("-"),
                    tag.get("tid").and_then(|v| v.as_str()).unwrap_or("-"),
                    tag.get("ant").and_then(|v| v.as_i64()).unwrap_or(0),
                    tag.get("rssi").and_then(|v| v.as_i64()).unwrap_or(0),
                    tag.get("count").and_then(|v| v.as_i64()).unwrap_or(1),
                    tag.get("device").and_then(|v| v.as_str()).unwrap_or("-"),
                );
            }
            println!("--------------------------------\n");
        }
    });

    // Log device status every 30 s
    let device_clones: Vec<_> = manager.devices.iter().map(|d| d.clone()).collect();
    let status_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        interval.tick().await;
        loop {
            interval.tick().await;
            println!("\n--- Device status ---");
            for d in &device_clones {
                println!(
                    "  {} [{}] connected={} reading={} serial={:?}",
                    d.name(),
                    d.device_type(),
                    d.is_connected(),
                    d.is_reading(),
                    d.serial_number(),
                );
            }
            println!("---------------------\n");
        }
    });

    tokio::signal::ctrl_c().await.ok();

    println!("\nStopping...");
    manager.cancel_connect_tasks().await;
    manager.disconnect_devices().await;
    dump_task.abort();
    status_task.abort();
    println!("Done.");
}
