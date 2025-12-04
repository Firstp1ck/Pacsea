## Summary
Implements a hybrid announcement system that supports both version-embedded announcements (hardcoded for specific app versions) and remote announcements fetched from a configurable GitHub Gist URL. This allows showing important messages to users at startup, with the ability to mark them as read or dismiss temporarily.

Key features:
- Remote announcements from GitHub Gist with version range filtering and expiration dates
- Version-embedded announcements for release-specific messages (always shown regardless of settings)
- Dynamic modal sizing based on content length and text wrapping
- Persistent read status tracking by announcement ID
- Clickable URLs in announcement content (opens in default browser)
- Boolean setting `get_announcement` to enable/disable remote Gist fetching
- Sequential announcement queue system (embedded → remote → news)
- News items queued to show after all announcements are dismissed

## Type of change
- [x] feat (new feature)
- [ ] fix (bug fix)
- [ ] docs (documentation only)
- [ ] refactor (no functional change)
- [ ] perf (performance)
- [ ] test (add/update tests)
- [ ] chore (build/infra/CI)
- [x] ui (visual/interaction changes)
- [ ] breaking change (incompatible behavior)

## How to test
1. Configure remote announcements in `~/.config/pacsea/settings.conf`:
   ```
   get_announcement = true
   ```
   (Gist URL is hardcoded in the application)

2. Create a Gist with JSON content:
   ```json
   {
     "id": "2025-01-test",
     "title": "Test Announcement",
     "content": "Your announcement content here",
     "min_version": "0.6.0",
     "max_version": null,
     "expires": "2025-12-31"
   }
   ```

3. Run the application:
   ```bash
   cargo run
   ```

4. Verify:
   - Announcement modal appears at startup
   - Title displays from JSON (not hardcoded "Announcement")
   - Modal height adjusts to content length and text wrapping
   - URLs in content are highlighted (mauve, underlined, bold) and clickable
   - Press `r` to mark as read (won't show again)
   - Press `Enter` or `Esc` to dismiss (shows again on next startup)
   - Footer keybinds are always visible with grey color
   - Buffer space exists between content and footer
   - Version announcements always show regardless of `get_announcement` setting
   - Embedded and remote announcements show sequentially (embedded first, then remote)
   - News items show after all announcements are dismissed

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test -- --test-threads=1
cargo run
```

## Checklist

**Code Quality:**
- [x] Code compiles locally (`cargo check`)
- [ ] `cargo fmt --all` ran without changes
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` is clean
- [ ] `cargo test -- --test-threads=1` passes
- [ ] Complexity checks pass for new code (`cargo test complexity -- --nocapture`)
- [x] All new functions/methods have rustdoc comments (What, Inputs, Output, Details)
- [x] No `unwrap()` or `expect()` in non-test code

**Testing:**
- [ ] Added or updated tests where it makes sense
- [ ] For bug fixes: created failing tests first, then fixed the issue
- [ ] Tests are meaningful and cover the functionality

**Documentation:**
- [ ] Updated README if behavior, options, or keybinds changed (keep high-level, reference wiki)
- [ ] Updated relevant wiki pages if needed
- [x] Updated config examples in `config/` directory if config keys changed
- [ ] For UI changes: included screenshots and updated `Images/` if applicable

**Compatibility:**
- [x] Changes respect `--dry-run` flag
- [x] Code degrades gracefully if `pacman`/`paru`/`yay` are unavailable
- [x] No breaking changes (or clearly documented if intentional)

**Other:**
- [x] Not a packaging change for AUR (otherwise propose in `pacsea-bin` or `pacsea-git` repos)

## Notes for reviewers
- The announcement system uses a HashSet to track read announcement IDs, supporting both version-embedded (e.g., "v0.6.0") and remote IDs (e.g., "2025-01-update")
- GitHub Gist raw URLs are cached by CDN; content changes may take 5-10 minutes to propagate
- The `strip_inline_comment` function was updated to preserve `://` in URLs
- Modal height now calculates wrapped line count to properly size for long content
- URLs are detected and styled (mauve, underlined, bold) with click detection via mouse coordinates
- Gist URL is hardcoded in `auxiliary.rs` since it's always the same
- Version announcements are checked independently and always show regardless of `get_announcement` setting
- Announcement queue system ensures embedded announcements show first, then remote announcements, then news items
- Modal restoration logic checks announcement ID to prevent overwriting pending announcements when dismissing embedded announcements

## Breaking changes
- Config key changed from `announcement_url` (string) to `get_announcement` (boolean)
- Default value is `true` (enables remote Gist fetching)
- Gist URL is now hardcoded in the application (no longer configurable)

## Additional context
JSON format for remote announcements:
```json
{
  "id": "unique-announcement-id",
  "title": "Announcement Title",
  "content": "Markdown content with **bold** and ## headers",
  "min_version": "0.6.0",      // optional: minimum app version
  "max_version": null,          // optional: maximum app version  
  "expires": "2025-12-31"       // optional: ISO date for expiration
}
```

Files changed:
- `src/announcements.rs` - New module with announcement types and version matching logic
- `src/ui/modals/announcement.rs` - Modal rendering with dynamic sizing, URL detection, and click tracking
- `src/app/runtime/workers/auxiliary.rs` - Async Gist fetching with hardcoded URL
- `src/app/runtime/event_loop.rs` - Remote announcement handling and queue management
- `src/app/runtime/init.rs` - Version announcement check and queue initialization
- `src/app/runtime/tick_handler.rs` - News queue handling when modal is open
- `src/events/modals/common.rs` - Announcement queue processing and sequential display logic
- `src/events/modals/handlers.rs` - Fixed modal restoration to prevent overwriting pending announcements
- `src/state/modal.rs` - Added title field to Announcement variant
- `src/state/app_state/mod.rs` - Added `pending_announcements` and `pending_news` queues, `announcement_urls` for clickable URL tracking
- `src/state/app_state/defaults.rs` - Initialize announcement and news queues
- `src/state/app_state/default_impl.rs` - Default initialization for queue fields
- `src/events/mouse/modals/simple.rs` - URL click handling in announcement modal
- `src/theme/types.rs` - Changed `announcement_url` to `get_announcement` (boolean)
- `src/theme/settings/parse_settings.rs` - Parse boolean `get_announcement` setting
- `src/theme/parsing.rs` - Fixed URL parsing in config

