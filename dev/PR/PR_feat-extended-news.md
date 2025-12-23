## Summary

**What's New:**
- **News Mode**: Complete news feed system with Arch Linux news, security advisories, package updates, and AUR comments. Filter, sort, bookmark, and track read/unread status. Optional startup mode via `app_start_mode = news`.
- **JSON Caching**: Cache AUR and official package JSON responses to disk for change detection and offline date fallback
- **Change Detection**: Compare cached vs current JSON to detect package changes (version, maintainer, dependencies, etc.) and display in news content
- **Background Retry Queue**: Failed package date fetches are queued and retried sequentially with exponential backoff (10s, 20s, 40s), up to 3 attempts per package
- **Background Continuation**: After initial limit (50 items), continue fetching all remaining items in background and stream to UI at 1 per second
- **Package Date Fetching**: Fetches package update dates from archlinux.org JSON endpoints with fallback to cached data, handles multiple repo/arch combinations, and distinguishes HTTP status codes (404 vs 429/5xx)
- **Date Parsing**: Handles RFC3339 format with milliseconds, RSS dates, and normalizes to YYYY-MM-DD for consistent sorting
- **AUR Package Allocation**: AUR packages get dedicated allocation (half of limit) to ensure representation alongside official packages
- **Reliability**: Rate limiting, circuit breakers, and error recovery prevent IP blocking from archlinux.org (404s don't trigger circuit breaker)
- **Performance**: Multi-layer caching (15min memory, 14 days disk) reduces network requests
- **Code Quality**: Improved clippy allow comments, reduced function complexity, added CodeQL workflow
- **Refactoring**: Modularized large source files into organized submodules (sources/feeds, sources/news, events/modals/tests, ui/results/title, app_state, workers)
- **Logging**: Promoted important operational messages from DEBUG to INFO level for better visibility
- **i18n**: Made config directory alert detection language-agnostic using path patterns instead of hardcoded strings

**Bug Fixes (to existing code in main branch):**
- Fixed updates window text alignment when package names wrap
- Fixed options menu key bindings to match display order
- Fixed `installed_packages.txt` export to respect `installed_packages_mode` setting
- Fixed alert title showing "Connection issue" instead of "Configuration Directories" for config directory messages after package removal
- Fixed Shift+Tab keybind to also work in News mode (previously only worked in Package mode)

## Type of change
- [x] feat (new feature)
- [x] fix (bug fix)
- [x] refactor (no functional change)
- [x] perf (performance)
- [x] test (add/update tests)
- [x] chore (build/infra/CI)
- [x] style (formatting, code style)
- [x] ui (visual/interaction changes)

## How to test

**Basic Tests:**
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo check
cargo test -- --test-threads=1
```

**News Mode:**
1. Launch Pacsea, switch to News mode (or set `app_start_mode = news`)
2. Verify news items load (Arch news, advisories, updates, AUR comments)
3. Test filters, sorting, read/unread tracking, and bookmarks
4. Check loading messages appear on first launch
5. Test Shift+Tab cycles through news sort modes (Date↓, Date↑, Title, Source+Title, Severity+Date, Unread+Date)
6. Verify background continuation streams additional items after initial 50 (check logs for "continuation worker")
7. Verify package update dates are correct (not showing today's date when network fails)
8. Check news content shows JSON change descriptions for AUR and official packages
9. Verify AUR packages appear even when official packages fill the limit

**Reliability:**
- Verify no 429 errors in logs (rate limiting working)
- Test cached content loads faster on subsequent views
- Verify circuit breaker activates on failures and recovers

**Bug Fixes:**
- See "Bug Fixes (to existing code in main branch)" section above

## Checklist

- [x] Code compiles, formats, and passes clippy
- [x] All tests pass
- [x] New functions have rustdoc comments
- [x] No `unwrap()` or `expect()` in non-test code
- [x] Changes respect `--dry-run` flag
- [x] Code degrades gracefully if tools unavailable
- [x] No breaking changes

## Notes for reviewers

**Configuration:**
- `app_start_mode`: "news" to start in News mode (default: "package")
- `news_filter_*`: Toggle filters for Arch news, advisories, updates, AUR updates/comments
- `news_max_age_days`: Maximum age filter (default: unlimited)

**New Files:**
- `news_feed.json`, `news_content_cache.json`, `news_seen_pkg_updates.json`, `news_seen_aur_comments.json`, `news_recent_searches.json`, `news_bookmarks.json`, `news_read_urls.json`
- `cache/aur_json/` - Cached AUR package JSON responses for change detection
- `cache/official_json/` - Cached official package JSON responses for change detection and date fallback

**Technical Highlights:**
- **Rate Limiting**: Serialized archlinux.org requests (1 at a time) with exponential backoff (2s→4s→8s→16s, max 60s)
- **Circuit Breaker**: Per-endpoint failure detection prevents cascading failures (404s don't trigger circuit breaker)
- **Conditional Requests**: ETag/Last-Modified headers for efficient updates
- **Timeouts**: 15s connect, 30s total for news; 5s for AUR comments; 2s for package dates
- **Fallback**: Uses `checkupdates` when database sync fails (Landlock restrictions)
- **UI**: Multi-line keybinds, improved alignment, better menu organization
- **Code Quality**: Enhanced clippy comments with line counts, reduced complexity via helper functions and type aliases, CodeQL workflow
- **Refactoring**: Split large files (2981-line feeds.rs, 1731-line news.rs, 1689-line tests.rs, 1448-line title.rs) into modular subdirectories; extracted alert message type detection and formatting into helper functions
- **Documentation**: Added comments explaining intentionally unused parameters
- **i18n**: Added translation keys for config directory alerts (en-US, de-DE, hu-HU); made detection language-agnostic using path pattern matching

## Breaking changes
None. All changes are backward compatible.
