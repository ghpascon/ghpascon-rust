use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::{Duration, sleep, timeout};
use uuid::Uuid;

use super::config::{ParamMap, SatoConfig};
use super::transport::{SharedEventHandler, default_event_handler, dispatch_event};
use super::types::SatoEvent;

pub(crate) struct SatoShared {
    pub is_connected: AtomicBool,
    pub running: AtomicBool,
    pub writer: tokio::sync::Mutex<Option<tokio::net::tcp::OwnedWriteHalf>>,
    pub print_queue: tokio::sync::Mutex<VecDeque<String>>,
    pub status: tokio::sync::Mutex<String>,
}

impl SatoShared {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            is_connected: AtomicBool::new(false),
            running: AtomicBool::new(true),
            writer: tokio::sync::Mutex::new(None),
            print_queue: tokio::sync::Mutex::new(VecDeque::new()),
            status: tokio::sync::Mutex::new(String::new()),
        })
    }
}

/// SATO label printer (TCP/IP connection).
///
/// `clone()` is cheap – all runtime state is behind an `Arc`.
pub struct SatoPrinter {
    pub config: SatoConfig,
    pub on_event: SharedEventHandler,
    pub(crate) shared: Arc<SatoShared>,
}

impl Clone for SatoPrinter {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            on_event: Arc::clone(&self.on_event),
            shared: Arc::clone(&self.shared),
        }
    }
}

impl Default for SatoPrinter {
    fn default() -> Self {
        Self::new(SatoConfig::default())
    }
}

impl SatoPrinter {
    pub fn new(config: SatoConfig) -> Self {
        Self {
            config,
            on_event: default_event_handler(),
            shared: SatoShared::new(),
        }
    }

    pub fn from_map(data: HashMap<String, serde_json::Value>) -> Self {
        Self::new(SatoConfig::from_map(data))
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

    pub fn can_print(&self) -> bool {
        self.is_connected()
    }

    pub fn pending_print_jobs(&self) -> usize {
        self.shared
            .print_queue
            .try_lock()
            .map(|queue| queue.len())
            .unwrap_or(0)
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
                                    let trimmed = line.trim().to_string();
                                    if !trimmed.is_empty() {
                                        *recv_self.shared.status.lock().await = trimmed.clone();
                                        dispatch_event(
                                            &recv_self.on_event,
                                            &recv_self.config.name,
                                            &SatoEvent::Status(trimmed),
                                        );
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
            &SatoEvent::Connection(false),
        );
    }

    pub async fn print(&self, zpl: &str) -> Result<String, String> {
        if !self.is_connected() {
            return Err("not connected".to_string());
        }
        let print_id = Uuid::new_v4().to_string();
        let data = zpl.as_bytes().to_vec();
        let mut guard = self.shared.writer.lock().await;
        if let Some(writer) = guard.as_mut() {
            writer
                .write_all(&data)
                .await
                .map_err(|e| format!("print error: {e}"))?;
        } else {
            return Err("not connected".to_string());
        }
        drop(guard);
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &SatoEvent::PrintSent(print_id.clone()),
        );
        Ok(print_id)
    }

    pub async fn add_to_print_queue(&self, labels: Vec<String>) {
        {
            let mut queue = self.shared.print_queue.lock().await;
            for label in labels {
                queue.push_back(label);
            }
        }
        if self.is_connected() {
            self.process_queue().await;
        }
    }

    pub async fn process_queue(&self) {
        let mut queue = self.shared.print_queue.lock().await;
        let items: Vec<String> = queue.drain(..).collect();
        drop(queue);
        for label in items {
            self.print(&label).await.ok();
        }
    }

    fn on_connected(&self) {
        self.shared.is_connected.store(true, Ordering::Relaxed);
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &SatoEvent::Connection(true),
        );
        if self.pending_print_jobs() > 0 {
            let printer = self.clone();
            tokio::spawn(async move {
                printer.process_queue().await;
            });
        }
    }

    fn on_disconnected(&self) {
        self.shared.is_connected.store(false, Ordering::Relaxed);
        dispatch_event(
            &self.on_event,
            &self.config.name,
            &SatoEvent::Connection(false),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn queued_labels_are_kept_until_connected() {
        let printer = SatoPrinter::default();
        assert!(!printer.can_print());

        printer
            .add_to_print_queue(vec!["^XA^XZ".to_string(), "^XA^FO50,50^XZ".to_string()])
            .await;

        assert_eq!(printer.pending_print_jobs(), 2);
    }
}
