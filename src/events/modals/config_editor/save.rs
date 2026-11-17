//! Atomic saves, export redaction, stale-file guard, and reload.
use super::{
    AppState, ConfigEditorState, ConfigFile, EditableSetting, file_label_for_status,
    target_file_mtime,
};
use crate::state::config_editor::validate::{validate_keybind_conflict, validate_value};
use crate::theme::{
    PatchOutcome, PatchRequest, Sensitivity, ValueKind, patch_key, reload_theme,
    try_load_theme_from_content, write_full_content,
};
/// What: Export the effective values of the selected file's editable keys.
///
/// Inputs:
/// - `state`: Editor state (selected file + status line).
/// - `dry_run`: When `true`, report the would-be export without writing.
///
/// Output:
/// - Writes `effective_<file>` under the Pacsea lists directory and reports
///   the destination in the status line; sensitive values are exported as
///   `[REDACTED]`.
///
/// Details:
/// - Values come from [`crate::state::config_editor::current_value_string`],
///   i.e. the merged effective configuration the running app uses — not raw
///   file content — which is the useful artifact for support requests.
pub(super) fn export_effective_config(state: &mut ConfigEditorState, dry_run: bool) {
    use std::fmt::Write as _;
    let entries = crate::theme::settings_for(state.selected_file);
    if entries.is_empty() {
        state.status = Some("No editable keys to export for this file.".to_string());
        return;
    }
    let label = file_label_for_status(state.selected_file);
    let mut out = format!("# Pacsea effective values for {label} (config editor export)\n");
    for entry in &entries {
        let raw = crate::state::config_editor::current_value_string(entry);
        let value = if matches!(entry.sensitivity, Sensitivity::Sensitive)
            || (entry.key == "pi_scan_https_proxy" && raw.contains('@'))
        {
            "[REDACTED]".to_string()
        } else {
            raw
        };
        let _ = writeln!(out, "{} = {value}", entry.key);
    }
    let path = crate::theme::lists_dir().join(format!("effective_{label}"));
    if dry_run {
        state.status = Some(format!(
            "Dry-run: would export {} keys → {}",
            entries.len(),
            path.display()
        ));
        return;
    }
    match std::fs::write(&path, out) {
        Ok(()) => {
            state.status = Some(format!(
                "Exported {} keys → {}",
                entries.len(),
                path.display()
            ));
        }
        Err(e) => {
            state.status = Some(format!("Export failed: {e}"));
        }
    }
}

/// Build a [`PatchRequest`], call [`patch_key`], and on success trigger
/// settings reload. Updates `state.status` with the result and closes
/// the popup on success. Theme keys route through a pre-commit validation
/// path; a stale on-disk file (changed since the popup opened) warns once
/// before allowing an explicit overwrite.
pub(super) fn try_save_popup(app: &mut AppState, state: &mut ConfigEditorState) {
    let Some(popup) = state.popup.as_ref() else {
        return;
    };
    let setting = popup.setting;
    let value = popup.effective_value();
    let opened_mtime = popup.opened_mtime;
    let stale_armed = popup.stale_overwrite_armed;
    if let Err(msg) = validate_value(setting, &value) {
        state.status = Some(format!("Invalid value: {msg}"));
        return;
    }
    if matches!(setting.kind, ValueKind::KeyChord)
        && let Err(msg) = validate_keybind_conflict(setting, &value)
    {
        state.status = Some(format!("Conflict: {msg}"));
        return;
    }
    if !stale_armed && target_file_mtime(setting.file) != opened_mtime {
        state.status = Some(format!(
            "{} changed on disk while editing; press Ctrl+S again to overwrite.",
            file_label_for_status(setting.file)
        ));
        if let Some(p) = state.popup.as_mut() {
            p.stale_overwrite_armed = true;
        }
        return;
    }
    if matches!(setting.file, ConfigFile::Theme) {
        save_theme_popup(app, state, setting, &value);
        return;
    }
    let req = PatchRequest {
        file: setting.file,
        key: setting.key,
        aliases: setting.aliases,
        value: &value,
        dry_run: app.dry_run,
    };
    let key_label = setting.key;
    match patch_key(&req) {
        Ok(PatchOutcome::Written { path }) => {
            let note = if matches!(
                setting.reload,
                crate::theme::ReloadBehavior::RequiresRestart
            ) {
                " (restart required)"
            } else {
                ""
            };
            state.status = Some(format!("Saved {key_label} → {}{note}", path.display()));
            reload_after_save(app);
            state.popup = None;
        }
        Ok(PatchOutcome::NoChange { .. }) => {
            state.status = Some(format!("{key_label} is already set to that value."));
            state.popup = None;
        }
        Ok(PatchOutcome::DryRun { path, .. }) => {
            state.status = Some(format!(
                "Dry-run: would update {key_label} → {}",
                path.display()
            ));
            state.popup = None;
        }
        Err(e) => {
            state.status = Some(format!("Save failed: {e}"));
        }
    }
}

/// What: Save a theme color through whole-file pre-commit validation.
///
/// Inputs:
/// - `app`: Application state (dry-run flag).
/// - `state`: Editor state (status line + popup lifecycle).
/// - `setting`: Theme schema row being edited.
/// - `value`: Validated color literal about to be written.
///
/// Output:
/// - On success writes `theme.conf` atomically, re-runs
///   [`reload_theme`](crate::theme::reload_theme) so the palette applies
///   live, and closes the popup; on validation failure leaves disk and the
///   in-memory theme untouched and keeps the popup open.
///
/// Details:
/// - Builds the proposed full file via a dry-run [`patch_key`] pass, then
///   validates it with [`try_load_theme_from_content`] before committing via
///   [`write_full_content`], per the Phase 3 plan (invalid edits must never
///   reach `theme.conf`).
fn save_theme_popup(
    app: &AppState,
    state: &mut ConfigEditorState,
    setting: &'static EditableSetting,
    value: &str,
) {
    let probe = PatchRequest {
        file: setting.file,
        key: setting.key,
        aliases: setting.aliases,
        value,
        dry_run: true,
    };
    let proposed = match patch_key(&probe) {
        Ok(PatchOutcome::NoChange { .. }) => {
            state.status = Some(format!("{} is already set to that value.", setting.key));
            state.popup = None;
            return;
        }
        Ok(PatchOutcome::DryRun { proposed, .. }) => proposed,
        Ok(PatchOutcome::Written { .. }) => {
            state.status = Some("Save failed: unexpected write during validation.".to_string());
            return;
        }
        Err(e) => {
            state.status = Some(format!("Save failed: {e}"));
            return;
        }
    };
    if let Err(diagnostics) = try_load_theme_from_content(&proposed) {
        state.status = Some(format!(
            "Invalid theme rejected (file unchanged): {}",
            diagnostics.replace('\n', " ")
        ));
        return;
    }
    match write_full_content(ConfigFile::Theme, &proposed, app.dry_run) {
        Ok(PatchOutcome::Written { path }) => {
            let applied = match reload_theme() {
                Ok(()) => "theme applied",
                Err(_) => "reload failed; takes effect on restart",
            };
            state.status = Some(format!(
                "Saved {} → {} ({applied})",
                setting.key,
                path.display()
            ));
            state.popup = None;
        }
        Ok(PatchOutcome::NoChange { .. }) => {
            state.status = Some(format!("{} is already set to that value.", setting.key));
            state.popup = None;
        }
        Ok(PatchOutcome::DryRun { path, .. }) => {
            state.status = Some(format!(
                "Dry-run: would update {} → {}",
                setting.key,
                path.display()
            ));
            state.popup = None;
        }
        Err(e) => {
            state.status = Some(format!("Save failed: {e}"));
        }
    }
}

/// What: Reload the in-memory `Settings` snapshot after a successful save.
///
/// Inputs:
/// - `app`: Mutable application state.
///
/// Output:
/// - Mutates fields on `app` derived from `Settings`.
///
/// Details:
/// - Mirrors the relevant subset of `events::global::handle_reload_config`
///   for the settings-only Phase 1 flow. Theme/repos/locale reloads stay
///   on the existing `Reload Config` keybind path until later phases.
fn reload_after_save(app: &mut AppState) {
    let new_settings = crate::theme::settings();
    let editor_mode = app.app_mode;
    crate::app::apply_settings_to_app_state(app, &new_settings);
    // Applying settings also applies the startup mode; an editor save must retain its workspace.
    app.app_mode = editor_mode;
}
