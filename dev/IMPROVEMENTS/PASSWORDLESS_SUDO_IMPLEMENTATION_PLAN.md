# Passwordless Sudo Implementation Plan

## Executive Summary

This document outlines a comprehensive plan for implementing secure passwordless sudo support in Pacsea. The feature will allow users to configure Pacsea to use passwordless sudo for package installation and update operations, eliminating the need for repeated password prompts while maintaining security best practices.

**Key Objectives:**
- Provide an opt-in configuration option for passwordless sudo
- Restrict passwordless sudo to specific operations only (install `-S` and update `-Syu`/`-Syyu`)
- Maintain backward compatibility with existing password-based authentication
- Implement robust security checks and warnings
- Support both automatic detection and manual configuration
- Ensure graceful degradation when passwordless sudo is unavailable

---

## Table of Contents

1. [Security Considerations](#security-considerations)
2. [Current Implementation Analysis](#current-implementation-analysis)
3. [Design Overview](#design-overview)
4. [Configuration Design](#configuration-design)
5. [Implementation Details](#implementation-details)
6. [Code Changes Required](#code-changes-required)
7. [Testing Strategy](#testing-strategy)
8. [User Education & Warnings](#user-education--warnings)
9. [Implementation Steps](#implementation-steps)
10. [Risk Assessment](#risk-assessment)
11. [Future Enhancements](#future-enhancements)

---

## Security Considerations

### 1. Understanding Passwordless Sudo

Passwordless sudo allows users to execute privileged commands without entering a password. This is typically configured in `/etc/sudoers` or `/etc/sudoers.d/` using the `NOPASSWD` directive.

**Common Configuration Examples:**

```bash
# Allow specific user to run pacman without password
username ALL=(ALL) NOPASSWD: /usr/bin/pacman

# Allow specific user to run all commands without password (NOT RECOMMENDED)
username ALL=(ALL) NOPASSWD: ALL

# Allow specific user to run pacman and AUR helpers without password
# WARNING: Including yay/paru here is UNSAFE - PKGBUILDs execute with sudo privileges
username ALL=(ALL) NOPASSWD: /usr/bin/pacman, /usr/bin/paru, /usr/bin/yay
```

### 2. Security Risks

**High Risk Scenarios:**
- **Overly Broad Permissions**: `NOPASSWD: ALL` grants unrestricted root access
- **Compromised User Account**: If a user account is compromised, attackers gain root access without password
- **Physical Access**: Anyone with physical access to an unlocked session can execute privileged commands
- **Malicious Packages**: AUR packages with malicious PKGBUILDs could execute arbitrary commands as root

**Mitigation Strategies:**
- Recommend limiting `NOPASSWD` to specific commands only (pacman, paru, yay)
- Restrict passwordless sudo to specific operations (install `-S` and update `-Syu`/`-Syyu` only)
- Require passwords for destructive operations (remove `-R`, cache clean `-Sc`, etc.)
- Provide clear warnings about security implications
- Suggest using `timestamp_timeout=0` to require re-authentication for each sudo session
- Recommend using `requiretty` option for additional security
- Provide documentation on secure sudoers configuration

### 3. Security Best Practices

**Recommended Sudoers Configuration (Based on ArchWiki Guidelines):**
```bash
# Secure configuration example following ArchWiki best practices
# Use absolute paths, specific commands, and Cmnd_Alias for maintainability
# IMPORTANT: Never include yay/paru in NOPASSWD - PKGBUILDs execute with sudo privileges

# Define command aliases for safe operations
Cmnd_Alias PACMAN_SAFE_INSTALL = /usr/bin/pacman -S, /usr/bin/pacman -Syu, /usr/bin/pacman -Syyu
Cmnd_Alias PACMAN_SAFE_QUERY = /usr/bin/pacman -Ss *, /usr/bin/pacman -Qi *, /usr/bin/pacman -Q *, /usr/bin/pacman -F *

# Allow safe operations without password (ONLY pacman, NOT yay/paru)
username ALL=(ALL) NOPASSWD: PACMAN_SAFE_INSTALL, PACMAN_SAFE_QUERY

# Security defaults (ArchWiki recommendations)
Defaults:username timestamp_timeout=0
Defaults:username requiretty
Defaults:username env_reset
Defaults:username secure_path="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
Defaults:username !tty_tickets  # Per-terminal sudo (default, safer)

# Use NOEXEC for query operations to prevent shell escapes
Defaults:username NOEXEC: PACMAN_SAFE_QUERY

# NOTE: yay/paru are intentionally NOT included - PKGBUILDs contain bash code
# that executes during build/install, and allowing them with NOPASSWD would
# let malicious PKGBUILDs execute arbitrary commands as root
```

**ArchWiki Security Requirements:**
- Sudoers file must be owned by `root:root` with mode `0440`
- Always use `visudo` to edit sudoers (validates syntax)
- Use absolute paths only (no wildcards in paths, only in arguments where safe)
- Commands must be in system directories, not user-writable locations
- Use `Cmnd_Alias` for maintainability and clarity

**Key Security Principles:**
1. **Principle of Least Privilege**: Only grant NOPASSWD for specific commands needed
2. **Operation Restriction**: Limit passwordless sudo to safe operations only (install and update from official repos)
3. **AUR Package Restriction**: AUR package installations always require password, even if passwordless sudo is enabled
4. **Explicit Opt-in**: Require users to explicitly enable this feature
5. **Clear Documentation**: Provide comprehensive security warnings
6. **Graceful Degradation**: Fall back to password prompts if passwordless sudo fails
7. **Audit Trail**: Log all privileged operations
8. **Combined Operation Protection**: Prevent exploitation via operation chaining (e.g., search + install)

---

## Current Implementation Analysis

### 1. Password Detection Logic

The codebase already includes passwordless sudo detection in `src/args/update.rs`:

```rust
// Check if passwordless sudo is available
if Command::new("sudo")
    .args(["-n", "true"])
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .status()
    .is_ok_and(|s| s.success())
{
    // Passwordless sudo works, no password needed
    write_log("Passwordless sudo detected, skipping password prompt");
    return None;
}
```

**Current Behavior:**
- Automatically detects passwordless sudo in CLI update mode
- Skips password prompt if passwordless sudo is available
- Falls back to password prompt if passwordless sudo is not available

### 2. Password Handling Locations

**Key Files:**
- `src/args/update.rs`: CLI update password handling
- `src/logic/password.rs`: Password validation utilities
- `src/app/runtime/workers/executor.rs`: Runtime executor password handling
- `src/install/executor.rs`: Install executor password handling
- `src/events/modals/handlers.rs`: TUI password prompt handlers
- `src/ui/modals/password.rs`: Password prompt UI rendering

**Current Flow:**
1. Operation requires sudo
2. Check if passwordless sudo is available (CLI only)
3. If not available, show password prompt modal (TUI) or prompt in terminal (CLI)
4. Validate password
5. Execute command with password piped to sudo

### 3. Gaps in Current Implementation

**Missing Features:**
- No configuration option to enable/disable passwordless sudo usage
- No TUI support for passwordless sudo detection
- No user warnings about security implications
- No validation of sudoers configuration security
- No option to prefer password prompts even when passwordless sudo is available

---

## Design Overview

### 1. Architecture

The implementation will follow a layered approach:

```
┌─────────────────────────────────────────┐
│         User Configuration              │
│  (settings.conf: use_passwordless_sudo)│
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│      Passwordless Sudo Detection        │
│  (check_sudo_passwordless_available())  │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│      Security Validation                │
│  (validate_sudoers_security())          │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│      Password Decision Logic            │
│  (should_use_passwordless_sudo())       │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│      Command Execution                  │
│  (with or without password)             │
└─────────────────────────────────────────┘
```

### 2. Core Principles

1. **Opt-in by Default**: Feature is disabled by default, requiring explicit user configuration
2. **Automatic Detection**: When enabled, automatically detects if passwordless sudo is available
3. **Security Warnings**: Display clear warnings about security implications
4. **Graceful Fallback**: Always fall back to password prompts if passwordless sudo fails
5. **Backward Compatibility**: Existing password-based flow remains unchanged when feature is disabled

### 3. User Experience Flow

**First-Time Setup:**
1. User enables `use_passwordless_sudo = true` in settings.conf
2. On next operation requiring sudo:
   - Check if passwordless sudo is available
   - If available, show one-time security warning modal
   - User acknowledges warning
   - Proceed with passwordless sudo
   - If not available, show helpful error message with sudoers configuration instructions

**Subsequent Operations:**
1. Check configuration: `use_passwordless_sudo = true`
2. Verify passwordless sudo is still available
3. If available, proceed without password prompt
4. If not available, fall back to password prompt with warning

---

## Configuration Design

### 1. Settings Configuration

**New Configuration Keys:**
```conf
# Passwordless sudo configuration
# When true, Pacsea will attempt to use passwordless sudo for package operations
# if it is configured on the system. When false, Pacsea will always prompt for password.
# Default is false (password prompts required).
# 
# SECURITY WARNING: Enabling this feature requires configuring passwordless sudo
# in /etc/sudoers. This grants elevated privileges without password authentication.
# Only enable this if you understand the security implications and have configured
# sudoers appropriately (limiting NOPASSWD to specific commands like pacman/paru/yay).
# 
# See: https://wiki.archlinux.org/title/Sudo#Configuration_examples
use_passwordless_sudo = false

# Passwordless sudo security validation
# When true, Pacsea will validate that sudoers configuration is secure before using
# passwordless sudo. This checks that NOPASSWD is limited to specific commands
# rather than ALL commands. Default is true.
# 
# If validation fails, Pacsea will fall back to password prompts even if
# use_passwordless_sudo is enabled.
validate_sudoers_security = true

# Passwordless sudo allowed operations
# Restricts passwordless sudo to specific pacman operations only.
# This provides an additional security layer by requiring passwords for
# destructive operations (remove, cache clean, etc.).
# 
# Allowed values (comma-separated):
#   - install: Allow passwordless sudo for package installation (pacman -S)
#              SAFE: Installs packages from trusted repositories
#   - update: Allow passwordless sudo for system updates (pacman -Syu, pacman -Syyu)
#             SAFE: Standard system maintenance operation
#   - sync: Allow passwordless sudo for database sync only (pacman -Sy, pacman -Fy)
#           SAFE: Only updates package metadata, does not install or remove anything
#   - search: Allow passwordless sudo for package search (pacman -Ss)
#             SAFE: Read-only operation, only queries package database
#   - query: Allow passwordless sudo for package information queries (pacman -Si, -Qi, -Ql, -Qo, -Qs)
#           SAFE: Read-only operations, only displays information
#   - filequery: Allow passwordless sudo for file database queries (pacman -F)
#                SAFE: Read-only operation, finds which package provides a file
# 
# Default: "install,update" (allows both install and update operations)
# 
# SECURITY NOTE: The following operations ALWAYS require a password, even if
# passwordless sudo is enabled:
#   - Remove operations (-R, -Rs, -Rns, -Rc, -Rdd): Can break system by removing critical packages
#   - Cache clean (-Sc, -Scc): Can cause data loss if cached packages are needed
#   - Install from local file (-U): High risk of malicious packages from unverified sources
#   - Operations with unsafe flags (--force, --nodeps, --overwrite, etc.)
#   - Unknown operations: Default to requiring password for safety
passwordless_sudo_allowed_operations = install,update
```

### 2. Settings Structure

**Add to `src/theme/types.rs`:**
```rust
pub struct Settings {
    // ... existing fields ...
    
    /// Whether to use passwordless sudo when available.
    /// When false, always prompt for password (default).
    /// When true, attempt to use passwordless sudo if configured.
    pub use_passwordless_sudo: bool,
    
    /// Whether to validate sudoers security before using passwordless sudo.
    /// When true, checks that NOPASSWD is limited to specific commands.
    /// When false, uses passwordless sudo without validation (not recommended).
    pub validate_sudoers_security: bool,
    
    /// Whether user has acknowledged passwordless sudo security warning.
    /// When false, shows one-time security warning modal on first use.
    pub passwordless_sudo_warning_acknowledged: bool,
    
    /// Allowed operations for passwordless sudo.
    /// Restricts passwordless sudo to specific pacman operations.
    /// Default: ["install", "update"] (allows install and update only).
    pub passwordless_sudo_allowed_operations: Vec<String>,
}
```

### 3. Default Values

**In `src/theme/types.rs` `Settings::default()`:**
```rust
impl Default for Settings {
    fn default() -> Self {
        Self {
            // ... existing defaults ...
            use_passwordless_sudo: false,
            validate_sudoers_security: true,
            passwordless_sudo_warning_acknowledged: false,
            passwordless_sudo_allowed_operations: vec!["install".to_string(), "update".to_string()],
        }
    }
}
```

### 4. Settings Parsing

**Add to `src/theme/settings/parse_settings.rs`:**
```rust
// Parse use_passwordless_sudo
if key == "use_passwordless_sudo" {
    if let Ok(b) = parse_bool(value) {
        settings.use_passwordless_sudo = b;
    }
    continue;
}

// Parse validate_sudoers_security
if key == "validate_sudoers_security" {
    if let Ok(b) = parse_bool(value) {
        settings.validate_sudoers_security = b;
    }
    continue;
}

// Parse passwordless_sudo_warning_acknowledged
if key == "passwordless_sudo_warning_acknowledged" {
    if let Ok(b) = parse_bool(value) {
        settings.passwordless_sudo_warning_acknowledged = b;
    }
    continue;
}

// Parse passwordless_sudo_allowed_operations
if key == "passwordless_sudo_allowed_operations" {
    // Parse comma-separated list of operations
    let operations: Vec<String> = value
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if !operations.is_empty() {
        settings.passwordless_sudo_allowed_operations = operations;
    }
    continue;
}
```

### 4. Safe and Unsafe Operations Reference

This section provides a comprehensive reference of all pacman/yay/paru operations and flags, categorized by safety level.

#### Safe Operations (Can be allowed with passwordless sudo)

**1. Install Operations (`install`)**
- **Flags**: `-S`, `-S --needed`, `-S --noconfirm` (for official packages only)
- **Why Safe (Official Only)**: Installs packages from trusted official repositories. Packages are verified and signed by Arch Linux maintainers. Risk is limited to package quality, not system integrity.
- **Why Unsafe (AUR)**: AUR packages are user-submitted and unverified. PKGBUILDs contain bash code that executes during build/install - if AUR helpers (yay/paru) run with sudo/NOPASSWD privileges, malicious PKGBUILDs can execute arbitrary commands as root, making this a critical security vulnerability.
- **Security Consideration**: Should distinguish between official repo installs (safer) and AUR installs (higher risk). AUR installs should require password even if passwordless sudo is enabled.
- **Use Cases**: Installing new packages from official repos, reinstalling packages
- **Example**: `pacman -S package-name` (official), `paru -S package-name` (AUR - should require password)

**2. Update Operations (`update`)**
- **Flags**: `-Syu`, `-Syyu`, `-Syu --noconfirm`
- **Why Safe**: Standard system maintenance operation. Updates all installed packages to latest versions. Well-tested operation used by all Arch users.
- **Use Cases**: System updates, keeping packages current
- **Example**: `pacman -Syu`

**3. Sync Operations (`sync`)**
- **Flags**: `-Fy` (file database sync), `-Sy` (repo sync) - **WITH CAUTION**
- **Why Safe (File DB)**: `-Fy` only updates file database metadata. Does NOT install, remove, or modify any packages. Pure read/update operation on metadata.
- **ArchWiki Warning - Partial Upgrades**: `pacman -Sy` without `-u` is explicitly warned against in ArchWiki. It can lead to dependency mismatches and system breakage. Only allow `-Sy` if it's immediately followed by `-u` (i.e., `-Syu`). Standalone `-Sy` should be restricted or require password.
- **Use Cases**: Refreshing file database (`-Fy`), refreshing package database (only as part of `-Syu`)
- **Example**: `pacman -Fy` (safe), `pacman -Syu` (safe - full upgrade), `pacman -Sy` alone (UNSAFE - partial upgrade risk)

**4. Search Operations (`search`)**
- **Flags**: `-Ss`, `-Ss <pattern>`
- **Why Safe**: Read-only operation. Only queries package database for matching package names/descriptions. No system modifications.
- **Use Cases**: Searching for packages by name or description
- **Example**: `pacman -Ss firefox`

**5. Query Operations (`query`)**
- **Flags**: `-Si`, `-Qi`, `-Ql`, `-Qo`, `-Qs`, `-Q`, `-Qe`, `-Qm`, `-Qn`, `-Qt`, `-Qu`, `-Qq`, `-Qqq`, `-Qk`, `-Qkk`
- **Why Safe**: All read-only operations that display information about packages. No modifications to system.
  - `-Si`: Show info about package in sync database
  - `-Qi`: Show info about installed package
  - `-Ql`: List files belonging to installed package
  - `-Qo`: Query which package owns a file
  - `-Qs`: Search installed packages
  - `-Q`: List installed packages
  - `-Qe`: List explicitly installed packages
  - `-Qm`: List foreign packages (AUR)
  - `-Qn`: List native packages (official repos)
  - `-Qt`: List packages not required by any other package
  - `-Qu`: List packages with updates available (read-only check)
  - `-Qq`: Quiet mode - list package names only
  - `-Qqq`: Very quiet mode - minimal output
  - `-Qk`: Check package file integrity (read-only verification)
  - `-Qkk`: Check package file integrity with detailed output
- **Use Cases**: Getting package information, listing installed packages, checking package status, verifying package integrity
- **Example**: `pacman -Qi package-name`, `pacman -Ql package-name`, `pacman -Qu` (check updates), `pacman -Qk` (verify integrity)

**6. File Query Operations (`filequery`)**
- **Flags**: `-F`, `-Fy` (sync), `-Fl`, `-Fo`, `-Fq`
- **Why Safe**: Read-only operations that query the file database. Finds which package provides a file.
  - `-F`: Find which package provides a file
  - `-Fy`: Sync file database (safe, only updates metadata)
  - `-Fl`: List files provided by package
  - `-Fo`: Query file owner
  - `-Fq`: Quiet mode (no output formatting)
- **Use Cases**: Finding which package provides a file, syncing file database
- **Example**: `pacman -F /usr/bin/firefox`

#### Unsafe Operations (ALWAYS require password, cannot be allowed)

**1. Remove Operations (`remove`)**
- **Flags**: `-R`, `-Rs`, `-Rns`, `-Rc`, `-Rdd`, `-Ru`, `-Rsc`, `-Rscn`
- **Why Unsafe**: Removes packages and potentially dependencies. Can break system by removing critical packages (kernel, glibc, systemd, etc.). High risk of system breakage.
- **Risk Level**: CRITICAL
- **Examples**:
  - `pacman -R package-name`: Removes package only
  - `pacman -Rs package-name`: Removes package and unused dependencies
  - `pacman -Rns package-name`: Removes package, dependencies, and config files
  - `pacman -Rdd package-name`: Removes package ignoring dependencies (very dangerous)
- **Mitigation**: Always require password, even with passwordless sudo enabled

**2. Cache Clean Operations (`cacheclean`)**
- **Flags**: `-Sc`, `-Scc`
- **Why Unsafe**: Deletes cached packages from `/var/cache/pacman/pkg/`. Can cause data loss if packages are needed later (e.g., for downgrades, reinstalls). `-Scc` removes all caches including unused sync databases.
- **Risk Level**: HIGH
- **Examples**:
  - `pacman -Sc`: Remove unused cached packages
  - `pacman -Scc`: Remove all cached packages and unused sync databases
- **Mitigation**: Always require password, even with passwordless sudo enabled

**3. Install from Local File (`installlocal`)**
- **Flags**: `-U`, `-U <file.pkg.tar.xz>`
- **Why Unsafe**: Installs packages from unverified local files. High risk of malicious packages. No repository verification. Can install packages with malicious code.
- **Risk Level**: CRITICAL
- **Example**: `pacman -U malicious-package.pkg.tar.xz`
- **Mitigation**: Always require password, even with passwordless sudo enabled

**7. Update Check Operations (`updatecheck`)**
- **Flags**: `-Qu`, `-Qua` (for AUR helpers, but only query, not install)
- **Why Safe**: Read-only operation that checks for available updates without installing anything. Does NOT modify system state. Only queries package databases.
- **Use Cases**: Checking for available updates, monitoring package status
- **Example**: `pacman -Qu` (check official repo updates)
- **Note**: AUR helper update checks (`yay -Qua`, `paru -Qua`) are safe as read-only queries, but actual AUR installs must require password

**8. Database Operations (`database`)**
- **Flags**: `--database`, `--asdeps`, `--asexplicit` (when used with query operations only)
- **Why Safe (Query Context)**: Database query operations are read-only. However, `--asdeps`/`--asexplicit` when used with install operations can change dependency status, so they should be restricted.
- **Use Cases**: Querying database state, checking dependency relationships
- **Example**: `pacman --database --check` (verify database integrity)
- **Note**: `--asdeps`/`--asexplicit` flags should only be allowed in query context, not with install operations

#### Additional Safe Operations (Lower Priority)

**9. List Operations (`list`)**
- **Flags**: `-Qq`, `-Qqq`, `-Sl`, `-Ql`
- **Why Safe**: Read-only list operations. Output package names or file lists without modifying system.
- **Use Cases**: Scripting, automation, getting package lists
- **Example**: `pacman -Qq` (list all installed packages, quiet mode)

**10. Info Operations (`info`)**
- **Flags**: `-Si`, `-Qi`, `-Ss` (already covered in search/query)
- **Why Safe**: Read-only information display. No system modifications.
- **Use Cases**: Getting detailed package information
- **Example**: `pacman -Si package-name`

#### Unsafe Operations (ALWAYS require password, cannot be allowed)

**1. Remove Operations (`remove`)**

These flags bypass safety checks and should NEVER be allowed with passwordless sudo, regardless of operation:

- **`--force`**: Forces overwriting of files and ignoring certain dependency checks. Very dangerous.
- **`--nodeps`**: Skip dependency checks. Can result in broken system with missing dependencies.
- **`--overwrite <pattern>`**: Write over conflicting files. Can overwrite system files.
- **`--disable-sandbox`**: Disables security sandbox (pacman v7+). Bypasses important security protections.
- **`--allow-downgrade`**: Allows downgrading packages. Can break system if critical packages are downgraded.
- **`--assume-installed <package>`**: Assumes package is installed. Can cause dependency resolution issues.
- **`--ignore <package>`**: Ignores package upgrades. Can lead to security vulnerabilities.
- **`--skip-checks`**: Skips various safety checks. Dangerous.
- **`--skip-validation`**: Skips package validation. Very dangerous.

**5. Unknown Operations (`other`)**
- **Why Unsafe**: Operations that cannot be categorized are treated as unsafe by default. This follows the principle of "fail secure" - if we don't know what it does, require authentication.
- **Risk Level**: UNKNOWN (treat as HIGH)
- **Mitigation**: Always require password for unknown operations

#### Complete Unsafe Operations List

**Summary of operations that ALWAYS require password:**

1. **Remove Operations**:
   - `-R`, `-Rs`, `-Rns`, `-Rc`, `-Rdd`, `-Ru`, `-Rsc`, `-Rscn`
   - Any operation that removes packages

2. **Cache Operations**:
   - `-Sc`, `-Scc`
   - Any operation that deletes cached packages

3. **Local File Install**:
   - `-U <file>`
   - Any operation that installs from local files

4. **AUR Package Installations**:
   - `paru -S <package>`, `yay -S <package>`
   - Any installation via AUR helpers (paru/yay)
   - **Why**: AUR packages are user-submitted and unverified. PKGBUILDs contain bash code that executes during build/install - if AUR helpers run with sudo/NOPASSWD privileges, malicious PKGBUILDs can execute arbitrary commands as root, making this a critical security vulnerability.

5. **Operations with Unsafe Flags**:
   - Any operation combined with: `--force`, `--nodeps`, `--overwrite`, `--disable-sandbox`, `--allow-downgrade`, `--assume-installed`, `--ignore`, `--skip-checks`, `--skip-validation`

6. **Unknown Operations**:
   - Any operation that cannot be categorized

7. **Combined Operations (Security Risk)**:
   - Sequences of operations that could be exploited (e.g., search + install of AUR packages)
   - **Mitigation**: AUR installs always require password, regardless of passwordless sudo configuration

#### Configuration Examples

**Minimal Safe Configuration (Recommended Default):**
```conf
passwordless_sudo_allowed_operations = install,update
```
Allows only package installation and system updates. Most common use case.

**Extended Safe Configuration:**
```conf
passwordless_sudo_allowed_operations = install,update,sync,search,query,filequery,download,verify,updatecheck
```
Allows all safe read-only and metadata operations, plus download and verification. Useful for automation scripts that need to query package information, pre-download packages, or verify system integrity.

**Read-Only Only Configuration:**
```conf
passwordless_sudo_allowed_operations = sync,search,query,filequery,verify,updatecheck
```
Allows only read-only operations. No package installation or updates. Maximum safety for query-only use cases. Includes verification and update checking for system monitoring.

**Download-Only Configuration:**
```conf
passwordless_sudo_allowed_operations = download,query,search
```
Allows downloading packages and querying information, but requires password for installation. Useful for pre-downloading packages during off-peak hours.

**Install Only Configuration:**
```conf
passwordless_sudo_allowed_operations = install
```
Allows only package installation. Updates require password. Useful if you want to control when system updates happen.

---

## Implementation Details

### 1. Passwordless Sudo Detection

**New Function in `src/logic/password.rs`:**

```rust
/// What: Check if passwordless sudo is available for the current user.
///
/// Inputs:
/// - None (checks system configuration).
///
/// Output:
/// - `Ok(true)` if passwordless sudo is available, `Ok(false)` if not, or `Err(String)` on error.
///
/// Details:
/// - Uses `sudo -n true` to test if sudo can run without password.
/// - Returns `Ok(false)` if sudo is not available or requires password.
/// - Returns `Err` if the check cannot be executed (e.g., sudo not installed).
pub fn check_sudo_passwordless_available() -> Result<bool, String> {
    use std::process::Command;
    
    let output = Command::new("sudo")
        .args(["-n", "true"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("Failed to check passwordless sudo: {e}"))?;
    
    Ok(output.success())
}
```

### 2. Sudoers Security Validation

**New Function in `src/logic/password.rs`:**

```rust
/// What: Validate that sudoers configuration is secure for passwordless sudo.
///
/// Inputs:
/// - None (reads sudoers configuration).
///
/// Output:
/// - `Ok(true)` if configuration is secure, `Ok(false)` if insecure, or `Err(String)` on error.
///
/// Details:
/// - Checks that NOPASSWD is limited to specific commands with absolute paths.
/// - Validates sudoers file permissions (must be 0440, root:root).
/// - Warns if NOPASSWD: ALL is found for the current user.
/// - Checks for use of Cmnd_Alias (good practice per ArchWiki).
/// - Verifies commands use absolute paths (not relative or wildcard paths).
/// - Returns `Ok(false)` if insecure configuration is detected.
/// - Returns `Err` if sudoers cannot be read or parsed.
pub fn validate_sudoers_security() -> Result<bool, String> {
    use std::process::Command;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    
    // Get current username
    let username = std::env::var("USER")
        .map_err(|_| "Cannot determine current username".to_string())?;
    
    // Check sudoers file permissions (ArchWiki requirement: 0440, root:root)
    let sudoers_path = "/etc/sudoers";
    if let Ok(metadata) = fs::metadata(sudoers_path) {
        let perms = metadata.permissions();
        let mode = perms.mode() & 0o7777;
        if mode != 0o440 {
            tracing::warn!(
                path = sudoers_path,
                mode = format!("{:o}", mode),
                "Sudoers file has incorrect permissions (should be 0440 per ArchWiki)"
            );
            return Ok(false);
        }
    }
    
    // Read sudoers configuration
    // Use visudo -c to validate and cat to read (requires sudo)
    let output = Command::new("sudo")
        .args(["-n", "cat", "/etc/sudoers", "/etc/sudoers.d/*"])
        .output()
        .map_err(|e| format!("Failed to read sudoers: {e}"))?;
    
    if !output.status.success() {
        // If we can't read sudoers, assume insecure (fail safe)
        return Ok(false);
    }
    
    let sudoers_content = String::from_utf8_lossy(&output.stdout);
    
    // Check for dangerous patterns (NOPASSWD: ALL)
    let dangerous_patterns = [
        format!("{username} ALL=(ALL) NOPASSWD: ALL"),
        format!("{username} ALL=(ALL) NOPASSWD:ALL"),
        format!("%{username} ALL=(ALL) NOPASSWD: ALL"),
        format!("%{username} ALL=(ALL) NOPASSWD:ALL"),
    ];
    
    for pattern in &dangerous_patterns {
        if sudoers_content.contains(pattern) {
            tracing::warn!(
                username = %username,
                "Insecure sudoers configuration detected: NOPASSWD: ALL"
            );
            return Ok(false);
        }
    }
    
    // Check for secure patterns (specific commands with absolute paths)
    // ArchWiki requires absolute paths, not relative or wildcard paths
    let secure_commands = ["/usr/bin/pacman", "/usr/bin/paru", "/usr/bin/yay"];
    let mut has_secure_config = false;
    let mut has_absolute_paths = false;
    
    for cmd in &secure_commands {
        // Check for absolute path usage (ArchWiki requirement)
        if sudoers_content.contains(cmd) {
            has_absolute_paths = true;
        }
        
        // Check for NOPASSWD entries with this command
        let secure_patterns = [
            format!("NOPASSWD: {cmd}"),
            format!("NOPASSWD:{cmd}"),
            format!("NOPASSWD: {cmd} "),
        ];
        
        for pattern in &secure_patterns {
            if sudoers_content.contains(pattern) {
                has_secure_config = true;
                break;
            }
        }
    }
    
    // Check for Cmnd_Alias usage (ArchWiki best practice)
    let has_cmnd_alias = sudoers_content.contains("Cmnd_Alias");
    
    // Check for relative paths or wildcard paths (unsafe per ArchWiki)
    let unsafe_path_patterns = [
        "pacman",  // Relative path (should be /usr/bin/pacman)
        "paru",    // Relative path
        "yay",     // Relative path
        "*/pacman", // Wildcard path
        "~/pacman", // User home path
    ];
    
    for pattern in &unsafe_path_patterns {
        // Only flag if it's in a NOPASSWD context
        if sudoers_content.contains(&format!("NOPASSWD: {}", pattern)) {
            tracing::warn!(
                pattern = pattern,
                "Unsafe path pattern in sudoers (should use absolute paths per ArchWiki)"
            );
            return Ok(false);
        }
    }
    
    if !has_secure_config {
        tracing::warn!(
            username = %username,
            "No secure passwordless sudo configuration found for package managers"
        );
        return Ok(false);
    }
    
    if !has_absolute_paths {
        tracing::warn!(
            "Sudoers configuration does not use absolute paths (ArchWiki requirement)"
        );
        return Ok(false);
    }
    
    Ok(true)
}
```

**ArchWiki Validation Requirements:**
- Sudoers file permissions must be `0440` (root:root)
- Commands must use absolute paths (e.g., `/usr/bin/pacman`, not `pacman`)
- No wildcard paths in command specifications
- Commands must be in system directories, not user-writable locations
- Use `visudo` to validate syntax (should be checked before applying)

**Note:** This validation is complex and may need refinement. Consider:
- Parsing sudoers more carefully (handling comments, line continuations, Cmnd_Alias)
- Supporting group-based configurations
- Handling include directives
- Validating against `visudo -c` output
- Providing more detailed error messages

### 3. Operation Detection and Validation

**New Enum in `src/logic/password.rs`:**

```rust
/// What: Type of pacman operation being performed.
///
/// Details:
/// - Used to determine if passwordless sudo is allowed for a specific operation.
/// - Operations are categorized by safety level and potential for system modification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacmanOperation {
    /// Package installation (pacman -S)
    /// SAFE: Installs packages from trusted repositories. Risk is limited to package quality.
    Install,
    /// System update (pacman -Syu, pacman -Syyu)
    /// SAFE: Updates system packages. Standard maintenance operation.
    Update,
    /// Database sync only (pacman -Sy, pacman -Fy)
    /// SAFE: Only downloads/updates package metadata. Does not install or remove anything.
    Sync,
    /// Package search (pacman -Ss)
    /// SAFE: Read-only operation. Only queries package database.
    Search,
    /// Package information query (pacman -Si, pacman -Qi, pacman -Ql, pacman -Qo)
    /// SAFE: Read-only operations. Only displays information about packages.
    Query,
    /// File database query (pacman -F)
    /// SAFE: Read-only operation. Finds which package provides a file.
    FileQuery,
    /// Download-only operation (pacman -Sw)
    /// SAFE: Downloads packages to cache without installing. No system modifications.
    Download,
    /// Verification operation (pacman --check, -Qk)
    /// SAFE: Read-only integrity checks. No system modifications.
    Verify,
    /// Update check operation (pacman -Qu)
    /// SAFE: Read-only operation. Checks for updates without installing.
    UpdateCheck,
    /// Package removal (pacman -R, pacman -Rs, pacman -Rns, pacman -Rc, pacman -Rdd)
    /// UNSAFE: Removes packages and potentially dependencies. Can break system.
    Remove,
    /// Cache clean (pacman -Sc, pacman -Scc)
    /// UNSAFE: Deletes cached packages. Can cause data loss if packages are needed.
    CacheClean,
    /// Install from local file (pacman -U)
    /// UNSAFE: Installs from unverified local files. High risk of malicious packages.
    InstallLocal,
    /// Other operations (not explicitly categorized)
    /// UNSAFE: Unknown operations default to requiring password for safety.
    Other,
}

/// What: Detect the type of pacman operation from command arguments.
///
/// Inputs:
/// - `args`: Command arguments (e.g., ["pacman", "-Syu", "--noconfirm"]).
///
/// Output:
/// - `PacmanOperation` variant matching the operation type.
///
/// Details:
/// - Checks operations in order of specificity (most specific first).
/// - Detects unsafe flags that should always require password.
/// - Returns `Other` if operation type cannot be determined (defaults to unsafe).
pub fn detect_pacman_operation(args: &[String]) -> PacmanOperation {
    // Convert args to a single string for easier pattern matching
    let args_str = args.join(" ");
    
    // Check for unsafe flags first - these always require password regardless of operation
    let unsafe_flags = [
        "--force", "--nodeps", "--overwrite", "--disable-sandbox",
        "--allow-downgrade", "--assume-installed", "--ignore",
        "--skip-checks", "--skip-validation",
    ];
    for flag in &unsafe_flags {
        if args_str.contains(flag) {
            // Unsafe flag detected - always require password
            return PacmanOperation::Other;
        }
    }
    
    // Check for install from local file (-U) - UNSAFE
    if args_str.contains(" -U ") || args_str.starts_with("-U ") {
        return PacmanOperation::InstallLocal;
    }
    
    // Check for update operations first (they also contain -S)
    if args_str.contains("-Syyu") || args_str.contains("-Syu") {
        return PacmanOperation::Update;
    }
    
    // Check for remove operations - UNSAFE
    if args_str.contains("-Rns") || args_str.contains("-Rs") || 
       args_str.contains("-Rc") || args_str.contains("-Rdd") ||
       args_str.contains(" -R ") {
        return PacmanOperation::Remove;
    }
    
    // Check for cache clean - UNSAFE
    if args_str.contains("-Scc") || args_str.contains("-Sc") {
        return PacmanOperation::CacheClean;
    }
    
    // Check for sync operations (database refresh only, no install)
    if args_str.contains("-Fy") || (args_str.contains("-Sy") && !args_str.contains("-Syu") && !args_str.contains("-Syyu")) {
        return PacmanOperation::Sync;
    }
    
    // Check for verification operations (read-only integrity checks)
    if args_str.contains("--check") || args_str.contains("--verify") ||
       args_str.contains(" -Qk ") || args_str.contains(" -Qkk ") {
        return PacmanOperation::Verify;
    }
    
    // Check for update check operations (read-only, no install)
    if args_str.contains(" -Qu ") || args_str.starts_with("-Qu ") {
        return PacmanOperation::UpdateCheck;
    }
    
    // Check for download-only operations
    if args_str.contains(" -Sw ") || args_str.starts_with("-Sw ") {
        return PacmanOperation::Download;
    }
    
    // Check for file query operations
    if args_str.contains(" -F ") || args_str.starts_with("-F ") {
        return PacmanOperation::FileQuery;
    }
    
    // Check for search operations
    if args_str.contains(" -Ss ") || args_str.starts_with("-Ss ") {
        return PacmanOperation::Search;
    }
    
    // Check for query operations (read-only information)
    if args_str.contains(" -Si ") || args_str.contains(" -Qi ") ||
       args_str.contains(" -Ql ") || args_str.contains(" -Qo ") ||
       args_str.contains(" -Qs ") || args_str.starts_with("-Q") {
        return PacmanOperation::Query;
    }
    
    // Check for install operation (must contain -S but not update/sync flags)
    if args_str.contains(" -S ") || args_str.starts_with("-S ") {
        return PacmanOperation::Install;
    }
    
    // Unknown operation - default to unsafe
    PacmanOperation::Other
}

/// What: Check if an operation is allowed for passwordless sudo.
///
/// Inputs:
/// - `operation`: Type of pacman operation.
/// - `allowed_operations`: List of allowed operation names (e.g., ["install", "update", "sync"]).
///
/// Output:
/// - `true` if operation is allowed, `false` otherwise.
///
/// Details:
/// - Maps `PacmanOperation` enum to string names.
/// - Checks if operation name is in the allowed list.
/// - Destructive operations (Remove, CacheClean, InstallLocal) are never allowed.
/// - Unknown operations (Other) are never allowed for safety.
pub fn is_operation_allowed(
    operation: PacmanOperation,
    allowed_operations: &[String],
) -> bool {
    // Destructive operations always require password (never allow passwordless sudo)
    if matches!(
        operation,
        PacmanOperation::Remove
            | PacmanOperation::CacheClean
            | PacmanOperation::InstallLocal
            | PacmanOperation::Other
    ) {
        return false;
    }
    
    // Map operation to string name
    let operation_name = match operation {
        PacmanOperation::Install => "install",
        PacmanOperation::Update => "update",
        PacmanOperation::Sync => "sync",
        PacmanOperation::Search => "search",
        PacmanOperation::Query => "query",
        PacmanOperation::FileQuery => "filequery",
        PacmanOperation::Download => "download",
        PacmanOperation::Verify => "verify",
        PacmanOperation::UpdateCheck => "updatecheck",
        // Destructive operations (already handled above)
        PacmanOperation::Remove => return false,
        PacmanOperation::CacheClean => return false,
        PacmanOperation::InstallLocal => return false,
        PacmanOperation::Other => return false,
    };
    
    // Check if operation is in allowed list
    allowed_operations
        .iter()
        .any(|op| op.eq_ignore_ascii_case(operation_name))
}
```

### 4. Password Decision Logic

**Updated Function in `src/logic/password.rs`:**

```rust
/// What: Determine if passwordless sudo should be used for an operation.
///
/// Inputs:
/// - `settings`: Application settings containing passwordless sudo configuration.
/// - `operation`: Type of pacman operation being performed.
///
/// Output:
/// - `Ok(Some(String))` if password is required, `Ok(None)` if passwordless sudo should be used,
///   or `Err(String)` on error.
///
/// Details:
/// - Checks user configuration first (use_passwordless_sudo).
/// - Checks if operation is allowed for passwordless sudo.
/// - If enabled, checks if passwordless sudo is available.
/// - If security validation is enabled, validates sudoers configuration.
/// - Returns `Ok(None)` if passwordless sudo should be used.
/// - Returns `Ok(Some(String))` if password is required (for prompting).
pub fn should_use_passwordless_sudo(
    settings: &crate::theme::types::Settings,
    operation: PacmanOperation,
) -> Result<Option<String>, String> {
    // If feature is disabled, always require password
    if !settings.use_passwordless_sudo {
        return Ok(Some("Password required (passwordless sudo disabled)".to_string()));
    }
    
    // Check if operation is allowed for passwordless sudo
    if !is_operation_allowed(operation, &settings.passwordless_sudo_allowed_operations) {
        return Ok(Some(
            format!("Password required (operation {:?} not allowed for passwordless sudo)", operation)
        ));
    }
    
    // Check if passwordless sudo is available
    let is_available = check_sudo_passwordless_available()
        .map_err(|e| format!("Failed to check passwordless sudo availability: {e}"))?;
    
    if !is_available {
        return Ok(Some(
            "Password required (passwordless sudo not configured)".to_string()
        ));
    }
    
    // If security validation is enabled, validate sudoers configuration
    if settings.validate_sudoers_security {
        let is_secure = validate_sudoers_security()
            .map_err(|e| format!("Failed to validate sudoers security: {e}"))?;
        
        if !is_secure {
            tracing::warn!("Passwordless sudo available but configuration is insecure, falling back to password prompt");
            return Ok(Some(
                "Password required (insecure sudoers configuration)".to_string()
            ));
        }
    }
    
    // Passwordless sudo is available and secure
    Ok(None)
}
```

### 5. Integration Points

**Update `src/args/update.rs` `prompt_and_validate_password()`:**

```rust
#[cfg(not(target_os = "windows"))]
fn prompt_and_validate_password(write_log: &(dyn Fn(&str) + Send + Sync)) -> Option<String> {
    use crate::logic::password::{should_use_passwordless_sudo, PacmanOperation};
    use crate::theme::settings;
    
    let app_settings = settings::settings();
    
    // Update operation uses -Syu or -Syyu
    let operation = PacmanOperation::Update;
    
    // Check if passwordless sudo should be used
    match should_use_passwordless_sudo(&app_settings, operation) {
        Ok(None) => {
            // Passwordless sudo should be used
            write_log("Using passwordless sudo (configured and available)");
            return None;
        }
        Ok(Some(reason)) => {
            // Password is required
            write_log(&format!("Password required: {}", reason));
            // Continue to password prompt
        }
        Err(e) => {
            write_log(&format!("Error checking passwordless sudo: {}, falling back to password prompt", e));
            // Continue to password prompt
        }
    }
    
    // ... existing password prompt logic ...
}
```

**Update `src/app/runtime/workers/executor.rs` `handle_install_request()`:**

```rust
#[cfg(not(target_os = "windows"))]
fn handle_install_request(
    items: Vec<crate::state::PackageItem>,
    password: Option<String>,
    dry_run: bool,
    res_tx: mpsc::UnboundedSender<ExecutorOutput>,
) {
    use crate::logic::password::{should_use_passwordless_sudo, PacmanOperation};
    use crate::theme::settings;
    
    let app_settings = settings::settings();
    
    // Install operation uses -S
    let operation = PacmanOperation::Install;
    
    // Determine if password is needed
    let effective_password = match should_use_passwordless_sudo(&app_settings, operation) {
        Ok(None) => {
            // Passwordless sudo available, no password needed
            tracing::info!("Using passwordless sudo for install operation");
            None
        }
        Ok(Some(_)) => {
            // Password required, use provided password
            password
        }
        Err(e) => {
            tracing::warn!("Error checking passwordless sudo: {}, using provided password", e);
            password
        }
    };
    
    // ... rest of function using effective_password instead of password ...
}
```

**Update `src/install/executor.rs` for remove operations:**

```rust
// In build_remove_command_for_executor or similar function
use crate::logic::password::{should_use_passwordless_sudo, PacmanOperation, detect_pacman_operation};

// Detect operation type from command arguments
let operation = detect_pacman_operation(&args);
// For remove operations, this will be PacmanOperation::Remove

// Check if passwordless sudo should be used
match should_use_passwordless_sudo(&settings, operation) {
    Ok(None) => {
        // Passwordless sudo available (shouldn't happen for remove, but handle gracefully)
        tracing::info!("Using passwordless sudo for operation");
        // Build command without password
    }
    Ok(Some(_)) => {
        // Password required (expected for remove operations)
        // Build command with password
    }
    Err(e) => {
        // Error checking, require password for safety
        // Build command with password
    }
}
```

**Similar updates needed for:**
- `src/install/executor.rs`: Install executor password handling
- `src/events/modals/handlers.rs`: TUI password prompt handlers
- Any other locations that handle sudo password

### 5. Security Warning Modal

**New Modal Type in `src/state/modal.rs`:**

```rust
pub enum Modal {
    // ... existing variants ...
    
    /// Security warning for passwordless sudo.
    PasswordlessSudoWarning {
        /// Callback to execute after user acknowledges warning.
        on_acknowledge: Box<dyn FnOnce(&mut AppState) + Send>,
    },
}
```

**New UI Component in `src/ui/modals/passwordless_sudo_warning.rs`:**

```rust
/// What: Render security warning modal for passwordless sudo.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state
/// - `area`: Full screen area
///
/// Output:
/// - Draws the security warning dialog.
///
/// Details:
/// - Shows comprehensive security warning about passwordless sudo.
/// - Provides options to acknowledge and continue, or cancel.
pub fn render_passwordless_sudo_warning(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
) {
    // Implementation similar to other modals
    // Shows warning text, security implications, and acknowledge/cancel buttons
}
```

**Handler in `src/events/modals/passwordless_sudo_warning.rs`:**

```rust
/// What: Handle passwordless sudo warning modal events.
///
/// Inputs:
/// - `app`: Application state
/// - `key`: Key pressed
///
/// Output:
/// - Handles acknowledge/cancel actions.
///
/// Details:
/// - On acknowledge: marks warning as acknowledged in settings, executes callback.
/// - On cancel: closes modal without executing callback.
pub fn handle_passwordless_sudo_warning(
    app: &mut AppState,
    key: KeyCode,
) -> bool {
    // Handle key events
    // On acknowledge: save settings, execute callback
    // On cancel: close modal
}
```

**Integration in password decision logic:**

```rust
// Before using passwordless sudo for the first time
if !settings.passwordless_sudo_warning_acknowledged {
    // Show warning modal
    app.modal = Modal::PasswordlessSudoWarning {
        on_acknowledge: Box::new(|app| {
            // Mark warning as acknowledged
            // Save settings
            // Continue with operation
        }),
    };
    return; // Wait for user acknowledgment
}
```

---

## Code Changes Required

### 1. New Files

- `src/logic/passwordless_sudo.rs`: Core passwordless sudo logic (or extend `src/logic/password.rs`)
- `src/ui/modals/passwordless_sudo_warning.rs`: Warning modal UI
- `src/events/modals/passwordless_sudo_warning.rs`: Warning modal event handlers

### 2. Modified Files

**Settings:**
- `src/theme/types.rs`: Add new settings fields
- `src/theme/settings/parse_settings.rs`: Parse new settings
- `src/theme/config/settings_save.rs`: Save new settings
- `config/settings.conf`: Add configuration examples

**Password Logic:**
- `src/logic/password.rs`: Add passwordless sudo detection and validation functions
- `src/args/update.rs`: Integrate passwordless sudo check
- `src/app/runtime/workers/executor.rs`: Integrate passwordless sudo check
- `src/install/executor.rs`: Integrate passwordless sudo check

**UI/Events:**
- `src/state/modal.rs`: Add warning modal variant
- `src/ui/modals/mod.rs`: Add warning modal module
- `src/ui/modals/renderer.rs`: Add warning modal rendering
- `src/events/modals/handlers.rs`: Add warning modal handlers

**Translations:**
- `config/locales/en-US.yml`: Add translation keys
- `config/locales/de-DE.yml`: Add translation keys
- `config/locales/hu-HU.yml`: Add translation keys (with TODO comments)

### 3. Function Signatures

**New Functions:**
```rust
// In src/logic/password.rs
pub fn check_sudo_passwordless_available() -> Result<bool, String>
pub fn validate_sudoers_security() -> Result<bool, String>
pub fn detect_pacman_operation(args: &[String]) -> PacmanOperation
pub fn is_operation_allowed(operation: PacmanOperation, allowed_operations: &[String]) -> bool
pub fn should_use_passwordless_sudo(settings: &Settings, operation: PacmanOperation) -> Result<Option<String>, String>

// In src/events/modals/passwordless_sudo_warning.rs
pub fn handle_passwordless_sudo_warning(app: &mut AppState, key: KeyCode) -> bool
```

**New Types:**
```rust
// In src/logic/password.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacmanOperation {
    Install,      // Package installation (pacman -S)
    Update,       // System update (pacman -Syu, pacman -Syyu)
    Sync,         // Database sync only (pacman -Sy, pacman -Fy)
    Search,       // Package search (pacman -Ss)
    Query,        // Package information query (pacman -Si, -Qi, -Ql, etc.)
    FileQuery,    // File database query (pacman -F)
    Download,     // Download-only operation (pacman -Sw)
    Verify,       // Verification operation (pacman --check, -Qk)
    UpdateCheck,  // Update check operation (pacman -Qu)
    Remove,       // Package removal (ALWAYS requires password)
    CacheClean,   // Cache clean (ALWAYS requires password)
    InstallLocal, // Install from local file (ALWAYS requires password)
    Other,        // Unknown operations (ALWAYS requires password)
}
```

**Modified Functions:**
```rust
// In src/args/update.rs
fn prompt_and_validate_password(...) -> Option<String>  // Add passwordless sudo check

// In src/app/runtime/workers/executor.rs
fn handle_install_request(..., password: Option<String>, ...)  // Add passwordless sudo check

// In src/install/executor.rs
fn build_install_command_for_executor(..., password: Option<&str>, ...)  // Add passwordless sudo check
```

---

## Testing Strategy

### 1. Unit Tests

**Test Passwordless Sudo Detection:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_check_sudo_passwordless_available() {
        // Test when passwordless sudo is available
        // Test when passwordless sudo is not available
        // Test when sudo is not installed
    }
    
    #[test]
    fn test_validate_sudoers_security() {
        // Test with secure configuration (specific commands)
        // Test with insecure configuration (NOPASSWD: ALL)
        // Test when sudoers cannot be read
    }
    
    #[test]
    fn test_detect_pacman_operation() {
        // Test detection of install operation (-S)
        assert_eq!(
            detect_pacman_operation(&["pacman", "-S", "package".to_string()]),
            PacmanOperation::Install
        );
        // Test detection of update operation (-Syu, -Syyu)
        assert_eq!(
            detect_pacman_operation(&["pacman", "-Syu", "--noconfirm".to_string()]),
            PacmanOperation::Update
        );
        // Test detection of remove operation (-R, -Rs, -Rns)
        assert_eq!(
            detect_pacman_operation(&["pacman", "-Rns", "package".to_string()]),
            PacmanOperation::Remove
        );
        // Test detection of cache clean operation (-Sc)
        assert_eq!(
            detect_pacman_operation(&["pacman", "-Sc", "--noconfirm".to_string()]),
            PacmanOperation::CacheClean
        );
        // Test detection of file query operation (-F)
        assert_eq!(
            detect_pacman_operation(&["pacman", "-F", "/usr/bin/firefox".to_string()]),
            PacmanOperation::FileQuery
        );
        // Test detection of download-only operation (-Sw)
        assert_eq!(
            detect_pacman_operation(&["pacman", "-Sw", "package".to_string()]),
            PacmanOperation::Download
        );
        // Test detection of verification operation (--check, -Qk)
        assert_eq!(
            detect_pacman_operation(&["pacman", "--check".to_string()]),
            PacmanOperation::Verify
        );
        assert_eq!(
            detect_pacman_operation(&["pacman", "-Qk".to_string()]),
            PacmanOperation::Verify
        );
        // Test detection of update check operation (-Qu)
        assert_eq!(
            detect_pacman_operation(&["pacman", "-Qu".to_string()]),
            PacmanOperation::UpdateCheck
        );
    }
    
    #[test]
    fn test_is_operation_allowed() {
        let allowed = vec!["install".to_string(), "update".to_string()];
        
        // Test allowed operations
        assert!(is_operation_allowed(PacmanOperation::Install, &allowed));
        assert!(is_operation_allowed(PacmanOperation::Update, &allowed));
        
        // Test disallowed operations (remove, cache clean, install local always require password)
        assert!(!is_operation_allowed(PacmanOperation::Remove, &allowed));
        assert!(!is_operation_allowed(PacmanOperation::CacheClean, &allowed));
        assert!(!is_operation_allowed(PacmanOperation::InstallLocal, &allowed));
        assert!(!is_operation_allowed(PacmanOperation::Other, &allowed));
        
        // Test with extended allowed operations
        let extended = vec![
            "install".to_string(),
            "update".to_string(),
            "sync".to_string(),
            "search".to_string(),
            "query".to_string(),
            "filequery".to_string(),
            "download".to_string(),
            "verify".to_string(),
            "updatecheck".to_string(),
        ];
        assert!(is_operation_allowed(PacmanOperation::Sync, &extended));
        assert!(is_operation_allowed(PacmanOperation::Search, &extended));
        assert!(is_operation_allowed(PacmanOperation::Query, &extended));
        assert!(is_operation_allowed(PacmanOperation::FileQuery, &extended));
        assert!(is_operation_allowed(PacmanOperation::Download, &extended));
        assert!(is_operation_allowed(PacmanOperation::Verify, &extended));
        assert!(is_operation_allowed(PacmanOperation::UpdateCheck, &extended));
        assert!(is_operation_allowed(PacmanOperation::Verify, &extended));
        assert!(is_operation_allowed(PacmanOperation::UpdateCheck, &extended));
        
        // Test with only install allowed
        let install_only = vec!["install".to_string()];
        assert!(is_operation_allowed(PacmanOperation::Install, &install_only));
        assert!(!is_operation_allowed(PacmanOperation::Update, &install_only));
    }
    
    #[test]
    fn test_should_use_passwordless_sudo() {
        // Test with feature disabled
        // Test with feature enabled and available
        // Test with feature enabled but not available
        // Test with feature enabled but insecure configuration
        // Test with operation not allowed (remove, cache clean)
        // Test with operation allowed (install, update)
    }
}
```

### 2. Integration Tests

**Test Install Flow:**
```rust
#[cfg(test)]
mod integration_tests {
    #[test]
    fn test_install_with_passwordless_sudo() {
        // Configure passwordless sudo
        // Enable feature in settings
        // Attempt install
        // Verify no password prompt
        // Verify install succeeds
    }
    
    #[test]
    fn test_install_without_passwordless_sudo() {
        // Disable passwordless sudo
        // Attempt install
        // Verify password prompt appears
        // Verify install succeeds with password
    }
    
    #[test]
    fn test_install_fallback_to_password() {
        // Enable passwordless sudo but misconfigure sudoers
        // Attempt install
        // Verify fallback to password prompt
        // Verify install succeeds with password
    }
    
    #[test]
    fn test_remove_always_requires_password() {
        // Enable passwordless sudo with install and update allowed
        // Attempt remove operation
        // Verify password prompt appears (even with passwordless sudo enabled)
        // Verify remove succeeds with password
    }
    
    #[test]
    fn test_update_with_passwordless_sudo() {
        // Configure passwordless sudo
        // Enable feature in settings with update allowed
        // Attempt system update (-Syu)
        // Verify no password prompt
        // Verify update succeeds
    }
    
    #[test]
    fn test_install_with_operation_restriction() {
        // Configure passwordless sudo
        // Enable feature but only allow "update" operation
        // Attempt install
        // Verify password prompt appears (install not allowed)
        // Verify install succeeds with password
    }
    
    #[test]
    fn test_aur_install_always_requires_password() {
        // Configure passwordless sudo with install allowed
        // Attempt AUR package installation (paru -S malicious-package)
        // Verify password prompt appears (AUR installs always require password)
        // Verify install succeeds with password
        // This prevents exploitation: search AUR package -> install without password
    }
    
    #[test]
    fn test_combined_operation_exploitation_prevention() {
        // Test scenario: search + install of AUR package
        // 1. Search for AUR package (safe, read-only)
        // 2. Attempt to install AUR package without password
        // Verify: Password is required for AUR install, even if passwordless sudo
        //         is enabled and install operation is allowed
        // This prevents exploitation via operation chaining
    }
}
```

### 3. Manual Testing Checklist

- [ ] Enable `use_passwordless_sudo = true` in settings.conf
- [ ] Configure passwordless sudo in /etc/sudoers
- [ ] Verify security warning modal appears on first use
- [ ] Acknowledge warning and verify it doesn't appear again
- [ ] Perform install operation and verify no password prompt
- [ ] Perform update operation and verify no password prompt
- [ ] Disable feature and verify password prompts return
- [ ] Test with insecure sudoers configuration (NOPASSWD: ALL)
- [ ] Verify fallback to password prompt when insecure
- [ ] Test with passwordless sudo not configured
- [ ] Verify graceful error messages
- [ ] Test install operation with passwordless sudo enabled
- [ ] Test update operation (-Syu) with passwordless sudo enabled
- [ ] Test remove operation (-R) - verify password always required
- [ ] Test cache clean operation (-Sc) - verify password always required
- [ ] Test with only "install" allowed (update should require password)
- [ ] Test with only "update" allowed (install should require password)
- [ ] Test AUR package installation (paru/yay -S) - verify password always required
- [ ] Test combined operation: search AUR package then attempt install - verify password required
- [ ] Verify official repo installs work with passwordless sudo
- [ ] Verify AUR installs always require password, even with passwordless sudo enabled

### 4. Security Testing

- [ ] Verify that insecure configurations are detected
- [ ] Verify that secure configurations are accepted
- [ ] Test with various sudoers configurations
- [ ] Verify that validation doesn't break with complex sudoers files
- [ ] Test edge cases (comments, line continuations, includes)

---

## User Education & Warnings

### 1. Security Warning Modal Content

**Warning Text:**
```
⚠️  SECURITY WARNING: Passwordless Sudo

You have enabled passwordless sudo for package operations. This feature
allows Pacsea to execute privileged commands without prompting for your
password.

OPERATION RESTRICTIONS:
• Passwordless sudo is restricted to: install (-S) and update (-Syu/-Syyu)
• Remove operations (-R, -Rs, -Rns) always require password
• Cache operations (-Sc) always require password
• This provides additional security for destructive operations

SECURITY IMPLICATIONS:
• If your user account is compromised, attackers can install/update
  packages without your password
• Anyone with access to your unlocked session can execute privileged
  commands
• Malicious AUR packages could execute arbitrary commands as root

RECOMMENDED CONFIGURATION:
Limit NOPASSWD to specific commands only:
  username ALL=(ALL) NOPASSWD: /usr/bin/pacman, /usr/bin/paru, /usr/bin/yay

DO NOT use:
  username ALL=(ALL) NOPASSWD: ALL

For more information, see:
https://wiki.archlinux.org/title/Sudo#Configuration_examples

[Enter] Acknowledge and continue  [Esc] Cancel
```

### 2. Configuration File Comments

Add comprehensive comments to `config/settings.conf` explaining:
- What the feature does
- Security implications
- How to configure sudoers securely
- Links to documentation

### 3. Error Messages

**When passwordless sudo is not configured:**
```
Passwordless sudo is enabled but not configured on your system.

To configure passwordless sudo, add the following to /etc/sudoers.d/pacsea:
  username ALL=(ALL) NOPASSWD: /usr/bin/pacman, /usr/bin/paru, /usr/bin/yay

Replace 'username' with your actual username.

For security, limit NOPASSWD to specific commands only.
See: https://wiki.archlinux.org/title/Sudo#Configuration_examples

Falling back to password prompt...
```

**When sudoers configuration is insecure:**
```
Passwordless sudo is available but your sudoers configuration is insecure.

Detected: NOPASSWD: ALL (grants unrestricted root access)

For security, limit NOPASSWD to specific commands:
  username ALL=(ALL) NOPASSWD: /usr/bin/pacman, /usr/bin/paru, /usr/bin/yay

Falling back to password prompt for this operation.
```

**When operation is not allowed for passwordless sudo:**
```
Passwordless sudo is enabled but this operation requires a password.

Operation: Install AUR package (paru -S malicious-package)
Reason: AUR package installations always require password authentication for security.

SECURITY NOTE: AUR packages are user-submitted and unverified. PKGBUILDs contain
bash code that executes during build/install - if AUR helpers (yay/paru) run with
sudo/NOPASSWD privileges, malicious PKGBUILDs can execute arbitrary commands as
root, making this a critical security vulnerability.

This prevents exploitation via operation chaining:
  1. Search for AUR package (safe, read-only)
  2. Install malicious package without password (BLOCKED - requires password)

Passwordless sudo is only available for safe operations:
  - install (-S): Package installation from OFFICIAL repositories only (pacman -S)
  - update (-Syu/-Syyu): System updates from official repositories
  - sync (-Sy, -Fy): Database synchronization
  - search (-Ss): Package search (read-only)
  - query (-Si, -Qi, etc.): Package information (read-only)
  - filequery (-F): File database queries (read-only)

The following operations ALWAYS require password:
  - AUR package installations (paru/yay -S): User-submitted, unverified packages
  - Remove (-R, -Rs, -Rns, etc.): Can break system
  - Cache clean (-Sc, -Scc): Can cause data loss
  - Install from local file (-U): High risk of malicious packages
  - Operations with unsafe flags (--force, --nodeps, etc.)

Falling back to password prompt...
```

### 4. Documentation

**Add to README or create separate documentation:**
- Security considerations section
- Operation restrictions (install/update only, remove/cache always require password)
- Step-by-step sudoers configuration guide following ArchWiki best practices
- ArchWiki warnings (partial upgrades, AUR security, etc.)
- Sudoers file requirements (permissions, absolute paths, visudo usage)
- Troubleshooting common issues
- Best practices (Cmnd_Alias, NOEXEC, secure_path, env_reset)
- Configuration examples for different use cases
- Command chaining prevention
- Environment sanitization requirements

---

## Implementation Steps

### Phase 1: Core Infrastructure (Week 1)

1. **Add Settings Fields**
   - [ ] Add `use_passwordless_sudo` to `Settings` struct
   - [ ] Add `validate_sudoers_security` to `Settings` struct
   - [ ] Add `passwordless_sudo_warning_acknowledged` to `Settings` struct
   - [ ] Add `passwordless_sudo_allowed_operations` to `Settings` struct
   - [ ] Update `Settings::default()`
   - [ ] Add parsing logic in `parse_settings.rs`
   - [ ] Add saving logic in `settings_save.rs`
   - [ ] Update `config/settings.conf` with examples

2. **Implement Detection Functions**
   - [ ] Implement `check_sudo_passwordless_available()`
   - [ ] Implement `validate_sudoers_security()`
   - [ ] Implement `PacmanOperation` enum
   - [ ] Implement `detect_pacman_operation()`
   - [ ] Implement `is_operation_allowed()`
   - [ ] Implement `should_use_passwordless_sudo()` with operation parameter
   - [ ] Add unit tests for detection functions
   - [ ] Add unit tests for operation detection and restriction
   - [ ] Add integration tests

### Phase 2: Integration (Week 2)

3. **Integrate with Existing Code**
   - [ ] Update `src/args/update.rs` `prompt_and_validate_password()`
   - [ ] Update `src/app/runtime/workers/executor.rs` `handle_install_request()`
   - [ ] Update `src/install/executor.rs` password handling
   - [ ] Update `src/events/modals/handlers.rs` password prompt logic
   - [ ] Test all integration points

4. **Add Warning Modal**
   - [ ] Add `PasswordlessSudoWarning` modal variant
   - [ ] Implement warning modal UI
   - [ ] Implement warning modal event handlers
   - [ ] Integrate warning modal into password decision logic
   - [ ] Add translations for warning text

### Phase 3: Testing & Refinement (Week 3)

5. **Comprehensive Testing**
   - [ ] Write unit tests for all new functions
   - [ ] Write integration tests for install/update flows
   - [ ] Perform manual testing with various configurations
   - [ ] Test security validation with different sudoers configurations
   - [ ] Test error handling and fallback scenarios

6. **Documentation & Polish**
   - [ ] Add comprehensive comments to configuration file
   - [ ] Write user documentation
   - [ ] Add error messages and help text
   - [ ] Update translations
   - [ ] Code review and refactoring

### Phase 4: Release Preparation (Week 4)

7. **Final Steps**
   - [ ] Run full test suite
   - [ ] Fix any remaining issues
   - [ ] Update CHANGELOG.md
   - [ ] Create PR with detailed description
   - [ ] Get code review feedback
   - [ ] Address review comments
   - [ ] Merge to main branch

---

## Risk Assessment

### 1. Security Risks

**Risk Level: MEDIUM-HIGH**

**Risks:**
- Users may configure insecure sudoers settings
- Malicious packages could exploit passwordless sudo
- Compromised user accounts gain root access

**Mitigation:**
- Default to disabled (opt-in)
- Security validation before use
- Clear warnings and documentation
- Fallback to password prompts on validation failure

### 2. Compatibility Risks

**Risk Level: LOW**

**Risks:**
- May not work with all sudoers configurations
- Security validation may have false positives/negatives
- Different sudo versions may behave differently

**Mitigation:**
- Graceful fallback to password prompts
- Extensive testing with various configurations
- Clear error messages for troubleshooting

### 3. User Experience Risks

**Risk Level: LOW**

**Risks:**
- Users may be confused by security warnings
- Configuration may be complex for some users
- False positives in security validation may frustrate users

**Mitigation:**
- Clear, concise warnings
- Comprehensive documentation
- Option to disable security validation (with warning)
- Helpful error messages with solutions

### 4. Maintenance Risks

**Risk Level: LOW**

**Risks:**
- Sudoers parsing may need updates for new sudo versions
- Security validation logic may need refinement
- Edge cases in sudoers configuration may not be handled

**Mitigation:**
- Well-documented code
- Comprehensive test coverage
- Clear error messages for edge cases
- Regular review and updates

---

## Future Enhancements

### 1. Advanced Security Validation

- Parse sudoers files more accurately (handle comments, line continuations)
- Support group-based configurations
- Handle include directives
- Validate timestamp_timeout and requiretty settings
- Check for other security-related sudo options

### 2. Sudoers Configuration Assistant

- Interactive tool to help users configure sudoers securely
- Generate recommended sudoers.d file
- Validate configuration before saving
- Test configuration before enabling in Pacsea

### 3. Per-Operation Configuration

- Allow different settings for install vs update operations
- Allow per-package exceptions (always require password for certain packages)
- Time-based restrictions (require password during certain hours)

### 4. Audit Logging

- Log all privileged operations when using passwordless sudo
- Track which packages were installed/updated without password
- Provide audit trail for security review

### 5. Session Management

- Cache passwordless sudo status (check periodically, not on every operation)
- Handle sudo timeout gracefully
- Re-validate configuration on sudo timeout

---

## Conclusion

This implementation plan provides a comprehensive approach to adding passwordless sudo support to Pacsea while maintaining security best practices. The feature is designed to be:

- **Secure**: Opt-in, with security validation and clear warnings
- **User-Friendly**: Automatic detection, graceful fallbacks, helpful error messages
- **Maintainable**: Well-documented, thoroughly tested, extensible

The phased implementation approach allows for incremental development and testing, reducing risk and ensuring quality at each stage.

**Key Success Criteria:**
1. Feature works reliably with secure sudoers configurations
2. Security warnings are clear and effective
3. Fallback to password prompts works in all scenarios
4. User experience is smooth and intuitive
5. Code is well-tested and maintainable

---

## Appendix: Sudoers Configuration Examples

### Secure Configuration (Following ArchWiki Best Practices)

```bash
# /etc/sudoers.d/pacsea
# Allow user to run specific package manager operations without password
# Following ArchWiki security guidelines

# Define command aliases for maintainability (ArchWiki best practice)
Cmnd_Alias PACMAN_SAFE_INSTALL = /usr/bin/pacman -S, /usr/bin/pacman -Syu, /usr/bin/pacman -Syyu
Cmnd_Alias PACMAN_SAFE_QUERY = /usr/bin/pacman -Ss *, /usr/bin/pacman -Qi *, /usr/bin/pacman -Q *, /usr/bin/pacman -F *

# Allow safe operations without password
username ALL=(ALL) NOPASSWD: PACMAN_SAFE_INSTALL, PACMAN_SAFE_QUERY

# Security defaults (ArchWiki recommendations)
Defaults:username timestamp_timeout=0
Defaults:username requiretty
Defaults:username env_reset
Defaults:username secure_path="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
Defaults:username !tty_tickets  # Per-terminal sudo (default, safer)

# Use NOEXEC for query operations to prevent shell escapes (ArchWiki security)
Defaults:username NOEXEC: PACMAN_SAFE_QUERY

# IMPORTANT: File must be owned by root:root with permissions 0440
# Always use: sudo visudo -f /etc/sudoers.d/pacsea
```

### Minimal Secure Configuration

```bash
# /etc/sudoers.d/pacsea
# Minimal configuration: only system updates

Cmnd_Alias PACMAN_UPDATE = /usr/bin/pacman -Syu, /usr/bin/pacman -Syyu

username ALL=(ALL) NOPASSWD: PACMAN_UPDATE

Defaults:username timestamp_timeout=0
Defaults:username requiretty
Defaults:username env_reset
```

### Insecure Configuration (NOT RECOMMENDED)

```bash
# /etc/sudoers.d/pacsea
# WARNING: This grants unrestricted root access without password
# DO NOT USE THIS CONFIGURATION

username ALL=(ALL) NOPASSWD: ALL
```

### Common Mistakes to Avoid (Based on ArchWiki Warnings)

```bash
# WRONG: Relative paths (ArchWiki requires absolute paths)
username ALL=(ALL) NOPASSWD: pacman
# Why unsafe: Relative paths can be exploited via PATH manipulation - attacker can create malicious "pacman" binary in user-writable directory
# CORRECT: Use absolute path
username ALL=(ALL) NOPASSWD: /usr/bin/pacman

# WRONG: Wildcard paths (unsafe)
username ALL=(ALL) NOPASSWD: /usr/bin/*
# Why unsafe: Wildcard allows ALL commands in /usr/bin, not just pacman - grants unrestricted root access to every system binary
# CORRECT: Specific commands only
username ALL=(ALL) NOPASSWD: /usr/bin/pacman -Syu

# WRONG: Commands in user-writable directories
username ALL=(ALL) NOPASSWD: ~/scripts/pacman-wrapper
# Why unsafe: User can modify the script to execute arbitrary commands as root - complete privilege escalation vulnerability
# CORRECT: Only system binaries
username ALL=(ALL) NOPASSWD: /usr/bin/pacman

# WRONG: Allowing editors or shell-escape commands
username ALL=(ALL) NOPASSWD: /usr/bin/vim, /usr/bin/nano
# Why unsafe: Editors allow shell escapes (e.g., :!/bin/bash in vim) which can execute arbitrary commands as root, bypassing sudo restrictions
# CORRECT: Use sudoedit instead, or don't allow editors

# WRONG: Allowing AUR helpers with sudo/NOPASSWD
username ALL=(ALL) NOPASSWD: /usr/bin/yay, /usr/bin/paru
# Why unsafe: PKGBUILDs contain bash code that executes during build/install - if yay/paru run with 
# sudo/NOPASSWD privileges, malicious PKGBUILDs can execute arbitrary commands as root. ArchWiki and 
# the Arch community explicitly warn against running makepkg or AUR helpers as root because untrusted 
# build scripts get full system access. Even if helpers run makepkg as non-root, install scripts 
# (.install files) and package() functions execute with elevated privileges.
# CORRECT: Never allow AUR helpers in passwordless sudo - always require password for AUR operations
```

### Group-Based Configuration

```bash
# /etc/sudoers.d/pacsea
# Allow members of 'packageusers' group to run package managers without password

Cmnd_Alias PACMAN_SAFE = /usr/bin/pacman -Syu, /usr/bin/pacman -Syyu, /usr/bin/pacman -S

%packageusers ALL=(ALL) NOPASSWD: PACMAN_SAFE

Defaults:%packageusers timestamp_timeout=0
Defaults:%packageusers requiretty
Defaults:%packageusers env_reset
```

### Validation Commands

After creating or modifying sudoers files, always validate:

```bash
# Validate sudoers syntax
sudo visudo -c

# Validate specific file
sudo visudo -f /etc/sudoers.d/pacsea

# Check file permissions (must be 0440, root:root)
ls -l /etc/sudoers.d/pacsea
# Should show: -r--r----- 1 root root

# Test passwordless sudo
sudo -n /usr/bin/pacman -Syu --noconfirm
```

---

## ArchWiki-Based Security Enhancements

This plan has been enhanced based on ArchWiki guidelines and security best practices. Key improvements include:

### 1. Sudoers Configuration Requirements (ArchWiki)
- **Absolute Paths**: All commands must use absolute paths (`/usr/bin/pacman`, not `pacman`)
- **File Permissions**: Sudoers files must be `0440` (root:root) - validated in code
- **Cmnd_Alias Usage**: Recommended for maintainability and clarity
- **NOEXEC Tag**: Use for query operations to prevent shell escapes
- **Environment Sanitization**: `env_reset` and `secure_path` requirements

### 2. Partial Upgrade Prevention (ArchWiki Warning)
- **`-Sy` without `-u`**: ArchWiki explicitly warns this can cause dependency breakage
- **Detection**: Code now detects and blocks partial upgrades
- **Only Allow**: `-Syu` or `-Syyu` for updates, never standalone `-Sy`

### 3. Command Chaining Prevention
- **Operators Blocked**: `&&`, `;`, `||` are detected and blocked
- **Prevents**: Combining safe and unsafe operations in single invocation
- **Security**: Prevents exploitation via operation chaining

### 4. Enhanced Flag Validation
- **ArchWiki-Specific Flags**: Added `--nosave`, `--asdeps`, `--asexplicit` to unsafe list
- **`--overwrite` Warning**: ArchWiki notes this should be "generally avoided unless necessary"
- **Comprehensive Blocking**: All unsafe flags block passwordless sudo regardless of operation

### 5. Sudoers Validation Enhancements
- **Permission Checking**: Validates file permissions (0440 requirement)
- **Path Validation**: Checks for absolute paths, rejects relative/wildcard paths
- **Cmnd_Alias Detection**: Recognizes and validates Cmnd_Alias usage
- **User-Writable Path Detection**: Blocks commands in user-writable directories

### 6. Documentation Improvements
- **ArchWiki References**: Added links and references to ArchWiki guidelines
- **Configuration Examples**: Updated with ArchWiki-compliant examples
- **Validation Commands**: Added `visudo -c` validation steps
- **Common Mistakes**: Added section on what NOT to do (based on ArchWiki warnings)

### 7. Security Defaults (ArchWiki Recommendations)
- **timestamp_timeout=0**: Require re-authentication for each session
- **requiretty**: Require TTY for additional security
- **env_reset**: Reset environment variables
- **secure_path**: Restrict PATH to system directories
- **!tty_tickets**: Per-terminal sudo (default, safer)

### 8. Testing Enhancements
- **Partial Upgrade Tests**: Verify `-Sy` without `-u` is blocked
- **Command Chaining Tests**: Verify `&&`, `;`, `||` are detected
- **Permission Validation**: Test sudoers file permission checking
- **Path Validation**: Test absolute path requirements

### Key ArchWiki Sources Referenced:
- `sudoers(5)` man page - Command specification and security
- `pacman(8)` man page - Flag safety and partial upgrade warnings
- ArchWiki Sudo page - Configuration best practices
- ArchWiki Pacman page - Partial upgrade warnings
- ArchWiki AUR page - Security considerations for AUR packages
- ArchWiki makepkg page - Warnings against running as root
- ArchWiki AUR helpers - Security warnings about running with sudo

### Verified Security Concerns:
- **PKGBUILDs with sudo**: ✅ VERIFIED - PKGBUILDs contain bash code that executes during build/install. If AUR helpers (yay/paru) run with sudo/NOPASSWD privileges, malicious PKGBUILDs can execute arbitrary commands as root. This is a critical security vulnerability confirmed by ArchWiki and the Arch community.
- **ArchWiki Warnings**: ✅ VERIFIED - ArchWiki and the Arch community explicitly warn against running makepkg or AUR helpers as root because untrusted build scripts get full system access.
- **makepkg Root Prohibition**: ✅ VERIFIED - makepkg itself refuses to run as root by default, precisely because of this security risk. Even if helpers run makepkg as non-root, install scripts (.install files) and package() functions can execute with elevated privileges when the helper runs with sudo.

### Additional Safe Operations Identified:
Based on ArchWiki guidelines and codebase analysis, the following additional safe operations have been added to the plan:

1. **Download-Only (`download`)**: `pacman -Sw` - Downloads packages to cache without installing. Safe because it only writes to cache directory, no system modifications.

2. **Verification (`verify`)**: `pacman --check`, `pacman -Qk`, `pacman -Qkk` - Read-only integrity checks. Safe because they only verify package integrity without modifying system state.

3. **Update Check (`updatecheck`)**: `pacman -Qu` - Read-only operation that checks for available updates without installing. Safe because it only queries databases, no system modifications.

These operations are all read-only or write-only to cache directories, making them safe for passwordless sudo when properly configured.

---

**Document Version:** 2.0  
**Last Updated:** 2026-01-18  
**Author:** Pacsea Development Team  
**ArchWiki Compliance:** Enhanced with ArchWiki security guidelines
