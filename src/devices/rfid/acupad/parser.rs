use super::types::{AcupadEvent, AcupadTag};

/// Parse a single raw line from the reader into zero or more typed events.
pub fn parse_line_to_events(input: &str) -> Vec<AcupadEvent> {
    let data = input
        .trim()
        .replace('\r', "")
        .replace('\n', "")
        .to_lowercase();
    if data.is_empty() {
        return Vec::new();
    }

    if data.starts_with("#read:") {
        let on = data.ends_with("on");
        return vec![AcupadEvent::Reading(on)];
    }

    if let Some(payload) = data.strip_prefix("#t+@") {
        return vec![AcupadEvent::Tag(parse_tag_frame(payload))];
    }

    if data == "#tags_cleared" {
        return vec![AcupadEvent::TagsCleared];
    }

    if data == "#setup_done" {
        return vec![AcupadEvent::SetupDone];
    }

    if data.starts_with("#name:") {
        let serial = data.strip_prefix("#name:").unwrap_or("").trim().to_string();
        return vec![AcupadEvent::SerialNumber(serial)];
    }

    vec![AcupadEvent::Receive(data)]
}

/// Parse the tag payload portion of a `#T+@...` frame.
pub fn parse_tag_frame(payload: &str) -> AcupadTag {
    let parts: Vec<&str> = payload.split('|').collect();
    let epc = parts
        .first()
        .copied()
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let tid = parts
        .get(1)
        .copied()
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let ant = parts
        .get(2)
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);
    let rssi = parts
        .get(3)
        .and_then(|v| v.parse::<i32>().ok())
        .map(|v| -v.abs())
        .unwrap_or(0);
    let protected = parts
        .get(4)
        .copied()
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    AcupadTag {
        epc,
        tid,
        ant,
        rssi,
        protected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tag_frame() {
        let events = parse_line_to_events("#T+@E28011|E20033|1|54|LOCKED");
        assert_eq!(events.len(), 1);
        let AcupadEvent::Tag(tag) = &events[0] else {
            panic!()
        };
        assert_eq!(tag.epc.as_deref(), Some("e28011"));
        assert_eq!(tag.rssi, -54);
        assert_eq!(tag.ant, 1);
    }

    #[test]
    fn parses_reading_on_off() {
        let on = parse_line_to_events("#READ:ON");
        assert!(matches!(on[0], AcupadEvent::Reading(true)));
        let off = parse_line_to_events("#READ:OFF");
        assert!(matches!(off[0], AcupadEvent::Reading(false)));
    }

    #[test]
    fn parses_serial_number() {
        let events = parse_line_to_events("#name:ACUPAD-99");
        assert!(matches!(&events[0], AcupadEvent::SerialNumber(s) if s == "acupad-99"));
    }

    #[test]
    fn empty_returns_empty() {
        assert!(parse_line_to_events("").is_empty());
        assert!(parse_line_to_events("  \r\n  ").is_empty());
    }
}
