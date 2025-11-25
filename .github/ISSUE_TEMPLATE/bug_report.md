---
name: Bug Report
about: Report a bug or unexpected behavior in Pacsea
title: '[BUG] '
labels: 'bug'
assignees: ''
---

## Bug Description
A clear and concise description of what the bug is.

## To Reproduce
Steps to reproduce the behavior:
1. 
2. 
3. 
4. 

## Expected Behavior
A clear and concise description of what you expected to happen.

## Actual Behavior
What actually happened instead.

## Environment Information

**Pacsea Version:**
- Version: `0.5.2` (or commit hash if building from source)
- Installation method: [ ] AUR (`pacsea-bin` or `pacsea-git`) [ ] Built from source

**System Information:**
- Distribution: [e.g., Arch Linux, EndeavourOS, Manjaro, CachyOS, Artix]
- Kernel version: [e.g., `6.x.x`]
- Display server: [ ] Wayland [ ] X11
- Terminal emulator: [e.g., `alacritty`, `kitty`, `xterm`]
- Terminal version: [e.g., `0.13.x`]

**AUR Helper:**
- Helper: [ ] `paru` [ ] `yay` [ ] None
- Version: [e.g., `1.11.x`]

## Logs

**Important**: Please run Pacsea with debug logging enabled and include relevant log output.

```bash
# Run with verbose flag
pacsea --verbose

# Or with debug logging
RUST_LOG=pacsea=debug pacsea
```

**Log location**: `~/.config/pacsea/logs/pacsea.log`

**Relevant log output:**
```
[Paste relevant log output here, especially errors and warnings]
```

**To extract warnings/errors only:**
```bash
grep -iE "(warn|error)" ~/.config/pacsea/logs/pacsea.log
```

## Screenshots
If applicable, add screenshots to help explain the problem. Screenshots are especially helpful for:
- UI rendering issues (misaligned text, broken layout, visual glitches)
- Error messages or dialogs
- Unexpected visual behavior

You can drag and drop images directly into this issue.

## Additional Context
- Does this happen consistently or intermittently?
- When did you first notice this issue?
- Did this work in a previous version? If so, which version?
- Any workarounds you've found?
- Related issues or discussions (if any)

## Checklist
- [ ] I have included all required environment information
- [ ] I have provided logs from running with `--verbose` or `RUST_LOG=pacsea=debug`
- [ ] I have checked the [Troubleshooting guide](https://github.com/Firstp1ck/Pacsea/wiki/Troubleshooting)
- [ ] I have searched existing issues to ensure this hasn't been reported before
