/// Exemplo: DeviceManager com TagList e handler customizado.
///
/// Aponta para `examples/devices/configs/` e carrega todos os `.json`.
/// O handler popula a TagList compartilhada e printa cada evento.
/// A cada 10 s faz dump de todas as tags. Ctrl+C para parar.
///
/// Run:
///   cargo run --example device_manager_example
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ghpascon_rust::device_manager::{DeviceManager, SharedEventHandler};
use ghpascon_rust::utils::tag_list::{TagList, make_tag};
use serde_json::Value;

/// Cria um handler que popula `tags` e printa cada evento recebido.
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
            "connection" => {
                let ok = data.and_then(|v| v.as_bool()).unwrap_or(false);
                if ok {
                    println!("[{}] ✅ Conectado", name);
                } else {
                    println!("[{}] 🔌 Desconectado", name);
                }
            }
            "reading" => {
                let on = data.and_then(|v| v.as_bool()).unwrap_or(false);
                println!("[{}] 📡 Reading: {}", name, if on { "ON" } else { "OFF" });
            }
            "serial_number" => {
                let sn = data
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default();
                println!("[{}] 🔢 Serial: {}", name, sn);
            }
            other => {
                println!("[{}] ℹ  {} → {:?}", name, other, data);
            }
        },
    )))
}

#[tokio::main]
async fn main() {
    // TagList compartilhada entre handler e timer
    let tags = Arc::new(TagList::builder().build());

    // DeviceManager aponta para a pasta de configs de exemplo
    let mut manager = DeviceManager::new("examples/devices/configs")
        .with_event_handler(build_handler(Arc::clone(&tags)));

    // Conecta todos os devices (carrega JSONs internamente)
    manager.connect_devices(false).await;

    println!(
        "\n📦 {} device(s) carregado(s): {:?}",
        manager.len(),
        manager.get_device_names()
    );
    println!("Ctrl+C para parar\n");

    // Task: dump de todas as tags a cada 10 s
    let tags_timer = Arc::clone(&tags);
    let dump_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        interval.tick().await; // pula o primeiro tick imediato
        loop {
            interval.tick().await;
            let all = tags_timer.get_all_sorted();
            println!("\n─── Tag collection ({} tags) ───", all.len());
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
            println!("────────────────────────────────\n");
        }
    });

    // Task: status periódico de todos os devices a cada 30 s
    let status_task = tokio::spawn({
        // Precisamos de uma snapshot dos nomes — o manager é movido ao main loop
        // O manager não é Send, então mantemos só o vetor de nomes para o log;
        // o acesso real ao status usa o clone atômico interno de cada device.
        // Como não podemos mover o manager para a task sem Send, usamos os
        // device clones previamente obtidos via Device::clone() para status.
        let device_clones: Vec<_> = manager.devices.iter().map(|d| d.clone()).collect();
        async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            interval.tick().await;
            loop {
                interval.tick().await;
                println!("\n─── Status dos devices ───");
                for d in &device_clones {
                    println!(
                        "  {} [{}] conectado={} lendo={} serial={:?}",
                        d.name(),
                        d.device_type(),
                        d.is_connected(),
                        d.is_reading(),
                        d.serial_number(),
                    );
                }
                println!("─────────────────────────\n");
            }
        }
    });

    // Aguarda Ctrl+C
    tokio::signal::ctrl_c().await.ok();

    println!("\n🛑 Parando…");
    manager.cancel_connect_tasks().await;
    manager.disconnect_devices().await;
    dump_task.abort();
    status_task.abort();
    println!("✅ Encerrado.");
}
