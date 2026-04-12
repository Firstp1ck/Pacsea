use crate::theme::types::Settings;

/// What: Clamp vertical layout limits so allocation stays well-defined.
///
/// Inputs:
/// - `settings`: Settings whose vertical min/max fields may have been user-edited.
///
/// Output:
/// - None (mutates `settings` in place).
///
/// Details:
/// - Mins are at least 1 and at most 500; max results/middle are capped and forced `>=` corresponding min.
fn normalize_vertical_layout(settings: &mut Settings) {
    const CAP: u16 = 500;
    let clamp = |v: u16| v.clamp(1, CAP);
    settings.vertical_min_results = clamp(settings.vertical_min_results);
    settings.vertical_min_middle = clamp(settings.vertical_min_middle);
    settings.vertical_min_package_info = clamp(settings.vertical_min_package_info);
    settings.vertical_max_results = clamp(settings.vertical_max_results);
    settings.vertical_max_middle = clamp(settings.vertical_max_middle);
    if settings.vertical_max_results < settings.vertical_min_results {
        settings.vertical_max_results = settings.vertical_min_results;
    }
    if settings.vertical_max_middle < settings.vertical_min_middle {
        settings.vertical_max_middle = settings.vertical_min_middle;
    }
}

/// What: Normalize settings values parsed from configuration files.
///
/// Inputs:
/// - `settings`: Mutable reference to `Settings` to normalize in-place.
///
/// Output:
/// - None (modifies `settings` in-place).
///
/// Details:
/// - Clamps vertical layout min/max fields first.
/// - Ensures `mirror_count` is between 1 and 200 (defaults to 20 if 0).
/// - Normalizes `selected_countries` by trimming and formatting comma-separated values.
/// - Normalizes `pkgbuild_shellcheck_exclude` the same way (comma-separated `ShellCheck` rule IDs).
/// - Trims whitespace from `VirusTotal` API key.
pub fn normalize(settings: &mut Settings) {
    normalize_vertical_layout(settings);
    // Normalize mirror settings parsed from settings.conf
    if settings.mirror_count == 0 {
        settings.mirror_count = 20;
    }
    if settings.mirror_count > 200 {
        settings.mirror_count = 200;
    }
    if !settings.selected_countries.is_empty() {
        settings.selected_countries = settings
            .selected_countries
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
    }
    if !settings.pkgbuild_shellcheck_exclude.is_empty() {
        settings.pkgbuild_shellcheck_exclude = settings
            .pkgbuild_shellcheck_exclude
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
    }
    // Normalize VirusTotal API key (trim whitespace)
    settings.virustotal_api_key = settings.virustotal_api_key.trim().to_string();
}

#[cfg(test)]
mod tests {
    use super::normalize;
    use crate::theme::types::Settings;

    #[test]
    fn normalize_vertical_clamps_middle_max_below_min() {
        let mut s = Settings {
            vertical_min_middle: 8,
            vertical_max_middle: 3,
            ..Settings::default()
        };
        normalize(&mut s);
        assert_eq!(s.vertical_max_middle, 8);
    }

    #[test]
    fn normalize_vertical_clamps_zero_mins_to_one() {
        let mut s = Settings {
            vertical_min_results: 0,
            ..Settings::default()
        };
        normalize(&mut s);
        assert_eq!(s.vertical_min_results, 1);
    }
}
