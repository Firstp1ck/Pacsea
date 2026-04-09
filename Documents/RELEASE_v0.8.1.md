# Release v0.8.1

## What's New

### ✨ Features

**Guided setup got a big upgrade**
- New startup setup selector with chained onboarding steps for optional dependencies, AUR SSH setup, VirusTotal, and news setup.
- New privilege setup wizards for both **sudo timestamp** and **doas persist**, with clearer checks and guidance.
- Optional deps now includes a direct **[Wizard]** entry to jump into guided setup.

**Updates view is faster and easier to use**
- Updates modal now uses a clearer `repo/name  old -> new` layout with improved version-diff highlighting.
- Added slash filtering, better keyboard navigation, multi-select behavior, and more reliable wrapped-row mouse handling.
- Update checks now surface when results are non-authoritative and explain why.

### 🛡 Security & Reliability

- Improved handling of sensitive operations to make privileged workflows safer by default.
- Strengthened how temporary files and local logs are managed to better protect local data.
- Reduced exposure of sensitive command output in persisted logs.
- Added extra safeguards around network-related operations to improve resilience.
- Expanded automated security checks in CI and local development workflows.

### 🐛 Bug Fixes

- Fixed first-start sequencing so startup dialogs and announcements no longer fight each other.
- Fixed stale or confusing startup setup labels by showing accurate disable reasons (e.g. requires sudo/doas).
- Fixed setup modal key handling so Enter/Esc/close actions do not leak into unintended next actions.
- Fixed startup news behavior to avoid unwanted auto-popups and stale modal restore paths.
- Improved startup/search/update edge cases (empty startup results, stale cache paths, and filtered empty-state handling).

## Technical Details

This patch release focuses on startup workflow quality, privilege tooling clarity, updates modal usability, and a broad round of hardening in command/logging/security paths. The result is a safer and more predictable daily flow, especially on first run and privileged operations.

## Full Changelog

**Full Changelog**: https://github.com/Firstp1ck/Pacsea/compare/v0.8.0...v0.8.1
