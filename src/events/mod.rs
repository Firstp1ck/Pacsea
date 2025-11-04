//! Event handling layer for Pacsea's TUI (modularized).
//!
//! This module re-exports `handle_event` and delegates pane-specific logic
//! and mouse handling to submodules to keep files small and maintainable.

use crossterm::event::{Event as CEvent, KeyCode, KeyEventKind, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::{AppState, Focus, PackageItem, QueryInput};
use crate::theme::reload_theme;

mod distro;
mod install;
mod mouse;
mod recent;
mod search;
mod utils;

// re-export intentionally omitted; handled internally

/// What: Dispatch a single terminal event (keyboard/mouse) and mutate the [`AppState`].
///
/// Inputs:
/// - `ev`: Terminal event (key or mouse)
/// - `app`: Mutable application state
/// - `query_tx`: Channel to send search queries
/// - `details_tx`: Channel to request package details
/// - `preview_tx`: Channel to request preview details for Recent
/// - `add_tx`: Channel to enqueue items into the install list
/// - `pkgb_tx`: Channel to request PKGBUILD content for the current selection
///
/// Output:
/// - `true` to signal the application should exit; otherwise `false`.
///
/// Details:
/// - Handles active modal interactions first (Alert/SystemUpdate/ConfirmInstall/ConfirmRemove/Help/News).
/// - Supports global shortcuts (help overlay, theme reload, exit, PKGBUILD viewer toggle, change sort).
/// - Delegates pane-specific handling to `search`, `recent`, and `install` submodules.
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
                mirror_count,
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
                    KeyCode::Char('-') => {
                        // Decrease mirror count when focused on the country/count row
                        if *mirror_count > 1 {
                            *mirror_count -= 1;
                            crate::theme::save_mirror_count(*mirror_count);
                        }
                    }
                    KeyCode::Char('+') => {
                        // Increase mirror count when focused on the country/count row
                        if *mirror_count < 200 {
                            *mirror_count += 1;
                            crate::theme::save_mirror_count(*mirror_count);
                        }
                    }
                    KeyCode::Enter => {
                        // Build the command lines and run in a terminal
                        let mut cmds: Vec<String> = Vec::new();
                        if *do_mirrors {
                            let sel = if *country_idx < countries.len() {
                                countries[*country_idx].as_str()
                            } else {
                                "Worldwide"
                            };
                            // Build distro-aware mirror command via helper using user settings for multi-country and count
                            let prefs = crate::theme::settings();
                            let countries_arg = if sel == "Worldwide" {
                                prefs.selected_countries.as_str()
                            } else {
                                sel
                            };
                            let count = *mirror_count;
                            // Persist selection and mirror count to settings.conf
                            crate::theme::save_selected_countries(countries_arg);
                            crate::theme::save_mirror_count(count);
                            cmds.push(distro::mirror_update_command(countries_arg, count));
                        }
                        if *do_pacman {
                            cmds.push("sudo pacman -Syyu --noconfirm".to_string());
                        }
                        if *do_aur {
                            cmds.push("(if command -v paru >/dev/null 2>&1 || sudo pacman -Qi paru >/dev/null 2>&1; then paru -Syyu --noconfirm; elif command -v yay >/dev/null 2>&1 || sudo pacman -Qi yay >/dev/null 2>&1; then yay -Syyu --noconfirm; else echo 'No AUR helper (paru/yay) found.'; echo; echo 'Choose AUR helper to install:'; echo '  1) paru'; echo '  2) yay'; echo '  3) cancel'; read -rp 'Enter 1/2/3: ' choice; case \"$choice\" in 1) git clone https://aur.archlinux.org/paru.git && cd paru && makepkg -si ;; 2) git clone https://aur.archlinux.org/yay.git && cd yay && makepkg -si ;; *) echo 'Cancelled.'; exit 1 ;; esac; if command -v paru >/dev/null 2>&1 || sudo pacman -Qi paru >/dev/null 2>&1; then paru -Syyu --noconfirm; elif command -v yay >/dev/null 2>&1 || sudo pacman -Qi yay >/dev/null 2>&1; then yay -Syyu --noconfirm; else echo 'AUR helper installation failed or was cancelled.'; exit 1; fi; fi)".to_string());
                        }
                        if *do_cache {
                            cmds.push("sudo pacman -Sc --noconfirm".to_string());
                            cmds.push("((command -v paru >/dev/null 2>&1 || sudo pacman -Qi paru >/dev/null 2>&1) && paru -Sc --noconfirm) || ((command -v yay >/dev/null 2>&1 || sudo pacman -Qi yay >/dev/null 2>&1) && yay -Sc --noconfirm) || true".to_string());
                        }
                        if cmds.is_empty() {
                            app.modal = crate::state::Modal::Alert {
                                message: "No actions selected".to_string(),
                            };
                        } else {
                            let to_run: Vec<String> = if app.dry_run {
                                cmds.iter()
                                    .map(|c| format!("echo DRY RUN: {}", c))
                                    .collect()
                            } else {
                                cmds
                            };
                            crate::install::spawn_shell_commands_in_terminal(&to_run);
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
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        // Build AUR package name list to scan
                        let list = items.clone();
                        let mut names: Vec<String> = Vec::new();
                        for it in list.iter() {
                            if matches!(it.source, crate::state::Source::Aur) {
                                names.push(it.name.clone());
                            }
                        }
                        if names.is_empty() {
                            app.modal = crate::state::Modal::Alert {
                                message: "No AUR packages selected to scan.\nSelect AUR results or add AUR packages to the Install list, then press 's'.".into(),
                            };
                        } else {
                            app.pending_install_names = Some(names);
                            // Open Scan Configuration modal initialized from settings.conf
                            let prefs = crate::theme::settings();
                            app.modal = crate::state::Modal::ScanConfig {
                                do_clamav: prefs.scan_do_clamav,
                                do_trivy: prefs.scan_do_trivy,
                                do_semgrep: prefs.scan_do_semgrep,
                                do_shellcheck: prefs.scan_do_shellcheck,
                                do_virustotal: prefs.scan_do_virustotal,
                                do_custom: prefs.scan_do_custom,
                                cursor: 0,
                            };
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
                let chord = (ke.code, ke.modifiers);
                let km = &app.keymap;
                if km.news_mark_read.iter().any(|c| (c.code, c.mods) == chord) {
                    if let Some(it) = items.get(*selected) {
                        app.news_read_urls.insert(it.url.clone());
                        app.news_read_dirty = true;
                    }
                    return false;
                }
                if km
                    .news_mark_all_read
                    .iter()
                    .any(|c| (c.code, c.mods) == chord)
                {
                    for it in items.iter() {
                        app.news_read_urls.insert(it.url.clone());
                    }
                    app.news_read_dirty = true;
                    return false;
                }
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
            crate::state::Modal::OptionalDeps { rows, selected } => {
                match ke.code {
                    KeyCode::Esc => {
                        app.modal = crate::state::Modal::None;
                    }
                    KeyCode::Up => {
                        if *selected > 0 {
                            *selected -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if *selected + 1 < rows.len() {
                            *selected += 1;
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(row) = rows.get(*selected) {
                            if row.package == "virustotal-setup" {
                                let current = crate::theme::settings().virustotal_api_key;
                                let cur_len = current.len();
                                app.modal = crate::state::Modal::VirusTotalSetup {
                                    input: current,
                                    cursor: cur_len,
                                };
                            } else if !row.installed && row.selectable {
                                let pkg = row.package.clone();
                                let cmd = if pkg == "paru" {
                                    "git clone https://aur.archlinux.org/paru.git && cd paru && makepkg -si".to_string()
                                } else if pkg == "yay" {
                                    "git clone https://aur.archlinux.org/yay.git && cd yay && makepkg -si".to_string()
                                } else if pkg == "semgrep-bin" {
                                    "git clone https://aur.archlinux.org/semgrep-bin.git && cd semgrep-bin && makepkg -si".to_string()
                                } else {
                                    format!("sudo pacman -S --needed --noconfirm {}", pkg)
                                };
                                let to_run = if app.dry_run {
                                    vec![format!("echo DRY RUN: {}", cmd)]
                                } else {
                                    vec![cmd]
                                };
                                crate::install::spawn_shell_commands_in_terminal(&to_run);
                                app.modal = crate::state::Modal::None;
                            }
                        }
                    }
                    _ => {}
                }
                return false;
            }
            crate::state::Modal::ScanConfig {
                do_clamav,
                do_trivy,
                do_semgrep,
                do_shellcheck,
                do_virustotal,
                do_custom,
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
                        if *cursor < 5 {
                            *cursor += 1;
                        }
                    }
                    KeyCode::Char(' ') => match *cursor {
                        0 => *do_clamav = !*do_clamav,
                        1 => *do_trivy = !*do_trivy,
                        2 => *do_semgrep = !*do_semgrep,
                        3 => *do_shellcheck = !*do_shellcheck,
                        4 => *do_virustotal = !*do_virustotal,
                        5 => *do_custom = !*do_custom,
                        _ => {}
                    },
                    KeyCode::Enter => {
                        tracing::info!(
                            event = "scan_config_confirm",
                            dry_run = app.dry_run,
                            do_clamav = *do_clamav,
                            do_trivy = *do_trivy,
                            do_semgrep = *do_semgrep,
                            do_shellcheck = *do_shellcheck,
                            do_virustotal = *do_virustotal,
                            do_custom = *do_custom,
                            pending_count =
                                app.pending_install_names.as_ref().map_or(0, |v| v.len()),
                            "Scan Configuration confirmed"
                        );
                        // Persist scan selection to settings.conf
                        crate::theme::save_scan_do_clamav(*do_clamav);
                        crate::theme::save_scan_do_trivy(*do_trivy);
                        crate::theme::save_scan_do_semgrep(*do_semgrep);
                        crate::theme::save_scan_do_shellcheck(*do_shellcheck);
                        crate::theme::save_scan_do_virustotal(*do_virustotal);
                        crate::theme::save_scan_do_custom(*do_custom);

                        // PACSEA_SCAN_DO_* flags are injected into the spawned terminal by spawn_aur_scan_for_with_config

                        // Spawn scans for pending names (set when opening modal)

                        #[cfg(not(target_os = "windows"))]
                        if let Some(names) = app.pending_install_names.clone() {
                            tracing::info!(
                                names = ?names,
                                count = names.len(),
                                dry_run = app.dry_run,
                                "Launching AUR scans"
                            );
                            if app.dry_run {
                                for n in names.iter() {
                                    tracing::info!(
                                        package = %n,
                                        "Dry-run: spawning AUR scan terminal"
                                    );
                                    let msg = format!(
                                        "echo DRY RUN: AUR scan {} (clamav={} trivy={} semgrep={} shellcheck={} virustotal={} custom={})",
                                        n,
                                        *do_clamav,
                                        *do_trivy,
                                        *do_semgrep,
                                        *do_shellcheck,
                                        *do_virustotal,
                                        *do_custom
                                    );
                                    crate::install::spawn_shell_commands_in_terminal(&[msg]);
                                }
                            } else {
                                for n in names.iter() {
                                    tracing::info!(
                                        package = %n,
                                        do_clamav = *do_clamav,
                                        do_trivy = *do_trivy,
                                        do_semgrep = *do_semgrep,
                                        do_shellcheck = *do_shellcheck,
                                        do_virustotal = *do_virustotal,
                                        do_custom = *do_custom,
                                        "Spawning AUR scan terminal"
                                    );
                                    crate::install::spawn_aur_scan_for_with_config(
                                        n,
                                        *do_clamav,
                                        *do_trivy,
                                        *do_semgrep,
                                        *do_shellcheck,
                                        *do_virustotal,
                                        *do_custom,
                                    );
                                }
                            }
                        } else {
                            tracing::warn!(
                                "Scan confirmed but no pending AUR package names were found"
                            );
                        }

                        app.modal = crate::state::Modal::None;
                    }
                    _ => {}
                }
                return false;
            }
            crate::state::Modal::VirusTotalSetup { input, cursor } => {
                match ke.code {
                    KeyCode::Esc => {
                        app.modal = crate::state::Modal::None;
                    }
                    KeyCode::Enter => {
                        let key = input.trim().to_string();
                        if key.is_empty() {
                            let url = "https://www.virustotal.com/gui/my-apikey";
                            std::thread::spawn(move || {
                                let _ = std::process::Command::new("xdg-open")
                                    .arg(url)
                                    .stdin(std::process::Stdio::null())
                                    .stdout(std::process::Stdio::null())
                                    .stderr(std::process::Stdio::null())
                                    .spawn();
                            });
                            // Keep the setup modal open so the user can paste the key after opening the link
                        } else {
                            crate::theme::save_virustotal_api_key(&key);
                            app.modal = crate::state::Modal::None;
                        }
                    }
                    KeyCode::Backspace => {
                        if *cursor > 0 && *cursor <= input.len() {
                            input.remove(*cursor - 1);
                            *cursor -= 1;
                        }
                    }
                    KeyCode::Left => {
                        if *cursor > 0 {
                            *cursor -= 1;
                        }
                    }
                    KeyCode::Right => {
                        if *cursor < input.len() {
                            *cursor += 1;
                        }
                    }
                    KeyCode::Home => {
                        *cursor = 0;
                    }
                    KeyCode::End => {
                        *cursor = input.len();
                    }
                    KeyCode::Char(ch) => {
                        if !ch.is_control() {
                            if *cursor <= input.len() {
                                input.insert(*cursor, ch);
                                *cursor += 1;
                            } else {
                                input.push(ch);
                                *cursor = input.len();
                            }
                        }
                    }
                    _ => {}
                }
                return false;
            }
            crate::state::Modal::GnomeTerminalPrompt => {
                match ke.code {
                    KeyCode::Enter => {
                        // Install GNOME Terminal, then close the prompt

                        let cmd = "(sudo pacman -S --needed --noconfirm gnome-terminal) || (sudo pacman -S --needed --noconfirm gnome-console) || (sudo pacman -S --needed --noconfirm kgx)".to_string();

                        if app.dry_run {
                            crate::install::spawn_shell_commands_in_terminal(&[format!(
                                "echo DRY RUN: {}",
                                cmd
                            )]);
                        } else {
                            crate::install::spawn_shell_commands_in_terminal(&[cmd]);
                        }
                        app.modal = crate::state::Modal::None;
                    }

                    KeyCode::Esc => {
                        // Warn user about potential unexpected behavior and close the prompt
                        app.toast_message = Some(
                            "Continuing without gnome-terminal may cause unexpected behavior"
                                .to_string(),
                        );
                        app.toast_expires_at =
                            Some(std::time::Instant::now() + std::time::Duration::from_secs(6));
                        app.modal = crate::state::Modal::None;
                    }
                    _ => {}
                }
                return false;
            }
            crate::state::Modal::None => {}
        }

        // If any modal remains open after handling above, consume the key to prevent main window interaction
        if !matches!(app.modal, crate::state::Modal::None) {
            return false;
        }

        // Global keymap shortcuts (regardless of focus)
        // First: allow ESC to close the PKGBUILD viewer if it is open
        // Esc does not close the PKGBUILD viewer here

        // If any dropdown is open, ESC closes it instead of changing modes
        if ke.code == KeyCode::Esc
            && (app.sort_menu_open
                || app.options_menu_open
                || app.panels_menu_open
                || app.config_menu_open)
        {
            if app.sort_menu_open {
                app.sort_menu_open = false;
                app.sort_menu_auto_close_at = None;
            }
            if app.options_menu_open {
                app.options_menu_open = false;
            }
            if app.panels_menu_open {
                app.panels_menu_open = false;
            }
            if app.config_menu_open {
                app.config_menu_open = false;
            }
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
                app.pkgb_scroll = 0;
                app.pkgb_rect = None;
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

        // Note: menu toggles (Shift+C/O/P) handled in Search Normal mode and not globally

        // Global: When a dropdown is open, allow numeric selection 1..9 to activate rows
        if let crossterm::event::KeyCode::Char(ch) = ke.code
            && ch.is_ascii_digit()
            && ch != '0'
        {
            let idx = (ch as u8 - b'1') as usize; // '1' -> 0
            // Options menu rows: 0 toggle installed-only, 1 update system, 2 news, 3 optional deps
            if app.options_menu_open {
                match idx {
                    0 => {
                        // same as mouse options handler row 0
                        if app.installed_only_mode {
                            if let Some(prev) = app.results_backup_for_toggle.take() {
                                app.all_results = prev;
                            }
                            app.installed_only_mode = false;
                            app.right_pane_focus = crate::state::RightPaneFocus::Install;
                            crate::logic::apply_filters_and_sort_preserve_selection(app);
                            utils::refresh_selected_details(app, details_tx);
                        } else {
                            app.results_backup_for_toggle = Some(app.all_results.clone());
                            let explicit = crate::index::explicit_names();
                            let mut items: Vec<crate::state::PackageItem> =
                                crate::index::all_official()
                                    .into_iter()
                                    .filter(|p| explicit.contains(&p.name))
                                    .collect();
                            use std::collections::HashSet;
                            let official_names: HashSet<String> =
                                items.iter().map(|p| p.name.clone()).collect();
                            for name in explicit.into_iter() {
                                if !official_names.contains(&name) {
                                    let is_eos = crate::index::is_eos_name(&name);
                                    let src = if is_eos {
                                        crate::state::Source::Official {
                                            repo: "EOS".to_string(),
                                            arch: String::new(),
                                        }
                                    } else {
                                        crate::state::Source::Aur
                                    };
                                    items.push(crate::state::PackageItem {
                                        name: name.clone(),
                                        version: String::new(),
                                        description: String::new(),
                                        source: src,
                                        popularity: None,
                                    });
                                }
                            }
                            app.all_results = items;
                            app.installed_only_mode = true;
                            app.right_pane_focus = crate::state::RightPaneFocus::Remove;
                            crate::logic::apply_filters_and_sort_preserve_selection(app);
                            utils::refresh_selected_details(app, details_tx);
                            let path = crate::theme::config_dir().join("installed_packages.txt");
                            let mut names: Vec<String> =
                                crate::index::explicit_names().into_iter().collect();
                            names.sort();
                            let body = names.join("\n");
                            let _ = std::fs::write(path, body);
                        }
                    }
                    1 => {
                        let countries = vec![
                            "Worldwide".to_string(),
                            "Germany".to_string(),
                            "United States".to_string(),
                            "United Kingdom".to_string(),
                            "France".to_string(),
                            "Netherlands".to_string(),
                            "Sweden".to_string(),
                            "Canada".to_string(),
                            "Australia".to_string(),
                            "Japan".to_string(),
                        ];
                        let prefs = crate::theme::settings();
                        let initial_country_idx = {
                            let sel = prefs
                                .selected_countries
                                .split(',')
                                .next()
                                .map(|s| s.trim().to_string())
                                .unwrap_or_else(|| "Worldwide".to_string());
                            countries.iter().position(|c| c == &sel).unwrap_or(0)
                        };
                        app.modal = crate::state::Modal::SystemUpdate {
                            do_mirrors: false,
                            do_pacman: true,
                            do_aur: true,
                            do_cache: false,
                            country_idx: initial_country_idx,
                            countries,
                            mirror_count: prefs.mirror_count,
                            cursor: 0,
                        };
                    }
                    2 => {
                        let (tx, rx) = std::sync::mpsc::channel();
                        std::thread::spawn(move || {
                            let rt = tokio::runtime::Builder::new_current_thread()
                                .enable_all()
                                .build();
                            let res = match rt {
                                Ok(rt) => rt.block_on(crate::sources::fetch_arch_news(10)),
                                Err(e) => {
                                    Err::<Vec<crate::state::NewsItem>, _>(format!("rt: {e}").into())
                                }
                            };
                            let _ = tx.send(res);
                        });
                        match rx.recv_timeout(std::time::Duration::from_secs(3)) {
                            Ok(Ok(list)) => {
                                app.modal = crate::state::Modal::News {
                                    items: list,
                                    selected: 0,
                                };
                            }
                            Ok(Err(e)) => {
                                app.modal = crate::state::Modal::Alert {
                                    message: format!("Failed to fetch news: {e}"),
                                };
                            }
                            Err(_) => {
                                app.modal = crate::state::Modal::Alert {
                                    message: "Timed out fetching news".to_string(),
                                };
                            }
                        }
                    }
                    3 => {
                        // Open Optional Deps modal (same as mouse handler row 3)
                        let mut rows: Vec<crate::state::types::OptionalDepRow> = Vec::new();
                        let is_pkg_installed = |pkg: &str| crate::index::is_installed(pkg);
                        let on_path = |cmd: &str| crate::install::command_on_path(cmd);
                        // (Security rows will be appended after AUR helper for desired order)
                        // Editor
                        let editor_candidates: &[(&str, &str)] = &[
                            ("nvim", "neovim"),
                            ("vim", "vim"),
                            ("hx", "helix"),
                            ("helix", "helix"),
                            ("emacsclient", "emacs"),
                            ("emacs", "emacs"),
                            ("nano", "nano"),
                        ];
                        let mut editor_installed: Option<(&str, &str)> = None;
                        for (bin, pkg) in editor_candidates.iter() {
                            if on_path(bin) || is_pkg_installed(pkg) {
                                editor_installed = Some((*bin, *pkg));
                                break;
                            }
                        }
                        if let Some((bin, pkg)) = editor_installed {
                            rows.push(crate::state::types::OptionalDepRow {
                                label: format!("Editor: {}", bin),
                                package: pkg.to_string(),
                                installed: (is_pkg_installed(pkg)
                                    || on_path(bin)
                                    || ((pkg == "helix") && (on_path("hx") || on_path("helix")))
                                    || ((pkg == "emacs")
                                        && (on_path("emacs") || on_path("emacsclient")))),
                                selectable: false,
                                note: None,
                            });
                        } else {
                            let mut seen = std::collections::HashSet::new();
                            for (bin, pkg) in editor_candidates.iter() {
                                if seen.insert(*pkg) {
                                    rows.push(crate::state::types::OptionalDepRow {
                                        label: format!("Editor: {}", bin),
                                        package: pkg.to_string(),
                                        installed: (is_pkg_installed(pkg)
                                            || on_path(bin)
                                            || ((*pkg == "helix")
                                                && (on_path("hx") || on_path("helix")))
                                            || ((*pkg == "emacs")
                                                && (on_path("emacs") || on_path("emacsclient")))),
                                        selectable: !(is_pkg_installed(pkg)
                                            || on_path(bin)
                                            || ((*pkg == "helix")
                                                && (on_path("hx") || on_path("helix")))
                                            || ((*pkg == "emacs")
                                                && (on_path("emacs") || on_path("emacsclient")))),
                                        note: None,
                                    });
                                }
                            }
                        }
                        // Terminal
                        let term_candidates: &[(&str, &str)] = &[
                            ("alacritty", "alacritty"),
                            ("kitty", "kitty"),
                            ("xterm", "xterm"),
                            ("gnome-terminal", "gnome-terminal"),
                            ("konsole", "konsole"),
                            ("xfce4-terminal", "xfce4-terminal"),
                            ("tilix", "tilix"),
                            ("mate-terminal", "mate-terminal"),
                        ];
                        let mut term_installed: Option<(&str, &str)> = None;
                        for (bin, pkg) in term_candidates.iter() {
                            if on_path(bin) || is_pkg_installed(pkg) {
                                term_installed = Some((*bin, *pkg));
                                break;
                            }
                        }
                        if let Some((bin, pkg)) = term_installed {
                            rows.push(crate::state::types::OptionalDepRow {
                                label: format!("Terminal: {}", bin),
                                package: pkg.to_string(),
                                installed: (is_pkg_installed(pkg) || on_path(bin)),
                                selectable: false,
                                note: None,
                            });
                        } else {
                            for (bin, pkg) in term_candidates.iter() {
                                rows.push(crate::state::types::OptionalDepRow {
                                    label: format!("Terminal: {}", bin),
                                    package: pkg.to_string(),
                                    installed: (is_pkg_installed(pkg) || on_path(bin)),
                                    selectable: !(is_pkg_installed(pkg) || on_path(bin)),
                                    note: None,
                                });
                            }
                        }
                        // Clipboard: Prefer Klipper when KDE session detected; else Wayland/X11 specific
                        let is_kde = std::env::var("KDE_FULL_SESSION").is_ok()
                            || std::env::var("XDG_CURRENT_DESKTOP")
                                .ok()
                                .map(|v| {
                                    let u = v.to_uppercase();
                                    u.contains("KDE") || u.contains("PLASMA")
                                })
                                .unwrap_or(false)
                            || on_path("klipper");
                        if is_kde {
                            let pkg = "plasma-workspace";
                            rows.push(crate::state::types::OptionalDepRow {
                                label: "Clipboard: Klipper (KDE)".to_string(),
                                package: pkg.to_string(),
                                installed: is_pkg_installed(pkg) || on_path("klipper"),
                                selectable: !(is_pkg_installed(pkg) || on_path("klipper")),
                                note: Some("KDE Plasma".to_string()),
                            });
                        } else {
                            let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
                            if is_wayland {
                                let pkg = "wl-clipboard";
                                rows.push(crate::state::types::OptionalDepRow {
                                    label: "Clipboard: wl-clipboard".to_string(),
                                    package: pkg.to_string(),
                                    installed: is_pkg_installed(pkg) || on_path("wl-copy"),
                                    selectable: !(is_pkg_installed(pkg) || on_path("wl-copy")),
                                    note: Some("Wayland".to_string()),
                                });
                            } else {
                                let pkg = "xclip";
                                rows.push(crate::state::types::OptionalDepRow {
                                    label: "Clipboard: xclip".to_string(),
                                    package: pkg.to_string(),
                                    installed: is_pkg_installed(pkg) || on_path("xclip"),
                                    selectable: !(is_pkg_installed(pkg) || on_path("xclip")),
                                    note: Some("X11".to_string()),
                                });
                            }
                        }
                        // Mirrors
                        let manjaro = std::fs::read_to_string("/etc/os-release")
                            .map(|s| s.contains("Manjaro"))
                            .unwrap_or(false);
                        if manjaro {
                            let pkg = "pacman-mirrors";
                            rows.push(crate::state::types::OptionalDepRow {
                                label: "Mirrors: pacman-mirrors".to_string(),
                                package: pkg.to_string(),
                                installed: is_pkg_installed(pkg),
                                selectable: !is_pkg_installed(pkg),
                                note: Some("Manjaro".to_string()),
                            });
                        } else {
                            let pkg = "reflector";
                            rows.push(crate::state::types::OptionalDepRow {
                                label: "Mirrors: reflector".to_string(),
                                package: pkg.to_string(),
                                installed: is_pkg_installed(pkg),
                                selectable: !is_pkg_installed(pkg),
                                note: None,
                            });
                        }
                        // (VirusTotal API row will be appended at the end for desired order)
                        // AUR helper
                        let paru_inst = on_path("paru") || is_pkg_installed("paru");
                        let yay_inst = on_path("yay") || is_pkg_installed("yay");
                        if paru_inst || yay_inst {
                            if paru_inst {
                                rows.push(crate::state::types::OptionalDepRow {
                                    label: "AUR helper: paru".to_string(),
                                    package: "paru".to_string(),
                                    installed: true,
                                    selectable: false,
                                    note: None,
                                });
                            } else if yay_inst {
                                rows.push(crate::state::types::OptionalDepRow {
                                    label: "AUR helper: yay".to_string(),
                                    package: "yay".to_string(),
                                    installed: true,
                                    selectable: false,
                                    note: None,
                                });
                            }
                        } else {
                            rows.push(crate::state::types::OptionalDepRow {
                                label: "AUR helper: paru".to_string(),
                                package: "paru".to_string(),
                                installed: false,
                                selectable: true,
                                note: Some("Install via git clone + makepkg -si".to_string()),
                            });
                            rows.push(crate::state::types::OptionalDepRow {
                                label: "AUR helper: yay".to_string(),
                                package: "yay".to_string(),
                                installed: false,
                                selectable: true,
                                note: Some("Install via git clone + makepkg -si".to_string()),
                            });
                        }
                        // Append Security rows after AUR helper
                        {
                            // Security: clamav (official)
                            let pkg = "clamav";
                            let installed = is_pkg_installed(pkg) || on_path("clamscan");
                            rows.push(crate::state::types::OptionalDepRow {
                                label: "Security: clamav".to_string(),
                                package: pkg.to_string(),
                                installed,
                                selectable: !installed,
                                note: None,
                            });
                            // Security: trivy (official)
                            let pkg = "trivy";
                            let installed = is_pkg_installed(pkg) || on_path("trivy");
                            rows.push(crate::state::types::OptionalDepRow {
                                label: "Security: trivy".to_string(),
                                package: pkg.to_string(),
                                installed,
                                selectable: !installed,
                                note: None,
                            });
                            // Security: semgrep-bin (AUR)
                            let pkg = "semgrep-bin";
                            let installed = is_pkg_installed(pkg) || on_path("semgrep");
                            rows.push(crate::state::types::OptionalDepRow {
                                label: "Security: semgrep-bin".to_string(),
                                package: pkg.to_string(),
                                installed,
                                selectable: !installed,
                                note: Some("AUR".to_string()),
                            });
                            // Security: shellcheck (official)
                            let pkg = "shellcheck";
                            let installed = is_pkg_installed(pkg) || on_path("shellcheck");
                            rows.push(crate::state::types::OptionalDepRow {
                                label: "Security: shellcheck".to_string(),
                                package: pkg.to_string(),
                                installed,
                                selectable: !installed,
                                note: None,
                            });
                            // Security: VirusTotal API (Setup)
                            {
                                let vt_key_present =
                                    !crate::theme::settings().virustotal_api_key.is_empty();
                                rows.push(crate::state::types::OptionalDepRow {
                                    label: "Security: VirusTotal API".to_string(),
                                    package: "virustotal-setup".to_string(),
                                    installed: vt_key_present,
                                    selectable: true,
                                    note: Some("Setup".to_string()),
                                });
                            }
                        }
                        app.modal = crate::state::Modal::OptionalDeps { rows, selected: 0 };
                    }
                    _ => {}
                }
                app.options_menu_open = false;
                return false;
            }
            // Panels menu rows: 0 recent, 1 install, 2 keybinds
            if app.panels_menu_open {
                match idx {
                    0 => {
                        app.show_recent_pane = !app.show_recent_pane;
                        if !app.show_recent_pane && matches!(app.focus, crate::state::Focus::Recent)
                        {
                            app.focus = crate::state::Focus::Search;
                        }
                        crate::theme::save_show_recent_pane(app.show_recent_pane);
                    }
                    1 => {
                        app.show_install_pane = !app.show_install_pane;
                        if !app.show_install_pane
                            && matches!(app.focus, crate::state::Focus::Install)
                        {
                            app.focus = crate::state::Focus::Search;
                        }
                        crate::theme::save_show_install_pane(app.show_install_pane);
                    }
                    2 => {
                        app.show_keybinds_footer = !app.show_keybinds_footer;
                        crate::theme::save_show_keybinds_footer(app.show_keybinds_footer);
                    }
                    _ => {}
                }
                app.panels_menu_open = false;
                return false;
            }
            // Config menu rows: 0 settings, 1 theme, 2 keybinds, 3 install list, 4 installed list, 5 recent
            if app.config_menu_open {
                let settings_path = crate::theme::config_dir().join("settings.conf");
                let theme_path = crate::theme::config_dir().join("theme.conf");
                let keybinds_path = crate::theme::config_dir().join("keybinds.conf");
                let install_path = app.install_path.clone();
                let recent_path = app.recent_path.clone();
                let installed_list_path = crate::theme::config_dir().join("installed_packages.txt");
                if idx == 4 {
                    let mut names: Vec<String> =
                        crate::index::explicit_names().into_iter().collect();
                    names.sort();
                    let body = names.join("\n");
                    let _ = std::fs::write(&installed_list_path, body);
                }
                let target = match idx {
                    0 => settings_path,
                    1 => theme_path,
                    2 => keybinds_path,
                    3 => install_path,
                    4 => installed_list_path,
                    5 => recent_path,
                    _ => {
                        app.config_menu_open = false;
                        return false;
                    }
                };
                let path_str = target.display().to_string();
                let editor_cmd = format!(
                    "((command -v nvim >/dev/null 2>&1 || sudo pacman -Qi neovim >/dev/null 2>&1) && nvim '{path_str}') || \\
                     ((command -v vim >/dev/null 2>&1 || sudo pacman -Qi vim >/dev/null 2>&1) && vim '{path_str}') || \\
                     ((command -v hx >/dev/null 2>&1 || sudo pacman -Qi helix >/dev/null 2>&1) && hx '{path_str}') || \\
                     ((command -v helix >/dev/null 2>&1 || sudo pacman -Qi helix >/dev/null 2>&1) && helix '{path_str}') || \\
                     ((command -v emacsclient >/dev/null 2>&1 || sudo pacman -Qi emacs >/dev/null 2>&1) && emacsclient -t '{path_str}') || \\
                     ((command -v emacs >/dev/null 2>&1 || sudo pacman -Qi emacs >/dev/null 2>&1) && emacs -nw '{path_str}') || \\
                     ((command -v nano >/dev/null 2>&1 || sudo pacman -Qi nano >/dev/null 2>&1) && nano '{path_str}') || \\
                     (echo 'No terminal editor found (nvim/vim/emacsclient/emacs/hx/helix/nano).'; echo 'File: {path_str}'; read -rn1 -s _ || true)",
                );
                let cmds = vec![editor_cmd];
                std::thread::spawn(move || {
                    crate::install::spawn_shell_commands_in_terminal(&cmds);
                });
                app.config_menu_open = false;
                return false;
            }
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

    #[test]
    /// What: Optional Deps shows only installed editor/terminal, X11 clipboard, reflector, and both AUR helpers when none installed
    ///
    /// - Setup: Fake PATH with nvim and kitty present; ensure X11 (no WAYLAND_DISPLAY)
    /// - Expect: Rows include:
    ///   - Editor: nvim (installed, not selectable)
    ///   - Terminal: kitty (installed, not selectable)
    ///   - Clipboard: xclip (not installed, selectable)
    ///   - Mirrors: reflector (not installed, selectable)
    ///   - AUR helper: paru and yay (both not installed, selectable)
    fn optional_deps_rows_reflect_installed_and_x11_and_reflector() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;

        // Create a temp directory with fake executables for editor and terminal
        let mut dir: PathBuf = std::env::temp_dir();
        dir.push(format!(
            "pacsea_test_optional_deps_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);

        // Helpers to create executable stubs
        let make_exec = |name: &str| {
            let mut p = dir.clone();
            p.push(name);
            fs::write(&p, b"#!/bin/sh\nexit 0\n").unwrap();
            let mut perms = fs::metadata(&p).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&p, perms).unwrap();
        };

        // Present nvim and kitty on PATH
        make_exec("nvim");
        make_exec("kitty");

        // Save and override PATH for deterministic detection; ensure X11 by clearing WAYLAND_DISPLAY
        let orig_path = std::env::var_os("PATH");
        unsafe { std::env::set_var("PATH", dir.display().to_string()) };
        let orig_wl = std::env::var_os("WAYLAND_DISPLAY");
        unsafe { std::env::remove_var("WAYLAND_DISPLAY") };

        // Drive the event handler: open Options then press '4' to open Optional Deps
        let mut app = AppState {
            ..Default::default()
        };
        let (qtx, _qrx) = mpsc::unbounded_channel();
        let (dtx, _drx) = mpsc::unbounded_channel();
        let (ptx, _prx) = mpsc::unbounded_channel();
        let (atx, _arx) = mpsc::unbounded_channel();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel();

        // Open Options via click
        app.options_button_rect = Some((5, 5, 12, 1));
        let click_options = CEvent::Mouse(crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 6,
            row: 5,
            modifiers: KeyModifiers::empty(),
        });
        let _ = super::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
        assert!(app.options_menu_open);

        // Press '4' (row index 3) to open Optional Deps
        let key_four = CEvent::Key(crossterm::event::KeyEvent::new(
            KeyCode::Char('4'),
            KeyModifiers::empty(),
        ));
        let _ = super::handle_event(key_four, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);

        match &app.modal {
            crate::state::Modal::OptionalDeps { rows, .. } => {
                // Find helper to locate row by label prefix
                let find = |prefix: &str| rows.iter().find(|r| r.label.starts_with(prefix));

                // Editor: nvim
                let ed = find("Editor: nvim").expect("editor row nvim");
                assert!(ed.installed, "nvim should be marked installed");
                assert!(!ed.selectable, "installed editor should not be selectable");

                // Terminal: kitty
                let term = find("Terminal: kitty").expect("terminal row kitty");
                assert!(term.installed, "kitty should be marked installed");
                assert!(
                    !term.selectable,
                    "installed terminal should not be selectable"
                );

                // Clipboard: xclip (X11)
                let clip = find("Clipboard: xclip").expect("clipboard xclip row");
                assert!(
                    !clip.installed,
                    "xclip should not appear installed by default"
                );
                assert!(
                    clip.selectable,
                    "xclip should be selectable when not installed"
                );
                assert_eq!(clip.note.as_deref(), Some("X11"));

                // Mirrors: reflector (non-Manjaro default)
                let mirrors = find("Mirrors: reflector").expect("reflector row");
                assert!(
                    !mirrors.installed,
                    "reflector should not be installed by default"
                );
                assert!(mirrors.selectable, "reflector should be selectable");

                // AUR helper: both paru and yay should be present and selectable when not installed
                let paru = find("AUR helper: paru").expect("paru row");
                assert!(!paru.installed);
                assert!(paru.selectable);
                let yay = find("AUR helper: yay").expect("yay row");
                assert!(!yay.installed);
                assert!(yay.selectable);
            }
            other => panic!("Expected OptionalDeps modal, got {:?}", other),
        }

        // Restore environment
        unsafe {
            if let Some(v) = orig_path {
                std::env::set_var("PATH", v);
            } else {
                std::env::remove_var("PATH");
            }
            if let Some(v) = orig_wl {
                std::env::set_var("WAYLAND_DISPLAY", v);
            } else {
                std::env::remove_var("WAYLAND_DISPLAY");
            }
        }

        // Cleanup temp dir
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    /// What: Optional Deps shows Wayland clipboard (wl-clipboard) when WAYLAND_DISPLAY is set
    ///
    /// - Setup: Empty PATH; set WAYLAND_DISPLAY
    /// - Expect: A row "Clipboard: wl-clipboard" with note "Wayland", not installed and selectable
    fn optional_deps_rows_wayland_shows_wl_clipboard() {
        use std::fs;
        use std::path::PathBuf;

        // Temp PATH directory (empty)
        let mut dir: PathBuf = std::env::temp_dir();
        dir.push(format!(
            "pacsea_test_optional_deps_wl_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);

        let orig_path = std::env::var_os("PATH");
        unsafe { std::env::set_var("PATH", dir.display().to_string()) };
        let orig_wl = std::env::var_os("WAYLAND_DISPLAY");
        unsafe { std::env::set_var("WAYLAND_DISPLAY", "1") };

        let mut app = AppState {
            ..Default::default()
        };
        let (qtx, _qrx) = mpsc::unbounded_channel();
        let (dtx, _drx) = mpsc::unbounded_channel();
        let (ptx, _prx) = mpsc::unbounded_channel();
        let (atx, _arx) = mpsc::unbounded_channel();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel();

        // Open Options via click
        app.options_button_rect = Some((5, 5, 12, 1));
        let click_options = CEvent::Mouse(crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: 6,
            row: 5,
            modifiers: KeyModifiers::empty(),
        });
        let _ = super::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
        assert!(app.options_menu_open);

        // Press '4' to open Optional Deps
        let key_four = CEvent::Key(crossterm::event::KeyEvent::new(
            KeyCode::Char('4'),
            KeyModifiers::empty(),
        ));
        let _ = super::handle_event(key_four, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);

        match &app.modal {
            crate::state::Modal::OptionalDeps { rows, .. } => {
                let clip = rows
                    .iter()
                    .find(|r| r.label.starts_with("Clipboard: wl-clipboard"))
                    .expect("wl-clipboard row");
                assert_eq!(clip.note.as_deref(), Some("Wayland"));
                assert!(!clip.installed);
                assert!(clip.selectable);
                // Ensure xclip is not presented when Wayland is active
                assert!(
                    !rows.iter().any(|r| r.label.starts_with("Clipboard: xclip")),
                    "xclip should not be listed on Wayland"
                );
            }
            other => panic!("Expected OptionalDeps modal, got {:?}", other),
        }

        // Restore env and cleanup
        unsafe {
            if let Some(v) = orig_path {
                std::env::set_var("PATH", v);
            } else {
                std::env::remove_var("PATH");
            }
            if let Some(v) = orig_wl {
                std::env::set_var("WAYLAND_DISPLAY", v);
            } else {
                std::env::remove_var("WAYLAND_DISPLAY");
            }
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
