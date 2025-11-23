//! Options menu content handling (optional deps building).

use crate::state::AppState;

/// What: Check if a tool is installed either as a package or available on PATH.
///
/// Inputs:
/// - `pkg`: Package name to check
/// - `bin`: Binary name to check on PATH
///
/// Output:
/// - `true` if the tool is installed or available on PATH
///
/// Details:
/// - Checks both package installation and PATH availability
#[allow(clippy::missing_const_for_fn)]
fn is_tool_installed(pkg: &str, bin: &str) -> bool {
    crate::index::is_installed(pkg) || crate::install::command_on_path(bin)
}

/// What: Create an `OptionalDepRow` with standard fields.
///
/// Inputs:
/// - `app`: Application state for i18n
/// - `category_key`: i18n key for the category
/// - `label_suffix`: Suffix to append to category label
/// - `package`: Package name
/// - `installed`: Whether the tool is installed
/// - `note`: Optional note string
///
/// Output:
/// - An `OptionalDepRow` with the specified fields
///
/// Details:
/// - Sets `selectable` to `!installed` (only selectable if not installed)
fn create_optional_dep_row(
    app: &AppState,
    category_key: &str,
    label_suffix: &str,
    package: String,
    installed: bool,
    note: Option<String>,
) -> crate::state::types::OptionalDepRow {
    crate::state::types::OptionalDepRow {
        label: format!("{}: {label_suffix}", crate::i18n::t(app, category_key)),
        package,
        installed,
        selectable: !installed,
        note,
    }
}

/// What: Find the first installed candidate from a list of (binary, package) pairs.
///
/// Inputs:
/// - `candidates`: Slice of (`binary_name`, `package_name`) tuples
///
/// Output:
/// - `Some((binary, package))` if an installed candidate is found, `None` otherwise
///
/// Details:
/// - Checks both PATH and package installation for each candidate
fn find_first_installed_candidate<'a>(
    candidates: &'a [(&'a str, &'a str)],
) -> Option<(&'a str, &'a str)> {
    for (bin, pkg) in candidates {
        if is_tool_installed(pkg, bin) {
            return Some((*bin, *pkg));
        }
    }
    None
}

/// What: Check if helix editor is installed (handles hx/helix aliases).
///
/// Inputs:
/// - `pkg`: Package name (should be "helix")
/// - `bin`: Binary name to check
///
/// Output:
/// - `true` if helix is installed via any alias
///
/// Details:
/// - Checks both hx and helix binaries for helix package
fn is_helix_installed(pkg: &str, bin: &str) -> bool {
    if pkg == "helix" {
        is_tool_installed(pkg, bin) || is_tool_installed(pkg, "hx")
    } else {
        is_tool_installed(pkg, bin)
    }
}

/// What: Check if emacs editor is installed (handles emacs/emacsclient aliases).
///
/// Inputs:
/// - `pkg`: Package name (should be "emacs")
/// - `bin`: Binary name to check
///
/// Output:
/// - `true` if emacs is installed via any alias
///
/// Details:
/// - Checks both emacs and emacsclient binaries for emacs package
fn is_emacs_installed(pkg: &str, bin: &str) -> bool {
    if pkg == "emacs" {
        is_tool_installed(pkg, bin) || is_tool_installed(pkg, "emacsclient")
    } else {
        is_tool_installed(pkg, bin)
    }
}

/// What: Check if an editor candidate is installed (handles special aliases).
///
/// Inputs:
/// - `pkg`: Package name
/// - `bin`: Binary name
///
/// Output:
/// - `true` if the editor is installed
///
/// Details:
/// - Handles helix and emacs aliases specially
fn is_editor_installed(pkg: &str, bin: &str) -> bool {
    if pkg == "helix" {
        is_helix_installed(pkg, bin)
    } else if pkg == "emacs" {
        is_emacs_installed(pkg, bin)
    } else {
        is_tool_installed(pkg, bin)
    }
}

/// What: Build editor rows for the optional deps modal.
///
/// Inputs:
/// - `app`: Application state for i18n
/// - `rows`: Mutable vector to append rows to
///
/// Output:
/// - Appends editor rows to the provided vector
///
/// Details:
/// - Shows first installed editor, or all candidates if none installed
/// - Handles helix (hx/helix) and emacs (emacs/emacsclient) aliases
fn build_editor_rows(app: &AppState, rows: &mut Vec<crate::state::types::OptionalDepRow>) {
    let editor_candidates: &[(&str, &str)] = &[
        ("nvim", "neovim"),
        ("vim", "vim"),
        ("hx", "helix"),
        ("helix", "helix"),
        ("emacsclient", "emacs"),
        ("emacs", "emacs"),
        ("nano", "nano"),
    ];

    if let Some((bin, pkg)) = find_first_installed_candidate(editor_candidates) {
        let installed = is_editor_installed(pkg, bin);
        rows.push(create_optional_dep_row(
            app,
            "app.optional_deps.categories.editor",
            bin,
            pkg.to_string(),
            installed,
            None,
        ));
    } else {
        // Show unique packages (avoid hx+helix duplication)
        let mut seen = std::collections::HashSet::new();
        for (bin, pkg) in editor_candidates {
            if seen.insert(*pkg) {
                let installed = is_editor_installed(pkg, bin);
                rows.push(create_optional_dep_row(
                    app,
                    "app.optional_deps.categories.editor",
                    bin,
                    (*pkg).to_string(),
                    installed,
                    None,
                ));
            }
        }
    }
}

/// What: Build terminal rows for the optional deps modal.
///
/// Inputs:
/// - `app`: Application state for i18n
/// - `rows`: Mutable vector to append rows to
///
/// Output:
/// - Appends terminal rows to the provided vector
///
/// Details:
/// - Shows first installed terminal, or all candidates if none installed
fn build_terminal_rows(app: &AppState, rows: &mut Vec<crate::state::types::OptionalDepRow>) {
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

    if let Some((bin, pkg)) = find_first_installed_candidate(term_candidates) {
        let installed = is_tool_installed(pkg, bin);
        rows.push(create_optional_dep_row(
            app,
            "app.optional_deps.categories.terminal",
            bin,
            pkg.to_string(),
            installed,
            None,
        ));
    } else {
        for (bin, pkg) in term_candidates {
            let installed = is_tool_installed(pkg, bin);
            rows.push(create_optional_dep_row(
                app,
                "app.optional_deps.categories.terminal",
                bin,
                (*pkg).to_string(),
                installed,
                None,
            ));
        }
    }
}

/// What: Check if KDE session is active.
///
/// Inputs:
/// - None (reads environment variables)
///
/// Output:
/// - `true` if KDE session is detected
///
/// Details:
/// - Checks `KDE_FULL_SESSION`, `XDG_CURRENT_DESKTOP`, and `klipper` command
fn is_kde_session() -> bool {
    std::env::var("KDE_FULL_SESSION").is_ok()
        || std::env::var("XDG_CURRENT_DESKTOP").ok().is_some_and(|v| {
            let u = v.to_uppercase();
            u.contains("KDE") || u.contains("PLASMA")
        })
        || crate::install::command_on_path("klipper")
}

/// What: Build clipboard rows for the optional deps modal.
///
/// Inputs:
/// - `app`: Application state for i18n
/// - `rows`: Mutable vector to append rows to
///
/// Output:
/// - Appends clipboard rows to the provided vector
///
/// Details:
/// - Prefers Klipper for KDE, then wl-clipboard for Wayland, else xclip for X11
fn build_clipboard_rows(app: &AppState, rows: &mut Vec<crate::state::types::OptionalDepRow>) {
    if is_kde_session() {
        let pkg = "plasma-workspace";
        let installed = is_tool_installed(pkg, "klipper");
        rows.push(create_optional_dep_row(
            app,
            "app.optional_deps.categories.clipboard",
            "Klipper (KDE)",
            pkg.to_string(),
            installed,
            Some("KDE Plasma".to_string()),
        ));
    } else if std::env::var("WAYLAND_DISPLAY").is_ok() {
        let pkg = "wl-clipboard";
        let installed = is_tool_installed(pkg, "wl-copy");
        rows.push(create_optional_dep_row(
            app,
            "app.optional_deps.categories.clipboard",
            "wl-clipboard",
            pkg.to_string(),
            installed,
            Some("Wayland".to_string()),
        ));
    } else {
        let pkg = "xclip";
        let installed = is_tool_installed(pkg, "xclip");
        rows.push(create_optional_dep_row(
            app,
            "app.optional_deps.categories.clipboard",
            "xclip",
            pkg.to_string(),
            installed,
            Some("X11".to_string()),
        ));
    }
}

/// What: Build mirror manager rows for the optional deps modal.
///
/// Inputs:
/// - `rows`: Mutable vector to append rows to
///
/// Output:
/// - Appends mirror manager rows to the provided vector
///
/// Details:
/// - Detects Manjaro (pacman-mirrors), Artix (rate-mirrors), or default (reflector)
fn build_mirror_rows(rows: &mut Vec<crate::state::types::OptionalDepRow>) {
    let os_release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let manjaro = os_release.contains("Manjaro");
    let artix = os_release.contains("Artix");

    if manjaro {
        let pkg = "pacman-mirrors";
        let installed = crate::index::is_installed(pkg);
        rows.push(crate::state::types::OptionalDepRow {
            label: "Mirrors: pacman-mirrors".to_string(),
            package: pkg.to_string(),
            installed,
            selectable: !installed,
            note: Some("Manjaro".to_string()),
        });
    } else if artix {
        let pkg = "rate-mirrors";
        let installed = is_tool_installed(pkg, "rate-mirrors");
        rows.push(crate::state::types::OptionalDepRow {
            label: "Mirrors: rate mirrors".to_string(),
            package: pkg.to_string(),
            installed,
            selectable: !installed,
            note: Some("Artix".to_string()),
        });
    } else {
        let pkg = "reflector";
        let installed = crate::index::is_installed(pkg);
        rows.push(crate::state::types::OptionalDepRow {
            label: "Mirrors: reflector".to_string(),
            package: pkg.to_string(),
            installed,
            selectable: !installed,
            note: None,
        });
    }
}

/// What: Build AUR helper rows for the optional deps modal.
///
/// Inputs:
/// - `app`: Application state for i18n
/// - `rows`: Mutable vector to append rows to
///
/// Output:
/// - Appends AUR helper rows to the provided vector
///
/// Details:
/// - Shows installed paru/yay if present, or both if neither installed
fn build_aur_helper_rows(app: &AppState, rows: &mut Vec<crate::state::types::OptionalDepRow>) {
    let paru_inst = is_tool_installed("paru", "paru");
    let yay_inst = is_tool_installed("yay", "yay");

    if paru_inst {
        rows.push(create_optional_dep_row(
            app,
            "app.optional_deps.categories.aur_helper",
            "paru",
            "paru".to_string(),
            true,
            None,
        ));
    } else if yay_inst {
        rows.push(create_optional_dep_row(
            app,
            "app.optional_deps.categories.aur_helper",
            "yay",
            "yay".to_string(),
            true,
            None,
        ));
    } else {
        let note = Some("Install via git clone + makepkg -si".to_string());
        rows.push(create_optional_dep_row(
            app,
            "app.optional_deps.categories.aur_helper",
            "paru",
            "paru".to_string(),
            false,
            note.clone(),
        ));
        rows.push(create_optional_dep_row(
            app,
            "app.optional_deps.categories.aur_helper",
            "yay",
            "yay".to_string(),
            false,
            note,
        ));
    }
}

/// What: Build security scanner rows for the optional deps modal.
///
/// Inputs:
/// - `app`: Application state for i18n
/// - `rows`: Mutable vector to append rows to
///
/// Output:
/// - Appends security scanner rows to the provided vector
///
/// Details:
/// - Includes `ClamAV`, `Trivy`, `Semgrep`, `ShellCheck`, `VirusTotal API`, and `aur-sleuth`
fn build_security_scanner_rows(
    app: &AppState,
    rows: &mut Vec<crate::state::types::OptionalDepRow>,
) {
    // ClamAV
    let installed = is_tool_installed("clamav", "clamscan");
    rows.push(create_optional_dep_row(
        app,
        "app.optional_deps.categories.security",
        "clamav",
        "clamav".to_string(),
        installed,
        None,
    ));

    // Trivy
    let installed = is_tool_installed("trivy", "trivy");
    rows.push(create_optional_dep_row(
        app,
        "app.optional_deps.categories.security",
        "trivy",
        "trivy".to_string(),
        installed,
        None,
    ));

    // Semgrep
    let installed = is_tool_installed("semgrep-bin", "semgrep");
    rows.push(create_optional_dep_row(
        app,
        "app.optional_deps.categories.security",
        "semgrep-bin",
        "semgrep-bin".to_string(),
        installed,
        Some("AUR".to_string()),
    ));

    // ShellCheck
    let installed = is_tool_installed("shellcheck", "shellcheck");
    rows.push(create_optional_dep_row(
        app,
        "app.optional_deps.categories.security",
        "shellcheck",
        "shellcheck".to_string(),
        installed,
        None,
    ));

    // VirusTotal API setup
    let vt_key_present = !crate::theme::settings().virustotal_api_key.is_empty();
    rows.push(create_optional_dep_row(
        app,
        "app.optional_deps.categories.security",
        "VirusTotal API",
        "virustotal-setup".to_string(),
        vt_key_present,
        Some("Setup".to_string()),
    ));

    // aur-sleuth setup
    let sleuth_installed = {
        let onpath = crate::install::command_on_path("aur-sleuth");
        let home = std::env::var("HOME").ok();
        let user_local = home.as_deref().is_some_and(|h| {
            std::path::Path::new(h)
                .join(".local/bin/aur-sleuth")
                .exists()
        });
        let system_local = std::path::Path::new("/usr/local/bin/aur-sleuth").exists();
        onpath || user_local || system_local
    };
    rows.push(create_optional_dep_row(
        app,
        "app.optional_deps.categories.security",
        "aur-sleuth",
        "aur-sleuth-setup".to_string(),
        sleuth_installed,
        Some("Setup".to_string()),
    ));
}

/// Build optional dependencies rows for the `OptionalDeps` modal.
///
/// What: Scan the system for installed editors, terminals, clipboard tools, mirror managers,
/// AUR helpers, and security scanners, then build a list of `OptionalDepRow` items for display.
///
/// Inputs:
/// - `app`: Application state (used for i18n translations)
///
/// Output:
/// - Vector of `OptionalDepRow` items ready to be displayed in the `OptionalDeps` modal.
///
/// Details:
/// - Editor: Shows the first installed editor found (`nvim`, `vim`, `hx`/`helix`, `emacsclient`/`emacs`, `nano`),
///   or all candidates if none installed. Handles helix (`hx`/`helix`) and emacs (`emacs`/`emacsclient`) aliases.
/// - Terminal: Shows the first installed terminal found, or all candidates if none installed.
/// - Clipboard: Detects `KDE` (`Klipper`), `Wayland` (`wl-clipboard`), or `X11` (`xclip`) and shows appropriate tool.
/// - Mirrors: Detects `Manjaro` (`pacman-mirrors`), `Artix` (`rate-mirrors`), or default (`reflector`).
/// - AUR helper: Shows installed `paru`/`yay` if present, or both if neither installed.
/// - Security scanners: Always includes `ClamAV`, `Trivy`, `Semgrep`, `ShellCheck`, `VirusTotal API` setup,
///   and `aur-sleuth` setup. Marks installed items as non-selectable.
pub fn build_optional_deps_rows(app: &AppState) -> Vec<crate::state::types::OptionalDepRow> {
    let mut rows: Vec<crate::state::types::OptionalDepRow> = Vec::new();

    build_editor_rows(app, &mut rows);
    build_terminal_rows(app, &mut rows);
    build_clipboard_rows(app, &mut rows);
    build_mirror_rows(&mut rows);
    build_aur_helper_rows(app, &mut rows);
    build_security_scanner_rows(app, &mut rows);

    rows
}
