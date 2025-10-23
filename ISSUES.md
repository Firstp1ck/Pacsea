# xfce4-terminal: Unknown option: "-lc" when running System Update

## Summary
When running Options → Update System and pressing Enter with default options, launching the update in `xfce4-terminal` can fail with:

```
Unknown option: -lc
```

This occurs because `xfce4-terminal` is invoked with `-e bash -lc <cmd>`. For `xfce4-terminal`, `-e` (or `--command`) expects a single argument containing the whole command; passing multiple tokens after `-e` leads to `-lc` being interpreted as a terminal option instead of a `bash` argument.

## Reproduction (GUI)
- Open the app
- Click Options → Update System
- Press Enter to run with defaults
- If `xfce4-terminal` is chosen by the launcher, the spawned window fails and prints: `Unknown option: -lc`.

## Where it comes from (code pointers)
Terminal invocation tables include this entry for `xfce4-terminal`:
- `src/install/shell.rs`: ("xfce4-terminal", &["-e", "bash", "-lc"], false)
- `src/install/single.rs`: ("xfce4-terminal", &["-e", "bash", "-lc"], false)
- `src/install/remove.rs`: ("xfce4-terminal", &["-e", "bash", "-lc"], false)
- `src/install/batch.rs`: ("xfce4-terminal", &["-e", "bash", "-lc"], false)

By contrast, `gnome-terminal` is invoked with `--`:
- e.g. ("gnome-terminal", ["--", "bash", "-lc"], false)

## Root cause
`xfce4-terminal` treats `-e`/`--command` as taking a single argument (the command string). Using `-e` with separate tokens (`bash`, then `-lc`, then the script) leads the terminal to parse `-lc` as its own option, triggering the error.

## Impact
- System Update flow fails to open under XFCE for users where `xfce4-terminal` is selected.
- Any other flows using the same table will also misbehave when `xfce4-terminal` is chosen.

## Potential solutions
- Use end-of-options delimiter for `xfce4-terminal`:
  - Change to: ["--", "bash", "-lc"] so `-lc` is not parsed by the terminal.
- Use `--command` with a single string:
  - Example: `xfce4-terminal --command "bash -lc '<joined_cmds>'"` (requires careful quoting/escaping).
- Keep `-e` but pass a single string:
  - Example: `-e "bash -lc '<joined_cmds>'"` (works on many setups but `--command` or `--` is generally preferred).
- Provide a user setting to override terminal and args template.

## Recommendation
Adopt the `--` delimiter approach for `xfce4-terminal` across all four sites for consistency with `gnome-terminal`. Optionally add a config override for users to select their terminal template.

## Validation approach
- Existing test `ui_options_update_system_enter_triggers_xfce4_args_shape` captures current argv for `xfce4-terminal` as `-e`, `bash`, `-lc`, `<cmd>`. After fixing, it should capture `--`, `bash`, `-lc`, `<cmd>`.
- Manual: verify an update window opens and runs without the `Unknown option: -lc` error.

## Workarounds until fixed
- Install/use another terminal that appears earlier in detection (e.g., `gnome-terminal`, `kitty`, `alacritty`).
- Run the update chain manually: `bash -lc '<joined_cmds>'`.

## likely for other VTE-based terminals too. Specifically:
  - Likely affected: xfce4-terminal, mate-terminal, tilix. Their -e/--command expects a single command string; passing -e bash -lc … can misparse -lc as a terminal option.
  - Unlikely affected: gnome-terminal (we already use --), kitty (no -e), xterm (-e consumes following args correctly), alacritty (-e passes command + args), konsole (-e generally accepts program + args).

- Practical mitigation pattern:
  - For VTE-based terminals (xfce4-terminal, mate-terminal, tilix): prefer -- before bash -lc, or use --command/-e with a single string (e.g., "bash -lc '<cmd>'").
  - Keep existing forms for xterm/alacritty/konsole/kitty.