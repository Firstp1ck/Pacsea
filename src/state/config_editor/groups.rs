//! Stable settings groups shared by sorting and rendering.
use crate::theme::{ConfigFile, EditableSetting};

/// Display groups ordered within each file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SettingGroup {
    /// General entries.
    General,
    /// Layout entries.
    Layout,
    /// Search entries.
    Search,
    /// Privileges entries.
    Privileges,
    /// Mirrors entries.
    Mirrors,
    /// Updates entries.
    Updates,
    /// Scans entries.
    Scans,
    /// Pkgbuild entries.
    Pkgbuild,
    /// News entries.
    News,
    /// `StartupNews` entries.
    StartupNews,
    /// `AurVoting` entries.
    AurVoting,
    /// `PiScan` entries.
    PiScan,
    /// `KeybindGlobal` entries.
    KeybindGlobal,
    /// `KeybindSearch` entries.
    KeybindSearch,
    /// `KeybindSearchNormal` entries.
    KeybindSearchNormal,
    /// `KeybindRecent` entries.
    KeybindRecent,
    /// `KeybindInstall` entries.
    KeybindInstall,
    /// `KeybindNews` entries.
    KeybindNews,
    /// Backgrounds entries.
    Backgrounds,
    /// `SurfacesOverlays` entries.
    SurfacesOverlays,
    /// Text entries.
    Text,
    /// Accents entries.
    Accents,
    /// Semantic entries.
    Semantic,
}
impl SettingGroup {
    /// Translation key for this group heading.
    #[must_use]
    pub const fn i18n_key(self) -> &'static str {
        match self {
            Self::General => "app.modals.config_editor.groups.general",
            Self::Layout => "app.modals.config_editor.groups.layout",
            Self::Search => "app.modals.config_editor.groups.search",
            Self::Privileges => "app.modals.config_editor.groups.privileges",
            Self::Mirrors => "app.modals.config_editor.groups.mirrors",
            Self::Updates => "app.modals.config_editor.groups.updates",
            Self::Scans => "app.modals.config_editor.groups.scans",
            Self::Pkgbuild => "app.modals.config_editor.groups.pkgbuild",
            Self::News => "app.modals.config_editor.groups.news",
            Self::StartupNews => "app.modals.config_editor.groups.startup_news",
            Self::AurVoting => "app.modals.config_editor.groups.aur_voting",
            Self::PiScan => "app.modals.config_editor.groups.pi_scan",
            Self::KeybindGlobal => "app.modals.config_editor.groups.keybind_global",
            Self::KeybindSearch => "app.modals.config_editor.groups.keybind_search",
            Self::KeybindSearchNormal => "app.modals.config_editor.groups.keybind_search_normal",
            Self::KeybindRecent => "app.modals.config_editor.groups.keybind_recent",
            Self::KeybindInstall => "app.modals.config_editor.groups.keybind_install",
            Self::KeybindNews => "app.modals.config_editor.groups.keybind_news",
            Self::Backgrounds => "app.modals.config_editor.groups.backgrounds",
            Self::SurfacesOverlays => "app.modals.config_editor.groups.surfaces_overlays",
            Self::Text => "app.modals.config_editor.groups.text",
            Self::Accents => "app.modals.config_editor.groups.accents",
            Self::Semantic => "app.modals.config_editor.groups.semantic",
        }
    }
}

/// What: Assign a stable display group to a schema entry.
///
/// Inputs: Schema entry.
///
/// Output: Group ordered within the entry's file.
///
/// Details: Keybind grouping reuses the conflict-checking scopes.
#[must_use]
pub fn setting_group(entry: &EditableSetting) -> SettingGroup {
    match entry.file {
        ConfigFile::Keybinds => match crate::theme::keybind_scope(entry.key) {
            "search" => SettingGroup::KeybindSearch,
            "search_normal" => SettingGroup::KeybindSearchNormal,
            "recent" => SettingGroup::KeybindRecent,
            "install" => SettingGroup::KeybindInstall,
            "news" => SettingGroup::KeybindNews,
            _ => SettingGroup::KeybindGlobal,
        },
        ConfigFile::Theme => match entry.key.split('_').next().unwrap_or("") {
            "background" => SettingGroup::Backgrounds,
            "surface" | "overlay" => SettingGroup::SurfacesOverlays,
            "text" => SettingGroup::Text,
            "accent" => SettingGroup::Accents,
            _ => SettingGroup::Semantic,
        },
        ConfigFile::Settings | ConfigFile::Repos => settings_group(entry.key),
    }
}

/// Classify settings using their canonical names, preserving schema order within groups.
fn settings_group(key: &str) -> SettingGroup {
    match key {
        key if key.starts_with("pi_scan_") => SettingGroup::PiScan,
        key if key.starts_with("startup_news_") => SettingGroup::StartupNews,
        key if key.starts_with("aur_vote_") => SettingGroup::AurVoting,
        key if key.starts_with("pkgbuild_") => SettingGroup::Pkgbuild,
        key if key.starts_with("news_") => SettingGroup::News,
        key if key.starts_with("scan_") || key == "virustotal_api_key" => SettingGroup::Scans,
        key if key.starts_with("updates_") => SettingGroup::Updates,
        "mirror_count" | "selected_countries" => SettingGroup::Mirrors,
        "privilege_mode" | "skip_preflight" | "use_passwordless_sudo" | "auth_mode" => {
            SettingGroup::Privileges
        }
        "sort_mode" | "search_startup_mode" | "fuzzy_search" | "installed_packages_mode" => {
            SettingGroup::Search
        }
        key if key.starts_with("layout_")
            || key.starts_with("vertical_")
            || key.starts_with("show_")
            || matches!(key, "main_pane_order" | "package_marker") =>
        {
            SettingGroup::Layout
        }
        _ => SettingGroup::General,
    }
}

/// Sort files in tab order independently of the schema enum declaration.
pub(super) const fn file_order(file: ConfigFile) -> u8 {
    match file {
        ConfigFile::Settings => 0,
        ConfigFile::Keybinds => 1,
        ConfigFile::Theme => 2,
        ConfigFile::Repos => 3,
    }
}
