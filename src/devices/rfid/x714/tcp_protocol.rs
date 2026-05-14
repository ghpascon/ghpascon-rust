use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::sleep;

use super::types::X714Event;
use super::x714::X714;

impl X714 {
    /// Full TCP connection + reconnection loop.
    pub(crate) async fn run_tcp_loop(&self) {
        loop {
            if !self.shared.running.load(Ordering::Relaxed) {
                break;
            }

            let addr = format!("{}:{}", self.config.tcp.ip, self.config.tcp.port);

            eprintln!("[{}] TCP connecting to {}...", self.config.name, addr);

            match tokio::time::timeout(Duration::from_secs(3), TcpStream::connect(&addr)).await {
                Ok(Ok(stream)) => {
                    eprintln!("[{}] ✅ TCP connected", self.config.name);

                    // -----------------------------------------------------
                    // TCP tuning
                    // -----------------------------------------------------
                    if let Err(e) = stream.set_nodelay(true) {
                        eprintln!("[{}] set_nodelay error: {}", self.config.name, e);
                    }

                    self.shared.is_connected.store(true, Ordering::Relaxed);

                    // -----------------------------------------------------
                    // Shared heartbeat timestamp
                    // -----------------------------------------------------
                    let last_rx = Arc::new(AtomicU64::new(Self::now_millis()));

                    let (read_half, write_half) = stream.into_split();

                    *self.shared.writer.lock().await =
                        Some(Box::new(write_half) as Box<dyn tokio::io::AsyncWrite + Send + Unpin>);

                    self.on_connected().await;

                    // -----------------------------------------------------
                    // Spawn tasks
                    // -----------------------------------------------------
                    let recv_self = self.clone();
                    let heartbeat_self = self.clone();

                    let recv_last_rx = last_rx.clone();
                    let heartbeat_last_rx = last_rx.clone();

                    let mut recv_task = tokio::spawn(async move {
                        recv_self.tcp_receive_loop(read_half, recv_last_rx).await;
                    });

                    let mut heartbeat_task = tokio::spawn(async move {
                        heartbeat_self.tcp_heartbeat_loop(heartbeat_last_rx).await;
                    });

                    // -----------------------------------------------------
                    // Wait until one task exits
                    // -----------------------------------------------------
                    tokio::select! {
                        _ = &mut recv_task => {
                            heartbeat_task.abort();
                        }

                        _ = &mut heartbeat_task => {
                            recv_task.abort();
                        }
                    }

                    // -----------------------------------------------------
                    // Cleanup
                    // -----------------------------------------------------
                    self.shared.is_connected.store(false, Ordering::Relaxed);

                    *self.shared.writer.lock().await = None;

                    self.on_disconnected();

                    eprintln!("[{}] 🔌 TCP disconnected", self.config.name);
                }

                Ok(Err(e)) => {
                    eprintln!("[{}] TCP connect error: {}", self.config.name, e);
                }

                Err(_) => {
                    eprintln!("[{}] ⏱️ TCP connect timeout: {}", self.config.name, addr);
                }
            }

            if !self.shared.running.load(Ordering::Relaxed) {
                break;
            }

            sleep(Duration::from_secs(self.config.reconnection_time)).await;
        }
    }

    /// Continuous receive loop.
    async fn tcp_receive_loop(
        &self,
        reader: tokio::net::tcp::OwnedReadHalf,
        last_rx: Arc<AtomicU64>,
    ) {
        let mut buf_reader = BufReader::new(reader);

        let mut line = String::new();

        loop {
            if !self.shared.is_connected.load(Ordering::Relaxed) {
                break;
            }

            line.clear();

            match buf_reader.read_line(&mut line).await {
                Ok(0) => {
                    eprintln!("[{}] TCP EOF", self.config.name);

                    self.shared.is_connected.store(false, Ordering::Relaxed);

                    break;
                }

                Ok(_) => {
                    last_rx.store(Self::now_millis(), Ordering::Relaxed);

                    let trimmed = line.trim();

                    if !trimmed.is_empty() {
                        self.on_receive(trimmed);
                    }
                }

                Err(e) => {
                    eprintln!("[{}] TCP read error: {}", self.config.name, e);

                    self.shared.is_connected.store(false, Ordering::Relaxed);

                    break;
                }
            }
        }
    }

    /// Heartbeat + timeout detection.
    async fn tcp_heartbeat_loop(&self, last_rx: Arc<AtomicU64>) {
        const PING_INTERVAL: Duration = Duration::from_secs(2);

        const CONNECTION_TIMEOUT: Duration = Duration::from_secs(3);

        loop {
            sleep(PING_INTERVAL).await;

            if !self.shared.is_connected.load(Ordering::Relaxed) {
                break;
            }

            // -----------------------------------------------------
            // RX timeout
            // -----------------------------------------------------
            let now = Self::now_millis();

            let last = last_rx.load(Ordering::Relaxed);

            let elapsed = now.saturating_sub(last);

            if elapsed > CONNECTION_TIMEOUT.as_millis() as u64 {
                eprintln!("[{}] heartbeat timeout ({} ms)", self.config.name, elapsed);

                self.shared.is_connected.store(false, Ordering::Relaxed);

                break;
            }

            // -----------------------------------------------------
            // heartbeat ping
            // -----------------------------------------------------
            if let Err(e) = self.write("ping").await {
                eprintln!("[{}] heartbeat write failed: {}", self.config.name, e);

                self.shared.is_connected.store(false, Ordering::Relaxed);

                break;
            }
        }
    }

    fn now_millis() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    /// Dispatch a single event.
    pub fn dispatch(&self, event: &X714Event) {
        self.apply_event_to_state(event);

        super::transport::dispatch_event(&self.on_event, &self.config.name, event);
    }
}
