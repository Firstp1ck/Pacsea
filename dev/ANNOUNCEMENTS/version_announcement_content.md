## What's New

- New `privilege_tool` setting: `auto` | `sudo` | `doas`
- Commands now run through the selected tool (or auto-detected one) instead of always using sudo
- New `auth_mode` setting: `prompt` | `passwordless_only` | `interactive`
- Interactive mode hands off to the terminal so sudo/doas can handle PAM prompts directly (including fingerprint via fprintd, when configured)
- Detects the `blackarch` repo and adds a toggle/filter in results when available

