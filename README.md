# Pacsea

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Made with Rust](https://img.shields.io/badge/Made%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![Target: Arch Linux](https://img.shields.io/badge/Target-Arch%20Linux-1793D1?logo=arch-linux&logoColor=white)](#)

Pacsea is a fast, friendly TUI for browsing and installing Arch and AUR packages — built for speed and minimal keystrokes written in Rust.

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

## Usage
1. Start typing to search.
2. Move with ↑/↓ or PageUp/PageDown.
3. Press Space to add to the Install list.
4. Press Enter to install (or confirm the Install list).
5. Press F1 or ? anytime for a help overlay.

### Handy shortcuts
- **Help**: F1 or ?
- **Switch panes**: Tab / Shift+Tab, ← / →
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
```

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
### Panels hidden
![Panels hidden (v0.4.1)](Images/PaneHided_v0.4.1.png "Panels hidden (v0.4.1)")

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
- **PKGBUILD copy**: For the “Check Package Build” button, install `wl-clipboard` (Wayland) or `xclip` (X11).

## Roadmap
- Vote or suggest features: [Feature discussion](https://github.com/Firstp1ck/Pacsea/discussions/4)
- Upcoming focus: better dependency insights, accessibility, and update controls.

## Credits
- Inspired by Omarchy workflows; built with ratatui + crossterm; powered by Arch + AUR.

## License
MIT — see [LICENSE](LICENSE).

## Wiki
Check out the [Wiki](https://github.com/Firstp1ck/Pacsea/wiki) for more information.