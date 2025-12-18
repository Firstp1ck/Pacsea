## Summary

**What's New:**
- **News Mode**: Complete news feed system with Arch Linux news, security advisories, package updates, and AUR comments. Filter, sort, bookmark, and track read/unread status. Optional startup mode via `app_start_mode = news`.
- **Reliability**: Rate limiting, circuit breakers, and error recovery prevent IP blocking from archlinux.org.
- **Performance**: Multi-layer caching (15min memory, 14 days disk) reduces network requests.
- **Code Quality**: Improved clippy allow comments, reduced function complexity, added CodeQL workflow.

**Bug Fixes:**
- Fixed update detection for Landlock-restricted environments
- Fixed updates window text alignment when package names wrap
- Fixed options menu key bindings to match display order
- Fixed `installed_packages.txt` export to respect `installed_packages_mode` setting
- Improved AUR comment date filtering

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

**Reliability:**
- Verify no 429 errors in logs (rate limiting working)
- Test cached content loads faster on subsequent views
- Verify circuit breaker activates on failures and recovers

**Bug Fixes:**
- Updates window alignment when package names wrap
- Options menu key bindings match display order
- `installed_packages.txt` respects `installed_packages_mode` setting

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

**Technical Highlights:**
- **Rate Limiting**: Serialized archlinux.org requests (1 at a time) with exponential backoff (2s→4s→8s→16s, max 60s)
- **Circuit Breaker**: Per-endpoint failure detection prevents cascading failures
- **Caching**: 15min in-memory, 14 days disk cache
- **Conditional Requests**: ETag/Last-Modified headers for efficient updates
- **Timeouts**: 15s connect, 30s total for news; 5s for AUR comments; 2s for package dates
- **Fallback**: Uses `checkupdates` when database sync fails (Landlock restrictions)
- **UI**: Multi-line keybinds, improved alignment, better menu organization
- **Code Quality**: Enhanced clippy comments, reduced complexity, CodeQL workflow

## Breaking changes
None. All changes are backward compatible.
