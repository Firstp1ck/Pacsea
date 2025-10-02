# Pacsea v0.1.0 — Initial release

Date: 2025-09-26

Pacsea is a fast, keyboard‑first TUI for searching, inspecting, and queueing pacman/AUR packages on Arch Linux. This is the first public release.

---

## Highlights
- Fast search across official repos and the AUR in one place
- Clear three‑pane view: results, history, install list, and package details
- Add packages with Space; install everything with one Enter
- Remembers results so it stays quick next time
- Keeps an install history in `install_log.txt`
- Safe trial mode with `--dry-run` (no changes made)

## Features (v0.1.0)
- Search
  - Type to find packages from both official repos and the AUR
  - Shows useful details right away; preloads top results for smoother browsing
  - Stays responsive while you type
- Layout & navigation
  - Always‑visible panels so you never lose context
  - Switch panels easily with Tab or arrow keys
  - Short on‑screen help at the bottom
- History & install list
  - Reuse past searches from the Recent list (Enter runs again; Space adds the first match)
  - Build an Install list with Space; press Enter to install everything at once
  - Quickly search within Recent or Install using “/”
- Installing
  - Installs official packages with pacman
  - Installs AUR packages with paru or yay if available
  - Falls back to a simple shell when needed
  - Writes a timestamped install log
- Speed & data
  - Uses local data when possible for speed; goes online if needed
  - Refreshes its package list in the background and adds more details as you browse
- Saved files
  - `official_index.json` — cached list of official packages
  - `details_cache.json` — extra info used for the details panel
  - `recent_searches.json` — your recent searches
  - `install_list.json` — the current install queue
  - `install_log.txt` — what you chose to install
- Command‑line
  - `--dry-run` — try Pacsea without actually installing anything

## Credits
- Omarchy Distro — workflow inspiration
- ratatui, crossterm — TUI foundations
- Arch Linux & AUR — data and tooling ecosystem