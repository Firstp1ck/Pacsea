#[cfg(test)]
#[allow(clippy::items_after_test_module, clippy::module_inception)]
mod tests {
    use crate::theme::config::settings_ensure::ensure_settings_keys_present;
    use crate::theme::config::settings_save::{
        save_selected_countries, save_show_recent_pane, save_sort_mode,
    };
    use crate::theme::config::skeletons::SETTINGS_SKELETON_CONTENT;
    use crate::theme::config::theme_loader::try_load_theme_with_diagnostics;

    #[test]
    /// What: Exercise the theme loader on both valid and invalid theme files.
    ///
    /// Inputs:
    /// - Minimal theme file containing required canonical keys.
    /// - Second file with an unknown key and missing requirements.
    ///
    /// Output:
    /// - Successful load for the valid file and descriptive error messages for the invalid one.
    ///
    /// Details:
    /// - Uses temporary directories to avoid touching user configuration and cleans them up afterwards.
    fn config_try_load_theme_success_and_errors() {
        use std::fs;
        use std::io::Write;
        use std::path::PathBuf;
        // Minimal valid theme with required canonical keys
        let mut dir: PathBuf = std::env::temp_dir();
        dir.push(format!(
            "pacsea_test_theme_cfg_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);
        let mut p = dir.clone();
        p.push("theme.conf");
        let content = "base=#000000\nmantle=#000000\ncrust=#000000\nsurface1=#000000\nsurface2=#000000\noverlay1=#000000\noverlay2=#000000\ntext=#000000\nsubtext0=#000000\nsubtext1=#000000\nsapphire=#000000\nmauve=#000000\ngreen=#000000\nyellow=#000000\nred=#000000\nlavender=#000000\n";
        fs::write(&p, content).unwrap();
        let t = try_load_theme_with_diagnostics(&p).expect("valid theme");
        let _ = t.base; // use

        // Error case: unknown key + missing required
        let mut pe = dir.clone();
        pe.push("bad.conf");
        let mut f = fs::File::create(&pe).unwrap();
        writeln!(f, "unknown_key = #fff").unwrap();
        let err = try_load_theme_with_diagnostics(&pe).unwrap_err();
        assert!(err.contains("Unknown key"));
        assert!(err.contains("Missing required keys"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    /// What: Validate settings configuration scaffolding, persistence, and regeneration paths.
    ///
    /// Inputs:
    /// - Skeleton config content, temporary config directory, and helper functions for ensuring/saving settings.
    ///
    /// Output:
    /// - Confirms skeleton covers all expected keys, missing files regenerate, settings persist, and defaults apply when keys are absent.
    ///
    /// Details:
    /// - Manipulates `HOME`/`XDG_CONFIG_HOME` to isolate test data and cleans up generated files on completion.
    fn config_settings_comprehensive_parameter_check() {
        use std::collections::HashSet;
        use std::fs;

        let _guard = crate::theme::test_mutex().lock().unwrap();
        let orig_home = std::env::var_os("HOME");
        let orig_xdg = std::env::var_os("XDG_CONFIG_HOME");

        // Create temporary test directory
        let base = std::env::temp_dir().join(format!(
            "pacsea_test_config_params_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cfg_dir = base.join(".config").join("pacsea");
        let _ = fs::create_dir_all(&cfg_dir);
        unsafe {
            std::env::set_var("HOME", base.display().to_string());
            std::env::remove_var("XDG_CONFIG_HOME");
        }

        // Test 1: Verify all Settings fields are present in skeleton config
        let skeleton_content = SETTINGS_SKELETON_CONTENT;
        let skeleton_keys: HashSet<String> = skeleton_content
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
                    return None;
                }
                if let Some(eq_pos) = trimmed.find('=') {
                    let key = trimmed[..eq_pos]
                        .trim()
                        .to_lowercase()
                        .replace(['.', '-', ' '], "_");
                    Some(key)
                } else {
                    None
                }
            })
            .collect();

        // All expected Settings keys (excluding keymap which is in keybinds.conf)
        let expected_keys: HashSet<&str> = [
            "layout_left_pct",
            "layout_center_pct",
            "layout_right_pct",
            "app_dry_run_default",
            "sort_mode",
            "clipboard_suffix",
            "show_recent_pane",
            "show_install_pane",
            "show_keybinds_footer",
            "selected_countries",
            "mirror_count",
            "virustotal_api_key",
            "scan_do_clamav",
            "scan_do_trivy",
            "scan_do_semgrep",
            "scan_do_shellcheck",
            "scan_do_virustotal",
            "scan_do_custom",
            "scan_do_sleuth",
            "news_read_symbol",
            "news_unread_symbol",
            "preferred_terminal",
        ]
        .into_iter()
        .collect();

        for key in &expected_keys {
            assert!(
                skeleton_keys.contains(*key),
                "Missing key '{}' in skeleton config",
                key
            );
        }

        // Test 2: Missing config file is correctly generated with skeleton
        let settings_path = cfg_dir.join("settings.conf");
        assert!(
            !settings_path.exists(),
            "Settings file should not exist initially"
        );

        // Call ensure_settings_keys_present - should create file with skeleton
        let default_prefs = crate::theme::types::Settings::default();
        ensure_settings_keys_present(&default_prefs);

        assert!(settings_path.exists(), "Settings file should be created");
        let generated_content = fs::read_to_string(&settings_path).unwrap();
        assert!(
            !generated_content.is_empty(),
            "Generated config file should not be empty"
        );

        // Verify skeleton content matches generated file (ignoring whitespace differences)
        let generated_keys: HashSet<String> = generated_content
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
                    return None;
                }
                if let Some(eq_pos) = trimmed.find('=') {
                    let key = trimmed[..eq_pos]
                        .trim()
                        .to_lowercase()
                        .replace(['.', '-', ' '], "_");
                    Some(key)
                } else {
                    None
                }
            })
            .collect();

        for key in &expected_keys {
            assert!(
                generated_keys.contains(*key),
                "Missing key '{}' in generated config file",
                key
            );
        }

        // Test 3: All parameters are loaded with defaults when missing
        // Delete the config file and test loading
        fs::remove_file(&settings_path).unwrap();
        let loaded_settings = crate::theme::settings::settings();
        let default_settings = crate::theme::types::Settings::default();

        // Verify all fields match defaults
        assert_eq!(
            loaded_settings.layout_left_pct, default_settings.layout_left_pct,
            "layout_left_pct should match default"
        );
        assert_eq!(
            loaded_settings.layout_center_pct, default_settings.layout_center_pct,
            "layout_center_pct should match default"
        );
        assert_eq!(
            loaded_settings.layout_right_pct, default_settings.layout_right_pct,
            "layout_right_pct should match default"
        );
        assert_eq!(
            loaded_settings.app_dry_run_default, default_settings.app_dry_run_default,
            "app_dry_run_default should match default"
        );
        assert_eq!(
            loaded_settings.sort_mode.as_config_key(),
            default_settings.sort_mode.as_config_key(),
            "sort_mode should match default"
        );
        assert_eq!(
            loaded_settings.clipboard_suffix, default_settings.clipboard_suffix,
            "clipboard_suffix should match default"
        );
        assert_eq!(
            loaded_settings.show_recent_pane, default_settings.show_recent_pane,
            "show_recent_pane should match default"
        );
        assert_eq!(
            loaded_settings.show_install_pane, default_settings.show_install_pane,
            "show_install_pane should match default"
        );
        assert_eq!(
            loaded_settings.show_keybinds_footer, default_settings.show_keybinds_footer,
            "show_keybinds_footer should match default"
        );
        assert_eq!(
            loaded_settings.selected_countries, default_settings.selected_countries,
            "selected_countries should match default"
        );
        assert_eq!(
            loaded_settings.mirror_count, default_settings.mirror_count,
            "mirror_count should match default"
        );
        assert_eq!(
            loaded_settings.virustotal_api_key, default_settings.virustotal_api_key,
            "virustotal_api_key should match default"
        );
        assert_eq!(
            loaded_settings.scan_do_clamav, default_settings.scan_do_clamav,
            "scan_do_clamav should match default"
        );
        assert_eq!(
            loaded_settings.scan_do_trivy, default_settings.scan_do_trivy,
            "scan_do_trivy should match default"
        );
        assert_eq!(
            loaded_settings.scan_do_semgrep, default_settings.scan_do_semgrep,
            "scan_do_semgrep should match default"
        );
        assert_eq!(
            loaded_settings.scan_do_shellcheck, default_settings.scan_do_shellcheck,
            "scan_do_shellcheck should match default"
        );
        assert_eq!(
            loaded_settings.scan_do_virustotal, default_settings.scan_do_virustotal,
            "scan_do_virustotal should match default"
        );
        assert_eq!(
            loaded_settings.scan_do_custom, default_settings.scan_do_custom,
            "scan_do_custom should match default"
        );
        assert_eq!(
            loaded_settings.scan_do_sleuth, default_settings.scan_do_sleuth,
            "scan_do_sleuth should match default"
        );
        assert_eq!(
            loaded_settings.news_read_symbol, default_settings.news_read_symbol,
            "news_read_symbol should match default"
        );
        assert_eq!(
            loaded_settings.news_unread_symbol, default_settings.news_unread_symbol,
            "news_unread_symbol should match default"
        );
        assert_eq!(
            loaded_settings.preferred_terminal, default_settings.preferred_terminal,
            "preferred_terminal should match default"
        );

        // Test 4: Missing keys are added to config with defaults
        // Create a minimal config file with only one key
        fs::write(
            &settings_path,
            "# Minimal config\nsort_mode = aur_popularity\n",
        )
        .unwrap();

        // Call ensure_settings_keys_present - should add missing keys
        let modified_prefs = crate::theme::types::Settings {
            sort_mode: crate::state::SortMode::AurPopularityThenOfficial,
            ..crate::theme::types::Settings::default()
        };
        ensure_settings_keys_present(&modified_prefs);

        // Verify file now contains all keys
        let updated_content = fs::read_to_string(&settings_path).unwrap();
        let updated_keys: HashSet<String> = updated_content
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
                    return None;
                }
                if let Some(eq_pos) = trimmed.find('=') {
                    let key = trimmed[..eq_pos]
                        .trim()
                        .to_lowercase()
                        .replace(['.', '-', ' '], "_");
                    Some(key)
                } else {
                    None
                }
            })
            .collect();

        for key in &expected_keys {
            assert!(
                updated_keys.contains(*key),
                "Missing key '{}' after ensure_settings_keys_present",
                key
            );
        }

        // Verify sort_mode was preserved
        assert!(
            updated_content.contains("sort_mode = aur_popularity")
                || updated_content.contains("sort_mode=aur_popularity"),
            "sort_mode should be preserved in config"
        );

        // Test 5: Parameters can be loaded from config file
        // Write a config file with custom values
        fs::write(
            &settings_path,
            "layout_left_pct = 25\n\
             layout_center_pct = 50\n\
             layout_right_pct = 25\n\
             app_dry_run_default = true\n\
             sort_mode = alphabetical\n\
             clipboard_suffix = Custom suffix\n\
             show_recent_pane = false\n\
             show_install_pane = false\n\
             show_keybinds_footer = false\n\
             selected_countries = Germany, France\n\
             mirror_count = 30\n\
             virustotal_api_key = test_api_key\n\
             scan_do_clamav = false\n\
             scan_do_trivy = false\n\
             scan_do_semgrep = false\n\
             scan_do_shellcheck = false\n\
             scan_do_virustotal = false\n\
             scan_do_custom = false\n\
             scan_do_sleuth = false\n\
             news_read_symbol = READ\n\
             news_unread_symbol = UNREAD\n\
             preferred_terminal = alacritty\n",
        )
        .unwrap();

        let loaded_custom = crate::theme::settings::settings();
        assert_eq!(loaded_custom.layout_left_pct, 25);
        assert_eq!(loaded_custom.layout_center_pct, 50);
        assert_eq!(loaded_custom.layout_right_pct, 25);
        assert!(loaded_custom.app_dry_run_default);
        assert_eq!(loaded_custom.sort_mode.as_config_key(), "alphabetical");
        assert_eq!(loaded_custom.clipboard_suffix, "Custom suffix");
        assert!(!loaded_custom.show_recent_pane);
        assert!(!loaded_custom.show_install_pane);
        assert!(!loaded_custom.show_keybinds_footer);
        assert_eq!(loaded_custom.selected_countries, "Germany, France");
        assert_eq!(loaded_custom.mirror_count, 30);
        assert_eq!(loaded_custom.virustotal_api_key, "test_api_key");
        assert!(!loaded_custom.scan_do_clamav);
        assert!(!loaded_custom.scan_do_trivy);
        assert!(!loaded_custom.scan_do_semgrep);
        assert!(!loaded_custom.scan_do_shellcheck);
        assert!(!loaded_custom.scan_do_virustotal);
        assert!(!loaded_custom.scan_do_custom);
        assert!(!loaded_custom.scan_do_sleuth);
        assert_eq!(loaded_custom.news_read_symbol, "READ");
        assert_eq!(loaded_custom.news_unread_symbol, "UNREAD");
        assert_eq!(loaded_custom.preferred_terminal, "alacritty");

        // Test 6: Save functions persist values correctly
        // Test save_sort_mode
        save_sort_mode(crate::state::SortMode::BestMatches);
        let saved_content = fs::read_to_string(&settings_path).unwrap();
        assert!(
            saved_content.contains("sort_mode = best_matches")
                || saved_content.contains("sort_mode=best_matches"),
            "save_sort_mode should persist sort_mode"
        );

        // Test save_boolean_key via save_show_recent_pane
        save_show_recent_pane(true);
        let saved_content2 = fs::read_to_string(&settings_path).unwrap();
        assert!(
            saved_content2.contains("show_recent_pane = true")
                || saved_content2.contains("show_recent_pane=true"),
            "save_show_recent_pane should persist value"
        );

        // Test save_string_key via save_selected_countries
        save_selected_countries("Switzerland, Austria");
        let saved_content3 = fs::read_to_string(&settings_path).unwrap();
        assert!(
            saved_content3.contains("selected_countries = Switzerland, Austria")
                || saved_content3.contains("selected_countries=Switzerland, Austria"),
            "save_selected_countries should persist value"
        );

        // Verify saved values are loaded back
        let reloaded = crate::theme::settings::settings();
        assert_eq!(reloaded.sort_mode.as_config_key(), "best_matches");
        assert!(reloaded.show_recent_pane);
        assert_eq!(reloaded.selected_countries, "Switzerland, Austria");

        // Cleanup
        unsafe {
            if let Some(v) = orig_home {
                std::env::set_var("HOME", v);
            } else {
                std::env::remove_var("HOME");
            }
            if let Some(v) = orig_xdg {
                std::env::set_var("XDG_CONFIG_HOME", v);
            } else {
                std::env::remove_var("XDG_CONFIG_HOME");
            }
        }
        let _ = fs::remove_dir_all(&base);
    }
}
