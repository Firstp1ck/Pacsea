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

## Supported Platforms
| Supported Distributions | Supported Languages |
|:---|:---|
| [![Arch Linux](https://img.shields.io/badge/Arch%20Linux-1793D1?logo=arch-linux&logoColor=white)](https://archlinux.org/) | [![English](https://img.shields.io/badge/English-1793D1)](https://github.com/Firstp1ck/Pacsea) |
| [![EndeavourOS](https://img.shields.io/badge/EndeavourOS-1793D1?logo=endeavouros&logoColor=white)](https://endeavouros.com/) | [![German](https://img.shields.io/badge/German-1793D1)](https://github.com/Firstp1ck/Pacsea) |
| [![CachyOS](https://img.shields.io/badge/CachyOS-1793D1?logo=arch-linux&logoColor=white)](https://cachyos.org/) | [![Hungarian](https://img.shields.io/badge/Hungarian-1793D1)](https://github.com/Firstp1ck/Pacsea) |
| [![Manjaro](https://img.shields.io/badge/Manjaro-35BF5C?logo=manjaro&logoColor=white)](https://manjaro.org/) | |
| [![Artix](https://img.shields.io/badge/Artix-1793D1?logo=arch-linux&logoColor=white)](https://artixlinux.org/) | |


### Main app view
![Main app view (v0.5.2)](Images/AppView_v0.5.2.png "Main app view (v0.5.2)")

## Table of Contents
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

- **Run**:
```bash
pacsea
```

> Prefer a dry run first? Add `--dry-run`.

## Features
- **Security Scan for AUR Packages**: Comprehensive security scanning workflow with multiple tools (ClamAV, Trivy, Semgrep, ShellCheck, VirusTotal, custom patterns, aur-sleuth) and detailed scan summaries
- **Fuzzy Search**: Toggle flexible fuzzy search mode to find packages even without exact names
- **Unified search**: Fast results across official repos and the AUR.
- **Package Update Availability**: Automatic background checks with detailed version comparison view
- **Keyboard‑first**: Minimal keystrokes, Vim‑friendly navigation.
- **Queue & install**: Add packages to queue and confirm installs. Run security scans for AUR packages before installing.
- **Always‑visible details**: Open package links with a click.
- **PKGBUILD preview**: Toggle viewer; copy PKGBUILD with one click.
- **Persistent lists**: Recent searches and Install list are saved.
- **Installed‑only mode**: Review and remove installed packages safely. Configure filter mode to show only leaf packages (default) or all explicitly installed packages.
- **Package downgrade**: Downgrade installed packages to previous versions using the `downgrade` tool.
- **Distro-aware updates**: Automatic detection and use of appropriate mirror tools for Manjaro, EndeavourOS, CachyOS, Artix, and standard Arch
- **Helpful tools**: System update dialog with distro-aware mirror management and Arch News popup.

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

For a complete reference of all keyboard shortcuts, see the [Keyboard Shortcuts](https://github.com/Firstp1ck/Pacsea/wiki/Keyboard-Shortcuts) wiki page.

### PKGBUILD preview
![PKGBUILD preview (v0.4.1)](Images/PKGBUILD_v0.4.5.png "PKGBUILD preview (v0.4.1)")

## CLI Commands

Pacsea supports powerful command-line operations, allowing you to manage packages without launching the TUI. For a complete list of all CLI commands, options, and detailed usage instructions, see the [CLI Commands](https://github.com/Firstp1ck/Pacsea/wiki/How-to-use-Pacsea#cli-commands) section in the wiki.

You can also run `pacsea --help` to see all available commands and options.

## Configuration

Pacsea uses three configuration files located in `~/.config/pacsea/`:
- `settings.conf` — app behavior (layout, defaults, visibility, scans, news, etc.)
- `theme.conf` — colors and styling
- `keybinds.conf` — keyboard shortcuts

For complete configuration documentation, including all available settings, theme customization, and keybind configuration, see the [Configuration](https://github.com/Firstp1ck/Pacsea/wiki/Configuration) wiki page.

Example configuration files are available in the [`config/`](config/) directory.

![Settings overview (v0.4.1)](Images/Settings_v0.4.1.png "Settings overview (v0.4.1)")

### Preflight Modal

By default, Pacsea shows a Preflight review modal before installs/removals. This allows you to inspect dependencies, files, config conflicts, and optionally run AUR security scans.

**For Install actions**: Review dependencies that will be installed, files that will be added, and optionally run security scans for AUR packages.

**For Remove actions**: Review reverse dependencies (packages that depend on what you're removing), affected services, and files that will be removed. Meta-packages show warnings when they have no reverse dependencies, as removal may affect system state.

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

### Community Suggestions: Priority Features
- **Add Flags in result pane for packages that are: not maintained, orphaned and outdated**
- **Adjustable Height of the "Results", "Package Info" and "Search" panes**
- **Add possibility to switch locations of Top/Center/Bottom panes**
- **View AUR package comments**
- **Vote for AUR packages via SSH connection**


### Other Potential Features
- **Show with Hover over button, what the button does**
- **Mirror Search and extensive Mirror Selection**
- **Add Chaotic AUR setup and add Garuda Repository Support**
- **Multi Package Manager Support for: Debian-Based (apt), Fedora-Based (dnf) and Flatpak Support**
- **Add custom Repository Support (e.g Make Cachy/Manjaro/EOS Repositories available to other Arch based Systems)**
- **Ability to resolve dependency conflicts**
- **Add accessability themes for visual impairments**
- **Add PKGBUILD Preview shellcheck and namcap**
- **Add custom upgrade commands**
- **Add possibility to view News for the respectiv Distro: EndeavourOS, Manjaro, Garuda and  CachyOS**
- **Add possibility to view News based on installed Packages (Including AUR comments)**
- **Available Update button. Opens a Preview with old and new version. -> done;**
  - grouped by system critical updates like Kernel, systemd and other CORE packages that need restart and other packages (pacman and aur, incl. search/filter)
- **Commandline Flags -u and --update use set "Update System" from the TUI settings.**
- **Ability to maintain your AUR packages**

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
