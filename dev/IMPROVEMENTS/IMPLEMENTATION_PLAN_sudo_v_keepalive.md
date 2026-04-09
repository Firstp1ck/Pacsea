# Implementation plan: sudo credential keepalive (`sudo -v`) during long Pacsea jobs

**Goal:** Reduce mid-run failures during long installs, updates, and other privileged operations when the user’s **sudo timestamp** expires before the child toolchain (`paru`, `makepkg`, chained `pacman`, etc.) asks for privilege again. While the user still has a **valid** cached sudo ticket, Pacsea periodically runs **`sudo -v`** / **`sudo -nv`** in the **background** to **refresh** the sudo credential cache, without blocking the main PTY executor.

**Scope note:** This keepalive applies to **`sudo` only**. **`doas`** has no supported equivalent in Pacsea’s privilege model (see below). **PAM / fingerprint** does not change the keepalive design; it only affects **how** the user performs the **initial** authentication that creates the sudo ticket.

**Non-goal:** Replacing `sudoers` configuration (`timestamp_timeout`, `passwd_timeout`), replacing interactive authentication modes, or injecting passwords into an already-running PTY when the ticket is **fully** expired (that remains a separate, high-complexity topic).

---

## Problem statement

- Default `sudo` policy often uses a **short** `timestamp_timeout` (for example five or fifteen minutes).
- AUR builds and large sync operations can run **longer** than that window.
- Subsequent `sudo` invocations inside the same logical “session” may then **prompt again** (or fail in non-interactive paths). Users who are not watching the embedded log can hit **`passwd_timeout`** or miss the prompt, which looks like a stuck or failed install.

**What `sudo -v` does:** When a ticket is still valid (or policy allows refresh without a new secret), `sudo -v` **updates** the user’s cached credentials. It is typically **fast** and **non-interactive** in that situation.

**Hard limit:** If the ticket is **already** expired or missing, `sudo -v` may **require** authentication again. That cannot be solved silently without either user interaction or reusing a stored password (Pacsea already has distinct flows for `auth_mode` / `sudo -S`). The keepalive only **extends** an **existing** valid window.

---

## Other privilege backends: `doas` and PAM / fingerprint

Pacsea’s privilege layer is documented in [`src/logic/privilege.rs`](../../src/logic/privilege.rs). The keepalive feature must respect **different rules per tool**, not assume “whatever works for sudo works everywhere.”

### `doas` (OpenDoas)

| Topic | Rule for this plan |
|--------|-------------------|
| **`sudo -v`-style refresh** | **Not applicable.** Module docs explicitly: *Credential refresh \| `sudo -v` \| **NOT supported** (doas)*. `PrivilegeCapabilities::supports_credential_refresh` is **`false`** for `Doas`. |
| **Stdin password / `sudo -S`** | Doas **does not** support piping the password from Pacsea; the in-app password modal is **coerced to interactive** for doas. |
| **What doas uses instead** | Policy lives in **`/etc/doas.conf`**. OpenDoas supports a **`persist`** option on rules: after a successful authentication, further `doas` invocations may **skip re-prompting** for a configurable period (behavior is **doas.conf**, not a separate “`-v`” command Pacsea can call). |
| **Implementation stance** | When `active_tool() == Doas`, the keepalive timer should **no-op** (or never arm). Document in settings/rustdoc: *long-job credential extension via background refresh is a **sudo-only** feature; doas users should rely on **`persist`** (and appropriate timeouts in `doas.conf`) or switch **`privilege_tool = sudo`** if they need this behavior.* |
| **Phase 3 “research”** | Replace with: confirm no stable portable “refresh” API across doas versions; **do not** ship speculative `doas` subprocesses that might prompt on a headless side channel. |

### PAM, fingerprint (`pam_fprintd`), and `auth_mode`

Fingerprint authentication is **not** a separate privilege tool in Pacsea; it is part of **how PAM authenticates** when `sudo` or `doas` runs **interactively** on a TTY.

| Topic | Rule for this plan |
|--------|-------------------|
| **Detection** | Pacsea can detect likely fingerprint setups via `is_fingerprint_available` (uses `detect_pam_fingerprint` + `detect_fprintd_enrolled` in [`privilege.rs`](../../src/logic/privilege.rs)). Informational only today (e.g. password-modal hint). |
| **`auth_mode = interactive`** | Sudo/doas prompts run in a context where PAM can offer **password or fingerprint** (order depends on `/etc/pam.d/sudo` or `doas`). This is the **right** mode when the user wants fingerprint for **live** prompts inside the PTY-backed install. |
| **`auth_mode = prompt` + `sudo -S`** | Known limitation (see `settings.conf` skeleton): if **`pam_fprintd` runs before `pam_unix`**, stdin password piping can **stall**. Fingerprint may **not** run as expected on that path. Keepalive does **not** fix PAM ordering. |
| **Background `sudo -nv` keepalive** | Runs **non-interactively**. It does **not** open a fingerprint UI. It only succeeds if a **sudo timestamp ticket is already valid** (same as password-only sudo). So: **fingerprint users still benefit** from keepalive **exactly when** their earlier interactive (or password-piped) auth created a **sudo** ticket that is still within `timestamp_timeout`. |
| **Ticket expired mid-job** | `sudo -nv` fails; the **next** real `sudo` inside the install may show an **interactive** PAM prompt (fingerprint + password) **on the PTY**—Pacsea still does not inject a modal into that stream. |

**Summary:** Fingerprint changes **first-factor UX**, not the keepalive mechanism. Keepalive = **silent extension of sudo’s existing timestamp**; fingerprint is irrelevant to `-nv` except indirectly (user had to authenticate somehow first).

---

## Proposed behavior

1. **When:** Only while Pacsea is running a **long-running privileged job** it owns—primarily when the **PTY executor** is active and `Modal::PreflightExec` (or equivalent) shows in-progress output. Optionally extend to other explicit multi-step privileged sequences if they share the same lifecycle hooks.
2. **What:** On a fixed **interval** (for example every **three to four minutes**, configurable), spawn a **short** subprocess: `sudo -v` (or `sudo -nv` first—see design choices below). No stdin password for the keepalive path; if refresh fails, **log** and optionally **toast once**; do **not** block the main install PTY.
3. **Respect `dry_run`:** In dry-run mode, do **not** run real `sudo -v`; optionally log a single debug line or no-op.
4. **Privilege tool:** Reuse [`crate::logic::privilege`](../../src/logic/privilege.rs). Run keepalive **only** when `active_tool() == Sudo` (or gate on `capabilities().supports_credential_refresh`). For **doas**, **never** run a keepalive subprocess; document **`persist`** in `doas.conf` as the analogous policy knob.
5. **Auth modes:**  
   - **`passwordless_only` / passwordless sudo:** Keepalive may succeed without friction.  
   - **`prompt` (in-app password):** The user already authenticated for the main command; OS sudo timestamp is usually **shared** with child `sudo` calls—keepalive can still help **extend** that timestamp while the job runs.  
   - **`interactive`:** Keepalive subprocess may lack a controlling TTY; prefer **`sudo -nv`** so a dead ticket fails fast instead of hanging. If refresh fails, rely on existing interactive behavior for the **next** privileged step (no modal injection into the PTY).

---

## Design choices (to decide during implementation)

| Topic | Option A | Option B | Recommendation |
|-------|-----------|-----------|------------------|
| Command | `sudo -v` | `sudo -nv` only | Prefer **`-nv` first** (non-interactive, no hang); optionally document that some systems behave identically for `-v` when ticket valid. |
| Interval | Fixed constant (e.g. 4 min) | `settings.conf` key | Ship a **sane default** + optional **`settings.conf`** override (e.g. `sudo_keepalive_interval_secs`, `0` = disabled) for power users. |
| Scheduler | `std::thread` + sleep from tick or executor worker | `tokio::time::interval` if runtime already in scope | Align with **existing** periodic work in [`tick_handler`](../../src/app/runtime/tick_handler.rs) or the **executor worker** in [`workers/executor.rs`](../../src/app/runtime/workers/executor.rs)—avoid duplicate timers. |
| Lifecycle | Start when executor starts; stop on `Finished` / `Error` | Tied to `AppState` flag | Single **boolean + last_run** on `AppState` or executor-side guard so keepalive **cannot** leak across jobs. |
| Failure UX | Silent after first debug log | One **toast** (“sudo session expired; job may prompt again”) | **Toast once per job** on repeated failure to avoid spam. |

---

## Relation to the current codebase

| Area | Location | Notes |
|------|-----------|--------|
| PTY executor | [`src/app/runtime/workers/executor.rs`](../../src/app/runtime/workers/executor.rs) | Long-running `execute_command_pty`; natural place to **arm/disarm** keepalive or signal app state. |
| Executor output | [`src/app/runtime/event_loop.rs`](../../src/app/runtime/event_loop.rs) (`handle_executor_output`) | `Finished` / `Error` → clear keepalive flag. |
| Privilege resolution | [`src/logic/privilege.rs`](../../src/logic/privilege.rs) | Gate keepalive on `supports_credential_refresh` (**sudo** only); doas **no-op**. |
| Fingerprint helpers | [`privilege.rs`](../../src/logic/privilege.rs) (`is_fingerprint_available`, `detect_pam_fingerprint`, `detect_fprintd_enrolled`) | Optional UX hint only; **not** required for keepalive logic. |
| Password / auth policy | [`src/logic/password.rs`](../../src/logic/password.rs), `settings.conf` | Do not run keepalive when it would contradict security expectations (document **doas** gaps). |
| Settings | [`src/theme/types.rs`](../../src/theme/types.rs), [`parse_settings.rs`](../../src/theme/settings/parse_settings.rs), [`skeletons.rs`](../../src/theme/config/skeletons.rs) | New optional keys + backward-compatible defaults. |
| Sudo wizard (related UX) | [`src/logic/sudo_timestamp_setup.rs`](../../src/logic/sudo_timestamp_setup.rs) | Optional cross-link in rustdoc or help: keepalive **complements** longer `timestamp_timeout`, does not replace it. |

---

## Testing strategy

1. **Unit tests:** Pure functions for “should run keepalive now?” (interval + flag + dry_run) with **injected** clock or last-run instant.
2. **Integration tests:** Prefer **not** calling real `sudo` in CI. Use `PACSEA_*` test env vars (pattern already used in [`password.rs`](../../src/logic/password.rs) tests) to stub the keepalive command or skip when not integration.
3. **Manual:** Long `paru`/`yay` build with **`timestamp_timeout=1`** in sudoers (test VM): confirm keepalive extends the window; confirm no duplicate prompts when ticket is valid; confirm failure path when ticket cleared with `sudo -k`.

---

## Documentation and config

- Add **rustdoc** on any new public helpers (What / Inputs / Output / Details per `AGENTS.md`).
- Update shipped **`config/settings.conf`** skeleton with commented keys and defaults (`0` = off, if offered).
- **Do not** update wiki or root `README.md` unless explicitly requested (project policy).

---

## Risks and mitigations

| Risk | Mitigation |
|------|------------|
| Background `sudo -v` prompts if ticket dead | Use **`sudo -nv`** or **timeout** wrapper; treat failure as non-fatal. |
| Spamming subprocesses | Single timer; coalesce with **last_run** timestamp. |
| doas has no `sudo -v` equivalent | **Gate** on `PrivilegeCapabilities::supports_credential_refresh`; document **`persist`** in `doas.conf`. |
| User expects fingerprint mid-job | Keepalive does not show PAM UI; only **extends** existing sudo ticket. Expired ticket → next prompt is whatever PAM does on the **PTY** (`auth_mode = interactive`). |
| Security concern (“silent” refresh) | Only extends **existing** user consent window; same as user running `sudo -v` in a shell during a build. |

---

## Suggested implementation phases

| Phase | Scope | Outcome |
|-------|--------|---------|
| **1** | Minimal: internal constant interval, `sudo -nv` only, arm on PTY executor start, disarm on `Finished`/`Error`, respect `dry_run` | MVP for Linux + sudo. |
| **2** | `settings.conf` interval + enable/disable; i18n toast on repeated failure | User-tunable. |
| **3** | Documentation + settings skeleton notes: **sudo-only** keepalive; **doas** + **`persist`**; **fingerprint** + **`auth_mode`** / PAM caveats; optional unit test that doas path never schedules keepalive | No false expectation of parity with doas. |

---

## Open questions

1. Should keepalive run during **`ExecutorRequest::Update`** (system update) only, or **any** PTY-backed request that may call `sudo` (install, remove, custom command)?
2. Is **`sudo -nv`** sufficient on all target distros, or do we need **`timeout 5 sudo -v`** to catch edge hangs?
3. Should failure increment a metric or **tracing** span for support diagnostics?
4. Should the UI show a one-line hint when **doas** is active (“Credential keepalive is disabled; use `persist` in doas.conf or `privilege_tool = sudo`”) to reduce support churn?

---

## References

- `sudoers(5)`: `timestamp_timeout`, `passwd_timeout`
- `sudo(8)`: `-v`, `-n`
- `doas.conf(5)` / OpenDoas: `persist` and rule semantics (distribution-specific; verify on Arch `extra/opendoas`)
- Pacsea: PTY executor [`src/app/runtime/workers/executor.rs`](../../src/app/runtime/workers/executor.rs), privilege table and capabilities [`src/logic/privilege.rs`](../../src/logic/privilege.rs) (module-level **sudo vs doas** matrix, `detect_pam_fingerprint`)
