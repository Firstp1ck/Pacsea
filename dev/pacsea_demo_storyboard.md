# Pacsea — Demo Video Storyboard

**Total runtime:** ~3:34  
**Format:** Screen recording (OBS for MP4 / asciinema for lightweight cast)  
**Terminal setup:** 180+ cols · 14–16pt font · Alacritty or Kitty  
**Pre-recording:** Take a btrfs/VM snapshot so you can restore state between takes.

---

## Pre-recording Checklist

- [ ] Terminal width ≥ 180 columns, font 14–16pt
- [ ] `pacsea` installed and up to date
- [ ] BlackArch repo enabled on demo machine (for Ch.1 Scene 6)
- [ ] At least one AUR package with [OOD] status available in search
- [ ] `downgrade` tool installed
- [ ] SSH configured for AUR voting
- [ ] Real pending package updates on the machine (5+ looks good)
- [ ] Unread news items + an unread startup announcement queued
- [ ] Security scan tools installed: ClamAV, Trivy, Semgrep, ShellCheck
- [ ] VirusTotal API key configured in Optional Deps
- [ ] AUR package pre-selected for security scan (clean package, fast scan)
- [ ] `settings.conf` with default values ready to edit live during Ch.7
- [ ] Fingerprint/PAM configured if showing `auth_mode = interactive` (optional but impressive)
- [ ] Editor (zed / nvim) ready to open `~/.config/pacsea/settings.conf` for Ch.7
- [ ] Snapshot/checkpoint taken — restore between bad takes

---

## Chapter 1 — Search & Discovery
> Unified search · Fuzzy mode · BlackArch · AUR status markings

---

### Scene 1 · `0:00` · ~5s — Cold open

| | |
|---|---|
| **On screen** | Launch `pacsea`. 3-pane layout appears. Let it sit 2 seconds. No input yet. |
| **Narration** | *(none)* — Subtitle only: `pacsea — package management TUI for Arch` |
| **Keys** | `pacsea ↵` |
| **Tip** | No talking, no typing. Just the UI breathing. Strong first impression. |

---

### Scene 2 · `0:05` · ~8s — Unified search

| | |
|---|---|
| **On screen** | Type a package name live. Results appear instantly from both official repos and AUR. Point at the repo badges: `core` / `extra` / `AUR`. |
| **Narration** | *"Search official repos and the AUR simultaneously — no switching tools."* |
| **Keys** | Type `btop` (or any package present in both official AND AUR) |
| **Tip** | Pick a package that exists in BOTH official AND AUR so the unified column is clearly visible. |

---

### Scene 3 · `0:13` · ~5s — Always-visible details pane

| | |
|---|---|
| **On screen** | Arrow down through results. Right pane updates live — version, description, deps, links. |
| **Narration** | *"Package details always visible — no extra commands needed."* |
| **Keys** | `↓ ↓ ↓` |
| **Tip** | Pick a dep-heavy package so the detail pane looks rich. |

---

### Scene 4 · `0:18` · ~6s — Fuzzy search toggle

| | |
|---|---|
| **On screen** | Toggle fuzzy mode on with `Ctrl+F`. Type a partial/misspelled query — correct package still found. Toggle off. |
| **Narration** | *"Fuzzy mode — find packages even without the exact name."* |
| **Keys** | `Ctrl+F` → type partial name |
| **Tip** | Use a deliberate typo to make the difference obvious. E.g. search `btopp` — show it still finds `btop`. |

---

### Scene 5 · `0:24` · ~5s — AUR status markings

| | |
|---|---|
| **On screen** | Navigate to an AUR package marked `[OOD]` or `[ORPHAN]`. Let the badge sit visible for 2–3 seconds. |
| **Narration** | *"Out-of-date and orphaned packages are clearly marked in results."* |
| **Keys** | Search for a known OOD package |
| **Tip** | Pre-check the AUR before recording — find a reliably OOD package name to type. |

---

### Scene 6 · `0:29` · ~5s — BlackArch repo support

| | |
|---|---|
| **On screen** | Show a BlackArch package in results with the BlackArch label. Toggle the title-bar filter chip on and off. |
| **Narration** | *"If you have BlackArch enabled, those packages appear right alongside everything else."* |
| **Keys** | Filter chip toggle |
| **Tip** | Only include this scene if BlackArch is actually enabled on the demo machine. Skip if not. |

---

## Chapter 2 — Package Inspection
> PKGBUILD preview · AUR Comments · Persistent lists

---

### Scene 7 · `0:34` · ~7s — PKGBUILD preview

| | |
|---|---|
| **On screen** | Navigate to an AUR package. Toggle PKGBUILD viewer open — script is visible. Hit copy. Close viewer. |
| **Narration** | *"Preview and copy PKGBUILDs before anything builds — one keypress."* |
| **Keys** | `Ctrl+P` → copy → `Ctrl+P` |
| **Tip** | Pick a PKGBUILD with patches or multiple sources so it looks interesting on screen. |

---

### Scene 8 · `0:41` · ~7s — AUR Comments viewer

| | |
|---|---|
| **On screen** | On a popular AUR package, open comments pane with `Ctrl+T`. Scroll through — show markdown rendering and a clickable URL. |
| **Narration** | *"Community comments for any AUR package — live, with markdown and links."* |
| **Keys** | `Ctrl+T` → scroll |
| **Tip** | Pick a popular AUR package with recent active comments. The pane should look alive, not empty. |

---

### Scene 9 · `0:48` · ~4s — Persistent recent searches

| | |
|---|---|
| **On screen** | Open the recent searches list — show several entries from this session. Brief flash. |
| **Narration** | *"Recent searches and your install list survive across sessions."* |
| **Keys** | Open recents panel |
| **Tip** | Keep this short — 3–4 seconds max. Supporting feature, not a headline. |

---

## Chapter 3 — Queue, Install & Process Execution
> Queue · Preflight modal · Integrated execution · sudo/doas/fprintd

---

### Scene 10 · `0:52` · ~6s — Queue multiple packages

| | |
|---|---|
| **On screen** | `Space` to queue 3 packages — at least one official, one AUR. Queue counter increments visibly. |
| **Narration** | *"Queue packages from official repos and AUR in one unified list."* |
| **Keys** | `Space` × 3 |
| **Tip** | The growing queue counter is the visual payoff. Make sure it's clearly visible. |

---

### Scene 11 · `0:58` · ~10s — Preflight modal — deps & review

| | |
|---|---|
| **On screen** | Open install list → Preflight. Tab through: deps list, files to be added, warnings. Show AUR badge styling on AUR entries. |
| **Narration** | *"Review every dependency, file change, and conflict before your system is touched."* |
| **Keys** | `↵` → tab through Preflight |
| **Tip** | Queue a package with at least 3–4 deps so the deps tab looks meaningful. |

---

### Scene 12 · `1:08` · ~10s — Integrated process execution

| | |
|---|---|
| **On screen** | Confirm install from Preflight. Real-time output streams inside the TUI — progress bars, scrolling log, password prompt inline. No external terminal window. |
| **Narration** | *"Everything runs inside the TUI — real-time output, progress bars, password prompts. No external terminal window."* |
| **Keys** | Confirm → watch it run |
| **Tip** | Strong visual moment — let the output scroll visibly. Use a small real package so it's fast but genuine. Fingerprint/PAM prompt is a bonus if available. |

---

### Scene 13 · `1:18` · ~5s — Keyboard-first navigation

| | |
|---|---|
| **On screen** | Quick montage: `j`/`k` scrolling, `g`/`G` top/bottom, numpad `↵` confirming a prompt. |
| **Narration** | *"Keyboard-first throughout — vim motions, numpad Enter supported."* |
| **Keys** | `j k g G` numpad `↵` |
| **Tip** | Fast montage — 3 seconds of fluid navigation is enough. Don't linger. |

---

## Chapter 4 — Security
> AUR scanning (ClamAV / Trivy / Semgrep / ShellCheck / VirusTotal / aur-sleuth) · AUR voting

---

### Scene 14 · `1:23` · ~8s — AUR security scan — config

| | |
|---|---|
| **On screen** | From Preflight on an AUR package, open the security scan config. Show all tools listed: ClamAV, Trivy, Semgrep, ShellCheck, VirusTotal, custom patterns, aur-sleuth. |
| **Narration** | *"Before building any AUR package, run a full security suite — antivirus, static analysis, PKGBUILD linting, and VirusTotal hash lookups."* |
| **Keys** | Security scan → config modal |
| **Tip** | **This is your biggest differentiator.** Let every tool name sit readable for at least 2 seconds. Don't rush. |

---

### Scene 15 · `1:31` · ~12s — AUR security scan — run & summary

| | |
|---|---|
| **On screen** | Run the scan. Per-tool progress visible in real time. Final summary: severity breakdown, Semgrep findings count, VirusTotal stats. |
| **Narration** | *"Detailed summary — severity breakdown, Semgrep hits, VirusTotal statistics."* |
| **Keys** | Run → wait → summary |
| **Tip** | Pre-scan offline so it runs fast on camera. A clean result (no findings) is fine — the workflow is the point. |

---

### Scene 16 · `1:43` · ~8s — AUR package voting via SSH

| | |
|---|---|
| **On screen** | From search results, vote on an AUR package. SSH workflow runs — connection check passes. Vote confirmed. |
| **Narration** | *"Vote and unvote AUR packages directly from search results — SSH-based with guided setup."* |
| **Keys** | Vote keybind on an AUR package |
| **Tip** | Make sure SSH is pre-configured so the guided setup wizard doesn't appear unexpectedly. |

---

## Chapter 5 — System Management
> Updates · Installed-only mode · Downgrade · Distro-aware tools

---

### Scene 17 · `1:51` · ~8s — Updates modal

| | |
|---|---|
| **On screen** | Show "Updates available" banner. Click it — version comparison list for official + AUR. Open Preflight from there. |
| **Narration** | *"Background update checks with per-package version diff — and Preflight before applying anything."* |
| **Keys** | Updates button → list → Preflight |
| **Tip** | Have 5+ real pending updates. The list looks convincing when it's populated. |

---

### Scene 18 · `1:59` · ~6s — Installed-only mode

| | |
|---|---|
| **On screen** | Switch to installed-only mode. Show default leaf-package filter. Switch to all-explicitly-installed mode. |
| **Narration** | *"Review what's installed — filter to leaf packages or everything explicitly installed."* |
| **Keys** | Mode toggle |
| **Tip** | Natural segue into downgrade — stay on an installed package at the end of this scene. |

---

### Scene 19 · `2:05` · ~6s — Package downgrade

| | |
|---|---|
| **On screen** | Select an installed package. Trigger the downgrade workflow. Show previous version list from the `downgrade` tool. |
| **Narration** | *"Downgrade any installed package to a previous version — powered by the downgrade tool."* |
| **Keys** | Downgrade keybind |
| **Tip** | Only include if `downgrade` is installed on the demo machine. Skip and adjust timestamps if not. |

---

### Scene 20 · `2:11` · ~7s — Distro-aware updates & helpful tools

| | |
|---|---|
| **On screen** | Open system update dialog. Show Force Sync (`-Syyu`). Show mirror management — correct tool auto-detected (reflector / eos-rankmirrors / cachyos-rate-mirrors). Show AUR update fallback prompt. |
| **Narration** | *"Distro-aware: Arch, EndeavourOS, Manjaro, CachyOS, Artix — the right mirror tool, auto-detected."* |
| **Keys** | System update dialog |
| **Tip** | Mention distro names visually — signals broad compatibility to viewers. |

---

## Chapter 6 — News & Announcements
> News feed · Advisories · Bookmarks · Startup announcements

---

### Scene 21 · `2:18` · ~8s — News feed — unified view

| | |
|---|---|
| **On screen** | Switch to news mode. Feed shows: Arch news, security advisories, package update notifications, AUR comments. Filter by source. |
| **Narration** | *"Arch news, security advisories, and AUR comments — one feed, cached offline, updated in the background."* |
| **Keys** | Options → News (or direct bind) |
| **Tip** | Make sure you have unread items. Show at least one advisory and one AUR comment entry. |

---

### Scene 22 · `2:26` · ~6s — Bookmarks, read/unread, search

| | |
|---|---|
| **On screen** | Bookmark an advisory. Mark another as read. Switch to search in news mode — type a query, show search history dropdown. |
| **Narration** | *"Bookmark advisories, track read/unread status, search with history."* |
| **Keys** | Bookmark key → mark read → search |
| **Tip** | Quick scene — 4–5 seconds is enough. |

---

### Scene 23 · `2:32` · ~4s — Startup announcements

| | |
|---|---|
| **On screen** | Relaunch pacsea — version-specific announcement appears at startup with a clickable URL. Dismiss it. |
| **Narration** | *"Version announcements at startup — with links and persistent read state."* |
| **Keys** | Relaunch `pacsea` |
| **Tip** | Make sure there's a real unread announcement queued before recording this scene. |

---

## Chapter 7 — Configuration & Setup
> Layout · Search behavior · Privilege escalation · Scan toggles · Mirrors · News filters · AUR voting

**Recording setup for this chapter:** Open `~/.config/pacsea/settings.conf` in a split pane alongside the running app. Edit a value, save, and show the live effect. The cause-and-effect is what makes config demos compelling.

---

### Scene 24 · `2:36` · ~8s — Layout & UI customization

| | |
|---|---|
| **On screen** | Show `settings.conf`. Point at the layout percentage values. Change `layout_center_pct` from `60` to `80` (reduce others) — panes resize live. Then toggle `show_search_history_pane = false` — pane disappears. Toggle `show_keybinds_footer = false`. Restore defaults. |
| **Narration** | *"The three-pane layout is fully adjustable — resize or hide any pane."* |
| **Config keys** | `layout_left_pct`, `layout_center_pct`, `layout_right_pct`, `show_search_history_pane`, `show_install_pane`, `show_keybinds_footer` |
| **Tip** | Make the resize dramatic — e.g. 20/60/20 → 10/80/10 — so it's obvious on screen. |

---

### Scene 25 · `2:44` · ~8s — Search & sort behavior

| | |
|---|---|
| **On screen** | Show `sort_mode = best_matches`. Switch to `aur_popularity` — results visibly reorder. Show `search_startup_mode` options (`insert_mode` / `normal_mode`). Show `installed_packages_mode = leaf` vs `all`. |
| **Narration** | *"Sort by best match, AUR popularity, or alphabetical. Control startup input mode and installed-package filter."* |
| **Config keys** | `sort_mode`, `search_startup_mode`, `fuzzy_search`, `installed_packages_mode` |
| **Tip** | The sort mode reorder is the most visual moment — do it first. Keep the rest as brief text callouts. |

---

### Scene 26 · `2:52` · ~12s — Privilege escalation & authentication

| | |
|---|---|
| **On screen** | Show `privilege_tool = auto` — explain it prefers doas, falls back to sudo. Show switching to `sudo` explicitly. Then show `auth_mode`: explain `prompt` (default, pipes password via `-S`), `passwordless_only` (skips prompt when allowed), and `interactive` (lets the privilege tool handle auth — enables PAM fingerprint via fprintd). Show `use_passwordless_sudo`. Point at `skip_preflight` and flag it as the safety bypass. |
| **Narration** | *"Choose sudo or doas, configure passwordless mode, or use interactive auth for fingerprint via PAM — including fprintd support."* |
| **Config keys** | `privilege_tool`, `auth_mode`, `use_passwordless_sudo`, `skip_preflight` |
| **Tip** | **Pause on `auth_mode = interactive` and call out fingerprint support explicitly** — it's an unexpected feature. Warn clearly that `skip_preflight = true` removes the safety confirmation modal. |

---

### Scene 27 · `3:04` · ~8s — Scan configuration

| | |
|---|---|
| **On screen** | Show all `scan_do_*` toggles — ClamAV, Trivy, Semgrep, ShellCheck, VirusTotal, custom patterns, aur-sleuth — all `true`. Pan the list slowly. Show `virustotal_api_key` field. Show `pkgbuild_shellcheck_exclude = SC2034,SC2164,SC2148,SC2154` — briefly explain these suppress known PKGBUILD false positives. |
| **Narration** | *"Every scan tool toggleable individually. Add your VirusTotal API key once — pacsea handles the rest. ShellCheck exclusions prevent false positives on standard PKGBUILD patterns."* |
| **Config keys** | `scan_do_clamav`, `scan_do_trivy`, `scan_do_semgrep`, `scan_do_shellcheck`, `scan_do_virustotal`, `scan_do_custom`, `scan_do_sleuth`, `virustotal_api_key`, `pkgbuild_shellcheck_exclude` |
| **Tip** | Don't read every key aloud — pan the list slowly so viewers can read it themselves. Pause on `virustotal_api_key`. |

---

### Scene 28 · `3:12` · ~8s — Mirrors, news filters & locale

| | |
|---|---|
| **On screen** | Show `selected_countries = Worldwide`. Change to `Switzerland, Germany, Austria` live — explain it scopes mirror selection to those countries. Show `mirror_count = 20`. Then show `news_max_age_days = 30` and `news_filter_installed_only = false`. Point at `locale` — mention `en-US`, `de-DE`, `hu-HU` are available. Show `updates_refresh_interval = 30`. |
| **Narration** | *"Scope mirrors to your region, filter news to packages you actually have installed, set your language, and tune the update check interval."* |
| **Config keys** | `selected_countries`, `mirror_count`, `news_max_age_days`, `news_filter_installed_only`, `locale`, `updates_refresh_interval` |
| **Tip** | Change `selected_countries` to something real during recording — it makes the setting feel concrete rather than theoretical. |

---

### Scene 29 · `3:20` · ~6s — AUR voting & terminal settings

| | |
|---|---|
| **On screen** | Show `aur_vote_enabled = true`. Show `aur_vote_ssh_command = ssh` — mention it can be overridden for non-standard setups. Show `preferred_terminal` field — explain pacsea uses this when it needs an external terminal. Toggle `use_terminal_theme = true` and show the UI adopt terminal colors live. |
| **Narration** | *"Configure AUR voting SSH settings, your preferred terminal, and whether to inherit your terminal's color theme."* |
| **Config keys** | `aur_vote_enabled`, `aur_vote_ssh_command`, `aur_vote_ssh_timeout_seconds`, `preferred_terminal`, `use_terminal_theme` |
| **Tip** | The `use_terminal_theme` toggle is very visual — do it last as a satisfying visual payoff for the chapter. |

---

## Outro

---

### Scene 30 · `3:26` · ~8s — Outro

| | |
|---|---|
| **On screen** | Cut to clean terminal prompt. Show install commands. Hold on GitHub URL for 4+ seconds. |
| **Narration** | `paru -S pacsea-bin`  ·  `github.com/Firstp1ck/Pacsea` |
| **Keys** | *(none — static frame)* |
| **Tip** | End on a static frame people can pause and screenshot. No fade music. The URL is the call to action. |

---

## Chapter Timestamps Summary

| # | Chapter | Start | Duration | Content |
|---|---|---|---|---|
| 1 | Search & Discovery | `0:00` | ~34s | Cold open, unified search, details pane, fuzzy, OOD markings, BlackArch |
| 2 | Package Inspection | `0:34` | ~18s | PKGBUILD preview, AUR comments, recent searches |
| 3 | Queue & Process | `0:52` | ~31s | Queue, Preflight, integrated execution, keyboard nav |
| 4 | Security | `1:23` | ~28s | Scan config, scan run + summary, AUR voting |
| 5 | System Management | `1:51` | ~27s | Updates modal, installed-only, downgrade, distro-aware tools |
| 6 | News & Announcements | `2:18` | ~18s | News feed, bookmarks/search, startup announcements |
| 7 | Configuration & Setup | `2:36` | ~50s | Layout, search/sort, privilege/auth, scan toggles, mirrors/news/locale, AUR voting/theme |
| — | Outro | `3:26` | ~8s | Install commands + GitHub URL |

**Total: ~3:34**

---

## Recording Tips

- **One chapter per take.** Don't attempt the full video in one run.
- **Restore your snapshot between chapters** so state is always clean and predictable.
- **Hide your mouse cursor** during recording — it's a keyboard-first tool, keep it keyboard-first visually.
- **Pacing:** pause 0.5s after every key action before moving on. Confident, not rushed.
- **Chapter 7 setup:** have `settings.conf` open in a split pane. Edit a value, save (`Ctrl+S`), switch to pacsea, show the effect. The live cause-and-effect is what makes config demos land.
- **Editing:** cut between chapters freely. Use the chapter names as title cards.
- **YouTube chapters:** paste the timestamp summary table into the video description — YouTube auto-generates chapter markers from `0:00 Title` format lines.
- **Short clips for Reddit/Mastodon:**
  - Scenes 14–15 (security scan) → 20s GIF, best standalone clip
  - Scene 26 (fingerprint auth) → surprising feature, good second clip
  - Scene 24 (live layout resize) → visually satisfying, good for r/unixporn
