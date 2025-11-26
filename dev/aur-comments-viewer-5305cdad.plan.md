<!-- 5305cdad-57cd-4af2-be7e-c58a451720bd 48482e8c-19a1-4b82-9666-6246f52a1dd7 -->
# AUR Package Comments Viewer Implementation Plan

## Overview

Add the ability to view AUR package comments by web scraping the AUR package page. Comments will be displayed in a toggleable pane that splits the Package Info area horizontally, accessible via a keybind and a "Show comments" button.

## Architecture

### Data Flow

1. User toggles comments view (keybind or button click)
2. Background worker fetches HTML from `https://aur.archlinux.org/packages/<pkgname>`
3. HTML is parsed to extract comment elements (author, date, content)
4. Comments are mapped to a data struct and sent via channel
5. UI renders comments in a scrollable List widget
6. Comments are cached in memory with timestamp to avoid repeated fetches

### Key Components

#### 1. Dependencies

- Add `scraper` crate to `Cargo.toml` for HTML parsing (lightweight, CSS selector-based)
- `reqwest` is already available

#### 2. State Management (`src/state/app_state/mod.rs`)

Add comments-related fields to `AppState`:

- `comments_visible: bool` - Toggle state for comments pane
- `comments_button_rect: Option<(u16, u16, u16, u16)>` - Button hit-test rect
- `comments: Vec<AurComment>` - Cached comments data
- `comments_package_name: Option<String>` - Package name for current comments
- `comments_fetched_at: Option<Instant>` - Timestamp for cache invalidation
- `comments_scroll: u16` - Scroll offset for comments list
- `comments_rect: Option<(u16, u16, u16, u16)>` - Content rectangle when visible
- `comments_loading: bool` - Loading state
- `comments_error: Option<String>` - Error message if fetch fails

#### 3. Data Structures (`src/state/types.rs`)

Add `AurComment` struct:

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AurComment {
    pub author: String,
    pub date: String,
    pub content: String,
}
```

#### 4. HTML Scraping (`src/sources/comments.rs`)

Create new module:

- `fetch_aur_comments(pkgname: String) -> Result<Vec<AurComment>>`
  - Uses `reqwest` to GET the AUR package page
  - Uses `scraper` to parse HTML and select comment elements
  - Extracts author, date, and content text
  - Returns parsed comments or error

#### 5. Background Worker (`src/app/runtime/workers/comments.rs`)

Create worker similar to PKGBUILD worker:

- Spawns async task listening to comments request channel
- Calls `fetch_aur_comments` in background
- Sends results via response channel
- Handles errors gracefully (sends error message instead of panicking)

#### 6. Channels (`src/app/runtime/channels.rs`)

Add to `Channels` struct:

- `comments_req_tx: mpsc::UnboundedSender<String>` - Request channel (package name)
- `comments_res_tx: mpsc::UnboundedSender<Result<Vec<AurComment>, String>>` - Response channel

#### 7. Layout Updates (`src/ui/details/layout.rs`)

Modify `calculate_layout_areas` to handle three-way split:

- When only PKGBUILD visible: Package info 50%, PKGBUILD 50% (current)
- When only comments visible: Package info 50%, Comments 50%
- When both visible: Package info 50%, remaining 50% split between PKGBUILD and Comments (25% each)
- Update to use `Layout` with appropriate constraints

#### 8. UI Rendering (`src/ui/details/comments.rs`)

Create new module:

- `render_comments(f: &mut Frame, app: &mut AppState, area: Rect)`
  - Renders scrollable List of comments
  - Uses `ratatui::widgets::List` with `ListItems`
  - Styles author/date differently using `Text`/`Line`
  - Shows loading state or error message
  - Records content rect for hit-testing

#### 9. Package Info Updates (`src/ui/details/package_info.rs`)

- Add "Show comments" / "Hide comments" button text below PKGBUILD button
- Calculate `comments_button_rect` similar to `pkgb_button_rect`
- Update button calculation to include comments button

#### 10. Event Handlers

- **Global keybind** (`src/events/global.rs`): Add `handle_toggle_comments` function
- **Mouse handler** (`src/events/mouse/details.rs`): Handle clicks on comments button
- **Keybind config**: Add `comments_toggle` to keymap (suggest `Ctrl+C` or similar)

#### 11. Integration (`src/ui/details/mod.rs`)

- Import and call `render_comments` when `app.comments_visible` is true
- Update layout calculation to pass comments area rect

#### 12. Caching Strategy

- Cache comments in memory only (per user preference)
- Use `comments_fetched_at` timestamp to avoid refetching within same session
- Clear cache when package changes or on explicit reload
- Cache key: package name

## Implementation Files

### New Files

- `src/sources/comments.rs` - HTML scraping logic
- `src/app/runtime/workers/comments.rs` - Background worker
- `src/ui/details/comments.rs` - Comments rendering

### Modified Files

- `Cargo.toml` - Add `scraper` dependency
- `src/state/types.rs` - Add `AurComment` struct
- `src/state/app_state/mod.rs` - Add comments state fields
- `src/state/app_state/defaults.rs` - Add default comments state
- `src/app/runtime/channels.rs` - Add comments channels
- `src/app/runtime/mod.rs` - Spawn comments worker
- `src/ui/details/layout.rs` - Update layout for three-way split
- `src/ui/details/package_info.rs` - Add comments button
- `src/ui/details/mod.rs` - Integrate comments rendering
- `src/events/global.rs` - Add toggle handler
- `src/events/mouse/details.rs` - Add button click handler
- `src/theme/settings/mod.rs` - Add comments toggle keybind config

## Error Handling

- Network errors: Show "Failed to fetch comments" message in comments pane
- Parse errors: Show "Failed to parse comments" message
- Empty comments: Show "No comments yet" message
- Non-AUR packages: Disable/hide comments button for official packages

## Testing Considerations

- Unit tests for HTML parsing (use sample HTML fixtures)
- Integration test for worker channel communication
- UI rendering test for comments list
- Error state rendering test

## CSS Selectors for AUR Comments

Based on AUR website structure, comments are typically in:

- Container: `.comment-list` or similar
- Individual comment: `.comment` or `article.comment`
- Author: `.comment-author` or `strong` within comment
- Date: `.comment-date` or timestamp element
- Content: `.comment-content` or main text area

Note: Actual selectors need to be verified by inspecting the AUR website HTML structure.

## Sorting

- Comments are sorted by date in descending order (latest first)
- Sorting occurs in `fetch_aur_comments` after parsing all comments
- The `AurComment` struct should include a `date_timestamp: Option<i64>` field for reliable chronological sorting
- Parse date strings to Unix timestamps when possible for accurate sorting
- If date parsing fails (timestamp is None), fall back to string comparison or place at end of list

### To-dos

- [ ] Add scraper crate to Cargo.toml for HTML parsing
- [ ] Add AurComment struct to src/state/types.rs with author, date, content fields
- [ ] Add comments state fields to AppState (visible, button_rect, comments, scroll, etc.)
- [ ] Create src/sources/comments.rs with fetch_aur_comments function using reqwest + scraper
- [ ] Create src/app/runtime/workers/comments.rs background worker for async fetching
- [ ] Add comments request/response channels to Channels struct and spawn worker
- [ ] Update layout.rs to handle Package Info + Comments split (and three-way when both PKGBUILD and comments visible)
- [ ] Create src/ui/details/comments.rs to render scrollable comments list with List widget
- [ ] Add Show comments button to package_info.rs below PKGBUILD button
- [ ] Integrate comments rendering in details/mod.rs when comments_visible is true
- [ ] Add toggle handlers in events/global.rs and events/mouse/details.rs for keybind and button clicks
- [ ] Add comments_toggle keybind to theme settings and keymap
- [ ] Disable/hide comments button for official repository packages (AUR only)