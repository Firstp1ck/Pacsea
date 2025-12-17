## Summary
- **News Mode**: Added comprehensive news feed with Arch news, security advisories, package updates, and AUR comments. Includes filters, sorting (by severity/unread status), read/unread tracking, bookmarks, and optional startup mode (`app_start_mode = news`).
- **Performance & Reliability**: Implemented circuit breaker pattern, rate limiting with exponential backoff, conditional HTTP requests (ETag/Last-Modified), and connection pooling to improve reliability and reduce bandwidth usage when fetching from archlinux.org.
- **Caching**: Multi-layer caching system with persistent storage for news feeds, article content, and last-seen updates/comments. Increased cache TTLs (15min in-memory, 14d disk) to reduce network requests.
- **UI Improvements**: Enhanced footer layout with multi-line keybinds, added loading indicators, improved filter chips, and extended Shift+char keybind support across all panes and modes.
- **Fixes**: Fixed update detection fallback (checkupdates) for Landlock-restricted environments, improved AUR comment date filtering, enhanced error handling for HTTP requests, fixed updates window alignment when text wraps, and aligned options menu key bindings with display order in Package and News modes.

## Type of change
- [x] feat (new feature)
- [x] fix (bug fix)
- [ ] docs (documentation only)
- [x] refactor (no functional change)
- [x] perf (performance)
- [x] test (add/update tests)
- [ ] chore (build/infra/CI)
- [x] ui (visual/interaction changes)
- [ ] breaking change (incompatible behavior)

## How to test
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo check
cargo test -- --test-threads=1
```

**User-facing features:**
- Launch Pacsea, switch to News mode (or set `app_start_mode = news`), verify news/advisories load, toggle filters, and mark items read/unread/bookmarked.
- Verify startup configuration modal appears on first launch.
- Test sorting options (severity/unread), bookmarks persistence, and Shift+char keybinds across all panes.
- Verify updates window shows aligned rows when package names/versions wrap to multiple lines.

**Technical validation:**
- Verify caching works (subsequent views use cached content), rate limiting prevents excessive requests, and circuit breaker activates on repeated failures.
- Test update detection fallback when database sync fails, and verify cache clearing removes all news-related cache files.

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

**Configuration & Persistence:**
- New settings: `app_start_mode` (package/news), `news_filter_*` toggles, `news_max_age_days`. Persisted files: `news_feed.json`, `news_seen_pkg_updates.json`, `news_seen_aur_comments.json`, `news_content_cache.json`, `news_recent_searches.json`, `news_bookmarks.json`, `news_read_urls.json`.
- Cache clearing now includes: `pkgbuild_parse_cache.json`, `arch_news_cache.json`, `advisories_cache.json`, `news_article_cache.json`.

**Performance & Reliability:**
- Circuit breaker per endpoint (opens on failures, closes after recovery), rate limiting with exponential backoff (2s→4s→8s→16s, max 60s), random jitter (0-500ms), HTTP 429 handling with 60s backoff.
- Conditional requests: ETag/Last-Modified headers, Retry-After parsing, connection pooling, cache TTLs (15min in-memory, 14d disk).
- Timeouts: 10s connect, 15s max for fetching; 10s timeout for content loading.

**Technical Details:**
- Update detection: `checkupdates` fallback when temp database sync fails (Landlock restrictions).
- AUR comments: excludes invalid/future dates from "Recent" section, shows "Latest comment" fallback.
- Code quality: migrated deprecated rand API, improved curl parser, refactored HTTP error handling, added test script (`dev/scripts/test_arch.sh`).
- UI: Shift+char keybinds via `handle_shift_keybinds`, improved footer layout, loading toasts, filter chips with clickable rects.
- Updates window: fixed alignment issue when package names/versions wrap by pre-calculating wrapping and padding panes with empty lines to maintain row alignment across all three columns (left, center, right).
- Options menu: reordered menu handlers to match displayed menu order in Package mode (List installed=1, Update system=2, TUI Optional Deps=3, News management=4) and News mode (Update system=1, TUI Optional Deps=2, Package mode=3), updated test cases to use correct key bindings.

## Breaking changes
None.

## Additional context
News feed/event/ UI work touches many files; no breaking changes expected, but configs now include news defaults and new persisted news cache files under the lists directory.

