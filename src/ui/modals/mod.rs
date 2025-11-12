use ratatui::{Frame, prelude::Rect, style::Style, widgets::Block};

use crate::state::AppState;
use crate::theme::theme;

mod alert;
mod common;
mod confirm;
mod help;
mod misc;
mod news;
mod post_summary;
mod preflight;
mod preflight_exec;
mod system_update;

/// What: Render modal overlays (Alert, ConfirmInstall, ConfirmRemove, SystemUpdate, Help, News).
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (modal state, rects)
/// - `area`: Full available area; modals are centered within it
///
/// Output:
/// - Draws the active modal overlay and updates any modal-specific rects for hit-testing.
///
/// Details:
/// - Clears the area behind the modal; draws a styled centered box; content varies by modal.
/// - Help dynamically reflects keymap; News draws a selectable list and records list rect.
pub fn render_modals(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();
    // Draw a full-screen scrim behind any active modal to avoid underlying text bleed/concatenation
    if !matches!(app.modal, crate::state::Modal::None) {
        let scrim = Block::default().style(Style::default().bg(th.mantle));
        f.render_widget(scrim, area);
    }

    // Extract modal to avoid borrow conflicts
    let modal = std::mem::replace(&mut app.modal, crate::state::Modal::None);

    match modal {
        crate::state::Modal::Alert { message } => {
            alert::render_alert(f, app, area, &message);
            app.modal = crate::state::Modal::Alert { message };
        }
        crate::state::Modal::ConfirmInstall { items } => {
            confirm::render_confirm_install(f, app, area, &items);
            app.modal = crate::state::Modal::ConfirmInstall { items };
        }
        crate::state::Modal::Preflight {
            items,
            action,
            tab,
            mut summary,
            mut header_chips,
            mut dependency_info,
            mut dep_selected,
            dep_tree_expanded,
            mut deps_error,
            mut file_info,
            mut file_selected,
            file_tree_expanded,
            mut files_error,
            mut service_info,
            mut service_selected,
            mut services_loaded,
            mut services_error,
            mut sandbox_info,
            mut sandbox_selected,
            sandbox_tree_expanded,
            mut sandbox_loaded,
            mut sandbox_error,
            mut selected_optdepends,
            cascade_mode,
        } => {
            preflight::render_preflight(
                f,
                area,
                app,
                &items,
                &action,
                &tab,
                &mut summary,
                &mut header_chips,
                &mut dependency_info,
                &mut dep_selected,
                &dep_tree_expanded,
                &mut deps_error,
                &mut file_info,
                &mut file_selected,
                &file_tree_expanded,
                &mut files_error,
                &mut service_info,
                &mut service_selected,
                &mut services_loaded,
                &mut services_error,
                &mut sandbox_info,
                &mut sandbox_selected,
                &sandbox_tree_expanded,
                &mut sandbox_loaded,
                &mut sandbox_error,
                &mut selected_optdepends,
                cascade_mode,
            );
            app.modal = crate::state::Modal::Preflight {
                items,
                action,
                tab,
                summary,
                header_chips,
                dependency_info,
                dep_selected,
                dep_tree_expanded,
                deps_error,
                file_info,
                file_selected,
                file_tree_expanded,
                files_error,
                service_info,
                service_selected,
                services_loaded,
                services_error,
                sandbox_info,
                sandbox_selected,
                sandbox_tree_expanded,
                sandbox_loaded,
                sandbox_error,
                selected_optdepends,
                cascade_mode,
            };
        }
        crate::state::Modal::PreflightExec {
            items,
            action,
            tab,
            verbose,
            log_lines,
            abortable,
            header_chips,
        } => {
            preflight_exec::render_preflight_exec(
                f,
                area,
                &items,
                action,
                tab,
                verbose,
                &log_lines,
                abortable,
                &header_chips,
            );
            app.modal = crate::state::Modal::PreflightExec {
                items,
                action,
                tab,
                verbose,
                log_lines,
                abortable,
                header_chips,
            };
        }
        crate::state::Modal::PostSummary {
            success,
            changed_files,
            pacnew_count,
            pacsave_count,
            services_pending,
            snapshot_label,
        } => {
            post_summary::render_post_summary(
                f,
                app,
                area,
                success,
                changed_files,
                pacnew_count,
                pacsave_count,
                &services_pending,
                snapshot_label.as_ref(),
            );
            app.modal = crate::state::Modal::PostSummary {
                success,
                changed_files,
                pacnew_count,
                pacsave_count,
                services_pending,
                snapshot_label,
            };
        }
        crate::state::Modal::ConfirmRemove { items } => {
            confirm::render_confirm_remove(f, app, area, &items);
            app.modal = crate::state::Modal::ConfirmRemove { items };
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
            system_update::render_system_update(
                f,
                app,
                area,
                do_mirrors,
                do_pacman,
                do_aur,
                do_cache,
                country_idx,
                &countries,
                mirror_count,
                cursor,
            );
            app.modal = crate::state::Modal::SystemUpdate {
                do_mirrors,
                do_pacman,
                do_aur,
                do_cache,
                country_idx,
                countries,
                mirror_count,
                cursor,
            };
        }
        crate::state::Modal::Help => {
            help::render_help(f, app, area);
            app.modal = crate::state::Modal::Help;
        }
        crate::state::Modal::News { items, selected } => {
            news::render_news(f, app, area, &items, selected);
            app.modal = crate::state::Modal::News { items, selected };
        }
        crate::state::Modal::OptionalDeps { rows, selected } => {
            misc::render_optional_deps(f, area, &rows, selected, app);
            app.modal = crate::state::Modal::OptionalDeps { rows, selected };
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
            misc::render_scan_config(
                f,
                area,
                do_clamav,
                do_trivy,
                do_semgrep,
                do_shellcheck,
                do_virustotal,
                do_custom,
                do_sleuth,
                cursor,
            );
            app.modal = crate::state::Modal::ScanConfig {
                do_clamav,
                do_trivy,
                do_semgrep,
                do_shellcheck,
                do_virustotal,
                do_custom,
                do_sleuth,
                cursor,
            };
        }
        crate::state::Modal::GnomeTerminalPrompt => {
            misc::render_gnome_terminal_prompt(f, area);
            app.modal = crate::state::Modal::GnomeTerminalPrompt;
        }
        crate::state::Modal::VirusTotalSetup { input, cursor } => {
            misc::render_virustotal_setup(f, app, area, &input);
            app.modal = crate::state::Modal::VirusTotalSetup { input, cursor };
        }
        crate::state::Modal::ImportHelp => {
            misc::render_import_help(f, area);
            app.modal = crate::state::Modal::ImportHelp;
        }
        crate::state::Modal::None => {
            app.modal = crate::state::Modal::None;
        }
    }
}

#[cfg(test)]
mod tests {
    /// What: Render each modal variant to ensure layout rects and state assignments succeed without panic.
    ///
    /// Inputs:
    /// - Iterates through Alert, ConfirmInstall, ConfirmRemove (core item), Help, and News variants.
    ///
    /// Output:
    /// - Rendering completes without error, with Help and News variants setting their associated rectangles.
    ///
    /// Details:
    /// - Uses a `TestBackend` terminal to capture layout side effects while mutating `app.modal` as each branch runs.
    #[test]
    fn modals_set_rects_and_render_variants() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(100, 28);
        let mut term = Terminal::new(backend).unwrap();
        let mut app = crate::state::AppState {
            ..Default::default()
        };

        // Alert
        app.modal = crate::state::Modal::Alert {
            message: "Test".into(),
        };
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area)
        })
        .unwrap();

        // ConfirmInstall
        app.modal = crate::state::Modal::ConfirmInstall { items: vec![] };
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area)
        })
        .unwrap();

        // ConfirmRemove with core warn
        app.modal = crate::state::Modal::ConfirmRemove {
            items: vec![crate::state::PackageItem {
                name: "glibc".into(),
                version: "1".into(),
                description: String::new(),
                source: crate::state::Source::Official {
                    repo: "core".into(),
                    arch: "x86_64".into(),
                },
                popularity: None,
            }],
        };
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area)
        })
        .unwrap();

        // Help
        app.modal = crate::state::Modal::Help;
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area)
        })
        .unwrap();
        assert!(app.help_rect.is_some());

        // News
        app.modal = crate::state::Modal::News {
            items: vec![crate::state::NewsItem {
                date: "2025-10-11".into(),
                title: "Test".into(),
                url: "".into(),
            }],
            selected: 0,
        };
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area)
        })
        .unwrap();
        assert!(app.news_rect.is_some());
        assert!(app.news_list_rect.is_some());
    }
}
