use std::collections::HashMap;
use std::ops::Deref;

use serde_json::Value;

use super::config::SatoConfig;
use super::sato::SatoPrinter;
use super::transport::SharedEventHandler;

pub struct SatoWs4Printer(pub SatoPrinter);

impl Deref for SatoWs4Printer {
    type Target = SatoPrinter;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Clone for SatoWs4Printer {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl SatoWs4Printer {
    pub fn new(config: SatoConfig) -> Self {
        Self(SatoPrinter::new(config))
    }

    pub fn from_map(data: HashMap<String, Value>) -> Self {
        Self(SatoPrinter::from_map(data))
    }

    pub fn is_connected(&self) -> bool {
        self.0.is_connected()
    }

    pub fn connect_instruction(&self) -> String {
        self.0.connect_instruction()
    }

    pub fn set_event_handler(&mut self, handler: SharedEventHandler) {
        self.0.set_event_handler(handler)
    }

    pub async fn print(&self, zpl: &str) -> Result<String, String> {
        self.0.print(zpl).await
    }
}

impl Default for SatoWs4Printer {
    fn default() -> Self {
        let mut config = SatoConfig::default();
        config.ip = "192.168.1.102".to_string();
        config.name = "SATO_WS4".to_string();
        Self::new(config)
    }
}
