//! Options menu content handling (optional deps building).

use crate::state::AppState;

/// Build optional dependencies rows for the OptionalDeps modal.
///
/// What: Scan the system for installed editors, terminals, clipboard tools, mirror managers,
/// AUR helpers, and security scanners, then build a list of OptionalDepRow items for display.
///
/// Inputs:
/// - `app`: Application state (used for i18n translations)
///
/// Output:
/// - Vector of `OptionalDepRow` items ready to be displayed in the OptionalDeps modal.
///
/// Details:
/// - Editor: Shows the first installed editor found (nvim, vim, hx/helix, emacsclient/emacs, nano),
///   or all candidates if none installed. Handles helix (hx/helix) and emacs (emacs/emacsclient) aliases.
/// - Terminal: Shows the first installed terminal found, or all candidates if none installed.
/// - Clipboard: Detects KDE (Klipper), Wayland (wl-clipboard), or X11 (xclip) and shows appropriate tool.
/// - Mirrors: Detects Manjaro (pacman-mirrors), Artix (rate-mirrors), or default (reflector).
/// - AUR helper: Shows installed paru/yay if present, or both if neither installed.
/// - Security scanners: Always includes ClamAV, Trivy, Semgrep, ShellCheck, VirusTotal API setup,
///   and aur-sleuth setup. Marks installed items as non-selectable.
pub(super) fn build_optional_deps_rows(app: &AppState) -> Vec<crate::state::types::OptionalDepRow> {
    let mut rows: Vec<crate::state::types::OptionalDepRow> = Vec::new();
    let is_pkg_installed = |pkg: &str| crate::index::is_installed(pkg);
    let on_path = |cmd: &str| crate::install::command_on_path(cmd);

    // Editor: show the one installed, otherwise all possibilities
    // Map: (binary, package)
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
            label: format!(
                "{}: {}",
                crate::i18n::t(app, "app.optional_deps.categories.editor"),
                bin
            ),
            package: pkg.to_string(),
            installed: (is_pkg_installed(pkg)
                || on_path(bin)
                || ((pkg == "helix") && (on_path("hx") || on_path("helix")))
                || ((pkg == "emacs") && (on_path("emacs") || on_path("emacsclient")))),
            selectable: false,
            note: None,
        });
    } else {
        // Show unique packages (avoid hx+helix duplication)
        let mut seen = std::collections::HashSet::new();
        for (bin, pkg) in editor_candidates.iter() {
            if seen.insert(*pkg) {
                rows.push(crate::state::types::OptionalDepRow {
                    label: format!(
                        "{}: {}",
                        crate::i18n::t(app, "app.optional_deps.categories.editor"),
                        bin
                    ),
                    package: pkg.to_string(),
                    installed: (is_pkg_installed(pkg)
                        || on_path(bin)
                        || ((*pkg == "helix") && (on_path("hx") || on_path("helix")))
                        || ((*pkg == "emacs") && (on_path("emacs") || on_path("emacsclient")))),
                    selectable: !(is_pkg_installed(pkg)
                        || on_path(bin)
                        || ((*pkg == "helix") && (on_path("hx") || on_path("helix")))
                        || ((*pkg == "emacs") && (on_path("emacs") || on_path("emacsclient")))),
                    note: None,
                });
            }
        }
    }

    // Terminal: show only the one installed, otherwise all possibilities
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
            label: format!(
                "{}: {}",
                crate::i18n::t(app, "app.optional_deps.categories.terminal"),
                bin
            ),
            package: pkg.to_string(),
            installed: (is_pkg_installed(pkg) || on_path(bin)),
            selectable: false,
            note: None,
        });
    } else {
        for (bin, pkg) in term_candidates.iter() {
            rows.push(crate::state::types::OptionalDepRow {
                label: format!(
                    "{}: {}",
                    crate::i18n::t(app, "app.optional_deps.categories.terminal"),
                    bin
                ),
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
            label: format!(
                "{}: Klipper (KDE)",
                crate::i18n::t(app, "app.optional_deps.categories.clipboard")
            ),
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
                label: format!(
                    "{}: wl-clipboard",
                    crate::i18n::t(app, "app.optional_deps.categories.clipboard")
                ),
                package: pkg.to_string(),
                installed: is_pkg_installed(pkg) || on_path("wl-copy"),
                selectable: !(is_pkg_installed(pkg) || on_path("wl-copy")),
                note: Some("Wayland".to_string()),
            });
        } else {
            let pkg = "xclip";
            rows.push(crate::state::types::OptionalDepRow {
                label: format!(
                    "{}: xclip",
                    crate::i18n::t(app, "app.optional_deps.categories.clipboard")
                ),
                package: pkg.to_string(),
                installed: is_pkg_installed(pkg) || on_path("xclip"),
                selectable: !(is_pkg_installed(pkg) || on_path("xclip")),
                note: Some("X11".to_string()),
            });
        }
    }

    // Mirrors: Manjaro -> pacman-mirrors, Artix -> rate-mirrors, else reflector
    let os_release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let manjaro = os_release.contains("Manjaro");
    let artix = os_release.contains("Artix");
    if manjaro {
        let pkg = "pacman-mirrors";
        rows.push(crate::state::types::OptionalDepRow {
            label: "Mirrors: pacman-mirrors".to_string(),
            package: pkg.to_string(),
            installed: is_pkg_installed(pkg),
            selectable: !is_pkg_installed(pkg),
            note: Some("Manjaro".to_string()),
        });
    } else if artix {
        let pkg = "rate-mirrors";
        rows.push(crate::state::types::OptionalDepRow {
            label: "Mirrors: rate mirrors".to_string(),
            package: pkg.to_string(),
            installed: on_path("rate-mirrors") || is_pkg_installed(pkg),
            selectable: !(on_path("rate-mirrors") || is_pkg_installed(pkg)),
            note: Some("Artix".to_string()),
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

    // AUR helper: if one is installed show only that; else show both
    let paru_inst = on_path("paru") || is_pkg_installed("paru");
    let yay_inst = on_path("yay") || is_pkg_installed("yay");
    if paru_inst || yay_inst {
        if paru_inst {
            rows.push(crate::state::types::OptionalDepRow {
                label: format!(
                    "{}: paru",
                    crate::i18n::t(app, "app.optional_deps.categories.aur_helper")
                ),
                package: "paru".to_string(),
                installed: true,
                selectable: false,
                note: None,
            });
        } else if yay_inst {
            rows.push(crate::state::types::OptionalDepRow {
                label: format!(
                    "{}: yay",
                    crate::i18n::t(app, "app.optional_deps.categories.aur_helper")
                ),
                package: "yay".to_string(),
                installed: true,
                selectable: false,
                note: None,
            });
        }
    } else {
        rows.push(crate::state::types::OptionalDepRow {
            label: format!(
                "{}: paru",
                crate::i18n::t(app, "app.optional_deps.categories.aur_helper")
            ),
            package: "paru".to_string(),
            installed: false,
            selectable: true,
            note: Some("Install via git clone + makepkg -si".to_string()),
        });
        rows.push(crate::state::types::OptionalDepRow {
            label: format!(
                "{}: yay",
                crate::i18n::t(app, "app.optional_deps.categories.aur_helper")
            ),
            package: "yay".to_string(),
            installed: false,
            selectable: true,
            note: Some("Install via git clone + makepkg -si".to_string()),
        });
    }

    // Security scanners (after AUR helper)
    {
        // ClamAV (official)
        let pkg = "clamav";
        let installed = is_pkg_installed(pkg) || on_path("clamscan");
        rows.push(crate::state::types::OptionalDepRow {
            label: format!(
                "{}: clamav",
                crate::i18n::t(app, "app.optional_deps.categories.security")
            ),
            package: pkg.to_string(),
            installed,
            selectable: !installed,
            note: None,
        });
        // Trivy (official)
        let pkg = "trivy";
        let installed = is_pkg_installed(pkg) || on_path("trivy");
        rows.push(crate::state::types::OptionalDepRow {
            label: format!(
                "{}: trivy",
                crate::i18n::t(app, "app.optional_deps.categories.security")
            ),
            package: pkg.to_string(),
            installed,
            selectable: !installed,
            note: None,
        });
        // Semgrep (AUR: semgrep-bin)
        let pkg = "semgrep-bin";
        let installed = is_pkg_installed(pkg) || on_path("semgrep");
        rows.push(crate::state::types::OptionalDepRow {
            label: format!(
                "{}: semgrep-bin",
                crate::i18n::t(app, "app.optional_deps.categories.security")
            ),
            package: pkg.to_string(),
            installed,
            selectable: !installed,
            note: Some("AUR".to_string()),
        });
        // ShellCheck (official)
        let pkg = "shellcheck";
        let installed = is_pkg_installed(pkg) || on_path("shellcheck");
        rows.push(crate::state::types::OptionalDepRow {
            label: format!(
                "{}: shellcheck",
                crate::i18n::t(app, "app.optional_deps.categories.security")
            ),
            package: pkg.to_string(),
            installed,
            selectable: !installed,
            note: None,
        });
    }
    // VirusTotal API setup
    {
        let vt_key_present = !crate::theme::settings().virustotal_api_key.is_empty();
        rows.push(crate::state::types::OptionalDepRow {
            label: format!(
                "{}: VirusTotal API",
                crate::i18n::t(app, "app.optional_deps.categories.security")
            ),
            package: "virustotal-setup".to_string(),
            installed: vt_key_present,
            selectable: true,
            note: Some("Setup".to_string()),
        });
    }

    // aur-sleuth setup
    {
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
            let usr_local = std::path::Path::new("/usr/local/bin/aur-sleuth").exists();
            onpath || user_local || usr_local
        };
        rows.push(crate::state::types::OptionalDepRow {
            label: format!(
                "{}: aur-sleuth",
                crate::i18n::t(app, "app.optional_deps.categories.security")
            ),
            package: "aur-sleuth-setup".to_string(),
            installed: sleuth_installed,
            selectable: true,
            note: Some("Setup".to_string()),
        });
    }

    rows
}
