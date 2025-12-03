# Testing Checklist for Integrated Process Features

This document provides a simplified testing checklist focusing on user flows for the `feat/integrated-process` PR.

## Prerequisites

Before testing, ensure:
- [ ] System has `pacman` available
- [ ] System has `paru` or `yay` available (for AUR tests)
- [ ] You have sudo access (for password prompt tests)
- [ ] Test environment is safe (consider using a VM or test system)

---

## 1. Package Installation Flow

**What it checks**: Complete installation workflow from selection to completion

- [ ] **Install official package**: Select and install an official package (e.g., `ripgrep`)
  - Verifies: Live output streaming, password prompt if needed, successful installation

- [ ] **Install AUR package**: Select and install an AUR package
  - Verifies: AUR helper integration, live build output, password prompt handling

- [ ] **Install multiple packages**: Select and install multiple packages at once
  - Verifies: Batch processing, sequential execution, all packages install correctly

- [ ] **Install already installed package**: Attempt to install a package that's already installed
  - Verifies: Reinstall confirmation modal appears, can confirm or cancel

---

## 2. Package Removal Flow

**What it checks**: Complete removal workflow with different cascade modes

- [ ] **Remove package**: Select and remove a package
  - Verifies: Password prompt if needed, live output, successful removal

- [ ] **Remove with dependencies**: Remove package with cascade modes (Basic/Cascade/CascadeWithConfigs)
  - Verifies: Different removal modes work correctly, dependencies handled properly

- [ ] **Remove multiple packages**: Select and remove multiple packages
  - Verifies: Batch removal, all packages removed successfully

---

## 3. Preflight Modal Flow

**What it checks**: Preflight review and execution workflow

- [ ] **Preflight review and install**: Select packages, open preflight, review summary, confirm installation
  - Verifies: Summary generation, risk calculation with dependents, execution via PTY, live output

- [ ] **Preflight review and remove**: Select packages to remove, open preflight, review summary, confirm removal
  - Verifies: Removal summary, cascade mode selection, execution works correctly

- [ ] **Preflight with dependencies**: Select package with dependencies, view preflight summary
  - Verifies: Dependent packages listed, risk calculation includes dependents (+2 per dependent)

---

## 4. Direct Install/Remove Flow

**What it checks**: Bypass preflight and install/remove directly

- [ ] **Direct install**: Configure to skip preflight, install package directly
  - Verifies: Bypasses preflight, executes via PTY, live output, password prompt if needed

- [ ] **Direct remove**: Configure to skip preflight, remove package directly
  - Verifies: Bypasses preflight, executes via PTY, live output, password prompt if needed

- [ ] **Direct install with reinstall**: Direct install already installed package
  - Verifies: Reinstall confirmation still appears, works with direct flow

---

## 5. System Update Flow

**What it checks**: Complete system update workflow

- [ ] **Mirror update**: Trigger mirror update
  - Verifies: Command executes via PTY, live output, password prompt if needed

- [ ] **Pacman database update**: Trigger pacman database update
  - Verifies: Command executes via PTY, live output streams

- [ ] **AUR update**: Trigger AUR helper update
  - Verifies: AUR helper command executes, live output streams

- [ ] **Cache cleanup**: Trigger cache cleanup
  - Verifies: Command executes via PTY, live output, password prompt if needed

- [ ] **Full system update sequence**: Trigger all update operations
  - Verifies: Commands execute in sequence, each output visible, chain stops on failure

---

## 6. Password Prompt Flow

**What it checks**: Password prompt modal behavior in various scenarios

- [ ] **Install requiring sudo**: Install package that requires sudo
  - Verifies: Password modal appears, shows correct purpose, input is masked, correct password proceeds, incorrect password shows error

- [ ] **File database sync**: Trigger file database sync (press 'F' in search)
  - Verifies: Non-sudo attempt first, password prompt if needed, shows FileSync purpose, executes sudo command

- [ ] **Cancel password prompt**: Trigger operation requiring password, then cancel
  - Verifies: Operation aborts gracefully, no partial state

---

## 7. Security Scan Flow

**What it checks**: Security scanning workflow for AUR packages

- [ ] **Single scan type**: Enable one scan type (ClamAV/Trivy/Semgrep/ShellCheck/VirusTotal/Custom pattern)
  - Verifies: Scan executes via PTY, live output, results displayed correctly

- [ ] **Multiple scans**: Enable multiple scan types simultaneously
  - Verifies: All scans execute via PTY, output from all visible, results combined properly

- [ ] **aur-sleuth scan**: Enable aur-sleuth scan
  - Verifies: aur-sleuth runs in separate terminal (not PTY), other scans still via PTY, both work simultaneously

---

## 8. Optional Dependencies Flow

**What it checks**: Optional dependency installation workflow

- [ ] **Install optional dependency**: Trigger installation of optional dependency (e.g., semgrep-bin)
  - Verifies: Uses AUR helper flow, executes via PTY, live output, password prompt if needed

- [ ] **Install paru/yay**: Install paru or yay as optional dependency
  - Verifies: Uses temporary directory for cloning, prevents accidental deletion, installation completes

---

## 9. Downgrade Flow

**What it checks**: Package downgrade workflow (runs in separate terminal, not PTY)

- [ ] **Downgrade package**: Trigger downgrade for a package (with downgrade tool installed)
  - Verifies: Command executes in separate terminal (not PTY), interactive prompts work, password prompt if needed

- [ ] **Downgrade tool not found**: Trigger downgrade without downgrade tool installed
  - Verifies: Error message appears, suggests installing downgrade package

- [ ] **Downgrade multiple packages**: Select multiple packages to downgrade
  - Verifies: All packages included in command, downgrade executes correctly in separate terminal

---

## 10. Custom Command Flow

**What it checks**: Custom command execution workflow (used for special cases like paru/yay installation and file sync)

**Note**: Custom commands are used internally for:
- Installing optional dependencies (paru and yay) via `makepkg -si`
- File database sync (`sudo pacman -Fy`)
- They execute via PTY (not separate terminal)

- [ ] **Sudo custom command**: Trigger operation that uses custom command requiring sudo (e.g., install paru/yay, file sync)
  - Verifies: Password prompt appears, password passed correctly, command executes via PTY, live output

- [ ] **Non-sudo custom command**: Execute custom command without sudo
  - Verifies: No password prompt, command executes successfully via PTY

---

## 11. Loading and UI Feedback Flow

**What it checks**: Loading states and UI feedback during operations

- [ ] **Loading modal**: Trigger preflight summary generation
  - Verifies: Loading modal appears during async computation, shows appropriate message, disappears when complete

- [ ] **Auto-scrolling logs**: Start long-running operation (e.g., install multiple packages)
  - Verifies: Log panel auto-scrolls to show latest output, progress bars update correctly, latest output always visible

- [ ] **Progress bars**: Run operation with progress bars (e.g., pacman downloads)
  - Verifies: Progress bars display correctly, update in real-time, carriage return handling works

---

## 12. Dry-Run Mode Flow

**What it checks**: Dry-run mode respects all operations

- [ ] **Dry-run install/remove**: Run install or remove with `--dry-run` flag
  - Verifies: Commands shown but not executed, "DRY RUN:" prefix visible, no system changes

- [ ] **Dry-run system update**: Run system update with `--dry-run` flag
  - Verifies: All update commands shown but not executed, proper command quoting

- [ ] **Dry-run scan**: Run security scan with `--dry-run` flag
  - Verifies: Scan commands shown but not executed

- [ ] **Dry-run custom command**: Run custom command with `--dry-run` flag
  - Verifies: Command shown but not executed

---

## 13. Error Handling Flow

**What it checks**: Error conditions and recovery

- [ ] **Command execution failure**: Trigger operation that will fail (e.g., install non-existent package)
  - Verifies: Error displayed correctly, UI recovers gracefully, user can continue

- [ ] **Network failure**: Disconnect network during package installation
  - Verifies: Error detected and displayed, operation fails gracefully

- [ ] **Password prompt timeout**: Leave password prompt open for extended time
  - Verifies: System handles timeout correctly

---

## 14. Edge Cases

**What it checks**: Unusual scenarios and edge conditions

- [ ] **Empty package list**: Attempt to install/remove with empty list
  - Verifies: Appropriate message shown, no errors occur

- [ ] **Concurrent operations**: Attempt to trigger multiple operations simultaneously
  - Verifies: System handles correctly (queues or prevents), no race conditions

- [ ] **Large output**: Trigger operation with very large output
  - Verifies: Log panel handles correctly, performance remains acceptable

---

