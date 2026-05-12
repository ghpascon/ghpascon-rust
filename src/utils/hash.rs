use sha2::{Digest, Sha256};

/// Returns the SHA-256 hex digest of the given string.
pub fn get_hash(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_hash() {
        // echo -n "hello" | sha256sum
        assert_eq!(
            get_hash("hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(
            get_hash(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_deterministic() {
        assert_eq!(get_hash("rust"), get_hash("rust"));
    }
}
