# Changelog

All notable changes to Pacsea will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---
## [0.7.2] - 2026-01-18

# Release v0.7.2

## What's New

### 🔒 Security Updates

**Dependency Updates:**
- Updated multiple dependencies to address low-severity security vulnerabilities
- Updated core dependencies including `clap`, `ratatui`, `tokio`, `reqwest`, and more
- Improved overall security posture of the application

### 🐛 Bug Fixes

**Code Quality Improvements:**
- Fixed CodeQL security analysis issues (#2, #3, #4, #5)
- Enhanced input validation in import modals
- Improved error handling in mirror index operations
- Strengthened utility function safety checks

### 🌍 Localization

**Translation Updates:**
- Added Hungarian translations for notification titles
- Improved localization coverage for better international user experience

### 📚 Documentation

**Documentation Enhancements:**
- Updated README with improved news feed feature description
- Added news feed screenshot to documentation
- Updated SECURITY.md with supported version information
- Added comprehensive passwordless sudo implementation plan

## Technical Details

This release focuses on security improvements, bug fixes, and documentation updates. All changes are backward compatible.

## Full Changelog

**Full Changelog**: https://github.com/Firstp1ck/Pacsea/compare/v0.7.1...v0.7.2

---

## [0.7.1] - 2025-12-24

# Release v0.7.1

## What's New

### 🐛 Bug Fixes & Improvements

**News Mode Enhancements:**
- **Separated search inputs**: News mode and Package mode now have independent search fields
  - No more shared state issues when switching between modes
  - Search text is preserved when switching modes
- **Improved mark-as-read behavior**: Mark read actions (`r` key) now only work in normal mode
  - Prevents accidental marking when typing 'r' in insert mode
  - More consistent with vim-like behavior

**Toast Notifications:**
- Improved toast clearing logic for better user experience
- Enhanced toast title detection for news, clipboard, and notification types
- Added notification title translations

**UI Polish:**
- Sort menu no longer auto-closes (stays open until you select an option or close it)
- Added `change_sort` keybind to help footer in News mode
- Fixed help text punctuation for better readability

## Technical Details

This release focuses on bug fixes, and user experience refinements. All changes are backward compatible.

## Full Changelog

**Full Changelog**: https://github.com/Firstp1ck/Pacsea/compare/v0.7.0...v0.7.1


---

## [0.7.0] - 2025-12-24

# Release v0.7.0

## What's New

### 📰 Extended News Mode
Pacsea now includes a comprehensive **News Mode** with advanced features:

**News Sources:**
- **Arch Linux News**: Latest announcements and updates from archlinux.org
- **Security Advisories**: Security alerts with severity indicators and affected packages
- **Package Updates**: Track version changes for your installed packages with change detection
- **AUR Comments**: Recent community discussions and feedback

**Smart Features:**
- **Change Detection**: Automatically detects package changes (version, maintainer, dependencies)
- **Offline Support**: Caches package data to disk for offline access and faster loading
- **Background Processing**: Failed requests are automatically retried in the background
- **Streaming Updates**: After initial 50 items, additional news items load automatically
- **AUR Balance**: Ensures AUR packages are always represented alongside official packages

**User Experience:**
- Switch to News mode or set `app_start_mode = news` in settings to start in News mode
- Filter by type (Arch news, advisories, updates, comments)
- Sort by date, title, severity, or unread status (use `Shift+Tab` to cycle through sort modes)
- Mark items as read/unread with `r` key
- Bookmark important items for quick access
- Persistent read/unread tracking across sessions

### ⚡ Performance & Reliability Improvements
- **Smart error handling**: Automatically handles repeated failures gracefully without blocking your workflow
- **Rate limiting**: Prevents server blocking with intelligent request management
- **Smart caching**: Multi-layer caching system reduces bandwidth and speeds up loading
  - Fast in-memory cache for instant access
  - Persistent disk cache for offline access
- **Efficient updates**: Only downloads changed data to minimize bandwidth usage
- **Background retries**: Failed requests are automatically retried in the background
- **Better compatibility**: Improved connection handling for better reliability

### 🔧 Code Quality Improvements
- **Better organization**: Code has been reorganized for improved maintainability
- **Enhanced documentation**: Improved code documentation throughout
- **Security scanning**: Added automated security checks
- **Better logging**: Improved visibility of important operational messages

### 🎨 UI Improvements
- **Enhanced footer**: Multi-line keybinds display for better readability
- **Loading indicators**: Visual feedback during data fetching with informative messages
- **Improved filters**: Better filter chips with clickable areas
- **Extended keybinds**: Shift+char keybind support across all panes and modes
- **Better alignment**: Fixed text wrapping issues in updates window

### 🐛 Bug Fixes
- Fixed updates window text alignment when package names/versions wrap to multiple lines
- Fixed options menu key bindings to match display order in Package and News modes
- Fixed `installed_packages.txt` export to respect `installed_packages_mode` setting
- Fixed alert title showing "Connection issue" instead of "Configuration Directories" for config directory messages
- Fixed Shift+Tab keybind to work in News mode (previously only worked in Package mode)
- Fixed scroll position issues
- Improved AUR comment date filtering (excludes invalid dates)
- Enhanced date parsing to handle various date formats correctly
- Fixed package date fetching for better reliability
- Improved error detection and handling

### 🌍 Internationalization
- Improved config directory alert detection for all languages
- Added translations for config directory alerts in English, German, and Hungarian
- Improved loading messages with full translation support

## Technical Details

This release includes significant improvements to code organization, performance, and reliability.

## Configuration

New settings available:
- `app_start_mode`: Set to "news" to start in News mode (default: "package")
- `news_filter_*`: Toggle filters for Arch news, advisories, updates, AUR updates/comments
- `news_max_age_days`: Maximum age filter for news items (default: unlimited)

## Installation

Update to v0.7.0:

```bash
# For stable release
paru -S pacsea-bin   # or: yay -S pacsea-bin

# For latest from git
paru -S pacsea-git   # or: yay -S pacsea-git
```

## Full Changelog

**Full Changelog**: https://github.com/Firstp1ck/Pacsea/compare/v0.6.2...v0.7.0

---

## [0.6.2] - 2025-12-05

### Added
- **Force Sync Option**: System Update modal now includes a Force Sync mode
- Toggle between `Normal (-Syu)` and `Force Sync (-Syyu)` on the pacman update row
- Use `←`/`→` or `Tab` keys to switch sync mode
- Force sync refreshes all package databases even if unchanged

### Fixed
- **Install list preserved**: System update no longer clears queued packages from the install list
- **Faster exit**: App now closes immediately when exiting during preflight loading
- **Auto-refresh**: Available updates count refreshes automatically after install/remove/downgrade operations

### Changed
- Updated Hungarian translations

### Contributors
- @Firstp1ck
- @summoner001 

---
## [0.6.1] - 2025-12-05

### Added
- Announcement modal system - view important updates and version notes
- Press `r` to mark announcements as read (won't show again)
- Press `Enter`, `Esc`, or `q` to dismiss temporarily
- Use arrow keys or `j`/`k` to scroll through long announcements

### Fixed
- Global keybinds interfering with modals - keyboard shortcuts now properly respect modal state

### Changed
- Updated Hungarian translations

### Contributors
- @summoner001
- @CooperTrooper21
- @Firstp1ck

---

## [0.6.0] - 2025-12-03

### Added
- **Integrated Process Execution**: All operations now execute directly within the TUI instead of spawning external terminals
- Live output streaming with real-time progress display
- Auto-scrolling log panel for command output
- Inline password prompts when sudo authentication is required
- Integrated password prompt modal for sudo operations
- Password validation and faillock lockout detection

### Supported Operations
- Package installation (official repositories and AUR packages)
- Package removal with cascade modes (Basic, Cascade, Cascade with Configs)
- System updates (mirror updates, database updates, AUR updates, cache cleanup)
- Security scans (ClamAV, Trivy, Semgrep, ShellCheck, VirusTotal, custom patterns)
- File database sync
- Optional dependency installation
- Preflight modal execution

### Technical
- Command output streams in real-time via PTY
- Progress bars display during package downloads and installations
- Windows compatibility maintained with conditional compilation

### Contributors
- @summoner001
- @Firstp1ck

---

## [0.5.3] - 2025-11-30

### Added
- **AUR Comments Viewer**: View community comments for AUR packages directly in Pacsea
- Press `CTRL+T` or click "Show comments" to toggle
- Features markdown support, clickable URLs, and auto-updates when navigating packages
- **AUR Package Status Markings**: AUR packages now show status indicators
  - `[OOD]` - Out-of-date packages
  - `[ORPHAN]` - Orphaned packages (no maintainer)
- **Collapsed Menu**: Automatically appears when the window is too narrow
- **Package Downgrade**: Fixed support for downgrading installed packages using the `downgrade` tool
- Enhanced **aur-sleuth** (LLM-based audit tool) for the security scanning workflow
- Runs in a separate terminal window with setup wizard in Optional Deps modal

### Fixed
- Fixed preflight tabs not resolving when opening packages directly from results
- Fixed Artix filter menu expanding in tight spaces

### Changed
- Faster tab switching with cached dependency reports for remove operations
- Better warnings for meta-packages and dependency conflicts
- Improved service impact detection
- Press Enter in Updates Modal to open Preflight for detailed review before installing updates
- Auto-refreshes after installation with real-time progress display
- Updated Hungarian translations

### Contributors
- @summoner001
- @Max-Guenther
- @Firstp1ck

---

## [0.5.2] - 2025-11-23

### Added
- **Fuzzy Search**: Find packages faster with fuzzy search
- Press `CTRL+F` to toggle fuzzy search mode
- Smarter and more flexible search for easier package discovery

### Changed
- Press `CTRL+R` to reload your configuration (theme, settings, locale)
- Use `Shift+Del` in insert mode to clear the search input
- Choose your preferred startup mode (normal or insert) in settings
- Close any popup window with `q`
- Click and navigate the "available updates" window with your mouse
- Updated Hungarian translations
- Localization for Import help messages

### Fixed
- Fixed keybind conflicts in news viewer
- Fixed alignment issues for Hungarian translation using Unicode display width
- Fixed CLI command exit behavior documentation

### Contributors
- @summoner001
- @LoayGhreeb
- @Firstp1ck

---

## [0.5.1] - 2025-11-21

### Added
- **Artix Linux Support**: Pacsea now fully supports Artix Linux
- Full repository filtering and mirror management for Artix systems
- Automatic Artix detection with appropriate tools
- **Hungarian Language Support**: Full interface translation to Hungarian (hu-HU)
- **Package Update Availability**: Automatic background check for available package updates
- "Updates available" button showing count of packages with updates ready
- Detailed update list with current and new versions for each package
- Supports both official repository packages and AUR packages
- Preflight modal now supports scrolling for better navigation
- New scrollable updates modal with side-by-side version comparison

### Changed
- Improved display of installed packages
- Better visual feedback during package operations
- Enhanced dropdown menus with ESC key support to close them quickly

### Fixed
- Fixed issues with test output interfering with mouse interactions
- Improved handling of edge cases when no AUR packages are present
- Better error handling throughout the application

### Contributors
- @summoner001
- @Firstp1ck

---

## [0.5.0] - 2025-11-14

### Added
- Initial release of version 0.5.x series
- Foundation for security scanning features
- Enhanced package management capabilities

---

## [0.4.5] - 2025-11-15

### Added
- VirusTotal API integration for security scanning
- Setup guide for VirusTotal API key in Optional Deps
- Security tool installation via Optional Deps modal

---

## [0.4.4] - 2025-10-27

### Added
- Optional Deps modal: environment-aware, keyboard-driven install of editors, terminals, clipboard tools, mirror updaters, and AUR helpers
- Desktop-aware defaults: prefer GNOME Terminal on GNOME, Klipper on KDE
- Emacs/emacsclient editor options

### Changed
- Mirror selection/count settings
- Forced DB refresh with `-Syyu` for Pacman/AUR

### Improved
- Headless mode + smoke test
- Deduplicated search results
- Viewport-limited rendering
- Periodic Arch status refresh
- Software-rendering flags for GNOME Console/KGX

---

## [0.4.3] - 2025-10-26

### Added
- Manjaro support with `[Manjaro]` filter label in Results
- New keybinds to open/close dropdowns for faster navigation

### Fixed
- Update dialog skips mirror refresh if `reflector` is missing; pacman and AUR updates continue

### Changed
- Unified Manjaro detection (name `manjaro-` or owner contains "manjaro")
- Mirror refresh uses `pacman-mirrors` on Manjaro, `reflector` on Arch (both optional)
- If no AUR helper is found, Pacsea offers to install `paru` or `yay`

### Contributors
- @MS-Jahan

---

## [0.4.2] - 2025-10-23

### Fixed
- System Update/Install on XFCE now runs correctly
- Changed `xfce4-terminal` launch to use `--command "bash -lc '<cmd>'"` to avoid parsing issues

### Changed
- Terminal selection prefers PATH order for predictability
- Added safe single-quote escaping helper for shell commands

### Technical
- Ensure output file parent directories exist in tests
- Tests now run single-threaded to avoid PATH/env races

### Contributors
- @MS-Jahan

---

## [0.4.1] - 2025-10-12

### Fixed
- Left Arrow in the Search pane no longer crashes the TUI

### Added
- Downgrade workflow to roll back installed packages to a previous version
- Confirmation prompts for safety during downgrade
- Respects dry-run behavior during downgrade

---

## [0.4.0] - 2025-10-11

### Added
- **System Update dialog**: Toggle mirror updates (with country selector), system packages, AUR packages, and cache cleanup
- **Arch Linux News popup**: Shows recent items with "critical" or "manual intervention" highlighted; Enter opens link
- Auto-check for news on startup (shows popup if news dated today)
- AUR/Arch status indicator from `status.archlinux.org` with color cue
- **Installed-only mode**: Switch Results to installed packages with Remove List
- Core-package warning during uninstall
- EndeavourOS repo support (`eos`, `endeavouros`) with `[EOS]` filter
- Panels dropdown to hide/show Recent, Install/Remove, and Keybinds footer
- Config/Lists dropdown for quick access to config files

### Files
- `removed_packages.txt` — names of removed packages

---

## [0.3.0] - 2025-10-09

### Added
- **PKGBUILD viewer** with toggle (`Ctrl+X`) and clipboard copy
- Mouse wheel scrolling in PKGBUILD viewer
- Clickable repo filters in Results: `[AUR]`, `[core]`, `[extra]`, `[multilib]`
- Repo badges and AUR popularity in Results and Install lists
- **Best matches** relevance sort mode
- Vim-style Search modes: Normal/Insert with select/delete
- Package Info follows focused pane (Results/Recent/Install)
- Live config reload with `Ctrl+R`
- Customizable keybindings for global and per-pane actions
- `--dry-run` mode prints exact commands without performing installs

---

## [0.2.0] - 2025-10-02

### Added
- In-pane search with `/` key in Recent and Install panels
- Clickable URLs in package information (opens in default browser)
- Vim-style `j`/`k` navigation in all panes
- Modal confirmation dialogs for installations
- Ring prefetching for faster navigation

### Improved
- Enhanced pane switching with Tab/Shift+Tab
- Smoother scrolling with reduced lag
- Better visual indicators for loading and selection states
- Optimized memory usage and async task management
- Enhanced error handling with clearer user feedback

---

## [0.1.0] - 2025-09-25

### Added
- **Initial release** of Pacsea
- Fast search across official repos and the AUR in one place
- Three-pane view: Results, Recent history, Install list, and Package details
- Add packages with Space; install everything with Enter
- Recent searches remembered for quick reuse
- Install history logged to `install_log.txt`
- Safe trial mode with `--dry-run`
- Preloads top results for smoother browsing
- Background package list refresh
- Uses local data when possible, goes online if needed

### Files
- `official_index.json` — cached list of official packages
- `details_cache.json` — extra info for details panel
- `recent_searches.json` — recent searches
- `install_list.json` — current install queue
- `install_log.txt` — installation history

---

[0.6.2]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.6.2
[0.6.1]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.6.1
[0.6.0]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.6.0
[0.5.3]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.5.3
[0.5.2]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.5.2
[0.5.1]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.5.1
[0.5.0]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.5.0
[0.4.5]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.4.5
[0.4.4]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.4.4
[0.4.3]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.4.3
[0.4.2]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.4.2
[0.4.1]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.4.1
[0.4.0]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.4.0
[0.3.0]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.3.0
[0.2.0]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.2.0
[0.1.0]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.1.0

