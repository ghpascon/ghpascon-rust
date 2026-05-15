use std::future::Future;
use std::sync::atomic::Ordering;
use std::time::Duration;

use btleplug::api::{
    Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter,
    ValueNotification, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::StreamExt;
use tokio::time::{Instant, MissedTickBehavior, interval, sleep, timeout};
use uuid::Uuid;

use super::x714::X714;

/// Checks that all Linux BLE requirements are installed.
/// If any requirement is missing, prints install instructions and exits the process.
#[cfg(target_os = "linux")]
fn check_linux_ble_requirements(device_name: &str) {
    use std::process::Command;

    struct Requirement {
        name: &'static str,
        check_cmd: &'static str,
        check_arg: &'static str,
        install_cmd: &'static str,
    }

    let requirements = [
        Requirement {
            name: "bluez (bluetoothctl)",
            check_cmd: "which",
            check_arg: "bluetoothctl",
            install_cmd: "sudo apt-get install -y bluez",
        },
        Requirement {
            name: "dbus",
            check_cmd: "which",
            check_arg: "dbus-daemon",
            install_cmd: "sudo apt-get install -y dbus",
        },
    ];

    let mut missing: Vec<(&str, &str)> = Vec::new();

    for req in &requirements {
        let ok = Command::new(req.check_cmd)
            .arg(req.check_arg)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !ok {
            missing.push((req.name, req.install_cmd));
        }
    }

    let bt_active = Command::new("systemctl")
        .args(["is-active", "--quiet", "bluetooth"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !bt_active {
        missing.push(("serviço bluetooth", "sudo systemctl enable --now bluetooth"));
    }

    if missing.is_empty() {
        println!(
            "[ble:{}][linux][requirements_ok] All BLE requirements satisfied.",
            device_name
        );
        return;
    }

    println!("[ble:{}][linux][requirements_missing]", device_name);
    println!();
    println!("Missing Linux BLE requirements:");
    println!();
    for (name, cmd) in &missing {
        println!("  - {} is missing.", name);
        println!("    Run: {}", cmd);
        println!();
    }
    println!("After installing the requirements, restart the application.");
    std::process::exit(1);
}

const CONNECT_RETRIES: usize = 8;
const DISCOVER_RETRIES: usize = 3;
const SUBSCRIBE_RETRIES: usize = 3;

const SCAN_TIMEOUT: Duration = Duration::from_secs(5);
const PING_INTERVAL: Duration = Duration::from_secs(5);

const OP_TIMEOUT_SHORT: Duration = Duration::from_millis(900);
const OP_TIMEOUT_MEDIUM: Duration = Duration::from_millis(1600);
const OP_TIMEOUT_CONNECT: Duration = Duration::from_millis(2200);
const OP_TIMEOUT_DISCOVER: Duration = Duration::from_millis(2400);
const CONNECT_CONFIRM_TIMEOUT: Duration = Duration::from_millis(2400);

const STOP_SCAN_SETTLE: Duration = Duration::from_millis(180);
const BUSY_BACKOFF_BASE_MS: u64 = 180;
const CONNECT_POLL_INTERVAL: Duration = Duration::from_millis(120);

#[derive(Clone)]
struct BleCandidate {
    peripheral: Peripheral,
    name: String,
    address: String,
    rssi: Option<i16>,
    has_expected_service: bool,
}

fn log(device: &str, stage: &str, event: &str, extra: Option<&str>) {
    match extra {
        Some(v) => println!("[ble:{}][{}][{}] {}", device, stage, event, v),
        None => println!("[ble:{}][{}][{}]", device, stage, event),
    }
}

fn parse_uuid(raw: &str, field: &str) -> Result<Uuid, String> {
    Uuid::parse_str(raw).map_err(|e| format!("invalid {field}: {e}"))
}

fn is_busy_error(msg: &str) -> bool {
    let m = msg.to_lowercase();
    m.contains("operation already in progress")
        || m.contains("in progress")
        || m.contains("timeout")
}

impl X714 {
    async fn resolve_adapter(&self) -> Result<Adapter, String> {
        let manager = Manager::new().await.map_err(|e| e.to_string())?;
        let adapters = manager.adapters().await.map_err(|e| e.to_string())?;
        adapters
            .into_iter()
            .next()
            .ok_or_else(|| "no_ble_adapter_found".to_string())
    }

    fn matches_name(&self, name: &str) -> bool {
        let expected = self.config.ble.name.trim();
        if expected.is_empty() {
            return true;
        }
        let n = name.to_lowercase();
        let e = expected.to_lowercase();
        n == e || n.starts_with(&e)
    }

    async fn scan_and_pick_device(
        &self,
        adapter: &Adapter,
        service_uuid: Uuid,
    ) -> Result<Peripheral, String> {
        let fixed_address = self
            .config
            .ble
            .address
            .as_ref()
            .map(|v| v.trim().to_uppercase())
            .filter(|v| !v.is_empty());

        let _ = run_ble_op("stop previous scan", OP_TIMEOUT_SHORT, adapter.stop_scan()).await;
        run_ble_op(
            "start scan",
            OP_TIMEOUT_MEDIUM,
            adapter.start_scan(ScanFilter::default()),
        )
        .await?;

        log(
            &self.config.name,
            "scan",
            "start",
            Some(&format!("timeout={}s", SCAN_TIMEOUT.as_secs())),
        );

        let deadline = Instant::now() + SCAN_TIMEOUT;
        let mut candidates: Vec<BleCandidate> = Vec::new();

        while Instant::now() < deadline {
            let peripherals =
                run_ble_op("list peripherals", OP_TIMEOUT_SHORT, adapter.peripherals()).await?;

            for peripheral in peripherals {
                let props =
                    match run_ble_op("read properties", OP_TIMEOUT_SHORT, peripheral.properties())
                        .await
                    {
                        Ok(Some(v)) => v,
                        Ok(None) => continue,
                        Err(_) => continue,
                    };

                let name = props
                    .local_name
                    .clone()
                    .unwrap_or_else(|| "<sem_nome>".to_string());
                let address = props.address.to_string().to_uppercase();
                let has_expected_service = props.services.contains(&service_uuid);

                if let Some(fixed) = &fixed_address {
                    if address == *fixed {
                        log(
                            &self.config.name,
                            "scan",
                            "fixed_address_match",
                            Some(&address),
                        );
                        let _ =
                            run_ble_op("stop scan", OP_TIMEOUT_SHORT, adapter.stop_scan()).await;
                        sleep(STOP_SCAN_SETTLE).await;
                        return Ok(peripheral);
                    }
                    continue;
                }

                if !self.matches_name(&name) {
                    continue;
                }

                candidates.push(BleCandidate {
                    peripheral,
                    name,
                    address,
                    rssi: props.rssi,
                    has_expected_service,
                });
            }

            sleep(Duration::from_millis(220)).await;
        }

        let _ = run_ble_op("stop scan", OP_TIMEOUT_SHORT, adapter.stop_scan()).await;

        candidates
            .into_iter()
            .max_by_key(|c| {
                (
                    if c.has_expected_service { 1 } else { 0 },
                    c.rssi.unwrap_or(i16::MIN),
                )
            })
            .map(|c| {
                log(
                    &self.config.name,
                    "scan",
                    "selected",
                    Some(&format!("{} {} rssi={:?}", c.name, c.address, c.rssi)),
                );
                c.peripheral
            })
            .ok_or_else(|| "no_suitable_ble_device_found".to_string())
    }

    async fn connect_with_retry(&self, peripheral: &Peripheral) -> Result<(), String> {
        if run_ble_op(
            "check connected",
            OP_TIMEOUT_SHORT,
            peripheral.is_connected(),
        )
        .await
        .unwrap_or(false)
        {
            self.ensure_connected(peripheral).await?;
            return Ok(());
        }

        for attempt in 1..=CONNECT_RETRIES {
            let _ = run_ble_op(
                "preventive disconnect",
                OP_TIMEOUT_SHORT,
                peripheral.disconnect(),
            )
            .await;
            sleep(Duration::from_millis(120)).await;

            match run_ble_op("connect", OP_TIMEOUT_CONNECT, peripheral.connect()).await {
                Ok(_) => {
                    self.ensure_connected(peripheral).await?;
                    return Ok(());
                }
                Err(err) => {
                    let msg = err.to_lowercase();
                    if msg.contains("already connected") {
                        return Ok(());
                    }

                    if is_busy_error(&msg) {
                        let backoff_ms = BUSY_BACKOFF_BASE_MS * attempt as u64;
                        log(
                            &self.config.name,
                            "connect",
                            "busy_retry",
                            Some(&format!(
                                "attempt={attempt}/{CONNECT_RETRIES} wait={}ms",
                                backoff_ms
                            )),
                        );
                        sleep(Duration::from_millis(backoff_ms)).await;
                        continue;
                    }

                    return Err(err);
                }
            }
        }

        Err("connect_failed_ble_stack_busy".to_string())
    }

    async fn ensure_connected(&self, peripheral: &Peripheral) -> Result<(), String> {
        let deadline = Instant::now() + CONNECT_CONFIRM_TIMEOUT;

        while Instant::now() < deadline {
            if run_ble_op(
                "check connected",
                OP_TIMEOUT_SHORT,
                peripheral.is_connected(),
            )
            .await
            .unwrap_or(false)
            {
                return Ok(());
            }
            sleep(CONNECT_POLL_INTERVAL).await;
        }

        Err("connection_not_stable".to_string())
    }

    async fn discover_services_with_retry(&self, peripheral: &Peripheral) -> Result<(), String> {
        for attempt in 1..=DISCOVER_RETRIES {
            match run_ble_op(
                "discover services",
                OP_TIMEOUT_DISCOVER,
                peripheral.discover_services(),
            )
            .await
            {
                Ok(_) => return Ok(()),
                Err(err) => {
                    if attempt == DISCOVER_RETRIES {
                        return Err(err);
                    }
                    log(
                        &self.config.name,
                        "discover",
                        "retry",
                        Some(&format!("attempt={attempt}/{DISCOVER_RETRIES} err={err}")),
                    );
                    sleep(Duration::from_millis(220)).await;
                }
            }
        }

        Err("discover_unexpected_failure".to_string())
    }

    fn find_nus_characteristics(
        &self,
        peripheral: &Peripheral,
        service_uuid: Uuid,
        rx_uuid: Uuid,
        tx_uuid: Uuid,
    ) -> Result<(Characteristic, Characteristic), String> {
        let chars = peripheral.characteristics();
        let list: Vec<Characteristic> = chars.iter().cloned().collect();

        let has_nus_service = list.iter().any(|c| c.service_uuid == service_uuid);
        if !has_nus_service {
            return Err("nus_service_not_found".to_string());
        }

        let rx = list
            .iter()
            .find(|c| c.uuid == rx_uuid)
            .cloned()
            .or_else(|| {
                list.iter()
                    .find(|c| {
                        c.service_uuid == service_uuid
                            && (c.properties.contains(CharPropFlags::WRITE)
                                || c.properties.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE))
                    })
                    .cloned()
            })
            .ok_or_else(|| "nus_rx_characteristic_not_found".to_string())?;

        let tx = list
            .iter()
            .find(|c| c.uuid == tx_uuid)
            .cloned()
            .or_else(|| {
                list.iter()
                    .find(|c| {
                        c.service_uuid == service_uuid
                            && (c.properties.contains(CharPropFlags::NOTIFY)
                                || c.properties.contains(CharPropFlags::INDICATE))
                    })
                    .cloned()
            })
            .ok_or_else(|| "nus_tx_characteristic_not_found".to_string())?;

        Ok((rx, tx))
    }

    async fn subscribe_with_retry(
        &self,
        peripheral: &Peripheral,
        tx_char: &Characteristic,
    ) -> Result<(), String> {
        for attempt in 1..=SUBSCRIBE_RETRIES {
            if !run_ble_op(
                "check connected",
                OP_TIMEOUT_SHORT,
                peripheral.is_connected(),
            )
            .await
            .unwrap_or(false)
            {
                self.connect_with_retry(peripheral).await?;
                self.discover_services_with_retry(peripheral).await?;
            }

            match run_ble_op(
                "subscribe notify",
                OP_TIMEOUT_MEDIUM,
                peripheral.subscribe(tx_char),
            )
            .await
            {
                Ok(_) => return Ok(()),
                Err(err) => {
                    let msg = err.to_lowercase();
                    let needs_reconnect = msg.contains("not connected")
                        || msg.contains("failed to register notify session")
                        || msg.contains("no such file or directory");

                    if attempt == SUBSCRIBE_RETRIES {
                        return Err(err);
                    }

                    if needs_reconnect {
                        let _ = run_ble_op(
                            "disconnect to recover subscribe",
                            OP_TIMEOUT_SHORT,
                            peripheral.disconnect(),
                        )
                        .await;
                        sleep(Duration::from_millis(180)).await;
                        self.connect_with_retry(peripheral).await?;
                        self.discover_services_with_retry(peripheral).await?;
                    }

                    log(
                        &self.config.name,
                        "notify",
                        "retry",
                        Some(&format!("attempt={attempt}/{SUBSCRIBE_RETRIES} err={err}")),
                    );
                    sleep(Duration::from_millis(180)).await;
                }
            }
        }

        Err("subscribe_unexpected_failure".to_string())
    }

    async fn write_ble_payload(
        &self,
        peripheral: &Peripheral,
        rx_char: &Characteristic,
        payload: &[u8],
    ) -> Result<(), String> {
        match run_ble_op(
            "write without response",
            OP_TIMEOUT_MEDIUM,
            peripheral.write(rx_char, payload, WriteType::WithoutResponse),
        )
        .await
        {
            Ok(_) => Ok(()),
            Err(first_err) => {
                let second = run_ble_op(
                    "write with response",
                    OP_TIMEOUT_MEDIUM,
                    peripheral.write(rx_char, payload, WriteType::WithResponse),
                )
                .await;
                match second {
                    Ok(_) => Ok(()),
                    Err(second_err) => Err(format!(
                        "ble_write_failed: first={first_err}; fallback={second_err}"
                    )),
                }
            }
        }
    }

    fn process_notification_frame(
        &self,
        notification: ValueNotification,
        expected_tx_uuid: Uuid,
        buffer: &mut String,
    ) {
        if notification.uuid != expected_tx_uuid {
            return;
        }

        let text = String::from_utf8_lossy(&notification.value);
        buffer.push_str(&text);
        buffer.retain(|c| c != '\0');

        loop {
            let next_lf = buffer.find('\n');
            let next_cr = buffer.find('\r');
            let pos = match (next_lf, next_cr) {
                (Some(a), Some(b)) => a.min(b),
                (Some(a), None) => a,
                (None, Some(b)) => b,
                (None, None) => break,
            };

            if pos == 0 {
                buffer.drain(..=pos);
                continue;
            }

            let line: String = buffer.drain(..=pos).collect();
            let trimmed = line.trim_matches(|c| c == '\r' || c == '\n').trim();
            if !trimmed.is_empty() {
                self.on_receive(trimmed);
            }
        }

        // If frames arrive without CR/LF, many firmwares still prefix each frame with '#'.
        // Split concatenated buffers using this marker: "#frame1#frame2...".
        loop {
            let Some(first_hash) = buffer.find('#') else {
                buffer.clear();
                break;
            };

            if first_hash > 0 {
                buffer.drain(..first_hash);
            }

            let rest = &buffer[1..];
            let Some(next_rel) = rest.find('#') else {
                break;
            };
            let split_at = 1 + next_rel;

            let frame = buffer[..split_at].trim();
            if !frame.is_empty() {
                self.on_receive(frame);
            }

            buffer.drain(..split_at);
        }

        // Some BLE firmwares send one full frame per notification without CR/LF.
        // Handle known complete patterns directly to mirror Python behavior.
        let trimmed = buffer.trim();
        let looks_complete_tag = if let Some(payload) = trimmed.strip_prefix("#t+@") {
            let payload = payload.trim();
            let simple_hex = !payload.is_empty()
                && payload.len() >= 8
                && payload.chars().all(|c| c.is_ascii_hexdigit());
            payload.contains('|') || simple_hex
        } else {
            false
        };
        let looks_complete_meta = trimmed.starts_with("#read:")
            || trimmed.starts_with("#start_reading:")
            || trimmed.starts_with("#name:")
            || trimmed == "#setup_done"
            || trimmed == "#tags_cleared"
            || trimmed == "#pong";

        if !trimmed.is_empty() && (looks_complete_tag || looks_complete_meta) {
            self.on_receive(trimmed);
            buffer.clear();
        }
    }

    async fn ble_connect_once(&self, attempt: u32) -> Result<(), String> {
        log(
            &self.config.name,
            "connect",
            "attempt_start",
            Some(&format!("attempt={attempt}")),
        );

        let service_uuid = parse_uuid(&self.config.ble.service_uuid, "ble_service_uuid")?;
        let rx_uuid = parse_uuid(&self.config.ble.rx_uuid, "ble_rx_uuid")?;
        let tx_uuid = parse_uuid(&self.config.ble.tx_uuid, "ble_tx_uuid")?;

        let adapter = self.resolve_adapter().await?;
        let peripheral = self.scan_and_pick_device(&adapter, service_uuid).await?;

        self.connect_with_retry(&peripheral).await?;
        self.discover_services_with_retry(&peripheral).await?;

        let (rx_char, tx_char) =
            self.find_nus_characteristics(&peripheral, service_uuid, rx_uuid, tx_uuid)?;

        let mut notifications = run_ble_op(
            "open notifications stream",
            OP_TIMEOUT_MEDIUM,
            peripheral.notifications(),
        )
        .await?;
        self.subscribe_with_retry(&peripheral, &tx_char).await?;

        let (write_tx, mut write_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        *self.shared.ble_write_tx.lock().await = Some(write_tx);

        self.on_connected().await;

        let mut ping_interval = interval(PING_INTERVAL);
        ping_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut health_interval = interval(Duration::from_secs(2));
        health_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut rx_buffer = String::new();

        loop {
            if !self.shared.running.load(Ordering::Relaxed) {
                break;
            }

            tokio::select! {
                _ = ping_interval.tick() => {
                    if !run_ble_op("check connected", OP_TIMEOUT_SHORT, peripheral.is_connected()).await.unwrap_or(false) {
                        return Err("device_disconnected".to_string());
                    }
                    self.write_ble_payload(&peripheral, &rx_char, b"#ping").await?;
                }
                _ = health_interval.tick() => {
                    if !run_ble_op("check connected", OP_TIMEOUT_SHORT, peripheral.is_connected()).await.unwrap_or(false) {
                        return Err("device_disconnected".to_string());
                    }
                }
                maybe_cmd = write_rx.recv() => {
                    match maybe_cmd {
                        Some(payload) => {
                            self.write_ble_payload(&peripheral, &rx_char, &payload).await?;
                            sleep(Duration::from_millis(150)).await;
                        }
                        None => return Err("ble_write_channel_closed".to_string()),
                    }
                }
                maybe_notification = notifications.next() => {
                    match maybe_notification {
                        Some(notification) => self.process_notification_frame(notification, tx_char.uuid, &mut rx_buffer),
                        None => return Err("notification_stream_closed".to_string()),
                    }
                }
            }
        }

        let _ = run_ble_op("disconnect", OP_TIMEOUT_SHORT, peripheral.disconnect()).await;
        Ok(())
    }

    pub(crate) async fn run_ble_loop(&self) {
        #[cfg(target_os = "linux")]
        check_linux_ble_requirements(&self.config.name);

        let mut attempt = 0u32;

        loop {
            if !self.shared.running.load(Ordering::Relaxed) {
                break;
            }

            match self.ble_connect_once(attempt).await {
                Ok(_) => {
                    attempt = 0;
                    log(&self.config.name, "loop", "connection_stable", None);
                }
                Err(e) => {
                    log(&self.config.name, "loop", "error", Some(&e));
                    attempt += 1;
                }
            }

            *self.shared.ble_write_tx.lock().await = None;
            self.on_disconnected();

            let backoff = self.calculate_backoff(attempt);
            log(
                &self.config.name,
                "loop",
                "reconnect_wait",
                Some(&format!("backoff={}s", backoff)),
            );
            sleep(Duration::from_secs(backoff)).await;
        }
    }

    fn calculate_backoff(&self, attempt: u32) -> u64 {
        self.config.reconnection_time * 2u64.saturating_pow(attempt.min(5))
    }
}

async fn run_ble_op<T, F>(label: &str, timeout_dur: Duration, fut: F) -> Result<T, String>
where
    F: Future<Output = Result<T, btleplug::Error>>,
{
    match timeout(timeout_dur, fut).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(err)) => Err(format!("{label} failed: {err}")),
        Err(_) => Err(format!(
            "{label} timeout after {}ms",
            timeout_dur.as_millis()
        )),
    }
}
