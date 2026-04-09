# Pacsea

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Made with Rust](https://img.shields.io/badge/Made%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![Target: Arch Linux](https://img.shields.io/badge/Target-Arch%20Linux-1793D1?logo=arch-linux&logoColor=white)](https://archlinux.org/)

Pacsea is a TUI application for browsing and installing Arch and AUR packages. It includes an integrated Arch news and advisory feed, keyboard-first navigation, and optional support for extra package repositories you configure yourself.

## Community
<p align="center">
✨ Idea or bug? <strong><a href="https://github.com/Firstp1ck/Pacsea/issues">Open an issue</a></strong> or check out <strong><a href="https://github.com/Firstp1ck/Pacsea/discussions/11">Idea Discussions</a></strong><br/>
❤️ Thank you to the Pacsea community for your ideas, reports, and support!
</p>

## Supported Platforms
| Supported Distributions | Supported Languages |
|:---|:---|
| [![Arch Linux](https://img.shields.io/badge/Arch%20Linux-1793D1?logo=arch-linux&logoColor=white)](https://archlinux.org/) | [![English](https://img.shields.io/badge/English-1793D1)](https://github.com/Firstp1ck/Pacsea) |
| [![EndeavourOS](https://img.shields.io/badge/EndeavourOS-1793D1?logo=endeavouros&logoColor=white)](https://endeavouros.com/) | [![German](https://img.shields.io/badge/German-1793D1)](https://github.com/Firstp1ck/Pacsea) |
| [![CachyOS](https://img.shields.io/badge/CachyOS-1793D1?logo=arch-linux&logoColor=white)](https://cachyos.org/) | [![Hungarian](https://img.shields.io/badge/Hungarian-1793D1)](https://github.com/Firstp1ck/Pacsea) |
| [![Manjaro](https://img.shields.io/badge/Manjaro-35BF5C?logo=manjaro&logoColor=white)](https://manjaro.org/) | |
| [![Artix](https://img.shields.io/badge/Artix-1793D1?logo=arch-linux&logoColor=white)](https://artixlinux.org/) | |
| [![BlackArch](https://img.shields.io/badge/BlackArch-000000?logo=blackarch&logoColor=white)](https://blackarch.org/) | |


### Main app view
![Main app view (v0.7.4)](Images/AppView_v0.7.4.png "Main app view (v0.7.4)")

### News feed view
Browse Arch news, security advisories, package updates, and AUR comments in a unified feed. Filter by source, search with history, bookmark important items, and track read/unread status. All content is cached for offline access and automatically updated in the background.

![News feed view (v0.7.1)](Images/News_feed_v0.7.1.png "News feed view (v0.7.1)")

## Demo

**Part 1** of a multipart walkthrough: [Pacsea demo on YouTube](https://youtu.be/QlAh0Fu1Ges)

## Table of Contents
- [Demo](#demo)
- [Quick start](#quick-start)
- [Features](#features)
- [Usage](#usage)
- [CLI Commands](#cli-commands)
- [Configuration](#configuration)
- [Troubleshooting](#troubleshooting)
- [Roadmap](#roadmap)
- [Credits](#credits)
- [License](#license)

## Quick start
- **Install (stable)**:
```bash
paru -S pacsea-bin   # or: yay -S pacsea-bin
```

- **Install (latest)**:
```bash
paru -S pacsea-git   # or: yay -S pacsea-git
```

- **Install via Cargo**:
```bash
cargo install pacsea
```

- **Run**:
```bash
pacsea
```

> Prefer a dry run first? Add `--dry-run`.

## Features

| Feature | Description |
|---------|-------------|
| **Integrated Process Execution** | Operations execute within the TUI with real-time output streaming and progress bars. Supports configurable privilege escalation (`sudo`/`doas`/auto), passwordless mode when available, and optional interactive authentication handoff so sudo/doas can prompt in the terminal when needed (password or PAM/fingerprint via fprintd, when configured) |
| **Privilege setup wizards** | Built-in setup flows for `sudo` timestamp caching and `doas persist`, including guided steps, validation checklists, and context-aware visibility based on your selected privilege tool |
| **News feed & advisories** | Unified news feed combining Arch news, security advisories, package update notifications, and AUR package comments. Includes offline access with automatic caching, filtering by source or date, search with history, bookmarking, read/unread tracking, and background updates |
| **Security Scan for AUR Packages** | Comprehensive security scanning workflow with multiple tools (ClamAV, Trivy, Semgrep, ShellCheck, VirusTotal, custom patterns, aur-sleuth) and detailed scan summaries |
| **Fuzzy Search** | Toggle flexible fuzzy search mode to find packages even without exact names |
| **Unified search** | Fast results across official packages, any extra repositories you configure, and the AUR.
| **Custom sync repositories (optional)** | Add optional package sources to the same search as the defaults and the AUR, and show or hide each source in the UI.
| **Package Update Availability** | Automatic background checks with detailed version comparison view |
| **Keyboard‑first** | Minimal keystrokes, Vim‑friendly navigation; numpad Enter works for submit in prompts and modals |
| **Queue & install** | Add packages to queue and confirm installs. Run security scans for AUR packages before installing |
| **Always‑visible details** | Open package links with a click |
| **PKGBUILD preview** | Toggle viewer; copy PKGBUILD with one click |
| **AUR Comments viewer** | View community comments for AUR packages with markdown support, clickable URLs, and automatic updates when navigating packages |
| **AUR status markings** | Visual indicators for out-of-date [OOD] and orphaned [ORPHAN] packages |
| **Persistent lists** | Recent searches and Install list are saved |
| **Installed‑only mode** | Review and remove installed packages safely. Configure filter mode to show only leaf packages (default) or all explicitly installed packages |
| **Package downgrade** | Downgrade installed packages to previous versions using the `downgrade` tool |
| **Distro-aware updates** | Automatic detection and use of appropriate mirror tools for Manjaro, EndeavourOS, CachyOS, Artix, and standard Arch |
| **Updates modal** | View available updates with Preflight integration for safe installation |
| **Helpful tools** | System update dialog with Force Sync mode (-Syyu), AUR update confirmation when pacman fails, distro-aware mirror management, and Arch News popup |
| **Long-run auth readiness checks** | Pacsea evaluates whether your current privilege/auth setup can complete long-running actions and shows a one-time warning with next steps before update/install/remove tasks start |
| **AUR package voting (SSH)** | Vote and unvote AUR packages directly from search results through an SSH-based workflow, including guided setup and connection checks |
| **Announcements** | Version-specific and remote announcements shown at startup with clickable URLs and persistent read status |

## Security-first approach for AUR Packages

- **Security-first approach for installing AUR Packages**.
- **Security Coverage** via automatic **Scans** and **Optional Measures** (e.g., converting AUR packages to **Flatpak** (in Planning)), and more

![Scan configuration (v0.4.5)](Images/AUR_Scan_v0.4.5.png "Scan configuration (v0.4.5)")

### Security scans for AUR
Pacsea adds a security‑first workflow for AUR installs. Before building you can run one or more checks — ClamAV (antivirus), Trivy (filesystem), Semgrep (static analysis), ShellCheck for PKGBUILD/.install, VirusTotal hash lookups, custom suspicious pattern scanning, and aur-sleuth (LLM audit). Scans generate a comprehensive summary showing infections, vulnerabilities by severity, Semgrep findings count, and VirusTotal statistics.

**VirusTotal API Setup**: Configure your VirusTotal API key directly from the Optional Deps modal. The modal blocks main UI interactions to prevent accidental clicks/keys. For detailed setup instructions, see the [How to use Pacsea](https://github.com/Firstp1ck/Pacsea/wiki/How-to-use-Pacsea#security-scans-for-aur-packages) wiki page.

### System update dialog
![System update dialog (v0.4.1)](Images/SystemUpdateView_v0.4.5.png "System update dialog (v0.4.1)")

### TUI Optional Deps
- Install and verify recommended helper tools directly from a dedicated view with environment-aware defaults. 
- Desktop-aware preferences include GNOME Terminal on GNOME, Klipper on KDE, and support for multiple editors (nvim, vim, helix, emacs/emacsclient, nano). 
- The modal detects your:
  - environment (Wayland/X11, desktop environment, distro) 
  - and shows relevant options. 
  - Tools include editors, terminals, clipboard utilities (wl-clipboard for Wayland, xclip for X11), 
  - mirror updaters (reflector, pacman-mirrors, eos-rankmirrors, cachyos-rate-mirrors), 
  - AUR helpers (paru, yay), and 
  - security utilities (ClamAV, Trivy, Semgrep, ShellCheck, VirusTotal API setup, aur-sleuth). 
- Quickly see what's installed and install missing packages directly from the modal.

![TUI Optional Deps (v0.4.5)](Images/Optional_Deps_v0.4.5.png "TUI Optional Deps (v0.4.5)")

## Usage

Pacsea provides a keyboard-first interface for searching, queueing, and installing packages. For detailed usage instructions, keyboard shortcuts, and workflows, see the [How to use Pacsea](https://github.com/Firstp1ck/Pacsea/wiki/How-to-use-Pacsea) wiki page.

**Quick overview:**
- Type to search packages across official repos and AUR
- Queue packages for installation
- Review packages before installing with the Preflight modal
- Run security scans for AUR packages
- Manage installed packages, including removal and downgrade
- Switch to News mode (Options → News) or start directly in News mode to browse Arch news, security advisories, package updates, and AUR comments. Filter by source/age/installed-only, search with history (independent search inputs for each mode), bookmark/read items, and track package changes with automatic detection
- Open **Options → Repositories** to review and apply your optional repository settings—**Space** turns lines on or off; after changes are applied, you might see a short prompt if something you already installed also shows up in a newly enabled source
- All operations execute directly in the TUI with real-time output and progress indicators

For a complete reference of all keyboard shortcuts, see the [Keyboard Shortcuts](https://github.com/Firstp1ck/Pacsea/wiki/Keyboard-Shortcuts) wiki page.

### PKGBUILD preview
![PKGBUILD preview (v0.4.1)](Images/PKGBUILD_v0.4.5.png "PKGBUILD preview (v0.4.1)")

### AUR Comments viewer
View community comments for AUR packages directly in Pacsea. Comments are automatically fetched and displayed with markdown formatting support, clickable URLs, and user profile links. The comments pane splits the Package Info area and updates automatically when navigating between packages. Toggle comments visibility with `Ctrl+T` or click the "Show comments" button in Package Info.

## CLI Commands

Pacsea supports powerful command-line operations, allowing you to manage packages without launching the TUI. For a complete list of all CLI commands, options, and detailed usage instructions, see the [CLI Commands](https://github.com/Firstp1ck/Pacsea/wiki/How-to-use-Pacsea#cli-commands) section in the wiki.

You can also run `pacsea --help` to see all available commands and options.

## Configuration

Pacsea uses configuration files in `~/.config/pacsea/`:
- `settings.conf` — app behavior (layout, defaults, visibility, scans, news, custom repo filter toggles, etc.)
- `theme.conf` — colors and styling
- `keybinds.conf` — keyboard shortcuts
- `repos.conf` (optional) — third-party sync repo recipes for search, filters, and the Repositories modal; baseline template: [`config/repos.conf`](config/repos.conf)

Opening any of these from the app uses your `VISUAL` or `EDITOR` environment variable.

Privilege/auth behavior is configurable via `privilege_tool` (`auto` | `sudo` | `doas`) and `auth_mode` (`prompt` | `passwordless_only` | `interactive`) in `settings.conf`.

AUR voting over SSH is configurable in `settings.conf` with `aur_vote_enabled`, `aur_vote_ssh_timeout_seconds`, and `aur_vote_ssh_command`.

For complete configuration documentation, including all available settings, theme customization, and keybind configuration, see the [Configuration](https://github.com/Firstp1ck/Pacsea/wiki/Configuration) wiki page.

Example configuration files are available in the [`config/`](config/) directory.

News mode supports multiple sources (Arch news, advisories, package updates, AUR comments) with smart caching and background processing. Configure via `app_start_mode` (`package` or `news` to start in News mode), `news_filter_*` toggles for each source type, `news_filter_installed_only`, and `news_max_age_days` (default: unlimited) in `settings.conf`.

![Settings overview (v0.4.1)](Images/Settings_v0.4.1.png "Settings overview (v0.4.1)")

### Preflight Modal

By default, Pacsea shows a Preflight review modal before installs/removals. This allows you to inspect dependencies, files, config conflicts, and optionally run AUR security scans.

**For Install actions**: Review dependencies that will be installed, files that will be added, and optionally run security scans for AUR packages.

**For Remove actions**: Review reverse dependencies (packages that depend on what you're removing), affected services, and files that will be removed. Meta-packages show warnings when they have no reverse dependencies, as removal may affect system state. Dependency reports are cached for faster tab switching.

The Install list shows all packages queued for installation. You can export your list to a file or import packages from a previously saved list. The blue refresh icon next to each package indicates the loading/update status.

![Install list (v0.5.0)](Images/Install_List_v0.5.0.png "Install list (v0.5.0)")

![Preflight summary (v0.5.0)](Images/Preflight_summery_v0.5.0.png "Preflight summary (v0.5.0)")

![Preflight sandbox (v0.5.0)](Images/Preflight_sandbox_v0.5.0.png "Preflight sandbox (v0.5.0)")

For detailed information about the Preflight modal, including how to configure it, see the [How to use Pacsea](https://github.com/Firstp1ck/Pacsea/wiki/How-to-use-Pacsea#security-scans-for-aur-packages) wiki page.

### Panels hidden
![Panels hidden (v0.4.1)](Images/PaneHided_v0.4.5.png "Panels hidden (v0.4.1)")
## Optional: build from source
```bash
sudo pacman -S rustup && rustup default stable
git clone https://github.com/Firstp1ck/Pacsea
cd Pacsea
cargo run
```

## Troubleshooting

For troubleshooting common issues, solutions, and diagnostic information, see the [Troubleshooting](https://github.com/Firstp1ck/Pacsea/wiki/Troubleshooting) wiki page.

## Roadmap
- Vote or suggest features: [Feature discussion](https://github.com/Firstp1ck/Pacsea/discussions/11)

- Check out what's next and what I am working on [What's Next...?](https://github.com/Firstp1ck/Pacsea/discussions/26)

### Potential future Features

Longer specs for tracked items live in [`dev/ROADMAP/`](https://github.com/Firstp1ck/Pacsea/tree/main/dev/ROADMAP). Priority tiers and a **GitHub issue cross-reference** table are in [`dev/IMPROVEMENTS/FEATURE_PRIORITY.md`](https://github.com/Firstp1ck/Pacsea/blob/main/dev/IMPROVEMENTS/FEATURE_PRIORITY.md).

### Community Suggestions: Priority Features
- **Adjustable Height of the "Results", "Package Info" and "Search" panes** ([#135](https://github.com/Firstp1ck/Pacsea/issues/135))
- **Add possibility to switch locations of Top/Center/Bottom panes** ([#136](https://github.com/Firstp1ck/Pacsea/issues/136))

### Other Potential Features
- **Show with Hover over button, what the button does** ([#140](https://github.com/Firstp1ck/Pacsea/issues/140))
- **Mirror Search and extensive Mirror Selection** ([#145](https://github.com/Firstp1ck/Pacsea/issues/145))
- **Add Garuda Repository Support** — Garuda/Chaotic-style sync databases are mostly covered by optional **`repos.conf`** (see [#132](https://github.com/Firstp1ck/Pacsea/issues/132), closed); dedicated one-click presets remain a polish item
- **Add possibility to view News for the respective Distro: EndeavourOS, Manjaro, Garuda and CachyOS** ([#131](https://github.com/Firstp1ck/Pacsea/issues/131))
  - grouped by system critical updates like Kernel, systemd and other CORE packages that need restart and other packages (pacman and aur, incl. search/filter) — overlaps [#134](https://github.com/Firstp1ck/Pacsea/issues/134)
- **Implement `rebuild-detector` that checks if a package needs to be rebuild** ([#134](https://github.com/Firstp1ck/Pacsea/issues/134))
- **Add custom upgrade commands** ([#134](https://github.com/Firstp1ck/Pacsea/issues/134))
- **Add accessibility themes for visual impairments** ([#129](https://github.com/Firstp1ck/Pacsea/issues/129))
- **Add System Tray Support for popular Bars like Waybar, Quickshell, Hyprbar, Swaybar, etc.** ([#129](https://github.com/Firstp1ck/Pacsea/issues/129))
- **Ability to resolve dependency conflicts** ([#134](https://github.com/Firstp1ck/Pacsea/issues/134))
- **Ability to maintain your AUR packages** ([#130](https://github.com/Firstp1ck/Pacsea/issues/130))
- **Implement Wiki into the TUI** ([#130](https://github.com/Firstp1ck/Pacsea/issues/130))
- **Multi Package Manager Support for: Debian-Based (apt), Fedora-Based (dnf) and Flatpak Support** ([#130](https://github.com/Firstp1ck/Pacsea/issues/130))
- **View or fetch descriptions for optional dependencies** (ALPM/AUR) ([#102](https://github.com/Firstp1ck/Pacsea/issues/102))
- **Update packages whose sources track GitHub** (detect newer tags/releases) ([#104](https://github.com/Firstp1ck/Pacsea/issues/104))
- **Service restart logic** after relevant package updates ([#99](https://github.com/Firstp1ck/Pacsea/issues/99))
- **Transaction abort / safer cancellation** during long operations ([#98](https://github.com/Firstp1ck/Pacsea/issues/98))
- **Sequential multi-package AUR security scans** ([#95](https://github.com/Firstp1ck/Pacsea/issues/95))
- **CLI: remove packages from a saved install-list file** ([#93](https://github.com/Firstp1ck/Pacsea/issues/93))

## Credits
- Inspired by the following yay commandline: `yay -Slq | fzf --multi --preview 'yay -Sii {}' --preview-window=down:75% --layout=default | xargs -ro yay -S`
- Built with [Ratatui](https://ratatui.rs/) + [Crossterm](https://crates.io/crates/crossterm)
- Powered by Arch + AUR

## License
MIT — see [LICENSE](LICENSE).

## Wiki
Check out the [Wiki](https://github.com/Firstp1ck/Pacsea/wiki) for more information.

## Contributing
Contributions are welcome! Please read the [CONTRIBUTING](CONTRIBUTING.md)
