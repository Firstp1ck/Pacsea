## Summary
- Added News mode with Arch news, security advisories, installed updates, and AUR comments plus filters/sorting, read/unread tracking, bookmarks, and optional `app_start_mode = news` startup setting (alias: `start_in_news`).
- Added a dedicated `[AUR Upd]` news filter toggle so AUR update items can be shown/hidden independently of official package updates; defaults on and persists in settings.
- Added cached news feed and last-seen update/comment maps (`news_feed.json`, `news_seen_pkg_updates.json`, `news_seen_aur_comments.json`) with loading state and filter chips for updates/comments.
- Added news content caching mechanism with persistence (`news_content_cache.json`) to improve loading performance; cached article bodies are reused on subsequent views.
- Added news content loading timeout (6-second limit) with enhanced logging for requests/responses to prevent indefinite loading states.
- Added startup news popup configuration modal allowing users to configure which news types to display (Arch news, advisories, AUR updates, AUR comments, package updates) on first launch.
- Added timeout settings for news fetching (10s connect, 15s max) to prevent blocking on slow or unreachable servers.
- Added severity-based sorting (`SeverityThenDate`) and unread-based sorting (`UnreadThenDate`) options for prioritizing critical advisories and unread items.
- Added stale content clearing for bookmarks: when loading a bookmark without cached content, stale content is cleared and loading state is reset.
- Updated UI panes/menus/modals and localization to render news summaries, highlight AUR comment keywords/links, and extend news history/bookmark panes and workers for the richer feed.

## Type of change
- [x] feat (new feature)
- [ ] fix (bug fix)
- [ ] docs (documentation only)
- [ ] refactor (no functional change)
- [ ] perf (performance)
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
cargo test complexity -- --nocapture --test-threads=1
```
- Launch Pacsea, switch to News mode (or set `app_start_mode = news`), and verify Arch news + advisories load; toggle filter chips and mark items read/unread/bookmarked.
- On first launch, verify the startup news popup configuration modal appears and allows configuring news display preferences.
- Run a news search, confirm the history pane records queries, and restart to ensure history/bookmarks persist.
- Open a news item to fetch content, scroll, and ensure subsequent openings use the cached body from `news_content_cache.json`.
- Test news content timeout: open a news item from a slow/unreachable server and verify it times out after 6 seconds with appropriate error feedback.
- Test sorting options: verify `SeverityThenDate` prioritizes critical advisories and `UnreadThenDate` shows unread items first.
- Load a news bookmark that has no cached HTML/content and confirm the details pane clears stale content, resets `news_content_loading`, and allows a fresh content request.

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
- News mode persists `news_recent_searches.json`, `news_bookmarks.json`, `news_read_urls.json`, and `news_content_cache.json` (bookmarks loader keeps backward compatibility with old feed item format).
- Settings gain `app_start_mode` (`package`/`news`, alias `start_in_news`), `news_filter_show_arch_news`, `news_filter_show_advisories`, `news_filter_show_pkg_updates`, `news_filter_show_aur_comments`, `news_filter_installed_only`, and `news_max_age_days`; defaults/locales updated to match.
- News content worker caches article bodies in `news_content_cache.json` (error messages filtered out) and treats AUR package URLs as comment views; filter chips expose clickable rects for mouse-driven toggles.
- News content loading includes a 6-second timeout with detailed logging; application state tracks `news_content_loading_since` for timeout management.
- News fetching uses shorter timeouts (10s connect, 15s max) to prevent blocking on slow servers; curl calls include timeout arguments.
- Startup news popup configuration modal appears on first launch to configure news display preferences; state tracks `news_startup_config_completed`.
- News sorting includes `SeverityThenDate` (prioritizes critical advisories) and `UnreadThenDate` (prioritizes unread items) modes; severity ranking system implemented.
- News feed cache and last-seen maps persist under `lists/` (`news_feed.json`, `news_seen_pkg_updates.json`, `news_seen_aur_comments.json`); feed reloads from cache until background refresh completes.

## Breaking changes
None.

## Additional context
News feed/event/ UI work touches many files; no breaking changes expected, but configs now include news defaults and new persisted news cache files under the lists directory.

