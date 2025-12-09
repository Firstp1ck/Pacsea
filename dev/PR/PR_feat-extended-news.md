## Summary
- Added News mode with Arch news, security advisories, installed updates, and AUR comments plus filters/sorting, read/unread tracking, bookmarks, and optional `app_start_mode = news` startup setting (alias: `start_in_news`).
- Added a dedicated `[AUR Upd]` news filter toggle so AUR update items can be shown/hidden independently of official package updates; defaults on and persists in settings.
- Added cached news feed and last-seen update/comment maps (`news_feed.json`, `news_seen_pkg_updates.json`, `news_seen_aur_comments.json`) with loading state and filter chips for updates/comments.
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

## Related issues
Closes #N/A

## How to test
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo check
cargo test -- --test-threads=1
cargo test complexity -- --nocapture --test-threads=1
```
- Launch Pacsea, switch to News mode (or set `app_start_mode = news`), and verify Arch news + advisories load; toggle filter chips and mark items read/unread/bookmarked.
- Run a news search, confirm the history pane records queries, and restart to ensure history/bookmarks persist.
- Open a news item to fetch content, scroll, and ensure subsequent openings use the cached body.
- Load a news bookmark that has no cached HTML/content and confirm the details pane clears stale content, resets `news_content_loading`, and allows a fresh content request.

## Screenshots / recordings (if UI changes)
N/A

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
- News mode persists `news_recent_searches.json`, `news_bookmarks.json`, and `news_read_urls.json` (bookmarks loader keeps backward compatibility with old feed item format).
- Settings gain `app_start_mode` (`package`/`news`, alias `start_in_news`), `news_filter_show_arch_news`, `news_filter_show_advisories`, `news_filter_show_pkg_updates`, `news_filter_show_aur_comments`, `news_filter_installed_only`, and `news_max_age_days`; defaults/locales updated to match.
- News content worker caches article bodies and treats AUR package URLs as comment views; filter chips expose clickable rects for mouse-driven toggles.
- News feed cache and last-seen maps persist under `lists/` (`news_feed.json`, `news_seen_pkg_updates.json`, `news_seen_aur_comments.json`); feed reloads from cache until background refresh completes.

## Breaking changes
None.

## Additional context
News feed/event/ UI work touches many files; no breaking changes expected, but configs now include news defaults and new persisted news cache files under the lists directory.

