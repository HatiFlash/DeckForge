pub fn equal(a: &str, b: &str) -> bool {
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        diff |= usize::from(a.as_bytes().get(i).copied().unwrap_or(0) ^ b.as_bytes().get(i).copied().unwrap_or(0));
    }
    diff == 0
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equal_length_and_different_length_tokens() {
        assert!(equal("abcdef", "abcdef"));
        assert!(!equal("abcdef", "abcdeg"));
        assert!(!equal("abcdef", "abcde"));
    }
}
