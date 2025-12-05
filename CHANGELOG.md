# Changelog

All notable changes to Pacsea will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

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
- Security scanning before AUR package installation
- ClamAV malware scanning integration
- Trivy vulnerability scanning integration
- Semgrep static analysis integration
- ShellCheck PKGBUILD linting
- VirusTotal hash-based malware detection
- Custom suspicious bash pattern detection
- aur-sleuth LLM-powered security audit integration

### How to Use
1. Add AUR package(s) to Install list → Enter (Confirm Install)
2. Press `S` to run the scan
3. Configure which scans to run
4. Review the summary at the end

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

[0.6.1]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.6.1
[0.6.0]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.6.0
[0.5.3]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.5.3
[0.4.5]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.4.5
[0.4.4]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.4.4
[0.4.3]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.4.3
[0.4.2]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.4.2
[0.4.1]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.4.1
[0.4.0]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.4.0
[0.3.0]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.3.0
[0.2.0]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.2.0
[0.1.0]: https://github.com/Firstp1ck/Pacsea/releases/tag/v0.1.0

