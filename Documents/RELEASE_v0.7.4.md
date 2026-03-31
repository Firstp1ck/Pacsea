# Release v0.7.4

## What's New

### ✨ Features

**Configurable privilege escalation (sudo / doas)**
- New `privilege_tool` setting: `auto` | `sudo` | `doas`
- Commands now run through the selected tool (or auto-detected one) instead of always using sudo

**Interactive authentication mode (better for fingerprint / PAM prompts)**
- New `auth_mode` setting: `prompt` | `passwordless_only` | `interactive`
- Interactive mode hands off to the terminal so sudo/doas can handle PAM prompts directly (including fingerprint via fprintd, when configured)

**BlackArch repository support**
- Detects the `blackarch` repo and adds a toggle/filter in results when available

### 🐛 Bug Fixes

**Clear errors for misconfigured sudo/doas**
- Privilege-tool resolution errors are now surfaced instead of being silently masked

**Theme config robustness**
- Ensures required `theme.conf` keys exist before first theme load (auto-backfills from the shipped skeleton)

## Technical Details

This release focuses on making privileged operations more reliable across environments (sudo vs doas, passwordless vs interactive auth), adds optional BlackArch repo handling, and improves startup/theme config preflight. Backward compatibility is preserved via defaults and legacy setting mappings.

## Full Changelog

**Full Changelog**: https://github.com/Firstp1ck/Pacsea/compare/v0.7.3...v0.7.4

