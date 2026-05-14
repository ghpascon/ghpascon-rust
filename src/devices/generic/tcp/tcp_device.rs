use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::{Duration, sleep, timeout};

use super::config::{ParamMap, TcpDeviceConfig};
use super::transport::{SharedEventHandler, default_event_handler, dispatch_event};
use super::types::TcpDeviceEvent;

pub(crate) struct TcpDeviceShared {
    pub is_connected: AtomicBool,
    pub writer: tokio::sync::Mutex<Option<tokio::net::tcp::OwnedWriteHalf>>,
    pub running: AtomicBool,
}

impl TcpDeviceShared {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            is_connected: AtomicBool::new(false),
            writer: tokio::sync::Mutex::new(None),
            running: AtomicBool::new(true),
        })
    }
}

/// Generic TCP device.
///
/// `clone()` is cheap – all runtime state is behind an `Arc`.
pub struct TcpDevice {
    pub config: TcpDeviceConfig,
    pub on_event: SharedEventHandler,
    pub(crate) shared: Arc<TcpDeviceShared>,
}

impl Clone for TcpDevice {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            on_event: Arc::clone(&self.on_event),
            shared: Arc::clone(&self.shared),
        }
    }
}

impl Default for TcpDevice {
    fn default() -> Self {
        Self::new(TcpDeviceConfig::default())
    }
}

impl TcpDevice {
    pub fn new(config: TcpDeviceConfig) -> Self {
        Self {
            config,
            on_event: default_event_handler(),
            shared: TcpDeviceShared::new(),
        }
    }

    pub fn from_map(data: ParamMap) -> Self {
        Self::new(TcpDeviceConfig::from_map(data))
    }

    pub fn with_event_handler(mut self, handler: SharedEventHandler) -> Self {
        self.on_event = handler;
        self
    }

    pub fn set_event_handler(&mut self, handler: SharedEventHandler) {
        self.on_event = handler;
    }

    pub fn is_connected(&self) -> bool {
        self.shared.is_connected.load(Ordering::Relaxed)
    }

    pub fn to_map(&self) -> ParamMap {
        self.config.to_map()
    }

    pub fn connect_instruction(&self) -> String {
        format!("TCP {}:{}", self.config.ip, self.config.port)
    }

    pub async fn connect(&self) {
        self.shared.running.store(true, Ordering::Relaxed);
        loop {
            if !self.shared.running.load(Ordering::Relaxed) {
                break;
            }

            let addr = format!("{}:{}", self.config.ip, self.config.port);
            match timeout(
                Duration::from_secs(3),
                tokio::net::TcpStream::connect(&addr),
            )
            .await
            {
                Ok(Ok(stream)) => {
                    let (read_half, write_half) = stream.into_split();
                    *self.shared.writer.lock().await = Some(write_half);
                    self.on_connected();

                    let recv_self = self.clone();
                    let recv_task = tokio::spawn(async move {
                        let mut buf_reader = BufReader::new(read_half);
                        let mut line = String::new();
                        loop {
                            if !recv_self.shared.is_connected.load(Ordering::Relaxed) {
                                break;
                            }
                            line.clear();
                            match buf_reader.read_line(&mut line).await {
                                Ok(0) => {
                                    recv_self
                                        .shared
                                        .is_connected
                                        .store(false, Ordering::Relaxed);
                                    break;
                                }
                                Ok(_) => {
                                    let trimmed = line.trim();
                                    if !trimmed.is_empty() {
                                        recv_self.on_receive(trimmed);
                                    }
                                }
                                Err(_) => {
                                    recv_self
                                        .shared
                                        .is_connected
                                        .store(false, Ordering::Relaxed);
                                    break;
                                }
                            }
                        }
                    });
                    recv_task.await.ok();
                    *self.shared.writer.lock().await = None;
                    self.on_disconnected();
                }
                _ => {
                    eprintln!(
                        "[{}] TCP connection failed to {}, retrying in {}s",
                        self.config.name, addr, self.config.reconnection_time
                    );
                }
            }

            if !self.shared.running.load(Ordering::Relaxed) {
                break;
            }
            sleep(Duration::from_secs(self.config.reconnection_time)).await;
        }
    }

    pub async fn close(&self) {
        self.shared.running.store(false, Ordering::Relaxed);
        self.shared.is_connected.store(false, Ordering::Relaxed);
        *self.shared.writer.lock().await = None;
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &TcpDeviceEvent::Connection(false),
        );
    }

    pub async fn write(&self, data: &str) -> Result<(), String> {
        let frame = format!("{}\n", data.trim()).into_bytes();
        let mut guard = self.shared.writer.lock().await;
        if let Some(writer) = guard.as_mut() {
            writer
                .write_all(&frame)
                .await
                .map_err(|e| format!("write error: {e}"))
        } else {
            Err("not connected".to_string())
        }
    }

    pub fn on_receive(&self, data: &str) {
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &TcpDeviceEvent::Data(data.to_string()),
        );
    }

    fn on_connected(&self) {
        self.shared.is_connected.store(true, Ordering::Relaxed);
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &TcpDeviceEvent::Connection(true),
        );
    }

    fn on_disconnected(&self) {
        self.shared.is_connected.store(false, Ordering::Relaxed);
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &TcpDeviceEvent::Connection(false),
        );
    }
}
