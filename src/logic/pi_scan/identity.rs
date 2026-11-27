//! Package, repository, commit, and scan target identity types for Pi scanning.

use std::fmt;
use std::str::FromStr;

/// What: Error types that occur during identity validation or construction.
///
/// Inputs:
/// - Input string or parameter that failed identity validation.
///
/// Output:
/// - Structured error describing the identity validation failure.
///
/// Details:
/// - Provides distinct variants for invalid package identifiers, commit OIDs, and AUR repository URLs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityError {
    /// Package name or package base string is invalid.
    InvalidPackageName {
        /// Raw value that failed validation.
        input: String,
        /// Reason for validation failure.
        reason: String,
    },
    /// Commit OID string is invalid.
    InvalidCommitOid {
        /// Raw value that failed validation.
        input: String,
        /// Reason for validation failure.
        reason: String,
    },
    /// Official AUR repository URL is invalid.
    InvalidAurRepoUrl {
        /// Raw value that failed validation.
        input: String,
        /// Reason for validation failure.
        reason: String,
    },
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPackageName { input, reason } => {
                write!(f, "Invalid package name/base '{input}': {reason}")
            }
            Self::InvalidCommitOid { input, reason } => {
                write!(f, "Invalid commit OID '{input}': {reason}")
            }
            Self::InvalidAurRepoUrl { input, reason } => {
                write!(f, "Invalid AUR repository URL '{input}': {reason}")
            }
        }
    }
}

impl std::error::Error for IdentityError {}

/// What: Validate an Arch package name or package base string against strict rules.
///
/// Inputs:
/// - `s`: String slice to validate.
///
/// Output:
/// - `Ok(())` if valid, `Err(IdentityError)` with descriptive reason otherwise.
///
/// Details:
/// - Rejects empty input, length > 255, leading dashes `-`, uppercase letters, slashes, parent paths `..`, control characters, whitespace, and shell metacharacters.
///
/// # Errors
/// Returns `IdentityError::InvalidPackageName` if validation constraints are violated.
fn validate_package_identifier(s: &str) -> Result<(), IdentityError> {
    if s.is_empty() {
        return Err(IdentityError::InvalidPackageName {
            input: s.to_string(),
            reason: "package identifier cannot be empty".to_string(),
        });
    }
    if s.len() > 255 {
        return Err(IdentityError::InvalidPackageName {
            input: s.to_string(),
            reason: "package identifier exceeds maximum length of 255 characters".to_string(),
        });
    }
    if s.starts_with('-') {
        return Err(IdentityError::InvalidPackageName {
            input: s.to_string(),
            reason: "package identifier cannot begin with a leading dash".to_string(),
        });
    }

    let Some(first) = s.chars().next() else {
        return Err(IdentityError::InvalidPackageName {
            input: s.to_string(),
            reason: "package identifier cannot be empty".to_string(),
        });
    };

    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return Err(IdentityError::InvalidPackageName {
            input: s.to_string(),
            reason: "package identifier must start with a lowercase ASCII letter or digit"
                .to_string(),
        });
    }

    for c in s.chars() {
        if !c.is_ascii_lowercase()
            && !c.is_ascii_digit()
            && c != '+'
            && c != '.'
            && c != '_'
            && c != '-'
        {
            return Err(IdentityError::InvalidPackageName {
                input: s.to_string(),
                reason: format!("package identifier contains forbidden character '{c}'"),
            });
        }
    }

    if s.contains("..") {
        return Err(IdentityError::InvalidPackageName {
            input: s.to_string(),
            reason: "package identifier cannot contain parent directory traversal '..'".to_string(),
        });
    }

    Ok(())
}

/// What: Validated Arch Linux package name.
///
/// Inputs:
/// - A string representing an individual package name.
///
/// Output:
/// - Guaranteed non-empty, lowercase, injection-safe package name.
///
/// Details:
/// - Strictly follows Arch Linux package naming rules: `^[a-z0-9][a-z0-9+._-]*$`.
/// - Rejects leading dashes `-`, slashes `/`, `..`, whitespace, NUL/control characters, uppercase letters, and shell metacharacters.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct PackageName(String);

impl PackageName {
    /// What: Validate and construct a `PackageName`.
    ///
    /// Inputs:
    /// - `input`: Raw string candidate for a package name.
    ///
    /// Output:
    /// - `Ok(PackageName)` if valid, `Err(IdentityError)` otherwise.
    ///
    /// Details:
    /// - Enforces length boundaries (1..=255) and strict safe character set.
    ///
    /// # Errors
    /// Returns `IdentityError::InvalidPackageName` if input violates naming rules.
    pub fn new(input: impl AsRef<str>) -> Result<Self, IdentityError> {
        let s = input.as_ref();
        validate_package_identifier(s)?;
        Ok(Self(s.to_string()))
    }

    /// What: Borrow the underlying validated package name as a string slice.
    ///
    /// Inputs: None
    ///
    /// Output:
    /// - String slice of the package name.
    ///
    /// Details:
    /// - Zero-copy string borrow.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PackageName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for PackageName {
    type Err = IdentityError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

/// What: Validated Arch Linux package base name (`pkgbase`).
///
/// Inputs:
/// - A string representing a package base name.
///
/// Output:
/// - Guaranteed non-empty, lowercase, injection-safe package base name.
///
/// Details:
/// - Follows the same validation constraints as `PackageName`.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct PackageBase(String);

impl PackageBase {
    /// What: Validate and construct a `PackageBase`.
    ///
    /// Inputs:
    /// - `input`: Raw string candidate for a package base.
    ///
    /// Output:
    /// - `Ok(PackageBase)` if valid, `Err(IdentityError)` otherwise.
    ///
    /// Details:
    /// - Enforces length boundaries (1..=255) and strict safe character set.
    ///
    /// # Errors
    /// Returns `IdentityError::InvalidPackageName` if input violates naming rules.
    pub fn new(input: impl AsRef<str>) -> Result<Self, IdentityError> {
        let s = input.as_ref();
        validate_package_identifier(s)?;
        Ok(Self(s.to_string()))
    }

    /// What: Borrow the underlying validated package base as a string slice.
    ///
    /// Inputs: None
    ///
    /// Output:
    /// - String slice of the package base.
    ///
    /// Details:
    /// - Zero-copy string borrow.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PackageBase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for PackageBase {
    type Err = IdentityError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

/// What: Immutable 40-character hexadecimal Git commit OID.
///
/// Inputs:
/// - A 40-character hexadecimal string.
///
/// Output:
/// - Strictly validated, lowercase 40-hex commit OID.
///
/// Details:
/// - Rejects abbreviated OIDs, non-hex characters, and invalid lengths.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct CommitOid(String);

impl CommitOid {
    /// What: Validate and construct a `CommitOid`.
    ///
    /// Inputs:
    /// - `input`: Raw string candidate for Git commit OID.
    ///
    /// Output:
    /// - `Ok(CommitOid)` if valid, `Err(IdentityError)` otherwise.
    ///
    /// Details:
    /// - Converts uppercase hex characters to lowercase automatically.
    ///
    /// # Errors
    /// Returns `IdentityError::InvalidCommitOid` if input is not 40 hexadecimal characters.
    pub fn new(input: impl AsRef<str>) -> Result<Self, IdentityError> {
        let s = input.as_ref();
        if s.len() != 40 {
            return Err(IdentityError::InvalidCommitOid {
                input: s.to_string(),
                reason: format!("commit OID must be exactly 40 characters, got {}", s.len()),
            });
        }
        for c in s.chars() {
            if !c.is_ascii_hexdigit() {
                return Err(IdentityError::InvalidCommitOid {
                    input: s.to_string(),
                    reason: format!("commit OID contains non-hexadecimal character '{c}'"),
                });
            }
        }
        Ok(Self(s.to_ascii_lowercase()))
    }

    /// What: Borrow the normalized commit OID as a string slice.
    ///
    /// Inputs: None
    ///
    /// Output:
    /// - String slice of 40 lowercase hexadecimal characters.
    ///
    /// Details:
    /// - Zero-copy borrow.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CommitOid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for CommitOid {
    type Err = IdentityError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

/// What: Canonical official AUR Git repository URL.
///
/// Inputs:
/// - Package base name and canonical host `aur.archlinux.org`.
///
/// Output:
/// - Guaranteed canonical official AUR repo URL in format `https://aur.archlinux.org/<pkgbase>.git`.
///
/// Details:
/// - Rejects non-HTTPS schemes, alternate hosts, custom ports, userinfo, query strings, fragments, and invalid package bases.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct AurRepoUrl(String);

impl AurRepoUrl {
    /// What: Construct a canonical official AUR repository URL for a package base.
    ///
    /// Inputs:
    /// - `pkgbase`: Validated package base.
    ///
    /// Output:
    /// - Canonical `AurRepoUrl`.
    ///
    /// Details:
    /// - Formats `https://aur.archlinux.org/<pkgbase>.git`.
    #[must_use]
    pub fn for_package_base(pkgbase: &PackageBase) -> Self {
        Self(format!(
            "https://aur.archlinux.org/{}.git",
            pkgbase.as_str()
        ))
    }

    /// What: Parse and validate a string candidate as a canonical official AUR repository URL.
    ///
    /// Inputs:
    /// - `input`: Raw URL string slice.
    ///
    /// Output:
    /// - `Ok((AurRepoUrl, PackageBase))` if canonical, `Err(IdentityError)` otherwise.
    ///
    /// Details:
    /// - Verifies scheme is `https`, host is `aur.archlinux.org`, port is default, path matches `/<pkgbase>.git`, no query/fragment/userinfo.
    ///
    /// # Errors
    /// Returns `IdentityError::InvalidAurRepoUrl` if URL is non-canonical or malformed.
    pub fn parse_canonical(input: &str) -> Result<(Self, PackageBase), IdentityError> {
        if input.contains('?') {
            return Err(IdentityError::InvalidAurRepoUrl {
                input: input.to_string(),
                reason: "URL must not contain query parameters".to_string(),
            });
        }
        if input.contains('#') {
            return Err(IdentityError::InvalidAurRepoUrl {
                input: input.to_string(),
                reason: "URL must not contain fragment identifier".to_string(),
            });
        }
        if input.contains('@') {
            return Err(IdentityError::InvalidAurRepoUrl {
                input: input.to_string(),
                reason: "URL must not contain userinfo/credentials".to_string(),
            });
        }

        let prefix = "https://aur.archlinux.org/";
        if !input.starts_with(prefix) {
            return Err(IdentityError::InvalidAurRepoUrl {
                input: input.to_string(),
                reason: format!("URL must start with '{prefix}'"),
            });
        }

        let rest = &input[prefix.len()..];
        if !rest.to_ascii_lowercase().ends_with(".git") {
            return Err(IdentityError::InvalidAurRepoUrl {
                input: input.to_string(),
                reason: "URL path must end with '.git'".to_string(),
            });
        }

        let pkgbase_str = &rest[..rest.len() - 4];
        if pkgbase_str.contains('/') {
            return Err(IdentityError::InvalidAurRepoUrl {
                input: input.to_string(),
                reason: "URL path contains extra directory segments".to_string(),
            });
        }

        let pkgbase =
            PackageBase::new(pkgbase_str).map_err(|e| IdentityError::InvalidAurRepoUrl {
                input: input.to_string(),
                reason: format!("URL package base is invalid: {e}"),
            })?;

        Ok((Self(format!("{prefix}{pkgbase_str}.git")), pkgbase))
    }

    /// What: Borrow the canonical URL string slice.
    ///
    /// Inputs: None
    ///
    /// Output:
    /// - String slice of the URL.
    ///
    /// Details:
    /// - Zero-copy borrow.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AurRepoUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// What: Installed package metadata representing a foreign or AUR package on the system.
///
/// Inputs:
/// - `installed_name`: Package name as registered in local Pacman database.
/// - `package_base`: Package base name declared in `.PKGINFO` / `.SRCINFO`.
/// - `version`: Installed version string (e.g. `1.2.3-1`).
///
/// Output:
/// - Struct containing installed package identity fields.
///
/// Details:
/// - Supports split packages where multiple installed names map to one package base.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstalledPackage {
    /// Local installed package name.
    pub installed_name: PackageName,
    /// Package base name shared by split packages.
    pub package_base: PackageBase,
    /// Installed package version.
    pub version: String,
}

/// What: Group of installed packages sharing a single package base.
///
/// Inputs:
/// - `package_base`: Shared package base.
/// - `installed_names`: Deduplicated list of installed package names under this base.
/// - `primary_version`: Primary installed version associated with the package base.
///
/// Output:
/// - Grouped split package identity structure.
///
/// Details:
/// - Preserves all affected installed package names while grouping operations by package base.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SplitPackageGroup {
    /// Package base shared by all member packages.
    pub package_base: PackageBase,
    /// Sorted, deduplicated installed package names belonging to this package base.
    pub installed_names: Vec<PackageName>,
    /// Primary installed version associated with the package base.
    pub primary_version: String,
}

/// What: Deduplicate installed packages by package base while retaining all installed package names.
///
/// Inputs:
/// - `packages`: Slice of `InstalledPackage` instances.
///
/// Output:
/// - Vector of `SplitPackageGroup` instances grouped by package base and sorted by package base name.
///
/// Details:
/// - Ensures every installed package name is preserved under its owning package base.
/// - Handles split packages where multiple installed names share the same package base.
#[must_use]
pub fn deduplicate_split_packages(packages: &[InstalledPackage]) -> Vec<SplitPackageGroup> {
    use std::collections::BTreeMap;

    let mut map: BTreeMap<PackageBase, (Vec<PackageName>, String)> = BTreeMap::new();

    for pkg in packages {
        let entry = map
            .entry(pkg.package_base.clone())
            .or_insert_with(|| (Vec::new(), pkg.version.clone()));
        if !entry.0.contains(&pkg.installed_name) {
            entry.0.push(pkg.installed_name.clone());
        }
    }

    map.into_iter()
        .map(|(package_base, (mut installed_names, primary_version))| {
            installed_names.sort();
            SplitPackageGroup {
                package_base,
                installed_names,
                primary_version,
            }
        })
        .collect()
}

/// What: Frozen scan target identity for an explicit scan operation.
///
/// Inputs:
/// - Target package base, installed names, version, candidate version, observed commit OID, and cycle ID.
///
/// Output:
/// - Struct capturing exact scan target identity.
///
/// Details:
/// - Prevents reconstructing identity from mutable text files during or after a scan cycle.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ScanTargetIdentity {
    /// Package base being scanned.
    pub package_base: PackageBase,
    /// Installed package names associated with this package base.
    pub installed_names: Vec<PackageName>,
    /// Installed version at time of target creation.
    pub installed_version: String,
    /// Candidate version (if update scan), or `None`.
    pub candidate_version: Option<String>,
    /// Immutable Git commit OID observed for this scan.
    pub commit_oid: CommitOid,
    /// Scan cycle identifier.
    pub cycle_id: String,
}
