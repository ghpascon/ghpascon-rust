use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::{Duration, sleep};

use super::config::{ParamMap, SerialDeviceConfig};
use super::transport::{SharedEventHandler, default_event_handler, dispatch_event};
use super::types::SerialDeviceEvent;

pub(crate) struct SerialDeviceShared {
    pub is_connected: AtomicBool,
    pub writer: tokio::sync::Mutex<Option<Box<dyn tokio::io::AsyncWrite + Send + Unpin>>>,
    pub running: AtomicBool,
}

impl SerialDeviceShared {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            is_connected: AtomicBool::new(false),
            writer: tokio::sync::Mutex::new(None),
            running: AtomicBool::new(true),
        })
    }
}

/// Generic serial device.
///
/// `clone()` is cheap – all runtime state is behind an `Arc`.
pub struct SerialDevice {
    pub config: SerialDeviceConfig,
    pub on_event: SharedEventHandler,
    pub(crate) shared: Arc<SerialDeviceShared>,
}

impl Clone for SerialDevice {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            on_event: Arc::clone(&self.on_event),
            shared: Arc::clone(&self.shared),
        }
    }
}

impl Default for SerialDevice {
    fn default() -> Self {
        Self::new(SerialDeviceConfig::default())
    }
}

impl SerialDevice {
    pub fn new(config: SerialDeviceConfig) -> Self {
        Self {
            config,
            on_event: default_event_handler(),
            shared: SerialDeviceShared::new(),
        }
    }

    pub fn from_map(data: ParamMap) -> Self {
        Self::new(SerialDeviceConfig::from_map(data))
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
        format!(
            "SERIAL {} @ {} (VID={:#06x}, PID={:#06x})",
            self.config.port,
            self.config.baudrate,
            self.config.vid,
            self.config.pid
        )
    }

    pub async fn connect(&self) {
        self.shared.running.store(true, Ordering::Relaxed);
        loop {
            if !self.shared.running.load(Ordering::Relaxed) {
                break;
            }

            let port_name = if self.config.port.to_uppercase() == "AUTO" {
                match detect_serial_port(self.config.vid, self.config.pid) {
                    Some(p) => p,
                    None => {
                        eprintln!(
                            "[{}] No serial port found (VID={:#06x} PID={:#06x}), retrying in {}s",
                            self.config.name,
                            self.config.vid,
                            self.config.pid,
                            self.config.reconnection_time
                        );
                        sleep(Duration::from_secs(self.config.reconnection_time)).await;
                        continue;
                    }
                }
            } else {
                self.config.port.clone()
            };

            let builder = tokio_serial::new(&port_name, self.config.baudrate);
            match tokio_serial::SerialStream::open(&builder) {
                Ok(stream) => {
                    let (read_half, write_half) = tokio::io::split(stream);
                    *self.shared.writer.lock().await =
                        Some(Box::new(write_half) as Box<dyn tokio::io::AsyncWrite + Send + Unpin>);
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
                                    recv_self.shared.is_connected.store(false, Ordering::Relaxed);
                                    break;
                                }
                                Ok(_) => {
                                    let trimmed = line.trim();
                                    if !trimmed.is_empty() {
                                        recv_self.on_receive(trimmed);
                                    }
                                }
                                Err(_) => {
                                    recv_self.shared.is_connected.store(false, Ordering::Relaxed);
                                    break;
                                }
                            }
                        }
                    });
                    recv_task.await.ok();
                    *self.shared.writer.lock().await = None;
                    self.on_disconnected();
                }
                Err(e) => {
                    eprintln!("[{}] Serial open error: {}", self.config.name, e);
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
            &SerialDeviceEvent::Connection(false),
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
            &SerialDeviceEvent::Data(data.to_string()),
        );
    }

    fn on_connected(&self) {
        self.shared.is_connected.store(true, Ordering::Relaxed);
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &SerialDeviceEvent::Connection(true),
        );
    }

    fn on_disconnected(&self) {
        self.shared.is_connected.store(false, Ordering::Relaxed);
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &SerialDeviceEvent::Connection(false),
        );
    }
}

fn detect_serial_port(vid: u16, pid: u16) -> Option<String> {
    let ports = serialport::available_ports().ok()?;
    for port in ports {
        if let serialport::SerialPortType::UsbPort(info) = port.port_type {
            if info.vid == vid && info.pid == pid {
                return Some(port.port_name);
            }
        }
    }
    None
}
