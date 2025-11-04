Below is a concise, actionable plan to integrate both aur-sleuth and ShellCheck into your “scan before install” flow for AUR packages, plus how to expose aur-sleuth in Optional Deps and configure sane OpenAI defaults.

Plan overview
1) Dependencies
- Install at least:
  - shellcheck (for PKGBUILD/.install linting)
  - uv (aur-sleuth dependency)
- Make both aur-sleuth and shellcheck Optional Deps of your package/app so users can opt-in easily.

2) Install aur-sleuth (user or system)
- System:
  - sudo pacman -S uv shellcheck
  - git clone https://github.com/mgalgs/aur-sleuth.git
  - cd aur-sleuth
  - sudo make install
- User-local:
  - sudo pacman -S uv shellcheck
  - git clone https://github.com/mgalgs/aur-sleuth.git
  - cd aur-sleuth
  - make install PREFIX=$HOME/.local  (ensure $HOME/.local/bin is in PATH)

3) Configuration and defaults
- You’ll need OPENAI_API_KEY.
- Set defaults for:
  - OPENAI_BASE_URL = https://api.openai.com/v1
  - OPENAI_MODEL = gpt-5-mini
- Put these in:
  - system-wide: /etc/aur-sleuth.conf
  - user-only: ~/.config/aur-sleuth.conf
- OpenAI API base URL (clickable): https://api.openai.com/v1

4) Integrate into your pre-install scan
- Use a makepkg wrapper that:
  - Runs ShellCheck on PKGBUILD and any *.install or *.sh files
  - Runs aur-sleuth audit (fatal on failure by default; configurable)
  - Falls back gracefully if tools are missing
- Wire this wrapper into yay (or your flow) so the audit runs automatically before build/install.

5) Document how to use
- Show how to enable the wrapper in yay:
  - yay --makepkg makepkg-with-sleuthing --save  (or your enhanced wrapper path)
- Document env toggles:
  - SHELLCHECK_FATAL=1 to make ShellCheck failures fatal
  - AUDIT_FAILURE_FATAL=false to allow install after aur-sleuth warnings

6) Test
- Test with a safe package (e.g., hello-style packages) and audit-only flows.
- Test with edge cases (packages that embed shell scripts, .install hooks, or custom sources).
- Validate behavior when API key is missing or ShellCheck not installed.

Optional Deps snippet (PKGBUILD)
```/dev/null/PKGBUILD#L1-8
optdepends=(
  'aur-sleuth: LLM-based security audit before building AUR packages (requires OPENAI_API_KEY)'
  'shellcheck: Lints PKGBUILD and install scripts before build'
  'uv: Runtime manager used by aur-sleuth'
)
```

Recommended aur-sleuth config (OpenAI defaults)
- System-wide:
```/dev/null/aur-sleuth.conf#L1-8
[default]
OPENAI_API_KEY = your-openai-key
OPENAI_BASE_URL = https://api.openai.com/v1
OPENAI_MODEL = gpt-5-mini
MAX_LLM_JOBS = 3
AUDIT_FAILURE_FATAL = true
```
- User-local:
```/dev/null/.config/aur-sleuth.conf#L1-8
[default]
OPENAI_API_KEY = your-openai-key
OPENAI_BASE_URL = https://api.openai.com/v1
OPENAI_MODEL = gpt-5-mini
MAX_LLM_JOBS = 3
AUDIT_FAILURE_FATAL = true
```

One-off environment export (if you prefer shell exports)
```/dev/null/bash#L1-5
export OPENAI_API_KEY="your-openai-key"
export OPENAI_BASE_URL="https://api.openai.com/v1"
export OPENAI_MODEL="gpt-5-mini"
```

Enhanced makepkg wrapper that adds ShellCheck before aur-sleuth
- This augments the upstream makepkg-with-sleuthing by running ShellCheck first. Name it something like makepkg-with-sleuthing-plus-shellcheck, place it in PATH, and point yay to it.
```/dev/null/makepkg-with-sleuthing-plus-shellcheck#L1-99
#!/usr/bin/env bash
set -euo pipefail

# Skip audit for certain makepkg modes (same as upstream behavior)
for arg in "$@"; do
  case "$arg" in
    --verifysource|--nobuild|--geninteg|-o|-g)
      exec /usr/bin/makepkg "$@"
      ;;
  esac
done

# 1) ShellCheck stage (lint PKGBUILD and local shell scripts)
if command -v shellcheck >/dev/null 2>&1; then
  files=(PKGBUILD)
  while IFS= read -r -d '' f; do files+=("$f"); done < <(find . -maxdepth 1 -type f \( -name "*.install" -o -name "*.sh" \) -print0)
  if [ "${#files[@]}" -gt 0 ]; then
    echo "[shellcheck] Analyzing: ${files[*]}"
    # -x helps follow sourced files; tune severity as desired
    set +e
    shellcheck -S warning -x "${files[@]}"
    sc_rc=$?
    set -e
    if [ $sc_rc -ne 0 ]; then
      echo "[shellcheck] Issues detected."
      if [ "${SHELLCHECK_FATAL:-0}" = "1" ]; then
        echo "[shellcheck] Failing due to SHELLCHECK_FATAL=1"
        exit 1
      else
        echo "[shellcheck] Continuing (set SHELLCHECK_FATAL=1 to fail on lint warnings/errors)."
      fi
    fi
  fi
else
  echo "[shellcheck] Not found. Install with: sudo pacman -S shellcheck"
fi

# 2) aur-sleuth stage (LLM-based audit)
if command -v aur-sleuth >/dev/null 2>&1; then
  # Respect AUDIT_FAILURE_FATAL via aur-sleuth config; default is true.
  # Use plain output so logs are readable in helpers.
  if ! aur-sleuth --pkgdir . --output plain; then
    echo "[aur-sleuth] Audit failed. Edit /etc/aur-sleuth.conf or ~/.config/aur-sleuth.conf to set AUDIT_FAILURE_FATAL=false if you want to proceed anyway."
    exit 1
  fi
else
  echo "[aur-sleuth] Not found. Install aur-sleuth to enable LLM-based auditing."
  echo "           System: sudo make install"
  echo "           User:   make install PREFIX=\$HOME/.local"
fi

# 3) Build/install via makepkg
exec /usr/bin/makepkg "$@"
```

Wire the wrapper into yay
- One-time:
```/dev/null/bash#L1-3
# Use upstream wrapper if preferred:
yay --makepkg makepkg-with-sleuthing --save
# or use the enhanced wrapper with ShellCheck:
yay --makepkg makepkg-with-sleuthing-plus-shellcheck --save
```

Installation commands (as you requested)
```/dev/null/bash#L1-8
sudo pacman -S uv shellcheck
git clone https://github.com/mgalgs/aur-sleuth.git
cd aur-sleuth

# Choose one:
sudo make install                               # System-wide
# or
make install PREFIX=$HOME/.local                # User-local (ensure ~/.local/bin in PATH)
```

Notes pulled from the aur-sleuth repo
- Requires uv (sudo pacman -S uv).
- Supports OpenAI-compatible endpoints, configurable via environment or .conf files:
  - OPENAI_API_KEY (required)
  - OPENAI_BASE_URL (we’re setting default to OpenAI here)
  - OPENAI_MODEL (we’re setting default to gpt-5-mini here)
  - MAX_LLM_JOBS, NUM_FILES_TO_REVIEW, LLM_TEMPERATURE, LLM_TOP_P, AUDIT_FAILURE_FATAL
- Upstream wrapper is makepkg-with-sleuthing (you can use that as-is, or the enhanced version above).
- Repo: https://github.com/mgalgs/aur-sleuth

Quick toggles and behavior
- Make ShellCheck failures fatal: export SHELLCHECK_FATAL=1
- Allow install even if aur-sleuth reports issues: set AUDIT_FAILURE_FATAL=false in config or export AUDIT_FAILURE_FATAL=false
