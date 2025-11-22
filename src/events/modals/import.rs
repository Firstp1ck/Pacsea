//! Import help modal handling.

use crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem};

/// What: Handle key events for `ImportHelp` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `add_tx`: Channel for adding packages
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Handles Esc to close, Enter to open file picker and import packages
pub(crate) fn handle_import_help(
    ke: KeyEvent,
    app: &mut AppState,
    add_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    match ke.code {
        KeyCode::Enter => {
            app.modal = crate::state::Modal::None;
            handle_import_help_enter(add_tx);
        }
        KeyCode::Esc => app.modal = crate::state::Modal::None,
        _ => {}
    }
    false
}

/// What: Handle Enter key in `ImportHelp` modal - open file picker and import packages.
///
/// Inputs:
/// - `add_tx`: Channel for adding packages
///
/// Output: None (spawns background thread)
///
/// Details:
/// - Opens file picker dialog and imports package names from selected file
/// - During tests, this is a no-op to avoid opening real file picker dialogs
fn handle_import_help_enter(_add_tx: &mpsc::UnboundedSender<PackageItem>) {
    // Skip actual file picker during tests
    #[cfg(not(test))]
    {
        tracing::info!("import: Enter pressed in ImportHelp modal");
        let add_tx_clone = _add_tx.clone();
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
                let try_cmd = |prog: &str, args: &[&str]| -> Option<String> {
                    tracing::debug!(prog = %prog, "import: trying file picker");
                    let res = std::process::Command::new(prog)
                        .args(args)
                        .stdin(std::process::Stdio::null())
                        .output()
                        .ok()?;
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
}
