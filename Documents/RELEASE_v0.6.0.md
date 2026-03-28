# Release v0.6.0

## Integrated Process Execution

All operations now execute directly within the TUI instead of spawning external terminals. Command output is displayed in real-time with live streaming.

### Features

**Command Execution**
- Operations execute within the TUI with live output streaming
- Real-time progress display with auto-scrolling log panel
- Password prompts appear inline when sudo authentication is required

**Supported Operations**
- Package installation (official repositories and AUR packages)
- Package removal with cascade modes (Basic, Cascade, Cascade with Configs)
- System updates (mirror updates, database updates, AUR updates, cache cleanup)
- Security scans (ClamAV, Trivy, Semgrep, ShellCheck, VirusTotal, custom patterns)
- File database sync
- Optional dependency installation
- Preflight modal execution

**Password Authentication**
- Integrated password prompt modal for sudo operations
- Password validation and faillock lockout detection

### Technical Notes

- Command output streams in real-time via PTY
- Progress bars display during package downloads and installations
- Windows compatibility maintained with conditional compilation

### Changes

* Translation: Update hu.yml by @summoner001 in https://github.com/Firstp1ck/Pacsea/pull/77
* Translation: Update hungarian hu.yml by @summoner001 in https://github.com/Firstp1ck/Pacsea/pull/78
* Feat/integrated process by @Firstp1ck in https://github.com/Firstp1ck/Pacsea/pull/76

**Full Changelog**: https://github.com/Firstp1ck/Pacsea/compare/v0.5.3...v0.6.0

