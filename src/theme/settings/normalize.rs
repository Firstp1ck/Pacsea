use crate::theme::types::Settings;

/// What: Normalize settings values parsed from configuration files.
///
/// Inputs:
/// - `settings`: Mutable reference to `Settings` to normalize in-place.
///
/// Output:
/// - None (modifies `settings` in-place).
///
/// Details:
/// - Ensures `mirror_count` is between 1 and 200 (defaults to 20 if 0).
/// - Normalizes `selected_countries` by trimming and formatting comma-separated values.
/// - Trims whitespace from `VirusTotal` API key.
pub fn normalize(settings: &mut Settings) {
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
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
    }
    // Normalize VirusTotal API key (trim whitespace)
    settings.virustotal_api_key = settings.virustotal_api_key.trim().to_string();
}
