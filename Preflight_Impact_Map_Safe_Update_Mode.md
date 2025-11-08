# Pacsea: Preflight Impact Map + Safe‑Update Mode (Spec)

## Goal
Give users a precise, actionable preview of install/update/remove operations with file‑level changes, config risks, and service impact, then execute safely with snapshots and one‑click rollback.

## TUI Flow

1) Select Items
- From search or Updates view, user selects packages to Install/Update/Remove.
- Press Enter → opens Preflight modal, not the terminal.

2) Preflight Screen (modal with tabs)
- Header: summary chips: packages count, download size, install size delta, AUR count, risk score indicator.
- Tabs (Left/Right switch):
  - Summary: actions list (install/update/remove), version changes, major bumps highlighted.
  - Deps: dependency graph (collapsed), reverse deps at risk, core/system markers.
  - Files: file tree diff for each package (new/changed/removed), predicted pacnew/pacsave.
  - Services: impacted systemd units, restart required? offer restart or defer.
- Footer actions: [c] Create snapshot  [d] Toggle Dry‑Run  [p] Proceed  [q] Cancel  [s] Sandbox AUR preflight (if AUR present)

3) Optional: AUR Sandbox Preflight
- If AUR included, pressing [s] opens a panel:
  - Shows makedepends/depends parsed from .SRCINFO.
  - Dry chroot/nspawn build plan resolution; warns for missing toolchains.
  - Disk/time estimates using cache.

4) Proceed → Execution Screen
- Split view: live log on the right, sticky sidebar on the left with the same summary chips and tabs (read‑only) for context.
- Actions during execution: [l] toggle verbose logs, [x] abort if safe.

5) Post‑Transaction Summary
- Success/fail status, changed files count, services pending restart, pacnew/pacsave created, snapshot label with [r] Rollback.
- Offer one‑click restarts or snooze.

## Keybindings
- Preflight: Tab cycles tabs; Arrow keys navigate; p Proceed; q Cancel; c Snapshot; d Dry‑run; s Sandbox AUR.
- Execution: l Toggle verbosity; x Attempt abort (guarded).
- Post: r Rollback; Enter dismiss.

## Minimal Technical Plan

### Data Sources (no root required for preview)
- Transaction preview:
  - Packages and versions: `pacman -Sp --print-format "%n %v" <names>`
  - Download size: `pacman -Sp --print-format "%n %k" <names>`
  - Installed version lookup: `pacman -Q <name>` (ignore error if not installed)
  - Metadata: `pacman -Si <name>` (batch in chunks)
- Files before install (remote file lists):
  - Sync file DB (once): `pacman -Fy` (cache result timestamp)
  - List: `pacman -Fl <name>` → filter per package
- pacnew/pacsave prediction:
  - Official: parse `PKGBUILD` backup array from upstream (we already fetch PKGBUILD); fallback: `pacman -Qii <name>` for installed packages to read backup files
  - AUR: fetch `PKGBUILD` or `.SRCINFO` and parse `backup=()`
- Service impact:
  - Map package files to units: files under `/usr/lib/systemd/system/*.service` → unit names
  - Heuristic: if a package ships a unit or replaces binaries used by active units, mark restart
  - Query active units: `systemctl list-units --type=service --no-legend`
  - Optional: `systemctl show <unit> -p ExecStart,FragmentPath`
- Risk cues:
  - Major version bump if `semver_major(new) > semver_major(old)`
  - Core/system package list (static allowlist): linux, systemd, glibc, openssl, pacman, bash, util-linux, filesystem
  - AUR present, pacnew predicted, services affected → increment risk score

### Snapshot Integration (optional, auto‑detect)
- Detect in order: Snapper (btrfs), Timeshift, pure btrfs subvolume, LVM, ZFS.
- Commands (execute only on Proceed when enabled):
  - Snapper: `snapper -c root create -d "Pacsea preflight <timestamp>"`
  - Timeshift: `timeshift --create --comments "Pacsea preflight <timestamp>"`
  - btrfs subvol: `btrfs subvolume snapshot -r <root-subvol> <dest>` (requires config)
  - LVM: `lvcreate -s -n pacsea_<timestamp> -l 5%ORIGIN <lv>`
  - ZFS: `zfs snapshot <pool>@pacsea-<timestamp>`
- Store the snapshot reference and show [r] Rollback action with the right command.

### AUR Sandbox (minimal viable)
- Prepare build root on first run: `mkarchroot` or `systemd-nspawn -D /var/lib/pacsea/buildroot pacstrap` (configurable)
- Bind caches (`/var/cache/pacman/pkg`, source cache) for speed.
- Preflight:
  - Clone AUR repo → parse `.SRCINFO` and `PKGBUILD`
  - Compute makedepends/depends deltas vs host
  - Optionally run `makepkg --nobuild` inside sandbox to validate

### Execution
- Keep current execution helpers for pacman/paru/yay but gate them behind the Preflight screen.
- If snapshot enabled, create it first and persist its handle.
- After success, check for `.pacnew/.pacsave` in `/etc` and record findings.
- Detect impacted services and offer restarts with `systemctl restart <unit>`.

## UI Integration (Ratatui)

### State additions
- `AppState.preflight: Option<PreflightPlan>`
- `PreflightPlan` contains:
  - `actions: Vec<PlanItem>`: Install/Update/Remove with from→to versions
  - `download_bytes, install_size_delta`
  - `deps: Graph` (flattened for TUI list/indent view)
  - `files: Vec<FileChange>` grouped by package
  - `services: Vec<ServiceImpact>`
  - `pacnew_candidates: Vec<ConfigFile>`
  - `aur_items: Vec<AurPlan>`
  - `risk_score: u8` and `risk_reasons: Vec<String>`

### Screens/Widgets
- New modal enum: `Modal::Preflight { plan: PreflightPlan, tab: PreflightTab }`
- Renderers: `ui::preflight::render_summary`, `render_deps`, `render_files`, `render_services`
- Execution view: reuse terminal integration; add sidebar chips

### Events
- On ConfirmInstall, compute plan async, then switch to `Modal::Preflight` instead of launching terminal.
- Key handlers per bindings above; proceed dispatches to existing spawn logic.

## Minimal Milestones

1) Milestone A: Core Preflight
- Compute plan for Official packages only (no AUR);
- Tabs: Summary + Files (using `pacman -Fl`), risk score w/ major version and core package detection;
- Proceed executes current flow; post summary shows pacnew created (best‑effort via backup array + /etc scan).

2) Milestone B: Services + Snapshots
- Service impact tab (unit shipped by package), restart prompts;
- Snapshots: autodetect Snapper/Timeshift and create before execution; rollback command surfaced.

3) Milestone C: AUR Sandbox
- Parse `.SRCINFO`, show build deps; optional `--nspawn` preflight; warnings surfaced in Summary.

## Commands Cheat‑Sheet (used by background workers)

```bash
# Preview names/versions/sizes
pacman -Sp --print-format "%n %v" foo bar
pacman -Sp --print-format "%n %k" foo bar

# Remote files (requires one‑time pacman -Fy)
pacman -Fl foo | sed "s/^foo //"

# Metadata and installed versions
pacman -Si foo
pacman -Q foo || true

# AUR metadata
curl -s "https://aur.archlinux.org/rpc/v5/info?arg=foo"
curl -s "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h=foo"

# Services
systemctl list-units --type=service --no-legend
systemctl show sshd.service -p ExecStart,FragmentPath
```

## Risk Score Heuristic (initial)
- +3: kernel/glibc/systemd/pacman updated
- +2: major version bump detected
- +2: any AUR package included
- +1: pacnew predicted
- +1: services impacted that are active
- Buckets: 0–2 low, 3–4 medium, 5+ high (colorize in header chip)

## Config Options (in `pacsea.conf`)
- `preflight.enabled` (default true)
- `preflight.snapshots` (auto | off | snapper | timeshift | btrfs | lvm | zfs)
- `preflight.sandbox` (auto | off)
- `preflight.show_files` (true)

## Notes and Limitations
- File‑list accuracy depends on `pacman -Fy` freshness; refresh weekly or on demand.
- pacnew/pacsave prediction is best‑effort; real outcomes depend on local edits.
- Snapshot support executes external tools; surface clear errors and don’t block installs if they fail unless user opted "require snapshot".


## Implementation TODO (step‑by‑step)

- [x] Wire Enter to open Preflight modal from Search and Install/Remove
- [x] Add Preflight tabs: Summary, Deps, Files, Services, Sandbox
- [x] Add Proceed → Execution screen with sidebar and log
- [x] Add Post‑Transaction Summary modal (structure and UI)

- [ ] Summary tab: compute versions/sizes and risk score
  - [ ] Fetch installed versions (`pacman -Q <name>`) and target versions (`pacman -Sp`)
  - [ ] Compute download bytes and install size delta (`pacman -Sp --print-format`/`-Si`)
  - [ ] Risk score: core packages, major version bumps, AUR present, pacnew predicted, services impacted

- [ ] Deps tab: dependency and reverse‑dependency view
  - [ ] Build dependency closure from `-Si Depends/OptDepends` (AUR via `.SRCINFO`)
  - [ ] Reverse deps for installed packages (`pacman -Qi` Required By)
  - [ ] Highlight core/system packages

- [ ] Files tab: file‑level diff and pacnew/pacsave prediction
  - [ ] Ensure `pacman -Fy` freshness and cache timestamp
  - [ ] List remote files per package (`pacman -Fl <name>`) and diff against installed (`pacman -Ql`)
  - [ ] Predict config conflicts from `backup=()` in PKGBUILD/`.SRCINFO`; highlight under `/etc`

- [ ] Services tab: impacted services and restart plan
  - [ ] Detect shipped units (`/usr/lib/systemd/system/*.service` from `-Fl`)
  - [ ] Check active units (`systemctl list-units --type=service --no-legend`)
  - [ ] Offer restart or defer; remember selection

- [ ] Sandbox tab (AUR only)
  - [ ] Fetch and parse `.SRCINFO`/PKGBUILD; show `makedepends`/`depends`
  - [ ] Optional `makepkg --nobuild` in chroot/nspawn; report issues

- [ ] Snapshot integration
  - [ ] Detect Snapper/Timeshift/btrfs/LVM/ZFS (auto)
  - [ ] Create snapshot on Proceed and store label/handle
  - [ ] Implement Rollback action in Post‑Summary

- [ ] Execution wiring
  - [ ] Replace placeholder with real pacman/paru/yay spawn; stream stdout/stderr to log
  - [ ] Set `abortable` when safe; handle user abort
  - [ ] Capture exit status to mark success/failure

- [ ] Post‑Transaction data (real)
  - [ ] Re‑scan `/etc` for new `.pacnew/.pacsave` created this run (delta)
  - [ ] Count changed files using `pacman -Ql` after transaction
  - [ ] Enable service restart actions (iterate `systemctl restart <unit>`) and snooze

- [ ] Settings integration (`pacsea.conf`)
  - [ ] `preflight.enabled`, `preflight.snapshots`, `preflight.sandbox`, `preflight.show_files`
  - [ ] Persist dry‑run default and last used snapshot mode

- [ ] Error handling and UX polish
  - [ ] Surface network/command errors inside tabs with actionable hints
  - [ ] Truncate/scroll long lists; keep performance on large transactions


