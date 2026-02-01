# Using the User's Terminal Theme When No theme.conf Is Set

This document outlines possibilities and considerations for using the user's terminal color theme when no theme is set in `theme.conf`, so Pacsea can blend with the terminal palette instead of forcing a built-in theme.

## Current Behavior

- Theme is loaded from `theme.conf` (or legacy `pacsea.conf`) via `resolve_theme_config_path()`.
- If no config exists or the file is empty, a **default skeleton** (Catppuccin Mocha) is written to `config_dir()/theme.conf` and loaded.
- The app requires **16 semantic colors**: `base`, `mantle`, `crust`, `surface1`, `surface2`, `overlay1`, `overlay2`, `text`, `subtext0`, `subtext1`, `sapphire`, `mauve`, `green`, `yellow`, `red`, `lavender`.
- All colors are `ratatui::style::Color::Rgb(r, g, b)`; there is no current path for "use terminal default."

## What “No Theme Set” Could Mean

| Option | Description | Pros / Cons |
|--------|-------------|-------------|
| **A. No theme.conf file** | If `theme.conf` (and legacy paths) do not exist, use terminal theme instead of creating a skeleton. | Simple trigger; may surprise users who expect a file to be created. |
| **B. Empty theme.conf** | If the file exists but is empty (or only comments), use terminal theme. | Clear “I didn’t configure anything” signal. |
| **C. Explicit key** | New key in `theme.conf` or `settings.conf`, e.g. `use_terminal_theme = true`. When set, ignore file colors and use terminal. | Explicit user choice; works even when file exists with other keys. |
| **D. Partial override** | File defines only some keys; missing keys fall back to terminal (or derived). | Flexible; more complex to implement and document. |

Recommendation: **B** or **C** are the least ambiguous. **C** is best if we want “terminal theme” to be a first-class option even when the user has a theme file.

## OSC Sequences Required for Supported Terminals

For a terminal to be listed as **supported** for the “use terminal theme” feature, it must respond to the following **query** sequences (not only support *setting* colors).

### Required (minimum for terminal-theme support)

| OSC | Name | Query sequence (bytes) | Expected reply |
|-----|------|-------------------------|----------------|
| **OSC 10** | VT100 text foreground | `\033]10;?\033\\` (ESC ] 1 0 ; ? ESC \\) | `\033]10;rgb:rrrr/gggg/bbbb\033\\` or `\033]10;rgba:rrrr/gggg/bbbb/aaaa\033\\` |
| **OSC 11** | VT100 text background | `\033]11;?\033\\` (ESC ] 1 1 ; ? ESC \\) | `\033]11;rgb:rrrr/gggg/bbbb\033\\` or `\033]11;rgba:rrrr/gggg/bbbb/aaaa\033\\` |

- **OSC** = `\033]` (ESC + ]).
- **ST** (string terminator) = `\033\\` (ESC + \\) or `\007` (BEL); terminals may use either.
- **Reply format**: `rgb:rrrr/gggg/bbbb` with four hex digits per component (e.g. `rgb:0000/0000/0000` for black). Some terminals use `rgba:...` with an alpha field; the app can ignore alpha.
- The terminal must **echo the reply** to the application’s input (e.g. stdin) so the app can read and parse it. Support for *query* (the `?`) is required; supporting only *set* (e.g. `OSC 10;#ff0000 ST`) is not sufficient.

### Optional (for richer palette use)

| OSC | Name | Query sequence (bytes) | Expected reply |
|-----|------|-------------------------|----------------|
| **OSC 4 ; c ; ?** | Color palette index `c` (0–255) | `\033]4;c;?\033\\` | `\033]4;c;rgb:rrrr/gggg/bbbb\033\\` |

- If supported, the app can read specific palette entries (e.g. indices 0–15) and map them to Pacsea’s semantic colors. Not required for “supported” status if OSC 10/11 are implemented.

### Summary for terminal implementers

- **Required**: Respond to **OSC 10 ; ?** (foreground) and **OSC 11 ; ?** (background) by sending back the current fg/bg in `rgb:rrrr/gggg/bbbb` form so the application can read it from the same channel (e.g. stdin).
- **Optional**: Respond to **OSC 4 ; c ; ?** for palette index `c` to allow use of the 256-color palette.

## Ways to Obtain Terminal Colors

### 1. OSC 10 / OSC 11 (foreground / background)

- **OSC 10**: VT100 text foreground.
- **OSC 11**: VT100 text background.
- **Query**: Send `\033]10;?\033\\` and `\033]11;?\033\\`; terminal may reply with e.g. `OSC 10;rgb:rrrr/gggg/bbbb ST`.
- **Support**: Common in xterm, many modern terminals (Alacritty, Kitty, WezTerm, etc.). Not all terminals support *query* (e.g. some only support *set*). Terminal.app / iTerm have limited or different support.
- **Limitation**: Only two colors (fg + bg). Pacsea needs 16; the rest must be derived or use another source.

### 2. OSC 4 (256-color palette)

- **OSC 4 ; c ; ? ST**: Query color index `c` (0–255). Can be used to read the 256-color palette.
- **Support**: Not universally implemented for *query*; behavior varies by terminal.
- **Use**: Could map semantic roles to palette indices (e.g. 0–7 for backgrounds, 8–15 for text, etc.) if we assume a conventional palette, or read a subset of indices and map to Pacsea’s 16 roles.

### 3. Crossterm / Ratatui

- **Crossterm** (current backend) focuses on *setting* attributes/colors, not *querying* terminal colors. No built-in API for OSC 10/11 or palette query.
- **Ratatui** uses `Color::Reset`, `Color::Indexed(u8)`, etc. Using `Color::Reset` or indexed colors would “use terminal default” for those cells, but Pacsea’s theme is currently a full RGB palette; switching to Reset/Indexed for “terminal theme” would require a different code path (e.g. “terminal theme mode” that uses Reset/Indexed instead of `Theme` RGB).

### 4. Custom OSC query implementation

- Implement query by:
  1. Putting the terminal in raw/canonical mode as needed.
  2. Sending OSC 10/11 (and optionally OSC 4) query sequences.
  3. Reading stdin with a timeout and parsing replies (e.g. `rgb:rrrr/gggg/bbbb`).
- **Challenges**: Async/timing (reply can be interleaved with other input), portability (not all terminals reply), and TTY vs non-TTY (e.g. when running under a pager or IDE). Needs a clear fallback when query fails or times out.

## Mapping Terminal Colors to Pacsea’s 16 Roles

- **Only fg/bg available (OSC 10/11)**  
  - Use **background** for `base`, and derive `mantle`/`crust` by darkening (e.g. shade/tint).  
  - Use **foreground** for `text`; derive `subtext0`/`subtext1` by reducing contrast (e.g. blend with background).  
  - For surfaces/overlays: derive from background (lighter) or foreground (muted).  
  - For accents (sapphire, mauve, green, yellow, red, lavender): either use a small fixed set of offsets from fg/bg, or fall back to a minimal built-in palette (e.g. ANSI-like) when “terminal theme” is on.

- **256-color palette available (OSC 4)**  
  - Assume a conventional mapping (e.g. indices 0, 8 for background/text, 1–6 for accents).  
  - Risk: palette layout differs across themes; mapping may look wrong on some setups.

- **Hybrid**  
  - Prefer OSC 10/11 for bg/fg and primary text; use derived or fixed colors for the rest, with an option to “prefer terminal palette” if we implement OSC 4 query.

## Implementation Options

| Approach | Description | Complexity | Notes |
|----------|-------------|-------------|--------|
| **1. Query at startup** | Before loading theme.conf (or when `use_terminal_theme = true`), run OSC 10/11 query, parse reply, build a `Theme` from fg/bg + derivation rules. | Medium | Requires raw stdin handling, timeout, fallback to skeleton/default on failure. |
| **2. Use Reset / Indexed in “terminal mode”** | When “use terminal theme” is set, do not fill a full RGB `Theme`; instead, use `Color::Reset` or `Color::Indexed` in the UI where theme colors are used. | High | Large UI change: two code paths (theme-based RGB vs terminal Reset/Indexed). |
| **3. Optional feature flag** | Implement terminal query and derivation behind a feature (e.g. `terminal-theme`), default off. | Low | Keeps default behavior unchanged; allows testing and gradual rollout. |
| **4. Fallback chain** | 1) If `use_terminal_theme = true` or theme.conf empty → try terminal query; 2) On failure → use built-in default (skeleton or minimal palette). | Medium | Same as 1 with explicit fallback; avoids exiting on query failure. |

Recommendation: **1 + 4** (query at startup with fallback), optionally behind **3** initially. **2** is a larger refactor and only worth it if we want true “inherit terminal for every cell” rather than “build a Theme from terminal fg/bg.”

## Considerations

### Terminal / environment support

- Not all terminals support **querying** OSC 10/11; some only support setting. Query must be best-effort with a short timeout and fallback.
- In non-TTY environments (pipes, CI, IDE terminals), query may hang or return nothing; fallback to default theme is essential.
- Document which terminals are known to work (e.g. xterm, Alacritty, Kitty, WezTerm) and that “terminal theme” may fall back to default when unsupported. Terminals listed as supported must implement the OSC sequences described in **OSC Sequences Required for Supported Terminals** above.

### Reload behavior

- `reload_theme()` currently reloads from file. If “terminal theme” is active, reload could mean “re-query terminal colors” (useful after terminal theme change) or “re-read config and maybe switch back to file theme.” Behavior should be defined and documented.

### Testing and CI

- Unit tests cannot rely on a real TTY or OSC replies. Terminal-theme path should be testable with:
  - Injected “mock” fg/bg (e.g. derivation from two RGB values), and/or
  - Feature off in CI so default theme path is used.

### Backward compatibility

- Existing installs have `theme.conf` with 16 keys. No change for them.
- New installs: if we add “no theme set” → terminal theme, we must either not create a skeleton when using terminal theme (so theme.conf may be missing) or create a commented skeleton that explains `use_terminal_theme = true`.

### Config surface

- If we add `use_terminal_theme = true`, decide: theme.conf only, or also in settings.conf (since it’s a “preference”). theme.conf is the natural place so all theme-related choices stay together.

### Documentation

- Explain in comments (and optionally wiki) that “use terminal theme” uses the terminal’s foreground/background and derives other colors, and that support depends on the terminal. Point to fallback behavior when query is not supported.

## Summary

- **Possible**: Use the user’s terminal theme when no theme is set (or when `use_terminal_theme = true`) by querying OSC 10/11, then building Pacsea’s 16-color theme from fg/bg and derivation rules, with a fallback to the current default/skeleton when query fails or is unsupported.
- **Recommended trigger**: Empty theme.conf or an explicit `use_terminal_theme = true` in theme.conf.
- **Recommended implementation**: Query at startup when terminal theme is requested; on success build `Theme` from fg/bg + derivation; on failure or timeout use existing default theme. Optionally gate behind a feature flag until behavior is stable.
- **Not recommended** (for a first version): Full “use terminal palette for every role” via Reset/Indexed, due to UI complexity and terminal palette variability.

---

## Implementation (Completed)

The terminal theme fallback feature has been implemented following the recommendations above. Here are the details:

### Configuration

- **Setting**: `use_terminal_theme` in `settings.conf` (default: `false`)
- **Aliases**: `terminal_theme` is accepted as an alias for compatibility
- When `true`, Pacsea will attempt to use terminal colors even if `theme.conf` has a valid theme
- When `false` (or unset), terminal colors are only used as a fallback when `theme.conf` is missing, empty, or invalid

### Supported Terminals

The following terminals are recognized as supporting OSC 10/11 queries:

| Terminal | Detection Method |
|----------|------------------|
| Alacritty | `TERM_PROGRAM` env, parent process name |
| Kitty | `TERM_PROGRAM` env, parent process name |
| Konsole | `COLORTERM` env, parent process name |
| Ghostty | `TERM_PROGRAM` env, parent process name |
| xterm | `TERM_PROGRAM` env, parent process name |
| gnome-terminal | Parent process name |
| xfce4-terminal | Parent process name |
| tilix | Parent process name |
| mate-terminal | Parent process name |

**Detection priority**:
1. `TERM_PROGRAM` environment variable
2. `TERM` environment variable (most reliable - e.g., "alacritty", "xterm-256color")
3. `COLORTERM` environment variable (if not just "truecolor" or "24bit")
4. Parent process name (Linux only, via `/proc`)

### Resolution Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                    Theme Resolution Flow                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. Try load theme.conf                                          │
│     ↓                                                            │
│  2. Valid theme from file?                                       │
│     ├─ YES + use_terminal_theme=false → Use file theme           │
│     ├─ YES + use_terminal_theme=true  → Go to step 3             │
│     └─ NO                             → Go to step 3             │
│     ↓                                                            │
│  3. Terminal supported?                                          │
│     ├─ YES → Query OSC 10/11                                     │
│     │         ├─ Success → Use terminal-derived theme            │
│     │         └─ Failure → Go to step 4                          │
│     └─ NO  → Go to step 4                                        │
│     ↓                                                            │
│  4. Valid theme from file?                                       │
│     ├─ YES → Use file theme                                      │
│     └─ NO  → Use codebase default (Catppuccin Mocha)             │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Color Derivation

When using terminal theme, the 16 semantic colors are derived from the foreground and background:

- **Background colors** (base, mantle, crust): Derived from terminal background with varying darkness
- **Surface colors** (surface1, surface2): Interpolated between background and foreground
- **Overlay colors** (overlay1, overlay2): Further interpolated toward foreground
- **Text colors** (text, subtext0, subtext1): Terminal foreground with varying opacity toward background
- **Accent colors** (sapphire, mauve, green, yellow, red, lavender): Fixed Catppuccin-inspired palette

**Light/Dark detection**: Based on background luminance calculation (`0.299*R + 0.587*G + 0.114*B`). Luminance > 127 = light theme, otherwise dark theme.

### OSC Query Details

- **Timeout**: 150ms for reading terminal response
- **Query format**: `\033]10;?\007` for foreground, `\033]11;?\007` for background
- **Response parsing**: Handles `rgb:RRRR/GGGG/BBBB` format (4 hex digits per channel)
- **Raw mode**: Uses crossterm to enter raw terminal mode during query

### Files Modified/Created

| File | Description |
|------|-------------|
| `src/theme/types.rs` | Added `use_terminal_theme: bool` field to Settings |
| `src/theme/settings/parse_settings.rs` | Added parsing for `use_terminal_theme` |
| `src/theme/config/skeletons.rs` | Added skeleton entry for the setting |
| `src/theme/config/settings_ensure.rs` | Added getter for the setting |
| `src/theme/terminal_detect.rs` | **New** - Terminal detection logic |
| `src/theme/terminal_query.rs` | **New** - OSC query and theme derivation |
| `src/theme/resolve.rs` | **New** - Unified resolution logic |
| `src/theme/store.rs` | Updated to use resolution logic |
| `src/theme/mod.rs` | Added new module declarations |
| `src/events/global.rs` | Fixed reload order (settings before theme) |
| `config/settings.conf` | Added example setting |

### Testing

Unit tests cover:
- Terminal detection for all 9 supported terminals
- Color parsing from OSC response strings
- Theme derivation from fg/bg colors
- Skeleton theme parsing
- Resolution logic branches

### Usage

To enable terminal theme:

```conf
# In ~/.config/pacsea/settings.conf
use_terminal_theme = true
```

Or simply delete/empty `theme.conf` when using a supported terminal - Pacsea will automatically use the terminal's colors.
