use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::sleep;

use super::types::X714Event;
use super::x714::X714;

impl X714 {
    /// Full TCP connection + reconnection loop, mirrors Python's `connect_tcp()`.
    /// Runs forever until `self.shared.running` is set to `false`.
    pub(crate) async fn run_tcp_loop(&self) {
        loop {
            if !self.shared.running.load(Ordering::Relaxed) {
                break;
            }

            let addr = format!("{}:{}", self.config.tcp.ip, self.config.tcp.port);
            eprintln!("[{}] TCP connecting to {}…", self.config.name, addr);

            match tokio::time::timeout(
                Duration::from_secs(3),
                tokio::net::TcpStream::connect(&addr),
            )
            .await
            {
                Ok(Ok(stream)) => {
                    eprintln!("[{}] ✅ TCP connected", self.config.name);

                    let (read_half, write_half) = stream.into_split();
                    *self.shared.writer.lock().await =
                        Some(Box::new(write_half) as Box<dyn tokio::io::AsyncWrite + Send + Unpin>);

                    self.on_connected().await;

                    // ── Spawn the three background tasks (like Python) ─────────────
                    let recv_self = self.clone();
                    let monitor_self = self.clone();
                    let ping_self = self.clone();

                    let mut recv_task = tokio::spawn(async move {
                        recv_self.tcp_receive_loop(read_half).await;
                    });
                    let mut monitor_task = tokio::spawn(async move {
                        monitor_self.tcp_monitor_loop().await;
                    });
                    let mut ping_task = tokio::spawn(async move {
                        ping_self.tcp_ping_loop(10).await;
                    });

                    // Wait until the first task finishes (connection dropped), then
                    // abort the remaining ones to avoid leaking background tasks.
                    tokio::select! {
                        _ = &mut recv_task    => { monitor_task.abort(); ping_task.abort(); },
                        _ = &mut monitor_task => { recv_task.abort();    ping_task.abort(); },
                        _ = &mut ping_task    => { recv_task.abort();    monitor_task.abort(); },
                    }

                    // ── Cleanup after disconnect ───────────────────────────────────
                    *self.shared.writer.lock().await = None;
                    self.on_disconnected();
                    eprintln!("[{}] 🔌 TCP disconnected, reconnecting…", self.config.name);
                }
                Ok(Err(e)) => {
                    eprintln!("[{}] TCP error: {}", self.config.name, e);
                }
                Err(_) => {
                    eprintln!(
                        "[{}] ⏱️ TCP timeout connecting to {}",
                        self.config.name, addr
                    );
                }
            }

            if !self.shared.running.load(Ordering::Relaxed) {
                break;
            }

            sleep(Duration::from_secs(self.config.reconnection_time)).await;
        }
    }

    /// Continuous read loop – reads lines and calls `on_receive`.
    async fn tcp_receive_loop(&self, reader: tokio::net::tcp::OwnedReadHalf) {
        let mut buf_reader = BufReader::new(reader);
        let mut line = String::new();

        loop {
            if !self.shared.is_connected.load(Ordering::Relaxed) {
                break;
            }

            line.clear();
            match tokio::time::timeout(Duration::from_millis(100), buf_reader.read_line(&mut line))
                .await
            {
                Ok(Ok(0)) => {
                    // EOF – connection closed by peer
                    eprintln!("[{}] TCP EOF", self.config.name);
                    self.shared.is_connected.store(false, Ordering::Relaxed);
                    break;
                }
                Ok(Ok(_)) => {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        self.on_receive(trimmed);
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("[{}] TCP read error: {}", self.config.name, e);
                    self.shared.is_connected.store(false, Ordering::Relaxed);
                    break;
                }
                Err(_) => {
                    // Timeout – nothing received, loop again
                }
            }
        }
    }

    /// Watches the connection health. Mirrors Python's `monitor_connection()`.
    async fn tcp_monitor_loop(&self) {
        let interval = Duration::from_secs(self.config.reconnection_time);
        while self.shared.is_connected.load(Ordering::Relaxed) {
            sleep(interval).await;
            if !self.shared.is_connected.load(Ordering::Relaxed) {
                break;
            }
            // Try to send a ping; failure marks disconnect
            if self.write("ping").await.is_err() {
                eprintln!(
                    "[{}] TCP monitor: ping failed, marking disconnected",
                    self.config.name
                );
                self.shared.is_connected.store(false, Ordering::Relaxed);
                break;
            }
        }
    }

    /// Sends a periodic ping. Mirrors Python's `periodic_ping()`.
    async fn tcp_ping_loop(&self, interval_secs: u64) {
        let interval = Duration::from_secs(interval_secs);
        while self.shared.is_connected.load(Ordering::Relaxed) {
            sleep(interval).await;
            if !self.shared.is_connected.load(Ordering::Relaxed) {
                break;
            }
            let _ = self.write("ping").await;
        }
    }

    /// Dispatch a single `X714Event` directly (used in transport tests / examples).
    pub fn dispatch(&self, event: &X714Event) {
        self.apply_event_to_state(event);
        super::transport::dispatch_event(&self.on_event, &self.config.name, event);
    }
}
