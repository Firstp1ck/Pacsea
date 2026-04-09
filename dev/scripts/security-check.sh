#!/bin/bash
set -uo pipefail

# Local mirror of .github/workflows/security.yml
# Runs every security check that can run without GitHub APIs.
# Skipped: dependency-review (requires PR context on GitHub)

BOLD='\033[1m'
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
RESET='\033[0m'

PASS=0
FAIL=0
SKIP=0
FAILURES=()

section() { printf '\n%b── %s ──%b\n' "${BOLD}${CYAN}" "$1" "$RESET"; }
pass()    { printf '%b  ✓ %s%b\n' "$GREEN" "$1" "$RESET"; PASS=$((PASS + 1)); }
skip()    { printf '%b  ⊘ %s (not installed — %s)%b\n' "$YELLOW" "$1" "$2" "$RESET"; SKIP=$((SKIP + 1)); }

fail() {
    printf '%b  ✗ %s%b\n' "$RED" "$1" "$RESET"
    FAILURES+=("$1")
    FAIL=$((FAIL + 1))
}

cd "$(git rev-parse --show-toplevel)" || exit

has_cargo_sub() { cargo "$1" --version >/dev/null 2>&1; }

# ── rustfmt ──────────────────────────────────────────────────────────────
section "rustfmt"
if cargo fmt --all -- --check >/dev/null 2>&1; then
    pass "cargo fmt --all -- --check"
else
    fail "cargo fmt: unformatted code detected (run: cargo fmt --all)"
fi

# ── clippy ───────────────────────────────────────────────────────────────
section "clippy"
clippy_output=$(cargo clippy --all-targets --all-features -- -D warnings 2>&1) && clippy_rc=0 || clippy_rc=$?
if [[ $clippy_rc -eq 0 ]]; then
    pass "cargo clippy --all-targets --all-features -- -D warnings"
else
    printf "%s\n" "$clippy_output"
    fail "cargo clippy: lint violations found"
fi

# ── cargo audit ──────────────────────────────────────────────────────────
section "cargo audit"
if has_cargo_sub audit; then
    audit_output=$(cargo audit 2>&1) && audit_rc=0 || audit_rc=$?
    if [[ $audit_rc -eq 0 ]]; then
        pass "cargo audit"
    else
        printf "%s\n" "$audit_output"
        fail "cargo audit: vulnerabilities found"
    fi
else
    skip "cargo audit" "cargo install cargo-audit --locked"
fi

# ── cargo deny ───────────────────────────────────────────────────────────
section "cargo deny"
if has_cargo_sub deny; then
    deny_output=$(cargo deny check 2>&1) && deny_rc=0 || deny_rc=$?
    if [[ $deny_rc -eq 0 ]]; then
        pass "cargo deny check"
    else
        printf "%s\n" "$deny_output"
        fail "cargo deny: policy violations found"
    fi
else
    skip "cargo deny" "cargo install cargo-deny --locked"
fi

# ── gitleaks (secret scanning) ───────────────────────────────────────────
section "gitleaks"
if command -v gitleaks >/dev/null 2>&1; then
    leaks_output=$(gitleaks detect --source . --no-banner --verbose 2>&1) && leaks_rc=0 || leaks_rc=$?
    if [[ $leaks_rc -eq 0 ]]; then
        pass "gitleaks: no secrets found"
    else
        printf "%s\n" "$leaks_output"
        fail "gitleaks: secrets detected in repository"
    fi
else
    skip "gitleaks" "pacman -S gitleaks  OR  https://github.com/gitleaks/gitleaks#installing"
fi

# ── summary ──────────────────────────────────────────────────────────────
section "Summary"
printf '  %bpassed: %d%b  %bfailed: %d%b  %bskipped: %d%b\n' \
    "$GREEN" "$PASS" "$RESET" \
    "$RED" "$FAIL" "$RESET" \
    "$YELLOW" "$SKIP" "$RESET"

if [[ ${#FAILURES[@]} -gt 0 ]]; then
    printf '\n%b%bFailures:%b\n' "$RED" "$BOLD" "$RESET"
    for f in "${FAILURES[@]}"; do
        printf '%b  • %s%b\n' "$RED" "$f" "$RESET"
    done
fi

if [[ $SKIP -gt 0 ]]; then
    printf '\n%bInstall missing tools:%b\n' "$YELLOW" "$RESET"
    has_cargo_sub audit || printf '  cargo install cargo-audit --locked\n'
    has_cargo_sub deny  || printf '  cargo install cargo-deny --locked\n'
    command -v gitleaks    >/dev/null 2>&1 || printf '  pacman -S gitleaks\n'
fi

printf '\n'
exit "$FAIL"
