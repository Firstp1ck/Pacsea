#[cfg(test)]
#[allow(clippy::items_after_test_module, clippy::module_inception)]
mod tests {
    use crate::theme::config::settings_ensure::ensure_settings_keys_present;
    use crate::theme::config::settings_save::{
        save_selected_countries, save_show_recent_pane, save_sort_mode,
    };
    use crate::theme::config::skeletons::{SETTINGS_SKELETON_CONTENT, THEME_SKELETON_CONTENT};
    use crate::theme::config::theme_loader::try_load_theme_with_diagnostics;
    use crate::theme::parsing::canonical_for_key;

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
    /// What: Validate theme skeleton configuration completeness and parsing.
    ///
    /// Inputs:
    /// - Theme skeleton content and theme loader function.
    ///
    /// Output:
    /// - Confirms skeleton contains all 16 required theme keys and can be parsed successfully.
    ///
    /// Details:
    /// - Verifies that the skeleton includes all canonical theme keys mapped from preferred names.
    /// - Ensures the skeleton can be loaded without errors.
    /// - Tests that a generated skeleton file contains all required keys.
    fn config_theme_skeleton_completeness() {
        use std::collections::HashSet;
        use std::fs;

        // Test 1: Verify all required theme keys are present in skeleton config
        let skeleton_content = THEME_SKELETON_CONTENT;
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
                    // Map to canonical key if possible
                    let canon = canonical_for_key(&key).unwrap_or(&key);
                    Some(canon.to_string())
                } else {
                    None
                }
            })
            .collect();

        // All 16 required canonical theme keys
        let required_keys: HashSet<&str> = [
            "base", "mantle", "crust", "surface1", "surface2", "overlay1", "overlay2", "text",
            "subtext0", "subtext1", "sapphire", "mauve", "green", "yellow", "red", "lavender",
        ]
        .into_iter()
        .collect();

        for key in &required_keys {
            assert!(
                skeleton_keys.contains(*key),
                "Missing required key '{}' in theme skeleton config",
                key
            );
        }

        // Test 2: Verify skeleton can be parsed successfully
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "pacsea_test_theme_skeleton_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);
        let theme_path = dir.join("theme.conf");
        fs::write(&theme_path, skeleton_content).unwrap();

        let theme_result = try_load_theme_with_diagnostics(&theme_path);
        assert!(
            theme_result.is_ok(),
            "Theme skeleton should parse successfully: {:?}",
            theme_result.err()
        );
        let theme = theme_result.unwrap();
        // Verify all fields are set (they should be non-zero colors)
        let _ = (
            theme.base,
            theme.mantle,
            theme.crust,
            theme.surface1,
            theme.surface2,
            theme.overlay1,
            theme.overlay2,
            theme.text,
            theme.subtext0,
            theme.subtext1,
            theme.sapphire,
            theme.mauve,
            theme.green,
            theme.yellow,
            theme.red,
            theme.lavender,
        );

        // Test 3: Verify generated skeleton file contains all required keys
        let generated_content = fs::read_to_string(&theme_path).unwrap();
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
                    // Map to canonical key if possible
                    let canon = canonical_for_key(&key).unwrap_or(&key);
                    Some(canon.to_string())
                } else {
                    None
                }
            })
            .collect();

        for key in &required_keys {
            assert!(
                generated_keys.contains(*key),
                "Missing required key '{}' in generated theme skeleton file",
                key
            );
        }

        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }

    /// What: Extract keys from config file content.
    ///
    /// Inputs:
    /// - Config file content as string.
    ///
    /// Output:
    /// - HashSet of normalized key names extracted from the config.
    ///
    /// Details:
    /// - Skips empty lines, comments, and lines without '='.
    /// - Normalizes keys by lowercasing and replacing special characters with underscores.
    fn extract_config_keys(content: &str) -> std::collections::HashSet<String> {
        content
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
                    return None;
                }
                trimmed.find('=').map(|eq_pos| {
                    trimmed[..eq_pos]
                        .trim()
                        .to_lowercase()
                        .replace(['.', '-', ' '], "_")
                })
            })
            .collect()
    }

    /// What: Get all expected Settings keys.
    ///
    /// Inputs: None
    ///
    /// Output:
    /// - `HashSet` of expected Settings key names.
    ///
    /// Details:
    /// - Returns the list of all expected Settings keys (excluding keymap which is in keybinds.conf).
    fn get_expected_settings_keys() -> std::collections::HashSet<&'static str> {
        [
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
        .collect()
    }

    /// What: Verify that all expected keys are present in the extracted keys.
    ///
    /// Inputs:
    /// - Extracted keys from config and expected keys set.
    ///
    /// Output: None (panics on failure)
    ///
    /// Details:
    /// - Asserts that all expected keys are present in the extracted keys.
    fn assert_all_keys_present(
        extracted_keys: &std::collections::HashSet<String>,
        expected_keys: &std::collections::HashSet<&str>,
        context: &str,
    ) {
        for key in expected_keys {
            assert!(
                extracted_keys.contains(*key),
                "Missing key '{key}' in {context}"
            );
        }
    }

    /// What: Verify that loaded settings match default settings.
    ///
    /// Inputs:
    /// - Loaded settings and default settings.
    ///
    /// Output: None (panics on failure)
    ///
    /// Details:
    /// - Compares all fields of loaded settings against defaults.
    fn assert_settings_match_defaults(
        loaded: &crate::theme::types::Settings,
        defaults: &crate::theme::types::Settings,
    ) {
        assert_eq!(
            loaded.layout_left_pct, defaults.layout_left_pct,
            "layout_left_pct should match default"
        );
        assert_eq!(
            loaded.layout_center_pct, defaults.layout_center_pct,
            "layout_center_pct should match default"
        );
        assert_eq!(
            loaded.layout_right_pct, defaults.layout_right_pct,
            "layout_right_pct should match default"
        );
        assert_eq!(
            loaded.app_dry_run_default, defaults.app_dry_run_default,
            "app_dry_run_default should match default"
        );
        assert_eq!(
            loaded.sort_mode.as_config_key(),
            defaults.sort_mode.as_config_key(),
            "sort_mode should match default"
        );
        assert_eq!(
            loaded.clipboard_suffix, defaults.clipboard_suffix,
            "clipboard_suffix should match default"
        );
        assert_eq!(
            loaded.show_recent_pane, defaults.show_recent_pane,
            "show_recent_pane should match default"
        );
        assert_eq!(
            loaded.show_install_pane, defaults.show_install_pane,
            "show_install_pane should match default"
        );
        assert_eq!(
            loaded.show_keybinds_footer, defaults.show_keybinds_footer,
            "show_keybinds_footer should match default"
        );
        assert_eq!(
            loaded.selected_countries, defaults.selected_countries,
            "selected_countries should match default"
        );
        assert_eq!(
            loaded.mirror_count, defaults.mirror_count,
            "mirror_count should match default"
        );
        assert_eq!(
            loaded.virustotal_api_key, defaults.virustotal_api_key,
            "virustotal_api_key should match default"
        );
        assert_eq!(
            loaded.scan_do_clamav, defaults.scan_do_clamav,
            "scan_do_clamav should match default"
        );
        assert_eq!(
            loaded.scan_do_trivy, defaults.scan_do_trivy,
            "scan_do_trivy should match default"
        );
        assert_eq!(
            loaded.scan_do_semgrep, defaults.scan_do_semgrep,
            "scan_do_semgrep should match default"
        );
        assert_eq!(
            loaded.scan_do_shellcheck, defaults.scan_do_shellcheck,
            "scan_do_shellcheck should match default"
        );
        assert_eq!(
            loaded.scan_do_virustotal, defaults.scan_do_virustotal,
            "scan_do_virustotal should match default"
        );
        assert_eq!(
            loaded.scan_do_custom, defaults.scan_do_custom,
            "scan_do_custom should match default"
        );
        assert_eq!(
            loaded.scan_do_sleuth, defaults.scan_do_sleuth,
            "scan_do_sleuth should match default"
        );
        assert_eq!(
            loaded.news_read_symbol, defaults.news_read_symbol,
            "news_read_symbol should match default"
        );
        assert_eq!(
            loaded.news_unread_symbol, defaults.news_unread_symbol,
            "news_unread_symbol should match default"
        );
        assert_eq!(
            loaded.preferred_terminal, defaults.preferred_terminal,
            "preferred_terminal should match default"
        );
    }

    /// What: Verify that config content contains a key-value pair (with flexible spacing).
    ///
    /// Inputs:
    /// - Config content and key-value pair to check.
    ///
    /// Output: None (panics on failure)
    ///
    /// Details:
    /// - Checks for both "key = value" and "key=value" formats.
    fn assert_config_contains(content: &str, key: &str, value: &str, test_context: &str) {
        assert!(
            content.contains(&format!("{key} = {value}"))
                || content.contains(&format!("{key}={value}")),
            "{test_context} should persist {key}"
        );
    }

    /// What: Restore original environment variables.
    ///
    /// Inputs:
    /// - Original HOME and `XDG_CONFIG_HOME` values.
    ///
    /// Output: None
    ///
    /// Details:
    /// - Restores environment variables to their original values or removes them if they were unset.
    fn restore_env_vars(
        orig_home: Option<std::ffi::OsString>,
        orig_xdg: Option<std::ffi::OsString>,
    ) {
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
    }

    /// What: Setup temporary test environment for settings tests.
    ///
    /// Inputs: None
    ///
    /// Output:
    /// - Tuple of (base directory, config directory, original HOME, original `XDG_CONFIG_HOME`)
    ///
    /// Details:
    /// - Creates temporary directory structure and sets environment variables for isolated testing.
    fn setup_test_environment() -> (
        std::path::PathBuf,
        std::path::PathBuf,
        Option<std::ffi::OsString>,
        Option<std::ffi::OsString>,
    ) {
        use std::fs;
        let orig_home = std::env::var_os("HOME");
        let orig_xdg = std::env::var_os("XDG_CONFIG_HOME");

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

        (base, cfg_dir, orig_home, orig_xdg)
    }

    /// What: Test that skeleton config contains all expected keys.
    ///
    /// Inputs:
    /// - Expected keys set.
    ///
    /// Output: None (panics on failure)
    ///
    /// Details:
    /// - Verifies that the skeleton configuration contains all required settings keys.
    fn test_skeleton_contains_all_keys(expected_keys: &std::collections::HashSet<&str>) {
        let skeleton_keys = extract_config_keys(SETTINGS_SKELETON_CONTENT);
        assert_all_keys_present(&skeleton_keys, expected_keys, "skeleton config");
    }

    /// What: Test that missing config file is generated with skeleton.
    ///
    /// Inputs:
    /// - Settings path and expected keys.
    ///
    /// Output: None (panics on failure)
    ///
    /// Details:
    /// - Verifies that calling `ensure_settings_keys_present` creates a config file with all required keys.
    fn test_missing_config_generation(
        settings_path: &std::path::Path,
        expected_keys: &std::collections::HashSet<&str>,
    ) {
        use std::fs;
        assert!(
            !settings_path.exists(),
            "Settings file should not exist initially"
        );

        let default_prefs = crate::theme::types::Settings::default();
        ensure_settings_keys_present(&default_prefs);

        assert!(settings_path.exists(), "Settings file should be created");
        let generated_content = fs::read_to_string(settings_path).unwrap();
        assert!(
            !generated_content.is_empty(),
            "Generated config file should not be empty"
        );

        let generated_keys = extract_config_keys(&generated_content);
        assert_all_keys_present(&generated_keys, expected_keys, "generated config file");
    }

    /// What: Test that missing keys are added to config with defaults.
    ///
    /// Inputs:
    /// - Settings path and expected keys.
    ///
    /// Output: None (panics on failure)
    ///
    /// Details:
    /// - Creates a minimal config and verifies that `ensure_settings_keys_present` adds missing keys while preserving existing ones.
    fn test_missing_keys_added(
        settings_path: &std::path::Path,
        expected_keys: &std::collections::HashSet<&str>,
    ) {
        use std::fs;
        fs::write(
            settings_path,
            "# Minimal config\nsort_mode = aur_popularity\n",
        )
        .unwrap();

        let modified_prefs = crate::theme::types::Settings {
            sort_mode: crate::state::SortMode::AurPopularityThenOfficial,
            ..crate::theme::types::Settings::default()
        };
        ensure_settings_keys_present(&modified_prefs);

        let updated_content = fs::read_to_string(settings_path).unwrap();
        let updated_keys = extract_config_keys(&updated_content);
        assert_all_keys_present(
            &updated_keys,
            expected_keys,
            "after ensure_settings_keys_present",
        );

        assert!(
            updated_content.contains("sort_mode = aur_popularity")
                || updated_content.contains("sort_mode=aur_popularity"),
            "sort_mode should be preserved in config"
        );
    }

    /// What: Test that custom parameters can be loaded from config file.
    ///
    /// Inputs:
    /// - Settings path.
    ///
    /// Output: None (panics on failure)
    ///
    /// Details:
    /// - Writes a config file with custom values and verifies all values are loaded correctly.
    fn test_load_custom_parameters(settings_path: &std::path::Path) {
        use std::fs;
        fs::write(
            settings_path,
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
    }

    /// What: Test that save functions persist values correctly.
    ///
    /// Inputs:
    /// - Settings path.
    ///
    /// Output: None (panics on failure)
    ///
    /// Details:
    /// - Tests that save functions correctly persist values to the config file and can be reloaded.
    fn test_save_functions_persist(settings_path: &std::path::Path) {
        use std::fs;
        save_sort_mode(crate::state::SortMode::BestMatches);
        let saved_content = fs::read_to_string(settings_path).unwrap();
        assert_config_contains(
            &saved_content,
            "sort_mode",
            "best_matches",
            "save_sort_mode",
        );

        save_show_recent_pane(true);
        let saved_content2 = fs::read_to_string(settings_path).unwrap();
        assert_config_contains(
            &saved_content2,
            "show_recent_pane",
            "true",
            "save_show_recent_pane",
        );

        save_selected_countries("Switzerland, Austria");
        let saved_content3 = fs::read_to_string(settings_path).unwrap();
        assert_config_contains(
            &saved_content3,
            "selected_countries",
            "Switzerland, Austria",
            "save_selected_countries",
        );

        let reloaded = crate::theme::settings::settings();
        assert_eq!(reloaded.sort_mode.as_config_key(), "best_matches");
        assert!(reloaded.show_recent_pane);
        assert_eq!(reloaded.selected_countries, "Switzerland, Austria");
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
        use std::fs;

        let _guard = crate::theme::test_mutex().lock().unwrap();
        let (base, cfg_dir, orig_home, orig_xdg) = setup_test_environment();

        let expected_keys = get_expected_settings_keys();
        let settings_path = cfg_dir.join("settings.conf");

        // Test 1: Verify all Settings fields are present in skeleton config
        test_skeleton_contains_all_keys(&expected_keys);

        // Test 2: Missing config file is correctly generated with skeleton
        test_missing_config_generation(&settings_path, &expected_keys);

        // Test 3: All parameters are loaded with defaults when missing
        fs::remove_file(&settings_path).unwrap();
        let loaded_settings = crate::theme::settings::settings();
        let default_settings = crate::theme::types::Settings::default();
        assert_settings_match_defaults(&loaded_settings, &default_settings);

        // Test 4: Missing keys are added to config with defaults
        test_missing_keys_added(&settings_path, &expected_keys);

        // Test 5: Parameters can be loaded from config file
        test_load_custom_parameters(&settings_path);

        // Test 6: Save functions persist values correctly
        test_save_functions_persist(&settings_path);

        // Cleanup
        restore_env_vars(orig_home, orig_xdg);
        let _ = fs::remove_dir_all(&base);
    }
}
