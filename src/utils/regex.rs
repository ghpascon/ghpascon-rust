use regex::Regex;

pub fn regex_hex(value: &str) -> bool {
    // only hex characters (0-9, a-f, A-F)
    let re = Regex::new(r"^[0-9a-fA-F]+$").unwrap();

    // validate pattern
    return re.is_match(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex_ok() {
        assert!(regex_hex("1a2b3c"));
        assert!(regex_hex("1A2B3C"));
        assert!(regex_hex("1a2b3c"));
    }

    #[test]
    fn test_non_hex() {
        assert!(!regex_hex("1a2b3g"));
        assert!(!regex_hex("xyz"));
        assert!(!regex_hex("12345z"));
    }
}
