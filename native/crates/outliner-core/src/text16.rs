//! Strings measured and cut in UTF-16 code units, as JS strings (and the platforms' text inputs) count them.
//! Offsets that fall inside a surrogate pair move to the start of that character: a Rust string cannot hold
//! half of one.

/// `s.length` in JS.
pub fn len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// The byte offset of UTF-16 offset `at` (clamped to the end; inside a pair: the pair's start).
pub fn byte_at(s: &str, at: usize) -> usize {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        if units + c.len_utf16() > at {
            return i;
        }
        units += c.len_utf16();
    }
    s.len()
}

/// `s.slice(a, b)` in JS, for non-negative offsets.
pub fn slice(s: &str, a: usize, b: usize) -> &str {
    if b <= a {
        return "";
    }
    &s[byte_at(s, a)..byte_at(s, b)]
}

/// `s.slice(a)` in JS.
pub fn slice_from(s: &str, a: usize) -> &str {
    &s[byte_at(s, a)..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_in_utf16_units() {
        let s = "a😀b한";
        assert_eq!(len(s), 5);
        assert_eq!(slice(s, 1, 3), "😀");
        assert_eq!(slice(s, 3, 9), "b한");
        assert_eq!(slice_from(s, 4), "한");
        assert_eq!(slice(s, 2, 1), "");
        // inside the pair: the character's start
        assert_eq!(slice(s, 0, 2), "a");
    }
}
