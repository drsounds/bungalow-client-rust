//! String helpers (port of `SPString.h`).

pub fn starts_with(value: &str, prefix: &str) -> bool {
    value.starts_with(prefix)
}

pub fn ends_with(value: &str, suffix: &str) -> bool {
    value.ends_with(suffix)
}

/// Splits on every delimiter, keeping empty parts (like the C++ helper).
pub fn split(value: &str, delimiter: char) -> Vec<String> {
    value.split(delimiter).map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_keeps_empty_parts() {
        assert_eq!(split("a::b", ':'), vec!["a", "", "b"]);
        assert_eq!(split("", ':'), vec![""]);
        assert!(starts_with("bungalow:artist", "bungalow:"));
        assert!(ends_with("index.xml", ".xml"));
    }
}
