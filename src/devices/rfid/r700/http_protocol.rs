use std::sync::atomic::Ordering;
use std::time::Duration;

use reqwest::{Client, ClientBuilder, Response};
use serde_json::Value;
use tokio::time::sleep;

use super::r700::R700;
use super::transport::dispatch_event;
use super::types::{R700Event, R700Tag};

impl R700 {
    // ─── HTTP client factory ──────────────────────────────────────────────────

    /// Build a reqwest client with Basic Auth and TLS verification disabled
    /// (R700 uses a self-signed certificate).
    pub(crate) fn make_client(&self) -> Result<Client, String> {
        ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("HTTP client error: {e}"))
    }

    /// Client without any timeout — used exclusively for the NDJSON event stream.
    fn make_stream_client(&self) -> Result<Client, String> {
        ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .build()
            .map_err(|e| format!("HTTP stream client error: {e}"))
    }

    // ─── Low-level HTTP helpers ────────────────────────────────────────────────

    pub(crate) async fn post_to_reader(
        &self,
        client: &Client,
        url: &str,
        payload: Option<&Value>,
    ) -> Result<(), String> {
        let req = if let Some(body) = payload {
            client
                .post(url)
                .basic_auth(&self.config.username, Some(&self.config.password))
                .json(body)
        } else {
            client
                .post(url)
                .basic_auth(&self.config.username, Some(&self.config.password))
        };
        match req.send().await {
            Ok(resp) if resp.status().as_u16() == 204 => Ok(()),
            Ok(resp) => {
                eprintln!(
                    "[{}] POST {} returned {}",
                    self.config.name,
                    url,
                    resp.status()
                );
                Err(format!("unexpected status {}", resp.status()))
            }
            Err(e) => Err(format!("POST {url}: {e}")),
        }
    }

    pub(crate) async fn put_to_reader(
        &self,
        client: &Client,
        url: &str,
        payload: &Value,
    ) -> Result<(), String> {
        match client
            .put(url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .json(payload)
            .send()
            .await
        {
            Ok(resp) if resp.status().as_u16() == 204 => Ok(()),
            Ok(resp) => {
                eprintln!(
                    "[{}] PUT {} returned {}",
                    self.config.name,
                    url,
                    resp.status()
                );
                Err(format!("unexpected status {}", resp.status()))
            }
            Err(e) => Err(format!("PUT {url}: {e}")),
        }
    }

    async fn get_json(&self, client: &Client, url: &str) -> Option<Value> {
        match client
            .get(url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => resp.json::<Value>().await.ok(),
            Ok(resp) => {
                eprintln!(
                    "[{}] GET {} returned {}",
                    self.config.name,
                    url,
                    resp.status()
                );
                None
            }
            Err(e) => {
                eprintln!("[{}] GET {}: {}", self.config.name, url, e);
                None
            }
        }
    }

    // ─── Setup steps ─────────────────────────────────────────────────────────

    async fn configure_interface(&self, client: &Client) -> bool {
        let url = format!("{}/system/rfid/interface", self.config.base_url());
        let payload = serde_json::json!({"rfidInterface": "rest"});
        self.put_to_reader(client, &url, &payload).await.is_ok()
    }

    async fn check_firmware_version(&self, client: &Client) -> bool {
        let url = format!("{}/system/image", self.config.base_url());
        let Some(info) = self.get_json(client, &url).await else {
            return false;
        };
        match &self.config.firmware_version {
            None => true,
            Some(expected) => {
                let actual = info
                    .get("primaryFirmware")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                actual.starts_with(expected.as_str())
            }
        }
    }

    async fn stop_profile(&self, client: &Client) -> bool {
        // Check current status first; skip if already idle
        let status_url = format!("{}/status", self.config.base_url());
        if let Some(status) = self.get_json(client, &status_url).await {
            if status.get("status").and_then(|v| v.as_str()) == Some("idle") {
                return true;
            }
        }
        let url = format!("{}/profiles/stop", self.config.base_url());
        self.post_to_reader(client, &url, None).await.is_ok()
    }

    async fn start_profile(&self, client: &Client) -> bool {
        let url = format!("{}/profiles/inventory/start", self.config.base_url());
        let body = self.config.build_reading_config();
        self.post_to_reader(client, &url, Some(&body)).await.is_ok()
    }

    async fn fetch_reader_info(&self, client: &Client) {
        let url = format!("{}/system", self.config.base_url());
        if let Some(info) = self.get_json(client, &url).await {
            if let Some(sn) = info.get("serialNumber").and_then(|v| v.as_str()) {
                *self.shared.serial_number.lock().unwrap() = Some(sn.to_string());
                dispatch_event(
                    &self.on_event,
                    &self.config.name,
                    &R700Event::SerialNumber(sn.to_string()),
                );
            }
        }
    }

    // ─── Tag stream ───────────────────────────────────────────────────────────

    /// Stream NDJSON events from `/data/stream` until connection drops or `running` is false.
    async fn stream_tags(&self) {
        let url = format!("{}/data/stream", self.config.base_url());
        let stream_client = match self.make_stream_client() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[{}] ❌ {}", self.config.name, e);
                return;
            }
        };
        let response: Response = match stream_client
            .get(&url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                eprintln!(
                    "[{}] ❌ Data stream returned {}",
                    self.config.name,
                    r.status()
                );
                return;
            }
            Err(e) => {
                eprintln!("[{}] ❌ Data stream error: {}", self.config.name, e);
                return;
            }
        };

        eprintln!("[{}] 📡 Data stream connected", self.config.name);

        let mut buffer = String::new();
        let mut stream = response;

        loop {
            if !self.shared.running.load(Ordering::Relaxed) {
                break;
            }
            match stream.chunk().await {
                Ok(Some(chunk)) => {
                    let text = String::from_utf8_lossy(&chunk);
                    buffer.push_str(&text);
                    // Drain processed lines without reallocating the whole buffer.
                    while let Some(pos) = buffer.find('\n') {
                        let line = buffer[..pos].trim().to_string();
                        buffer.drain(..pos + 1);
                        if !line.is_empty() {
                            self.process_stream_line(&line);
                        }
                    }
                }
                Ok(None) => {
                    eprintln!("[{}] 🔌 Data stream closed by reader", self.config.name);
                    break;
                }
                Err(e) => {
                    eprintln!("[{}] ❌ Data stream read error: {}", self.config.name, e);
                    break;
                }
            }
        }
    }

    fn process_stream_line(&self, line: &str) {
        let Ok(json) = serde_json::from_str::<Value>(line) else {
            return;
        };

        if let Some(status_evt) = json.get("inventoryStatusEvent") {
            let status = status_evt
                .get("inventoryStatus")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if status == "running" {
                self.shared.is_reading.store(true, Ordering::Relaxed);
                dispatch_event(&self.on_event, &self.config.name, &R700Event::Reading(true));
            } else {
                self.shared.is_reading.store(false, Ordering::Relaxed);
                dispatch_event(
                    &self.on_event,
                    &self.config.name,
                    &R700Event::Reading(false),
                );
            }
        } else if let Some(tag_evt) = json.get("tagInventoryEvent") {
            let tag = R700Tag::from_json(tag_evt);
            self.on_tag(tag);
        }
    }

    // ─── Main reconnection loop ───────────────────────────────────────────────

    /// HTTP reconnection loop, matching Python's `connect()`.
    pub(crate) async fn run_http_loop(&self) {
        while self.shared.running.load(Ordering::Relaxed) {
            self.shared.is_connected.store(false, Ordering::Relaxed);
            self.shared.is_reading.store(false, Ordering::Relaxed);

            eprintln!(
                "[{}] 🔌 Connecting to {}…",
                self.config.name, self.config.ip
            );

            let client = match self.make_client() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[{}] ❌ {}", self.config.name, e);
                    sleep(Duration::from_secs(self.config.reconnection_time)).await;
                    continue;
                }
            };

            // 1. Configure REST interface
            if !self.configure_interface(&client).await {
                eprintln!("[{}] ❌ Failed to configure interface", self.config.name);
                sleep(Duration::from_secs(self.config.reconnection_time)).await;
                continue;
            }

            // 2. Check firmware (skip if not specified)
            if !self.check_firmware_version(&client).await {
                eprintln!(
                    "[{}] ❌ Incompatible firmware (expected {:?})",
                    self.config.name, self.config.firmware_version
                );
                sleep(Duration::from_secs(5)).await;
                continue;
            }

            // 3. Stop any running profile
            if !self.stop_profile(&client).await {
                eprintln!("[{}] ⚠️  Could not stop profiles", self.config.name);
            }

            // 4. Retrieve reader info (serial number)
            self.fetch_reader_info(&client).await;

            // 5. Start inventory if needed
            if self.config.start_reading || self.config.gpi_start {
                if !self.start_profile(&client).await {
                    eprintln!("[{}] ❌ Failed to start inventory", self.config.name);
                    sleep(Duration::from_secs(self.config.reconnection_time)).await;
                    continue;
                }
                if self.config.start_reading {
                    self.shared.is_reading.store(true, Ordering::Relaxed);
                    dispatch_event(&self.on_event, &self.config.name, &R700Event::Reading(true));
                }
            }

            // 6. Mark connected and emit event
            self.on_connected();
            eprintln!("[{}] ✅ Connected", self.config.name);

            // 7. Stream events (blocks until stream ends or reader disconnects)
            self.stream_tags().await;

            // 8. Cleanup
            self.on_disconnected();

            if !self.shared.running.load(Ordering::Relaxed) {
                break;
            }

            eprintln!(
                "[{}] 🔄 Reconnecting in {}s…",
                self.config.name, self.config.reconnection_time
            );
            sleep(Duration::from_secs(self.config.reconnection_time)).await;
        }
    }
}
