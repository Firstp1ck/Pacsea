<!-- Thank you for contributing to Pacsea! 

**Important references:**
- [CONTRIBUTING.md](../CONTRIBUTING.md) — Full contribution guidelines and PR process
- [PR_DESCRIPTION.md](../Documents/PR_DESCRIPTION.md) — Detailed PR description template
- [Development Wiki](https://github.com/Firstp1ck/Pacsea/wiki/Development) — Development tools and debugging

Please ensure you've reviewed these before submitting your PR.
-->

## Summary
This PR implements performance optimizations and cache hardening across the application, plus small reliability fixes:

- **Performance optimizations:**
  - Implemented cache-based O(n) reordering for sort modes, enabling O(1) sort mode switching
  - Added HashMap index (`name_to_idx`) to `OfficialIndex` for O(1) package name lookups
  - Optimized install/remove/downgrade list operations using HashSet for O(1) membership checking
  - Added search result caching to avoid redundant query processing
  - Hardened cache persistence and invalidation for deps/files/services/sandbox caches
  - Switched recent searches to an `LruCache` (O(1) dedup, bounded to 20, MRU-first ordering)

- **Cache improvements:**
  - Strengthened settings cache invalidation by comparing file sizes to avoid stale reloads after config rewrites
  - Added explicit tracing and error handling for cache persistence/cleanup
  - Improved cache synchronization with runtime for better consistency
  - Hardened PKGBUILD parse cache writes to avoid stale overwrites during concurrent parses; added regression coverage

- **Additional features (within this branch):**
  - Enhanced event loop with index notification and updates handling
  - Added comprehensive test coverage for modals and system update functionality
  - Improved logging with ChangeLogger helper to reduce duplicate UI debug output
  - Quieted noisy logs by clamping HTML parser crates and service resolution logs to warn level
  - Reduced preflight dependency log spam by logging only on state changes
  - Avoided futile `pacman -Sl eos` probes; only `endeavouros` is queried to prevent warnings

## Type of change
- [ ] feat (new feature)
- [x] fix (bug fix)
- [ ] docs (documentation only)
- [ ] refactor (no functional change)
- [x] perf (performance)
- [x] test (add/update tests)
- [ ] chore (build/infra/CI)
- [ ] ui (visual/interaction changes)
- [ ] breaking change (incompatible behavior)

## How to test
List exact steps and commands to verify the change. Include flags like `--dry-run` when appropriate.

```bash
# Format and lint
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo test -- --test-threads=1

# Manual UI verification (optional)
# 1. Run in dry-run mode: cargo run -- --dry-run
# 2. Perform >20 unique searches; confirm oldest entries evict and duplicates move to the front
# 3. Restart and ensure recent history reloads with the same ordering
```

## Checklist

**Code Quality:**
- [x] Code compiles locally (`cargo check`)
- [x] `cargo fmt --all` ran without changes
- [x] `cargo clippy --all-targets --all-features -- -D warnings` is clean
- [x] `cargo test -- --test-threads=1` passes
- [ ] Complexity checks pass for new code (`cargo test complexity -- --nocapture`)
- [x] All new functions/methods have rustdoc comments (What, Inputs, Output, Details)
- [x] No `unwrap()` or `expect()` in non-test code

**Testing:**
- [x] Added or updated tests where it makes sense
- [x] For bug fixes: created failing tests first, then fixed the issue
- [x] Tests are meaningful and cover the functionality

**Documentation:**
- [ ] Updated README if behavior, options, or keybinds changed (keep high-level, reference wiki)
- [ ] Updated relevant wiki pages if needed:
  - [How to use Pacsea](https://github.com/Firstp1ck/Pacsea/wiki/How-to-use-Pacsea)
  - [Configuration](https://github.com/Firstp1ck/Pacsea/wiki/Configuration)
  - [Keyboard Shortcuts](https://github.com/Firstp1ck/Pacsea/wiki/Keyboard-Shortcuts)
- [ ] Updated config examples in `config/` directory if config keys changed
- [ ] For UI changes: included screenshots and updated `Images/` if applicable

**Compatibility:**
- [x] Changes respect `--dry-run` flag
- [x] Code degrades gracefully if `pacman`/`paru`/`yay` are unavailable
- [x] No breaking changes (or clearly documented if intentional)

**Other:**
- [x] Not a packaging change for AUR (otherwise propose in `pacsea-bin` or `pacsea-git` repos)

## Notes for reviewers

**Performance optimizations:**
- **Sort cache reordering**: Implemented O(n) reordering instead of O(n log n) full sort when switching between cached sort modes. Cache invalidation is integrated and triggers on filter/search result changes.
- **Package name index**: Added `HashMap<String, usize>` to `OfficialIndex` for O(1) lookups via `find_package_by_name()`. Index is rebuilt on index load/update operations.
- **Install list HashSet**: Replaced linear scans with `HashSet<String>` for O(1) membership checks in install/remove/downgrade operations. Uses case-insensitive keys via lowercase conversion.
- **Search result caching**: Added `search_cache_query`, `search_cache_fuzzy`, and `search_cache_results` fields to `AppState` to cache last query/results pair and avoid redundant processing.

**Cache improvements:**
- **Settings cache invalidation**: Added file size comparison to force reload after config rewrites, fixing CI-only test flake where cache appeared valid but content was stale.
- **Cache persistence**: Enhanced deps/files/services/sandbox caches with explicit tracing and error handling. Caches use signature-based validation (sorted package names) to detect changes.
- **PKGBUILD cache writes**: Added signature re-checking to prevent stale parses from overwriting newer cache entries; deterministic concurrent regression test included.

**Code quality:**
- All new code includes rustdoc comments with What/Inputs/Output/Details sections
- Complexity checks: New functions maintain cyclomatic complexity < 25 and data flow complexity < 25
- Comprehensive test coverage added for modal interactions and system update functionality

**Focus areas for review:**
1. Recent search LRU path (`AppState.recent`, persistence/load, pane filtering/rendering)
2. Cache invalidation logic in `src/app/persist.rs` and cache modules
3. Performance-critical paths in `src/logic/sort.rs` and `src/logic/lists.rs`
4. Index rebuilding logic in `src/index/mod.rs`
5. Search cache integration in `src/app/runtime/handlers/search.rs`

## Breaking changes
None. All changes are backward compatible.

## Additional context

**Performance impact:**
- Sort mode switching: O(n log n) → O(n) when cache is valid
- Package name lookup: O(n) → O(1) via HashMap index
- Install list membership: O(n) → O(1) via HashSet
- Search result reuse: Eliminates redundant query processing for repeated queries

**Implementation details:**
- Sort cache fields: `sort_cache_repo_name`, `sort_cache_aur_popularity`, `sort_cache_signature` in `AppState`
- Cache invalidation: Integrated into filter/search result change handlers
- Index rebuilding: Automatic on `load_from_disk()`, index fetch/update operations
- HashSet keys: Case-insensitive via `to_lowercase()` for consistent membership checks

**Related documentation:**
- See `dev/PERFORMANCE_IMPLEMENTATION_PRIORITY.md` for tracking of performance optimizations
- Cache infrastructure follows existing patterns in `src/app/*_cache.rs` modules

