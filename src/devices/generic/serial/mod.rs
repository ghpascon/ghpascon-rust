mod config;
pub mod config_example;
mod serial_device;
mod transport;
mod types;

pub use config::{ParamMap, SerialDeviceConfig};
pub use serial_device::SerialDevice;
pub use transport::{EventHandler, SharedEventHandler};
pub use types::SerialDeviceEvent;
