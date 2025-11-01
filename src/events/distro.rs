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
/// - Otherwise, attempts to use `reflector` to write `/etc/pacman.d/mirrorlist`; if not found, prints a notice.
pub fn mirror_update_command(countries: &str, count: u16) -> String {
    if countries.eq("Worldwide") {
        format!(
            "(if grep -q 'Manjaro' /etc/os-release 2>/dev/null; then \
    (command -v pacman-mirrors >/dev/null 2>&1 || sudo pacman -S --needed --noconfirm pacman-mirrors) && \
    sudo pacman-mirrors --fasttrack {count} && \
    sudo pacman -Syy; \
  else \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  fi)"
        )
    } else {
        format!(
            "(if grep -q 'Manjaro' /etc/os-release 2>/dev/null; then \
    (command -v pacman-mirrors >/dev/null 2>&1 || sudo pacman -S --needed --noconfirm pacman-mirrors) && \
    sudo pacman-mirrors --method rank --country '{countries}' && \
    sudo pacman -Syy; \
  else \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --country '{countries}' --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  fi)"
        )
    }
}
