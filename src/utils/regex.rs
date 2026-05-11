use regex::Regex;

pub fn regex_hex(value: &str, len: Option<usize>) -> bool {
    // only hex characters (0-9, a-f, A-F)
    let re = Regex::new(r"^[0-9a-fA-F]+$").unwrap();

    // validate pattern
    if !re.is_match(value) {
        return false;
    }

    // validate length if provided
    if let Some(expected_len) = len {
        return value.len() == expected_len;
    }

    return true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex_ok() {
        assert!(regex_hex("1a2b3c", None));
        assert!(regex_hex("1A2B3C", None));
        assert!(regex_hex("1a2b3c", Some(6)));
    }

    #[test]
    fn test_non_hex() {
        assert!(!regex_hex("1a2b3g", None));
        assert!(!regex_hex("xyz", None));
        assert!(!regex_hex("12345z", None));
    }

    #[test]
    fn test_length() {
        assert!(regex_hex("1a2b3c", Some(6)));
        assert!(!regex_hex("1a2b3c", Some(5)));
        assert!(!regex_hex("1a2b3c", Some(7)));
    }
}
