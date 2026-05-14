#[derive(Debug, Clone)]
pub enum SerialDeviceEvent {
    Connection(bool),
    Data(String),
}
