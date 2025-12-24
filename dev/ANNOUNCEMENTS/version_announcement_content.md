## What's New

### News Mode Enhancements
- **Separated search inputs**: News mode and Package mode now have independent search fields
  - No more shared state issues when switching between modes
  - Search text is preserved when switching modes
- **Improved mark-as-read behavior**: Mark read actions (`r` key) now only work in normal mode
  - Prevents accidental marking when typing 'r' in insert mode
  - More consistent with vim-like behavior

### Toast Notifications
- Improved toast clearing logic for better user experience
- Enhanced toast title detection for news, clipboard, and notification types
- Added notification title translations

### UI Polish
- Sort menu no longer auto-closes (stays open until you select an option or close it)
- Added `change_sort` keybind to help footer in News mode
- Fixed help text punctuation for better readability
