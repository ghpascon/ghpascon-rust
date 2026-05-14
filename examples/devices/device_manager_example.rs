/// Interactive DeviceManager example:
/// - Lists all built-in device config variants from DeviceManager.
/// - Lets you pick one variant.
/// - Prompts each key with its default value (Enter keeps default).
/// - Writes a JSON config into a temporary directory.
/// - Uses DeviceManager to connect and stream events.
/// - Stores tag events into TagList and prints periodic summaries.
///
/// Run:
///   cargo run --example device_manager_example
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ghpascon_rust::device_manager::{DeviceManager, SharedEventHandler};
use ghpascon_rust::utils::tag_list::{TagList, make_tag};
use serde_json::{Number, Value};
use tempfile::TempDir;

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
                    "[{}] TAG {} epc={} tid={} ant={} rssi={}",
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
                    "[{}] CONNECTION {}",
                    name,
                    if ok { "connected" } else { "disconnected" }
                );
            }
            "reading" => {
                let on = data.and_then(|v| v.as_bool()).unwrap_or(false);
                println!("[{}] READING {}", name, if on { "ON" } else { "OFF" });
            }
            "serial_number" => {
                let sn = data
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default();
                println!("[{}] SERIAL {}", name, sn);
            }
            other => println!("[{}] EVENT {} -> {:?}", name, other, data),
        },
    )))
}

fn family_of(example_name: &str) -> &'static str {
    if example_name.starts_with("X714") {
        "X714"
    } else if example_name.starts_with("R700") {
        "R700"
    } else if example_name.starts_with("SERIAL") {
        "SERIAL"
    } else if example_name.starts_with("TCP") {
        "TCP"
    } else if example_name.starts_with("SATO") {
        "SATO"
    } else {
        "OTHER"
    }
}

fn read_line(prompt: &str) -> io::Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;

    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(buf.trim().to_string())
}

fn select_example_name() -> io::Result<String> {
    let mut examples: Vec<String> = DeviceManager::get_config_examples()
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    examples.sort();

    if examples.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no built-in config examples available",
        ));
    }

    println!("\nAvailable device variants:");
    let mut last_family = "";
    for (index, name) in examples.iter().enumerate() {
        let family = family_of(name);
        if family != last_family {
            println!("\n{} variants:", family);
            last_family = family;
        }
        println!("  {:>2}) {}", index + 1, name);
    }

    loop {
        let answer = read_line("\nSelect a variant by number (Enter = 1): ")?;
        if answer.is_empty() {
            return Ok(examples[0].clone());
        }

        if let Ok(index) = answer.parse::<usize>() {
            if (1..=examples.len()).contains(&index) {
                return Ok(examples[index - 1].clone());
            }
        }

        println!(
            "Invalid selection. Type a number between 1 and {}.",
            examples.len()
        );
    }
}

fn value_preview(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<invalid-json>".to_string())
}

fn parse_bool(input: &str) -> Option<bool> {
    match input.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "y" | "on" => Some(true),
        "false" | "0" | "no" | "n" | "off" => Some(false),
        _ => None,
    }
}

fn parse_input_like_default(input: &str, default: &Value) -> Result<Value, String> {
    match default {
        Value::Bool(_) => parse_bool(input)
            .map(Value::Bool)
            .ok_or_else(|| "expected boolean: true/false, yes/no, 1/0".to_string()),
        Value::Number(number) => {
            if number.is_i64() {
                input
                    .parse::<i64>()
                    .map(|v| Value::Number(Number::from(v)))
                    .map_err(|_| "expected signed integer".to_string())
            } else if number.is_u64() {
                input
                    .parse::<u64>()
                    .map(|v| Value::Number(Number::from(v)))
                    .map_err(|_| "expected unsigned integer".to_string())
            } else {
                let parsed = input
                    .parse::<f64>()
                    .map_err(|_| "expected floating-point number".to_string())?;
                let num = Number::from_f64(parsed)
                    .ok_or_else(|| "invalid floating-point number".to_string())?;
                Ok(Value::Number(num))
            }
        }
        Value::Array(_) | Value::Object(_) => {
            serde_json::from_str::<Value>(input).map_err(|e| format!("expected valid JSON: {}", e))
        }
        Value::Null => {
            if input.eq_ignore_ascii_case("null") {
                Ok(Value::Null)
            } else {
                Ok(Value::String(input.to_string()))
            }
        }
        Value::String(_) => Ok(Value::String(input.to_string())),
    }
}

fn prompt_config_overrides(
    defaults: &HashMap<String, Value>,
) -> io::Result<HashMap<String, Value>> {
    let mut keys: Vec<String> = defaults.keys().cloned().collect();
    keys.sort();

    println!("\nConfig review (Enter keeps default):");
    let mut out = defaults.clone();

    for key in keys {
        let Some(default_value) = defaults.get(&key) else {
            continue;
        };

        loop {
            let prompt = format!("  {} [{}]: ", key, value_preview(default_value));
            let raw = read_line(&prompt)?;

            if raw.is_empty() {
                out.insert(key.clone(), default_value.clone());
                break;
            }

            match parse_input_like_default(&raw, default_value) {
                Ok(parsed) => {
                    out.insert(key.clone(), parsed);
                    break;
                }
                Err(msg) => println!("    invalid value for '{}': {}", key, msg),
            }
        }
    }

    Ok(out)
}

fn sanitize_name(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').to_string()
}

fn write_temp_config(
    example_name: &str,
    config: &HashMap<String, Value>,
) -> io::Result<(TempDir, PathBuf)> {
    let temp_dir = tempfile::tempdir()?;

    let file_base = sanitize_name(example_name);
    let file_name = if file_base.is_empty() {
        "interactive_device.json".to_string()
    } else {
        format!("{}.json", file_base)
    };
    let file_path = temp_dir.path().join(file_name);

    let json = serde_json::to_string_pretty(config)
        .map_err(|e| io::Error::other(format!("could not serialize JSON: {}", e)))?;
    fs::write(&file_path, json)?;

    Ok((temp_dir, file_path))
}

#[tokio::main]
async fn main() {
    println!("Interactive DeviceManager example");
    println!("This will create a temporary JSON config and connect one device.\n");

    let selected_example = match select_example_name() {
        Ok(name) => name,
        Err(e) => {
            eprintln!("Could not select example: {}", e);
            return;
        }
    };

    let Some(default_config) = DeviceManager::get_config_example(&selected_example) else {
        eprintln!(
            "Could not load selected config example '{}'.",
            selected_example
        );
        return;
    };

    println!("\nSelected variant: {}", selected_example);
    println!(
        "Default config: {}",
        serde_json::to_string_pretty(&default_config).unwrap_or_default()
    );

    let final_config = match prompt_config_overrides(&default_config) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Could not read config overrides: {}", e);
            return;
        }
    };

    let (temp_dir, config_path) = match write_temp_config(&selected_example, &final_config) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Could not write temporary config: {}", e);
            return;
        }
    };

    println!(
        "\nTemporary config file created at: {}",
        config_path.display()
    );
    println!(
        "Temp config directory for DeviceManager: {}",
        temp_dir.path().display()
    );

    let tags = Arc::new(TagList::builder().build());
    let mut manager =
        DeviceManager::new(temp_dir.path()).with_event_handler(build_handler(Arc::clone(&tags)));

    manager.connect_devices(false).await;

    let device_names = manager.get_device_names();
    println!("\nLoaded {} device(s): {:?}", manager.len(), device_names);
    println!(
        "Device info snapshot: {}",
        serde_json::to_string_pretty(&manager.get_device_info(None)).unwrap_or_default()
    );
    println!("\nPress Ctrl+C to stop.\n");

    // Keep cloning device handles so status reporting can run while manager is mutably used later.
    let device_clones: Vec<_> = manager.devices.to_vec();

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
            println!("--------------------------------");
        }
    });

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
            println!("---------------------");
        }
    });

    tokio::signal::ctrl_c().await.ok();

    println!("\nStopping...");
    let _ = manager.stop_inventory_all().await;
    manager.cancel_connect_tasks().await;
    manager.disconnect_devices().await;
    dump_task.abort();
    status_task.abort();
    println!("Done.");
}
