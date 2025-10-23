use crate::state::{PackageItem, Source};

/// Build a shell command to install `item` and indicate whether `sudo` is used.
///
/// Returns `(command_string, uses_sudo)`.
pub fn build_install_command(
    item: &PackageItem,
    password: Option<&str>,
    dry_run: bool,
) -> (String, bool) {
    match &item.source {
        Source::Official { .. } => {
            let base_cmd = format!("pacman -S --needed --noconfirm {}", item.name);
            let hold_tail = "; echo; echo 'Finished.'; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";
            if dry_run {
                let bash = format!("echo DRY RUN: sudo {base_cmd}{hold_tail}");
                return (bash, true);
            }
            let pass = password.unwrap_or("");
            if pass.is_empty() {
                let bash = format!("sudo {base_cmd}{hold_tail}");
                (bash, true)
            } else {
                let escaped = pass.replace('\'', "'\"'\"'\''");
                let pipe = format!("echo '{escaped}' | ");
                let bash = format!("{pipe}sudo -S {base_cmd}{hold_tail}");
                (bash, true)
            }
        }
        Source::Aur => {
            let hold_tail = "; echo; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";
            let aur_cmd = if dry_run {
                format!(
                    "echo DRY RUN: paru -S --needed --noconfirm {n} || yay -S --needed --noconfirm {n}{hold}",
                    n = item.name,
                    hold = hold_tail
                )
            } else {
                format!(
                    "(command -v paru >/dev/null 2>&1 && paru -S --needed --noconfirm {n}) || (command -v yay >/dev/null 2>&1 && yay -S --needed --noconfirm {n}) || (echo 'No AUR helper (paru/yay) found.'; echo; echo 'Choose AUR helper to install:'; echo '  1) paru'; echo '  2) yay'; echo '  3) cancel'; read -rp 'Enter 1/2/3: ' choice; case \"$choice\" in 1) git clone https://aur.archlinux.org/paru.git && cd paru && makepkg -si ;; 2) git clone https://aur.archlinux.org/yay.git && cd yay && makepkg -si ;; *) echo 'Cancelled.'; exit 1 ;; esac; if command -v paru >/dev/null 2>&1; then paru -S --needed --noconfirm {n}; elif command -v yay >/dev/null 2>&1; then yay -S --needed --noconfirm {n}; else echo 'AUR helper installation failed or was cancelled.'; exit 1; fi){hold}",
                    n = item.name,
                    hold = hold_tail
                )
            };
            (aur_cmd, false)
        }
    }
}
