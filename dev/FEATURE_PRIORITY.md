# Pacsea Feature Priority List

> Generated based on analysis of the current codebase architecture, user impact, and implementation complexity.

---

## Quick Version Reference

| Version | Key Feature(s) |
|---------|---------------|
| `v0.6.0` | ✅ **Render actions in TUI (no terminal spawn)** - **COMPLETED** |
| `v0.6.1` | 🔴 Adjustable pane heights |
| `v0.6.2` | 🔴 CLI `--update` respects TUI settings |
| `v0.7.0` | 🔴 PKGBUILD ShellCheck/namcap |
| `v0.7.1` | 🟠 Button tooltips |
| `v0.7.2` | 🟠 Distro news (EOS, Manjaro, etc.) |
| `v0.7.3` | 🟠 Switch pane locations |
| `v0.8.0` | 🟡 Accessibility themes |
| `v0.8.1` | 🟡 Mirror search UI |
| `v0.8.2` | 🟡 Update grouping by criticality |
| `v0.9.0` | 🟠 AUR SSH voting |
| `v0.9.1` | 🟡 Package-based news/comments |
| `v1.0.0` | 🎉 **Stable Release** |
| `v1.1.0` | 🟢 Chaotic AUR / Garuda repos |
| `v1.2.0` | 🟢 Custom repository support |
| `v1.3.0` | 🟢 Dependency conflict resolution |
| `v1.4.0` | 🟢 AUR maintenance tools |
| `v2.0.0` | 🔵 Multi-PM (apt, dnf, Flatpak) |

---

## Priority Tiers Overview

| Tier | Description | Timeline Suggestion |
|------|-------------|---------------------|
| 🔴 **Tier 1** | High impact, reasonable complexity, core UX improvements | Next 1-2 releases |
| 🟠 **Tier 2** | Good value, moderate complexity, extends existing systems | Next 3-4 releases |
| 🟡 **Tier 3** | Valuable for specific use cases, medium effort | Roadmap items |
| 🟢 **Tier 4** | Niche or higher complexity, community-driven | Future consideration |
| 🔵 **Tier 5** | Major architectural changes, long-term vision | Future major version |

---

## 🔴 Tier 1 - High Priority

### 1. Render Actions Directly in TUI (Instead of Spawning Terminals) ✅ **COMPLETED**
**Target Version: `v0.6.0`** | **Status: ✅ Completed** | **Community Request** | **Impact: ⭐⭐⭐⭐⭐** | **Complexity: High**

**What:** Instead of spawning external terminals for install, removal, update, scans, downgrade, and config operations, render the output directly within the TUI.

**Implementation Summary:**
- ✅ PTY-based command execution with live output streaming implemented
- ✅ All operations (install, remove, update, scan, downgrade, file sync, optional deps) now use integrated executor pattern
- ✅ Real-time progress display with auto-scrolling log panel
- ✅ Password prompt modal for sudo authentication
- ✅ Security enhancements: password validation, faillock lockout detection
- ✅ Comprehensive test suite covering all workflows

**Why Priority #1:**
- **Biggest UX friction point** - Spawning external terminals breaks user flow, loses context, and feels disconnected from the TUI experience
- **Architecture already supports this** - Ratatui handles real-time rendering; `crossterm` supports raw mode I/O
- **Reduces external dependencies** - No longer needs to detect/configure terminal emulators (alacritty, kitty, gnome-terminal, etc.)
- **Enables future features** - Once output is internal, you can add progress bars, cancellation, log viewing, etc.

**Implementation Details:**
- PTY executor worker (`src/app/runtime/workers/executor.rs`) streams command output in real-time
- PreflightExec modal displays live output with progress bar support
- Password prompt modal handles sudo authentication with validation
- All operations integrated: install/remove, updates, scans, downgrades, file sync, optional deps
- Windows compatibility: conditional compilation for PTY-dependent functionality

**Key Files:** `src/app/runtime/workers/executor.rs`, `src/install/executor.rs`, `src/ui/modals/preflight_exec.rs`, `src/ui/modals/password.rs`

---

### 2. Adjustable Height of Results/Package Info/Search Panes
**Target Version: `v0.6.1`** | **Community Request** | **Impact: ⭐⭐⭐⭐** | **Complexity: Medium**

**What:** Allow users to resize the vertical split between the top (search input), middle (results list), and bottom (package info/PKGBUILD) sections.

**Why Priority #2:**
- **Directly requested by community** - Users have different screen sizes and preferences
- **Layout system already exists** - `settings.conf` has `layout_left_pct`, `layout_center_pct`, `layout_right_pct` for horizontal splits
- **Minimal architectural changes** - Extend existing percentage-based layout to vertical axis
- **Keyboard-first design** - Add keybinds like `Ctrl+Up/Down` to resize

**Implementation Notes:**
- Add `layout_top_pct`, `layout_middle_pct`, `layout_bottom_pct` to settings
- Implement resize keybinds (e.g., `Ctrl+Shift+J/K` or similar)
- Consider mouse drag support for vertical dividers
- Save preferences persistently

**Files to Modify:** `config/settings.conf`, `src/theme/mod.rs` (settings), `src/ui.rs` (layout calculation)

---

### 3. CLI Flags `-u`/`--update` Respect TUI Settings
**Target Version: `v0.6.2`** | **Community Request** | **Impact: ⭐⭐⭐⭐** | **Complexity: Low**

**What:** When running `pacsea -u` or `pacsea --update`, use the mirror settings, preferred AUR helper, and other update-related configurations from `settings.conf`.

**Why Priority #3:**
- **Already partially implemented** - `--update` flag exists in `src/args/args.rs` and `src/args/update.rs`
- **Settings infrastructure exists** - `crate::theme::settings()` loads all config values
- **Quick win** - Low effort, high consistency value
- **Enables scripting** - Users can automate updates that behave like TUI

**Implementation Notes:**
- In `src/args/update.rs`, load `selected_countries`, `mirror_count`, and AUR helper preference
- Pass these to the update command builder
- Consider adding `--use-settings` flag as explicit opt-in (or make it default)

**Files to Modify:** `src/args/update.rs`, potentially `src/logic/distro.rs`

---

### 4. PKGBUILD Preview with ShellCheck and Namcap Integration
**Target Version: `v0.7.0`** | **Community Request** | **Impact: ⭐⭐⭐⭐** | **Complexity: Medium**

**What:** In the PKGBUILD preview pane, show inline linting warnings from ShellCheck and namcap for AUR packages.

**Why Priority #4:**
- **Security-first alignment** - Core project philosophy; helps users spot issues before installing
- **Existing infrastructure** - ShellCheck already integrated in security scans (`scan_do_shellcheck`)
- **PKGBUILD viewer exists** - `src/sources/pkgbuild.rs` and `src/ui/details/` already render PKGBUILDs
- **Natural extension** - Syntect already does highlighting; add diagnostic overlays

**Implementation Notes:**
- Run ShellCheck on PKGBUILD content asynchronously
- Parse namcap output for packaging issues
- Display warnings as annotations (gutter icons or inline highlights)
- Cache results per PKGBUILD version

**Files to Modify:** `src/sources/pkgbuild.rs`, `src/ui/details/`, new linting module

---

## 🟠 Tier 2 - Medium Priority

### 5. Tooltip/Hover Hints for Buttons and Actions
**Target Version: `v0.7.1`** | **Community Request** | **Impact: ⭐⭐⭐** | **Complexity: Medium**

**What:** Show contextual help when hovering over buttons or focusing on interactive elements.

**Why This Tier:**
- Improves discoverability for new users
- TUI tooltip systems are non-trivial (need timer-based popup, positioning)
- Help overlay (`?` key) already provides keybind reference
- Lower urgency since experienced users rely on muscle memory

**Implementation Notes:**
- Create a tooltip component that appears after 500ms hover/focus
- Position near cursor/focused element
- Pull descriptions from i18n system for translations

---

### 6. Vote for AUR Packages via SSH Connection
**Target Version: `v0.9.0`** | **Community Request** | **Impact: ⭐⭐⭐** | **Complexity: Medium-High**

**What:** Allow users to vote for AUR packages directly from Pacsea using their AUR SSH key.

**Why This Tier:**
- Valuable for AUR contributors and power users
- Requires secure SSH key handling and AUR API knowledge
- Niche use case (not all users have AUR accounts)
- Could attract AUR maintainers to the project

**Implementation Notes:**
- Detect existing `~/.ssh/aur` or configurable key path
- Use `ssh aur@aur.archlinux.org vote <pkgname>` command
- Add vote status to package info display
- Handle authentication errors gracefully

---

### 7. Distro-Specific News Support
**Target Version: `v0.7.2`** | **Community Request** | **Impact: ⭐⭐⭐** | **Complexity: Low-Medium**

**What:** View news for EndeavourOS, Manjaro, Garuda, and CachyOS in addition to Arch Linux.

**Why This Tier:**
- **News infrastructure exists** - `src/sources/news.rs` parses Arch RSS
- Multiple distros already supported for updates
- Each distro has different RSS feed formats
- Good community engagement feature

**Implementation Notes:**
- Add RSS URLs for each distro (EOS, Manjaro, Garuda, CachyOS)
- Detect current distro from `/etc/os-release` (already done in `src/logic/distro.rs`)
- Allow switching news source or showing combined feed
- Handle different date formats per source

---

### 8. Switch Pane Locations (Top/Center/Bottom)
**Target Version: `v0.7.3`** | **Community Request** | **Impact: ⭐⭐⭐** | **Complexity: Medium**

**What:** Allow users to rearrange the three main panes (Recent, Search/Results, Install List) to different positions.

**Why This Tier:**
- Layout configuration exists
- Requires decoupling pane rendering from position
- Users have different workflow preferences
- Medium effort for personalization benefit

**Implementation Notes:**
- Add `pane_order` setting (e.g., `"recent,results,install"` or `"install,results,recent"`)
- Refactor UI rendering to use positional mapping
- Add keybind or settings toggle for swapping

---

## 🟡 Tier 3 - Roadmap Items

### 9. Accessibility Themes for Visual Impairments
**Target Version: `v0.8.0`** | **Impact: ⭐⭐⭐** | **Complexity: Medium**

**What:** High-contrast themes, screen reader hints, configurable font scaling indicators.

**Why This Tier:**
- Important for inclusivity
- Theme system exists (`theme.conf`)
- Requires research into terminal accessibility best practices
- May need testing with actual users

**Implementation Notes:**
- Create `theme-high-contrast.conf` with WCAG-compliant colors
- Consider ASCII alternatives to Unicode symbols
- Test with screen readers (if terminal supports it)

---

### 10. News Based on Installed Packages (Including AUR Comments)
**Target Version: `v0.9.1`** | **Community Request** | **Impact: ⭐⭐⭐** | **Complexity: Medium-High**

**What:** Watch for news, updates, and new AUR comments for your installed packages.

**Why This Tier:**
- AUR comments viewer already exists (`src/sources/comments.rs`)
- Would require background monitoring/notifications
- Complex: tracking state for many packages
- High value for security-conscious users

**Implementation Notes:**
- Store "last seen" comment timestamps per package
- Background task to check for new comments
- Highlight packages with new activity
- Consider RSS/Atom feeds where available

---

### 11. Mirror Search and Extensive Mirror Selection UI
**Target Version: `v0.8.1`** | **Community Request** | **Impact: ⭐⭐⭐** | **Complexity: Medium**

**What:** Interactive mirror browser with search, filtering by country/speed, and detailed mirror stats.

**Why This Tier:**
- Mirror infrastructure exists (`src/index/mirrors.rs`, `repository/mirrors.json`)
- Country selection already in settings
- UI enhancement on top of existing data
- Useful but not critical for most users

**Implementation Notes:**
- Create mirror browser modal
- Show speed test results, last sync time, protocols
- Allow multi-select with drag/rank

---

### 12. Update Grouping by Criticality
**Target Version: `v0.8.2`** | **Community Request (Partially Done)** | **Impact: ⭐⭐⭐** | **Complexity: Medium**

**What:** In the update preview, group packages by system criticality: kernel, systemd, core packages that need restart vs. regular packages.

**Why This Tier:**
- Update preview already exists
- Requires package classification logic
- Helps users make informed decisions about timing updates
- Aligns with security-first philosophy

**Implementation Notes:**
- Define critical package list (linux, systemd, glibc, etc.)
- Group and sort in update modal
- Add visual indicators (colors, icons)
- Consider reboot recommendation

---

## 🟢 Tier 4 - Future Consideration

### 13. Chaotic AUR and Garuda Repository Support
**Target Version: `v1.1.0`** | **Impact: ⭐⭐** | **Complexity: Medium**

**What:** Add Chaotic-AUR and Garuda repos as package sources.

**Why This Tier:**
- Benefits specific user segment
- Requires repository metadata parsing
- Similar architecture to existing repo support
- Lower priority than core UX improvements

---

### 14. Custom Repository Support
**Target Version: `v1.2.0`** | **Impact: ⭐⭐** | **Complexity: Medium-High**

**What:** Allow users to add custom repos (CachyOS, Manjaro, EOS repos on other Arch systems).

**Why This Tier:**
- Power user feature
- Requires repo configuration UI
- Potential for misconfiguration issues
- Would need careful safety checks

---

### 15. Dependency Conflict Resolution
**Target Version: `v1.3.0`** | **Impact: ⭐⭐⭐** | **Complexity: Very High**

**What:** Help users resolve dependency conflicts interactively.

**Why This Tier:**
- Very complex problem (pacman doesn't expose conflict resolution API easily)
- Would need to parse pacman error output
- Risky if implemented incorrectly
- Most users can handle conflicts manually

---

### 16. AUR Package Maintenance Features
**Target Version: `v1.4.0`** | **Impact: ⭐⭐** | **Complexity: High**

**What:** Tools for AUR maintainers: update PKGBUILDs, push changes, manage co-maintainers.

**Why This Tier:**
- Very niche audience
- Requires full AUR API integration
- Separate tool concern (aurpublish, etc.)
- Low overlap with primary use case (package consumption)

---

### 17. Custom Upgrade Commands
**Target Version: `v1.1.1`** | **Impact: ⭐⭐** | **Complexity: Low**

**What:** Allow users to define custom pre/post upgrade hooks or alternative upgrade commands.

**Why This Tier:**
- Nice to have for advanced users
- Settings system could support this
- Limited demand
- Potential security concerns with arbitrary commands

---

## 🔵 Tier 5 - Future Major Version

### 18. Multi Package Manager Support (apt, dnf, Flatpak)
**Target Version: `v2.0.0`** | **Impact: ⭐⭐⭐⭐⭐** | **Complexity: Extremely High**

**What:** Support Debian-based (apt), Fedora-based (dnf), and Flatpak package managers.

**Why Tier 5:**
- **Major architectural overhaul** - Current codebase is deeply Arch-specific (pacman, AUR, PKGBUILD parsing)
- **Essentially a new project** - Would need to abstract all package operations behind traits
- **Different ecosystems** - Each has unique metadata formats, repositories, and workflows
- **Flatpak is most feasible** - As an addition rather than replacement (can coexist with pacman)

**If Pursued:**
- Start with Flatpak support (runs alongside pacman)
- Create `PackageManager` trait with implementations per system
- Consider separate binaries or feature flags
- Could be a "Pacsea 2.0" rewrite goal

---

## Summary Matrix

| Feature | Version | Impact | Complexity | Dependencies | Tier |
|---------|---------|--------|------------|--------------|------|
| Render in TUI (no terminal spawn) | `v0.6.0` ✅ | ⭐⭐⭐⭐⭐ | High | None | 🔴 1 |
| Adjustable pane heights | `v0.6.1` | ⭐⭐⭐⭐ | Medium | None | 🔴 1 |
| CLI update respects settings | `v0.6.2` | ⭐⭐⭐⭐ | Low | None | 🔴 1 |
| PKGBUILD ShellCheck/namcap | `v0.7.0` | ⭐⭐⭐⭐ | Medium | ShellCheck, namcap | 🔴 1 |
| Button tooltips | `v0.7.1` | ⭐⭐⭐ | Medium | None | 🟠 2 |
| Distro news | `v0.7.2` | ⭐⭐⭐ | Low-Medium | RSS feeds | 🟠 2 |
| Switch pane locations | `v0.7.3` | ⭐⭐⭐ | Medium | None | 🟠 2 |
| Accessibility themes | `v0.8.0` | ⭐⭐⭐ | Medium | None | 🟡 3 |
| Mirror search UI | `v0.8.1` | ⭐⭐⭐ | Medium | None | 🟡 3 |
| Update criticality grouping | `v0.8.2` | ⭐⭐⭐ | Medium | None | 🟡 3 |
| AUR SSH voting | `v0.9.0` | ⭐⭐⭐ | Medium-High | SSH key, AUR account | 🟠 2 |
| Package-based news | `v0.9.1` | ⭐⭐⭐ | Medium-High | None | 🟡 3 |
| **v1.0.0 Release** | `v1.0.0` | — | — | Stability & polish | — |
| Chaotic AUR/Garuda | `v1.1.0` | ⭐⭐ | Medium | External repos | 🟢 4 |
| Custom upgrade commands | `v1.1.1` | ⭐⭐ | Low | None | 🟢 4 |
| Custom repository support | `v1.2.0` | ⭐⭐ | Medium-High | None | 🟢 4 |
| Dependency conflict resolution | `v1.3.0` | ⭐⭐⭐ | Very High | pacman internals | 🟢 4 |
| AUR maintenance tools | `v1.4.0` | ⭐⭐ | High | AUR API | 🟢 4 |
| Multi-PM (apt, dnf, Flatpak) | `v2.0.0` | ⭐⭐⭐⭐⭐ | Extremely High | Complete redesign | 🔵 5 |

---

## Recommended Development Path

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  v0.6.0 (current) ✅                                                        │
│  ✅ Render actions in TUI (PTY-based executor with live output streaming)  │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  v0.6.x Series - Core UX Improvements                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│  v0.6.0  │ ✅ Render actions in TUI (biggest UX win) - COMPLETED           │
│  v0.6.1  │ 🔴 Adjustable pane heights                                      │
│  v0.6.2  │ 🔴 CLI --update respects TUI settings                            │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  v0.7.x Series - Security & Discoverability                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│  v0.7.0  │ PKGBUILD ShellCheck/namcap integration                           │
│  v0.7.1  │ Button tooltips/hover hints                                      │
│  v0.7.2  │ Distro-specific news (EOS, Manjaro, Garuda, CachyOS)             │
│  v0.7.3  │ Switch pane locations                                            │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  v0.8.x Series - Accessibility & Mirror Improvements                        │
├─────────────────────────────────────────────────────────────────────────────┤
│  v0.8.0  │ Accessibility themes (high contrast, etc.)                       │
│  v0.8.1  │ Mirror search and selection UI                                   │
│  v0.8.2  │ Update grouping by criticality (kernel, systemd, etc.)           │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  v0.9.x Series - AUR Power User Features                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│  v0.9.0  │ Vote for AUR packages via SSH                                    │
│  v0.9.1  │ Package-based news/comments watching                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  v1.0.0 - Stable Release                                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│  Focus: Polish, stability, documentation, community feedback                │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  v1.x Series - Extended Repository Support                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│  v1.1.0  │ Chaotic AUR / Garuda repository support                          │
│  v1.1.1  │ Custom upgrade commands                                          │
│  v1.2.0  │ Custom repository support                                        │
│  v1.3.0  │ Dependency conflict resolution                                   │
│  v1.4.0  │ AUR package maintenance tools                                    │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  v2.0.0 - Multi Package Manager Support                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│  Major rewrite: apt, dnf, Flatpak support                                   │
│  Consider: PackageManager trait abstraction                                 │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

*Last updated: 2025-12-03*
*Based on Pacsea v0.6.0 codebase analysis*

