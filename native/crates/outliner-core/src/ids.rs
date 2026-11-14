//! Node ids. Port of `packages/core/src/ids.ts`.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// 16 random chars from a URL-safe alphabet (96 bits).
pub fn new_id() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("system randomness");
    bytes.iter().map(|b| ALPHABET[usize::from(b & 63)] as char).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids_are_16_url_safe_chars() {
        let id = super::new_id();
        assert_eq!(id.len(), 16);
        assert!(id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'));
        assert_ne!(id, super::new_id());
    }
}
