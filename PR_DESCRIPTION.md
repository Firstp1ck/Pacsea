<!-- Thank you for contributing to Pacsea! Please read CONTRIBUTING.md before submitting. -->

## Summary
This PR introduces comprehensive preflight functionality including services impact analysis, sandbox analysis, conflict detection, and full internationalization (i18n) support. The preflight modal now provides detailed information about dependencies, file changes, systemd service impacts, and AUR package sandbox analysis before package installation or removal. All UI strings have been localized to support both English and German languages.

## Type of change
- [x] feat (new feature)
- [ ] fix (bug fix)
- [ ] docs (documentation only)
- [x] refactor (no functional change)
- [ ] perf (performance)
- [ ] test (add/update tests)
- [ ] chore (build/infra/CI)
- [x] ui (visual/interaction changes)
- [ ] breaking change (incompatible behavior)

## How to test

```bash
# Format and lint
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo test -- --test-threads=1

# Test preflight modal with dry-run
RUST_LOG=pacsea=debug cargo run -- --dry-run

# Test localization (switch language in config)
# Edit settings.conf to set locale = "de-DE" or locale = "en-US"
cargo run -- --dry-run

# Test preflight tabs:
# 1. Add packages to install list
# 2. Open preflight modal (defaults to Deps tab)
# 3. Navigate through tabs: Summary, Deps, Files, Services, Sandbox
# 4. Verify all strings are localized correctly
# 5. Test conflict detection with conflicting packages
# 6. Test service restart decisions
# 7. Test sandbox analysis for AUR packages
```

## Checklist
- [x] Code compiles locally
- [x] `cargo fmt --all` ran without changes
- [x] `cargo clippy --all-targets --all-features -- -D warnings` is clean
- [x] `cargo test -- --test-threads=1` passes
- [x] Added or updated tests where it makes sense
- [ ] Updated docs if behavior, options, or keybinds changed (README, config examples)
- [x] Changes respect `--dry-run` and degrade gracefully if `pacman`/`paru`/`yay` are unavailable
- [ ] If config keys changed: updated README sections for `settings.conf`, `theme.conf`, and `keybinds.conf`
- [x] Not a packaging change for AUR (otherwise propose in `pacsea-bin` or `pacsea-git` repos)

## Notes for reviewers

### Key Changes:

1. **Preflight Services & Summary**:
   - Added `Services` tab to preflight modal showing systemd service impacts
   - Implemented service restart decision logic with defer/restart options
   - Added comprehensive preflight summary calculation with risk assessment
   - Services are cached for performance

2. **Conflict Detection**:
   - Enhanced dependency resolution to detect package conflicts
   - Parses conflicts from both `pacman` queries and `.SRCINFO` files
   - Displays conflicts prominently in Deps tab with red warning indicators
   - Added localized conflict messages

3. **Sandbox Analysis**:
   - Added `Sandbox` tab for AUR package analysis
   - Analyzes build dependencies (depends, makedepends, checkdepends, optdepends)
   - Caches sandbox analysis results
   - Only analyzes AUR packages (official packages are pre-built)

4. **Internationalization (i18n)**:
   - Complete i18n module with locale detection and loading
   - All UI strings moved to locale files (`locales/en-US.yml`, `locales/de-DE.yml`)
   - Supports English and German with fallback to English
   - Locale can be configured in `settings.conf` with `locale = "de-DE"` or `locale = "en-US"` (empty = auto-detect)
   - Fixed hardcoded strings in Services, Files, and Sandbox tabs

5. **Caching**:
   - Services cache: `services_cache.rs` for service impact caching
   - Sandbox cache: `sandbox_cache.rs` for AUR dependency analysis caching
   - Caches are signature-based and persist across sessions
   - Background resolution when caches are empty

6. **Dependency Resolution Enhancements**:
   - Refactored dependency parsing and resolution logic
   - Improved `.SRCINFO` parsing for AUR packages
   - Better conflict detection and status reporting
   - Enhanced dependency source tracking (Official vs AUR)

### Technical Details:

- **Lazy Loading**: Preflight tabs load data on-demand (Deps loads immediately as default tab)
- **Background Resolution**: Files, services, and sandbox data resolve in background when packages are added
- **Cache Management**: All caches use signature-based invalidation
- **Error Handling**: Graceful degradation when caches are unavailable or resolution fails
- **Performance**: Viewport-based rendering for large lists (Services tab)

### Files Changed:
- `src/ui/modals/preflight.rs`: Major refactor, added Services and Sandbox tabs
- `src/logic/preflight.rs`: New preflight summary calculation
- `src/logic/services.rs`: Service impact resolution
- `src/logic/sandbox.rs`: Sandbox analysis for AUR packages
- `src/logic/deps/`: Enhanced conflict detection and parsing
- `src/i18n/`: Complete i18n implementation
- `locales/`: English and German locale files
- `src/app/runtime.rs`: Background resolution workers
- `src/app/services_cache.rs`: Services caching
- `src/app/sandbox_cache.rs`: Sandbox caching

## Additional context

### Preflight Tab Loading Priority:
1. **Deps Tab**: Loads immediately when modal opens (default tab)
2. **Files Tab**: Lazy-loaded when user navigates to it
3. **Services Tab**: Lazy-loaded when user navigates to it
4. **Sandbox Tab**: Lazy-loaded when user navigates to it

All tabs check cache first, then trigger background resolution if needed.

### Localization:
- All hardcoded strings have been moved to locale files
- Recent fixes ensure Services, Files, and Sandbox tabs use localized strings
- Added missing keys: `conflicts_sentence`, `pacnew_label`, `pacsave_label`, `package_label`, `no_packages`
- German locale includes all translations

### Conflict Detection:
- Conflicts are detected during dependency resolution
- Displayed with red warning indicators (⚠) in Deps tab
- Conflict reasons are parsed from package metadata
- Conflicts block installation and are clearly highlighted
