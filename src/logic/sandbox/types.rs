//! Type definitions for sandbox analysis.

/// What: Information about a dependency's status in the host environment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DependencyDelta {
    /// Package name (may include version requirements)
    pub name: String,
    /// Whether this dependency is installed on the host
    pub is_installed: bool,
    /// Installed version (if available)
    pub installed_version: Option<String>,
    /// Whether the installed version satisfies the requirement
    pub version_satisfied: bool,
}

/// What: Sandbox analysis result for an AUR package.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SandboxInfo {
    /// Package name
    pub package_name: String,
    /// Runtime dependencies (depends)
    pub depends: Vec<DependencyDelta>,
    /// Build-time dependencies (makedepends)
    pub makedepends: Vec<DependencyDelta>,
    /// Test dependencies (checkdepends)
    pub checkdepends: Vec<DependencyDelta>,
    /// Optional dependencies (optdepends)
    pub optdepends: Vec<DependencyDelta>,
}
