//! Distro-related logic helpers (filtering and labels).

/// What: Determine whether results from a repository should be visible under current toggles.
///
/// Inputs:
/// - `repo`: Name of the repository associated with a package result.
/// - `app`: Application state providing the filter toggles for official repos.
///
/// Output:
/// - `true` when the repository passes the active filters; otherwise `false`.
///
/// Details:
/// - Normalizes repository names and applies special-handling for EOS/CachyOS/Artix/BlackArch classification helpers.
/// - Repos listed in `repos.conf` use dynamic toggles (`results_filter_show_<canonical>` in `settings.conf`).
/// - Unknown repositories are only allowed when every official filter is enabled simultaneously.
#[must_use]
pub fn repo_toggle_for(repo: &str, app: &crate::state::AppState) -> bool {
    let r = repo.to_lowercase();
    if r == "core" {
        app.results_filter_show_core
    } else if r == "extra" {
        app.results_filter_show_extra
    } else if r == "multilib" {
        app.results_filter_show_multilib
    } else if crate::index::is_eos_repo(&r) {
        app.results_filter_show_eos
    } else if crate::index::is_cachyos_repo(&r) {
        app.results_filter_show_cachyos
    } else if crate::index::is_artix_omniverse(&r) {
        app.results_filter_show_artix_omniverse
    } else if crate::index::is_artix_universe(&r) {
        app.results_filter_show_artix_universe
    } else if crate::index::is_artix_lib32(&r) {
        app.results_filter_show_artix_lib32
    } else if crate::index::is_artix_galaxy(&r) {
        app.results_filter_show_artix_galaxy
    } else if crate::index::is_artix_world(&r) {
        app.results_filter_show_artix_world
    } else if crate::index::is_artix_system(&r) {
        app.results_filter_show_artix_system
    } else if crate::index::is_artix_repo(&r) {
        // Fallback for any other Artix repo (shouldn't happen, but safe)
        app.results_filter_show_artix
    } else if crate::index::is_blackarch_repo(&r) {
        app.results_filter_show_blackarch
    } else if let Some(filter_key) = app.repo_results_filter_by_name.get(&r) {
        app.results_filter_dynamic
            .get(filter_key)
            .copied()
            .unwrap_or(true)
    } else {
        // Unknown official repo: include only when all official filters are enabled
        app.results_filter_show_core
            && app.results_filter_show_extra
            && app.results_filter_show_multilib
            && app.results_filter_show_eos
            && app.results_filter_show_cachyos
            && app.results_filter_show_artix
            && app.results_filter_show_artix_omniverse
            && app.results_filter_show_artix_universe
            && app.results_filter_show_artix_lib32
            && app.results_filter_show_artix_galaxy
            && app.results_filter_show_artix_world
            && app.results_filter_show_artix_system
            && app.results_filter_show_blackarch
    }
}

/// What: Produce a human-friendly label for an official package entry.
///
/// Inputs:
/// - `repo`: Repository reported by the package source.
/// - `name`: Package name used to detect Manjaro naming conventions.
/// - `owner`: Optional upstream owner string available from package metadata.
///
/// Output:
/// - Returns a display label describing the ecosystem the package belongs to.
///
/// Details:
/// - Distinguishes `EndeavourOS`, `CachyOS`, `Artix Linux` repos (with specific labels for each Artix repo:
///   `OMNI`, `UNI`, `LIB32`, `GALAXY`, `WORLD`, `SYSTEM`), `BlackArch`, and detects `Manjaro` branding by name/owner heuristics.
/// - Falls back to the raw repository string when no special classification matches.
#[must_use]
pub fn label_for_official(repo: &str, name: &str, owner: &str) -> String {
    let r = repo.to_lowercase();
    if crate::index::is_eos_repo(&r) {
        "EOS".to_string()
    } else if crate::index::is_cachyos_repo(&r) {
        "CachyOS".to_string()
    } else if crate::index::is_artix_omniverse(&r) {
        "OMNI".to_string()
    } else if crate::index::is_artix_universe(&r) {
        "UNI".to_string()
    } else if crate::index::is_artix_lib32(&r) {
        "LIB32".to_string()
    } else if crate::index::is_artix_galaxy(&r) {
        "GALAXY".to_string()
    } else if crate::index::is_artix_world(&r) {
        "WORLD".to_string()
    } else if crate::index::is_artix_system(&r) {
        "SYSTEM".to_string()
    } else if crate::index::is_artix_repo(&r) {
        // Fallback for any other Artix repo (shouldn't happen, but safe)
        "Artix".to_string()
    } else if crate::index::is_blackarch_repo(&r) {
        "BlackArch".to_string()
    } else if crate::index::is_manjaro_name_or_owner(name, owner) {
        "Manjaro".to_string()
    } else {
        repo.to_string()
    }
}

/// What: Build a shell snippet to refresh pacman mirrors depending on the detected distro.
///
/// Inputs:
/// - `countries`: Either "Worldwide" to auto-rank mirrors, or a comma-separated list used by the underlying tool
/// - `count`: Mirror count used by Manjaro fasttrack when `countries` is "Worldwide", or top mirrors number for Artix rate-mirrors
///
/// Output:
/// - Shell command string suitable to run in a terminal
///
/// Details:
/// - On Manjaro (detected via /etc/os-release), ensures pacman-mirrors exists, then:
///   - Worldwide: `pacman-mirrors --fasttrack {count}`
///   - Countries: `pacman-mirrors --method rank --country '{countries}'`
/// - On `Artix` (detected via /etc/os-release), checks for rate-mirrors and `AUR` helper (`yay`/`paru`), prompts for installation if needed, creates backup of mirrorlist, then runs rate-mirrors with country filtering using --entry-country option (only one country allowed, global option must come before the artix command).
/// - On `EndeavourOS`, ensures `eos-rankmirrors` is installed, runs it, then runs `reflector`.
/// - On `CachyOS`, ensures `cachyos-rate-mirrors` is installed, runs it, then runs `reflector`.
/// - Otherwise, attempts to use `reflector` to write `/etc/pacman.d/mirrorlist`; if not found, prints a notice.
///
/// # Errors
///
/// Returns `Err` when the configured privilege tool cannot be resolved (see [`crate::logic::privilege::active_tool`]).
pub fn mirror_update_command(countries: &str, count: u16) -> Result<String, String> {
    let bin = crate::logic::privilege::active_tool()?.binary_name();
    if countries.eq("Worldwide") {
        Ok(format!(
            "(if grep -q 'Manjaro' /etc/os-release 2>/dev/null; then \
    ((command -v pacman-mirrors >/dev/null 2>&1) || pacman -Qi pacman-mirrors >/dev/null 2>&1 || {bin} pacman -S --needed --noconfirm pacman-mirrors) && \
    {bin} pacman-mirrors --fasttrack {count}; \
  elif grep -q 'EndeavourOS' /etc/os-release 2>/dev/null; then \
    ((command -v eos-rankmirrors >/dev/null 2>&1) || pacman -Qi eos-rankmirrors >/dev/null 2>&1 || {bin} pacman -S --needed --noconfirm eos-rankmirrors) && {bin} eos-rankmirrors || echo 'eos-rankmirrors failed'; \
    (command -v reflector >/dev/null 2>&1 && {bin} reflector --verbose --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  elif grep -q 'CachyOS' /etc/os-release 2>/dev/null; then \
    ((command -v cachyos-rate-mirrors >/dev/null 2>&1) || pacman -Qi cachyos-rate-mirrors >/dev/null 2>&1 || {bin} pacman -S --needed --noconfirm cachyos-rate-mirrors) && {bin} cachyos-rate-mirrors || echo 'cachyos-rate-mirrors failed'; \
    (command -v reflector >/dev/null 2>&1 && {bin} reflector --verbose --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  elif grep -q 'Artix' /etc/os-release 2>/dev/null; then \
    if ! (pacman -Qi rate-mirrors >/dev/null 2>&1 || command -v rate-mirrors >/dev/null 2>&1); then \
      if ! (pacman -Qi paru >/dev/null 2>&1 || command -v paru >/dev/null 2>&1) && ! (pacman -Qi yay >/dev/null 2>&1 || command -v yay >/dev/null 2>&1); then \
        echo 'Error: rate-mirrors is not installed and no AUR helper (yay/paru) found.'; \
        echo 'Please install yay or paru first, or install rate-mirrors manually.'; \
        exit 1; \
      fi; \
      echo 'rate-mirrors is not installed.'; \
      read -rp 'Do you want to install rate-mirrors from AUR? (y/n): ' install_choice; \
      if [ \"$install_choice\" != \"y\" ] && [ \"$install_choice\" != \"Y\" ]; then \
        echo 'Mirror update cancelled.'; \
        exit 1; \
      fi; \
      if pacman -Qi paru >/dev/null 2>&1 || command -v paru >/dev/null 2>&1; then \
        paru -S --needed --noconfirm rate-mirrors || exit 1; \
      elif pacman -Qi yay >/dev/null 2>&1 || command -v yay >/dev/null 2>&1; then \
        yay -S --needed --noconfirm rate-mirrors || exit 1; \
      fi; \
    fi; \
    {bin} cp /etc/pacman.d/mirrorlist /etc/pacman.d/mirrorlist.backup.$(date +%Y%m%d_%H%M%S) || exit 1; \
    if [ \"{countries}\" = \"Worldwide\" ]; then \
      rate-mirrors --protocol=https --allow-root --country-neighbors-per-country=3 --top-mirrors-number-to-retest={count} --max-jumps=10 artix | {bin} tee /etc/pacman.d/mirrorlist || exit 1; \
    else \
      country_count=$(echo '{countries}' | tr ',' '\\n' | wc -l); \
      if [ \"$country_count\" -ne 1 ]; then \
        echo 'Error: Only one country is allowed for Artix mirror update.'; \
        echo 'Please select a single country.'; \
        exit 1; \
      fi; \
      rate-mirrors --protocol=https --allow-root --entry-country='{countries}' --country-neighbors-per-country=3 --top-mirrors-number-to-retest={count} --max-jumps=10 artix | {bin} tee /etc/pacman.d/mirrorlist || exit 1; \
    fi; \
  else \
    (command -v reflector >/dev/null 2>&1 && {bin} reflector --verbose --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  fi)"
        ))
    } else {
        Ok(format!(
            "(if grep -q 'Manjaro' /etc/os-release 2>/dev/null; then \
    ((command -v pacman-mirrors >/dev/null 2>&1) || pacman -Qi pacman-mirrors >/dev/null 2>&1 || {bin} pacman -S --needed --noconfirm pacman-mirrors) && \
    {bin} pacman-mirrors --method rank --country '{countries}'; \
  elif grep -q 'EndeavourOS' /etc/os-release 2>/dev/null; then \
    ((command -v eos-rankmirrors >/dev/null 2>&1) || pacman -Qi eos-rankmirrors >/dev/null 2>&1 || {bin} pacman -S --needed --noconfirm eos-rankmirrors) && {bin} eos-rankmirrors || echo 'eos-rankmirrors failed'; \
    (command -v reflector >/dev/null 2>&1 && {bin} reflector --verbose --country '{countries}' --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  elif grep -q 'CachyOS' /etc/os-release 2>/dev/null; then \
    ((command -v cachyos-rate-mirrors >/dev/null 2>&1) || pacman -Qi cachyos-rate-mirrors >/dev/null 2>&1 || {bin} pacman -S --needed --noconfirm cachyos-rate-mirrors) && {bin} cachyos-rate-mirrors || echo 'cachyos-rate-mirrors failed'; \
    (command -v reflector >/dev/null 2>&1 && {bin} reflector --verbose --country '{countries}' --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  elif grep -q 'Artix' /etc/os-release 2>/dev/null; then \
    country_count=$(echo '{countries}' | tr ',' '\\n' | wc -l); \
    if [ \"$country_count\" -ne 1 ]; then \
      echo 'Error: Only one country is allowed for Artix mirror update.'; \
      echo 'Please select a single country.'; \
      exit 1; \
    fi; \
    if ! (pacman -Qi rate-mirrors >/dev/null 2>&1 || command -v rate-mirrors >/dev/null 2>&1); then \
      if ! (pacman -Qi paru >/dev/null 2>&1 || command -v paru >/dev/null 2>&1) && ! (pacman -Qi yay >/dev/null 2>&1 || command -v yay >/dev/null 2>&1); then \
        echo 'Error: rate-mirrors is not installed and no AUR helper (yay/paru) found.'; \
        echo 'Please install yay or paru first, or install rate-mirrors manually.'; \
        exit 1; \
      fi; \
      echo 'rate-mirrors is not installed.'; \
      read -rp 'Do you want to install rate-mirrors from AUR? (y/n): ' install_choice; \
      if [ \"$install_choice\" != \"y\" ] && [ \"$install_choice\" != \"Y\" ]; then \
        echo 'Mirror update cancelled.'; \
        exit 1; \
      fi; \
      if pacman -Qi paru >/dev/null 2>&1 || command -v paru >/dev/null 2>&1; then \
        paru -S --needed --noconfirm rate-mirrors || exit 1; \
      elif pacman -Qi yay >/dev/null 2>&1 || command -v yay >/dev/null 2>&1; then \
        yay -S --needed --noconfirm rate-mirrors || exit 1; \
      fi; \
    fi; \
    {bin} cp /etc/pacman.d/mirrorlist /etc/pacman.d/mirrorlist.backup.$(date +%Y%m%d_%H%M%S) || exit 1; \
    rate-mirrors --protocol=https --allow-root --entry-country='{countries}' --country-neighbors-per-country=3 --top-mirrors-number-to-retest={count} --max-jumps=10 artix | {bin} tee /etc/pacman.d/mirrorlist || exit 1; \
  else \
    (command -v reflector >/dev/null 2>&1 && {bin} reflector --verbose --country '{countries}' --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  fi)"
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::state::AppState;

    #[test]
    /// What: Validate canonical repository toggles deny disabled repositories while permitting enabled ones.
    ///
    /// Inputs:
    /// - `app`: Application state with `core` enabled and other official toggles disabled.
    ///
    /// Output:
    /// - `repo_toggle_for` allows `core` entries but rejects `extra` and `multilib`.
    ///
    /// Details:
    /// - Ensures the per-repository gate respects the individual boolean flags.
    fn repo_toggle_respects_individual_flags() {
        let app = AppState {
            results_filter_show_core: true,
            results_filter_show_extra: false,
            results_filter_show_multilib: false,
            results_filter_show_eos: false,
            results_filter_show_cachyos: false,
            results_filter_show_artix: false,
            results_filter_show_artix_omniverse: false,
            results_filter_show_artix_universe: false,
            results_filter_show_artix_lib32: false,
            results_filter_show_artix_galaxy: false,
            results_filter_show_artix_world: false,
            results_filter_show_artix_system: false,
            ..Default::default()
        };

        assert!(repo_toggle_for("core", &app));
        assert!(!repo_toggle_for("extra", &app));
        assert!(!repo_toggle_for("multilib", &app));
    }

    #[test]
    /// What: Ensure unknown official repositories require every official toggle to be enabled.
    ///
    /// Inputs:
    /// - `app`: Application state with all official flags on, then one flag disabled.
    ///
    /// Output:
    /// - Unknown repository accepted when fully enabled and rejected once any flag is turned off.
    ///
    /// Details:
    /// - Exercises the fallback clause guarding unfamiliar repositories.
    fn repo_toggle_unknown_only_with_full_whitelist() {
        let mut app = AppState {
            results_filter_show_core: true,
            results_filter_show_extra: true,
            results_filter_show_multilib: true,
            results_filter_show_eos: true,
            results_filter_show_cachyos: true,
            results_filter_show_artix: true,
            results_filter_show_artix_omniverse: true,
            results_filter_show_artix_universe: true,
            results_filter_show_artix_lib32: true,
            results_filter_show_artix_galaxy: true,
            results_filter_show_artix_world: true,
            results_filter_show_artix_system: true,
            ..Default::default()
        };

        assert!(repo_toggle_for("unlisted", &app));

        app.results_filter_show_multilib = false;
        assert!(!repo_toggle_for("unlisted", &app));
    }

    #[test]
    /// What: Confirm label helper emits ecosystem-specific aliases for recognised repositories.
    ///
    /// Inputs:
    /// - Repository/name permutations covering `EndeavourOS`, `CachyOS`, `Artix Linux` (with specific repo labels), `Manjaro`, and a generic repo.
    ///
    /// Output:
    /// - Labels reduce to `EOS`, `CachyOS`, `OMNI`, `UNI` (for specific Artix repos), `Manjaro`, and the original repo name respectively.
    ///
    /// Details:
    /// - Validates the Manjaro heuristic via package name and the repo classification helpers.
    /// - Confirms specific Artix repos return their specific labels (OMNI, UNI, etc.) rather than the generic "Artix" label.
    fn label_for_official_prefers_special_cases() {
        assert_eq!(label_for_official("endeavouros", "pkg", ""), "EOS");
        assert_eq!(label_for_official("cachyos-extra", "pkg", ""), "CachyOS");
        assert_eq!(label_for_official("omniverse", "pkg", ""), "OMNI");
        assert_eq!(label_for_official("universe", "pkg", ""), "UNI");
        assert_eq!(label_for_official("blackarch", "pkg", ""), "BlackArch");
        assert_eq!(label_for_official("extra", "manjaro-kernel", ""), "Manjaro");
        assert_eq!(label_for_official("core", "glibc", ""), "core");
    }

    #[test]
    /// What: Validate `BlackArch` toggle routes through its dedicated flag.
    ///
    /// Inputs:
    /// - `app`: Application state with `BlackArch` enabled, then disabled.
    ///
    /// Output:
    /// - `repo_toggle_for("blackarch", ...)` respects the `results_filter_show_blackarch` flag.
    ///
    /// Details:
    /// - Exercises the `BlackArch` branch in `repo_toggle_for`.
    fn repo_toggle_blackarch_flag() {
        let mut app = AppState {
            results_filter_show_blackarch: true,
            ..Default::default()
        };
        assert!(repo_toggle_for("blackarch", &app));
        assert!(repo_toggle_for("BlackArch", &app));

        app.results_filter_show_blackarch = false;
        assert!(!repo_toggle_for("blackarch", &app));
    }

    #[test]
    /// What: Ensure unknown-repo fallback includes `BlackArch` in its all-on requirement.
    ///
    /// Inputs:
    /// - `app`: Application state with all official flags on, then `BlackArch` disabled.
    ///
    /// Output:
    /// - Unknown repo rejected when `BlackArch` flag is off.
    ///
    /// Details:
    /// - Verifies the extended conjunction in the fallback clause.
    fn repo_toggle_unknown_includes_blackarch_flag() {
        let mut app = AppState {
            results_filter_show_core: true,
            results_filter_show_extra: true,
            results_filter_show_multilib: true,
            results_filter_show_eos: true,
            results_filter_show_cachyos: true,
            results_filter_show_artix: true,
            results_filter_show_artix_omniverse: true,
            results_filter_show_artix_universe: true,
            results_filter_show_artix_lib32: true,
            results_filter_show_artix_galaxy: true,
            results_filter_show_artix_world: true,
            results_filter_show_artix_system: true,
            results_filter_show_blackarch: true,
            ..Default::default()
        };
        assert!(repo_toggle_for("unlisted", &app));

        app.results_filter_show_blackarch = false;
        assert!(!repo_toggle_for("unlisted", &app));
    }

    #[test]
    /// What: Repos mapped via `repos.conf` respect `results_filter_dynamic`.
    ///
    /// Inputs:
    /// - `app`: Custom repo name `myvendor` with canonical filter `vendor_pkgs` toggled off.
    ///
    /// Output:
    /// - `repo_toggle_for("myvendor", ...)` is `false` when dynamic map disables the filter.
    ///
    /// Details:
    /// - Builtin branches take precedence; this exercises the dynamic `repos.conf` path only.
    fn repo_toggle_respects_repos_conf_dynamic_filter() {
        let mut by_name = HashMap::new();
        by_name.insert("myvendor".to_string(), "vendor_pkgs".to_string());
        let mut dynamic = HashMap::new();
        dynamic.insert("vendor_pkgs".to_string(), false);
        let app = AppState {
            repo_results_filter_by_name: by_name,
            results_filter_dynamic: dynamic,
            ..Default::default()
        };
        assert!(!repo_toggle_for("myvendor", &app));
        assert!(!repo_toggle_for("MyVendor", &app));
    }

    #[test]
    /// What: Ensure the Worldwide variant embeds the Manjaro fasttrack workflow.
    ///
    /// Inputs:
    /// - `countries`: `"Worldwide"` to trigger the fasttrack branch.
    /// - `count`: `8` mirror entries requested.
    ///
    /// Output:
    /// - Command string contains the Manjaro detection guard plus the fasttrack invocation with the requested count.
    ///
    /// Details:
    /// - Verifies the script includes the fasttrack invocation with the requested count.
    fn mirror_update_worldwide_includes_fasttrack_path() {
        let cmd = mirror_update_command("Worldwide", 8).expect("mirror command");
        assert!(cmd.contains("grep -q 'Manjaro' /etc/os-release"));
        assert!(cmd.contains("pacman-mirrors --fasttrack 8"));
    }

    #[test]
    /// What: Verify region-specific invocation propagates the provided country list to reflector helpers.
    ///
    /// Inputs:
    /// - `countries`: `"Germany,France"` representing a comma-separated selection.
    /// - `count`: Arbitrary `5`, unused in the non-Worldwide branch.
    ///
    /// Output:
    /// - Command string includes the quoted country list for both Manjaro rank and reflector fallback branches.
    ///
    /// Details:
    /// - Confirms the `EndeavourOS` clause still emits the reflector call with the customized country list.
    fn mirror_update_regional_propagates_country_argument() {
        let bin = crate::logic::privilege::active_tool()
            .expect("privilege tool")
            .binary_name();
        let countries = "Germany,France";
        let cmd = mirror_update_command(countries, 5).expect("mirror command");
        assert!(cmd.contains("--country 'Germany,France'"));
        assert!(cmd.contains("grep -q 'EndeavourOS' /etc/os-release"));
        assert!(
            cmd.contains(&format!(
                "{bin} reflector --verbose --country 'Germany,France'"
            )),
            "expected '{bin} reflector ...' in: {cmd}"
        );
    }

    #[test]
    /// What: Confirm `EndeavourOS` and `CachyOS` branches retain their helper invocations and fallback messaging.
    ///
    /// Inputs:
    /// - `countries`: `"Worldwide"` to pass through every branch without country filtering.
    /// - `count`: `3`, checking the value does not affect non-Manjaro logic.
    ///
    /// Output:
    /// - Command string references both `eos-rankmirrors` and `cachyos-rate-mirrors` along with their retry echo messages.
    ///
    /// Details:
    /// - Guards against accidental removal of distro-specific tooling when modifying the mirrored script.
    fn mirror_update_includes_distro_specific_helpers() {
        let bin = crate::logic::privilege::active_tool()
            .expect("privilege tool")
            .binary_name();
        let cmd = mirror_update_command("Worldwide", 3).expect("mirror command");
        assert!(
            cmd.contains(&format!("{bin} eos-rankmirrors")),
            "expected '{bin} eos-rankmirrors' in: {cmd}"
        );
        assert!(cmd.contains("cachyos-rate-mirrors"));
        assert!(cmd.contains("echo 'reflector not found; skipping mirror update'"));
    }

    #[test]
    /// What: Ensure Artix detection and rate-mirrors workflow is included in Worldwide variant.
    ///
    /// Inputs:
    /// - `countries`: `"Worldwide"` to trigger the Worldwide branch.
    /// - `count`: `10` top mirrors number for rate-mirrors.
    ///
    /// Output:
    /// - Command string contains Artix detection guard and rate-mirrors invocation without --entry-country.
    ///
    /// Details:
    /// - Verifies Artix branch uses rate-mirrors without --entry-country for Worldwide selection.
    fn mirror_update_worldwide_includes_artix_path() {
        let cmd = mirror_update_command("Worldwide", 10).expect("mirror command");
        assert!(cmd.contains("grep -q 'Artix' /etc/os-release"));
        assert!(cmd.contains("rate-mirrors"));
        assert!(cmd.contains("--top-mirrors-number-to-retest=10"));
        assert!(cmd.contains("--allow-root"));
        assert!(cmd.contains(" artix"));
    }

    #[test]
    /// What: Verify Artix regional invocation includes country validation and rate-mirrors with --entry-country.
    ///
    /// Inputs:
    /// - `countries`: `"Germany"` representing a single country selection.
    /// - `count`: `5` top mirrors number for rate-mirrors.
    ///
    /// Output:
    /// - Command string includes Artix detection, country count validation, and rate-mirrors with --entry-country.
    ///
    /// Details:
    /// - Confirms the Artix clause validates single country and uses --entry-country parameter.
    fn mirror_update_artix_regional_includes_country_validation() {
        let cmd = mirror_update_command("Germany", 5).expect("mirror command");
        assert!(cmd.contains("grep -q 'Artix' /etc/os-release"));
        assert!(cmd.contains("--entry-country='Germany'"));
        assert!(cmd.contains("Only one country is allowed"));
        assert!(cmd.contains("country_count=$(echo 'Germany'"));
        assert!(cmd.contains("--top-mirrors-number-to-retest=5"));
    }

    #[test]
    /// What: Confirm Artix branch includes rate-mirrors installation check and AUR helper detection.
    ///
    /// Inputs:
    /// - `countries`: `"Germany"` to trigger Artix branch.
    /// - `count`: `8` top mirrors number.
    ///
    /// Output:
    /// - Command string includes checks for rate-mirrors, yay, and paru, plus installation prompt.
    ///
    /// Details:
    /// - Validates that the script checks for rate-mirrors and AUR helpers before proceeding.
    fn mirror_update_artix_includes_installation_checks() {
        let cmd = mirror_update_command("Germany", 8).expect("mirror command");
        assert!(cmd.contains("pacman -Qi rate-mirrors"));
        assert!(cmd.contains("pacman -Qi paru"));
        assert!(cmd.contains("pacman -Qi yay"));
        assert!(cmd.contains("command -v rate-mirrors"));
        assert!(cmd.contains("command -v paru"));
        assert!(cmd.contains("command -v yay"));
        assert!(cmd.contains("Do you want to install rate-mirrors from AUR?"));
        assert!(cmd.contains("Mirror update cancelled"));
    }

    #[test]
    /// What: Verify Artix branch creates backup of mirrorlist before updating.
    ///
    /// Inputs:
    /// - `countries`: `"Germany"` to trigger Artix branch.
    /// - `count`: `5` top mirrors number.
    ///
    /// Output:
    /// - Command string includes backup creation with timestamp.
    ///
    /// Details:
    /// - Ensures mirrorlist backup is created before rate-mirrors execution.
    fn mirror_update_artix_includes_backup() {
        let bin = crate::logic::privilege::active_tool()
            .expect("privilege tool")
            .binary_name();
        let cmd = mirror_update_command("Germany", 5).expect("mirror command");
        assert!(
            cmd.contains(&format!("{bin} cp /etc/pacman.d/mirrorlist")),
            "expected '{bin} cp ...' in: {cmd}"
        );
        assert!(cmd.contains("mirrorlist.backup"));
        assert!(cmd.contains("date +%Y%m%d_%H%M%S"));
    }
}
