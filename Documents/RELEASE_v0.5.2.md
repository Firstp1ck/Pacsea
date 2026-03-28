# Release v0.5.2

## What's New

### 🔍 Fuzzy Search
Find packages faster with fuzzy search! Press `CTRL+F` to toggle fuzzy search mode, which lets you find packages even when you don't know the exact name. The search is smarter and more flexible, making package discovery easier.

### ⌨️ Better Controls
- Press `CTRL+R` to reload your configuration (theme, settings, locale)
- Use `Shift+Del` in insert mode to clear the search input
- Choose your preferred startup mode (normal or insert) in settings
- Close any popup window with `q`
- Click and navigate the "available updates" window with your mouse

### 🌍 Updated Translations
- Updated Hungarian translations
- Localization for Import help messages

### 🐛 Bug Fixes
- Fixed keybind conflicts in news viewer
- Fixed alignment issues for Hungarian translation using Unicode display width
- Fixed CLI command exit behavior documentation

## Contributors

* @summoner001
* @LoayGhreeb

## Installation

Update to v0.5.2:

```bash
# For stable release
paru -S pacsea-bin   # or: yay -S pacsea-bin

# For latest from git
paru -S pacsea-git   # or: yay -S pacsea-git
```

## Technical Details

### Code Improvements
- Fuzzy search implementation with matcher instance reuse for better performance
- Enhanced search query processing with dedicated scoring and sorting functions
- Refactored key binding logic consolidated into `build_default_keymap()`
- Extensive code refactoring to reduce complexity and improve maintainability
- Updated dependencies: `cc` 1.2.47, `hashbrown` 0.16.1, `indexmap` 2.12.1, `signal-hook-registry` 1.4.7, `syn` 2.0.111, `unicode-width` 0.2.0
- Improved function signatures using references instead of owned values
- Enhanced error handling with clearer messages
- Updated Clippy configuration (function line count threshold: 100 → 150)
- Better test consistency and error messages
- Updated README with comprehensive commandline flags documentation

### Pull Requests
- [#44](https://github.com/Firstp1ck/Pacsea/pull/44) - Translation: Update hu-HU.yml by @summoner001
- [#47](https://github.com/Firstp1ck/Pacsea/pull/47) - Translation: Update hu-HU.yml by @summoner001
- [#48](https://github.com/Firstp1ck/Pacsea/pull/48) - Dev/clippy fixes by @Firstp1ck
- [#49](https://github.com/Firstp1ck/Pacsea/pull/49) - Translation: Update hu-HU.yml by @summoner001
- [#50](https://github.com/Firstp1ck/Pacsea/pull/50) - Feat/harmonize keybinds by @Firstp1ck
- [#51](https://github.com/Firstp1ck/Pacsea/pull/51) - Translation: Update hu-HU.yml by @summoner001
- [#54](https://github.com/Firstp1ck/Pacsea/pull/54) - Add fuzzy search functionality and related enhancements by @Firstp1ck

## Full Changelog

**Full Changelog**: https://github.com/Firstp1ck/Pacsea/compare/v0.5.1...v0.5.2

