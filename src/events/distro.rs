//! Distro-specific helpers for events layer.

/// Build mirror update shell snippet depending on distro.
/// Uses pacman-mirrors for Manjaro, reflector for Arch/others.
pub fn mirror_update_command(country: &str) -> String {
    if country.eq("Worldwide") {
        "(if grep -q 'Manjaro' /etc/os-release 2>/dev/null; then \
    (command -v pacman-mirrors >/dev/null 2>&1 || sudo pacman -S --needed --noconfirm pacman-mirrors) && \
    sudo pacman-mirrors --fasttrack 20 && \
    sudo pacman -Syy; \
  else \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  fi)".to_string()
    } else {
        format!("(if grep -q 'Manjaro' /etc/os-release 2>/dev/null; then \
    (command -v pacman-mirrors >/dev/null 2>&1 || sudo pacman -S --needed --noconfirm pacman-mirrors) && \
    sudo pacman-mirrors --country '{country}' --method rank && \
    sudo pacman -Syy; \
  else \
    (command -v reflector >/dev/null 2>&1 && sudo reflector --verbose --country '{country}' --protocol https --sort rate --latest 20 --download-timeout 6 --save /etc/pacman.d/mirrorlist) || echo 'reflector not found; skipping mirror update'; \
  fi)")
    }
}


