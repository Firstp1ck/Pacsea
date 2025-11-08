//! Modal event handling (excluding Preflight which is in preflight.rs).

use crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc;

use crate::events::distro;
use crate::state::{AppState, PackageItem};

/// Handle key events for all modals except Preflight.
/// Returns true if the event was handled and should stop propagation.
pub(crate) fn handle_modal_key(
    ke: KeyEvent,
    app: &mut AppState,
    add_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    match &mut app.modal {
        crate::state::Modal::Alert { message } => {
            let is_help = message.contains("Help") || message.contains("Tab Help");
            match ke.code {
                KeyCode::Enter | KeyCode::Esc => {
                    if is_help {
                        app.help_scroll = 0; // Reset scroll when closing
                    }
                    // Restore previous modal if it was Preflight, otherwise close
                    if let Some(prev_modal) = app.previous_modal.take() {
                        app.modal = prev_modal;
                    } else {
                        app.modal = crate::state::Modal::None;
                    }
                }
                KeyCode::Up if is_help => {
                    app.help_scroll = app.help_scroll.saturating_sub(1);
                }
                KeyCode::Down if is_help => {
                    app.help_scroll = app.help_scroll.saturating_add(1);
                }
                _ => {}
            }
            return false;
        }
        crate::state::Modal::PreflightExec {
            verbose,
            log_lines: _,
            abortable,
            items,
            ..
        } => {
            match ke.code {
                KeyCode::Esc | KeyCode::Char('q') => app.modal = crate::state::Modal::None,
                KeyCode::Enter => {
                    // Compute real counts best-effort and show summary
                    let data = crate::logic::compute_post_summary(items);
                    app.modal = crate::state::Modal::PostSummary {
                        success: data.success,
                        changed_files: data.changed_files,
                        pacnew_count: data.pacnew_count,
                        pacsave_count: data.pacsave_count,
                        services_pending: data.services_pending,
                        snapshot_label: data.snapshot_label,
                    };
                }
                KeyCode::Char('l') => {
                    *verbose = !*verbose;
                    app.toast_message =
                        Some(format!("Verbose: {}", if *verbose { "ON" } else { "OFF" }));
                }
                KeyCode::Char('x') => {
                    if *abortable {
                        app.toast_message = Some("Abort requested (placeholder)".to_string());
                    }
                }
                _ => {}
            }
            return false;
        }
        crate::state::Modal::PostSummary {
            success: _,
            changed_files: _,
            pacnew_count: _,
            pacsave_count: _,
            services_pending,
            snapshot_label: _,
        } => {
            match ke.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                    app.modal = crate::state::Modal::None
                }
                KeyCode::Char('r') => {
                    app.toast_message = Some("Rollback (placeholder)".to_string());
                }
                KeyCode::Char('s') => {
                    if services_pending.is_empty() {
                        app.toast_message = Some("No services to restart".to_string());
                    } else {
                        app.toast_message = Some("Restart services (placeholder)".to_string());
                    }
                }
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
                        cmds.push("(if command -v paru >/dev/null 2>&1 || sudo pacman -Qi paru >/dev/null 2>&1; then paru -Syyu --noconfirm; elif command -v yay >/dev/null 2>&1 || sudo pacman -Qi yay >/dev/null 2>&1; then yay -Syyu --noconfirm; else echo 'No AUR helper (paru/yay) found.'; echo; echo 'Choose AUR helper to install:'; echo '  1) paru'; echo '  2) yay'; echo '  3) cancel'; read -rp 'Enter 1/2/3: ' choice; case \"$choice\" in 1) rm -rf paru && git clone https://aur.archlinux.org/paru.git && cd paru && makepkg -si ;; 2) rm -rf yay && git clone https://aur.archlinux.org/yay.git && cd yay && makepkg -si ;; *) echo 'Cancelled.'; exit 1 ;; esac; if command -v paru >/dev/null 2>&1 || sudo pacman -Qi paru >/dev/null 2>&1; then paru -Syyu --noconfirm; elif command -v yay >/dev/null 2>&1 || sudo pacman -Qi yay >/dev/null 2>&1; then yay -Syyu --noconfirm; else echo 'AUR helper installation failed or was cancelled.'; exit 1; fi; fi)".to_string());
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
                                    std::time::Instant::now() + std::time::Duration::from_secs(12),
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
                            do_sleuth: prefs.scan_do_sleuth,
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
                            app.refresh_installed_until =
                                Some(std::time::Instant::now() + std::time::Duration::from_secs(8));
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
                KeyCode::Esc => {
                    app.modal = crate::state::Modal::None;
                    return true; // Stop propagation to prevent global Esc handler from running
                }
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
                        crate::util::open_url(&it.url);
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
                        } else if row.package == "aur-sleuth-setup" {
                            let cmd = r##"(set -e
            if ! command -v aur-sleuth >/dev/null 2>&1; then
            echo "aur-sleuth not found."
            echo
            echo "Install aur-sleuth:"
            echo "  1) system (/usr/local) requires sudo"
            echo "  2) user (~/.local)"
            echo "  3) cancel"
            read -rp "Choose [1/2/3]: " choice
            case "$choice" in
            1)
            tmp="$(mktemp -d)"; cd "$tmp"
            git clone https://github.com/mgalgs/aur-sleuth.git
            cd aur-sleuth
            sudo make install
            ;;
            2)
            tmp="$(mktemp -d)"; cd "$tmp"
            git clone https://github.com/mgalgs/aur-sleuth.git
            cd aur-sleuth
            make install PREFIX="$HOME/.local"
            ;;
            *)
            echo "Cancelled."; echo "Press any key to close..."; read -rn1 -s _; exit 0;;
            esac
            else
            echo "aur-sleuth already installed; continuing to setup"
            fi
            conf="${XDG_CONFIG_HOME:-$HOME/.config}/aur-sleuth.conf"
            mkdir -p "$(dirname "$conf")"
            echo "# aur-sleuth configuration" > "$conf"
            echo "[default]" >> "$conf"
            read -rp "OPENAI_BASE_URL (e.g. https://openrouter.ai/api/v1 or http://localhost:11434/v1): " base
            read -rp "OPENAI_MODEL (e.g. qwen/qwen3-30b-a3b-instruct-2507 or llama3.1:8b): " model
            read -rp "OPENAI_API_KEY: " key
            read -rp "MAX_LLM_JOBS (default 3): " jobs
            read -rp "AUDIT_FAILURE_FATAL (true/false) [true]: " fatal
            jobs=${jobs:-3}
            fatal=${fatal:-true}
            [ -n "$base" ] && echo "OPENAI_BASE_URL = $base" >> "$conf"
            [ -n "$model" ] && echo "OPENAI_MODEL = $model" >> "$conf"
            echo "OPENAI_API_KEY = $key" >> "$conf"
            echo "MAX_LLM_JOBS = $jobs" >> "$conf"
            echo "AUDIT_FAILURE_FATAL = $fatal" >> "$conf"
            echo; echo "Wrote $conf"
            echo "Tip: You can run 'aur-sleuth package-name' or audit a local pkgdir with '--pkgdir .'"
            echo; echo "Press any key to close..."; read -rn1 -s _)"##
        .to_string();
                            let to_run = if app.dry_run {
                                vec![format!("echo DRY RUN: {}", cmd)]
                            } else {
                                vec![cmd]
                            };
                            crate::install::spawn_shell_commands_in_terminal(&to_run);
                            app.modal = crate::state::Modal::None;
                        } else if !row.installed && row.selectable {
                            let pkg = row.package.clone();
                            let cmd = if pkg == "paru" {
                                "rm -rf paru && git clone https://aur.archlinux.org/paru.git && cd paru && makepkg -si".to_string()
                            } else if pkg == "yay" {
                                "rm -rf yay && git clone https://aur.archlinux.org/yay.git && cd yay && makepkg -si".to_string()
                            } else if pkg == "semgrep-bin" {
                                "rm -rf semgrep-bin && git clone https://aur.archlinux.org/semgrep-bin.git && cd semgrep-bin && makepkg -si".to_string()
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
            do_sleuth,
            cursor,
        } => {
            match ke.code {
                KeyCode::Esc => {
                    // Restore previous modal if it was Preflight, otherwise close
                    if let Some(prev_modal) = app.previous_modal.take() {
                        app.modal = prev_modal;
                    } else {
                        app.modal = crate::state::Modal::None;
                    }
                }
                KeyCode::Up => {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
                KeyCode::Down => {
                    if *cursor < 6 {
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
                    6 => *do_sleuth = !*do_sleuth,
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
                        pending_count = app.pending_install_names.as_ref().map_or(0, |v| v.len()),
                        "Scan Configuration confirmed"
                    );
                    // Persist scan selection to settings.conf
                    crate::theme::save_scan_do_clamav(*do_clamav);
                    crate::theme::save_scan_do_trivy(*do_trivy);
                    crate::theme::save_scan_do_semgrep(*do_semgrep);
                    crate::theme::save_scan_do_shellcheck(*do_shellcheck);
                    crate::theme::save_scan_do_virustotal(*do_virustotal);
                    crate::theme::save_scan_do_custom(*do_custom);
                    crate::theme::save_scan_do_sleuth(*do_sleuth);

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
                                    "echo DRY RUN: AUR scan {} (clamav={} trivy={} semgrep={} shellcheck={} virustotal={} custom={} sleuth={})",
                                    n,
                                    *do_clamav,
                                    *do_trivy,
                                    *do_semgrep,
                                    *do_shellcheck,
                                    *do_virustotal,
                                    *do_custom,
                                    *do_sleuth
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
                                    *do_sleuth,
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
        crate::state::Modal::ImportHelp => {
            match ke.code {
                KeyCode::Enter => {
                    tracing::info!("import: Enter pressed in ImportHelp modal");
                    app.modal = crate::state::Modal::None;
                    // Trigger import file picker immediately (executed in background thread)
                    let add_tx_clone = add_tx.clone();
                    std::thread::spawn(move || {
                        tracing::info!("import: thread started, opening file picker");
                        #[cfg(target_os = "windows")]
                        let path_opt: Option<String> = {
                            let script = r#"
        Add-Type -AssemblyName System.Windows.Forms
        $ofd = New-Object System.Windows.Forms.OpenFileDialog
        $ofd.Filter = 'Text Files (*.txt)|*.txt|All Files (*.*)|*.*'
        $ofd.Multiselect = $false
        if ($ofd.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { Write-Output $ofd.FileName }
        "#;
                            let output = std::process::Command::new("powershell")
                                .args(["-NoProfile", "-Command", script])
                                .stdin(std::process::Stdio::null())
                                .output()
                                .ok();
                            output.and_then(|o| {
                                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                                if s.is_empty() { None } else { Some(s) }
                            })
                        };

                        #[cfg(not(target_os = "windows"))]
                        let path_opt: Option<String> = {
                            // Try zenity, then kdialog; else fall back to reading a default file path
                            let try_cmd = |prog: &str, args: &[&str]| -> Option<String> {
                                tracing::debug!(prog = %prog, "import: trying file picker");
                                let res = std::process::Command::new(prog)
                                    .args(args)
                                    .stdin(std::process::Stdio::null())
                                    .output()
                                    .ok()?;
                                // zenity/kdialog return non-zero exit code when cancelled
                                // Check stdout content - non-empty means file was selected
                                let s = String::from_utf8_lossy(&res.stdout).trim().to_string();
                                if s.is_empty() {
                                    tracing::debug!(prog = %prog, "import: file picker returned empty");
                                    None
                                } else {
                                    tracing::debug!(prog = %prog, path = %s, "import: file picker returned path");
                                    Some(s)
                                }
                            };
                            try_cmd(
                                "zenity",
                                &[
                                    "--file-selection",
                                    "--title=Import packages",
                                    "--file-filter=*.txt",
                                ],
                            )
                            .or_else(|| {
                                tracing::debug!("import: zenity failed, trying kdialog");
                                try_cmd("kdialog", &["--getopenfilename", ".", "*.txt"])
                            })
                        };

                        if let Some(path) = path_opt {
                            let path = path.trim().to_string();
                            tracing::info!(path = %path, "import: selected file");
                            if let Ok(body) = std::fs::read_to_string(&path) {
                                use std::collections::HashSet;
                                let mut official_names: HashSet<String> = HashSet::new();
                                for it in crate::index::all_official().iter() {
                                    official_names.insert(it.name.to_lowercase());
                                }
                                let mut imported: usize = 0;
                                for line in body.lines() {
                                    let name = line.trim();
                                    if name.is_empty() || name.starts_with('#') {
                                        continue;
                                    }
                                    let src = if official_names.contains(&name.to_lowercase()) {
                                        crate::state::Source::Official {
                                            repo: String::new(),
                                            arch: String::new(),
                                        }
                                    } else {
                                        crate::state::Source::Aur
                                    };
                                    let item = crate::state::PackageItem {
                                        name: name.to_string(),
                                        version: String::new(),
                                        description: String::new(),
                                        source: src,
                                        popularity: None,
                                    };
                                    let _ = add_tx_clone.send(item);
                                    imported += 1;
                                }
                                tracing::info!(path = %path, imported, "import: queued items from list");
                            } else {
                                tracing::warn!(path = %path, "import: failed to read file");
                            }
                        } else {
                            tracing::info!("import: canceled by user");
                        }
                    });
                }
                KeyCode::Esc => app.modal = crate::state::Modal::None,
                _ => {}
            }
            return false;
        }
        crate::state::Modal::None => {}
        crate::state::Modal::Preflight { .. } => {
            // Preflight is handled separately in preflight.rs
            return false;
        }
    }
    false
}
