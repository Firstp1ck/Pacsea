# Release v0.8.0

## What's New

### ✨ Features

**Custom Pacman repositories (`repos.conf`)**
- Define extra repos in `repos.conf`, edit them from an in-app **Repositories** modal, and apply changes into `pacman.conf` when you are ready (with privilege prompts when needed).
- Third-party repo databases are indexed (via `pacman -Sl`) so search and filters can see packages from those repos, alongside clearer deduplication by repo + package name.
- First-run seeding and a shipped **`repos_example.conf`** reference (with sample recipes) help you get started safely.

**AUR package voting over SSH (opt-in)**
- Vote or unvote AUR packages from search when enabled in settings, using SSH to `aur.archlinux.org`.
- Guided **SSH AUR setup** flow helps install or fix the `aur.archlinux.org` host block in your SSH config.
- Respects your configured SSH command, timeout, and **dry-run** semantics (no fake “dirty” vote cache in dry-run).

**PKGBUILD checks (ShellCheck + Namcap)**
- Run **ShellCheck** and **Namcap** on the PKGBUILD for the selected package from the details view (background worker, timeouts, graceful handling when tools are missing).
- Cycle between **PKGBUILD** and **checks** in the details pane (default **`Ctrl+D`**); optional raw checker output and ShellCheck exclude list in settings.
- Example keybinds include **`Ctrl+K`** to run checks; the checks panel stays aligned with the package you selected.

### 🐛 Fixes & hardening

**Repositories**
- Safer validation for repo apply paths, server URLs, and signing-key handling; stable mirror filenames; correct pending-state cleanup if you cancel auth or hit errors.

**PKGBUILD checks**
- Checker requests are wired reliably through the event loop; timeouts use Tokio instead of relying on an external `timeout` binary; clearer UI when a checker binary is not installed.

**UI**
- Wider-character-safe padding and truncation in repo-related lists and filter menus so columns line up in the terminal.

## Technical Details

This release adds optional **AUR voting**, optional **PKGBUILD static analysis**, and a full **custom repository** workflow (config, UI, indexing, and pacman integration), with emphasis on safe defaults, dry-run behavior, and missing-tool degradation.

## Full Changelog

**Full Changelog**: https://github.com/Firstp1ck/Pacsea/compare/v0.7.4...v0.8.0
