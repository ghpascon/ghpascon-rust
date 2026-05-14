use std::sync::atomic::Ordering;
use std::time::Duration;

use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::Manager;
use futures::StreamExt;
use tokio::time::sleep;
use uuid::Uuid;

use super::x714::X714;

impl X714 {
    /// BLE connection + reconnection loop.
    ///
    /// Scans for a BLE peripheral whose advertised name starts with `config.ble.name`
    /// (optionally filtered by `config.ble.address`), connects, and runs the NUS
    /// (Nordic UART Service) protocol:
    ///   - Subscribes to the TX characteristic for incoming data.
    ///   - Writes commands to the RX characteristic via an internal mpsc channel.
    ///   - Sends `#ping` every 5 s to keep the link alive.
    ///   - Reconnects automatically after any disconnection.
    pub(crate) async fn run_ble_loop(&self) {
        loop {
            if !self.shared.running.load(Ordering::Relaxed) {
                break;
            }

            match self.ble_connect_once().await {
                Ok(_) => {}
                Err(e) => eprintln!("[{}] BLE error: {e}", self.config.name),
            }

            self.on_disconnected();

            if !self.shared.running.load(Ordering::Relaxed) {
                break;
            }

            eprintln!(
                "[{}] BLE reconnecting in {}s...",
                self.config.name, self.config.reconnection_time
            );
            sleep(Duration::from_secs(self.config.reconnection_time)).await;
        }
    }

    async fn ble_connect_once(&self) -> Result<(), String> {
        // \u2500\u2500 1. Get adapter \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500
        let manager = Manager::new().await.map_err(|e| e.to_string())?;
        let adapters = manager.adapters().await.map_err(|e| e.to_string())?;
        let adapter = adapters
            .into_iter()
            .next()
            .ok_or_else(|| "no BLE adapter found".to_string())?;

        // \u2500\u2500 2. Scan for the target device \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500
        eprintln!(
            "[{}] BLE scanning for '{}'...",
            self.config.name, self.config.ble.name
        );
        adapter
            .start_scan(ScanFilter::default())
            .await
            .map_err(|e| e.to_string())?;

        let mut events = adapter.events().await.map_err(|e| e.to_string())?;
        let target_name = self.config.ble.name.clone();
        let target_address = self.config.ble.address.clone();

        let peripheral = loop {
            if !self.shared.running.load(Ordering::Relaxed) {
                adapter.stop_scan().await.ok();
                return Ok(());
            }

            let event = match tokio::time::timeout(Duration::from_secs(30), events.next()).await {
                Ok(Some(e)) => e,
                Ok(None) => return Err("BLE event stream ended".to_string()),
                Err(_) => return Err("BLE scan timeout (30 s)".to_string()),
            };

            let peripheral_id = match event {
                CentralEvent::DeviceDiscovered(id) | CentralEvent::DeviceUpdated(id) => id,
                _ => continue,
            };

            let p = match adapter.peripheral(&peripheral_id).await {
                Ok(p) => p,
                Err(_) => continue,
            };

            let props = match p.properties().await {
                Ok(Some(props)) => props,
                _ => continue,
            };

            let name_match = props
                .local_name
                .as_deref()
                .map(|n| n.starts_with(&target_name))
                .unwrap_or(false);

            if !name_match {
                continue;
            }

            // Optional address filter
            if let Some(addr) = &target_address {
                if props.address.to_string() != *addr {
                    continue;
                }
            }

            eprintln!(
                "[{}] BLE found '{}'",
                self.config.name,
                props.local_name.as_deref().unwrap_or("?")
            );
            break p;
        };

        adapter.stop_scan().await.ok();

        // \u2500\u2500 3. Connect and discover services \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500
        eprintln!("[{}] BLE connecting...", self.config.name);
        peripheral.connect().await.map_err(|e| e.to_string())?;
        peripheral
            .discover_services()
            .await
            .map_err(|e| e.to_string())?;
        eprintln!("[{}] BLE connected", self.config.name);

        // \u2500\u2500 4. Resolve NUS characteristics \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500
        let rx_uuid = Uuid::parse_str(&self.config.ble.rx_uuid)
            .map_err(|e| format!("invalid ble_rx_uuid: {e}"))?;
        let tx_uuid = Uuid::parse_str(&self.config.ble.tx_uuid)
            .map_err(|e| format!("invalid ble_tx_uuid: {e}"))?;

        let chars = peripheral.characteristics();
        let rx_char = chars
            .iter()
            .find(|c| c.uuid == rx_uuid)
            .ok_or_else(|| format!("RX characteristic {rx_uuid} not found"))?
            .clone();
        let tx_char = chars
            .iter()
            .find(|c| c.uuid == tx_uuid)
            .ok_or_else(|| format!("TX characteristic {tx_uuid} not found"))?
            .clone();

        // \u2500\u2500 5. Subscribe to TX (device \u2192 host) \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500
        peripheral
            .subscribe(&tx_char)
            .await
            .map_err(|e| e.to_string())?;
        let mut notif_stream = peripheral
            .notifications()
            .await
            .map_err(|e| e.to_string())?;

        // \u2500\u2500 6. Write channel (host \u2192 device via RX characteristic) \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500
        let (write_tx, mut write_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        *self.shared.ble_write_tx.lock().await = Some(write_tx);

        self.on_connected().await;

        // \u2500\u2500 7. Spawn tasks \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500

        // Receive: notification stream → on_receive
        let recv_self = self.clone();
        let mut recv_task = tokio::spawn(async move {
            while let Some(notif) = notif_stream.next().await {
                if let Ok(s) = std::str::from_utf8(&notif.value) {
                    for line in s.split('\n') {
                        let t = line.trim();
                        if !t.is_empty() {
                            recv_self.on_receive(t);
                        }
                    }
                }
            }
        });

        // Write: mpsc receiver → RX characteristic
        let p_write = peripheral.clone();
        let mut write_task = tokio::spawn(async move {
            while let Some(data) = write_rx.recv().await {
                let _ = p_write
                    .write(&rx_char, &data, WriteType::WithoutResponse)
                    .await;
            }
        });

        // Ping: keep connection alive every 5 s
        let ping_self = self.clone();
        let mut ping_task = tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(5)).await;
                if !ping_self.shared.is_connected.load(Ordering::Relaxed) {
                    break;
                }
                let _ = ping_self.write("#ping").await;
            }
        });

        // Monitor: poll is_connected() every 2 s
        let p_monitor = peripheral.clone();
        let monitor_self = self.clone();
        let mut monitor_task = tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(2)).await;
                match p_monitor.is_connected().await {
                    Ok(false) | Err(_) => {
                        monitor_self
                            .shared
                            .is_connected
                            .store(false, Ordering::Relaxed);
                        break;
                    }
                    _ => {}
                }
            }
        });

        // \u2500\u2500 8. Wait for first task to finish (disconnect / error) \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500
        tokio::select! {
            _ = &mut recv_task    => { write_task.abort(); ping_task.abort(); monitor_task.abort(); }
            _ = &mut write_task   => { recv_task.abort();  ping_task.abort(); monitor_task.abort(); }
            _ = &mut ping_task    => { recv_task.abort();  write_task.abort(); monitor_task.abort(); }
            _ = &mut monitor_task => { recv_task.abort();  write_task.abort(); ping_task.abort(); }
        }

        // \u2500\u2500 9. Cleanup \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500
        *self.shared.ble_write_tx.lock().await = None;
        peripheral.disconnect().await.ok();

        Ok(())
    }
}
