#[derive(Debug, Clone)]
pub enum TcpDeviceEvent {
    Connection(bool),
    Data(String),
}
