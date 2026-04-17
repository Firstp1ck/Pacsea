# Release v0.8.2

## What's New

Compared to **v0.8.1**, this release focuses on layout customization, smoother PKGBUILD viewing, better modal scrolling, and desktop launcher files. Packaging for **pacsea-git** on the AUR was aligned with the current repo layout (including merged [PR #158](https://github.com/Firstp1ck/Pacsea/pull/158)).

### ✨ Features

- **Configurable UI layout**: Set `main_pane_order` and per-role vertical min/max in `settings.conf` so search, results, and details appear in the order and proportions you prefer.
- **Mouse wheel in modals**: Scroll the focused row in System Update, Repositories, and Optional Dependencies modals when the pointer is over the list.
- **Desktop integration**: `.desktop` entry and SVG icon ship with the tree for menu launchers and file managers.

### 🛡 Security & reliability

- **PKGBUILD fetching**: Each fetch runs in its own async task so one slow host does not block the queue; stale results are dropped when you change rows.

### 🐛 Fixes

- Shorter connect timeouts on PKGBUILD `curl` calls so bad hosts fail faster.
- **pacsea-git** / `makepkg`: clear toolchain env (including `CHOST`) before builds when `makepkg.conf` has cross-compile defaults that would break a normal package build.
- **Packaging**: Correct source URLs and sparse-checkout paths in `PKGBUILD-git`; icon file permissions set for normal files (not executable).

## Technical Details

Layout rendering follows pane roles (not fixed slots), modal bounds are recorded for wheel hit-testing, and PKGBUILD viewer state resets cleanly on row changes so the UI stays in sync with fetches.

## Full Changelog

**Full Changelog**: https://github.com/Firstp1ck/Pacsea/compare/v0.8.1...v0.8.2
