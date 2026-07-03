# Release v0.5.1

## What's New

### 🎉 Artix Linux Support
Pacsea now fully supports Artix Linux! You can now use Pacsea on Artix systems with full repository filtering and mirror management. The app will automatically detect Artix and use the appropriate tools for managing your packages.

### 🌍 Hungarian Language Support
Hungarian speakers can now use Pacsea in their native language! The interface has been fully translated to Hungarian (hu-HU), with ongoing improvements and refinements to ensure the best possible experience for Hungarian users.

### ⚡ Performance & Stability Improvements
- Faster package dependency resolution
- Improved preflight modal that opens instantly without blocking the interface
- Better handling of offline scenarios with cached package information
- Enhanced error messages and user feedback

### 🔄 Package Update Availability
Pacsea now automatically checks for available package updates in the background! A new "Updates available" button appears at the top of the interface showing how many packages have updates ready. Click the button to view a detailed list of all available updates, showing both your current version and the new version for each package. The update list includes both official repository packages and AUR packages, making it easy to see what's ready to update at a glance.

### 🎨 User Interface Enhancements
- Preflight modal now supports scrolling for better navigation
- Improved display of installed packages
- Better visual feedback during package operations
- Enhanced dropdown menus with ESC key support to close them quickly
- New scrollable updates modal with side-by-side version comparison

### 🐛 Bug Fixes
- Fixed issues with test output interfering with mouse interactions
- Improved handling of edge cases when no AUR packages are present
- Better error handling throughout the application

## Technical Details

This release includes significant code refactoring to improve maintainability and reduce complexity. The refactoring focused on:
- Event handling improvements (modal events, mouse events, key bindings)
- Dependency resolution optimizations
- Enhanced logging and error handling
- Updated dependencies: clap 4.5.52 and various other packages
- Improved development tooling and scripts

## New Contributors

* @summoner001 made their first contribution by adding Hungarian language support

## Bug Reports

Thank you to the following contributors for reporting bugs:

- @skypher for reporting the Artix Linux Support bug
[34](https://github.com/Firstp1ck/Pacsea/issues/34) - Artix Linux Support

- @phihos for suggesting mark feature in "Results" pane (last Release)
[22](https://github.com/Firstp1ck/Pacsea/issues/22) - Mark feature in "Results" pane

## Pull Requests

- [#34](https://github.com/Firstp1ck/Pacsea/pull/34) - Feat/artix support by @Firstp1ck
- [#33](https://github.com/Firstp1ck/Pacsea/pull/33) - Translation: Create hungarian language file by @summoner001
- [#39](https://github.com/Firstp1ck/Pacsea/pull/39) - Translation: update hu-HU.yml by @summoner001
- [#40](https://github.com/Firstp1ck/Pacsea/pull/40) - Refactor/reduce complexity by @Firstp1ck
- [#41](https://github.com/Firstp1ck/Pacsea/pull/41) - Translation: Update hu-HU.yml by @summoner001
- [#42](https://github.com/Firstp1ck/Pacsea/pull/42) - Feat/update availability by @Firstp1ck

## Installation

Update to v0.5.1:

```bash
# For stable release
paru -S pacsea-bin   # or: yay -S pacsea-bin

# For latest from git
paru -S pacsea-git   # or: yay -S pacsea-git
```

## Full Changelog

**Full Changelog**: https://github.com/Firstp1ck/Pacsea/compare/v0.5.0...v0.5.1