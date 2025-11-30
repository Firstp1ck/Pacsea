# Terminal Process Migration Analysis

This document analyzes which terminal processes have been converted to integrated processes (PTY-based executor), which are still using terminal spawning, and which cannot or should not be converted.

## ✅ Completely Converted to Integrated Processes

These processes now use `ExecutorRequest` and execute via PTY in the TUI:

### 1. **Install (Batch) - via Preflight Modal**
- **Location**: `src/events/preflight/keys/action_keys.rs`
- **Status**: ✅ Converted
- **Implementation**: Uses `ExecutorRequest::Install` when triggered from Preflight modal
- **Executor Handler**: `src/app/runtime/workers/executor.rs` handles `ExecutorRequest::Install`
- **Command Builder**: `src/install/command.rs::build_install_command_for_executor`

### 2. **Remove - via Preflight Modal**
- **Location**: `src/events/preflight/keys/action_keys.rs`
- **Status**: ✅ Converted
- **Implementation**: Uses `ExecutorRequest::Remove` when triggered from Preflight modal
- **Executor Handler**: `src/app/runtime/workers/executor.rs` handles `ExecutorRequest::Remove`
- **Command Builder**: `src/install/command.rs::build_remove_command_for_executor`

### 3. **System Update**
- **Location**: `src/events/modals/system_update.rs`
- **Status**: ✅ Converted
- **Implementation**: Uses `ExecutorRequest::Update` for mirror updates, pacman updates, AUR updates, and cache cleanup
- **Executor Handler**: `src/app/runtime/workers/executor.rs` handles `ExecutorRequest::Update`
- **Command Builder**: `src/install/command.rs::build_update_command_for_executor`
- **Note**: Has test mode fallback to terminal spawning when `PACSEA_TEST_OUT` is set

### 4. **Custom Commands (paru/yay/semgrep-bin installation)**
- **Location**: `src/events/modals/handlers.rs`
- **Status**: ✅ Converted
- **Implementation**: Uses `ExecutorRequest::CustomCommand` for special package installations
- **Executor Handler**: `src/app/runtime/workers/executor.rs` handles `ExecutorRequest::CustomCommand`
- **Details**: Handles `makepkg -si` commands with sudo password via `SUDO_ASKPASS`

### 5. **Optional Dependencies Installation**
- **Location**: `src/events/modals/optional_deps.rs`
- **Status**: ✅ Converted
- **Implementation**: Uses `ExecutorRequest::Install` for installing optional dependencies
- **Note**: `semgrep-bin` now uses standard AUR helper flow; `paru`/`yay` use temporary directories for safe cloning

### 6. **Single Package Install (Direct)**
- **Location**: `src/install/direct.rs::start_integrated_install`
- **Status**: ✅ Converted
- **Usage**: Called when installing a single package directly (bypassing preflight modal)
- **Implementation**: Uses `ExecutorRequest::Install` via integrated process
- **Executor Handler**: `src/app/runtime/workers/executor.rs` handles `ExecutorRequest::Install`
- **Details**: Shows `PasswordPrompt` modal if password needed (official packages), otherwise goes directly to `PreflightExec`

### 7. **Batch Install (Direct)**
- **Location**: `src/install/direct.rs::start_integrated_install_all`
- **Status**: ✅ Converted
- **Usage**: Called when installing packages directly (bypassing preflight modal)
- **Implementation**: Uses `ExecutorRequest::Install` via integrated process
- **Executor Handler**: `src/app/runtime/workers/executor.rs` handles `ExecutorRequest::Install`
- **Details**: Shows `PasswordPrompt` modal if password needed, handles reinstall/batch update confirmation

### 8. **Remove (Direct)**
- **Location**: `src/install/direct.rs::start_integrated_remove_all`
- **Status**: ✅ Converted
- **Usage**: Called when removing packages directly (bypassing preflight modal)
- **Implementation**: Uses `ExecutorRequest::Remove` via integrated process
- **Executor Handler**: `src/app/runtime/workers/executor.rs` handles `ExecutorRequest::Remove`
- **Details**: Always shows `PasswordPrompt` modal as removal requires sudo

### 9. **Security Scans (excluding aur-sleuth)**
- **Location**: `src/events/modals/scan.rs`
- **Status**: ✅ Converted
- **Usage**: AUR package security scanning (ClamAV, Trivy, Semgrep, ShellCheck, VirusTotal, custom patterns)
- **Implementation**: Uses `ExecutorRequest::Scan` via integrated process
- **Executor Handler**: `src/app/runtime/workers/executor.rs` handles `ExecutorRequest::Scan`
- **Command Builder**: `src/install/executor.rs::build_scan_command_for_executor`
- **Details**: Scans run via PTY in TUI; aur-sleuth runs in separate terminal simultaneously when enabled

### 10. **File Database Sync Fallback**
- **Location**: `src/events/preflight/keys/command_keys.rs::handle_f_key`
- **Status**: ✅ Converted
- **Usage**: Fallback when file database sync fails (requires sudo)
- **Implementation**: Uses `ExecutorRequest::CustomCommand` via integrated process
- **Executor Handler**: `src/app/runtime/workers/executor.rs` handles `ExecutorRequest::CustomCommand`
- **Details**: Attempts `pacman -Fy` without sudo first; if it fails, shows `PasswordPrompt` modal and executes `sudo pacman -Fy` via integrated executor with `SUDO_ASKPASS`

## ⚠️ Still Using Terminal Spawning

These processes still spawn external terminals:

### 1. **Downgrade**
- **Location**: `src/events/modals/handlers.rs` (line 663)
- **Status**: ⚠️ Still uses terminal spawning
- **Usage**: Downgrade operations
- **Function**: `crate::install::spawn_shell_commands_in_terminal(&[cmd])`
- **Reason**: Explicitly documented as needing full terminal for interactive tool (`downgrade` command)
- **Conversion Feasibility**: ❌ Not recommended - interactive tool requires full terminal
- **Note**: Comment in code: "Spawn downgrade in a terminal (interactive tool needs full terminal)"

### 2. **aur-sleuth (Security Scan)**
- **Location**: `src/install/scan/spawn.rs::build_sleuth_command_for_terminal`
- **Status**: ⚠️ Still uses terminal spawning
- **Usage**: LLM-based AUR package audit tool
- **Function**: `build_sleuth_command_for_terminal(pkg: &str)` spawns terminal for aur-sleuth execution
- **Reason**: Long-running interactive scan operations that benefit from full terminal; runs simultaneously with other scans
- **Conversion Feasibility**: ❌ Not recommended - long-running interactive tool benefits from separate terminal
- **Note**: Runs in separate terminal while other security scans execute via integrated process

## ❌ Not Possible / Should Not Convert

### 1. **Downgrade**
- **Reason**: Interactive tool (`downgrade` command) requires full terminal for user interaction
- **Location**: `src/events/modals/handlers.rs:663`
- **Documentation**: Code explicitly states "interactive tool needs full terminal"

### 2. **AUR Scan (aur-sleuth)**
- **Reason**: Long-running interactive scan operations that benefit from full terminal
- **Location**: `src/install/scan/spawn.rs`
- **Documentation**: Tests explicitly allow terminal spawning for sleuth scans

## Summary Statistics

- **Converted**: 10 processes (Install via Preflight, Remove via Preflight, System Update, Custom Commands, Optional Deps, Single Install Direct, Batch Install Direct, Remove Direct, Security Scans, File DB Sync Fallback)
- **Still Using Terminal**: 2 processes (Downgrade, aur-sleuth)
- **Not Convertible**: 2 processes (Downgrade - interactive, aur-sleuth - long-running interactive)

## Recommendations

### Low Priority / Keep as Terminal
1. **Downgrade** - Keep as terminal (interactive tool requirement)
2. **aur-sleuth** - Keep as terminal (long-running interactive tool, runs simultaneously with other scans)

## Implementation Notes

- All converted processes use `ExecutorRequest` enum variants
- Executor worker in `src/app/runtime/workers/executor.rs` handles all ExecutorRequest types (refactored into helper functions)
- PTY execution provides live output streaming in PreflightExec modal
- Password handling is done via PasswordPrompt modal before execution
- Dry-run mode is supported for all executor operations
- Direct install/remove operations (`src/install/direct.rs`) handle password prompting and reinstall/batch update confirmation before execution
- Direct operations check for reinstall scenarios (installed packages without updates) and show appropriate confirmation modals
- Security scans use `ExecutorRequest::Scan` for integrated execution; aur-sleuth runs in separate terminal simultaneously when enabled
- File database sync attempts non-sudo sync first; on failure, shows password prompt and executes `sudo pacman -Fy` via integrated executor with `SUDO_ASKPASS`

