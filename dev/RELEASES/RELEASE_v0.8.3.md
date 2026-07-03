## Release v0.8.3

## Highlights

- Added pre-transaction guardrails for package operations: pacman database locks now block with guidance, while low disk space and stale sync databases warn before install/update work starts.
- Added machine-readable CLI JSON output for `--search`, `--list`, and `--news`, using a `schema_version` envelope for integrators.
- Made `--config-dir` effective before settings, cache/list, repo config, and log paths are resolved.
- Improved package workflows: installed-only search results can open remove preflight directly, `-R` can remove packages from a file, and `--update --mirrors` can refresh mirrors before updating.
- Improved reliability and security by centralizing command capture, reducing redundant official-index enrichment, and narrowing dependency features to avoid vulnerable transitive code paths.

## Full Change Summary

### Safety and transaction guardrails

- Added read-only guardrail checks for pacman database locks, pacman cache disk space, and stale sync databases.
- CLI install, remove, and update paths now run guardrails before starting package transactions.
- TUI preflight execution and system update flows now block when the pacman database is locked and show localized guidance.
- Added localized guardrail messages for English, German, and Hungarian.

### CLI and automation

- Added `--json` output support for CLI search, installed-package listing, and news output.
- Added a shared JSON output envelope with top-level `schema_version`, `command`, and `data` fields.
- Added remove-from-file support for `-R <file>` with the same package-file parsing used by install-from-file.
- Added `--mirrors` for `--update` to run a mirror refresh before the system update.
- Added `aur_helper = auto|paru|yay` settings support for CLI helper selection.
- Updated `config/settings.conf` with the new `aur_helper` setting.

### TUI workflow and UI polish

- Enter / the install action on an installed-only search result now starts the remove flow for that package instead of opening install preflight.
- Remove preflight and preflight-exec modals now use a distinct dark-orange accent so removal actions are easier to distinguish.
- Refined preflight key and mouse handling while keeping existing behavior.

### Mirror and distro handling

- Moved distro-aware mirror update command generation into `logic::distro`.
- Added mirror command coverage for Manjaro, EndeavourOS, CachyOS, Artix, and generic Arch flows.
- Shell-quoted mirror country selections before embedding them in generated shell commands.
- Improved fallback messaging when required mirror tools or AUR helpers are unavailable.

### Reliability, performance, and internals

- Centralized synchronous command execution and binary availability checks in `src/util/command.rs`.
- Reused the shared command runner from pacman, service, dependency, and preflight helpers.
- Centralized signature-validated cache handling for dependency, file, service, and sandbox caches.
- Avoided redundant `pacman -Si` enrichment when official package rows already have the needed metadata, reducing repeated disk writes and notifications.
- Added `--config-dir` override support for config files, repo config, lists, and logs.
- Added and updated unit tests for guardrails, JSON envelopes, settings parsing, path overrides, mirror commands, installed-only remove preflight, cache matching, and enrichment skip behavior.

### Security, dependencies, and platform fixes

- Narrowed `syntect` features to avoid unused vulnerable transitive loaders.
- Updated `rpassword`, `lru`, and lockfile dependency versions, including a rustls-webpki update.
- Improved Windows-specific test and terminal-query code paths.
- Updated the local security-check script description to mirror lint plus security checks.

### Packaging, CI, and documentation

- Added a GitHub Actions lint workflow.
- Added an Arch Linux news watcher workflow and follow-up RSS hash persistence fix.
- Updated release scripts to create annotated tags and align release asset naming.
- Updated PKGBUILD files and package clone exclusions.
- Added the Pacsea logo to the README.
- Consolidated improvement notes into `dev/IMPROVEMENTS/ROADMAP.md` and removed outdated planning files.
