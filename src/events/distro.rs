//! Distro-specific helpers for events layer.

/// What: Build a shell snippet to refresh pacman mirrors depending on the detected distro.
///
/// Inputs:
/// - `countries`: Either "Worldwide" to auto-rank mirrors, or a comma-separated list used by the underlying tool
/// - `count`: Mirror count used by Manjaro fasttrack when `countries` is "Worldwide"
///
/// Output:
/// - Shell command string suitable to run in a terminal
///
/// Details:
/// - On Manjaro (detected via /etc/os-release), ensures pacman-mirrors exists, then:
///   - Worldwide: `pacman-mirrors --fasttrack {count}` followed by `pacman -Syy`
///   - Countries: `pacman-mirrors --method rank --country '{countries}'` followed by `pacman -Syy`
/// - On EndeavourOS, ensures `eos-rankmirrors` is installed (retry once after `pacman -Syy` on failure), runs it (retry once after `pacman -Syy` on failure), then runs `reflector`.
/// - On CachyOS, ensures `cachyos-rate-mirrors` is installed (retry once after `pacman -Syy` on failure), runs it (retry once after `pacman -Syy` on failure), then runs `reflector`.
/// - Otherwise, attempts to use `reflector` to write `/etc/pacman.d/mirrorlist`; if not found, prints a notice.
pub fn mirror_update_command(countries: &str, count: u16) -> String {
    if countries.eq("Worldwide") {
        format!(
            "(if grep -q 'Manjaro' /etc/os-release 2>/dev/null; then \
    ((command -v pacman-mirrors >/dev/null 2>&1) || sudo pacman -Qi pacman-mirrors >/dev/null 2>&1 || sudo pacman -S --needed --noconfirm pacman-mirrors || (sudo pacman -Syy && sudo pacman -S --needed --noconfirm pacman-mirrors)) && \
    sudo pacman-mirrors --fasttrack {count} && \
    sudo pacman -Syy; \
  elif grep -q 'EndeavourOS' /etc/os-release 2>/dev/null; then \
    ((command -v eos-rankmirrors >/dev/null 2>&1) || sudo pacman -Qi eos-rankmirrors >/dev/null 2>&1 || sudo pacman -S --needed --noconfirm eos-rankmirrors || (sudo pacman -Syy && sudo pacman -S --needed --noconfirm eos-rankmirrors)) && (sudo eos-rankmirrors || (sudo pacman -Syy && sudo eos-rankmirrors)) || echo 'eos-rankmirrors failed'; \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  elif grep -q 'CachyOS' /etc/os-release 2>/dev/null; then \
    ((command -v cachyos-rate-mirrors >/dev/null 2>&1) || sudo pacman -Qi cachyos-rate-mirrors >/dev/null 2>&1 || sudo pacman -S --needed --noconfirm cachyos-rate-mirrors || (sudo pacman -Syy && sudo pacman -S --needed --noconfirm cachyos-rate-mirrors)) && (sudo cachyos-rate-mirrors || (sudo pacman -Syy && sudo cachyos-rate-mirrors)) || echo 'cachyos-rate-mirrors failed'; \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  else \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  fi)"
        )
    } else {
        format!(
            "(if grep -q 'Manjaro' /etc/os-release 2>/dev/null; then \
    ((command -v pacman-mirrors >/dev/null 2>&1) || sudo pacman -Qi pacman-mirrors >/dev/null 2>&1 || sudo pacman -S --needed --noconfirm pacman-mirrors || (sudo pacman -Syy && sudo pacman -S --needed --noconfirm pacman-mirrors)) && \
    sudo pacman-mirrors --method rank --country '{countries}' && \
    sudo pacman -Syy; \
  elif grep -q 'EndeavourOS' /etc/os-release 2>/dev/null; then \
    ((command -v eos-rankmirrors >/dev/null 2>&1) || sudo pacman -Qi eos-rankmirrors >/dev/null 2>&1 || sudo pacman -S --needed --noconfirm eos-rankmirrors || (sudo pacman -Syy && sudo pacman -S --needed --noconfirm eos-rankmirrors)) && (sudo eos-rankmirrors || (sudo pacman -Syy && sudo eos-rankmirrors)) || echo 'eos-rankmirrors failed'; \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --country '{countries}' --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  elif grep -q 'CachyOS' /etc/os-release 2>/dev/null; then \
    ((command -v cachyos-rate-mirrors >/dev/null 2>&1) || sudo pacman -Qi cachyos-rate-mirrors >/dev/null 2>&1 || sudo pacman -S --needed --noconfirm cachyos-rate-mirrors || (sudo pacman -Syy && sudo pacman -S --needed --noconfirm cachyos-rate-mirrors)) && (sudo cachyos-rate-mirrors || (sudo pacman -Syy && sudo cachyos-rate-mirrors)) || echo 'cachyos-rate-mirrors failed'; \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --country '{countries}' --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
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
    /// - Also verifies the script retains the trailing pacman refresh step used after ranking mirrors.
    fn mirror_update_worldwide_includes_fasttrack_path() {
        let cmd = mirror_update_command("Worldwide", 8);
        assert!(cmd.contains("grep -q 'Manjaro' /etc/os-release"));
        assert!(cmd.contains("pacman-mirrors --fasttrack 8"));
        assert!(cmd.contains("sudo pacman -Syy;"));
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
    /// - Confirms the EndeavourOS clause still emits the reflector call with the customized country list.
    fn mirror_update_regional_propagates_country_argument() {
        let countries = "Germany,France";
        let cmd = mirror_update_command(countries, 5);
        assert!(cmd.contains("--country 'Germany,France'"));
        assert!(cmd.contains("grep -q 'EndeavourOS' /etc/os-release"));
        assert!(cmd.contains("sudo reflector --verbose --country 'Germany,France'"));
    }

    #[test]
    /// What: Confirm EndeavourOS and CachyOS branches retain their helper invocations and fallback messaging.
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
}
