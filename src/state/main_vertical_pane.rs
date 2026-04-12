//! Vertical main-stack roles: results list, middle search row, and package info.

/// What: Identifies one of the three vertical regions in the main TUI stack.
///
/// Inputs: None (enum definition).
///
/// Output: None (enum definition).
///
/// Details:
/// - Distinct from horizontal [`crate::state::Focus`]; this only describes top-to-bottom placement.
/// - The active permutation is stored in [`crate::theme::Settings::main_pane_order`] and copied to
///   [`crate::state::AppState::main_pane_order`] at startup and on config reload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MainVerticalPane {
    /// Package search results list (title row + list).
    Results,
    /// Middle row: recent queries, search input, install list.
    Middle,
    /// Package details / news reader body.
    PackageInfo,
}

/// What: Default top-to-bottom order (results, then middle search row, then package info).
///
/// Inputs: None (constant).
///
/// Output: Constant array value.
///
/// Details:
/// - Matches the historical layout before `main_pane_order` existed.
pub const DEFAULT_MAIN_PANE_ORDER: [MainVerticalPane; 3] = [
    MainVerticalPane::Results,
    MainVerticalPane::Middle,
    MainVerticalPane::PackageInfo,
];

impl MainVerticalPane {
    /// What: Serialize this role to a stable settings token (lowercase).
    ///
    /// Inputs:
    /// - `self`: Pane role.
    ///
    /// Output:
    /// - Canonical token string.
    ///
    /// Details:
    /// - Used when writing defaults into `settings.conf`.
    #[must_use]
    pub const fn as_config_token(self) -> &'static str {
        match self {
            Self::Results => "results",
            Self::Middle => "search",
            Self::PackageInfo => "package_info",
        }
    }

    /// What: Parse a single role token from config (case-insensitive).
    ///
    /// Inputs:
    /// - `token`: One comma-separated fragment from `main_pane_order`.
    ///
    /// Output:
    /// - `Some(role)` when recognized, else `None`.
    ///
    /// Details:
    /// - Accepts aliases: `middle` for search row, `details` for package info.
    #[must_use]
    pub fn from_config_token(token: &str) -> Option<Self> {
        let t = token.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        match t.as_str() {
            "results" => Some(Self::Results),
            "search" | "middle" => Some(Self::Middle),
            "package_info" | "details" | "packageinfo" => Some(Self::PackageInfo),
            _ => None,
        }
    }
}

/// What: Parse `main_pane_order` value into a length-3 permutation of distinct roles.
///
/// Inputs:
/// - `value`: Comma-separated tokens (whitespace allowed).
///
/// Output:
/// - `Some(order)` when exactly three distinct known roles are present; otherwise `None`.
///
/// Details:
/// - Empty or duplicate roles yield `None`.
#[must_use]
pub fn parse_main_pane_order(value: &str) -> Option<[MainVerticalPane; 3]> {
    let parts: Vec<&str> = value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() != 3 {
        return None;
    }
    let mut seen = [false; 3];
    let mut out = [MainVerticalPane::Results; 3];
    for (i, part) in parts.iter().enumerate() {
        let role = MainVerticalPane::from_config_token(part)?;
        let idx = match role {
            MainVerticalPane::Results => 0usize,
            MainVerticalPane::Middle => 1usize,
            MainVerticalPane::PackageInfo => 2usize,
        };
        if seen[idx] {
            return None;
        }
        seen[idx] = true;
        out[i] = role;
    }
    Some(out)
}

/// What: Format a pane order for `settings.conf` (canonical tokens).
///
/// Inputs:
/// - `order`: Three distinct roles.
///
/// Output:
/// - Comma+space separated string.
///
/// Details:
/// - Intended for `ensure_settings_keys_present` and tests.
#[must_use]
pub fn format_main_pane_order(order: &[MainVerticalPane; 3]) -> String {
    format!(
        "{}, {}, {}",
        order[0].as_config_token(),
        order[1].as_config_token(),
        order[2].as_config_token()
    )
}

/// What: Min/max row heights for vertical layout allocation (semantic per pane).
///
/// Inputs: None (struct definition).
///
/// Output: None (struct definition).
///
/// Details:
/// - Values are applied after parsing and normalization from `settings.conf`.
/// - Package info has no user `max`; the allocator assigns remaining rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerticalLayoutLimits {
    /// Minimum height (terminal rows) for the results list region.
    pub min_results: u16,
    /// Maximum height for the results list region.
    pub max_results: u16,
    /// Minimum height for the middle (search) row.
    pub min_middle: u16,
    /// Maximum height for the middle row.
    pub max_middle: u16,
    /// Minimum height for package info when that band is visible.
    pub min_package_info: u16,
}

impl Default for VerticalLayoutLimits {
    /// What: Match historical hardcoded [`crate::ui`] constraints.
    fn default() -> Self {
        Self {
            min_results: 3,
            max_results: 17,
            min_middle: 3,
            max_middle: 5,
            min_package_info: 3,
        }
    }
}

impl VerticalLayoutLimits {
    /// What: Build limits from normalized numeric settings (avoids `state` ↔ `theme` cycles).
    ///
    /// Inputs:
    /// - `min_results`, `max_results`, `min_middle`, `max_middle`, `min_package_info`: Parsed settings.
    ///
    /// Output:
    /// - Populated limits struct.
    #[must_use]
    pub const fn from_u16s(
        min_results: u16,
        max_results: u16,
        min_middle: u16,
        max_middle: u16,
        min_package_info: u16,
    ) -> Self {
        Self {
            min_results,
            max_results,
            min_middle,
            max_middle,
            min_package_info,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_order_default_tokens() {
        let o = parse_main_pane_order("results, search, package_info").expect("parse");
        assert_eq!(o, DEFAULT_MAIN_PANE_ORDER);
    }

    #[test]
    fn parse_order_aliases_and_permutation() {
        let o = parse_main_pane_order("package_info,middle,results").expect("parse");
        assert_eq!(
            o,
            [
                MainVerticalPane::PackageInfo,
                MainVerticalPane::Middle,
                MainVerticalPane::Results
            ]
        );
    }

    #[test]
    fn parse_order_rejects_duplicates_and_bad_length() {
        assert!(parse_main_pane_order("results,results,middle").is_none());
        assert!(parse_main_pane_order("results,middle").is_none());
        assert!(parse_main_pane_order("a,b,c").is_none());
    }
}
