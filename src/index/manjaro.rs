//! Helpers specific to Manjaro package identification.

/// Return true when a package name should be considered Manjaro-branded.
/// Current rule: name starts with "manjaro-".
pub fn is_name_manjaro(name: &str) -> bool {
    name.to_lowercase().starts_with("manjaro-")
}

/// Return true when either the package name or the owner indicates Manjaro.
///
/// - Name rule: starts with "manjaro-"
/// - Owner rule: contains the substring "manjaro" (case-insensitive)
pub fn is_manjaro_name_or_owner(name: &str, owner: &str) -> bool {
    let name_l = name.to_lowercase();
    let owner_l = owner.to_lowercase();
    name_l.starts_with("manjaro-") || owner_l.contains("manjaro")
}

#[cfg(test)]
mod tests {
    #[test]
    fn manjaro_name_detection() {
        assert!(super::is_name_manjaro("manjaro-alsa"));
        assert!(super::is_name_manjaro("Manjaro-foo"));
        assert!(!super::is_name_manjaro("alsa"));
    }

    #[test]
    fn manjaro_name_or_owner_detection() {
        assert!(super::is_manjaro_name_or_owner("manjaro-alsa", ""));
        assert!(super::is_manjaro_name_or_owner("alsa", "Manjaro Team"));
        assert!(!super::is_manjaro_name_or_owner("alsa", "Arch Linux"));
    }
}
