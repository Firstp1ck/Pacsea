# Release v0.6.2

## What's New

### ⚡ Force Sync Option
System Update modal now includes a **Force Sync** mode:
- Toggle between `Normal (-Syu)` and `Force Sync (-Syyu)` on the pacman update row
- Use `←`/`→` or `Tab` keys to switch sync mode
- Force sync refreshes all package databases even if unchanged

### 🐛 Bug Fixes
- **Install list preserved**: System update no longer clears queued packages from the install list
- **Faster exit**: App now closes immediately when exiting during preflight loading
- **Auto-refresh**: Available updates count refreshes automatically after install/remove/downgrade operations

### 🌍 Translations
- Updated Hungarian translations

## Changes

* feat: add force sync option to system update modal by @Firstp1ck
* fix: system update no longer clears queued install_list packages by @Firstp1ck
* fix: ensure app closes immediately when exiting during preflight loading by @Firstp1ck
* Translation: Update hu-HU.yml by @summoner001

**Full Changelog**: https://github.com/Firstp1ck/Pacsea/compare/v0.6.1...v0.6.2

