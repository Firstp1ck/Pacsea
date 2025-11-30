# Testing Checklist for Integrated Process Features

This document provides a comprehensive testing checklist for all processes integrated in the `feat/integrated-process` PR. Use this to verify each feature works correctly.

## Prerequisites

Before testing, ensure:
- [ ] System has `pacman` available
- [ ] System has `paru` or `yay` available (for AUR tests)
- [ ] You have sudo access (for password prompt tests)
- [ ] Test environment is safe (consider using a VM or test system)

---

## 1. Live Output Streaming

**Location**: `src/app/runtime/workers/executor.rs`

### Test Scenarios

- [ ] **Install official package**: Install a small official package (e.g., `ripgrep`) and verify:
  - [ ] Output appears in real-time in the log panel
  - [ ] Progress indicators update live
  - [ ] No buffering delays visible
  - [ ] Output is properly formatted (no ANSI artifacts)

- [ ] **Install AUR package**: Install an AUR package and verify:
  - [ ] Live output from AUR helper (paru/yay)
  - [ ] Build progress visible in real-time
  - [ ] Output streams correctly

- [ ] **System update**: Run system update and verify:
  - [ ] Each command's output streams live
  - [ ] Progress bars update in real-time
  - [ ] Multiple commands show sequential output

---

## 2. Password Prompt Modal

**Location**: `src/ui/modals/password.rs`

### Test Scenarios

- [ ] **Install official package requiring sudo**:
  - [ ] Password prompt modal appears when sudo is needed
  - [ ] Modal shows correct purpose (Install/Remove/Update/etc.)
  - [ ] Password input is masked (not visible)
  - [ ] Entering correct password proceeds with installation
  - [ ] Entering incorrect password shows error and allows retry
  - [ ] Canceling password prompt cancels the operation

- [ ] **File database sync password prompt**:
  - [ ] Trigger file database sync (e.g., press 'F' in search)
  - [ ] If non-sudo sync fails, password prompt appears
  - [ ] Password prompt shows `FileSync` purpose
  - [ ] Entering password executes `sudo pacman -Fy`

- [ ] **Custom command password prompt**:
  - [ ] Trigger custom command requiring sudo (e.g., `makepkg -si`)
  - [ ] Password prompt appears
  - [ ] Password is correctly passed to command

---

## 3. Loading Modal

**Location**: `src/ui/modals/misc.rs`

### Test Scenarios

- [ ] **Post-summary computation**:
  - [ ] Trigger preflight summary generation
  - [ ] Loading modal appears during async computation
  - [ ] Modal shows appropriate message
  - [ ] Modal disappears when computation completes

---

## 4. Auto-Scrolling Logs

**Location**: `src/ui/modals/preflight_exec.rs`

### Test Scenarios

- [ ] **Auto-scroll during execution**:
  - [ ] Start a long-running operation (e.g., install multiple packages)
  - [ ] Verify log panel automatically scrolls to show latest output
  - [ ] Latest output is always visible
  - [ ] Progress bars update correctly

- [ ] **Progress bar support**:
  - [ ] Operations with progress bars (e.g., pacman downloads) show correctly
  - [ ] Progress bars update in real-time
  - [ ] Carriage return handling works (progress bars replace previous line)

---

## 5. Reinstall Confirmation

**Location**: `src/ui/modals/confirm.rs`, `src/events/modals/handlers.rs`

### Test Scenarios

- [ ] **Install already installed package**:
  - [ ] Select a package that is already installed
  - [ ] Attempt to install it
  - [ ] Reinstall confirmation modal appears
  - [ ] Confirming proceeds with reinstallation
  - [ ] Canceling aborts the operation

- [ ] **Batch reinstall**:
  - [ ] Select multiple packages, some already installed
  - [ ] Reinstall confirmation appears
  - [ ] Confirming reinstalls all selected packages

- [ ] **Direct install reinstall**:
  - [ ] Use direct install (bypassing preflight) for installed package
  - [ ] Reinstall confirmation appears
  - [ ] Works correctly with direct install flow

---

## 6. Enhanced Preflight Risk Calculation

**Location**: `src/logic/preflight/mod.rs`, `src/ui/modals/preflight/tabs/summary.rs`

### Test Scenarios

- [ ] **Dependent packages display**:
  - [ ] Select a package that has dependencies
  - [ ] View preflight summary
  - [ ] Dependent packages are listed in summary
  - [ ] Dependent packages are clearly labeled

- [ ] **Risk calculation with dependents**:
  - [ ] Select package with dependencies
  - [ ] Verify risk score includes +2 per dependent package
  - [ ] Risk calculation is accurate
  - [ ] Summary shows correct total risk

- [ ] **Multiple dependents**:
  - [ ] Select package with multiple dependencies
  - [ ] All dependents are shown
  - [ ] Risk calculation accounts for all dependents

---

## 7. System Update Integration

**Location**: `src/events/modals/system_update.rs`, `src/app/runtime/workers/executor.rs`

### Test Scenarios

- [ ] **Mirror update**:
  - [ ] Trigger mirror update
  - [ ] Command executes via PTY
  - [ ] Output streams live
  - [ ] Password prompt appears if needed

- [ ] **Pacman update**:
  - [ ] Trigger pacman database update
  - [ ] Command executes via PTY
  - [ ] Output streams live

- [ ] **AUR update**:
  - [ ] Trigger AUR update
  - [ ] Command executes via PTY
  - [ ] Output streams live

- [ ] **Cache cleanup**:
  - [ ] Trigger cache cleanup
  - [ ] Command executes via PTY
  - [ ] Output streams live
  - [ ] Password prompt appears if needed

- [ ] **Full system update sequence**:
  - [ ] Trigger all update operations
  - [ ] Commands execute in sequence
  - [ ] Each command's output is visible
  - [ ] Chain stops on first failure (if any)

- [ ] **Dry-run mode**:
  - [ ] Run system update in dry-run mode
  - [ ] Commands are shown but not executed
  - [ ] Output shows "DRY RUN:" prefix

---

## 8. Direct Install/Remove Integration

**Location**: `src/install/direct.rs`, `src/events/install/mod.rs`, `src/events/search/preflight_helpers.rs`

### Test Scenarios

- [ ] **Direct install official package**:
  - [ ] Configure to skip preflight
  - [ ] Install official package directly
  - [ ] Command executes via PTY
  - [ ] Output streams live
  - [ ] Password prompt appears if needed

- [ ] **Direct install AUR package**:
  - [ ] Configure to skip preflight
  - [ ] Install AUR package directly
  - [ ] Command executes via PTY
  - [ ] Output streams live

- [ ] **Direct remove package**:
  - [ ] Configure to skip preflight
  - [ ] Remove package directly
  - [ ] Command executes via PTY
  - [ ] Output streams live
  - [ ] Password prompt appears if needed

- [ ] **Direct install with reinstall**:
  - [ ] Direct install already installed package
  - [ ] Reinstall confirmation appears
  - [ ] Works correctly

- [ ] **Batch update logic**:
  - [ ] Direct install multiple packages
  - [ ] Batch update logic works correctly
  - [ ] All packages install successfully

- [ ] **Dry-run mode**:
  - [ ] Run direct install/remove in dry-run mode
  - [ ] Commands are shown but not executed

---

## 9. Security Scan Integration

**Location**: `src/events/modals/scan.rs`, `src/install/scan/pkg.rs`, `src/install/scan/spawn.rs`

### Test Scenarios

- [ ] **ClamAV scan**:
  - [ ] Enable ClamAV scan for AUR package
  - [ ] Scan executes via PTY
  - [ ] Output streams live
  - [ ] Results are displayed correctly

- [ ] **Trivy scan**:
  - [ ] Enable Trivy scan for AUR package
  - [ ] Scan executes via PTY
  - [ ] Output streams live
  - [ ] Results are displayed correctly

- [ ] **Semgrep scan**:
  - [ ] Enable Semgrep scan for AUR package
  - [ ] Scan executes via PTY
  - [ ] Output streams live
  - [ ] Results are displayed correctly

- [ ] **ShellCheck scan**:
  - [ ] Enable ShellCheck scan for AUR package
  - [ ] Scan executes via PTY
  - [ ] Output streams live
  - [ ] Results are displayed correctly

- [ ] **VirusTotal scan**:
  - [ ] Enable VirusTotal scan for AUR package
  - [ ] Scan executes via PTY
  - [ ] Output streams live
  - [ ] Results are displayed correctly

- [ ] **Custom pattern scan**:
  - [ ] Enable custom pattern scan for AUR package
  - [ ] Scan executes via PTY
  - [ ] Output streams live
  - [ ] Results are displayed correctly

- [ ] **Multiple scans**:
  - [ ] Enable multiple scan types
  - [ ] All scans execute via PTY
  - [ ] Output from all scans is visible
  - [ ] Results are properly combined

- [ ] **aur-sleuth terminal spawning**:
  - [ ] Enable aur-sleuth scan
  - [ ] aur-sleuth runs in separate terminal (not via PTY)
  - [ ] Other scans still run via PTY
  - [ ] Both processes work simultaneously

- [ ] **Scan without aur-sleuth**:
  - [ ] Run scans without aur-sleuth enabled
  - [ ] All scans execute via PTY
  - [ ] No terminal spawning occurs

- [ ] **Dry-run mode**:
  - [ ] Run scan in dry-run mode
  - [ ] Commands are shown but not executed

---

## 10. File Database Sync Fallback

**Location**: `src/events/preflight/keys/command_keys.rs`, `src/app/runtime/tick_handler.rs`

### Test Scenarios

- [ ] **Non-sudo sync success**:
  - [ ] Trigger file database sync (press 'F' in search)
  - [ ] Non-sudo `pacman -Fy` is attempted first
  - [ ] If successful, no password prompt appears
  - [ ] Sync completes successfully

- [ ] **Non-sudo sync failure with password prompt**:
  - [ ] Trigger file database sync
  - [ ] Non-sudo sync fails (permissions)
  - [ ] Password prompt appears
  - [ ] Entering password executes `sudo pacman -Fy`
  - [ ] Sync completes successfully

- [ ] **Password prompt cancellation**:
  - [ ] Trigger file database sync
  - [ ] Password prompt appears
  - [ ] Cancel the prompt
  - [ ] Operation is aborted gracefully

- [ ] **File sync result checking**:
  - [ ] After sync completes, verify result is checked
  - [ ] Success/failure is properly handled
  - [ ] UI updates correctly

---

## 11. Optional Deps Improvements

**Location**: `src/events/modals/optional_deps.rs`

### Test Scenarios

- [ ] **semgrep-bin AUR helper flow**:
  - [ ] Trigger semgrep-bin installation
  - [ ] Uses standard AUR helper flow (paru/yay)
  - [ ] Executes via PTY
  - [ ] Output streams live

- [ ] **paru installation with temp directory**:
  - [ ] Install paru as optional dependency
  - [ ] Uses temporary directory for cloning
  - [ ] Prevents accidental deletion of existing paru directory
  - [ ] Installation completes successfully

- [ ] **yay installation with temp directory**:
  - [ ] Install yay as optional dependency
  - [ ] Uses temporary directory for cloning
  - [ ] Prevents accidental deletion of existing yay directory
  - [ ] Installation completes successfully

- [ ] **Password prompt for optional deps**:
  - [ ] Install optional dependency requiring sudo
  - [ ] Password prompt appears if needed
  - [ ] Password is correctly passed

---

## 12. Downgrade Functionality

**Location**: `src/app/runtime/workers/executor.rs` (downgrade handler), `src/install/executor.rs`

### Test Scenarios

- [ ] **Downgrade with downgrade tool**:
  - [ ] Ensure `downgrade` tool is installed
  - [ ] Trigger downgrade for a package
  - [ ] Command executes via PTY
  - [ ] Interactive prompts work correctly
  - [ ] Password prompt appears if needed

- [ ] **Downgrade tool not found**:
  - [ ] Uninstall `downgrade` tool
  - [ ] Trigger downgrade
  - [ ] Error message appears: "downgrade tool not found"
  - [ ] Message suggests installing downgrade package

- [ ] **Downgrade multiple packages**:
  - [ ] Select multiple packages to downgrade
  - [ ] All packages are included in command
  - [ ] Downgrade executes correctly

- [ ] **Dry-run mode**:
  - [ ] Run downgrade in dry-run mode
  - [ ] Command is shown but not executed

- [ ] **Terminal spawning for interactive tools**:
  - [ ] If downgrade requires interactive selection
  - [ ] Terminal spawning works correctly
  - [ ] User can interact with downgrade tool

---

## 13. Preflight Modal Integration

**Location**: `src/events/preflight/keys/action_keys.rs`

### Test Scenarios

- [ ] **Install via preflight modal**:
  - [ ] Select packages and open preflight
  - [ ] Review summary
  - [ ] Confirm installation
  - [ ] Installation executes via PTY
  - [ ] Output streams live in log panel

- [ ] **Remove via preflight modal**:
  - [ ] Select packages to remove and open preflight
  - [ ] Review summary
  - [ ] Confirm removal
  - [ ] Removal executes via PTY
  - [ ] Output streams live in log panel

- [ ] **Cascade removal modes**:
  - [ ] Test Basic mode (-R)
  - [ ] Test Cascade mode (-Rs)
  - [ ] Test CascadeWithConfigs mode (-Rns)
  - [ ] Each mode executes correctly via PTY

---

## 14. Custom Command Handler

**Location**: `src/app/runtime/workers/executor.rs` (custom command handler), `src/events/modals/handlers.rs`

### Test Scenarios

- [ ] **makepkg -si command**:
  - [ ] Trigger custom command (e.g., for paru/yay installation)
  - [ ] Command executes via PTY
  - [ ] Password prompt appears if needed
  - [ ] Output streams live

- [ ] **Any sudo command**:
  - [ ] Test with various sudo commands
  - [ ] Password prompt appears
  - [ ] Password is correctly passed
  - [ ] Command executes successfully

- [ ] **Non-sudo custom command**:
  - [ ] Execute custom command without sudo
  - [ ] No password prompt appears
  - [ ] Command executes successfully

- [ ] **Dry-run mode**:
  - [ ] Run custom command in dry-run mode
  - [ ] Command is shown but not executed

---

## 15. Error Handling

### Test Scenarios

- [ ] **Command execution failure**:
  - [ ] Trigger operation that will fail (e.g., install non-existent package)
  - [ ] Error is displayed correctly
  - [ ] UI recovers gracefully
  - [ ] User can continue using the application

- [ ] **PTY creation failure**:
  - [ ] Simulate PTY creation failure (if possible)
  - [ ] Error is handled gracefully
  - [ ] User is informed of the issue

- [ ] **Password prompt timeout**:
  - [ ] Leave password prompt open for extended time
  - [ ] System handles timeout correctly

- [ ] **Network failure during operation**:
  - [ ] Disconnect network during package installation
  - [ ] Error is detected and displayed
  - [ ] Operation fails gracefully

---

## 16. Dry-Run Mode

### Test Scenarios

- [ ] **All operations respect dry-run**:
  - [ ] Install with `--dry-run`
  - [ ] Remove with `--dry-run`
  - [ ] Update with `--dry-run`
  - [ ] Scan with `--dry-run`
  - [ ] Custom command with `--dry-run`
  - [ ] All show "DRY RUN:" prefix
  - [ ] No actual system changes occur

- [ ] **Dry-run command quoting**:
  - [ ] Verify commands are properly quoted in dry-run mode
  - [ ] No syntax errors in dry-run commands
  - [ ] Special characters are escaped correctly

---

## 17. Edge Cases

### Test Scenarios

- [ ] **Empty package list**:
  - [ ] Attempt to install/remove with empty list
  - [ ] Appropriate message is shown
  - [ ] No errors occur

- [ ] **Very long package names**:
  - [ ] Install package with very long name
  - [ ] Command is properly formatted
  - [ ] Execution succeeds

- [ ] **Special characters in package names**:
  - [ ] Test with packages containing special characters
  - [ ] Commands are properly quoted
  - [ ] Execution succeeds

- [ ] **Concurrent operations**:
  - [ ] Attempt to trigger multiple operations simultaneously
  - [ ] System handles correctly (queues or prevents)
  - [ ] No race conditions

- [ ] **Large output**:
  - [ ] Trigger operation with very large output
  - [ ] Log panel handles large output correctly
  - [ ] Performance remains acceptable

---

## 18. UI/UX Verification

### Test Scenarios

- [ ] **Modal transitions**:
  - [ ] Password prompt appears/disappears smoothly
  - [ ] Loading modal transitions correctly
  - [ ] Reinstall confirmation modal works smoothly

- [ ] **Keyboard navigation**:
  - [ ] All modals are keyboard navigable
  - [ ] Tab order is logical
  - [ ] Escape key cancels appropriately

- [ ] **Visual feedback**:
  - [ ] Progress indicators are visible
  - [ ] Status messages are clear
  - [ ] Error messages are informative

- [ ] **Log panel behavior**:
  - [ ] Auto-scroll works correctly
  - [ ] Progress bars display correctly
  - [ ] Output is readable and formatted

---

## 19. Integration with Existing Features

### Test Scenarios

- [ ] **Preflight + Executor integration**:
  - [ ] Preflight summary generation works
  - [ ] Executor receives correct requests
  - [ ] Output displays in preflight exec modal

- [ ] **Search + Direct install**:
  - [ ] Search for package
  - [ ] Direct install works
  - [ ] Integration is seamless

- [ ] **Batch operations**:
  - [ ] Install multiple packages
  - [ ] Remove multiple packages
  - [ ] All operations complete successfully

---

## 20. Performance Testing

### Test Scenarios

- [ ] **Large package installation**:
  - [ ] Install many packages at once
  - [ ] Output streams without lag
  - [ ] Memory usage remains reasonable

- [ ] **Long-running operations**:
  - [ ] Run operation that takes several minutes
  - [ ] Output continues to stream
  - [ ] No memory leaks
  - [ ] UI remains responsive

---

## Test Execution Notes

### Recommended Test Order

1. Start with simple operations (single package install)
2. Progress to more complex (multiple packages, AUR)
3. Test edge cases and error conditions
4. Verify UI/UX aspects
5. Test performance with larger operations

### Test Environment

- Use a VM or test system when possible
- Have backup of important data
- Test with both `paru` and `yay` if both are available
- Test with and without sudo access

### Reporting Issues

When reporting issues, include:
- Exact steps to reproduce
- Expected behavior
- Actual behavior
- System information
- Relevant log output
- Screenshots if applicable

---

## Quick Reference: Key Files

- **Executor Worker**: `src/app/runtime/workers/executor.rs`
- **Executor Types**: `src/install/executor.rs`
- **Direct Install**: `src/install/direct.rs`
- **Password Modal**: `src/ui/modals/password.rs`
- **Loading Modal**: `src/ui/modals/misc.rs`
- **Log Panel**: `src/ui/modals/preflight_exec.rs`
- **Reinstall Confirmation**: `src/ui/modals/confirm.rs`
- **Scan Integration**: `src/events/modals/scan.rs`
- **System Update**: `src/events/modals/system_update.rs`
- **File Sync**: `src/events/preflight/keys/command_keys.rs`
- **Optional Deps**: `src/events/modals/optional_deps.rs`

---

## Completion Checklist

After completing all tests:

- [ ] All critical paths tested
- [ ] All error conditions tested
- [ ] All edge cases tested
- [ ] Performance verified
- [ ] UI/UX verified
- [ ] Documentation updated (if needed)
- [ ] Issues documented and reported

---

**Last Updated**: Based on PR `feat/integrated-process`
**Version**: 1.0

