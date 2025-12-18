## Summary

**What's New:**
- **News Mode**: Complete news feed system with Arch Linux news, security advisories, package updates, and AUR comments. Users can filter, sort, bookmark, and track read/unread status. Optional startup mode available via `app_start_mode = news` setting.
- **Improved Reliability**: Enhanced network request handling with rate limiting, circuit breakers, and better error recovery to prevent IP blocking from archlinux.org.
- **Better Performance**: Multi-layer caching (15min memory, 14 days disk) reduces network requests and speeds up subsequent views.
- **User Experience**: Internationalized loading messages, improved UI alignment, clearer menu organization, and better compatibility with archlinux.org's DDoS protection.

**Bug Fixes:**
- Fixed update detection for Landlock-restricted environments
- Fixed updates window text alignment when package names wrap
- Fixed options menu key bindings to match display order
- Fixed `installed_packages.txt` export to respect `installed_packages_mode` setting
- Improved AUR comment date filtering

## Type of change
- [x] feat (new feature)
- [x] fix (bug fix)
- [ ] docs (documentation only)
- [x] refactor (no functional change)
- [x] perf (performance)
- [x] test (add/update tests)
- [x] chore (build/infra/CI)
- [x] ui (visual/interaction changes)
- [ ] breaking change (incompatible behavior)

## How to test
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo check
cargo test -- --test-threads=1
```

**Testing News Mode:**
1. Launch Pacsea and switch to News mode (or set `app_start_mode = news` in settings)
2. Verify news items load (Arch news, advisories, package updates, AUR comments)
3. Test filters, sorting, and read/unread tracking
4. Verify bookmarks persist across sessions
5. Check that loading messages appear on first launch with informative hints

**Testing Reliability:**
- Verify rate limiting prevents IP blocking (no 429 errors in logs)
- Test that cached content loads faster on subsequent views
- Verify circuit breaker activates on repeated failures and recovers

**Testing Bug Fixes:**
- Verify updates window alignment when package names wrap
- Test options menu key bindings match display order
- Verify `installed_packages.txt` respects `installed_packages_mode` setting

## Checklist

**Code Quality:**
- [x] Code compiles locally (`cargo check`)
- [x] `cargo fmt --all` ran without changes
- [x] `cargo clippy --all-targets --all-features -- -D warnings` is clean
- [x] `cargo test -- --test-threads=1` passes
- [x] Complexity checks pass for new code (`cargo test complexity -- --nocapture`)
- [x] All new functions/methods have rustdoc comments (What, Inputs, Output, Details)
- [x] No `unwrap()` or `expect()` in non-test code

**Testing:**
- [x] Added or updated tests where it makes sense
- [x] For bug fixes: created failing tests first, then fixed the issue
- [x] Tests are meaningful and cover the functionality

**Documentation:**
- [x] Updated README if behavior, options, or keybinds changed (keep high-level, reference wiki)
- [x] Updated relevant wiki pages if needed:
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

**New Configuration Options:**
- `app_start_mode`: Set to "news" to start in News mode (default: "package")
- `news_filter_*`: Toggle filters for Arch news, advisories, package updates, AUR updates, and AUR comments
- `news_max_age_days`: Maximum age filter for news items (default: unlimited)

**New Persisted Files:**
- `news_feed.json` - Cached news feed data
- `news_content_cache.json` - Cached article content
- `news_seen_pkg_updates.json` - Tracked package update versions
- `news_seen_aur_comments.json` - Tracked AUR comment IDs
- `news_recent_searches.json` - News search history
- `news_bookmarks.json` - Bookmarked news items
- `news_read_urls.json` - Read news URLs

**Key Technical Improvements:**
- **Rate Limiting**: All archlinux.org requests are serialized (1 at a time) with exponential backoff (2s→4s→8s→16s, max 60s) to prevent IP blocking
- **Circuit Breaker**: Per-endpoint failure detection prevents cascading failures
- **Caching**: 15min in-memory, 14 days disk cache reduces network requests
- **Conditional Requests**: ETag/Last-Modified headers for efficient updates
- **Compatibility**: Browser-like headers and increased timeouts for archlinux.org DDoS protection
- **i18n**: Loading messages translated in en-US, de-DE, hu-HU with informative hints about first load duration

**Implementation Details:**
- Rate limiting applied to: news fetching, package date fetching, package index fetching
- Timeouts: 15s connect, 30s total for news; 5s for AUR comments; 2s for package dates
- Update detection fallback: Uses `checkupdates` when database sync fails (Landlock restrictions)
- UI improvements: Multi-line keybinds, improved alignment, better menu organization

## Breaking changes
None.

## Additional context

This PR introduces a complete news feed system to Pacsea. The implementation includes extensive rate limiting and reliability improvements to work well with archlinux.org's DDoS protection. All changes are backward compatible - existing users will continue to work as before, with News mode available as an optional feature.

**Files changed:** 144 files, ~18k insertions, ~1.2k deletions
**New files:** News feed UI, rate limiting infrastructure, i18n translations
**Breaking changes:** None

