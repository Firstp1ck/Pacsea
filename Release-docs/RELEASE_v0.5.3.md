# Release v0.5.3

## What's New

### 💬 AUR Comments Viewer
View community comments for AUR packages directly in Pacsea! Press `CTRL+T` or click "Show comments" to toggle. Features markdown support, clickable URLs, and auto-updates when navigating packages.

### 🏷️ AUR Package Status Markings
AUR packages now show status indicators:
- **[OOD]** - Out-of-date packages
- **[ORPHAN]** - Orphaned packages (no maintainer)

### 📱 Collapsed Menu
Automatically appears when the window is too narrow, providing access to all menu options in a compact layout.

### ⬇️ Package Downgrade
Fixed support for downgrading installed packages using the `downgrade` tool. Available in installed-only mode.

### 🔒 Enhanced Security Scanning
- Enhanced **aur-sleuth** (LLM-based audit tool) for the security scanning workflow
- Runs in a separate terminal window with setup wizard in Optional Deps modal

### 🔄 Preflight Improvements
- Faster tab switching with cached dependency reports for remove operations
- Better warnings for meta-packages and dependency conflicts
- Improved service impact detection

### 📦 Updates Modal
- Press Enter to open Preflight for detailed review before installing updates
- Auto-refreshes after installation with real-time progress display

### 🐛 Bug Fixes
- Fixed preflight tabs not resolving when opening packages directly from results
- Fixed Artix filter menu expanding in tight spaces

### 🌍 Translations
- Updated Hungarian translations
- Added translations for new features

## Contributors

* @summoner001
* @Max-Guenther
* @Firstp1ck

## Installation

Update to v0.5.3:

```bash
# For stable release
paru -S pacsea-bin   # or: yay -S pacsea-bin

# For latest from git
paru -S pacsea-git   # or: yay -S pacsea-git
```

## Technical Details

### Code Improvements
- AUR comments viewer with HTML parsing and markdown rendering
- AUR status markings (out-of-date, orphaned) tracking
- Collapsed menu with automatic visibility
- Preflight caching for remove operations
- Refactored update commands to follow ArchWiki guidelines
- Removed package database refresh functionality
- Enhanced configuration reload and error handling
- Added module-level documentation
- Code cleanup and refactoring

### Dependencies
- Added `scraper` 0.20 (HTML parsing)
- Added `chrono` 0.4 (date/time)
- Updated `rpassword` 7.4.0, `unicode-width` 0.2.0

### Pull Requests
- [#69](https://github.com/Firstp1ck/Pacsea/pull/69) - AUR markings by @Firstp1ck
- [#70](https://github.com/Firstp1ck/Pacsea/pull/70) - AUR comments viewer by @Firstp1ck
- [#61](https://github.com/Firstp1ck/Pacsea/pull/61) - Updates modal preflight integration by @Max-Guenther
- [#67](https://github.com/Firstp1ck/Pacsea/pull/67) - Preflight remove packages fixes by @Firstp1ck
- [#62](https://github.com/Firstp1ck/Pacsea/pull/62) - Commandline update fixes by @Firstp1ck
- [#73](https://github.com/Firstp1ck/Pacsea/pull/73) - Dropdown vanish fixes by @Firstp1ck
- [#56](https://github.com/Firstp1ck/Pacsea/pull/56), [#59](https://github.com/Firstp1ck/Pacsea/pull/59), [#65](https://github.com/Firstp1ck/Pacsea/pull/65), [#68](https://github.com/Firstp1ck/Pacsea/pull/68), [#71](https://github.com/Firstp1ck/Pacsea/pull/71), [#74](https://github.com/Firstp1ck/Pacsea/pull/74) - Translation updates by @summoner001

## Full Changelog

**Full Changelog**: https://github.com/Firstp1ck/Pacsea/compare/v0.5.2...v0.5.3

