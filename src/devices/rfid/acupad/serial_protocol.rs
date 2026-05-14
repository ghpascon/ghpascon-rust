use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::sleep;

use super::acupad::Acupad;

impl Acupad {
    /// Full serial connection + reconnection loop with VID/PID auto-detection.
    /// Runs forever until `self.shared.running` is set to `false`.
    pub(crate) async fn run_serial_loop(&self) {
        loop {
            if !self.shared.running.load(Ordering::Relaxed) {
                break;
            }

            let port_name = if self.config.port.to_uppercase() == "AUTO" {
                match detect_serial_port(self.config.vid, self.config.pid) {
                    Some(p) => {
                        eprintln!(
                            "[{}] 🔍 Auto-detected port: {} (VID={:#06x} PID={:#06x})",
                            self.config.name, p, self.config.vid, self.config.pid
                        );
                        p
                    }
                    None => {
                        eprintln!(
                            "[{}] ⚠️  No serial port with VID={:#06x} PID={:#06x} found – retrying in {}s…",
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

            eprintln!(
                "[{}] 🔌 Connecting to {} @ {} bps…",
                self.config.name, port_name, self.config.baudrate
            );

            let builder = tokio_serial::new(&port_name, self.config.baudrate);
            match tokio_serial::SerialStream::open(&builder) {
                Ok(stream) => {
                    eprintln!(
                        "[{}] ✅ Serial connected on {}",
                        self.config.name, port_name
                    );

                    let (read_half, write_half) = tokio::io::split(stream);
                    *self.shared.writer.lock().await =
                        Some(Box::new(write_half) as Box<dyn tokio::io::AsyncWrite + Send + Unpin>);

                    self.on_connected().await;

                    let recv_self = self.clone();
                    let recv_task = tokio::spawn(async move {
                        recv_self.serial_receive_loop(read_half).await;
                    });

                    recv_task.await.ok();

                    *self.shared.writer.lock().await = None;
                    self.on_disconnected();
                    eprintln!(
                        "[{}] 🔌 Serial disconnected, reconnecting…",
                        self.config.name
                    );
                }
                Err(e) => {
                    eprintln!(
                        "[{}] ❌ Serial open error on {}: {}",
                        self.config.name, port_name, e
                    );
                }
            }

            if !self.shared.running.load(Ordering::Relaxed) {
                break;
            }

            sleep(Duration::from_secs(self.config.reconnection_time)).await;
        }
    }

    pub(crate) async fn serial_receive_loop(
        &self,
        reader: tokio::io::ReadHalf<tokio_serial::SerialStream>,
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
                    self.shared.is_connected.store(false, Ordering::Relaxed);
                    break;
                }
                Ok(_) => {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        self.on_receive(trimmed);
                    }
                }
                Err(e) => {
                    eprintln!("[{}] Serial read error: {}", self.config.name, e);
                    self.shared.is_connected.store(false, Ordering::Relaxed);
                    break;
                }
            }
        }
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
