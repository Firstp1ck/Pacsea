# Release v0.8.2

## What's New

This release introduces UI customization enhancements, better mouse interaction, and improved PKGBUILD fetching reliability.

### ✨ Features

- **Configurable UI Layout**: Added the ability to configure the order of the main vertical panes (search, results, details). You can now set `main_pane_order` and per-role vertical size limits inside `settings.conf` and format the interface exactly how you want it.
- **Mouse Wheel Navigation**: You can now use the mouse scroll wheel to move row focus inside system update menus, repositories, and optional dependency modals.
- **Desktop Integration**: Added `.desktop` file and SVG application icon natively to the build files for better desktop environment integration.

### 🛡 Security & reliability

- **PKGBUILD fetching**: Moved PKGBUILD requests into their own separate asynchronous tasks so slow official fetches cannot stall the worker queue. Dropped stale fetch results from rendering on row transitions.

### 🐛 Fixes

- Added `--connect-timeout` constraints to PKGBUILD fetch curl calls so unreachable hosts fail faster.
- Fixed `makepkg` environment issues (like `CHOST` overlap) when building from git PKGBUILD on systems with cross-compilation configurations present in `makepkg.conf`.
- Restructured and improved the release workflow, including versions retrieval, artifact checklists, and proper sparse checkout configuration targets.

## Technical Details

v0.8.2 improves the underlying UI rendering constraints and adds the parser hooks for customized window layouts while vastly improving the stability of asynchronous PKGBUILD downloads.

## Full Changelog

**Full Changelog**: https://github.com/Firstp1ck/Pacsea/compare/v0.8.1...v0.8.2