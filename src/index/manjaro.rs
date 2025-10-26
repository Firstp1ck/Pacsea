//! Helpers specific to Manjaro package identification.

/// Return true when a package name should be considered Manjaro-branded.
/// Current rule: name starts with "manjaro-".
pub fn is_name_manjaro(name: &str) -> bool {
    name.to_lowercase().starts_with("manjaro-")
}

#[cfg(test)]
mod tests {
    #[test]
    fn manjaro_name_detection() {
        assert!(super::is_name_manjaro("manjaro-alsa"));
        assert!(super::is_name_manjaro("Manjaro-foo"));
        assert!(!super::is_name_manjaro("alsa"));
    }
}
