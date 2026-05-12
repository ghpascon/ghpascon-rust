use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::utils::dummy_event::dummy_event;

use super::types::{R700Event, R700Tag};

pub type EventHandler = dyn FnMut(&str, &str, Option<Value>) + Send + 'static;
pub type SharedEventHandler = Arc<Mutex<Box<EventHandler>>>;

pub fn default_event_handler() -> SharedEventHandler {
    Arc::new(Mutex::new(Box::new(|name, event_type, event_data| {
        dummy_event(name, event_type, event_data);
    })))
}

pub fn event_to_wire(event: &R700Event) -> (&'static str, Option<Value>) {
    match event {
        R700Event::Connection(v) => ("connection", Some(Value::Bool(*v))),
        R700Event::Reading(v) => ("reading", Some(Value::Bool(*v))),
        R700Event::Tag(tag) => ("tag", Some(tag_to_value(tag))),
        R700Event::SerialNumber(v) => ("serial_number", Some(Value::String(v.clone()))),
    }
}

pub fn dispatch_event(handler: &SharedEventHandler, name: &str, event: &R700Event) {
    let (event_type, payload) = event_to_wire(event);
    if let Ok(mut guard) = handler.lock() {
        (guard)(name, event_type, payload);
    }
}

fn tag_to_value(tag: &R700Tag) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert(
        "epc".to_string(),
        tag.epc.clone().map(Value::String).unwrap_or(Value::Null),
    );
    obj.insert(
        "tid".to_string(),
        tag.tid.clone().map(Value::String).unwrap_or(Value::Null),
    );
    obj.insert("ant".to_string(), Value::Number(tag.ant.into()));
    obj.insert("rssi".to_string(), Value::Number(tag.rssi.into()));
    obj.insert("protected".to_string(), Value::Bool(tag.protected));
    Value::Object(obj)
}
