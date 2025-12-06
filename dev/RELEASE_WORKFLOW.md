# Release Workflow

Automated release process using `release.fish`.

## Usage

```fish
# Full release
./dev/scripts/release.fish 0.7.0

# Preview (dry-run)
./dev/scripts/release.fish --dry-run 0.7.0
```

## Pre-flight Checks

Before starting, the script verifies:
- ✅ On `main` branch
- ✅ Working directory is clean (no uncommitted changes)

## Workflow Steps

### Phase 1: Version Update
1. Update `Cargo.toml` version
2. Run `cargo check` to update `Cargo.lock`

### Phase 2: Documentation
3. **[Cursor]** Run `/release-new {version}` → creates `Documents/RELEASE_v{version}.md`
4. Update `CHANGELOG.md` with release notes
5. Auto-generate announcement from release file
6. Run `update_version_announcement.py`
7. **[Cursor]** Run `/readme-update`
8. **[Cursor]** Run `/wiki-update`

### Phase 3: PKGBUILD Updates
9. Update `pkgver` in `~/aur-packages/pacsea-bin/PKGBUILD`
10. Update `pkgver` in `~/aur-packages/pacsea-git/PKGBUILD`
11. Reset `pkgrel=1` in both

### Phase 4: Build and Release
12. Run `cargo-dev` (tests/checks)
13. Build release binary
14. Commit and push all changes
15. Create git tag (tag = version, e.g., `0.7.0`)
16. Push tag to GitHub
17. Create GitHub release (binary uploaded by GitHub Action)

### Phase 5: AUR Update
18. **Wait for GitHub Action to complete**
19. Run `update-sha` in `~/aur-packages/pacsea-bin/`
20. Run `aur-push`
21. Copy PKGBUILDs back to Pacsea repo

## Manual Steps (Cursor AI)

The script pauses at these steps for you to run Cursor commands:

| Step | Command | Output |
|------|---------|--------|
| 3 | `/release-new {version}` | `Documents/RELEASE_v{version}.md` |
| 7 | `/readme-update` | Updates `README.md` |
| 8 | `/wiki-update` | Updates wiki files |

## Files Updated

| File | Description |
|------|-------------|
| `Cargo.toml` | Version number |
| `Cargo.lock` | Dependency lock |
| `Documents/RELEASE_v{version}.md` | Release notes |
| `CHANGELOG.md` | Cumulative changelog |
| `dev/ANNOUNCEMENTS/version_announcement_content.md` | In-app announcement |
| `src/announcements.rs` | Compiled announcement |
| `README.md` | Project readme |
| `PKGBUILD-bin` | AUR binary package |
| `PKGBUILD-git` | AUR git package |

## Prerequisites

- `cursor` CLI
- `gh` CLI (GitHub)
- `cargo`
- `git`
- `python3`
- Fish functions: `cargo-dev`, `update-sha`, `aur-push`

