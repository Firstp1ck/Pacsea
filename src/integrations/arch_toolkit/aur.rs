//! AUR operations and model conversion through arch-toolkit.

use crate::state::types::AurComment;
use crate::state::{PackageDetails, PackageItem, Source};

use super::ToolkitContext;

/// What: Search AUR through arch-toolkit while preserving Pacsea result policy.
///
/// Inputs:
/// - `context`: Shared configured toolkit clients.
/// - `query`: Raw user query.
///
/// Output:
/// - Up to 200 Pacsea package rows or an actionable error message.
///
/// Details:
/// - Empty queries are rejected before client access, empty package names are filtered, and
///   toolkit models do not cross this boundary.
pub async fn search(context: &ToolkitContext, query: &str) -> Result<Vec<PackageItem>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("AUR search query is empty; enter a package name and retry".to_string());
    }
    context
        .aur_client()
        .aur()
        .search(query)
        .await
        .map(|packages| {
            packages
                .into_iter()
                .take(200)
                .filter_map(package_item)
                .collect()
        })
        .map_err(|error| format!("AUR search unavailable: {error}; check the network and retry"))
}

/// What: Fetch one AUR package detail record through arch-toolkit.
///
/// Inputs:
/// - `context`: Shared configured toolkit clients.
/// - `item`: Existing Pacsea row used for fallback fields.
///
/// Output:
/// - Pacsea package details or an actionable error message.
///
/// Details:
/// - Missing toolkit fields retain the row's version and description.
pub async fn details(
    context: &ToolkitContext,
    item: PackageItem,
) -> Result<PackageDetails, String> {
    let records = context
        .aur_client()
        .aur()
        .info(&[item.name.as_str()])
        .await
        .map_err(|error| {
            format!(
                "AUR package details unavailable for {}: {error}; check the network and retry",
                item.name
            )
        })?;
    let Some(details) = records.into_iter().next() else {
        return Err(format!(
            "AUR returned no details for {}; verify that the package still exists",
            item.name
        ));
    };
    Ok(package_details(details, &item))
}

/// What: Fetch AUR comments through arch-toolkit.
///
/// Inputs:
/// - `context`: Shared configured toolkit clients.
/// - `package_name`: Valid AUR package name.
///
/// Output:
/// - Pacsea comment values or an actionable error message.
///
/// Details:
/// - arch-toolkit enforces a streamed 10 MiB response ceiling before parsing.
pub async fn comments(
    context: &ToolkitContext,
    package_name: &str,
) -> Result<Vec<AurComment>, String> {
    context
        .aur_client()
        .aur()
        .comments(package_name)
        .await
        .map(|comments| comments.into_iter().map(comment).collect())
        .map_err(|error| {
            format!(
                "failed to fetch comments for {package_name}: {error}; check the network and retry"
            )
        })
}

/// What: Fetch one AUR PKGBUILD through arch-toolkit.
///
/// Inputs:
/// - `context`: Shared configured toolkit clients.
/// - `package_name`: Valid AUR package name.
///
/// Output:
/// - Bounded PKGBUILD text or an actionable error message.
///
/// Details:
/// - The text is returned for display and static analysis only and is never executed here.
pub async fn pkgbuild(context: &ToolkitContext, package_name: &str) -> Result<String, String> {
    context
        .aur_client()
        .aur()
        .pkgbuild(package_name)
        .await
        .map_err(|error| {
            format!(
                "failed to fetch PKGBUILD for {package_name}: {error}; check the network and retry"
            )
        })
}

/// What: Convert one toolkit AUR search row into Pacsea state.
///
/// Inputs:
/// - `package`: Toolkit package row.
///
/// Output:
/// - `Some(PackageItem)` for a named package, otherwise `None`.
///
/// Details:
/// - Preserves popularity, out-of-date, and orphan status used by sorting and badges.
fn package_item(package: arch_toolkit::AurPackage) -> Option<PackageItem> {
    if package.name.is_empty() {
        return None;
    }
    Some(PackageItem {
        name: package.name,
        version: package.version,
        description: package.description,
        source: Source::Aur,
        popularity: package.popularity,
        out_of_date: package.out_of_date,
        orphaned: package.orphaned,
    })
}

/// What: Convert toolkit AUR details into Pacsea state.
///
/// Inputs:
/// - `details`: Toolkit detail record.
/// - `fallback`: Existing Pacsea search row.
///
/// Output:
/// - Pacsea package details preserving existing defaults.
///
/// Details:
/// - AUR has no download/install sizes or reverse dependency fields in this API.
fn package_details(
    details: arch_toolkit::AurPackageDetails,
    fallback: &PackageItem,
) -> PackageDetails {
    PackageDetails {
        repository: "AUR".to_string(),
        name: if details.name.is_empty() {
            fallback.name.clone()
        } else {
            details.name
        },
        version: if details.version.is_empty() {
            fallback.version.clone()
        } else {
            details.version
        },
        description: if details.description.is_empty() {
            fallback.description.clone()
        } else {
            details.description
        },
        architecture: "any".to_string(),
        url: details.url,
        licenses: details.licenses,
        groups: details.groups,
        provides: details.provides,
        depends: details.depends,
        opt_depends: details.opt_depends,
        required_by: Vec::new(),
        optional_for: Vec::new(),
        conflicts: details.conflicts,
        replaces: details.replaces,
        download_size: None,
        install_size: None,
        owner: details.maintainer.unwrap_or_default(),
        build_date: crate::util::ts_to_date(details.last_modified),
        popularity: details.popularity,
        out_of_date: details.out_of_date,
        orphaned: details.orphaned,
    }
}

/// What: Convert a toolkit AUR comment into Pacsea state.
///
/// Inputs:
/// - `comment`: Toolkit comment value.
///
/// Output:
/// - Equivalent Pacsea comment value.
///
/// Details:
/// - Stable IDs, timestamps, links, formatting, and pinned ordering are preserved.
fn comment(comment: arch_toolkit::AurComment) -> AurComment {
    AurComment {
        id: comment.id,
        author: comment.author,
        date: comment.date,
        date_timestamp: comment.date_timestamp,
        date_url: comment.date_url,
        content: comment.content,
        pinned: comment.pinned,
    }
}

#[cfg(test)]
mod tests {
    use crate::state::{PackageItem, Source};

    /// What: Verify toolkit AUR search rows preserve Pacsea status fields.
    ///
    /// Inputs:
    /// - One fully populated toolkit package row.
    ///
    /// Output:
    /// - Equivalent Pacsea AUR row.
    ///
    /// Details:
    /// - Protects the UI sorting and status badge contract during conversion.
    #[test]
    fn converts_aur_search_row() {
        let package = arch_toolkit::AurPackage {
            name: "paru".to_string(),
            version: "2.0".to_string(),
            description: "helper".to_string(),
            popularity: Some(3.5),
            out_of_date: Some(42),
            orphaned: true,
            maintainer: None,
        };
        let item = super::package_item(package).expect("named row should convert");
        assert_eq!(item.name, "paru");
        assert!(matches!(item.source, Source::Aur));
        assert_eq!(item.popularity, Some(3.5));
        assert_eq!(item.out_of_date, Some(42));
        assert!(item.orphaned);
    }

    /// What: Verify empty toolkit detail fields retain Pacsea search-row fallbacks.
    ///
    /// Inputs:
    /// - Empty toolkit detail record and populated fallback row.
    ///
    /// Output:
    /// - Pacsea details with fallback name, version, and description.
    ///
    /// Details:
    /// - Matches the pre-migration missing-field behavior.
    #[test]
    fn aur_details_keep_search_fallbacks() {
        let fallback = PackageItem {
            name: "paru".to_string(),
            version: "2.0".to_string(),
            description: "helper".to_string(),
            source: Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        };
        let converted =
            super::package_details(arch_toolkit::AurPackageDetails::default(), &fallback);
        assert_eq!(converted.name, "paru");
        assert_eq!(converted.version, "2.0");
        assert_eq!(converted.description, "helper");
        assert_eq!(converted.repository, "AUR");
    }
}
