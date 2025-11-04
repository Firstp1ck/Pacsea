Here’s where dry-run is not implemented yet in src (i.e., actions still execute real commands instead of echoing them when app.dry_run is true):

- Pacsea/src/events/mod.rs
  - Modal::SystemUpdate (KeyCode::Enter): Builds and runs mirror update, pacman -Syyu, AUR helper -Syyu, and pacman/paru/yay -Sc commands via spawn_shell_commands_in_terminal without checking app.dry_run. These should echo DRY RUN: … when dry-run is active.
  - Modal::OptionalDeps (KeyCode::Enter): Installs optional dependencies (paru, yay, semgrep-bin, or a pacman package) through spawn_shell_commands_in_terminal with real install commands regardless of app.dry_run. Should wrap with echo DRY RUN: … when dry-run is active.
  - Modal::GnomeTerminalPrompt (KeyCode::Enter): Installs GNOME Terminal (or gnome-console/kgx) via spawn_shell_commands_in_terminal, ignoring app.dry_run. Should honor dry-run by echoing the intended install command.
