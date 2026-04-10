#!/usr/bin/env bash
set -euo pipefail

# ============================================================================
# Script: update-sha256sums.sh
# ============================================================================
# Description:
#   Automated PKGBUILD checksum updater for Arch Linux AUR packages. Downloads
#   binary and source artifacts from GitHub releases, computes SHA-256 checksums,
#   and updates the PKGBUILD file. Supports both interactive and non-interactive
#   modes for CI/CD workflows.
#
# What it does:
#   - Derives repository, version, and tag from PKGBUILD or user input
#   - Downloads binary artifact from GitHub releases
#   - Downloads source tarball from GitHub releases
#   - Computes SHA-256 checksums for both artifacts
#   - Updates sha256sums array in PKGBUILD (up to 9 entries)
#   - Optionally updates .SRCINFO file
#   - Preserves PKGBUILD formatting
#   - Performs no-op if checksums are unchanged
#
# How to use:
#   Interactive mode (selects PKGBUILD automatically):
#     ./update-sha256sums.sh
#   
#   Non-interactive mode:
#     ./update-sha256sums.sh --package NAME --version X.Y.Z --yes
#   
#   Options:
#     -p, --pkgbuild PATH     Path to PKGBUILD
#     -P, --package NAME      Resolve PKGBUILD at $AUR_BASE/NAME-bin/PKGBUILD
#     -r, --repo REPO         GitHub repo (OWNER/REPO)
#     -v, --version X.Y.Z     Version number
#     -t, --tag TAG           Git tag (e.g., v0.4.0)
#     -y, --yes               Skip confirmation prompts
#     -U, --update-srcinfo    Update .SRCINFO after checksum update
#     -n, --dry-run           Preview without modifying files
#     -h, --help              Show help message
#
# Target:
#   - AUR package maintainers updating checksums
#   - CI/CD pipelines automating PKGBUILD updates
#   - Users maintaining binary AUR packages
# ============================================================================

# Gum detection
# HAS_GUM is not used in this script

#############################################
# Defaults
#############################################
PKGFILE="./PKGBUILD"
REPO=""                 # OWNER/REPO or just REPO; defaults to folder name (strip '-bin')
ASSET_NAME=""     # release asset file name (default: derived from directory, strip '-bin')
VERSION=""              # e.g. 0.4.0
TAG=""                  # e.g. v0.4.0
TAG_PREFIX="v"          # used when TAG is not provided
BINARY_URL=""           # optional explicit URL override
SOURCE_URL=""           # optional explicit URL override
DRY_RUN=false
YES=false
UPDATE_SRCINFO=false
PACKAGE_NAME=""

STEP=0
REPO_FROM_CLI=false
ASSET_FROM_CLI=false

# Exit codes
E_NO_PKG=10
E_DOWNLOAD=12
E_PARSE=13
E_NONINTERACTIVE=14

#############################################
# Helpers
#############################################
usage() {
  cat <<'EOF'
NAME
  update-sha256sums.sh - Update PKGBUILD sha256sums for binary and source artifacts

SYNOPSIS
  update-sha256sums.sh [OPTIONS]

DESCRIPTION
  A step-by-step, modular PKGBUILD checksum updater for GitHub releases.
  - Derives version/tag, infers repo from url= and source=(), downloads binary and source,
    computes sha256sums, and updates up to the first 9 entries of the target sha array.
  - Preserves formatting and performs a no-op when sums are unchanged.
  - Interactive prompts in TTY; non-interactive mode requires explicit inputs.

OPTIONS
  -p, --pkgbuild PATH         Path to PKGBUILD (default: ./PKGBUILD or interactive selection under $AUR_BASE; default AUR_BASE: $HOME/aur-packages)
  -P, --package NAME          Resolve PKGBUILD at $AUR_BASE/NAME-bin/PKGBUILD (non-interactive friendly)
  -r, --repo REPO|OWNER/REPO  GitHub repo. If only REPO is provided, owner is inferred from PKGBUILD url/source or $GITHUB_OWNER. Defaults to folder name (strip '-bin').
  -a, --asset NAME            Release asset filename (default: derived from PKGBUILD directory, strip '-bin')
  -v, --version X.Y.Z         Version; if TAG is unset, tag = TAG_PREFIX+version (default prefix: 'v')
  -t, --tag TAG               Exact tag (e.g. v0.4.0); if VERSION is unset, version is derived by stripping TAG_PREFIX
  -T, --tag-prefix PFX        Tag prefix used to build/strip tags (default: v)
  -B, --binary-url URL        Override binary download URL (disables repo/asset inference)
  -S, --source-url URL        Override source tarball URL (disables repo inference)
  -y, --yes                   Skip confirmation prompt (useful in CI)
  -U, --update-srcinfo        Run: makepkg --printsrcinfo > .SRCINFO after updating
  -n, --dry-run               Show actions and computed hashes but do not modify files
  -h, --help                  Show this help and exit

ENVIRONMENT
  AUR_BASE                   Base directory for AUR packages (default: $HOME/aur-packages)
  GITHUB_OWNER               Default owner to use when repo owner is not specified
  TAG_PREFIX                 Tag prefix (default: v)

EXIT CODES
  0   Success (updated or no changes needed)
  10  No PKGBUILD found
  11  Invalid version provided
  12  Download error
  13  Parse error (arrays or .SRCINFO)
  14  Non-interactive requirements not met

EXAMPLES
  # Use ./PKGBUILD or select interactively (TTY)
  update-sha256sums.sh

  # Explicit PKGBUILD
  update-sha256sums.sh -p ./PKGBUILD

  # CI/non-interactive with .SRCINFO update
  update-sha256sums.sh --package pacsea --version 0.4.0 --yes --update-srcinfo

  # Explicit repo and asset, derive tag from version
  update-sha256sums.sh -r Firstp1ck/Pacsea -a Pacsea -v 0.4.0

  # Exact tag
  update-sha256sums.sh -t v0.4.0

REQUIREMENTS
  curl, sha256sum, sed, grep, awk, find
  makepkg (only required when using --update-srcinfo)

EOF
}

log_step() {
  STEP=$((STEP+1))
  echo "[$STEP] ℹ️ $*" >&2
}

die() {
  echo "❌ Error: $*" >&2
  exit 1
}

require_cmd() {
  if command -v "$1" >/dev/null 2>&1; then
    echo "✅ Found required command: $1" >&2
  else
    die "Missing required command: $1"
  fi
}

parse_repo_from_url() {
  # Extract owner/repo from a URL like https://github.com/Owner/Repo
  # Returns via echo or empty if not parsable
  local url_line repo
  url_line=$(grep -E '^[[:space:]]*url=' "$PKGFILE" | head -n1 | cut -d '=' -f2- | tr -d '"' | tr -d "'")
  if [[ -n "${url_line:-}" ]]; then
    repo=$(echo "$url_line" | sed -nE 's#^.*/github\.com/([^/]+)/([^/]+)/?$#\1/\2#p')
    [[ -n "${repo:-}" ]] && echo "$repo"
  fi
}

parse_repo_from_source() {
  # Inspect source=() lines for GitHub URLs and extract OWNER/REPO
  # Handles forms like:
  #  - git+https://github.com/Owner/Repo.git#tag=v1.2.3
  #  - https://github.com/Owner/Repo/archive/refs/tags/v1.2.3.tar.gz
  #  - https://github.com/Owner/Repo/releases/download/v1.2.3/asset
  local url repo
  url=$(awk '/^[[:space:]]*source(_[A-Za-z0-9]+)?=\(/,/\)/ {print}' "$PKGFILE" \
    | tr -d '"' | tr -d "'" \
    | grep -Eo '(git\+)?https?://github\.com/[^ )]+' \
    | head -n1 || true)
  if [[ -n "${url:-}" ]]; then
    repo=$(echo "$url" | sed -nE 's#^([^:]+://)?(github\.com)/([^/]+)/([^/]+)(\.git)?(/.*)?$#\3/\4#p')
    [[ -n "${repo:-}" ]] && echo "$repo"
  fi
}

die_code() {
  local code="$1"; shift
  echo "❌ Error: $*" >&2
  exit "$code"
}

curl_retry() {
  # Usage: curl_retry URL OUT_PATH
  local url="$1" out="$2"
  local tries=0 max=3 delay=1
  while (( tries < max )); do
    if curl -fsSL -o "$out" "$url"; then
      return 0
    fi
    tries=$((tries+1))
    echo "⚠️ Download failed (${tries}/${max}). Retrying in ${delay}s: ${url}" >&2
    sleep "$delay"
    delay=$((delay*2))
  done
  return 1
}

is_interactive() {
  [[ -t 0 ]]
}

#############################################
# Parse args
#############################################
if [[ $# -gt 0 ]]; then
  while [[ $# -gt 0 ]]; do
    case "$1" in
      -p|--pkgbuild)
        PKGFILE="$2"; shift 2;;
      -P|--package)
        PACKAGE_NAME="$2"; shift 2;;
      -r|--repo)
        REPO="$2"; REPO_FROM_CLI=true; shift 2;;
      -a|--asset)
        ASSET_NAME="$2"; ASSET_FROM_CLI=true; shift 2;;
      -v|--version)
        VERSION="$2"; shift 2;;
      -t|--tag)
        TAG="$2"; shift 2;;
      -T|--tag-prefix)
        TAG_PREFIX="$2"; shift 2;;
      -B|--binary-url)
        BINARY_URL="$2"; shift 2;;
      -S|--source-url)
        SOURCE_URL="$2"; shift 2;;
      -y|--yes)
        YES=true; shift;;
      -U|--update-srcinfo)
        UPDATE_SRCINFO=true; shift;;
      -n|--dry-run)
        DRY_RUN=true; shift;;
      -h|--help)
        usage; exit 0;;
      *)
        # Backward-compat: allow a single positional PKGBUILD path
        if [[ "$1" != -* && "$#" -eq 1 ]]; then
          PKGFILE="$1"; shift
        else
          echo "Unknown option: $1" >&2
          usage
          exit 2
        fi
        ;;
    esac
  done
fi

#############################################
# Validations & discovery
#############################################
require_cmd curl
require_cmd sha256sum
require_cmd sed
require_cmd grep
require_cmd awk
require_cmd find

# Resolve PKGBUILD path
AUR_BASE="${AUR_BASE:-$HOME/aur-packages}"

# If --package is provided, prefer it
if [[ -n "${PACKAGE_NAME:-}" ]]; then
  pkgdir="${PACKAGE_NAME%-bin}-bin"
  PKGFILE="$AUR_BASE/$pkgdir/PKGBUILD"
fi

INTERACTIVE=false
if is_interactive; then INTERACTIVE=true; fi

# Prefer ./PKGBUILD; if missing and not provided, optionally select interactively or fail in non-interactive mode
if [[ ! -f "$PKGFILE" ]]; then
  echo "ℹ️ Looking for PKGBUILD under: $AUR_BASE" >&2
  if [[ ! -d "$AUR_BASE" ]]; then
    die_code $E_NO_PKG "PKGBUILD not found in current directory and AUR_BASE directory does not exist: $AUR_BASE (provide -p or --package)"
  fi

  if [[ "$INTERACTIVE" != true ]]; then
    die_code $E_NONINTERACTIVE "No PKGBUILD specified. Provide -p/--pkgbuild or --package NAME for non-interactive use."
  fi

  mapfile -t _pkgs < <(find "$AUR_BASE" -maxdepth 2 -type f -name PKGBUILD 2>/dev/null | awk -F/ '$(NF-1) ~ /-bin$/' | sort)
  if [[ ${#_pkgs[@]} -eq 0 ]]; then
    die_code $E_NO_PKG "No -bin packages with PKGBUILD found under: $AUR_BASE"
  fi
  echo "Please select a -bin package to use:" >&2
  for i in "${!_pkgs[@]}"; do
    d=$(dirname "${_pkgs[$i]}")
    printf "  [%d] %s\n" "$((i+1))" "$d" >&2
  done
  while true; do
    read -r -p "Enter selection [1-${#_pkgs[@]}]: " choice
    if [[ "$choice" =~ ^[0-9]+$ ]] && (( choice >= 1 && choice <= ${#_pkgs[@]} )); then
      sel_dir=$(dirname "${_pkgs[$((choice-1))]}")
      PKGFILE="$sel_dir/PKGBUILD"
      break
    else
      echo "Invalid selection. Try again." >&2
    fi
  done
fi
echo "✅ Using PKGBUILD: $PKGFILE" >&2

# Derive default asset name from current working directory if PKGFILE is in ./,
# otherwise from PKGBUILD's directory (resolved absolute). Strip trailing '-bin'.
if [[ -z "${ASSET_NAME:-}" ]]; then
  if [[ "$PKGFILE" == "./PKGBUILD" || "$PKGFILE" == "PKGBUILD" || "$(dirname -- "$PKGFILE")" == "." ]]; then
    dir="$PWD"
  else
    dir="$(cd -- "$(dirname -- "$PKGFILE")" >/dev/null 2>&1 && pwd -P)"
  fi
  base_name="$(basename "$dir")"
  ASSET_NAME="${base_name%-bin}"
fi
# Default REPO to folder name (strip '-bin') if not provided
if [[ -z "${REPO:-}" ]]; then
  REPO="${ASSET_NAME}"
fi

# Determine REPO if not provided via CLI
if [[ "${REPO_FROM_CLI:-false}" != true ]]; then
  owner_only=""
  from_url=$(parse_repo_from_url || true)
  from_src=$(parse_repo_from_source || true)
  if [[ -n "${from_url:-}" ]]; then
    owner_only="${from_url%%/*}"
  elif [[ -n "${from_src:-}" ]]; then
    owner_only="${from_src%%/*}"
    if [[ -z "${REPO:-}" ]]; then REPO="${from_src#*/}"; fi
  elif [[ -n "${GITHUB_OWNER:-}" ]]; then
    owner_only="$GITHUB_OWNER"
  fi
  suggested="$REPO"
  if [[ -n "${owner_only:-}" && "$REPO" != */* ]]; then
    suggested="${owner_only}/${REPO}"
  fi

  if [[ "$INTERACTIVE" == true ]]; then
    while true; do
      read -r -p "Enter GitHub repo (OWNER/REPO) [${suggested}]: " input_repo
      input_repo="${input_repo:-$suggested}"
      if [[ "$input_repo" != */* ]]; then
        if [[ -n "${owner_only:-}" ]]; then
          input_repo="${owner_only}/${input_repo}"
        else
          echo "Please provide in OWNER/REPO form." >&2
          continue
        fi
      fi
      REPO="$input_repo"
      break
    done
  else
    # Non-interactive: try to complete owner/repo automatically or fail
    if [[ "$REPO" != */* ]]; then
      if [[ -n "${owner_only:-}" ]]; then
        REPO="${owner_only}/${REPO}"
      else
        die_code $E_NONINTERACTIVE "Unable to infer GitHub repo owner. Provide --repo OWNER/REPO."
      fi
    fi
  fi
fi

# Prompt for ASSET_NAME if not provided via CLI (interactive only)
if [[ "${ASSET_FROM_CLI:-false}" != true && "$INTERACTIVE" == true ]]; then
  read -r -p "Enter release asset filename [${ASSET_NAME}]: " input_asset
  if [[ -n "${input_asset:-}" ]]; then
    ASSET_NAME="$input_asset"
  fi
fi

# Determine VERSION/TAG
if [[ -z "${VERSION:-}" && -z "${TAG:-}" ]]; then
  current_pkgver=$(grep -E '^[[:space:]]*pkgver=' "$PKGFILE" | head -n1 | cut -d '=' -f2 || true)
  if [[ -n "${current_pkgver:-}" ]]; then
    echo "Current version: ${current_pkgver}" >&2
  fi
  if [[ "$INTERACTIVE" == true ]]; then
    while true; do
      read -r -p "Enter version (x.x.x) [ENTER to keep current]: " VERSION
      if [[ -z "${VERSION:-}" ]]; then
        if [[ -n "${current_pkgver:-}" ]]; then
          VERSION="$current_pkgver"
          break
        else
          echo "No current version found in PKGBUILD. Please enter a version in 'x.x.x' format (e.g., 0.4.5)." >&2
          continue
        fi
      fi
      if [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        break
      else
        echo "Invalid version. Please enter in 'x.x.x' format (e.g., 0.4.5)." >&2
      fi
    done
  else
    if [[ -n "${current_pkgver:-}" ]]; then
      VERSION="$current_pkgver"
    else
      die_code $E_NONINTERACTIVE "Version/tag required. Provide --version X.Y.Z or --tag TAG for non-interactive use."
    fi
  fi
fi

# If only VERSION is known, build TAG from it
if [[ -z "${TAG:-}" && -n "${VERSION:-}" ]]; then
  TAG="${TAG_PREFIX}${VERSION}"
fi

# If only TAG is known, derive VERSION from it (strip TAG_PREFIX if present)
if [[ -z "${VERSION:-}" && -n "${TAG:-}" ]]; then
  if [[ -n "${TAG_PREFIX:-}" && "$TAG" == ${TAG_PREFIX}* ]]; then
    VERSION="${TAG#"${TAG_PREFIX}"}"
  else
    VERSION="${TAG}"
  fi
fi

if [[ -z "${BINARY_URL:-}" ]]; then
  log_step "Inferring repo and owner"
  # Default repo name to folder-derived name if still empty
  if [[ -z "${REPO:-}" ]]; then
    REPO="${ASSET_NAME}"
  fi
  # If owner is missing (no slash), try to infer from PKGBUILD url/source or $GITHUB_OWNER
  if [[ "$REPO" != */* ]]; then
    from_url=$(parse_repo_from_url || true)
    from_src=$(parse_repo_from_source || true)
    if [[ -n "${from_url:-}" ]]; then
      owner_part="${from_url%%/*}"
    elif [[ -n "${from_src:-}" ]]; then
      owner_part="${from_src%%/*}"
    else
      owner_part="${GITHUB_OWNER:-}"
    fi
    if [[ -n "${owner_part:-}" ]]; then
      REPO="${owner_part}/${REPO}"
    fi
  fi
fi

if [[ -z "${BINARY_URL:-}" ]]; then
  if [[ "$REPO" != */* ]]; then
    echo "Could not determine repo owner automatically." >&2
    default_repo="$ASSET_NAME"
    from_url=$(parse_repo_from_url || true)
    from_src=$(parse_repo_from_source || true)
    owner_only=""
    if [[ -n "${from_url:-}" ]]; then owner_only="${from_url%%/*}"; fi
    if [[ -z "${owner_only:-}" && -n "${from_src:-}" ]]; then owner_only="${from_src%%/*}"; fi
    if [[ -z "${owner_only:-}" && -n "${GITHUB_OWNER:-}" ]]; then owner_only="$GITHUB_OWNER"; fi
    if [[ -n "${owner_only:-}" ]]; then
      suggested="${owner_only}/${default_repo}"
    else
      suggested="${default_repo}"
    fi
    if [[ "$INTERACTIVE" == true ]]; then
      while true; do
        read -r -p "Enter GitHub repo (OWNER/REPO) [${suggested}]: " input_repo
        input_repo="${input_repo:-$suggested}"
        if [[ "$input_repo" != */* ]]; then
          if [[ -n "${owner_only:-}" ]]; then
            input_repo="${owner_only}/${input_repo}"
          else
            echo "Please provide in OWNER/REPO form." >&2
            continue
          fi
        fi
        REPO="$input_repo"
        break
      done
    else
      die_code $E_NONINTERACTIVE "Unable to infer full repo. Provide --repo OWNER/REPO for non-interactive use."
    fi
  fi
  BINARY_URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET_NAME}"
fi

if [[ -z "${SOURCE_URL:-}" ]]; then
  if [[ "$REPO" != */* ]]; then
    # Prompt for repo to build SOURCE_URL if still missing owner
    default_repo="$ASSET_NAME"
    from_url=$(parse_repo_from_url || true)
    from_src=$(parse_repo_from_source || true)
    owner_only=""
    if [[ -n "${from_url:-}" ]]; then owner_only="${from_url%%/*}"; fi
    if [[ -z "${owner_only:-}" && -n "${from_src:-}" ]]; then owner_only="${from_src%%/*}"; fi
    if [[ -z "${owner_only:-}" && -n "${GITHUB_OWNER:-}" ]]; then owner_only="$GITHUB_OWNER"; fi
    if [[ -n "${owner_only:-}" ]]; then
      suggested="${owner_only}/${default_repo}"
    else
      suggested="${default_repo}"
    fi
    if [[ "$INTERACTIVE" == true ]]; then
      while true; do
        read -r -p "Enter GitHub repo (OWNER/REPO) [${suggested}]: " input_repo
        input_repo="${input_repo:-$suggested}"
        if [[ "$input_repo" != */* ]]; then
          if [[ -n "${owner_only:-}" ]]; then
            input_repo="${owner_only}/${input_repo}"
          else
            echo "Please provide in OWNER/REPO form." >&2
            continue
          fi
        fi
        REPO="$input_repo"
        break
      done
    else
      die_code $E_NONINTERACTIVE "Unable to infer full repo for SOURCE_URL. Provide --repo OWNER/REPO."
    fi
  fi
  SOURCE_URL="https://github.com/${REPO}/archive/refs/tags/${TAG}.tar.gz"
fi

echo "ℹ️ Repo:       ${REPO:-"(n/a) (custom URLs)"}" >&2
echo "ℹ️ Version:    ${VERSION}" >&2
echo "ℹ️ Tag:        ${TAG}" >&2
echo "ℹ️ Asset:      ${ASSET_NAME}" >&2
echo "ℹ️ Binary URL: ${BINARY_URL}" >&2
echo "ℹ️ Source URL: ${SOURCE_URL}" >&2

#############################################
# Download artifacts and compute hashes
#############################################
log_step "Downloading artifacts for ${TAG}"
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT

bin_path="$tmpdir/${ASSET_NAME}"
src_path="$tmpdir/src-${TAG}.tar.gz"

if ! curl_retry "$BINARY_URL" "$bin_path"; then
  die_code $E_DOWNLOAD "Failed to download binary from $BINARY_URL"
fi
echo "✅ Downloaded binary artifact" >&2
if ! curl_retry "$SOURCE_URL" "$src_path"; then
  die_code $E_DOWNLOAD "Failed to download source from $SOURCE_URL"
fi
echo "✅ Downloaded source tarball" >&2

log_step "Computing sha256 sums"
sha_bin=$(sha256sum "$bin_path" | awk '{print $1}')
sha_src=$(sha256sum "$src_path" | awk '{print $1}')

echo "ℹ️ binary:  $sha_bin" >&2
echo "ℹ️ source:  $sha_src" >&2

#############################################
# Update PKGBUILD
#############################################
log_step "Detecting source/sha arrays in $PKGFILE"

# Determine target suffix based on available source arrays
target_suffix=""
if grep -Eq '^[[:space:]]*source_x86_64=\(' "$PKGFILE"; then
  target_suffix="_x86_64"
elif grep -Eq '^[[:space:]]*source=\(' "$PKGFILE"; then
  target_suffix=""
else
  first_src_name=$(grep -E '^[[:space:]]*source(_[A-Za-z0-9]+)?=\(' "$PKGFILE" | head -n1 | sed -nE 's/^[[:space:]]*(source(_[A-Za-z0-9]+)?)=.*/\1/p')
  if [[ "$first_src_name" =~ ^source(_[A-Za-z0-9]+)$ ]]; then
    target_suffix="${BASH_REMATCH[1]}"
  else
    target_suffix=""
  fi
fi
src_var="source${target_suffix}"
sha_var="sha256sums${target_suffix}"

log_step "Locating ${sha_var}=( in $PKGFILE"
sha_line=$(grep -nE "^[[:space:]]*${sha_var}=\\(" "$PKGFILE" | head -n1 | cut -d: -f1 || true)
[[ -n "${sha_line:-}" ]] || die_code $E_PARSE "Could not find ${sha_var}=( in $PKGFILE"

# Count entries in the source array to decide how many checksums we need to set
src_line=$(grep -nE "^[[:space:]]*${src_var}=\\(" "$PKGFILE" | head -n1 | cut -d: -f1 || true)
[[ -n "${src_line:-}" ]] || die_code $E_PARSE "Could not find ${src_var}=( in $PKGFILE"

src_entry_count=$(awk -v start="$src_line" -v var="${src_var}" '
  NR < start { next }
  {
    if (!inarr) {
      line_head=$0
      while (length(line_head)>0 && (substr(line_head,1,1)==" " || substr(line_head,1,1)=="\t")) line_head=substr(line_head,2)
      if (index(line_head, var "(")==1 || index(line_head, var "=(")==1) { inarr=1 }
    }
    if (inarr) {
      line=$0
      while (match(line, /"[^"]*"|\047[^\047]*\047/)) {
        c++
        line=substr(line, RSTART+RLENGTH)
      }
      if ($0 ~ /\)/) { print c+0; printed=1; exit }
    }
  }
  END { if (!printed) print c+0 }
' "$PKGFILE")

[[ -n "${src_entry_count:-}" ]] || die "Failed to parse ${src_var} entries in $PKGFILE"

# Count existing quoted entries in the target sha array
entry_count=$(awk -v start="$sha_line" -v var="${sha_var}" '
  NR < start { next }
  {
    if (!inarr) {
      line_head=$0
      while (length(line_head)>0 && (substr(line_head,1,1)==" " || substr(line_head,1,1)=="\t")) line_head=substr(line_head,2)
      if (index(line_head, var "(")==1 || index(line_head, var "=(")==1) { inarr=1 }
    }
    if (inarr) {
      line=$0
      while (match(line, /"[^"]*"|\047[^\047]*\047/)) {
        c++
        line=substr(line, RSTART+RLENGTH)
      }
      if ($0 ~ /\)/) { print c+0; printed=1; exit }
    }
  }
  END { if (!printed) print c+0 }
' "$PKGFILE")

[[ -n "${entry_count:-}" ]] || die "Failed to parse ${sha_var} entries in $PKGFILE"

# Match the number of source entries up to 9.
desired_n="$src_entry_count"
if [[ -z "${desired_n:-}" ]]; then desired_n=0; fi
if (( desired_n > 9 )); then desired_n=9; fi

# Prepare up to 9 computed checksums aligned to source entries
sha1="$sha_bin"
sha2="$sha_src"
sha3=""; sha4=""; sha5=""; sha6=""; sha7=""; sha8=""; sha9=""

# Extract source tokens for computing additional checksums (3..desired_n)
src_token_list=$(awk -v start="$src_line" -v var="${src_var}" '
  NR < start { next }
  {
    if (!inarr) {
      line_head=$0
      while (length(line_head)>0 && (substr(line_head,1,1)==" " || substr(line_head,1,1)=="\t")) line_head=substr(line_head,2)
      if (index(line_head, var "(")==1 || index(line_head, var "=(")==1) { inarr=1 }
    }
    if (inarr) {
      line=$0
      while (match(line, /"[^"]*"|\047[^\047]*\047/)) {
        tok=substr(line,RSTART,RLENGTH)
        gsub(/^["\047]|["\047]$/, "", tok)
        print tok
        line=substr(line, RSTART+RLENGTH)
      }
      if ($0 ~ /\)/) { exit }
    }
  }
' "$PKGFILE")

pkgdir_path="$(cd -- "$(dirname -- "$PKGFILE")" >/dev/null 2>&1 && pwd -P)"

for i in $(seq 3 "$desired_n"); do
  tok=$(printf "%s\n" "$src_token_list" | sed -n "${i}p")
  [[ -z "${tok:-}" ]] && continue
  raw="$tok"
  if [[ "$raw" == *"::"* ]]; then
    raw="${raw#*::}"
  fi
  sha=""
  if [[ "$raw" =~ ^https?:// ]]; then
    out="$tmpdir/src-$i"
    if curl_retry "$raw" "$out"; then
      sha=$(sha256sum "$out" | awk '{print $1}')
    fi
  else
    cand="$raw"
    if [[ ! -f "$cand" ]]; then
      cand="$pkgdir_path/$raw"
    fi
    if [[ -f "$cand" ]]; then
      sha=$(sha256sum "$cand" | awk '{print $1}')
    fi
  fi
  if [[ -n "${sha:-}" ]]; then
    eval "sha${i}=\"$sha\""
  fi
done

# Extract existing checksums from ${sha_var}
existing_list=$(awk -v start="$sha_line" -v var="${sha_var}" '
  NR < start { next }
  {
    if (!inarr) {
      line_head=$0
      while (length(line_head)>0 && (substr(line_head,1,1)==" " || substr(line_head,1,1)=="\t")) line_head=substr(line_head,2)
      if (index(line_head, var "(")==1 || index(line_head, var "=(")==1) { inarr=1 }
    }
    if (inarr) {
      line=$0
      while (match(line, /"[^"]*"|\047[^\047]*\047/)) {
        tok=substr(line,RSTART,RLENGTH)
        gsub(/^["\047]|["\047]$/, "", tok)
        print tok
        line=substr(line, RSTART+RLENGTH)
      }
      if ($0 ~ /\)/) { exit }
    }
  }
' "$PKGFILE")
# existing_1 and existing_2 are not needed - we access the list directly via loop

# Skip if unchanged
unchanged=false
# Recompute unchanged across all available computed hashes (up to desired_n)
unchanged=true
for i in $(seq 1 "$desired_n"); do
  newv=$(eval "printf '%s' \"\${sha${i}:-}\"")
  if [[ -n "${newv:-}" ]]; then
    oldv=$(printf "%s\n" "$existing_list" | sed -n "${i}p")
    if [[ "${newv}" != "${oldv:-}" ]]; then
      unchanged=false
      break
    fi
  fi
done

# Print summary and confirm
echo "----------------------------------------" >&2
echo "PKGFILE:    $PKGFILE" >&2
echo "Repo:       $REPO" >&2
echo "Tag:        $TAG (version: $VERSION)" >&2
echo "Asset:      $ASSET_NAME" >&2
echo "Binary URL: $BINARY_URL" >&2
echo "Source URL: $SOURCE_URL" >&2
echo "Checksums (existing -> new):" >&2
for i in $(seq 1 "$desired_n"); do
  oldv=$(printf "%s\n" "$existing_list" | sed -n "${i}p")
  case "$i" in
    1) label="binary"; newv="$sha_bin";;
    2) label="source"; newv="$sha_src";;
    *) label="extra"; newv="$(eval "printf '%s' \"\${sha${i}:-}\"")";;
  esac
  printf "  %d) %s: %s -> %s\n" "$i" "$label" "${oldv:-<none>}" "${newv:-<none>}" >&2
done
if $unchanged; then
  echo "ℹ️ Checksums are unchanged. Nothing to do." >&2
  exit 0
fi

if ! $DRY_RUN; then
  if [[ "$YES" != true ]]; then
    if ! is_interactive; then
      die_code $E_NONINTERACTIVE "Confirmation required. Re-run with --yes for non-interactive mode."
    fi
    read -r -p "Proceed to update ${sha_var} in $PKGFILE? [Y/n] " ans
    ans=${ans:-Y}
    if [[ ! "$ans" =~ ^[Yy]$ ]]; then
      echo "Aborted." >&2
      exit 0
    fi
  fi
fi

if $DRY_RUN; then
  echo "ℹ️ [dry-run] Found ${entry_count} existing ${sha_var} entries." >&2
  echo "ℹ️ [dry-run] Source entries in ${src_var}: ${src_entry_count} -> will set first ${desired_n} checksum(s)." >&2
  for i in $(seq 1 "$desired_n"); do
    oldv=$(printf "%s\n" "$existing_list" | sed -n "${i}p")
    case "$i" in
      1) label="binary"; newv="$sha_bin";;
      2) label="source"; newv="$sha_src";;
      *) label="extra"; newv="$(eval "printf '%s' \"\${sha${i}:-}\"")";;
    esac
    printf "ℹ️ [dry-run]   %d) %s: %s -> %s\n" "$i" "$label" "${oldv:-<none>}" "${newv:-<none>}" >&2
  done
  if (( entry_count < desired_n )); then
    echo "ℹ️ [dry-run] Missing entries will be inserted before the closing ')'." >&2
  fi
else
  log_step "Updating ${sha_var} array (multi-line safe)"
  awk -v var="${sha_var}" -v desired_n="$desired_n" \
      -v sha1="$sha1" -v sha2="$sha2" -v sha3="$sha3" -v sha4="$sha4" -v sha5="$sha5" -v sha6="$sha6" -v sha7="$sha7" -v sha8="$sha8" -v sha9="$sha9" '
    function repl(i) {
      if (i==1) return sha1;
      else if (i==2) return sha2;
      else if (i==3) return sha3;
      else if (i==4) return sha4;
      else if (i==5) return sha5;
      else if (i==6) return sha6;
      else if (i==7) return sha7;
      else if (i==8) return sha8;
      else if (i==9) return sha9;
      return "";
    }
    BEGIN{ inarr=0; idx=0 }
    {
      if (!inarr) {
        line_head=$0
        while (length(line_head)>0 && (substr(line_head,1,1)==" " || substr(line_head,1,1)=="\t")) line_head=substr(line_head,2)
        if (index(line_head, var "(")==1 || index(line_head, var "=(")==1) { inarr=1 }
      }
      if (inarr) {
        line=$0
        out=""
        while (match(line, /"[^"]*"|\047[^\047]*\047/)) {
          pre=substr(line,1,RSTART-1)
          tok=substr(line,RSTART,RLENGTH)
          post=substr(line,RSTART+RLENGTH)
          idx++
          r=repl(idx)
          if (length(r) > 0) { tok=sprintf("%c%s%c", 39, r, 39) }
          out=out pre tok
          line=post
        }
        $0=out line
        # If array closes on this line, insert any missing entries before )
        if ($0 ~ /\)/) {
          add=""
          for (k=idx+1; k<=desired_n; k++) {
            r=repl(k)
            if (length(r) > 0) add=add sprintf(" %c%s%c", 39, r, 39)
          }
          if (length(add) > 0) sub(/\)/, add ")")
          inarr=0
        }
      }
      print
    }
  ' "$PKGFILE" > "$PKGFILE.tmp" && mv "$PKGFILE.tmp" "$PKGFILE"
  echo "✅ Updated ${sha_var} in $PKGFILE" >&2
  if [[ "$UPDATE_SRCINFO" == true ]]; then
    if command -v makepkg >/dev/null 2>&1; then
      makepkg --printsrcinfo > "$(dirname "$PKGFILE")/.SRCINFO" || die_code $E_PARSE "Failed to update .SRCINFO"
      echo "✅ Updated .SRCINFO" >&2
    else
      echo "⚠️ makepkg not found; skipping .SRCINFO update" >&2
    fi
  fi
fi

echo >&2
echo "ℹ️ Next steps:" >&2
echo "  ℹ️ makepkg --printsrcinfo > .SRCINFO" >&2
echo "  ℹ️ git add . && git commit -m 'Update checksums for ${TAG}'" >&2

exit 0
