use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::time::sleep;

use super::x714::X714;

impl X714 {
    /// Full Serial connection + reconnection loop with VID/PID auto-detection.
    /// Mirrors Python's `connect_serial()` from `SerialProtocol`.
    /// Runs forever until `self.shared.running` is set to `false`.
    pub(crate) async fn run_serial_loop(&self) {
        loop {
            if !self.shared.running.load(Ordering::Relaxed) {
                break;
            }

            // ── Port selection ─────────────────────────────────────────────────
            let port_name = if self.config.serial.port.to_uppercase() == "AUTO" {
                match detect_serial_port(self.config.serial.vid, self.config.serial.pid) {
                    Some(p) => {
                        eprintln!(
                            "[{}] 🔍 Auto-detected port: {} (VID={:#06x} PID={:#06x})",
                            self.config.name, p, self.config.serial.vid, self.config.serial.pid
                        );
                        p
                    }
                    None => {
                        eprintln!(
                            "[{}] ⚠️  No serial port with VID={:#06x} PID={:#06x} found – retrying in {}s…",
                            self.config.name,
                            self.config.serial.vid,
                            self.config.serial.pid,
                            self.config.reconnection_time
                        );
                        sleep(Duration::from_secs(self.config.reconnection_time)).await;
                        continue;
                    }
                }
            } else {
                self.config.serial.port.clone()
            };

            eprintln!(
                "[{}] 🔌 Connecting to {} @ {} bps…",
                self.config.name, port_name, self.config.serial.baudrate
            );

            // ── Open port ─────────────────────────────────────────────────────
            let builder = tokio_serial::new(&port_name, self.config.serial.baudrate);
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

                    // ── Read loop ──────────────────────────────────────────────
                    let recv_self = self.clone();
                    let recv_task = tokio::spawn(async move {
                        recv_self.serial_receive_loop(read_half).await;
                    });

                    recv_task.await.ok();

                    // ── Cleanup ────────────────────────────────────────────────
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

    async fn serial_receive_loop(&self, reader: tokio::io::ReadHalf<tokio_serial::SerialStream>) {
        let mut reader = reader;
        let mut raw = [0u8; 2048];
        let mut rx_buffer = String::new();

        loop {
            if !self.shared.is_connected.load(Ordering::Relaxed) {
                break;
            }

            match reader.read(&mut raw).await {
                Ok(0) => {
                    // EOF – port disconnected
                    self.shared.is_connected.store(false, Ordering::Relaxed);
                    break;
                }
                Ok(n) => {
                    let chunk = String::from_utf8_lossy(&raw[..n]);
                    self.process_incoming_chunk(&chunk, &mut rx_buffer);
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

/// Enumerate available serial ports and return the device path matching the
/// given USB VID+PID, or `None` if not found.
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
