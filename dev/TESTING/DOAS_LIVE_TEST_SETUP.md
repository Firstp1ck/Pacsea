# Doas replacement — live setup and testing

This guide walks through installing OpenDoas, configuring Pacsea, and verifying privileged operations on an Arch-based system.

## Prerequisites

- Arch Linux or derivative (pacman, typical `/etc/doas.conf` layout).
- A way to recover if you lock yourself out of root (another TTY, live USB, or VM snapshot) when editing privilege rules.
- Optional: build Pacsea from this repo (`cargo run -- …`) or use an installed `pacsea` package — config paths are the same idea; use **your** real `settings.conf` path.

---

## Step 1 — Install OpenDoas

```bash
sudo pacman -S opendoas
```

Confirm the binary:

```bash
command -v doas
doas -h
```

---

## Step 2 — Configure `/etc/doas.conf`

Edit as root (e.g. `sudo visudo` has no direct doas twin; use `sudoedit /etc/doas.conf` or an editor as root).

**Minimal interactive rule** (replace `YOURUSER` with your login name):

```conf
permit persist YOURUSER as root
```

- With **`persist`**, you enter the password once per session (OpenDoas behavior depends on build; typical use is repeat prompts until satisfied).
- For **passwordless testing only** (narrow the rule in production):

```conf
permit nopass YOURUSER as root
```

**Syntax check** (optional): after saving, try a trivial command:

```bash
doas true && echo ok
```

**Non-interactive probe** (what Pacsea uses for “passwordless available”):

```bash
doas -n true && echo passwordless_ok || echo needs_password_or_denied
```

---

## Step 3 — Pacsea settings

Open your Pacsea `settings.conf` (see your install docs for path; the repo ships an example under `config/settings.conf`).

### 3.1 — Choose privilege tool

| Value   | Behavior |
|---------|----------|
| `auto`  | Prefer **doas** if it is on `$PATH`, otherwise **sudo**. If **both** exist, **doas wins**. |
| `doas`  | Always use **doas**; fails clearly if `doas` is missing. |
| `sudo`  | Always use **sudo**. |

For **focused doas testing**, set:

```conf
privilege_tool = doas
```

For **“like a user who only installed opendoas”** testing:

```conf
privilege_tool = auto
```

### 3.2 — Passwordless toggle (`use_passwordless_sudo`)

The key name is historical; it applies to **whichever** privilege tool is active.

- **`false` (default)**  
  For **sudo**: in-app password flow can apply where supported.  
  For **doas**: Pacsea **skips** in-app password validation (doas cannot use stdin piping like `sudo -S`) and relies on the **terminal / PTY** for doas prompts where needed.

- **`true`**  
  Pacsea will skip the password prompt **only if** the non-interactive check succeeds (`doas -n true`). Use together with **`permit nopass`** (or equivalent) in `doas.conf` if you want that path.

Example for strict interactive doas:

```conf
privilege_tool = doas
use_passwordless_sudo = false
```

Example for passwordless doas:

```conf
privilege_tool = doas
use_passwordless_sudo = true
```

(and `permit nopass …` in `doas.conf`)

---

## Step 4 — Dry-run first

Run Pacsea with **dry-run** enabled (in-app default or `--dry-run` if your build supports it).

Check previews / logged commands:

- Privileged lines should show **`doas …`**, not **`sudo …`**, when `privilege_tool` is `doas` or `auto` with doas selected.

---

## Step 5 — Live tests (ordered)

Run these only when you accept real package changes on the machine.

1. **Official repo package — install**  
   Add a tiny package, execute install, confirm `doas` prompts in the **terminal** (if not passwordless).

2. **Official package — remove**  
   Remove a non-critical package you just installed.

3. **System update path**  
   Trigger update from the UI; confirm chained commands use `doas` where you expect.

4. **Downgrade** (if you use `downgrade`)  
   Confirm the spawned command uses `doas downgrade …` and that availability checks use unprivileged `pacman -Qi` where applicable.

5. **AUR helper (paru/yay)**  
   Pacsea may prefix some steps with `doas`; helpers might still call **`sudo` internally** — that is normal and **not** the same as Pacsea’s privilege setting. If the helper hardcodes sudo, fix helper config (e.g. `SUDO`/`PACMAN` wrappers) separately.

---

## Step 6 — Troubleshooting

| Symptom | What to check |
|---------|----------------|
| `doas: not found` or resolution errors | `pacman -S opendoas`, `privilege_tool`, and `$PATH`. |
| `doas: auth failed` / operation denied | `/etc/doas.conf` user, typo, or missing `cmd`/`args` if you used restricted rules. |
| Still see `sudo` in previews | `privilege_tool` not saved, wrong config file path, or `auto` with sudo only on PATH. |
| AUR build fails on sudo | Helper configuration; not always fixed by Pacsea’s `privilege_tool` alone. |
| Stuck or confusing password UX | For doas, expect **terminal** prompting; in-app modal is not used for doas password piping. |

### Quick fallback

```conf
privilege_tool = sudo
```

Reload Pacsea / reload config so settings are picked up.

---

## Step 7 — Optional: run from source

```bash
cd /path/to/pacsea
cargo run -- --dry-run
# then without --dry-run when ready
```

Ensure the process reads the **same** `settings.conf` you edited (copy or symlink if your dev layout uses a different config directory).

---

## Reference — config keys

| Key | Values / notes |
|-----|----------------|
| `privilege_tool` | `auto` \| `sudo` \| `doas` |
| `use_passwordless_sudo` | `true` \| `false` — gates passwordless **and** interacts with non-interactive `-n true` checks |

---

## Safety

- Prefer testing on a **VM** or spare user when tuning `permit` / `nopass`.
- Keep a root shell or live session until `doas.conf` is verified.
- After tests, tighten `doas.conf` (drop `nopass`, add `cmd` restrictions) for daily use.
