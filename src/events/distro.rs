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
    ((command -v pacman-mirrors >/dev/null 2>&1) || sudo pacman -S --needed --noconfirm pacman-mirrors || (sudo pacman -Syy && sudo pacman -S --needed --noconfirm pacman-mirrors)) && \
    sudo pacman-mirrors --fasttrack {count} && \
    sudo pacman -Syy; \
  elif grep -q 'EndeavourOS' /etc/os-release 2>/dev/null; then \
    ((command -v eos-rankmirrors >/dev/null 2>&1) || sudo pacman -S --needed --noconfirm eos-rankmirrors || (sudo pacman -Syy && sudo pacman -S --needed --noconfirm eos-rankmirrors)) && (sudo eos-rankmirrors || (sudo pacman -Syy && sudo eos-rankmirrors)) || echo 'eos-rankmirrors failed'; \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  elif grep -q 'CachyOS' /etc/os-release 2>/dev/null; then \
    ((command -v cachyos-rate-mirrors >/dev/null 2>&1) || sudo pacman -S --needed --noconfirm cachyos-rate-mirrors || (sudo pacman -Syy && sudo pacman -S --needed --noconfirm cachyos-rate-mirrors)) && (sudo cachyos-rate-mirrors || (sudo pacman -Syy && sudo cachyos-rate-mirrors)) || echo 'cachyos-rate-mirrors failed'; \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  else \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  fi)"
        )
    } else {
        format!(
            "(if grep -q 'Manjaro' /etc/os-release 2>/dev/null; then \
    ((command -v pacman-mirrors >/dev/null 2>&1) || sudo pacman -S --needed --noconfirm pacman-mirrors || (sudo pacman -Syy && sudo pacman -S --needed --noconfirm pacman-mirrors)) && \
    sudo pacman-mirrors --method rank --country '{countries}' && \
    sudo pacman -Syy; \
  elif grep -q 'EndeavourOS' /etc/os-release 2>/dev/null; then \
    ((command -v eos-rankmirrors >/dev/null 2>&1) || sudo pacman -S --needed --noconfirm eos-rankmirrors || (sudo pacman -Syy && sudo pacman -S --needed --noconfirm eos-rankmirrors)) && (sudo eos-rankmirrors || (sudo pacman -Syy && sudo eos-rankmirrors)) || echo 'eos-rankmirrors failed'; \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --country '{countries}' --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  elif grep -q 'CachyOS' /etc/os-release 2>/dev/null; then \
    ((command -v cachyos-rate-mirrors >/dev/null 2>&1) || sudo pacman -S --needed --noconfirm cachyos-rate-mirrors || (sudo pacman -Syy && sudo pacman -S --needed --noconfirm cachyos-rate-mirrors)) && (sudo cachyos-rate-mirrors || (sudo pacman -Syy && sudo cachyos-rate-mirrors)) || echo 'cachyos-rate-mirrors failed'; \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --country '{countries}' --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  else \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --country '{countries}' --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  fi)"
        )
    }
}
