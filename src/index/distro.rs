//! Distro-specific helpers used across the app.

/// What: Determine if a package name is Manjaro-branded
///
/// Input:
/// - `name` package name
///
/// Output:
/// - `true` if it starts with "manjaro-" (case-insensitive)
///
/// Details:
/// - Compares a lowercased name with the "manjaro-" prefix.
pub fn is_name_manjaro(name: &str) -> bool {
    name.to_lowercase().starts_with("manjaro-")
}

/// What: Determine if a package or its owner indicates Manjaro
///
/// Input:
/// - `name` package name; `owner` maintainer/owner string
///
/// Output:
/// - `true` if name starts with "manjaro-" or owner contains "manjaro" (case-insensitive)
///
/// Details:
/// - Lowercases both inputs and checks the prefix/substring rules.
pub fn is_manjaro_name_or_owner(name: &str, owner: &str) -> bool {
    let name_l = name.to_lowercase();
    let owner_l = owner.to_lowercase();
    name_l.starts_with("manjaro-") || owner_l.contains("manjaro")
}

/// What: Check if a repo name is an `EndeavourOS` repo
///
/// Input:
/// - `repo` repository name
///
/// Output:
/// - `true` for "eos" or "endeavouros" (case-insensitive)
///
/// Details:
/// - Lowercases and matches exact names.
pub fn is_eos_repo(repo: &str) -> bool {
    let r = repo.to_lowercase();
    r == "eos" || r == "endeavouros"
}

/// What: Check if a repo name belongs to `CachyOS`
///
/// Input:
/// - `repo` repository name
///
/// Output:
/// - `true` if it starts with "cachyos" (case-insensitive)
///
/// Details:
/// - Lowercases and checks the "cachyos" prefix.
pub fn is_cachyos_repo(repo: &str) -> bool {
    let r = repo.to_lowercase();
    r.starts_with("cachyos")
}

/// What: Check if a repo name belongs to Artix Linux
///
/// Input:
/// - `repo` repository name
///
/// Output:
/// - `true` if it matches known Artix repository names (case-insensitive)
///
/// Details:
/// - Checks against the list of Artix repositories: omniverse, universe, lib32, galaxy, world, system.
pub fn is_artix_repo(repo: &str) -> bool {
    let r = repo.to_lowercase();
    matches!(
        r.as_str(),
        "omniverse" | "universe" | "lib32" | "galaxy" | "world" | "system"
    )
}

/// What: Check if a repo name is the Artix omniverse repository.
pub fn is_artix_omniverse(repo: &str) -> bool {
    repo.eq_ignore_ascii_case("omniverse")
}

/// What: Check if a repo name is the Artix universe repository.
pub fn is_artix_universe(repo: &str) -> bool {
    repo.eq_ignore_ascii_case("universe")
}

/// What: Check if a repo name is the Artix lib32 repository.
pub fn is_artix_lib32(repo: &str) -> bool {
    repo.eq_ignore_ascii_case("lib32")
}

/// What: Check if a repo name is the Artix galaxy repository.
pub fn is_artix_galaxy(repo: &str) -> bool {
    repo.eq_ignore_ascii_case("galaxy")
}

/// What: Check if a repo name is the Artix world repository.
pub fn is_artix_world(repo: &str) -> bool {
    repo.eq_ignore_ascii_case("world")
}

/// What: Check if a repo name is the Artix system repository.
pub fn is_artix_system(repo: &str) -> bool {
    repo.eq_ignore_ascii_case("system")
}

#[cfg(not(target_os = "windows"))]
/// What: Known `EndeavourOS` repo names usable with pacman -Sl
///
/// Output:
/// - Static slice of repo names
///
/// Details:
/// - Returns ["eos", "endeavouros"].
pub fn eos_repo_names() -> &'static [&'static str] {
    &["eos", "endeavouros"]
}

#[cfg(not(target_os = "windows"))]
/// What: Known `CachyOS` repo names usable with pacman -Sl
///
/// Output:
/// - Static slice of repo names
///
/// Details:
/// - Includes multiple generation-specific names (v3/v4) for compatibility.
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

#[cfg(not(target_os = "windows"))]
/// What: Known Artix Linux repo names usable with pacman -Sl
///
/// Output:
/// - Static slice of repo names
///
/// Details:
/// - Returns the standard Artix repositories: omniverse, universe, lib32, galaxy, world, system.
pub fn artix_repo_names() -> &'static [&'static str] {
    &[
        "omniverse",
        "universe",
        "lib32",
        "galaxy",
        "world",
        "system",
    ]
}

/// What: Heuristic to treat a name as EndeavourOS-branded
///
/// Input:
/// - `name` package name
///
/// Output:
/// - `true` if it contains "eos-" (case-insensitive)
///
/// Details:
/// - Used when reconstructing installed-only items not present in the official index.
pub fn is_eos_name(name: &str) -> bool {
    name.to_lowercase().contains("eos-")
}

#[cfg(test)]
mod tests {
    #[test]
    /// What: Validate Manjaro-specific name detection.
    ///
    /// Inputs:
    /// - Sample strings covering positive and negative cases.
    ///
    /// Output:
    /// - Assertions confirming only Manjaro-branded names return true.
    ///
    /// Details:
    /// - Exercises case-insensitive prefix handling.
    fn manjaro_name_detection() {
        assert!(super::is_name_manjaro("manjaro-alsa"));
        assert!(super::is_name_manjaro("Manjaro-foo"));
        assert!(!super::is_name_manjaro("alsa"));
    }

    #[test]
    /// What: Ensure Manjaro identification works on name or owner fields.
    ///
    /// Inputs:
    /// - Pairs of (name, owner) covering positive and negative scenarios.
    ///
    /// Output:
    /// - Assertions verifying either field triggers detection.
    ///
    /// Details:
    /// - Confirms substring search on owner and prefix match on name.
    fn manjaro_name_or_owner_detection() {
        assert!(super::is_manjaro_name_or_owner("manjaro-alsa", ""));
        assert!(super::is_manjaro_name_or_owner("alsa", "Manjaro Team"));
        assert!(!super::is_manjaro_name_or_owner("alsa", "Arch Linux"));
    }

    #[test]
    /// What: Confirm repo heuristics for EOS and CachyOS.
    ///
    /// Inputs:
    /// - Various repo strings spanning expected matches and misses.
    ///
    /// Output:
    /// - Assertions that only target repos return true.
    ///
    /// Details:
    /// - Checks both equality and prefix-based rules.
    fn eos_and_cachyos_repo_rules() {
        assert!(super::is_eos_repo("eos"));
        assert!(super::is_eos_repo("EndeavourOS"));
        assert!(!super::is_eos_repo("core"));

        assert!(super::is_cachyos_repo("cachyos-core"));
        assert!(super::is_cachyos_repo("CachyOS-extra"));
        assert!(!super::is_cachyos_repo("extra"));
    }

    #[test]
    /// What: Verify EOS-branded name heuristic.
    ///
    /// Inputs:
    /// - Strings with and without the "eos-" fragment.
    ///
    /// Output:
    /// - Assertions matching expected boolean results.
    ///
    /// Details:
    /// - Demonstrates case-insensitive substring detection.
    fn eos_name_rule() {
        assert!(super::is_eos_name("eos-hello"));
        assert!(super::is_eos_name("my-eos-helper"));
        assert!(!super::is_eos_name("hello"));
    }

    #[test]
    /// What: Confirm repo heuristics for Artix Linux.
    ///
    /// Inputs:
    /// - Various repo strings spanning expected matches and misses.
    ///
    /// Output:
    /// - Assertions that only Artix repos return true.
    ///
    /// Details:
    /// - Checks case-insensitive matching for all Artix repository names.
    fn artix_repo_rules() {
        assert!(super::is_artix_repo("omniverse"));
        assert!(super::is_artix_repo("Omniverse"));
        assert!(super::is_artix_repo("universe"));
        assert!(super::is_artix_repo("lib32"));
        assert!(super::is_artix_repo("galaxy"));
        assert!(super::is_artix_repo("world"));
        assert!(super::is_artix_repo("system"));
        assert!(!super::is_artix_repo("core"));
        assert!(!super::is_artix_repo("extra"));
        assert!(!super::is_artix_repo("cachyos"));
    }
}
