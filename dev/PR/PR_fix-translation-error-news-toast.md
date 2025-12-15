## Summary
- Fixed toast title detection for news messages in non-English locales
- Changed from English keyword matching (`msg.contains("news")`) to language-agnostic message comparison
- News toast now correctly displays "Hírek" (News) title in Hungarian instead of incorrectly showing "Vágólap" (Clipboard)
- Solution checks the toast message against a list of all known news-related translation keys, making it robust and extensible
- Future news-related toasts can be easily added to the list without code changes elsewhere

## Type of change
- [x] fix (bug fix)

## Related issues
Closes #103

## How to test
List exact steps and commands to verify the change. Include flags like `--dry-run` when appropriate.

```bash
# Build and format check
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo check
cargo test -- --test-threads=1

# Test with Hungarian locale
LANG=hu-HU.UTF-8 cargo run

# In the TUI:
# 1. Wait for automatic news check (or trigger via Options → News mode)
#    - Should see toast with title "Hírek" (News) when no new news
#    - Message: "Ma nincsenek új hírek"
# 2. Copy something to clipboard (e.g., PKGBUILD)
#    - Should see toast with title "Vágólap" (Clipboard)
#    - Message: "Másolva a vágólapra" or similar
```

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

**Compatibility:**
- [x] Changes respect `--dry-run` flag
- [x] Code degrades gracefully if `pacman`/`paru`/`yay` are unavailable
- [x] No breaking changes (or clearly documented if intentional)

**Other:**
- [x] Not a packaging change for AUR (otherwise propose in `pacsea-bin` or `pacsea-git` repos)

## Notes for reviewers
- The fix replaces language-specific keyword matching with a language-agnostic approach
- Instead of checking if the message contains "news" (English), we now check the toast message against a list of all known news-related translation keys
- This ensures the toast title detection works correctly for all locales, not just English
- The solution is robust and extensible: new news-related toasts can be added by simply adding their translation key to the `news_keys` array
- The approach is language-agnostic because it compares the actual translated text, not hardcoded keywords
- No changes to AppState structure were needed - the fix is purely in the rendering logic

## Breaking changes
None. This is a bug fix that improves internationalization support.

## Additional context
**Problem:** The toast title detection logic in `render_toast()` used `msg.to_lowercase().contains("news")` to determine if a toast was a news toast. This worked for English but failed for other languages like Hungarian, where "Ma nincsenek új hírek" (No new news today) doesn't contain the English word "news".

**Solution:** Changed the logic to check the toast message against a list of all known news-related translation keys. This is language-agnostic and extensible because:
1. We maintain a list of news-related translation keys (currently `["app.toasts.no_new_news"]`)
2. We iterate through the list and compare the toast message against each translated key
3. If any match is found, the toast is identified as a news toast
4. New news-related toasts can be added by simply adding their key to the array
5. This works for any language without hardcoded keywords

**Implementation details:**
- Uses `news_keys.iter().any()` to check if the message matches any news-related translation
- Each key is translated using `i18n::t()` and compared against the actual toast message
- This approach is robust against future additions of news-related toasts
