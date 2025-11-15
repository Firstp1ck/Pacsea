//! Global shortcuts and dropdown menu handling.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::events::utils;
use crate::state::{AppState, PackageItem};
use crate::theme::reload_theme;

/// What: Handle global shortcuts plus dropdown menus and optionally stop propagation.
///
/// Inputs:
/// - `ke`: Key event received from crossterm (code + modifiers)
/// - `app`: Mutable application state shared across panes and modals
/// - `details_tx`: Channel used to request package detail refreshes
/// - `pkgb_tx`: Channel used to request PKGBUILD content for the focused result
///
/// Output:
/// - `Some(true)` when the caller should exit (e.g., global exit keybind triggered)
/// - `Some(false)` when a global keybind was handled (key should not be processed further)
/// - `None` when the key was not handled by global shortcuts
///
/// Details:
/// - Gives precedence to closing dropdown menus on `Esc` before other bindings.
/// - Routes configured global chords (help overlay, theme reload, exit, PKGBUILD toggle, sort cycle).
/// - When sort mode changes it persists the preference, re-sorts results, and refreshes details.
/// - Supports menu number shortcuts (1-9) for Options/Panels/Config dropdowns while they are open.
pub(crate) fn handle_global_key(
    ke: KeyEvent,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    pkgb_tx: &mpsc::UnboundedSender<PackageItem>,
) -> Option<bool> {
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
        if app.artix_filter_menu_open {
            app.artix_filter_menu_open = false;
        }
        return Some(false); // Handled - don't process further
    }
    let km = &app.keymap;
    // Global keybinds (only if no modal is active, except Preflight which handles its own help)
    if !matches!(app.modal, crate::state::Modal::Preflight { .. }) {
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
            return Some(false); // Handled - don't process further
        }
    }
    // Normalize BackTab so that SHIFT modifier does not affect matching across terminals
    let normalized_mods = if matches!(ke.code, KeyCode::BackTab) {
        KeyModifiers::empty()
    } else {
        ke.modifiers
    };
    let chord = (ke.code, normalized_mods);
    let matches_any =
        |list: &Vec<crate::theme::KeyChord>| list.iter().any(|c| (c.code, c.mods) == chord);
    if matches_any(&km.reload_theme) {
        match reload_theme() {
            Ok(()) => {
                app.toast_message = Some(crate::i18n::t(app, "app.toasts.theme_reloaded"));
                app.toast_expires_at =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
            }
            Err(msg) => {
                app.modal = crate::state::Modal::Alert { message: msg };
            }
        }
        return Some(false); // Handled - don't process further
    }
    if matches_any(&km.exit) {
        return Some(true); // Exit requested
    }
    // Toggle PKGBUILD viewer globally
    if matches_any(&km.show_pkgbuild) {
        if app.pkgb_visible {
            app.pkgb_visible = false;
            app.pkgb_text = None;
            app.pkgb_package_name = None;
            app.pkgb_scroll = 0;
            app.pkgb_rect = None;
        } else {
            app.pkgb_visible = true;
            app.pkgb_text = None;
            app.pkgb_package_name = None;
            if let Some(item) = app.results.get(app.selected).cloned() {
                let _ = pkgb_tx.send(item);
            }
        }
        return Some(false); // Handled - don't process further
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
        return Some(false); // Handled - don't process further
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
                        ("ghostty", "ghostty"),
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
                            // aur-sleuth (LLM audit) setup
                            let sleuth_installed = {
                                let onpath = on_path("aur-sleuth");
                                let home = std::env::var("HOME").ok();
                                let user_local = home
                                    .as_deref()
                                    .map(|h| {
                                        std::path::Path::new(h)
                                            .join(".local/bin/aur-sleuth")
                                            .exists()
                                    })
                                    .unwrap_or(false);
                                let usr_local =
                                    std::path::Path::new("/usr/local/bin/aur-sleuth").exists();
                                onpath || user_local || usr_local
                            };
                            rows.push(crate::state::types::OptionalDepRow {
                                label: "Security: aur-sleuth".to_string(),
                                package: "aur-sleuth-setup".to_string(),
                                installed: sleuth_installed,
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
            return Some(false); // Handled - don't process further
        }
        // Panels menu rows: 0 recent, 1 install, 2 keybinds
        if app.panels_menu_open {
            match idx {
                0 => {
                    app.show_recent_pane = !app.show_recent_pane;
                    if !app.show_recent_pane && matches!(app.focus, crate::state::Focus::Recent) {
                        app.focus = crate::state::Focus::Search;
                    }
                    crate::theme::save_show_recent_pane(app.show_recent_pane);
                }
                1 => {
                    app.show_install_pane = !app.show_install_pane;
                    if !app.show_install_pane && matches!(app.focus, crate::state::Focus::Install) {
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
            // Keep menu open after toggling panels
            return Some(false); // Handled - don't process further
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
                let mut names: Vec<String> = crate::index::explicit_names().into_iter().collect();
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
                    app.artix_filter_menu_open = false;
                    return Some(false); // Handled - don't process further
                }
            };
            #[cfg(target_os = "windows")]
            {
                // On Windows, use PowerShell to open file with default application
                crate::util::open_file(&target);
            }
            #[cfg(not(target_os = "windows"))]
            {
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
            }
            app.config_menu_open = false;
            app.artix_filter_menu_open = false;
            return Some(false); // Handled - don't process further
        }
    }
    None // Key not handled by global shortcuts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn new_app() -> AppState {
        AppState {
            ..Default::default()
        }
    }

    #[test]
    /// What: Confirm pressing `Esc` while dropdowns are open closes them without exiting.
    ///
    /// Inputs:
    /// - App state with Options and Sort menus flagged open.
    /// - Synthetic `Esc` key event.
    ///
    /// Output:
    /// - Handler returns `false` and menu flags reset to `false`.
    ///
    /// Details:
    /// - Ensures the early escape branch short-circuits before other global shortcuts.
    fn global_escape_closes_dropdowns() {
        let mut app = new_app();
        app.sort_menu_open = true;
        app.options_menu_open = true;
        app.panels_menu_open = true;
        app.config_menu_open = true;

        let (details_tx, _details_rx) = mpsc::unbounded_channel::<PackageItem>();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel::<PackageItem>();

        let exit = handle_global_key(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()),
            &mut app,
            &details_tx,
            &pkgb_tx,
        );

        assert_eq!(exit, Some(false));
        assert!(!app.sort_menu_open);
        assert!(!app.options_menu_open);
        assert!(!app.panels_menu_open);
        assert!(!app.config_menu_open);
    }

    #[test]
    /// What: Verify the help overlay shortcut activates the Help modal.
    ///
    /// Inputs:
    /// - Default keymap (F1 assigned to help overlay).
    /// - `F1` key event with no modifiers.
    ///
    /// Output:
    /// - Handler returns `false` and sets `app.modal` to `Modal::Help`.
    ///
    /// Details:
    /// - Confirms BackTab normalization does not interfere with regular function keys.
    fn global_help_overlay_opens_modal() {
        let mut app = new_app();
        let (details_tx, _details_rx) = mpsc::unbounded_channel::<PackageItem>();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel::<PackageItem>();

        let exit = handle_global_key(
            KeyEvent::new(KeyCode::F(1), KeyModifiers::empty()),
            &mut app,
            &details_tx,
            &pkgb_tx,
        );

        assert_eq!(exit, Some(false));
        assert!(matches!(app.modal, crate::state::Modal::Help));
    }

    #[test]
    /// What: Ensure the PKGBUILD toggle opens the viewer and requests content.
    ///
    /// Inputs:
    /// - App state with a single selected result.
    /// - `Ctrl+X` key event matching the default `show_pkgbuild` chord.
    ///
    /// Output:
    /// - Handler returns `false`, sets `pkgb_visible`, and sends the selected item through `pkgb_tx`.
    ///
    /// Details:
    /// - Provides regression coverage for the channel send branch when the viewer becomes visible.
    fn global_show_pkgbuild_requests_content() {
        let mut app = new_app();
        app.results = vec![PackageItem {
            name: "ripgrep".into(),
            version: "14.0".into(),
            description: "fast search".into(),
            source: crate::state::Source::Aur,
            popularity: None,
        }];
        app.selected = 0;

        let (details_tx, _details_rx) = mpsc::unbounded_channel::<PackageItem>();
        let (pkgb_tx, mut pkgb_rx) = mpsc::unbounded_channel::<PackageItem>();

        let exit = handle_global_key(
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
            &mut app,
            &details_tx,
            &pkgb_tx,
        );

        assert_eq!(exit, Some(false));
        assert!(app.pkgb_visible);
        let sent = pkgb_rx.try_recv().expect("pkgb request dispatched");
        assert_eq!(sent.name, "ripgrep");
    }

    #[test]
    /// What: Validate the exit key chord signals the application loop to terminate.
    ///
    /// Inputs:
    /// - Default keymap with `Ctrl+C` bound to exit.
    /// - `Ctrl+C` key event routed through the handler.
    ///
    /// Output:
    /// - Handler returns `true`, indicating the caller should stop processing events.
    ///
    /// Details:
    /// - Provides regression coverage so global exit handling keeps matching the configured chord.
    fn global_exit_chord_requests_shutdown() {
        let mut app = new_app();
        let (details_tx, _details_rx) = mpsc::unbounded_channel::<PackageItem>();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel::<PackageItem>();

        let exit = handle_global_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &mut app,
            &details_tx,
            &pkgb_tx,
        );

        assert_eq!(exit, Some(true));
    }
}
