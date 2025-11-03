# Pacsea

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Made with Rust](https://img.shields.io/badge/Made%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![Target: Arch Linux](https://img.shields.io/badge/Target-Arch%20Linux-1793D1?logo=arch-linux&logoColor=white)](https://archlinux.org/)

Pacsea is a fast, friendly TUI for browsing and installing Arch and AUR packages — built for speed and minimal keystrokes written in Rust.

## Top Priority due to the spreading of Malware in the AUR Repository
<div align="center">
  <p>❗<strong>On-going: Security-first approach</strong> for installing <strong>AUR Packages</strong>.❗<br>
  ❗Target: <strong>Security Coverage </strong> via automatic <strong>Scans</strong> and <strong>Optional Measures</strong> (e.g., converting AUR packages to <strong>Flatpak</strong>), and more❗</p>
</div>

### New: Security scans for AUR
Pacsea adds a security‑first workflow for AUR installs. Before building you can run one or more checks — ClamAV (antivirus), Trivy (filesystem), Semgrep (static analysis), ShellCheck for PKGBUILD/.install, and VirusTotal hash lookups.
Future implementation will include: AI Security Scan (of course Optional)

![Scan configuration (v0.4.5)](Images/AUR_Scan_v0.4.5.png "Scan configuration (v0.4.5)")

## Community
<p align="center">
✨ Idea or bug? <strong><a href="https://github.com/Firstp1ck/Pacsea/issues">Open an issue</a></strong> or check out <strong><a href="https://github.com/Firstp1ck/Pacsea/discussions/11">Idea Discussions</a></strong><br/>
❤️ Thank you to the Pacsea community for your ideas, reports, and support!
</p>

### Main app view
![Main app view (v0.4.1)](Images/Appview_v0.4.1_noPKGBUILD.png "Main app view (v0.4.1)")

## Table of Contents
- [Quick start](#quick-start)
- [Features](#features)
- [Usage](#usage)
  - [Handy shortcuts](#handy-shortcuts)
- [Configuration](#configuration)
  - [Example: settings.conf](#example-settingsconf)
  - [Example: theme.conf](#example-themeconf)
  - [Example: keybinds.conf](#example-keybindsconf)
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
- **Unified search**: Fast results across official repos and the AUR.
- **Keyboard‑first**: Minimal keystrokes, Vim‑friendly navigation.
- **Queue & install**: Space to add, Enter to confirm installs.
- **Always‑visible details**: Open package links with a click.
- **PKGBUILD preview**: Toggle viewer; copy PKGBUILD with one click.
- **Persistent lists**: Recent searches and Install list are saved.
- **Installed‑only mode**: Review and remove installed packages safely.
- **Helpful tools**: System update dialog and Arch News popup.

### System update dialog
![System update dialog (v0.4.1)](Images/SystemUpdateView_v0.4.1.png "System update dialog (v0.4.1)")

### TUI Optional Deps
Install and verify recommended helper tools directly from a dedicated view (editor, terminal, AUR helper, and security utilities like ClamAV, Trivy, ShellCheck, and the VirusTotal API). Quickly see what's installed and press Enter to install missing packages.

![TUI Optional Deps (v0.4.5)](Images/Optional_Deps_v0.4.5.png "TUI Optional Deps (v0.4.5)")

## Usage
1. Start typing to search.
2. Move with ↑/↓ or PageUp/PageDown.
3. Press Space to add to the Install list.
4. Press Enter to install (or confirm the Install list).
5. Press F1 or ? anytime for a help overlay.

### Handy shortcuts
- **Help**: F1 or ?
- **Switch panes**: Tab , ← / →
- **Change sorting**: Shift+Tab
- **Add / Install**: Space / Enter
- **Toggle PKGBUILD viewer**: Ctrl+X (or click the label)
- **Quit**: Ctrl+C

### PKGBUILD preview
![PKGBUILD preview (v0.4.1)](Images/PKGBUILD_v0.4.1.png "PKGBUILD preview (v0.4.1)")

## Configuration
- Config lives in `~/.config/pacsea/` as three files:
  - `settings.conf` — app behavior (layout, defaults, visibility)
  - `theme.conf` — colors and styling
  - `keybinds.conf` — keyboard shortcuts
- Press **Ctrl+R** in the app to reload your config.

![Settings overview (v0.4.1)](Images/Settings_v0.4.1.png "Settings overview (v0.4.1)")

### Example: settings.conf
```ini
# Pane sizes (must sum to 100)
layout_left_pct = 20
layout_center_pct = 60
layout_right_pct = 20

# Defaults
app_dry_run_default = false
sort_mode = best_matches  # best_matches | popularity | alphabetical

# Visibility
show_recent_pane = true
show_install_pane = true
show_keybinds_footer = true

# Mirrors
# Select one or more countries (comma-separated). Example: "Switzerland, Germany, Austria"
selected_countries = Worldwide
# Number of HTTPS mirrors to consider when updating
mirror_count = 20
```

### Panels hidden
![Panels hidden (v0.4.1)](Images/PaneHided_v0.4.1.png "Panels hidden (v0.4.1)")

### Example: theme.conf
```ini
# Background
background_base = #1e1e2e
background_mantle = #181825
background_crust = #11111b

# Surfaces
surface_level1 = #45475a
surface_level2 = #585b70

# Text
text_primary = #cdd6f4
text_secondary = 166,173,200

# Accents
accent_interactive = #74c7ec
accent_heading = #cba6f7
accent_emphasis = #b4befe

# Semantics
semantic_success = #a6e3a1
semantic_warning = #f9e2af
semantic_error   = #f38ba8
```

### Example: keybinds.conf
```ini
# Global
keybind_help = F1
keybind_exit = CTRL+C
keybind_reload_theme = CTRL+R
keybind_show_pkgbuild = CTRL+X

# Search
keybind_search_move_up = Up
keybind_search_move_down = Down
keybind_search_add = Space
keybind_search_install = Enter

# Install pane
keybind_install_confirm = Enter
keybind_install_remove = Delete
keybind_install_clear = Shift+Delete
```
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
- **PKGBUILD copy**: For the "Copy Package Build” button, install `wl-clipboard` (Wayland) or `xclip` (X11).

## Roadmap
- Vote or suggest features: [Feature discussion](https://github.com/Firstp1ck/Pacsea/discussions/11)

### Potential future Features
- **Security Scan for AUR Packages (Source Code and PKGBUILD) using multiple tools: ClamAV, VirusTotal, Trivy and Semgrep** (on going)
- **Extensive Preflight Check** (on going)
- **Additional Language Support**
- **Default Mirrors, Mirror Search and extensive Mirror Selection**
- **Adjustable Width of the Middle Panes**
- **Adjustable Height of the Results and Package Info Panes**
- **Add Multiple CLI Commands like: `pacsea -S FILENAME.txt`, `pacsea -R FILENAME.txt`, `pacsea --news`, `pacsea -Syu` and more**
- **Multi Package Manager Support for: Debian-Based (apt), Fedora-Based (dnf) and Flatpak Support**
- **Add custom Repository Support (e.g Make Cachy/Manjaro/EOS Repositories available to other Arch based Systems)**
- **Extensive Testing**
- **Extensive Documentation**

## Credits
- Inspired by Omarchy workflows; built with ratatui + crossterm; powered by Arch + AUR.

## License
MIT — see [LICENSE](LICENSE).

## Wiki
Check out the [Wiki](https://github.com/Firstp1ck/Pacsea/wiki) for more information.
