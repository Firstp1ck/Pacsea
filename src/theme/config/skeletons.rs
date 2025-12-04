/// Skeleton configuration file content with default color values.
pub const THEME_SKELETON_CONTENT: &str = "# Pacsea theme configuration\n\
#\n\
# Format: key = value\n\
# Value formats supported:\n\
#   - #RRGGBB (hex)\n\
#   - R,G,B (decimal, 0-255 each)\n\
#   Example (decimal): text_primary = 205,214,244\n\
# Lines starting with # are comments.\n\
#\n\
# Key naming:\n\
#   Comprehensive names are preferred (shown first). Legacy keys remain supported\n\
#   for compatibility (e.g., \"base\", \"surface1\").\n\
#\n\
#-----------------------------------------------------------------------------------------------------------------------\n\
#\n\
# ---------- Catppuccin Mocha (dark) ----------\n\
#\n\
# Background layers (from darkest to lightest)\n\
background_base = #1e1e2e\n\
background_mantle = #181825\n\
background_crust = #11111b\n\
#\n\
# Component surfaces\n\
surface_level1 = #45475a\n\
surface_level2 = #585b70\n\
#\n\
# Low-contrast lines/borders\n\
overlay_primary = #7f849c\n\
overlay_secondary = #9399b2\n\
#\n\
# Text hierarchy\n\
text_primary = #cdd6f4\n\
text_secondary = #a6adc8\n\
text_tertiary = #bac2de\n\
#\n\
# Accents and semantic colors\n\
accent_interactive = #74c7ec\n\
accent_heading = #cba6f7\n\
accent_emphasis = #b4befe\n\
semantic_success = #a6e3a1\n\
semantic_warning = #f9e2af\n\
semantic_error = #f38ba8\n\
#\n\
# ---------- Alternative Theme (Light) ----------\n\
#\n\
# # Background layers (from lightest to darkest)\n\
# background_base = #f5f5f7\n\
# background_mantle = #eaeaee\n\
# background_crust = #dcdce1\n\
#\n\
# # Component surfaces\n\
# surface_level1 = #cfd1d7\n\
# surface_level2 = #b7bac3\n\
#\n\
# # Low-contrast lines/borders and secondary text accents\n\
# overlay_primary = #7a7d86\n\
# overlay_secondary = #63666f\n\
#\n\
# # Text hierarchy\n\
# text_primary = #1c1c22\n\
# text_secondary = #3c3f47\n\
# text_tertiary = #565a64\n\
#\n\
# # Accents and semantic colors\n\
# accent_interactive = #1e66f5\n\
# accent_heading = #8839ef\n\
# accent_emphasis = #7287fd\n\
# semantic_success = #40a02b\n\
# semantic_warning = #df8e1d\n\
# semantic_error = #d20f39\n\
\n\
# ---------- Alternative Theme (Tokyo Night — Night) ----------\n\
#\n\
# # Background layers (from darkest to lightest)\n\
# background_base = #1a1b26\n\
# background_mantle = #16161e\n\
# background_crust = #0f0f14\n\
#\n\
# # Component surfaces\n\
# surface_level1 = #24283b\n\
# surface_level2 = #1f2335\n\
#\n\
# # Low-contrast lines/borders\n\
# overlay_primary = #414868\n\
# overlay_secondary = #565f89\n\
#\n\
# # Text hierarchy\n\
# text_primary = #c0caf5\n\
# text_secondary = #a9b1d6\n\
# text_tertiary = #9aa5ce\n\
#\n\
# # Accents and semantic colors\n\
# accent_interactive = #7aa2f7\n\
# accent_heading = #bb9af7\n\
# accent_emphasis = #7dcfff\n\
# semantic_success = #9ece6a\n\
# semantic_warning = #e0af68\n\
# semantic_error = #f7768e\n\
\n\
# ---------- Alternative Theme (Nord) ----------\n\
#\n\
# # Background layers (from darkest to lightest)\n\
# background_base = #2e3440\n\
# background_mantle = #3b4252\n\
# background_crust = #434c5e\n\
#\n\
# # Component surfaces\n\
# surface_level1 = #3b4252\n\
# surface_level2 = #4c566a\n\
#\n\
# # Low-contrast lines/borders\n\
# overlay_primary = #4c566a\n\
# overlay_secondary = #616e88\n\
#\n\
# # Text hierarchy\n\
# text_primary = #e5e9f0\n\
# text_secondary = #d8dee9\n\
# text_tertiary = #eceff4\n\
#\n\
# # Accents and semantic colors\n\
# accent_interactive = #81a1c1\n\
# accent_heading = #b48ead\n\
# accent_emphasis = #88c0d0\n\
# semantic_success = #a3be8c\n\
# semantic_warning = #ebcb8b\n\
# semantic_error = #bf616a\n\
\n\
# ---------- Alternative Theme (Dracula) ----------\n\
#\n\
# # Background layers (from darkest to lightest)\n\
# background_base = #282a36\n\
# background_mantle = #21222c\n\
# background_crust = #44475a\n\
#\n\
# # Component surfaces\n\
# surface_level1 = #44475a\n\
# surface_level2 = #343746\n\
#\n\
# # Low-contrast lines/borders\n\
# overlay_primary = #44475a\n\
# overlay_secondary = #6272a4\n\
#\n\
# # Text hierarchy\n\
# text_primary = #f8f8f2\n\
# text_secondary = #e2e2e6\n\
# text_tertiary = #d6d6de\n\
#\n\
# # Accents and semantic colors\n\
# accent_interactive = #8be9fd\n\
# accent_heading = #bd93f9\n\
# accent_emphasis = #ff79c6\n\
# semantic_success = #50fa7b\n\
# semantic_warning = #f1fa8c\n\
# semantic_error = #ff5555\n\
#\n\
# ---------- Alternative Theme (Gruvbox Dark) ----------\n\
#\n\
# # Background layers (from darkest to lightest)\n\
# background_base = #171717\n\
# background_mantle = #1d2021\n\
# background_crust = #0d1011\n\
#\n\
# # Component surfaces\n\
# surface_level1 = #3c3836\n\
# surface_level2 = #504945\n\
#\n\
# # Low-contrast lines/borders\n\
# overlay_primary = #665c54\n\
# overlay_secondary = #7c6f64\n\
#\n\
# # Text hierarchy\n\
# text_primary = #ebdbb2\n\
# text_secondary = #d5c4a1\n\
# text_tertiary = #bdae93\n\
#\n\
# # Accents and semantic colors\n\
# accent_interactive = #83a598\n\
# accent_heading = #b16286\n\
# accent_emphasis = #d3869b\n\
# semantic_success = #b8bb26\n\
# semantic_warning = #fabd2f\n\
# semantic_error = #fb4934\n\
#\n\
#-----------------------------------------------------------------------------------------------------------------------\n";

/// Standalone settings skeleton used when initializing a separate settings.conf
pub const SETTINGS_SKELETON_CONTENT: &str = "# Pacsea settings configuration\n\
# Layout percentages for the middle row panes (must sum to 100)\n\
layout_left_pct = 20\n\
layout_center_pct = 60\n\
layout_right_pct = 20\n\
# Default dry-run behavior when starting the app (overridden by --dry-run)\n\
app_dry_run_default = false\n\
# Middle row visibility (default true)\n\
show_recent_pane = true\n\
show_install_pane = true\n\
show_keybinds_footer = true\n\
# Search input mode on startup\n\
# Allowed values: insert_mode | normal_mode\n\
# Default is insert_mode\n\
search_startup_mode = insert_mode\n\
# Fuzzy search mode\n\
# When true, uses fuzzy matching (fzf-style) instead of substring search\n\
# Default is false (normal substring search)\n\
fuzzy_search = false\n\
\n\
# Installed packages filter mode\n\
# Controls which packages are shown when viewing installed packages\n\
# Allowed values: leaf | all\n\
# - leaf: Show only leaf packages (explicitly installed, nothing depends on them) - default\n\
# - all: Show all explicitly installed packages (including those other packages depend on)\n\
installed_packages_mode = leaf\n\
\n\
# Results sorting\n\
# Allowed values: alphabetical | aur_popularity | best_matches\n\
sort_mode = best_matches\n\
\n\
# Clipboard\n\
# Text appended when copying PKGBUILD to the clipboard\n\
clipboard_suffix = Check PKGBUILD and source for suspicious and malicious activities\n\
\n\
# Preflight modal / safety confirmation\n\
# When true, Pacsea will bypass the Preflight confirmation modal and execute install/remove/downgrade actions immediately.\n\
# Recommended to keep this false for safety unless you understand the risks of executing package operations directly.\n\
skip_preflight = false\n\
\n\
# Mirrors\n\
# Select one or more countries (comma-separated). Example: \"Switzerland, Germany, Austria\"\n\
selected_countries = Worldwide\n\
# Number of HTTPS mirrors to consider when updating\n\
mirror_count = 20\n\
# Available countries (commented list; edit selected_countries above as needed):\n\
# Worldwide\n\
# Albania\n\
# Algeria\n\
# Argentina\n\
# Armenia\n\
# Australia\n\
# Austria\n\
# Azerbaijan\n\
# Belarus\n\
# Belgium\n\
# Bosnia and Herzegovina\n\
# Brazil\n\
# Bulgaria\n\
# Cambodia\n\
# Canada\n\
# Chile\n\
# China\n\
# Colombia\n\
# Costa Rica\n\
# Croatia\n\
# Cyprus\n\
# Czechia\n\
# Denmark\n\
# Ecuador\n\
# Estonia\n\
# Finland\n\
# France\n\
# Georgia\n\
# Germany\n\
# Greece\n\
# Hong Kong\n\
# Hungary\n\
# Iceland\n\
# India\n\
# Indonesia\n\
# Iran\n\
# Ireland\n\
# Israel\n\
# Italy\n\
# Japan\n\
# Kazakhstan\n\
# Latvia\n\
# Lithuania\n\
# Luxembourg\n\
# Malaysia\n\
# Mexico\n\
# Moldova\n\
# Netherlands\n\
# New Caledonia\n\
# New Zealand\n\
# Norway\n\
# Peru\n\
# Philippines\n\
# Poland\n\
# Portugal\n\
# Romania\n\
# Russia\n\
# Serbia\n\
# Singapore\n\
# Slovakia\n\
# Slovenia\n\
# South Africa\n\
# South Korea\n\
# Spain\n\
# Sweden\n\
# Switzerland\n\
# Taiwan\n\
# Thailand\n\
# Turkey\n\
# Ukraine\n\
# United Kingdom\n\
# United States\n\
# Uruguay\n\
# Vietnam\n\
\n\
# Scans\n\
# Default scan configuration used when opening Scan Configuration\n\
scan_do_clamav = true\n\
scan_do_trivy = true\n\
scan_do_semgrep = true\n\
scan_do_shellcheck = true\n\
scan_do_virustotal = true\n\
scan_do_custom = true\n\
scan_do_sleuth = true\n\
\n\
# News\n\
# Symbols for read/unread indicators in the News popup\n\
news_read_symbol = ✓\n\
news_unread_symbol = ∘\n\
\n\
# VirusTotal\n\
# API key used for VirusTotal scans (optional)\n\
virustotal_api_key = \n\
\n\
# Terminal\n\
# Preferred terminal emulator binary (optional): e.g., alacritty, kitty, gnome-terminal\n\
preferred_terminal = \n\
\n\
# Package selection marker\n\
# Visual marker for packages added to Install/Remove/Downgrade lists.\n\
# Allowed values: full_line | front | end\n\
# - full_line: color the entire line\n\
# - front: add marker at the front of the line (default)\n\
# - end: add marker at the end of the line\n\
package_marker = front

# Language / Locale
# Locale code for translations (e.g., \"en-US\", \"de-DE\").
# Leave empty to auto-detect from system locale (LANG/LC_ALL environment variables).
# Available locales: en-US, de-DE, hu-HU (more coming soon)
locale = \n\
\n\
# Updates refresh interval\n\
# Time in seconds between pacman -Qu and AUR helper checks.\n\
# Default is 30 seconds. Increase this value on systems with slow I/O or many packages to reduce resource usage.\n\
# Minimum value is 1 second.\n\
updates_refresh_interval = 30\n\
\n\
# Remote announcements\n\
# URL for fetching remote announcements (GitHub Gist raw URL)\n\
# Default: true\n\
# If true, fetches remote announcements from GitHub Gist\n\
# If false, remote announcements are disabled (version announcements still show)\n\
get_announcement = true\n";

/// Standalone keybinds skeleton used when initializing a separate keybinds.conf
pub const KEYBINDS_SKELETON_CONTENT: &str = "# Pacsea keybindings configuration\n\
# Modifiers can be one of: SUPER, CTRL, SHIFT, ALT.\n\
\n\
# GLOBAL — App\n\
keybind_help = F1\n\
# Alternative help shortcut\n\
keybind_help = ?\n\
keybind_reload_config = CTRL+R\n\
keybind_exit = CTRL+Q\n\
keybind_show_pkgbuild = CTRL+X\n\
keybind_comments_toggle = CTRL+T\n\
\n\
# GLOBAL — Pane switching\n\
keybind_pane_left = Left\n\
keybind_pane_right = Right\n\
keybind_pane_next = Tab\n\
# GLOBAL — Sorting\n\
keybind_change_sort = BackTab\n\
\n\
# SEARCH — Navigation\n\
keybind_search_move_up = Up\n\
keybind_search_move_down = Down\n\
keybind_search_page_up = PgUp\n\
keybind_search_page_down = PgDn\n\
\n\
# SEARCH — Actions\n\
keybind_search_add = Space\n\
keybind_search_install = Enter\n\
\n\
# SEARCH — Focus/Edit\n\
keybind_search_focus_left = Left\n\
keybind_search_focus_right = Right\n\
keybind_search_backspace = Backspace\n\
keybind_search_insert_clear = Shift+Del\n\
\n\
# SEARCH — Normal Mode (Focused Search Window)\n\
keybind_search_normal_toggle = Esc\n\
keybind_search_normal_insert = i\n\
keybind_search_normal_select_left = h\n\
keybind_search_normal_select_right = l\n\
keybind_search_normal_delete = d\n\
keybind_search_normal_clear = Shift+Del\n\
\n\
# SEARCH — Normal Mode (Menus)\n\
# Toggle dropdown menus while in Normal Mode\n\
keybind_toggle_config = Shift+C\n\
keybind_toggle_options = Shift+O\n\
keybind_toggle_panels = Shift+P\n\
\n\
# SEARCH — Normal Mode (Other)\n\
# Open Arch status page in default browser\n\
keybind_search_normal_open_status = Shift+S\n\
# Import packages list into Install list\n\
keybind_search_normal_import = Shift+I\n\
# Export current Install list to a file\n\
keybind_search_normal_export = Shift+E\n\
# Open Available Updates window\n\
keybind_search_normal_updates = Shift+U\n\
\n\
# SEARCH — Fuzzy Search Toggle\n\
# Toggle between normal substring search and fuzzy search (fzf-style)\n\
keybind_toggle_fuzzy = CTRL+F\n\
\n\
# RECENT — Navigation\n\
keybind_recent_move_up = k\n\
keybind_recent_move_down = j\n\
\n\
# RECENT — Actions\n\
keybind_recent_use = Enter\n\
keybind_recent_add = Space\n\
keybind_recent_remove = d\n\
keybind_recent_remove = Del\n\
\n\
# RECENT — Find/Focus\n\
keybind_recent_find = /\n\
keybind_recent_to_search = Esc\n\
keybind_recent_focus_right = Right\n\
\n\
# INSTALL — Navigation\n\
keybind_install_move_up = k\n\
keybind_install_move_down = j\n\
\n\
# INSTALL — Actions\n\
keybind_install_confirm = Enter\n\
keybind_install_remove = Del\n\
keybind_install_remove = d\n\
keybind_install_clear = Shift+Del\n\
\n\
# INSTALL — Find/Focus\n\
keybind_install_find = /\n\
keybind_install_to_search = Esc\n\
keybind_install_focus_left = Left\n\
\n\
# NEWS — Actions\n\
keybind_news_mark_read = r\n\
keybind_news_mark_all_read = CTRL+R\n";
