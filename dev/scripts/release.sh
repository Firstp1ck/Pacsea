#!/usr/bin/env bash
# release.sh - Automated version release script for Pacsea
#
# What: Bash equivalent of dev/scripts/release.fish.
#       Automates version bumps, docs, building, release, and AUR publishing.
#
# Usage:
#   ./release.sh [--dry-run] [--pkgrel MODE] [version]
#
# Options:
#   --dry-run    Preview all changes without executing them
#   --pkgrel     How to adjust AUR PKGBUILD pkgrel after setting pkgver:
#                reset — set pkgrel to 1 (use when pkgver bumps)
#                keep  — leave pkgrel unchanged
#                bump  — increment numeric pkgrel by 1 (same pkgver, packaging change)
#                If omitted and stdin is a TTY, you are prompted; else defaults to reset.
#   version      New version (e.g., 0.6.2). If not provided, will prompt.
#
# Layout (optional env overrides; if unset, defaults below are used and
# missing paths trigger interactive prompts):
#   PACSEA_AUR_BIN_DIR   Directory of the pacsea-bin AUR git clone (must contain PKGBUILD)
#   PACSEA_AUR_GIT_DIR   Directory of the pacsea-git AUR git clone (must contain PKGBUILD)
#   PACSEA_WIKI_DIR      Pacsea wiki git clone (optional; empty skip disables wiki push)

set -euo pipefail

# ============================================================================
# Configuration
# ============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACSEA_DIR="$(realpath "${SCRIPT_DIR}/../..")"
DEV_SCRIPTS_DIR="${PACSEA_DIR}/dev/scripts"
AUR_PUSH_SCRIPT="${DEV_SCRIPTS_DIR}/aur-push.sh"
UPDATE_SHA_SCRIPT="${DEV_SCRIPTS_DIR}/update-sha256sums.sh"
AUR_BIN_DIR="${PACSEA_AUR_BIN_DIR:-${HOME}/aur-packages/pacsea-bin}"
AUR_GIT_DIR="${PACSEA_AUR_GIT_DIR:-${HOME}/aur-packages/pacsea-git}"
WIKI_DIR="${PACSEA_WIKI_DIR:-/mnt/SSD_NVME_4TB/GitHub/Pacsea.wiki}"
DRY_RUN=false
# AUR PKGBUILD pkgrel handling in phase 3: reset | keep | bump (set via --pkgrel or prompt)
PKGREL_MODE=""
PKGREL_CLI_SET=false
SKIP_AUR_BIN_PROCESS=false
SKIP_AUR_GIT_PROCESS=false
SKIP_WIKI_PROCESS=false
RELEASE_REPORT_FILE=""
RELEASE_REPORT_MIRRORING_ENABLED=false
declare -a DONE_STEPS=()
declare -a SKIPPED_STEPS=()

# ANSI colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
BOLD='\033[1m'
RESET='\033[0m'
BOLD_CYAN='\033[1;36m'
BOLD_GREEN='\033[1;32m'

# ============================================================================
# Helper Functions
# ============================================================================

log_info() { printf "%b[INFO] %b%s\n" "${BLUE}" "${RESET}" "$*"; }
log_success() { printf "%b[SUCCESS] %b%s\n" "${GREEN}" "${RESET}" "$*"; }
log_warn() { printf "%b[WARN] %b%s\n" "${YELLOW}" "${RESET}" "$*"; }
log_error() { printf "%b[ERROR] %b%s\n" "${RED}" "${RESET}" "$*"; }
mark_done() { DONE_STEPS+=("$*"); }
mark_skipped() { SKIPPED_STEPS+=("$*"); }

initialize_release_report() {
  local report_dir ts
  report_dir="${PACSEA_DIR}/dev/RELEASE"
  mkdir -p "${report_dir}"
  ts="$(date +%Y%m%d-%H%M%S)"
  RELEASE_REPORT_FILE="${report_dir}/release-report-pending-${ts}.txt"
}

write_release_report() {
  local final_status="${1}"
  local done_count skipped_count
  local cleaned_report_file
  [[ -n "${RELEASE_REPORT_FILE}" ]] || return 0

  # Normalize the report at the end to avoid ANSI escape clutter and
  # stream interleaving artifacts from mirrored stdout/stderr.
  cleaned_report_file="$(mktemp)"
  sed -E 's/\x1B\[[0-9;]*[[:alpha:]]//g' "${RELEASE_REPORT_FILE}" > "${cleaned_report_file}"
  mv "${cleaned_report_file}" "${RELEASE_REPORT_FILE}"

  done_count="${#DONE_STEPS[@]}"
  skipped_count="${#SKIPPED_STEPS[@]}"
  cat >> "${RELEASE_REPORT_FILE}" <<EOF

=== RELEASE SUMMARY ===
final_status=${final_status}
completed_steps=${done_count}
skipped_steps=${skipped_count}
EOF

  if (( done_count > 0 )); then
    {
      echo
      echo "[Completed]"
      printf -- "- %s\n" "${DONE_STEPS[@]}"
    } >> "${RELEASE_REPORT_FILE}"
  fi

  if (( skipped_count > 0 )); then
    {
      echo
      echo "[Skipped]"
      printf -- "- %s\n" "${SKIPPED_STEPS[@]}"
    } >> "${RELEASE_REPORT_FILE}"
  fi

  {
    echo
    log_info "Release report written: ${RELEASE_REPORT_FILE}"
  } >/dev/null 2>&1 || true
}

enable_report_mirroring() {
  [[ -n "${RELEASE_REPORT_FILE}" ]] || return 1
  if [[ "${RELEASE_REPORT_MIRRORING_ENABLED}" != true ]]; then
    exec > >(tee -a "${RELEASE_REPORT_FILE}") 2>&1
    RELEASE_REPORT_MIRRORING_ENABLED=true
    echo "Pacsea release log started: $(date -Iseconds)"
    echo "Report file: ${RELEASE_REPORT_FILE}"
    echo
  fi
}

set_release_report_version_in_filename() {
  local ver="${1}"
  local safe_ver new_file
  [[ -n "${RELEASE_REPORT_FILE}" ]] || return 0
  [[ -n "${ver}" ]] || return 0

  safe_ver="${ver//\//-}"
  safe_ver="${safe_ver// /-}"
  new_file="${RELEASE_REPORT_FILE/pending/${safe_ver}}"

  if [[ "${new_file}" != "${RELEASE_REPORT_FILE}" ]]; then
    mv "${RELEASE_REPORT_FILE}" "${new_file}"
    RELEASE_REPORT_FILE="${new_file}"
    log_info "Updated report filename with version: ${RELEASE_REPORT_FILE}"
  fi
}

log_step() {
  echo
  printf "%b━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━%b\n" "${MAGENTA}" "${RESET}"
  printf "%b  STEP: %s%b\n" "${BOLD_CYAN}" "$*" "${RESET}"
  printf "%b━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━%b\n" "${MAGENTA}" "${RESET}"
}

log_phase() {
  echo
  printf "%b════════════════════════════════════════════════════════════════════════%b\n" "${BOLD_GREEN}" "${RESET}"
  printf "%b  PHASE: %s%b\n" "${BOLD_GREEN}" "$*" "${RESET}"
  printf "%b════════════════════════════════════════════════════════════════════════%b\n" "${BOLD_GREEN}" "${RESET}"
}

dry_run_cmd() {
  if [[ "${DRY_RUN}" == true ]]; then
    printf "%b[DRY-RUN] Would execute: %b%s\n" "${YELLOW}" "${RESET}" "$*"
    return 0
  fi
  "$@"
}

confirm_continue() {
  local msg="${1:-Continue?}"
  local response
  while true; do
    printf "%b%s [Y/n]: %b" "${CYAN}" "${msg}" "${RESET}"
    read -r response
    case "${response,,}" in
      ""|y|yes) return 0 ;;
      n|no) return 1 ;;
      *) echo "Please answer y or n" ;;
    esac
  done
}

wait_for_user() {
  local msg="${1:-Press Enter to continue...}"
  printf "%b%s%b" "${CYAN}" "${msg}" "${RESET}"
  read -r
}

validate_semver() {
  [[ "${1}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]
}

wait_for_release_asset() {
  local tag="${1}"
  local asset_name="${2}"
  local max_attempts=15
  local sleep_seconds=8
  local asset_url="https://github.com/Firstp1ck/Pacsea/releases/download/${tag}/${asset_name}"
  local i

  log_info "Checking release asset availability: ${asset_url}"

  for ((i=1; i<=max_attempts; i++)); do
    if curl -fsIL "${asset_url}" >/dev/null 2>&1; then
      log_success "Release asset is available (attempt ${i}/${max_attempts})"
      return 0
    fi
    if [[ "${i}" -lt "${max_attempts}" ]]; then
      log_warn "Asset not ready yet (attempt ${i}/${max_attempts}), retrying in ${sleep_seconds} seconds..."
      sleep "${sleep_seconds}"
    fi
  done

  log_error "Release asset is still unavailable after ${max_attempts} attempts"
  log_error "URL checked: ${asset_url}"
  return 1
}

extract_first_sha256() {
  local pkgbuild_file="${1}"
  [[ -f "${pkgbuild_file}" ]] || return 1
  rg -o --no-filename '[0-9a-f]{64}' "${pkgbuild_file}" | awk 'NR==1{print; exit}'
}

get_current_version() {
  sed -n 's/^version = "\(.*\)"/\1/p' "${PACSEA_DIR}/Cargo.toml" | awk 'NR==1{print; exit}'
}

is_prerelease_version() {
  local ver_str="${1}"
  local major
  major="$(cut -d. -f1 <<<"${ver_str}")"
  [[ "${major}" -lt 1 ]]
}

is_major_or_minor_change() {
  local old_ver="${1}"
  local new_ver="${2}"
  local old_major old_minor new_major new_minor

  old_major="$(cut -d. -f1 <<<"${old_ver}")"
  old_minor="$(cut -d. -f2 <<<"${old_ver}")"
  new_major="$(cut -d. -f1 <<<"${new_ver}")"
  new_minor="$(cut -d. -f2 <<<"${new_ver}")"

  [[ "${old_major}" != "${new_major}" || "${old_minor}" != "${new_minor}" ]]
}

# Expand leading ~ to HOME (no other shell expansion).
expand_tilde() {
  local p="${1}"
  if [[ "${p}" == "~" || "${p}" == ~/* ]]; then
    p="${p/#\~/${HOME}}"
  fi
  printf "%s" "${p}"
}

# ============================================================================
# Phase 1: Version Update
# ============================================================================

phase1_version_update() {
  local new_ver="${1}"
  local current_ver
  current_ver="$(get_current_version)"

  log_phase "1. Version Update"
  printf "%b[INFO] %bCurrent version: %b%s%b\n" "${BLUE}" "${RESET}" "${BOLD}" "${current_ver}" "${RESET}"
  printf "%b[INFO] %bNew version: %b%s%b\n" "${BLUE}" "${RESET}" "${BOLD}" "${new_ver}" "${RESET}"

  log_step "Updating Cargo.toml"
  if [[ "${DRY_RUN}" == true ]]; then
    log_info "[DRY-RUN] Would update version in Cargo.toml from ${current_ver} to ${new_ver}"
  else
    sed -i "s/^version = \"${current_ver}\"/version = \"${new_ver}\"/" "${PACSEA_DIR}/Cargo.toml"
    log_success "Updated Cargo.toml"
  fi

  log_step "Updating Cargo.lock"
  cd "${PACSEA_DIR}"
  dry_run_cmd cargo check
  log_success "Cargo.lock updated"
}

# ============================================================================
# Phase 2: Documentation
# ============================================================================

phase2_documentation() {
  local new_ver="${1}"
  local old_ver="${2}"
  local release_file="${PACSEA_DIR}/Release-docs/RELEASE_v${new_ver}.md"
  local announcement_file="${PACSEA_DIR}/dev/ANNOUNCEMENTS/version_announcement_content.md"

  log_phase "2. Documentation"

  log_step "Generate Release Notes"
  printf "%b[INFO] %bFollow %b.cursor/commands/release-new.md%b (version %s) with your AI/IDE tooling.\n" "${BLUE}" "${RESET}" "${BOLD}" "${RESET}" "${new_ver}"

  if [[ "${DRY_RUN}" == true ]]; then
    log_info "[DRY-RUN] Would wait for release notes generation"
  else
    wait_for_user "After completing the release-new workflow, press Enter..."
    if [[ ! -f "${release_file}" ]]; then
      log_warn "Release file not found at: ${release_file}"
      confirm_continue "Continue anyway?" || return 1
    else
      log_success "Release file created: ${release_file}"
    fi
  fi

  update_changelog "${new_ver}"

  log_step "Auto-generate Announcement"
  if [[ "${DRY_RUN}" == true ]]; then
    log_info "[DRY-RUN] Would generate announcement from release file"
  else
    if [[ -f "${release_file}" ]]; then
      generate_announcement "${release_file}" "${new_ver}"
    else
      log_warn "Skipping announcement generation (no release file)"
    fi
  fi

  log_step "Update Version Announcement in Code"
  if [[ "${DRY_RUN}" == true ]]; then
    log_info "[DRY-RUN] Would run update_version_announcement.py"
  else
    if [[ -f "${announcement_file}" ]]; then
      python3 "${PACSEA_DIR}/dev/scripts/update_version_announcement.py" \
        "${new_ver}" \
        "Version ${new_ver}" \
        --file "${announcement_file}" || {
          log_error "Failed to update announcement"
          confirm_continue "Continue anyway?" || return 1
        }
      log_success "Announcement updated in code"
    else
      log_warn "Announcement file not found, skipping..."
    fi
  fi

  log_step "Update README"
  printf "%b[INFO] %bFollow %b.cursor/commands/readme-update.md%b with your AI/IDE tooling.\n" "${BLUE}" "${RESET}" "${BOLD}" "${RESET}"
  if [[ "${DRY_RUN}" == true ]]; then
    log_info "[DRY-RUN] Would wait for README update"
  else
    wait_for_user "After completing the readme-update workflow, press Enter..."
    log_success "README update complete"
  fi

  log_step "Update Wiki"
  printf "%b[INFO] %bFollow %b.cursor/commands/wiki-update.md%b with your AI/IDE tooling.\n" "${BLUE}" "${RESET}" "${BOLD}" "${RESET}"
  if [[ "${DRY_RUN}" == true ]]; then
    log_info "[DRY-RUN] Would wait for wiki update"
  else
    wait_for_user "After completing the wiki-update workflow, press Enter..."
    log_success "Wiki update complete"
  fi

  if is_major_or_minor_change "${old_ver}" "${new_ver}"; then
    log_step "Update SECURITY.md"
    update_security_md "${new_ver}"
  else
    log_info "Skipping SECURITY.md update (patch release only)"
  fi
}

generate_announcement() {
  local release_file="${1}"
  local output_file="${PACSEA_DIR}/dev/ANNOUNCEMENTS/version_announcement_content.md"
  local tmp_output body_lines char_count

  log_info "Extracting full ## What's New section from release file..."
  body_lines="$(awk '/^##[[:space:]]+What/{sec=1;next} sec&&/^##[[:space:]]/{exit} sec{print}' "${release_file}")"
  body_lines="$(printf "%s" "${body_lines}" | sed '/./,$!d' | sed ':a;/^\n*$/{$d;N;ba};')"

  if [[ -z "${body_lines}" ]]; then
    log_warn "No content found under ## What's New in ${release_file} — wrote header only; edit ${output_file} if needed"
  fi

  tmp_output="$(mktemp)"
  {
    printf "## What's New\n\n"
    printf "%s\n" "${body_lines}"
    printf "\n"
  } > "${tmp_output}"

  char_count="$(wc -c < "${tmp_output}" | xargs)"
  if [[ "${char_count}" -gt 4000 ]]; then
    log_info "Announcement is long (${char_count} bytes); shorten ${output_file} if the startup modal feels crowded"
  fi

  mv "${tmp_output}" "${output_file}"
  log_success "Generated announcement (${char_count} bytes) at: ${output_file}"
  echo
  printf "%b--- Announcement Preview ---%b\n" "${CYAN}" "${RESET}"
  cat "${output_file}"
  printf "%b--- End Preview ---%b\n\n" "${CYAN}" "${RESET}"

  if ! confirm_continue "Accept this announcement?"; then
    log_info "Please edit the file manually..."
    wait_for_user "Press Enter after editing ${output_file}..."
  fi
}

# ============================================================================
# Phase 3: PKGBUILD Updates
# ============================================================================

# What: Apply pkgrel policy to one PKGBUILD after pkgver is set.
# Inputs: $1 = path to PKGBUILD; $2 = mode (reset | keep | bump)
# Output: Mutates pkgrel line unless mode is keep; returns non-zero on bump parse failure.
# Details: reset forces pkgrel=1; bump requires a numeric ^pkgrel= line.
apply_pkgrel_to_pkgbuild() {
  local pkgbuild="${1:?}"
  local mode="${2:?}"
  local current next
  case "${mode}" in
    reset)
      sed -i 's/^pkgrel=.*/pkgrel=1/' "${pkgbuild}"
      ;;
    keep)
      ;;
    bump)
      current="$(grep -m1 -E '^pkgrel=[0-9]+' "${pkgbuild}" | sed 's/^pkgrel=//' || true)"
      if [[ -z "${current}" ]]; then
        log_error "Could not read numeric pkgrel from ${pkgbuild} (needed for bump)"
        return 1
      fi
      next=$((current + 1))
      sed -i "s/^pkgrel=.*/pkgrel=${next}/" "${pkgbuild}"
      ;;
    *)
      log_error "Invalid PKGREL_MODE: ${mode} (expected reset, keep, or bump)"
      return 1
      ;;
  esac
}

phase3_dry_run_pkgrel_msg() {
  case "${PKGREL_MODE}" in
    reset) log_info "[DRY-RUN] Would set pkgrel to 1" ;;
    keep) log_info "[DRY-RUN] Would leave pkgrel unchanged" ;;
    bump) log_info "[DRY-RUN] Would bump pkgrel by 1" ;;
  esac
}

# What: Set PKGREL_MODE when --pkgrel was not passed.
# Inputs: Uses PKGREL_CLI_SET; stdin TTY for interactive menu.
# Output: Sets PKGREL_MODE to reset, keep, or bump.
# Details: Non-TTY stdin defaults to reset with a log line (CI / piped input).
prompt_pkgrel_mode_interactive_if_needed() {
  [[ "${PKGREL_CLI_SET}" == true ]] && return 0
  if [[ -t 0 ]]; then
    echo
    printf "%bAUR PKGBUILD: how should pkgrel change after pkgver is set?%b\n" "${CYAN}" "${RESET}"
    echo "  1) reset — pkgrel=1 (new version release)"
    echo "  2) keep  — leave pkgrel as-is"
    echo "  3) bump  — pkgrel +1 (same pkgver, packaging-only change)"
    while true; do
      printf "%bChoose [1-3] (default 1): %b" "${CYAN}" "${RESET}"
      read -r _pkgrel_choice || return 1
      case "${_pkgrel_choice}" in
        '' | 1) PKGREL_MODE=reset; break ;;
        2) PKGREL_MODE=keep; break ;;
        3) PKGREL_MODE=bump; break ;;
        *)
          log_warn "Enter 1, 2, or 3"
          ;;
      esac
    done
  else
    PKGREL_MODE=reset
    log_info "Non-interactive stdin: using AUR pkgrel mode reset (pass --pkgrel keep|bump|reset to set explicitly)"
  fi
}

phase3_pkgbuild_updates() {
  local new_ver="${1}"
  log_phase "3. PKGBUILD Updates"

  if [[ "${SKIP_AUR_BIN_PROCESS}" == true && "${SKIP_AUR_GIT_PROCESS}" == true ]]; then
    log_warn "Skipping PKGBUILD updates (both AUR processes skipped)."
    mark_skipped "Phase 3: PKGBUILD updates"
    return 0
  fi

  log_step "Update pacsea-bin PKGBUILD"
  if [[ "${SKIP_AUR_BIN_PROCESS}" == true ]]; then
    log_info "Skipping pacsea-bin PKGBUILD update."
    mark_skipped "Phase 3.1: pacsea-bin PKGBUILD update"
  elif [[ "${DRY_RUN}" == true ]]; then
    log_info "[DRY-RUN] Would update pkgver to ${new_ver} in ${AUR_BIN_DIR}/PKGBUILD"
    phase3_dry_run_pkgrel_msg
  else
    [[ -f "${AUR_BIN_DIR}/PKGBUILD" ]] || { log_error "PKGBUILD not found at ${AUR_BIN_DIR}/PKGBUILD"; return 1; }
    sed -i "s/^pkgver=.*/pkgver=${new_ver}/" "${AUR_BIN_DIR}/PKGBUILD"
    apply_pkgrel_to_pkgbuild "${AUR_BIN_DIR}/PKGBUILD" "${PKGREL_MODE}" || return 1
    log_success "Updated ${AUR_BIN_DIR}/PKGBUILD (pkgrel mode: ${PKGREL_MODE})"
    mark_done "Phase 3.1: pacsea-bin PKGBUILD updated"
  fi

  log_step "Update pacsea-git PKGBUILD"
  if [[ "${SKIP_AUR_GIT_PROCESS}" == true ]]; then
    log_info "Skipping pacsea-git PKGBUILD update."
    mark_skipped "Phase 3.2: pacsea-git PKGBUILD update"
  elif [[ "${DRY_RUN}" == true ]]; then
    log_info "[DRY-RUN] Would update pkgver to ${new_ver} in ${AUR_GIT_DIR}/PKGBUILD"
    phase3_dry_run_pkgrel_msg
    log_info "[DRY-RUN] Would remove git commit suffixes"
  else
    [[ -f "${AUR_GIT_DIR}/PKGBUILD" ]] || { log_error "PKGBUILD not found at ${AUR_GIT_DIR}/PKGBUILD"; return 1; }
    sed -i "s/^pkgver=.*/pkgver=${new_ver}/" "${AUR_GIT_DIR}/PKGBUILD"
    apply_pkgrel_to_pkgbuild "${AUR_GIT_DIR}/PKGBUILD" "${PKGREL_MODE}" || return 1
    log_success "Updated ${AUR_GIT_DIR}/PKGBUILD (pkgrel mode: ${PKGREL_MODE})"
    mark_done "Phase 3.2: pacsea-git PKGBUILD updated"
  fi

  log_success "PKGBUILD updates complete"
  mark_done "Phase 3: PKGBUILD updates"
}

# ============================================================================
# Phase 4: Build and Release
# ============================================================================

phase4_build_release() {
  local new_ver="${1}"
  local tag="v${new_ver}"
  local release_file="${PACSEA_DIR}/Release-docs/RELEASE_v${new_ver}.md"

  log_phase "4. Build and Release"
  cd "${PACSEA_DIR}"

  log_step "Running quality and security checks"
  if ! dry_run_cmd cargo fmt --all; then
    log_error "cargo fmt --all failed"
    confirm_continue "Continue despite cargo fmt failure?" || return 1
  fi
  if ! dry_run_cmd cargo clippy --all-targets --all-features -- -D warnings; then
    log_error "cargo clippy failed"
    confirm_continue "Continue despite clippy failure?" || return 1
  fi
  if ! dry_run_cmd cargo test -- --test-threads=1; then
    log_error "cargo test failed"
    confirm_continue "Continue despite test failure?" || return 1
  fi
  if ! dry_run_cmd cargo check; then
    log_error "cargo check failed"
    confirm_continue "Continue despite cargo check failure?" || return 1
  fi
  if ! dry_run_cmd cargo audit; then
    log_error "cargo audit failed"
    confirm_continue "Continue despite cargo audit failure?" || return 1
  fi
  if ! dry_run_cmd cargo deny check; then
    log_error "cargo deny check failed"
    confirm_continue "Continue despite cargo deny failure?" || return 1
  fi
  if ! dry_run_cmd gitleaks detect --source "${PACSEA_DIR}" --no-banner; then
    log_error "gitleaks detect failed"
    confirm_continue "Continue despite gitleaks failure?" || return 1
  fi
  log_success "All quality and security checks completed"

  log_step "Building release binary"
  dry_run_cmd cargo build --release
  log_success "Release binary built"

  log_step "Committing and pushing changes"
  if [[ "${DRY_RUN}" == true ]]; then
    log_info "[DRY-RUN] Would commit all changes with message: Release v${new_ver}"
    log_info "[DRY-RUN] Would push to origin"
  else
    git add -A
    git commit -m "Release v${new_ver}" || log_warn "Nothing to commit or commit failed"
    git push origin
    log_success "Changes pushed to origin"
  fi

  log_step "Creating git tag"
  if [[ "${DRY_RUN}" == true ]]; then
    log_info "[DRY-RUN] Would create tag: ${tag}"
  else
    if git tag -l | rg -q "^${tag}$"; then
      log_warn "Tag ${tag} already exists"
      if confirm_continue "Delete and recreate tag?"; then
        git tag -d "${tag}" || true
        git push origin --delete "${tag}" 2>/dev/null || true
      else
        log_info "Skipping tag creation"
        return 0
      fi
    fi
    git tag "${tag}"
    log_success "Created tag: ${tag}"
  fi

  log_step "Pushing tag to GitHub"
  dry_run_cmd git push origin "${tag}"
  log_success "Tag pushed to GitHub"

  log_step "Creating GitHub Release"
  if [[ "${DRY_RUN}" == true ]]; then
    log_info "[DRY-RUN] Would create GitHub release ${tag} with notes from ${release_file}"
    log_info "[DRY-RUN] Binary will be uploaded by GitHub Action"
  else
    if [[ -f "${release_file}" ]]; then
      if is_prerelease_version "${new_ver}"; then
        log_info "Version < 1.0.0: Creating as prerelease"
        gh release create "${tag}" --title "v${new_ver}" --prerelease --notes-file "${release_file}"
      else
        log_info "Version >= 1.0.0: Creating as stable release"
        gh release create "${tag}" --title "v${new_ver}" --notes-file "${release_file}"
      fi
      log_success "GitHub release created (binary will be uploaded by GitHub Action)"
    fi
  fi

  log_step "Verifying crates.io publish (dry-run)"
  dry_run_cmd cargo publish --dry-run
  log_success "crates.io publish verification passed"

  log_step "Publishing to crates.io"
  if [[ "${DRY_RUN}" == true ]]; then
    log_info "[DRY-RUN] Would run 'cargo publish' to publish to crates.io"
  else
    cargo publish || {
      log_error "Failed to publish to crates.io"
      confirm_continue "Continue anyway?" || return 1
    }
    log_success "Published to crates.io"
  fi
}

# ============================================================================
# Phase 5: AUR Update
# ============================================================================

phase5_aur_update() {
  local new_ver="${1}"
  local tag="v${new_ver}"
  local asset_name="pacsea"
  local pkgbuild_file sha_before sha_after wiki_status

  log_phase "5. AUR Update"

  if [[ "${SKIP_AUR_BIN_PROCESS}" == true && "${SKIP_AUR_GIT_PROCESS}" == true && "${SKIP_WIKI_PROCESS}" == true ]]; then
    log_warn "Skipping phase 5 (AUR bin/git and wiki processes all skipped)."
    mark_skipped "Phase 5: AUR/Wiki updates"
    return 0
  fi

  log_step "Updating AUR SHA sums"

  log_warn "Wait for GitHub Action 'release' to finish uploading the binary!"
  log_info "Check: https://github.com/Firstp1ck/Pacsea/actions"
  wait_for_user "Press Enter when GitHub Action has completed..."

  if [[ "${SKIP_AUR_BIN_PROCESS}" == true ]]; then
    log_info "Skipping AUR SHA sums (pacsea-bin process skipped)."
    mark_skipped "Phase 5.1: AUR SHA update"
  else
    if [[ "${DRY_RUN}" == true ]]; then
      log_info "[DRY-RUN] Would verify release asset availability: ${tag}/${asset_name}"
      log_info "[DRY-RUN] Would verify release asset availability: ${tag}/pacsea-aarch64"
      log_info "[DRY-RUN] Would change to ${AUR_BIN_DIR}"
      log_info "[DRY-RUN] Would run ${UPDATE_SHA_SCRIPT}"
    else
      wait_for_release_asset "${tag}" "${asset_name}" || {
        confirm_continue "Release asset is not ready yet. Continue anyway?" || return 1
      }
      wait_for_release_asset "${tag}" "pacsea-aarch64" || {
        confirm_continue "aarch64 release asset is not ready yet. Continue anyway?" || return 1
      }

      cd "${AUR_BIN_DIR}"
      log_info "Changed to: ${AUR_BIN_DIR}"
      log_info "Running ${UPDATE_SHA_SCRIPT} (interactive)..."

      pkgbuild_file="${AUR_BIN_DIR}/PKGBUILD"
      sha_before="$(extract_first_sha256 "${pkgbuild_file}" || true)"
      if [[ -n "${sha_before}" ]]; then
        log_info "Current SHA before update-sha: ${sha_before}"
      else
        log_warn "Could not parse existing SHA from ${pkgbuild_file}"
      fi

      "${UPDATE_SHA_SCRIPT}" || {
        log_warn "update-sha256sums.sh may have failed or was cancelled"
        confirm_continue "Continue anyway?" || return 1
      }

      sha_after="$(extract_first_sha256 "${pkgbuild_file}" || true)"
      if [[ -n "${sha_after}" ]]; then
        log_info "SHA after update-sha: ${sha_after}"
      else
        log_warn "Could not parse SHA after update-sha from ${pkgbuild_file}"
      fi

      if [[ -n "${sha_before}" && -n "${sha_after}" && "${sha_before}" == "${sha_after}" ]]; then
        log_warn "SHA did not change after update-sha"
        confirm_continue "SHA unchanged. Continue anyway?" || return 1
      else
        log_success "SHA sums updated"
        mark_done "Phase 5.1: AUR SHA updated"
      fi
    fi
  fi

  log_step "Pushing pacsea-bin to AUR"
  if [[ "${SKIP_AUR_BIN_PROCESS}" == true ]]; then
    log_info "Skipping pacsea-bin AUR push."
    mark_skipped "Phase 5.2: pacsea-bin AUR push"
  elif [[ "${DRY_RUN}" == true ]]; then
    log_info "[DRY-RUN] Would run ${AUR_PUSH_SCRIPT} in ${AUR_BIN_DIR}"
  else
    cd "${AUR_BIN_DIR}"
    log_info "Running ${AUR_PUSH_SCRIPT} in ${AUR_BIN_DIR}..."
    "${AUR_PUSH_SCRIPT}" || { log_warn "aur-push.sh may have failed"; confirm_continue "Continue anyway?" || return 1; }
    log_success "Pushed pacsea-bin to AUR"
    mark_done "Phase 5.2: pacsea-bin pushed to AUR"
  fi

  log_step "Pushing pacsea-git to AUR"
  if [[ "${SKIP_AUR_GIT_PROCESS}" == true ]]; then
    log_info "Skipping pacsea-git AUR push."
    mark_skipped "Phase 5.3: pacsea-git AUR push"
  elif [[ "${DRY_RUN}" == true ]]; then
    log_info "[DRY-RUN] Would run ${AUR_PUSH_SCRIPT} in ${AUR_GIT_DIR}"
  else
    cd "${AUR_GIT_DIR}"
    log_info "Running ${AUR_PUSH_SCRIPT} in ${AUR_GIT_DIR}..."
    "${AUR_PUSH_SCRIPT}" || { log_warn "aur-push.sh may have failed"; confirm_continue "Continue anyway?" || return 1; }
    log_success "Pushed pacsea-git to AUR"
    mark_done "Phase 5.3: pacsea-git pushed to AUR"
  fi

  log_step "Syncing PKGBUILDs to Pacsea repo"
  if [[ "${SKIP_AUR_BIN_PROCESS}" == true || "${SKIP_AUR_GIT_PROCESS}" == true ]]; then
    log_info "Skipping PKGBUILD sync (requires both AUR processes)."
    mark_skipped "Phase 5.4: PKGBUILD sync to Pacsea repo"
  elif [[ "${DRY_RUN}" == true ]]; then
    log_info "[DRY-RUN] Would copy ${AUR_BIN_DIR}/PKGBUILD to ${PACSEA_DIR}/PKGBUILD-bin"
    log_info "[DRY-RUN] Would copy ${AUR_GIT_DIR}/PKGBUILD to ${PACSEA_DIR}/PKGBUILD-git"
    log_info "[DRY-RUN] Would commit and push PKGBUILD changes to Pacsea repo"
  else
    cp "${AUR_BIN_DIR}/PKGBUILD" "${PACSEA_DIR}/PKGBUILD-bin"
    cp "${AUR_GIT_DIR}/PKGBUILD" "${PACSEA_DIR}/PKGBUILD-git"
    log_success "Copied PKGBUILD files"

    cd "${PACSEA_DIR}"
    git add PKGBUILD-bin PKGBUILD-git
    git commit -m "Update PKGBUILDs for v${new_ver}" || log_warn "PKGBUILD commit failed or nothing to commit"
    git push origin || { log_error "Failed to push PKGBUILD changes"; confirm_continue "Continue anyway?" || return 1; }
    log_success "PKGBUILD changes pushed to origin"
    mark_done "Phase 5.4: PKGBUILD sync to Pacsea repo"
  fi

  log_step "Pushing Wiki Changes"
  if [[ "${DRY_RUN}" == true ]]; then
    if [[ "${SKIP_WIKI_PROCESS}" == true || -z "${WIKI_DIR}" ]]; then
      log_info "[DRY-RUN] Would skip wiki (no wiki directory configured)"
    else
      log_info "[DRY-RUN] Would commit and push wiki changes in ${WIKI_DIR}"
    fi
  else
    if [[ "${SKIP_WIKI_PROCESS}" == true || -z "${WIKI_DIR}" ]]; then
      log_info "Wiki push skipped (no wiki directory configured)."
      mark_skipped "Phase 5.5: Wiki push"
    elif [[ -d "${WIKI_DIR}" ]]; then
      cd "${WIKI_DIR}"
      wiki_status="$(git status --porcelain)"
      if [[ -n "${wiki_status}" ]]; then
        git add -A
        git commit -m "Update wiki for v${new_ver}" || log_warn "Wiki commit failed or nothing to commit"
        git push origin || { log_error "Failed to push wiki"; confirm_continue "Continue anyway?" || return 1; }
        log_success "Wiki pushed to origin"
        mark_done "Phase 5.5: Wiki pushed"
      else
        log_info "No wiki changes to commit"
        mark_done "Phase 5.5: Wiki had no changes"
      fi
    else
      log_warn "Wiki directory not found: ${WIKI_DIR}"
      mark_skipped "Phase 5.5: Wiki push (directory missing)"
    fi
  fi
  mark_done "Phase 5: AUR/Wiki updates"
}

# ============================================================================
# Prerequisites Check
# ============================================================================

# What: Ensures AUR clone paths exist and contain PKGBUILD; wiki path is optional.
# Inputs: Uses and may update global AUR_BIN_DIR, AUR_GIT_DIR, WIKI_DIR.
# Output: Returns 0 when layout is OK; 1 on user abort or unrecoverable input.
# Details: Empty input on wiki prompt skips wiki push for this run (WIKI_DIR="").
ensure_release_layout_directories() {
  local input resolved
  log_info "Checking release layout directories (AUR clones, optional wiki)..."

  while true; do
    resolved="$(expand_tilde "${AUR_BIN_DIR}")"
    if [[ -d "${resolved}" ]] && [[ -f "${resolved}/PKGBUILD" ]]; then
      AUR_BIN_DIR="$(cd "${resolved}" && pwd)"
      log_success "pacsea-bin AUR directory: ${AUR_BIN_DIR}"
      mark_done "Configured pacsea-bin directory"
      break
    fi
    log_warn "pacsea-bin path invalid or missing PKGBUILD: ${resolved}"
    printf "%bEnter path to pacsea-bin clone (%bs%b to skip, %bq%b to abort): %b" "${CYAN}" "${BOLD}" "${RESET}" "${BOLD}" "${RESET}" "${RESET}"
    read -r input || return 1
    case "${input}" in
      s|S)
        SKIP_AUR_BIN_PROCESS=true
        AUR_BIN_DIR=""
        log_warn "pacsea-bin process will be skipped for this run."
        mark_skipped "Skipped pacsea-bin process by user choice"
        break
        ;;
      q|Q) log_error "Aborted."; return 1 ;;
    esac
    [[ -z "${input}" ]] && continue
    AUR_BIN_DIR="$(expand_tilde "${input}")"
  done

  while true; do
    resolved="$(expand_tilde "${AUR_GIT_DIR}")"
    if [[ -d "${resolved}" ]] && [[ -f "${resolved}/PKGBUILD" ]]; then
      AUR_GIT_DIR="$(cd "${resolved}" && pwd)"
      log_success "pacsea-git AUR directory: ${AUR_GIT_DIR}"
      mark_done "Configured pacsea-git directory"
      break
    fi
    log_warn "pacsea-git path invalid or missing PKGBUILD: ${resolved}"
    printf "%bEnter path to pacsea-git clone (%bs%b to skip, %bq%b to abort): %b" "${CYAN}" "${BOLD}" "${RESET}" "${BOLD}" "${RESET}" "${RESET}"
    read -r input || return 1
    case "${input}" in
      s|S)
        SKIP_AUR_GIT_PROCESS=true
        AUR_GIT_DIR=""
        log_warn "pacsea-git process will be skipped for this run."
        mark_skipped "Skipped pacsea-git process by user choice"
        break
        ;;
      q|Q) log_error "Aborted."; return 1 ;;
    esac
    [[ -z "${input}" ]] && continue
    AUR_GIT_DIR="$(expand_tilde "${input}")"
  done

  resolved="$(expand_tilde "${WIKI_DIR}")"
  if [[ -d "${resolved}" ]]; then
    WIKI_DIR="$(cd "${resolved}" && pwd)"
    log_success "Wiki directory: ${WIKI_DIR}"
    mark_done "Configured wiki directory"
  else
    log_warn "Wiki path not found: ${resolved}"
    while true; do
      printf "%bEnter path to Pacsea.wiki clone (%bEnter%b/%bs%b to skip wiki push): %b" "${CYAN}" "${BOLD}" "${RESET}" "${BOLD}" "${RESET}" "${RESET}"
      read -r input || return 1
      if [[ -z "${input}" || "${input}" == "s" || "${input}" == "S" ]]; then
        WIKI_DIR=""
        SKIP_WIKI_PROCESS=true
        log_info "Wiki push will be skipped for this run."
        mark_skipped "Skipped wiki process by user choice"
        break
      fi
      resolved="$(expand_tilde "${input}")"
      if [[ -d "${resolved}" ]]; then
        WIKI_DIR="$(cd "${resolved}" && pwd)"
        log_success "Wiki directory: ${WIKI_DIR}"
        mark_done "Configured wiki directory"
        break
      fi
      log_error "Not a directory: ${resolved}"
    done
  fi

  return 0
}

check_prerequisites() {
  local missing=()
  local cmd
  local required_commands=(
    cargo git gh python3
    curl rg awk sed realpath mktemp xargs
    makepkg updpkgsums
    gitleaks
  )
  log_info "Checking prerequisites..."

  for cmd in "${required_commands[@]}"; do
    command -v "${cmd}" >/dev/null 2>&1 || missing+=("${cmd}")
  done
  cargo audit --version >/dev/null 2>&1 || missing+=("cargo audit")
  cargo deny --version >/dev/null 2>&1 || missing+=("cargo deny")
  [[ -x "${AUR_PUSH_SCRIPT}" ]] || missing+=("${AUR_PUSH_SCRIPT} (executable)")
  [[ -x "${UPDATE_SHA_SCRIPT}" ]] || missing+=("${UPDATE_SHA_SCRIPT} (executable)")

  if [[ "${#missing[@]}" -gt 0 ]]; then
    log_error "Missing required commands: ${missing[*]}"
    return 1
  fi

  [[ -d "${PACSEA_DIR}" ]] || { log_error "Pacsea directory not found: ${PACSEA_DIR}"; return 1; }

  log_success "All prerequisites met"
}

# ============================================================================
# Pre-flight Checks
# ============================================================================

check_preflight() {
  local current_branch git_status
  log_info "Running pre-flight checks..."
  cd "${PACSEA_DIR}"

  current_branch="$(git branch --show-current)"
  if [[ "${current_branch}" != "main" ]]; then
    log_error "Not on main branch (current: ${current_branch})"
    confirm_continue "Continue on branch '${current_branch}'?" || return 1
  else
    log_success "On main branch"
  fi

  git_status="$(git status --porcelain)"
  if [[ -n "${git_status}" ]]; then
    log_error "Working directory is not clean"
    log_info "Uncommitted changes:"
    git status --short
    echo
    confirm_continue "Continue with uncommitted changes?" || return 1
  else
    log_success "Working directory is clean"
  fi
}

# ============================================================================
# SECURITY.md Update
# ============================================================================

update_security_md() {
  local new_ver="${1}"
  local security_file="${PACSEA_DIR}/SECURITY.md"
  local major minor major_minor tmp_file table_updated=false

  log_step "Updating SECURITY.md"
  if [[ "${DRY_RUN}" == true ]]; then
    log_info "[DRY-RUN] Would update SECURITY.md with new version ${new_ver}"
    return 0
  fi
  [[ -f "${security_file}" ]] || { log_error "SECURITY.md not found at: ${security_file}"; return 1; }

  major="$(cut -d. -f1 <<<"${new_ver}")"
  minor="$(cut -d. -f2 <<<"${new_ver}")"
  major_minor="${major}.${minor}"
  tmp_file="$(mktemp)"

  set +e
  awk -v mm="${major_minor}" '
    BEGIN { in_table=0; inserted=0; updated_lt=0 }
    /^\|\s*Version\s*\|\s*Supported/ { in_table=1; print; next }
    in_table && /^\|\s*-/ {
      print
      if (!inserted) {
        print "| " mm ".x   | :white_check_mark: |"
        inserted=1
      }
      next
    }
    in_table && /^\|\s*</ {
      print "| < " mm ".0   | :x:                |"
      updated_lt=1
      next
    }
    in_table && /^\|\s*[0-9]/ { next }
    {
      if (in_table && !/^\|/) in_table=0
      print
    }
    END {
      if (inserted && updated_lt) exit 0
      exit 2
    }
  ' "${security_file}" > "${tmp_file}"
  local awk_status=$?
  set -e

  if [[ "${awk_status}" -eq 2 ]]; then
    rm -f "${tmp_file}"
    log_warn "Could not find version table in SECURITY.md"
    return 1
  elif [[ "${awk_status}" -ne 0 ]]; then
    rm -f "${tmp_file}"
    log_error "Failed to parse SECURITY.md"
    return 1
  fi

  mv "${tmp_file}" "${security_file}"
  table_updated=true
  [[ "${table_updated}" == true ]] && log_success "SECURITY.md updated: ${major_minor}.x is now supported"
}

# ============================================================================
# CHANGELOG Update
# ============================================================================

update_changelog() {
  local new_ver="${1}"
  local changelog_file="${PACSEA_DIR}/CHANGELOG.md"
  local release_file="${PACSEA_DIR}/Release-docs/RELEASE_v${new_ver}.md"
  local release_date tmp_file existing_version_line version_start version_end first_version_line

  log_step "Updating CHANGELOG.md"
  if [[ "${DRY_RUN}" == true ]]; then
    log_info "[DRY-RUN] Would update CHANGELOG.md with release notes"
    return 0
  fi

  if [[ ! -f "${changelog_file}" ]]; then
    log_info "Creating CHANGELOG.md..."
    cat > "${changelog_file}" <<'EOF'
# Changelog

All notable changes to Pacsea will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---
EOF
  fi

  if [[ ! -f "${release_file}" ]]; then
    log_warn "Release file not found: ${release_file}"
    log_warn "Skipping CHANGELOG update"
    return 0
  fi

  release_date="$(date +%Y-%m-%d)"
  tmp_file="$(mktemp)"
  existing_version_line="$(awk -v v="${new_ver}" '/^##\s*\[/{if ($0 ~ "\\[" v "\\]"){print NR; exit}}' "${changelog_file}")"

  if [[ -n "${existing_version_line}" ]]; then
    log_info "Version ${new_ver} already exists, replacing in place..."
    version_start="${existing_version_line}"
    version_end="$(awk -v s="${version_start}" '
      NR<=s {next}
      /^---$/ && NR>(s+2) {print NR; exit}
      /^##\s*\[/ {print NR; exit}
      END {if (NR>0) print NR+1}
    ' "${changelog_file}")"

    if [[ "${version_start}" -gt 1 ]]; then
      awk -v end="$((version_start - 1))" 'NR<=end' "${changelog_file}" > "${tmp_file}"
    else
      : > "${tmp_file}"
    fi

    {
      printf "## [%s] - %s\n\n" "${new_ver}" "${release_date}"
      cat "${release_file}"
      printf "\n---\n\n"
    } >> "${tmp_file}"

    awk -v start="${version_end}" 'NR>=start' "${changelog_file}" >> "${tmp_file}"
  else
    log_info "Version ${new_ver} not found, adding to the top..."
    first_version_line="$(awk '/^##\s*\[.*\]/{print NR; exit}' "${changelog_file}")"

    if [[ -n "${first_version_line}" && "${first_version_line}" -gt 1 ]]; then
      awk -v end="$((first_version_line - 1))" 'NR<=end' "${changelog_file}" > "${tmp_file}"
      {
        printf "## [%s] - %s\n\n" "${new_ver}" "${release_date}"
        cat "${release_file}"
        printf "\n---\n\n"
      } >> "${tmp_file}"
      awk -v start="${first_version_line}" 'NR>=start' "${changelog_file}" >> "${tmp_file}"
    else
      {
        printf "## [%s] - %s\n\n" "${new_ver}" "${release_date}"
        cat "${release_file}"
        printf "\n---\n\n"
        cat "${changelog_file}"
      } > "${tmp_file}"
    fi
  fi

  mv "${tmp_file}" "${changelog_file}"
  log_success "CHANGELOG.md updated"
}

# ============================================================================
# Main
# ============================================================================

main() {
  local new_version=""
  local current old_version final_status

  while [[ $# -gt 0 ]]; do
    case "${1}" in
      --dry-run)
        DRY_RUN=true
        log_warn "DRY RUN MODE - No changes will be made"
        shift
        ;;
      --pkgrel)
        if [[ $# -lt 2 ]]; then
          log_error "--pkgrel requires an argument: reset, keep, or bump"
          return 1
        fi
        PKGREL_MODE="${2}"
        PKGREL_CLI_SET=true
        shift 2
        ;;
      -h|--help)
        cat <<'EOF'
Usage: release.sh [--dry-run] [--pkgrel MODE] [version]

Options:
  --dry-run       Preview all changes without executing them
  --pkgrel MODE   AUR pkgrel after pkgver update: reset, keep, or bump (omit = prompt if TTY)
  -h, --help      Show this help message

If version is not provided, you will be prompted to enter it.
EOF
        return 0
        ;;
      -*)
        log_error "Unknown option: ${1}"
        return 1
        ;;
      *)
        if [[ -z "${new_version}" ]]; then
          new_version="${1}"
        else
          log_error "Unexpected argument: ${1}"
          return 1
        fi
        shift
        ;;
    esac
  done

  if [[ "${PKGREL_CLI_SET}" == true ]]; then
    case "${PKGREL_MODE}" in
      reset|keep|bump) ;;
      *)
        log_error "Invalid --pkgrel value: ${PKGREL_MODE} (use reset, keep, or bump)"
        return 1
        ;;
    esac
  fi

  initialize_release_report
  enable_report_mirroring || return 1
  final_status="failed"

  echo
  printf "%b╔════════════════════════════════════════════════════════════════════════╗%b\n" "${BOLD_CYAN}" "${RESET}"
  printf "%b║                    PACSEA RELEASE AUTOMATION                          ║%b\n" "${BOLD_CYAN}" "${RESET}"
  printf "%b╚════════════════════════════════════════════════════════════════════════╝%b\n\n" "${BOLD_CYAN}" "${RESET}"

  check_prerequisites || { write_release_report "${final_status}"; return 1; }
  ensure_release_layout_directories || { write_release_report "${final_status}"; return 1; }
  check_preflight || { write_release_report "${final_status}"; return 1; }

  if [[ -z "${new_version}" ]]; then
    current="$(get_current_version)"
    if [[ "${DRY_RUN}" == true ]]; then
      new_version="${current}"
      log_info "DRY-RUN: using current Cargo.toml version: ${new_version}"
    else
      printf "%bEnter new version (current: %s): %b" "${CYAN}" "${current}" "${RESET}"
      read -r new_version
    fi
  fi
  set_release_report_version_in_filename "${new_version}"
  if ! validate_semver "${new_version}"; then
    log_error "Invalid version format: ${new_version} (expected: X.Y.Z)"
    return 1
  fi

  prompt_pkgrel_mode_interactive_if_needed || return 1
  case "${PKGREL_MODE}" in
    reset|keep|bump) ;;
    *)
      log_error "Invalid pkgrel mode: ${PKGREL_MODE}"
      return 1
      ;;
  esac

  echo
  printf "%b[INFO] %bRelease version: %b%s%b\n" "${BLUE}" "${RESET}" "${BOLD}" "${new_version}" "${RESET}"
  printf "%b[INFO] %bCurrent version: %b%s%b\n" "${BLUE}" "${RESET}" "${BOLD}" "$(get_current_version)" "${RESET}"
  printf "%b[INFO] %bAUR pkgrel mode: %b%s%b\n\n" "${BLUE}" "${RESET}" "${BOLD}" "${PKGREL_MODE}" "${RESET}"
  confirm_continue "Start release process?" || { log_info "Release cancelled"; mark_skipped "Release cancelled by user"; write_release_report "cancelled"; return 0; }

  old_version="$(get_current_version)"
  phase1_version_update "${new_version}" || { write_release_report "${final_status}"; return 1; }
  phase2_documentation "${new_version}" "${old_version}" || { write_release_report "${final_status}"; return 1; }
  phase3_pkgbuild_updates "${new_version}" || { write_release_report "${final_status}"; return 1; }
  phase4_build_release "${new_version}" || { write_release_report "${final_status}"; return 1; }
  phase5_aur_update "${new_version}" || { write_release_report "${final_status}"; return 1; }

  echo
  printf "%b╔════════════════════════════════════════════════════════════════════════╗%b\n" "${BOLD_GREEN}" "${RESET}"
  printf "%b║                    RELEASE COMPLETE! 🎉                               ║%b\n" "${BOLD_GREEN}" "${RESET}"
  printf "%b╚════════════════════════════════════════════════════════════════════════╝%b\n\n" "${BOLD_GREEN}" "${RESET}"
  log_success "Version ${new_version} has been released!"
  echo
  log_info "Don't forget to verify:"
  echo "  • GitHub release: https://github.com/Firstp1ck/Pacsea/releases"
  echo "  • GitHub Action uploaded the binary"
  echo "  • AUR packages are updated"
  echo

  final_status="success"
  write_release_report "${final_status}"
}

main "$@"
