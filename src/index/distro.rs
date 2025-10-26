//! Distro-specific helpers used across the app.

/// Return true when a package name should be considered Manjaro-branded.
/// Current rule: name starts with "manjaro-" (case-insensitive).
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

/// Return true if repo name maps to EndeavourOS official repos.
pub fn is_eos_repo(repo: &str) -> bool {
    let r = repo.to_lowercase();
    r == "eos" || r == "endeavouros"
}

/// Return true if repo name maps to any CachyOS official repos.
pub fn is_cachyos_repo(repo: &str) -> bool {
    let r = repo.to_lowercase();
    r.starts_with("cachyos")
}

/// Known EndeavourOS repo names we may query with pacman -Sl.
pub fn eos_repo_names() -> &'static [&'static str] {
    &["eos", "endeavouros"]
}

/// Known CachyOS repo names we may query with pacman -Sl.
pub fn cachyos_repo_names() -> &'static [&'static str] {
    &[
        "cachyos",
        "cachyos-core",
        "cachyos-extra",
        "cachyos-v3",
        "cachyos-core-v3",
        "cachyos-extra-v3",
        "cachyos-v4",
        "cachyos-core-v4",
        "cachyos-extra-v4",
    ]
}

/// Heuristic: treat names containing "eos-" as EndeavourOS-branded when
/// reconstructing installed-only items not present in the official index.
pub fn is_eos_name(name: &str) -> bool {
    name.to_lowercase().contains("eos-")
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

    #[test]
    fn eos_and_cachyos_repo_rules() {
        assert!(super::is_eos_repo("eos"));
        assert!(super::is_eos_repo("EndeavourOS"));
        assert!(!super::is_eos_repo("core"));

        assert!(super::is_cachyos_repo("cachyos-core"));
        assert!(super::is_cachyos_repo("CachyOS-extra"));
        assert!(!super::is_cachyos_repo("extra"));
    }

    #[test]
    fn eos_name_rule() {
        assert!(super::is_eos_name("eos-hello"));
        assert!(super::is_eos_name("my-eos-helper"));
        assert!(!super::is_eos_name("hello"));
    }
}
