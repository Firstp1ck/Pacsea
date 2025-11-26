//! Distro-specific helpers for events layer.

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
pub fn mirror_update_command(countries: &str, count: u16) -> String {
    if countries.eq("Worldwide") {
        format!(
            "(if grep -q 'Manjaro' /etc/os-release 2>/dev/null; then \
    ((command -v pacman-mirrors >/dev/null 2>&1) || sudo pacman -Qi pacman-mirrors >/dev/null 2>&1 || sudo pacman -S --needed --noconfirm pacman-mirrors) && \
    sudo pacman-mirrors --fasttrack {count}; \
  elif grep -q 'EndeavourOS' /etc/os-release 2>/dev/null; then \
    ((command -v eos-rankmirrors >/dev/null 2>&1) || sudo pacman -Qi eos-rankmirrors >/dev/null 2>&1 || sudo pacman -S --needed --noconfirm eos-rankmirrors) && sudo eos-rankmirrors || echo 'eos-rankmirrors failed'; \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  elif grep -q 'CachyOS' /etc/os-release 2>/dev/null; then \
    ((command -v cachyos-rate-mirrors >/dev/null 2>&1) || sudo pacman -Qi cachyos-rate-mirrors >/dev/null 2>&1 || sudo pacman -S --needed --noconfirm cachyos-rate-mirrors) && sudo cachyos-rate-mirrors || echo 'cachyos-rate-mirrors failed'; \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  elif grep -q 'Artix' /etc/os-release 2>/dev/null; then \
    if ! (sudo pacman -Qi rate-mirrors >/dev/null 2>&1 || command -v rate-mirrors >/dev/null 2>&1); then \
      if ! (sudo pacman -Qi paru >/dev/null 2>&1 || command -v paru >/dev/null 2>&1) && ! (sudo pacman -Qi yay >/dev/null 2>&1 || command -v yay >/dev/null 2>&1); then \
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
      if sudo pacman -Qi paru >/dev/null 2>&1 || command -v paru >/dev/null 2>&1; then \
        paru -S --needed --noconfirm rate-mirrors || exit 1; \
      elif sudo pacman -Qi yay >/dev/null 2>&1 || command -v yay >/dev/null 2>&1; then \
        yay -S --needed --noconfirm rate-mirrors || exit 1; \
      fi; \
    fi; \
    sudo cp /etc/pacman.d/mirrorlist /etc/pacman.d/mirrorlist.backup.$(date +%Y%m%d_%H%M%S) || exit 1; \
    if [ \"{countries}\" = \"Worldwide\" ]; then \
      rate-mirrors --protocol=https --allow-root --country-neighbors-per-country=3 --top-mirrors-number-to-retest={count} --max-jumps=10 artix | sudo tee /etc/pacman.d/mirrorlist || exit 1; \
    else \
      country_count=$(echo '{countries}' | tr ',' '\\n' | wc -l); \
      if [ \"$country_count\" -ne 1 ]; then \
        echo 'Error: Only one country is allowed for Artix mirror update.'; \
        echo 'Please select a single country.'; \
        exit 1; \
      fi; \
      rate-mirrors --protocol=https --allow-root --entry-country='{countries}' --country-neighbors-per-country=3 --top-mirrors-number-to-retest={count} --max-jumps=10 artix | sudo tee /etc/pacman.d/mirrorlist || exit 1; \
    fi; \
  else \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  fi)"
        )
    } else {
        format!(
            "(if grep -q 'Manjaro' /etc/os-release 2>/dev/null; then \
    ((command -v pacman-mirrors >/dev/null 2>&1) || sudo pacman -Qi pacman-mirrors >/dev/null 2>&1 || sudo pacman -S --needed --noconfirm pacman-mirrors) && \
    sudo pacman-mirrors --method rank --country '{countries}'; \
  elif grep -q 'EndeavourOS' /etc/os-release 2>/dev/null; then \
    ((command -v eos-rankmirrors >/dev/null 2>&1) || sudo pacman -Qi eos-rankmirrors >/dev/null 2>&1 || sudo pacman -S --needed --noconfirm eos-rankmirrors) && sudo eos-rankmirrors || echo 'eos-rankmirrors failed'; \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --country '{countries}' --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  elif grep -q 'CachyOS' /etc/os-release 2>/dev/null; then \
    ((command -v cachyos-rate-mirrors >/dev/null 2>&1) || sudo pacman -Qi cachyos-rate-mirrors >/dev/null 2>&1 || sudo pacman -S --needed --noconfirm cachyos-rate-mirrors) && sudo cachyos-rate-mirrors || echo 'cachyos-rate-mirrors failed'; \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --country '{countries}' --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  elif grep -q 'Artix' /etc/os-release 2>/dev/null; then \
    country_count=$(echo '{countries}' | tr ',' '\\n' | wc -l); \
    if [ \"$country_count\" -ne 1 ]; then \
      echo 'Error: Only one country is allowed for Artix mirror update.'; \
      echo 'Please select a single country.'; \
      exit 1; \
    fi; \
    if ! (sudo pacman -Qi rate-mirrors >/dev/null 2>&1 || command -v rate-mirrors >/dev/null 2>&1); then \
      if ! (sudo pacman -Qi paru >/dev/null 2>&1 || command -v paru >/dev/null 2>&1) && ! (sudo pacman -Qi yay >/dev/null 2>&1 || command -v yay >/dev/null 2>&1); then \
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
      if sudo pacman -Qi paru >/dev/null 2>&1 || command -v paru >/dev/null 2>&1; then \
        paru -S --needed --noconfirm rate-mirrors || exit 1; \
      elif sudo pacman -Qi yay >/dev/null 2>&1 || command -v yay >/dev/null 2>&1; then \
        yay -S --needed --noconfirm rate-mirrors || exit 1; \
      fi; \
    fi; \
    sudo cp /etc/pacman.d/mirrorlist /etc/pacman.d/mirrorlist.backup.$(date +%Y%m%d_%H%M%S) || exit 1; \
    rate-mirrors --protocol=https --allow-root --entry-country='{countries}' --country-neighbors-per-country=3 --top-mirrors-number-to-retest={count} --max-jumps=10 artix | sudo tee /etc/pacman.d/mirrorlist || exit 1; \
  else \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --country '{countries}' --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  fi)"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let cmd = mirror_update_command("Worldwide", 8);
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
        let countries = "Germany,France";
        let cmd = mirror_update_command(countries, 5);
        assert!(cmd.contains("--country 'Germany,France'"));
        assert!(cmd.contains("grep -q 'EndeavourOS' /etc/os-release"));
        assert!(cmd.contains("sudo reflector --verbose --country 'Germany,France'"));
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
        let cmd = mirror_update_command("Worldwide", 3);
        assert!(cmd.contains("sudo eos-rankmirrors"));
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
        let cmd = mirror_update_command("Worldwide", 10);
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
        let cmd = mirror_update_command("Germany", 5);
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
        let cmd = mirror_update_command("Germany", 8);
        assert!(cmd.contains("sudo pacman -Qi rate-mirrors"));
        assert!(cmd.contains("sudo pacman -Qi paru"));
        assert!(cmd.contains("sudo pacman -Qi yay"));
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
        let cmd = mirror_update_command("Germany", 5);
        assert!(cmd.contains("sudo cp /etc/pacman.d/mirrorlist"));
        assert!(cmd.contains("mirrorlist.backup"));
        assert!(cmd.contains("date +%Y%m%d_%H%M%S"));
    }
}
