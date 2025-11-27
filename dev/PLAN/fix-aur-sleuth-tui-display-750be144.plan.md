<!-- 750be144-fb83-4330-bc7b-f9294e5e54dc 5273b63d-3efb-46cd-8582-465b7f5bb365 -->
# Fix aur-sleuth TUI Display Issue

## Problem

When running security scans with `aur-sleuth`, the TUI doesn't display and the scan appears to hang. The issue is in `src/install/scan/common.rs` where `aur-sleuth` is executed with:

- `--output plain` flag forcing plain text output instead of TUI
- `| tee ./.pacsea_sleuth.txt` pipe that redirects stdout, preventing TUI applications from detecting a TTY

## Solution

Modify the `add_sleuth_scan` function in `src/install/scan/common.rs` to:

1. Remove the `--output plain` flag to allow aur-sleuth to use its default TUI mode
2. Remove the `tee` pipe to allow direct terminal control for the TUI
3. Ensure the command runs interactively so the TUI can display properly

## Changes Required

### File: `src/install/scan/common.rs`

- **Function**: `add_sleuth_scan` (line 99)
- **Change**: Modify the command string to remove `--output plain` and the `| tee ./.pacsea_sleuth.txt` pipe
- The command should run `aur-sleuth` directly with `--pkgdir .` without output redirection

## Implementation Details

- The aur-sleuth command will run in interactive mode, allowing its TUI to display
- Users will be able to see the scan progress and interact with the TUI
- The scan will no longer appear to hang since the TUI will be visible

## Testing

- Run a security scan with aur-sleuth enabled
- Verify the TUI displays correctly in the spawned terminal
- Confirm the scan completes and is visible to the user

### To-dos

- [ ] Modify add_sleuth_scan function to remove --output plain flag and tee pipe, allowing TUI to display
- [ ] Test that aur-sleuth TUI displays correctly when running security scans