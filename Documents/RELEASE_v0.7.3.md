# Release v0.7.3

## What's New

### ✨ Features

**Passwordless sudo**
- TUI install/update/downgrade operations can use passwordless sudo when configured
- Same behavior as CLI: no password prompt when sudo allows it
- Remove operations always ask for password for safety

**Config editing with your editor**
- Opening config files now uses your `VISUAL` or `EDITOR` environment variable
- Edit settings, theme, and keybinds in your preferred editor

### 🐛 Bug Fixes

**Numpad Enter (Issue #119)**
- Numpad Enter now submits in password prompt, search, and all modals
- Fixes submit not working when using numpad Enter in password field, search, optional deps, import, system update, and others

**Security / Code quality**
- Addressed CodeQL alerts for cleartext logging of sensitive data (#6, #7)
- Tests no longer expose passwords or API keys in assert messages

## Technical Details

This release adds passwordless sudo support, editor-based config editing, and numpad Enter handling. All changes are backward compatible.

## Full Changelog

**Full Changelog**: https://github.com/Firstp1ck/Pacsea/compare/v0.7.2...v0.7.3
