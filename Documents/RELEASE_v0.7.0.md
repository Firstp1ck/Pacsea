# Release v0.7.0

## What's New

### 📰 Extended News Mode
Pacsea now includes a comprehensive **News Mode** with advanced features:

**News Sources:**
- **Arch Linux News**: Latest announcements and updates from archlinux.org
- **Security Advisories**: Security alerts with severity indicators and affected packages
- **Package Updates**: Track version changes for your installed packages with change detection
- **AUR Comments**: Recent community discussions and feedback

**Smart Features:**
- **Change Detection**: Automatically detects package changes (version, maintainer, dependencies)
- **Offline Support**: Caches package data to disk for offline access and faster loading
- **Background Processing**: Failed requests are automatically retried in the background
- **Streaming Updates**: After initial 50 items, additional news items load automatically
- **AUR Balance**: Ensures AUR packages are always represented alongside official packages

**User Experience:**
- Switch to News mode or set `app_start_mode = news` in settings to start in News mode
- Filter by type (Arch news, advisories, updates, comments)
- Sort by date, title, severity, or unread status (use `Shift+Tab` to cycle through sort modes)
- Mark items as read/unread with `r` key
- Bookmark important items for quick access
- Persistent read/unread tracking across sessions

### ⚡ Performance & Reliability Improvements
- **Smart error handling**: Automatically handles repeated failures gracefully without blocking your workflow
- **Rate limiting**: Prevents server blocking with intelligent request management
- **Smart caching**: Multi-layer caching system reduces bandwidth and speeds up loading
  - Fast in-memory cache for instant access
  - Persistent disk cache for offline access
- **Efficient updates**: Only downloads changed data to minimize bandwidth usage
- **Background retries**: Failed requests are automatically retried in the background
- **Better compatibility**: Improved connection handling for better reliability

### 🔧 Code Quality Improvements
- **Better organization**: Code has been reorganized for improved maintainability
- **Enhanced documentation**: Improved code documentation throughout
- **Security scanning**: Added automated security checks
- **Better logging**: Improved visibility of important operational messages

### 🎨 UI Improvements
- **Enhanced footer**: Multi-line keybinds display for better readability
- **Loading indicators**: Visual feedback during data fetching with informative messages
- **Improved filters**: Better filter chips with clickable areas
- **Extended keybinds**: Shift+char keybind support across all panes and modes
- **Better alignment**: Fixed text wrapping issues in updates window

### 🐛 Bug Fixes
- Fixed updates window text alignment when package names/versions wrap to multiple lines
- Fixed options menu key bindings to match display order in Package and News modes
- Fixed `installed_packages.txt` export to respect `installed_packages_mode` setting
- Fixed alert title showing "Connection issue" instead of "Configuration Directories" for config directory messages
- Fixed Shift+Tab keybind to work in News mode (previously only worked in Package mode)
- Fixed scroll position issues
- Improved AUR comment date filtering (excludes invalid dates)
- Enhanced date parsing to handle various date formats correctly
- Fixed package date fetching for better reliability
- Improved error detection and handling

### 🌍 Internationalization
- Improved config directory alert detection for all languages
- Added translations for config directory alerts in English, German, and Hungarian
- Improved loading messages with full translation support

## Technical Details

This release includes significant improvements to code organization, performance, and reliability.

## Configuration

New settings available:
- `app_start_mode`: Set to "news" to start in News mode (default: "package")
- `news_filter_*`: Toggle filters for Arch news, advisories, updates, AUR updates/comments
- `news_max_age_days`: Maximum age filter for news items (default: unlimited)

## Installation

Update to v0.7.0:

```bash
# For stable release
paru -S pacsea-bin   # or: yay -S pacsea-bin

# For latest from git
paru -S pacsea-git   # or: yay -S pacsea-git
```

## Full Changelog

**Full Changelog**: https://github.com/Firstp1ck/Pacsea/compare/v0.6.2...v0.7.0

