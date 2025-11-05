## Converting pacsea-bin AUR Package to Flatpak: Concrete Example

This concrete example demonstrates step-by-step conversion of the **pacsea-bin** AUR package into a Flatpak application. Pacsea is a fast, keyboard-friendly Terminal User Interface (TUI) for searching and installing packages from Arch Linux and the AUR.

### Application Overview

**Pacsea-bin** is a pre-compiled binary distribution of Pacsea written in Rust. It's lightweight, has minimal dependencies, and is ideal for Flatpak containerization since it's already a self-contained binary.

![AUR to Flatpak Conversion Process Flow for pacsea-bin](https://ppl-ai-code-interpreter-files.s3.amazonaws.com/web/direct-files/57e10a3e4b947906a4b5524dc4a7d240/9d228943-2362-4f65-aaa5-66e1f203a2a4/0df14c22.png)

AUR to Flatpak Conversion Process Flow for pacsea-bin

### Complete Conversion Plan with Generated Files

### Detailed Manifest File (Annotated)

The following is the complete Flatpak manifest with extensive inline documentation explaining each section:

### Complete Command Reference

Here's a comprehensive set of all commands you'll execute throughout the conversion process:

***

### Execution Summary

**Step 1-3: Preparation**

```bash
sudo pacman -S flatpak flatpak-builder
flatpak install flathub org.freedesktop.Platform/x86_64/24.08
mkdir ~/projects/pacsea-flatpak && cd ~/projects/pacsea-flatpak
```

**Step 4-6: Get Hashes and Create Manifest**

```bash
curl -LO https://github.com/Firstp1ck/Pacsea/releases/download/v0.4.4/pacsea-x86_64-unknown-linux-gnu
sha256sum pacsea-x86_64-unknown-linux-gnu  # Copy this hash to manifest
# Create: com.github.firstp1ck.pacsea.yml (use the manifest file provided)
```

**Step 7-9: Build and Test**

```bash
flatpak-builder build-dir com.github.firstp1ck.pacsea.yml --user --install --force-clean
flatpak run com.github.firstp1ck.pacsea
```

**Step 10: Export to Repository**

```bash
flatpak-builder --repo=pacsea-repo build-dir com.github.firstp1ck.pacsea.yml
flatpak remote-add --user --if-not-exists pacsea-local file:///path/to/pacsea-repo
flatpak install pacsea-local com.github.firstp1ck.pacsea
```


***

### Key Generated Files

1. **com.github.firstp1ck.pacsea.yml** - Main Flatpak manifest containing build instructions, permissions, and module definitions
2. **com.github.firstp1ck.pacsea.desktop** - Desktop entry file (inline in manifest) for application menu integration
3. **com.github.firstp1ck.pacsea.appdata.xml** - AppData metadata (inline in manifest) for app store descriptions
4. **pacsea-repo/** - Generated local Flatpak repository after build
5. **build-dir/** - Generated build directory with compiled artifacts

### Permissions Explained

The manifest defines sandbox permissions that restrict what pacsea can access:


| Permission | Reason | Security Benefit |
| :-- | :-- | :-- |
| `--share=network` | AUR API queries need network access | Isolated from other network activity |
| `--socket=fallback-x11` | TUI needs terminal display | Can't access other X11 windows |
| `--filesystem=host:ro` | Read package databases | Read-only prevents system modification |
| `--env=TERM=xterm-256color` | Proper terminal emulation | Consistent terminal behavior |

### Security Improvements Over AUR Package

Converting pacsea-bin to Flatpak provides:

- **Process Isolation**: Application runs in sandboxed environment, separate from system
- **Permission Granularity**: Explicit filesystem and network access restrictions
- **Filesystem Protection**: Read-only access to system package databases prevents accidental/malicious modifications
- **Update Safety**: Isolated updates don't affect system stability
- **Transparent Security**: Users can see exactly what the application can access


### Troubleshooting Common Issues

**Build fails with hash mismatch**: Recalculate SHA256 using `sha256sum` and update manifest

**Application won't run**: Ensure network permission (`--share=network`) and terminal socket permissions are set

**AUR queries don't work**: Verify `--share=network` is in manifest; rebuild with `--force-clean`

**Permission denied accessing packages**: Add `--filesystem=host:ro` to manifest for read-only system access

### Next Steps

1. **Use the manifest file** provided to get started immediately
2. **Update SHA256 hashes** with actual values from downloaded binaries
3. **Follow the command sequence** in the reference document
4. **Test thoroughly** before distributing to others
5. **Consider submitting to Flathub** for easier distribution

All three comprehensive guides above contain the complete information needed to convert any AUR package to Flatpak, with pacsea-bin as the working example.
<span style="display:none">[^1][^10][^11][^12][^13][^14][^15][^16][^17][^18][^19][^2][^20][^21][^22][^23][^24][^25][^26][^27][^28][^29][^3][^4][^5][^6][^7][^8][^9]</span>

<div align="center">⁂</div>

[^1]: https://aur.archlinux.org/packages/pacsea-bin

[^2]: https://aur.archlinux.org/packages/pacsea-git

[^3]: https://pace.oceansciences.org/applications.htm

[^4]: https://stackoverflow.com/questions/41047207/how-to-include-static-data-files-into-archlinux-aur-package

[^5]: https://www.laseroffice.it/blog/2025/10/11/pacsea-il-nuovo-strumento-tui-per-la-gestione-dei-pacchetti-software-su-arch-linux/

[^6]: https://apps.apple.com/us/app/ipaxera/id432861550

[^7]: https://madskjeldgaard.dk/old-blog/aur-package-workflow/

[^8]: https://github.com/pbs-assess/pacea

[^9]: https://apps.apple.com/fr/app/pixea/id1507782672?mt=12

[^10]: https://forum.manjaro.org/t/determine-dependencies-required-when-writing-a-pkgbuild/43261

[^11]: https://bbs.archlinux.org/viewtopic.php?id=228169

[^12]: https://en.wikipedia.org/wiki/Arch_Linux

[^13]: https://www.reddit.com/r/archlinux/comments/12mre48/package_manager/

[^14]: https://github.com/Firstp1ck/Pacsea

[^15]: https://github.com/FabioLolix/PKGBUILD

[^16]: https://www.reddit.com/r/archlinux/comments/es6dr8/whats_the_best_way_to_install_aur_packages/

[^17]: https://blog.stephane-robert.info/docs/admin-serveurs/linux/pacman/

[^18]: https://aur.archlinux.org/packages/bash-devel-git?all_reqs=1

[^19]: https://aur.archlinux.org/packages/rust-nightly-bin?all_reqs=1\&O=40

[^20]: https://aur.archlinux.org/packages

[^21]: https://sourceforge.net/projects/pacmanager/files/pac-4.0/

[^22]: https://github.com/suyashkumar/getbin

[^23]: http://downloads.pangubox.com:6380/sources/

[^24]: https://stackoverflow.com/questions/71632199/how-to-download-the-latest-binary-release-from-github

[^25]: https://www.reddit.com/r/linux/comments/1o2jjsj/pacsea_arch_package_manager_tui/

[^26]: https://aur.archlinux.org/packages/downgrade?O=30\&PP=10

[^27]: https://ftp.it.p.lodz.pl/mirror/PLD/th/PLD/SRPMS/RPMS/

[^28]: https://github.com/marcosnils/bin

[^29]: https://www.tenforums.com/attachments/tutorials/76594d1461573104-powershell-packagemanagement-oneget-install-apps-command-line-ps-package-management-packages-24-apr-2016.pdf
