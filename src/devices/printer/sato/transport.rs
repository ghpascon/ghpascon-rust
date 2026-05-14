use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::utils::dummy_event::dummy_event;

use super::types::SatoEvent;

pub type EventHandler = dyn FnMut(&str, &str, Option<Value>) + Send + 'static;
pub type SharedEventHandler = Arc<Mutex<Box<EventHandler>>>;

pub fn default_event_handler() -> SharedEventHandler {
    Arc::new(Mutex::new(Box::new(|name, event_type, event_data| {
        dummy_event(name, event_type, event_data);
    })))
}

pub fn event_to_wire(event: &SatoEvent) -> (&'static str, Option<Value>) {
    match event {
        SatoEvent::Connection(value) => ("connection", Some(Value::Bool(*value))),
        SatoEvent::Status(value) => ("status", Some(Value::String(value.clone()))),
        SatoEvent::Error(value) => ("error", Some(Value::String(value.clone()))),
        SatoEvent::PrintSent(value) => ("print_sent", Some(Value::String(value.clone()))),
    }
}

pub fn dispatch_event(handler: &SharedEventHandler, name: &str, event: &SatoEvent) {
    let (event_type, payload) = event_to_wire(event);
    if let Ok(mut guard) = handler.lock() {
        (guard)(name, event_type, payload);
    }
}
