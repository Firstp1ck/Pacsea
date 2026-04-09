# Release v0.8.1

## What's New

Compared to **v0.8.0**, this release improves first-run setup, the updates experience, and the optional **AUR voting** SSH wizard. Everything below is new or different in **v0.8.1**; unchanged areas (custom repos, PKGBUILD checks, core voting behavior, etc.) are not repeated here.

### ✨ Features

- **Startup setup**: A selector runs optional setup tasks in order (optional dependencies, AUR SSH, VirusTotal, news, and related steps). Optional dependencies includes a **[Wizard]** entry. New **sudo timestamp** and **doas persist** setup wizards; install/update/remove can warn when long sessions may hit auth limits.
- **AUR SSH setup**: Guided flow through local key/config, pasting the key on AUR, then a live SSH check. Copy the public key with **C** or the copy row; open the AUR login with **O** when you need it. More reliable first connection to AUR (including host-key handling). Success feedback appears after the remote check succeeds.
- **Updates**: Layout shows **repo/name** and **old → new** versions with clearer diff highlighting. Slash filter, multi-select, and navigation behave more predictably, including with wrapped lines and the mouse. The app can indicate when an update list may be incomplete and why.

### 🛡 Security & reliability

- Tighter handling around privileged commands, temporary scripts, and saved command logs.

### 🐛 Fixes

- Calmer first-run order between setup dialogs and version announcements.
- Clearer labels when a setup task is unavailable (for example wrong privilege tool).
- Setup dialogs no longer leave stray keypresses for the next screen.
- Startup news no longer pops up on its own; leaving news setup does not resurrect an old Arch news window.
- Optional dependency batch installs go through the same auth/preflight path as other installs; terminal integration fixes for multiline follow-up commands and fallback ordering.

## Technical Details

v0.8.1 refines onboarding, privilege tooling, the updates modal, and the AUR SSH setup path shipped in v0.8.0.

## Full Changelog

**Full Changelog**: https://github.com/Firstp1ck/Pacsea/compare/v0.8.0...v0.8.1
