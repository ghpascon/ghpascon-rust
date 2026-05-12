/// Default event handler for protocol events.
///
/// Can be used as a placeholder to handle specific events like
/// connection status changes or received messages.
///
/// # Arguments
///
/// * `name` - Identifier of the handler
/// * `event_type` - Type of event (`"connection"`, `"receive"`, etc.)
/// * `event_data` - Optional data associated with the event (any type that implements `Debug`)

pub fn dummy_event<T: std::fmt::Debug>(name: &str, event_type: &str, event_data: Option<T>) {
    println!(
        "{} -> 🔔 Event: {}, Data: {:?}",
        name, event_type, event_data
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_dummy_event() {
        dummy_event("TestHandler", "connection", Some("Connected"));
        dummy_event("TestHandler", "receive", None::<String>);
        dummy_event("TestHandler", "int", Some(10));
        dummy_event("TestHandler", "float", Some(3.14));
        dummy_event("TestHandler", "bool", Some(true));
        dummy_event("TestHandler", "custom", Some(vec![1, 2, 3]));

        // mixed-type map: values are boxed as dyn Debug
        let mut map: HashMap<&str, Box<dyn std::fmt::Debug>> = HashMap::new();
        map.insert("key", Box::new("value"));
        map.insert("number", Box::new(42));
        map.insert("flag", Box::new(true));
        dummy_event("TestHandler", "map", Some(map));
    }
}
