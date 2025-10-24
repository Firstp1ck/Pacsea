//! Event handling layer for Pacsea's TUI (modularized).
//!
//! This module re-exports `handle_event` and delegates pane-specific logic
//! and mouse handling to submodules to keep files small and maintainable.

use crossterm::event::{Event as CEvent, KeyCode, KeyEventKind, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::{AppState, Focus, PackageItem, QueryInput};
use crate::theme::reload_theme;

mod install;
mod mouse;
mod recent;
mod search;
mod utils;

// re-export intentionally omitted; handled internally

/// Dispatch a single terminal event and mutate the [`AppState`].
///
/// Returns `true` to signal the application should exit; otherwise `false`.
pub fn handle_event(
    ev: CEvent,
    app: &mut AppState,
    query_tx: &mpsc::UnboundedSender<QueryInput>,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
    add_tx: &mpsc::UnboundedSender<PackageItem>,
    pkgb_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    if let CEvent::Key(ke) = ev {
        if ke.kind != KeyEventKind::Press {
            return false;
        }

        // Modal handling
        match &mut app.modal {
            crate::state::Modal::Alert { .. } => {
                match ke.code {
                    KeyCode::Enter | KeyCode::Esc => app.modal = crate::state::Modal::None,
                    _ => {}
                }
                return false;
            }
            crate::state::Modal::SystemUpdate {
                do_mirrors,
                do_pacman,
                do_aur,
                do_cache,
                country_idx,
                countries,
                cursor,
            } => {
                match ke.code {
                    KeyCode::Esc => {
                        app.modal = crate::state::Modal::None;
                    }
                    KeyCode::Up => {
                        if *cursor > 0 {
                            *cursor -= 1;
                        }
                    }
                    KeyCode::Down => {
                        let max = 4; // 4 options (0..3) + country row (index 4)
                        if *cursor < max {
                            *cursor += 1;
                        }
                    }
                    KeyCode::Left => {
                        if *cursor == 4 && !countries.is_empty() {
                            if *country_idx == 0 {
                                *country_idx = countries.len() - 1;
                            } else {
                                *country_idx -= 1;
                            }
                        }
                    }
                    KeyCode::Right => {
                        if *cursor == 4 && !countries.is_empty() {
                            *country_idx = (*country_idx + 1) % countries.len();
                        }
                    }
                    KeyCode::Char(' ') => match *cursor {
                        0 => *do_mirrors = !*do_mirrors,
                        1 => *do_pacman = !*do_pacman,
                        2 => *do_aur = !*do_aur,
                        3 => *do_cache = !*do_cache,
                        _ => {}
                    },
                    KeyCode::Enter => {
                        // Build the command lines and run in a terminal
                        let mut cmds: Vec<String> = Vec::new();
                        if *do_mirrors {
                            // Ensure reflector is installed before attempting to use it
                            cmds.push("(command -v reflector >/dev/null 2>&1 || sudo pacman -S --needed --noconfirm reflector)".to_string());
                            let country = if *country_idx < countries.len() {
                                &countries[*country_idx]
                            } else {
                                "Worldwide"
                            };
                            // For Worldwide, reflect without --country
                            if country.eq("Worldwide") {
                                cmds.push("(command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'".to_string());
                            } else {
                                cmds.push(format!("(command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --country '{country}' --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'"));
                            }
                        }
                        if *do_pacman {
                            cmds.push("sudo pacman -Syu --noconfirm".to_string());
                        }
                        if *do_aur {
                            cmds.push("(command -v paru >/dev/null 2>&1 && paru -Syu --noconfirm) || (command -v yay >/dev/null 2>&1 && yay -Syu --noconfirm) || (echo 'No AUR helper (paru/yay) found.'; echo; echo 'Choose AUR helper to install:'; echo '  1) paru'; echo '  2) yay'; echo '  3) cancel'; read -rp 'Enter 1/2/3: ' choice; case \"$choice\" in 1) git clone https://aur.archlinux.org/paru.git && cd paru && makepkg -si ;; 2) git clone https://aur.archlinux.org/yay.git && cd yay && makepkg -si ;; *) echo 'Cancelled.'; exit 1 ;; esac; if command -v paru >/dev/null 2>&1; then paru -Syu --noconfirm; elif command -v yay >/dev/null 2>&1; then yay -Syu --noconfirm; else echo 'AUR helper installation failed or was cancelled.'; exit 1; fi)".to_string());
                        }
                        if *do_cache {
                            cmds.push("sudo pacman -Sc --noconfirm".to_string());
                            cmds.push("(command -v paru >/dev/null 2>&1 && paru -Sc --noconfirm) || (command -v yay >/dev/null 2>&1 && yay -Sc --noconfirm) || true".to_string());
                        }
                        if cmds.is_empty() {
                            app.modal = crate::state::Modal::Alert {
                                message: "No actions selected".to_string(),
                            };
                        } else {
                            crate::install::spawn_shell_commands_in_terminal(&cmds);
                            app.modal = crate::state::Modal::None;
                        }
                    }
                    _ => {}
                }
                return false;
            }
            crate::state::Modal::ConfirmInstall { items } => {
                match ke.code {
                    KeyCode::Esc => {
                        app.modal = crate::state::Modal::None;
                    }
                    KeyCode::Enter => {
                        let list = items.clone();
                        app.modal = crate::state::Modal::None;
                        if list.len() <= 1 {
                            if let Some(it) = list.first() {
                                crate::install::spawn_install(it, None, app.dry_run);
                                if !app.dry_run {
                                    // Begin a short polling window to refresh installed caches
                                    app.refresh_installed_until = Some(
                                        std::time::Instant::now()
                                            + std::time::Duration::from_secs(12),
                                    );
                                    app.next_installed_refresh_at = None;
                                    app.pending_install_names = Some(vec![it.name.clone()]);
                                }
                            }
                        } else {
                            crate::install::spawn_install_all(&list, app.dry_run);
                            if !app.dry_run {
                                app.refresh_installed_until = Some(
                                    std::time::Instant::now() + std::time::Duration::from_secs(12),
                                );
                                app.next_installed_refresh_at = None;
                                app.pending_install_names =
                                    Some(list.iter().map(|p| p.name.clone()).collect());
                            }
                        }
                    }
                    _ => {}
                }
                return false;
            }
            crate::state::Modal::ConfirmRemove { items } => {
                match ke.code {
                    KeyCode::Esc | KeyCode::Enter => {
                        if ke.code == KeyCode::Enter {
                            let names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
                            if app.dry_run {
                                // Show the dry-run command and still remove from the list in UI
                                crate::install::spawn_remove_all(&names, true);
                                app.remove_list
                                    .retain(|p| !names.iter().any(|n| n == &p.name));
                                app.remove_state.select(None);
                            } else {
                                // Launch a terminal view to perform removal (non-blocking)
                                crate::install::spawn_remove_all(&names, false);
                                // Remove from remove_list in app state
                                app.remove_list
                                    .retain(|p| !names.iter().any(|n| n == &p.name));
                                app.remove_state.select(None);
                                // Begin a short polling window to refresh installed caches
                                app.refresh_installed_until = Some(
                                    std::time::Instant::now() + std::time::Duration::from_secs(8),
                                );
                                app.next_installed_refresh_at = None;
                                // Track pending removals to log after confirmation
                                app.pending_remove_names = Some(names);
                            }
                        }
                        app.modal = crate::state::Modal::None;
                    }
                    _ => {}
                }
                return false;
            }
            crate::state::Modal::Help => {
                match ke.code {
                    KeyCode::Esc | KeyCode::Enter => app.modal = crate::state::Modal::None,
                    _ => {}
                }
                return false;
            }
            crate::state::Modal::News { items, selected } => {
                match ke.code {
                    KeyCode::Esc => app.modal = crate::state::Modal::None,
                    KeyCode::Up => {
                        if *selected > 0 {
                            *selected -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if *selected + 1 < items.len() {
                            *selected += 1;
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(it) = items.get(*selected) {
                            let url = it.url.clone();
                            std::thread::spawn(move || {
                                let _ = std::process::Command::new("xdg-open")
                                    .arg(url)
                                    .stdin(std::process::Stdio::null())
                                    .stdout(std::process::Stdio::null())
                                    .stderr(std::process::Stdio::null())
                                    .spawn();
                            });
                        }
                    }
                    _ => {}
                }
                return false;
            }
            crate::state::Modal::None => {}
        }

        // Global keymap shortcuts (regardless of focus)
        // First: allow ESC to close the PKGBUILD viewer if it is open
        if ke.code == KeyCode::Esc && app.pkgb_visible {
            app.pkgb_visible = false;
            app.pkgb_text = None;
            return false;
        }

        let km = &app.keymap;
        // Normalize BackTab so that SHIFT modifier does not affect matching across terminals
        let normalized_mods = if matches!(ke.code, KeyCode::BackTab) {
            KeyModifiers::empty()
        } else {
            ke.modifiers
        };
        let chord = (ke.code, normalized_mods);
        let matches_any =
            |list: &Vec<crate::theme::KeyChord>| list.iter().any(|c| (c.code, c.mods) == chord);
        if matches_any(&km.help_overlay) {
            app.modal = crate::state::Modal::Help;
            return false;
        }
        if matches_any(&km.reload_theme) {
            match reload_theme() {
                Ok(()) => {}
                Err(msg) => {
                    app.modal = crate::state::Modal::Alert { message: msg };
                }
            }
            return false;
        }
        if matches_any(&km.exit) {
            return true;
        }
        // Toggle PKGBUILD viewer globally
        if matches_any(&km.show_pkgbuild) {
            if app.pkgb_visible {
                app.pkgb_visible = false;
                app.pkgb_text = None;
            } else {
                app.pkgb_visible = true;
                app.pkgb_text = None;
                if let Some(item) = app.results.get(app.selected).cloned() {
                    let _ = pkgb_tx.send(item);
                }
            }
            return false;
        }

        // Global: Change sorting via configured keybind
        if matches_any(&km.change_sort) {
            // Cycle through sort modes in fixed order
            app.sort_mode = match app.sort_mode {
                crate::state::SortMode::RepoThenName => {
                    crate::state::SortMode::AurPopularityThenOfficial
                }
                crate::state::SortMode::AurPopularityThenOfficial => {
                    crate::state::SortMode::BestMatches
                }
                crate::state::SortMode::BestMatches => crate::state::SortMode::RepoThenName,
            };
            // Persist preference and apply immediately
            crate::theme::save_sort_mode(app.sort_mode);
            crate::logic::sort_results_preserve_selection(app);
            // Jump selection to top and refresh details
            if !app.results.is_empty() {
                app.selected = 0;
                app.list_state.select(Some(0));
                utils::refresh_selected_details(app, details_tx);
            } else {
                app.list_state.select(None);
            }
            // Show the dropdown so the user sees the current option with a check mark
            app.sort_menu_open = true;
            app.sort_menu_auto_close_at =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
            return false;
        }

        // Recent pane focused
        if matches!(app.focus, Focus::Recent) {
            let should_exit =
                recent::handle_recent_key(ke, app, query_tx, details_tx, preview_tx, add_tx);
            return should_exit;
        }

        // Install pane focused
        if matches!(app.focus, Focus::Install) {
            let should_exit = install::handle_install_key(ke, app, details_tx, preview_tx, add_tx);
            return should_exit;
        }

        // Search pane focused (delegated)
        if matches!(app.focus, Focus::Search) {
            let should_exit =
                search::handle_search_key(ke, app, query_tx, details_tx, add_tx, preview_tx);
            return should_exit;
        }

        // Fallback: not handled
        return false;
    }

    // Mouse handling delegated
    if let CEvent::Mouse(m) = ev {
        return mouse::handle_mouse_event(m, app, details_tx, preview_tx, add_tx, pkgb_tx);
    }
    false
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    use super::*;
    use crossterm::event::{
        Event as CEvent, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    #[test]
    /// What: SystemUpdate Enter path spawns xfce4-terminal with safe args
    ///
    /// - Input: Fake xfce4-terminal on PATH; open Options->Update System, press Enter
    /// - Output: Args start with "--command" and value begins with "bash -lc "
    fn ui_options_update_system_enter_triggers_xfce4_args_shape() {
        // fake xfce4-terminal
        let mut dir: PathBuf = std::env::temp_dir();
        dir.push(format!(
            "pacsea_test_term_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);
        let mut out_path = dir.clone();
        out_path.push("args.txt");
        let mut term_path = dir.clone();
        term_path.push("xfce4-terminal");
        let script = "#!/bin/sh\n: > \"$PACSEA_TEST_OUT\"\nfor a in \"$@\"; do printf '%s\n' \"$a\" >> \"$PACSEA_TEST_OUT\"; done\n";
        fs::write(&term_path, script.as_bytes()).unwrap();
        let mut perms = fs::metadata(&term_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&term_path, perms).unwrap();
        let orig_path = std::env::var_os("PATH");
        let combined_path = match std::env::var("PATH") {
            Ok(p) => format!("{}:{}", dir.display(), p),
            Err(_) => dir.display().to_string(),
        };
        unsafe {
            std::env::set_var("PATH", combined_path);
            std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
        }

        let mut app = AppState {
            ..Default::default()
        };
        let (qtx, _qrx) = mpsc::unbounded_channel();
        let (dtx, _drx) = mpsc::unbounded_channel();
        let (ptx, _prx) = mpsc::unbounded_channel();
        let (atx, _arx) = mpsc::unbounded_channel();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel();
        app.options_button_rect = Some((5, 5, 10, 1));
        let click_options = CEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 6,
            row: 5,
            modifiers: KeyModifiers::empty(),
        });
        let _ = super::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
        assert!(app.options_menu_open);
        app.options_menu_rect = Some((5, 6, 20, 3));
        let click_menu_update = CEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 6,
            row: 7,
            modifiers: KeyModifiers::empty(),
        });
        let _ = super::handle_event(
            click_menu_update,
            &mut app,
            &qtx,
            &dtx,
            &ptx,
            &atx,
            &pkgb_tx,
        );
        let enter = CEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        let _ = super::handle_event(enter, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
        std::thread::sleep(std::time::Duration::from_millis(50));
        let body = fs::read_to_string(&out_path).expect("fake terminal args file written");
        let lines: Vec<&str> = body.lines().collect();
        assert!(lines.len() >= 2);
        assert_eq!(lines[0], "--command");
        assert!(lines[1].starts_with("bash -lc "));
        unsafe {
            if let Some(v) = orig_path {
                std::env::set_var("PATH", v);
            } else {
                std::env::remove_var("PATH");
            }
            std::env::remove_var("PACSEA_TEST_OUT");
        }
    }
}
