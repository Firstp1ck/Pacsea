# Pacsea

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Made with Rust](https://img.shields.io/badge/Made%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![Target: Arch Linux](https://img.shields.io/badge/Target-Arch%20Linux-1793D1?logo=arch-linux&logoColor=white)](https://archlinux.org/)

Pacsea is a fast, friendly TUI for browsing and installing Arch and AUR packages — built for speed and minimal keystrokes.

## Community
<p align="center">
✨ Idea or bug? <strong><a href="https://github.com/Firstp1ck/Pacsea/issues">Open an issue</a></strong> or check out <strong><a href="https://github.com/Firstp1ck/Pacsea/discussions/11">Idea Discussions</a></strong><br/>
❤️ Thank you to the Pacsea community for your ideas, reports, and support!
</p>

### Main app view
![Main app view (v0.4.1)](Images/AppView_v0.4.5_PKGBUILD_AUR.png "Main app view (v0.4.1)")

## Table of Contents
- [Quick start](#quick-start)
- [Features](#features)
- [Usage](#usage)
  - [Handy shortcuts](#handy-shortcuts)
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

- **Run**:
```bash
pacsea
```

> Prefer a dry run first? Add `--dry-run`.

## Features
- **Security Scan for AUR Packages**: Comprehensive security scanning workflow with multiple tools (ClamAV, Trivy, Semgrep, ShellCheck, VirusTotal, custom patterns, aur-sleuth) and detailed scan summaries
- **Unified search**: Fast results across official repos and the AUR.
- **Keyboard‑first**: Minimal keystrokes, Vim‑friendly navigation.
- **Queue & install**: Space to add, Enter to confirm installs. Press S in the confirm dialog to scan AUR packages before installing.
- **Always‑visible details**: Open package links with a click.
- **PKGBUILD preview**: Toggle viewer; copy PKGBUILD with one click.
- **Persistent lists**: Recent searches and Install list are saved.
- **Installed‑only mode**: Review and remove installed packages safely.
- **Distro-aware updates**: Automatic detection and use of appropriate mirror tools for Manjaro, EndeavourOS, CachyOS, and standard Arch
- **Helpful tools**: System update dialog with distro-aware mirror management and Arch News popup.

## Security-first approach for AUR Packages

- **Security-first approach for installing AUR Packages**.
- **Security Coverage** via automatic **Scans** and **Optional Measures** (e.g., converting AUR packages to **Flatpak**), and more

![Scan configuration (v0.4.5)](Images/AUR_Scan_v0.4.5.png "Scan configuration (v0.4.5)")

### New: Security scans for AUR
Pacsea adds a security‑first workflow for AUR installs. Before building you can run one or more checks — ClamAV (antivirus), Trivy (filesystem), Semgrep (static analysis), ShellCheck for PKGBUILD/.install, VirusTotal hash lookups, custom suspicious pattern scanning, and aur-sleuth (LLM audit). Scans generate a comprehensive summary showing infections, vulnerabilities by severity, Semgrep findings count, and VirusTotal statistics.

**VirusTotal API Setup**: Configure your VirusTotal API key directly from the Optional Deps modal. Press Enter on the "Security: VirusTotal API" entry to open the API key page, then paste and save your key. The modal blocks main UI interactions to prevent accidental clicks/keys.

Future implementation will include: Enhanced AI Security Scan (optional)

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
- Quickly see what's installed and press Enter to install missing packages.

![TUI Optional Deps (v0.4.5)](Images/Optional_Deps_v0.4.5.png "TUI Optional Deps (v0.4.5)")

## Usage
1. Start typing to search.
2. Move with ↑/↓ or PageUp/PageDown.
3. Press Space to add to the Install list.
4. Press Enter to install (or confirm the Install list).
5. **For AUR packages**: Press S in the confirm dialog to scan before installing.
6. Press F1 or ? anytime for a help overlay.
7. **PKGBUILD copy**: For the "Copy PKGBUILD" button, install `wl-clipboard` (Wayland) or `xclip` (X11). 
  The copied PKGBUILD includes a suffix configured in `settings.conf` (`clipboard_suffix`).

### Handy shortcuts
- **Help**: F1 or ?
- **Switch panes**: Tab , ← / →
- **Change sorting**: Shift+Tab
- **Add / Install**: Space / Enter
- **Toggle PKGBUILD viewer**: Ctrl+X (or click the label)
- **Quit**: Ctrl+C

### PKGBUILD preview
![PKGBUILD preview (v0.4.1)](Images/PKGBUILD_v0.4.5.png "PKGBUILD preview (v0.4.1)")

## Configuration
- Config lives in `~/.config/pacsea/` as three files:
  - `settings.conf` — app behavior (layout, defaults, visibility)
  - `theme.conf` — colors and styling
  - `keybinds.conf` — keyboard shortcuts
- Press **Ctrl+R** in the app to reload your theme (`theme.conf`). Settings and keybinds (`settings.conf`, `keybinds.conf`) are read fresh from disk automatically — no reload needed.

For example configuration files, see the [`config/`](config/) directory:
- [`config/settings.conf`](config/settings.conf) — app behavior (layout, defaults, visibility, scans, news, etc.)
- [`config/theme.conf`](config/theme.conf) — colors and styling with multiple theme examples
- [`config/keybinds.conf`](config/keybinds.conf) — keyboard shortcuts for all actions

![Settings overview (v0.4.1)](Images/Settings_v0.4.1.png "Settings overview (v0.4.1)")

### Preflight Modal
By default Pacsea shows a Preflight review modal before installs/removals. This allows you to inspect dependencies, files, config conflicts, and optionally run AUR security scans.

The Install list shows all packages queued for installation. You can export your list to a file or import packages from a previously saved list. The blue refresh icon next to each package indicates the loading/update status.

![Install list (v0.5.0)](Images/Install_List_v0.5.0.png "Install list (v0.5.0)")

![Preflight summary (v0.5.0)](Images/Preflight_summery_v0.5.0.png "Preflight summary (v0.5.0)")

![Preflight sandbox (v0.5.0)](Images/Preflight_sandbox_v0.5.0.png "Preflight sandbox (v0.5.0)")

To skip this modal, change the following key in `~/.config/pacsea/settings.conf`:
```
skip_preflight = true
```

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
- **AUR search errors**: Check your network and try again.
- **Installs don’t start**: Ensure you have a terminal installed (e.g. alacritty, kitty, xterm) and `sudo` working in a terminal.

## Roadmap
- Vote or suggest features: [Feature discussion](https://github.com/Firstp1ck/Pacsea/discussions/11)

- Check out what's next and what I am working on [What's Next...?](https://github.com/Firstp1ck/Pacsea/discussions/26)

### Potential future Features
- **User chooseable Terminal via Options (implemented via settings.conf)** (on going)
- **Keybind harmonization and improvements**
- **Mirror Search and extensive Mirror Selection**
- **Possibiltiy to switch between Normal search and Fuzzy search modes**
- **Add Chaotic AUR setup and add Garuda Repository Support**
- **Adjustable Width of the Middle Panes directly in "Panels" dropdown with instant sync**
- **Adjustable Height of the Results and Package Info Panes**
- **Add Multiple CLI Commands like: `pacsea -S FILENAME.txt` (Installing via File), `pacsea -R FILENAME.txt` (Removing via File), `pacsea --news/-n`, `pacsea --update/-u` (Updating with the set System update Settings from the TUI) and more**
- **Multi Package Manager Support for: Debian-Based (apt), Fedora-Based (dnf) and Flatpak Support**
- **Add custom Repository Support (e.g Make Cachy/Manjaro/EOS Repositories available to other Arch based Systems)**
- **Ability to resolve dependency conflicts**
- **Add accessability themes for visual impairments**
- **Add PKGBUILD Syntax Highlighting**
- **Add custom upgrade commands**
- **Add possibility to view News for the respectiv Distro: EndeavourOS, Manjaro, Garuda and  CachyOS**
- **Add possibility to view News based on installed Packages (Including AUR comments)**

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
