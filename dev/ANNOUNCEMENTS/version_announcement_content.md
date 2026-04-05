## What's New

### Custom and third-party repositories

- Configure extra repos in **`repos.conf`**, edit them from the app, and apply changes when you are ready (with privilege prompts when needed).
- Search and filters can include packages from those repos, with sensible handling when the same package name appears more than once.
- Toggle managed entries on or off; disabled repos are ignored until you enable them again. The repositories screen can refresh with up-to-date status after related dialogs.
- First run seeds a starter file; **`repos_example.conf`** ships as a copy-paste reference.

### After you add a repo

- If packages you installed also exist in the new repo, a short guided flow explains the situation and helps you choose what to do next (including optional cleanup). Preview-only mode stays accurate; canceling or errors should not leave the UI stuck.

### Overlapping names (AUR vs other sources)

- AUR installs go through your helper in a way that avoids wrong-source surprises when a community mirror lists the same name.
- Selecting an AUR hit that also appears as a normal Arch listing can show a one-time warning before you continue.

### Optional: AUR voting

- Vote or unvote AUR packages from search when enabled, using SSH to the AUR.
- Built-in **SSH AUR setup** helps you configure the host entry in your SSH config.
- Honors your SSH command, timeout, and preview-only mode (no fake vote state).

### Optional: PKGBUILD checks

- Run **ShellCheck** and **Namcap** on the selected package build file from the details view when those tools are installed (timeouts and missing tools handled gracefully).
- Switch between the PKGBUILD text and check results in the details pane; settings cover raw output and ShellCheck excludes.

### Bug fixes

- **Repositories:** Stricter validation for paths, server URLs, signing keys, and filter keys; safer behavior when apply is interrupted or does not complete successfully.
- **PKGBUILD checks:** More reliable when starting a check; time limits handled inside the app; clearer messaging when a checker is not installed.
- **Lists and filters:** Better column alignment when names use wide characters (e.g. some non-Latin scripts).
