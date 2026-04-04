#!/usr/bin/env bash
# Check Pacsea repos example TOML: parse/validate like repos.conf, optional HTTP reachability.
#
# Validates: TOML parse, required name/results_filter, canonical results_filter key, duplicate
# name (case-insensitive), rejected preset=, http(s) server and mirrorlist_url, key_id hex length
# (Pacsea apply), unknown keys (warn). HTTP mode curls the same URL pacman would use for the
# sync DB: <Server after $repo/$arch>/<name>.db. For CachyOS hosts defaulting to x86_64, *-v3 / *-v4
# section names are probed with x86_64_v3 / x86_64_v4 (CDN path). Other 404s: mirror drift, etc.
# Online mode also runs gpg --recv-keys into a throwaway GNUPGHOME and checks the key fingerprint
# matches key_id (full 40-hex or long/short id suffix), so copy-paste blocks are keyserver-safe.
#
# Usage:
#   ./dev/scripts/check_repos_example.sh [options] [path/to/repos.conf]
#
# Options:
#   --offline     Skip curl and gpg keyserver checks (TOML + logic only).
#   --skip-keys   Skip gpg key fetch/verify (still checks key_id hex length). Implied by --offline.
#   --arch ARCH   Base $arch for HTTP probes (default: uname -m). CachyOS *-v3/*-v4 rows still
#                 override x86_64 → x86_64_v3 / x86_64_v4 for that row only.
#   -h, --help    Show help.
#
# Requires: Python 3.11+ (stdlib tomllib), curl (unless --offline), gpg (unless --offline --skip-keys).
#
# Environment:
#   FORCE_COLOR   If 1/true/yes/always, colorize even when stdout/stderr is not a TTY
#                 (overrides NO_COLOR).
#   NO_COLOR      If set to any non-empty value, disable colors when FORCE_COLOR is unset.

set -euo pipefail

# Colors for stderr: TTY, or FORCE_COLOR (same rules as embedded Python).
# Use ${FORCE_COLOR-} so nounset does not trip when the variable is unset.
_fc_lc="${FORCE_COLOR-}"
_fc_lc="${_fc_lc,,}"
if [[ "$_fc_lc" == "1" || "$_fc_lc" == "true" || "$_fc_lc" == "yes" || "$_fc_lc" == "always" ]]; then
    C_ERR=$'\033[1;31m'
    C_RST=$'\033[0m'
elif [[ -z "${NO_COLOR:-}" ]] && [[ -t 2 ]]; then
    C_ERR=$'\033[1;31m'
    C_RST=$'\033[0m'
else
    C_ERR=""
    C_RST=""
fi
unset -v _fc_lc

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
DEFAULT_CONF="${REPO_ROOT}/config/examples/repos_example.conf"

OFFLINE=0
SKIP_KEYS=0
ARCH_OVERRIDE=""
CONF=""

usage() {
    sed -n '2,22p' "$0" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --offline)
            OFFLINE=1
            shift
            ;;
        --skip-keys)
            SKIP_KEYS=1
            shift
            ;;
        --arch)
            if [[ $# -lt 2 ]]; then
                echo "${C_ERR}error:${C_RST} --arch needs a value" >&2
                exit 2
            fi
            ARCH_OVERRIDE="$2"
            shift 2
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        -*)
            echo "${C_ERR}error:${C_RST} unknown option: $1" >&2
            exit 2
            ;;
        *)
            if [[ -n "${CONF}" ]]; then
                echo "${C_ERR}error:${C_RST} unexpected extra argument: $1" >&2
                exit 2
            fi
            CONF="$1"
            shift
            ;;
    esac
done

if [[ -z "${CONF}" ]]; then
    CONF="${DEFAULT_CONF}"
fi

if [[ ! -f "${CONF}" ]]; then
    echo "${C_ERR}error:${C_RST} file not found: ${CONF}" >&2
    exit 2
fi

export PACSEA_REPOS_CHECK_FILE="${CONF}"
export PACSEA_REPOS_CHECK_OFFLINE="${OFFLINE}"
export PACSEA_REPOS_CHECK_SKIP_KEYS="${SKIP_KEYS}"
export PACSEA_REPOS_CHECK_ARCH="${ARCH_OVERRIDE}"

if ! python3 -c 'import tomllib' 2>/dev/null; then
    echo "${C_ERR}error:${C_RST} Python 3.11+ required (stdlib tomllib)." >&2
    exit 2
fi

exec python3 - <<'PY'
"""Validate repos TOML the way Pacsea does, plus optional HTTP checks."""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
from typing import Any, TextIO

ALLOWED_KEYS = frozenset(
    {
        "id",
        "enabled",
        "preset",
        "name",
        "results_filter",
        "server",
        "sig_level",
        "key_id",
        "key_server",
        "mirrorlist",
        "mirrorlist_url",
    }
)


def canonical_results_filter_key(raw: str) -> str:
    lower = raw.strip().lower()
    out: list[str] = []
    prev_sep = True
    for ch in lower:
        if ch.isalnum() and ch.isascii():
            out.append(ch)
            prev_sep = False
        elif not prev_sep and out:
            out.append("_")
            prev_sep = True
    s = "".join(out)
    while s.endswith("_"):
        s = s[:-1]
    return s


def normalized_fingerprint(key_id: str) -> str | None:
    hex_digits = "".join(c for c in key_id if c in "0123456789abcdefABCDEF")
    if len(hex_digits) < 8:
        return None
    return hex_digits.upper()


def non_empty_trim(v: Any) -> str | None:
    if v is None:
        return None
    if not isinstance(v, str):
        return None
    t = v.strip()
    return t if t else None


def expand_server(url: str, repo_name: str, arch: str) -> str:
    return url.replace("$repo", repo_name).replace("$arch", arch)


def pacman_db_probe_url(expanded_server: str, repo_name: str | None) -> str:
    """Pacman syncs ``<Server>/<repo>.db``; probing the bare directory often 404s on GET."""
    base = expanded_server.rstrip("/")
    if repo_name:
        return f"{base}/{repo_name}.db"
    return base


def arch_for_http_probe(base_arch: str, repo_name: str | None, server: str) -> str:
    """What: Pick ``$arch`` for expanding ``server`` when probing HTTP.

    Inputs:
    - ``base_arch``: Host or ``--arch`` value (e.g. ``x86_64``).
    - ``repo_name``: Pacman repo section ``name``.
    - ``server``: Raw ``server =`` URL from TOML.

    Output:
    - Architecture string to substitute for ``$arch``.

    Details:
    - CachyOS CDN uses path segments ``x86_64_v3`` / ``x86_64_v4`` (underscores) for optimized
      tiers; generic ``[cachyos]`` stays on ``x86_64``. When ``base_arch`` is ``x86_64``, map
      ``*-v3`` / ``*-v4`` section names so probes match mirrors without requiring ``--arch``.
    """
    if "cachyos" not in server.lower():
        return base_arch
    if base_arch != "x86_64":
        return base_arch
    nm = (repo_name or "").lower()
    if nm.endswith("-v4"):
        return "x86_64_v4"
    if nm.endswith("-v3"):
        return "x86_64_v3"
    return base_arch


def curl_http_code(url: str, timeout_s: int = 25) -> tuple[int, str]:
    """Return (exit_code, http_code_or_error). http_code is 3 digits or '000' / empty."""
    try:
        p = subprocess.run(
            [
                "curl",
                "-sS",
                "-L",
                "-o",
                "/dev/null",
                "-w",
                "%{http_code}",
                "--max-time",
                str(timeout_s),
                url,
            ],
            check=False,
            capture_output=True,
            text=True,
        )
    except FileNotFoundError:
        return 127, "curl-missing"
    code = (p.stdout or "").strip()
    if not code.isdigit():
        code = "000"
    return p.returncode, code


def key_hex_material(raw: str) -> str:
    """Return uppercase hex digits from a user ``key_id`` string (Pacsea-style)."""
    return "".join(c for c in raw if c in "0123456789abcdefABCDEF").upper()


def fpr_lines_from_gpg_colons(text: str) -> list[str]:
    """What: Collect 40-hex fingerprints from ``gpg --list-keys --with-colons`` output.

    Inputs:
    - ``text``: Stdout from gpg.

    Output:
    - Uppercase fingerprint strings in file order.

    Details:
    - Parses ``fpr:`` records only.
    """
    out: list[str] = []
    for line in text.splitlines():
        if not line.startswith("fpr:"):
            continue
        m = re.search(r"([0-9A-Fa-f]{40})", line)
        if m:
            out.append(m.group(1).upper())
    return out


def key_id_matches_fpr_list(want_hex: str, fprs: list[str]) -> bool:
    """What: Decide if OpenPGP fingerprints satisfy the ``key_id`` from repos.conf.

    Inputs:
    - ``want_hex``: Hex-only material from TOML (any length ≥ 8 after prior validation).
    - ``fprs``: Full 40-char key fingerprints from gpg.

    Output:
    - True when any listed fingerprint matches full fingerprint or long/short id suffix.

    Details:
    - 40 hex → exact match. Shorter → ``fpr.endswith(want_hex)`` (long key id / short id).
    """
    w = want_hex.upper()
    if len(w) == 40:
        return any(fp == w for fp in fprs)
    return any(fp.endswith(w) for fp in fprs)


def verify_key_on_keyserver(
    key_id_raw: str,
    key_server: str | None,
    recv_timeout_s: int = 90,
) -> tuple[str, str]:
    """What: Fetch OpenPGP key from a keyserver into a temp homedir and verify ``key_id``.

    Inputs:
    - ``key_id_raw``: ``key_id`` value from TOML.
    - ``key_server``: Optional ``key_server`` host or URL; defaults to Ubuntu keyserver.
    - ``recv_timeout_s``: Subprocess timeout for ``--recv-keys``.

    Output:
    - ``('ok', primary_fpr40)``, ``('recv_fail', message)``, ``('mismatch', message)``, or
      ``('bad', message)``.

    Details:
    - Does not touch the user’s keyring. Uses an empty temp ``GNUPGHOME``.
    """
    want = key_hex_material(key_id_raw)
    if len(want) < 8:
        return "bad", "fewer than 8 hex digits"
    ks = (key_server or "keyserver.ubuntu.com").strip()
    if not ks:
        ks = "keyserver.ubuntu.com"

    tmp = tempfile.mkdtemp(prefix="pacsea-repos-keycheck-")
    try:
        recv = subprocess.run(
            [
                "gpg",
                "--homedir",
                tmp,
                "--batch",
                "--no-tty",
                "--keyserver",
                ks,
                "--recv-keys",
                "0x" + want,
            ],
            check=False,
            capture_output=True,
            text=True,
            timeout=recv_timeout_s,
        )
        if recv.returncode != 0:
            detail = (recv.stderr or recv.stdout or "").strip() or f"exit {recv.returncode}"
            return "recv_fail", detail[:500]

        lst = subprocess.run(
            [
                "gpg",
                "--homedir",
                tmp,
                "--batch",
                "--no-tty",
                "--list-keys",
                "--with-colons",
                "0x" + want,
            ],
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
        fprs = fpr_lines_from_gpg_colons(lst.stdout)
        if not fprs:
            return "mismatch", "recv succeeded but no fingerprint lines in --list-keys output"

        if not key_id_matches_fpr_list(want, fprs):
            return (
                "mismatch",
                f"key from {ks!r} has fingerprint(s) {fprs[:3]!r}...; "
                f"none match key_id suffix/full {want!r}",
            )
        primary = fprs[0]
        return "ok", primary
    except subprocess.TimeoutExpired:
        return "recv_fail", f"gpg --recv-keys timed out after {recv_timeout_s}s"
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def _use_ansi(stream: TextIO) -> bool:
    """Return True if we should emit ANSI styles on ``stream``."""
    force = os.environ.get("FORCE_COLOR", "").strip().lower()
    if force in ("1", "true", "yes", "always"):
        return True
    if os.environ.get("NO_COLOR", "").strip():
        return False
    return stream.isatty()


class TermStyle:
    """Minimal ANSI SGR codes; empty strings when colors are disabled."""

    def __init__(self, stream: TextIO) -> None:
        use = _use_ansi(stream)
        if use:
            self.reset = "\033[0m"
            self.bold = "\033[1m"
            self.dim = "\033[2m"
            self.red = "\033[31m"
            self.green = "\033[32m"
            self.yellow = "\033[33m"
            self.blue = "\033[34m"
            self.cyan = "\033[36m"
        else:
            self.reset = self.bold = self.dim = ""
            self.red = self.green = self.yellow = self.blue = self.cyan = ""


def _print_header(
    path: str, offline: bool, skip_keys: bool, arch: str, st_out: TermStyle
) -> None:
    """Print a short banner so long runs are easier to scan."""
    mode = "offline (no HTTP, no gpg fetch)" if offline else "online (HTTP + optional gpg)"
    keys = "skipped" if skip_keys else "gpg keyserver verify"
    if not st_out.bold:
        print(f"Pacsea repos check — {path}")
        print(f"Mode: {mode}  keys: {keys}  arch: {arch}")
        return
    width = 62
    rule = f"{st_out.dim}{'─' * width}{st_out.reset}"
    print(rule)
    title = f"{st_out.bold}{st_out.cyan}Pacsea{st_out.reset} repos example check"
    print(f"  {title}")
    print(f"  {st_out.dim}file{st_out.reset}  {path}")
    print(f"  {st_out.dim}mode{st_out.reset}  {mode}  {st_out.dim}keys{st_out.reset}  {keys}")
    print(f"  {st_out.dim}arch{st_out.reset}  {arch}")
    print(rule)
    print()


def _print_summary(
    path: str,
    repo_count: int,
    errors: int,
    warnings: int,
    offline: bool,
    skip_keys: bool,
    arch: str,
    st_out: TermStyle,
) -> None:
    """Print a boxed summary with colored counts."""
    mode = "offline" if offline else "online"
    if not st_out.bold:
        print(
            f"--- summary: file={path!r} repos={repo_count} "
            f"errors={errors} warnings={warnings} offline={offline} skip_keys={skip_keys} "
            f"arch={arch!r} ---"
        )
        return
    w = 62
    rule = f"{st_out.dim}{'─' * w}{st_out.reset}"
    err_s = f"{st_out.red}{st_out.bold}{errors}{st_out.reset}" if errors else f"{st_out.green}0{st_out.reset}"
    warn_s = (
        f"{st_out.yellow}{st_out.bold}{warnings}{st_out.reset}"
        if warnings
        else f"{st_out.dim}0{st_out.reset}"
    )
    status = (
        f"{st_out.green}{st_out.bold}OK{st_out.reset} — no errors"
        if errors == 0
        else f"{st_out.red}{st_out.bold}FAILED{st_out.reset} — {errors} error(s)"
    )
    print()
    print(rule)
    print(f"  {st_out.bold}Summary{st_out.reset}  {status}")
    print(rule)
    print(f"  {st_out.dim}file{st_out.reset}      {path}")
    print(f"  {st_out.dim}repos{st_out.reset}     {repo_count}")
    print(f"  {st_out.dim}errors{st_out.reset}    {err_s}")
    print(f"  {st_out.dim}warnings{st_out.reset}  {warn_s}")
    print(f"  {st_out.dim}mode{st_out.reset}      {mode}")
    print(f"  {st_out.dim}keys{st_out.reset}      {'skipped' if skip_keys else 'checked'}")
    print(f"  {st_out.dim}arch{st_out.reset}      {arch}")
    print(rule)


def main() -> int:
    path = os.environ.get("PACSEA_REPOS_CHECK_FILE", "")
    offline = os.environ.get("PACSEA_REPOS_CHECK_OFFLINE", "0") == "1"
    skip_keys = offline or os.environ.get("PACSEA_REPOS_CHECK_SKIP_KEYS", "0") == "1"
    arch_override = os.environ.get("PACSEA_REPOS_CHECK_ARCH", "").strip()

    st_err = TermStyle(sys.stderr)
    st_out = TermStyle(sys.stdout)

    if not path or not os.path.isfile(path):
        pre = f"{st_err.bold}{st_err.red}error:{st_err.reset}" if st_err.bold else "error:"
        print(f"{pre} bad PACSEA_REPOS_CHECK_FILE: {path!r}", file=sys.stderr)
        return 2

    try:
        arch = arch_override or os.uname().machine  # type: ignore[attr-defined]
    except AttributeError:
        arch = arch_override or "x86_64"

    _print_header(path, offline, skip_keys, arch, st_out)

    try:
        raw = open(path, "rb").read()
        data = tomllib.loads(raw.decode())
    except tomllib.TOMLDecodeError as e:
        pre = f"{st_err.bold}{st_err.red}error:{st_err.reset}" if st_err.bold else "error:"
        print(f"{pre} TOML parse failed: {e}", file=sys.stderr)
        return 1

    repos = data.get("repo")
    if repos is None:
        pre = f"{st_err.bold}{st_err.red}error:{st_err.reset}" if st_err.bold else "error:"
        print(
            f"{pre} no [[repo]] tables (expected TOML array of tables under key 'repo')",
            file=sys.stderr,
        )
        return 1
    if not isinstance(repos, list):
        pre = f"{st_err.bold}{st_err.red}error:{st_err.reset}" if st_err.bold else "error:"
        print(f"{pre} 'repo' must be a TOML array", file=sys.stderr)
        return 1

    errors = 0
    warnings = 0

    def err(msg: str) -> None:
        nonlocal errors
        errors += 1
        pre = f"{st_err.bold}{st_err.red}error:{st_err.reset}" if st_err.bold else "error:"
        print(f"{pre} {msg}", file=sys.stderr)

    def warn(msg: str) -> None:
        nonlocal warnings
        warnings += 1
        pre = f"{st_err.bold}{st_err.yellow}warn:{st_err.reset}" if st_err.bold else "warn:"
        print(f"{pre} {msg}", file=sys.stderr)

    def info(msg: str) -> None:
        pre = f"{st_out.dim}{st_out.cyan}info:{st_out.reset}" if st_out.dim else "info:"
        print(f"{pre} {msg}")

    seen_names: dict[str, int] = {}
    gpg_checked: set[tuple[str, str]] = set()
    gpg_warned = False

    for idx, row in enumerate(repos):
        label = f"[[repo]] #{idx + 1}"
        if not isinstance(row, dict):
            err(f"{label}: table must be a mapping, got {type(row).__name__}")
            continue

        unknown = set(row) - ALLOWED_KEYS
        if unknown:
            warn(f"{label}: unknown keys (ignored by Pacsea serde): {', '.join(sorted(unknown))}")

        preset = non_empty_trim(row.get("preset"))
        if preset is not None:
            err(
                f"{label}: `preset` is not supported; use a full [[repo]] block "
                "(see config/examples in the Pacsea tree)."
            )

        name = non_empty_trim(row.get("name"))
        if name is None:
            err(f"{label}: repo `name` is missing or empty")
        else:
            nl = name.lower()
            if nl in seen_names:
                err(
                    f"{label}: duplicate repo `name` {name!r} "
                    f"(case-insensitive; first at table #{seen_names[nl] + 1})"
                )
            else:
                seen_names[nl] = idx

        rf = non_empty_trim(row.get("results_filter"))
        if rf is None:
            err(f"{label}: repo `results_filter` is missing or empty")
        elif not canonical_results_filter_key(rf):
            err(
                f"{label}: `results_filter` has no ASCII letters/digits; "
                "cannot form results_filter_show_<id> key"
            )

        server = non_empty_trim(row.get("server"))
        ml = non_empty_trim(row.get("mirrorlist"))
        ml_url = non_empty_trim(row.get("mirrorlist_url"))

        has_apply_source = bool(server or ml or ml_url)
        if has_apply_source:
            if server is not None and not (
                server.startswith("http://") or server.startswith("https://")
            ):
                err(f"{label}: `server` must start with http:// or https:// (got {server[:40]!r}...)")
            if ml_url is not None and not (
                ml_url.startswith("http://") or ml_url.startswith("https://")
            ):
                err(
                    f"{label}: `mirrorlist_url` must start with http:// or https:// "
                    f"(got {ml_url[:40]!r}...)"
                )
            if ml is not None and not ml.startswith("/"):
                warn(f"{label}: `mirrorlist` is not an absolute path: {ml!r}")

        kid = non_empty_trim(row.get("key_id"))
        ks = non_empty_trim(row.get("key_server"))
        if kid is not None:
            if normalized_fingerprint(kid) is None:
                err(
                    f"{label}: `key_id` must contain at least 8 hexadecimal digits "
                    f"after stripping non-hex (Pacsea apply / key refresh)"
                )
        if ks is not None and kid is None:
            warn(f"{label}: `key_server` set without `key_id` (Pacsea will not use key_server)")

        sig = non_empty_trim(row.get("sig_level"))
        if sig is not None:
            if not re.fullmatch(r"[A-Za-z][A-Za-z0-9_ ]*", sig):
                warn(
                    f"{label}: `sig_level` has unusual characters; "
                    "verify against pacman.conf SigLevel syntax"
                )

        rid = non_empty_trim(row.get("id"))
        if rid is not None and not rid.replace("-", "").replace("_", "").isalnum():
            warn(f"{label}: `id` is non-empty but looks unusual: {rid!r}")

        if (
            not skip_keys
            and kid is not None
            and normalized_fingerprint(kid) is not None
        ):
            hex_u = key_hex_material(kid)
            ks_use = (ks or "keyserver.ubuntu.com").strip() or "keyserver.ubuntu.com"
            dedup_k = (hex_u, ks_use.lower())
            if dedup_k not in gpg_checked:
                gpg_checked.add(dedup_k)
                if shutil.which("gpg") is None:
                    if not gpg_warned:
                        warn(
                            "gpg not found on PATH; install gnupg or use --offline / --skip-keys "
                            "to skip OpenPGP keyserver checks"
                        )
                        gpg_warned = True
                else:
                    status, detail = verify_key_on_keyserver(kid, ks)
                    nm = name or "?"
                    if status == "ok":
                        info(
                            f"{label} [{nm}]: signing key OK (OpenPGP fpr {detail} from {ks_use!r})"
                        )
                    elif status == "recv_fail":
                        warn(
                            f"{label} [{nm}]: could not fetch key_id from {ks_use!r}: {detail}"
                        )
                    elif status == "mismatch":
                        err(
                            f"{label} [{nm}]: key from keyserver does not match key_id: {detail}"
                        )
                    else:
                        err(f"{label} [{nm}]: key check failed: {detail}")

        if offline or not has_apply_source:
            continue

        if server is not None:
            probe_arch = arch_for_http_probe(arch, name, server)
            expanded = expand_server(server, name or "unknown", probe_arch)
            probe = pacman_db_probe_url(expanded, name)
            rc, code = curl_http_code(probe)
            nm = name or "?"
            if rc == 127:
                err("curl not found; install curl or use --offline")
                break
            if rc != 0 or code == "000":
                err(
                    f"{label} [{nm}]: server DB not reachable "
                    f"(curl exit {rc}, http {code}; probed {probe})"
                )
            elif code.startswith("4") or code.startswith("5"):
                warn(
                    f"{label} [{nm}]: server returned HTTP {code} when probing {probe} "
                    f"(expanded Server line: {expanded})"
                )
            else:
                arch_note = f" arch={probe_arch}" if probe_arch != arch else ""
                info(f"{label} [{nm}]: server OK (HTTP {code}):{arch_note} {probe}")

        if ml_url is not None and server is None and ml is None:
            rc, code = curl_http_code(ml_url)
            nm = name or "?"
            if rc == 127:
                err("curl not found; install curl or use --offline")
                break
            if rc != 0 or code == "000":
                err(
                    f"{label} [{nm}]: mirrorlist_url not reachable "
                    f"(curl exit {rc}, http {code}): {ml_url}"
                )
            elif code.startswith("4") or code.startswith("5"):
                warn(
                    f"{label} [{nm}]: mirrorlist_url HTTP {code}: {ml_url}"
                )
            else:
                info(f"{label} [{nm}]: mirrorlist_url OK (HTTP {code}): {ml_url}")

        if ml is not None and ml.startswith("/") and not os.path.isfile(ml):
            info(
                f"{label} [{name or '?'}]: `mirrorlist` path not found on this machine "
                f"(expected for example files): {ml}"
            )

    _print_summary(path, len(repos), errors, warnings, offline, skip_keys, arch, st_out)
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
PY
