# Pacsea demo storyboard

These captioned walkthroughs follow an Arch Linux user from a package search to reviewing changes, maintaining the system, and automating routine queries. Each episode stands on its own. The captions below follow the videos in playback order.

## Versions shown

| Source | What the viewer sees |
| --- | --- |
| `main` at `c2d62f9e` (Pacsea 0.8.2) | The main-branch interface, including the integrated config editor and transaction scanner. This revision follows the v0.8.2 tag and still reports version 0.8.2. Episode 03 identifies later CLI behavior on screen. |
| `feat/config-editor-ux` at `52036f96` | Preview of the redesigned config editor and its navigation. |
| `feat/aur-scan-integrated` at `55ef9248` | Preview of the Pi Scan workspace. This tip includes the editor branch and later changes to scan limits and results. |

The earlier [Part 1 recording](https://youtu.be/QlAh0Fu1Ges) remains available.

## Recording notes

- Use the shipped English bindings. In the package view, the bottom band shows details. `Ctrl+X` opens PKGBUILD. In the Search pane, `Enter` acts on the highlighted result. To review a batch, move focus to Install first.
- In the discovery pilot, keep the same AUR row selected while opening its PKGBUILD, running the checks with `Ctrl+K`, and reading ShellCheck and Namcap with `Ctrl+D`. After the checks, close the recipe, select the official result, and queue it.
- On main, `Shift+O`, then `5`, opens News from the package view. On the feature branches, Pi Scan takes option `5` and News moves to `6`. The preview's `Shift+A` opens Pi Scan for the selected result.
- The command-line scenes use example inputs and outputs. Standalone install, remove, update, and cache-clearing flags dispatch before the TUI starts. Adding `--dry-run` to those commands does not make them safe to execute for a recording.
- For a Preflight scene, set `skip_preflight = false`. The generated default is false, while the checked-in settings example enables skipping. Show one AUR package for the seven-tool scan.
- Mirror tools depend on the Linux distribution. The downgrade picker needs an older package version, and fingerprint authentication needs an interactive PAM setup. Use illustrative package data, example scan findings, and redacted credentials in recreated screens.

## Episodes

[Download all eight MP4s in one ZIP](https://github.com/Firstp1ck/Pacsea/releases/download/walkthroughs-1/pacsea-walkthroughs-1.zip). The numbered filenames follow the order below.

| Episode | Interface | Runtime | Files |
| --- | --- | ---: | --- |
| 01 · Find and review Arch packages | main | 3:29.5 | [Thumbnail](demo-videos/01-discovery/thumbnail.png) · [YouTube description](demo-videos/01-discovery/youtube-description.txt) · [Storyboard](demo-videos/01-discovery/storyboard.json) |
| 02 · Read news and check Arch service status | main | 3:14 | [Thumbnail](demo-videos/02-news/thumbnail.png) · [YouTube description](demo-videos/02-news/youtube-description.txt) · [Storyboard](demo-videos/02-news/storyboard.json) |
| 03 · Use Pacsea at the command line | main | 3:02 | [Thumbnail](demo-videos/03-cli/thumbnail.png) · [YouTube description](demo-videos/03-cli/youtube-description.txt) · [Storyboard](demo-videos/03-cli/storyboard.json) |
| 04 · Review a package transaction | main | 3:02 | [Thumbnail](demo-videos/04-transactions/thumbnail.png) · [YouTube description](demo-videos/04-transactions/youtube-description.txt) · [Storyboard](demo-videos/04-transactions/storyboard.json) |
| 05 · Maintain an Arch installation | main | 3:03 | [Thumbnail](demo-videos/05-maintenance/thumbnail.png) · [YouTube description](demo-videos/05-maintenance/youtube-description.txt) · [Storyboard](demo-videos/05-maintenance/storyboard.json) |
| 06 · Use the Pi Scan workspace | Pi Scan preview | 3:10 | [Thumbnail](demo-videos/06-pi-scan/thumbnail.png) · [YouTube description](demo-videos/06-pi-scan/youtube-description.txt) · [Storyboard](demo-videos/06-pi-scan/storyboard.json) |
| 07 · Set up tools and manage repositories | main | 3:06 | [Thumbnail](demo-videos/07-setup-repositories/thumbnail.png) · [YouTube description](demo-videos/07-setup-repositories/youtube-description.txt) · [Storyboard](demo-videos/07-setup-repositories/storyboard.json) |
| 08 · Edit Pacsea’s configuration | main and editor preview | 3:04 | [Thumbnail](demo-videos/08-config-editor/thumbnail.png) · [YouTube description](demo-videos/08-config-editor/youtube-description.txt) · [Storyboard](demo-videos/08-config-editor/storyboard.json) |

## Shot list

Start times are measured from the beginning of each film. The captions describe the visible action or result.

### 01 · Find and review Arch packages

Find Arch packages in Pacsea. Page through results, edit a search, and narrow it to the repositories you use. Inspect and copy an AUR PKGBUILD, read its static checks, then queue an official package and review its download and disk size in Preflight.

| Start | What viewers see |
| ---: | --- |
| 0:00 | Pacsea: Find and review Arch packages |
| 0:08 | PgDn moves ten rows. PgUp returns to the first package. |
| 0:14.5 | Type a package name to see matching packages. |
| 0:20 | extra means official. AUR packages come from the community. |
| 0:25 | Esc enters Normal mode. h/l selects left or right. d deletes. |
| 0:31.5 | Shift+Del clears search. Ctrl+F turns on fuzzy matching. |
| 0:36.5 | Type a few letters. Names match those letters in order. |
| 0:42 | Choose a past search. Press Enter in history to repeat it. |
| 0:48 | Press Esc to browse. j moves down. k moves up. |
| 0:54.5 | OOD means outdated. ORPHAN means no maintainer. |
| 0:59 | Click AUR to hide its results. Click again to bring them back. |
| 1:04.5 | Open Repos. Uncheck work_tools to hide its package. |
| 1:10.5 | Open Sort. Choose AUR popularity. |
| 1:16 | Select an AUR result. Ctrl+T opens its comments. |
| 1:22.5 | Ctrl+E opens a vote. Esc cancels it. |
| 1:28.5 | Keep the AUR package selected. Ctrl+X opens its PKGBUILD. |
| 1:37.5 | Press Ctrl+X twice. Close and reopen to reload the recipe. |
| 1:44 | Click Copy PKGBUILD. Pacsea confirms the clipboard copy. |
| 1:50 | Ctrl+K runs static checks on the selected PKGBUILD. |
| 1:56 | Ctrl+D cycles results. Read ShellCheck, then Namcap. |
| 2:02 | Build the install list |
| 2:10.25 | Select an official result. Space adds it to the install list. |
| 2:18.25 | Choose a search from history. Space queues a matching package. |
| 2:27.5 | Press /, type a name. Esc exits find. Del removes the row. |
| 2:34 | Shift+Del clears the focused list. In Search, Space adds the package back. |
| 2:42 | Shift+E exports the list. Pacsea shows the saved file’s path. |
| 2:49 | Shift+I opens Import. Use one package name per line. |
| 2:56.75 | F1 opens Help. Find the fuzzy-search shortcut. |
| 3:01 | Focus the Install List. Enter opens Preflight when enabled. |
| 3:08.25 | DL shows download size. Size shows added disk space. |
| 3:13.25 | Press q to close Preflight. Your package stays queued. |
| 3:20.5 | Ready in the install list |

### 02 · Read news and check Arch service status

Use Pacsea to narrow Arch news and advisories to the items you want to read. Compare an update’s package versions, open the original article, manage bookmarks and check Arch service status.

| Start | What viewers see |
| ---: | --- |
| 0:00 | Pacsea: Read and filter Arch news |
| 0:08 | Press r to acknowledge. Enter or Esc shows these notes again. |
| 0:16 | Open Options. Press Shift+O in Normal mode. |
| 0:21 | Choose News management. Press 5 to switch to the feed. |
| 0:26 | Read the feed. Arch news, advisories and package activity. |
| 0:32 | Look for AUR comments. Package updates have their own source. |
| 0:37 | Toggle Updates. Hide official package update stories. |
| 0:43 | Click Advisories. Installed limits alerts to your packages. |
| 0:49 | Choose a date range. Cycle through 7, 30 or 90 days, or all. |
| 0:54 | Choose Unread. Only unread stories remain in this view. |
| 0:59 | Open Sort. Choose how to order the feed. |
| 1:03 | Choose Severity. Critical advisories come first. |
| 1:08 | Read the first advisory. High precedes Medium in this feed. |
| 1:13 | Type ripgrep. Search the news feed for a package. |
| 1:19 | Check the matches. Both an update and a comment match. |
| 1:24 | Press Left for history. Enter restores the selected search. |
| 1:30 | The search returns. Only the matching Arch story remains. |
| 1:34 | Read a story. |
| 1:42 | Select a package update. This example opens the bat update. |
| 1:47 | Compare versions. The News pane shows the change. |
| 1:53 | Open an Arch article. Choose Open in Browser on its details. |
| 1:58 | Read the original article. The link opens the selected story. |
| 2:02 | Press r in Normal mode. A check marks this story as read. |
| 2:07 | Press u for unread. Press t to toggle either way. |
| 2:12 | Press Space to bookmark. The selected article appears here. |
| 2:18 | Press Right for Bookmarks. Enter loads the saved story. |
| 2:23 | Read the saved story. Its title appears in the News pane. |
| 2:27 | Del removes the bookmark. The article stays in the feed. |
| 2:32 | Open the News window. Press Shift+U in News management. |
| 2:38 | Mark the window read. Ctrl+R marks every displayed story. |
| 2:44 | Check the list. Each article now carries a read mark. |
| 2:48 | Check Arch service status. Shift+S opens the public status page. |
| 2:52 | Read the status page. This service snapshot is illustrative. |
| 2:56 | Return to packages. Press Shift+O, then 4. |
| 3:01 | Package results return. The install list replaces Bookmarks. |
| 3:05 | Your news review is done. |

### 03 · Use Pacsea at the command line

Search Arch packages from the shell, inspect JSON with jq and save an installed-package list. See the syntax for install, remove and update operations, then read Arch news and open the interactive browser in dry-run mode.

| Start | What viewers see |
| ---: | --- |
| 0:00 | Pacsea: Work from the command line |
| 0:08 | Check your version. Use --help to see the flags in your build. |
| 0:15 | Search for a name. Compare repository and AUR matches. |
| 0:24 | Read a JSON result. jq selects the first package in the array. |
| 0:34 | Check the envelope. Identify the command and schema version. |
| 0:42 | List explicit leaves. The default matches pacman -Qetq. |
| 0:49 | Spell out --exp. The explicit leaf list stays the same. |
| 0:54 | List the remaining packages. --imp excludes the explicit leaves. |
| 1:01 | List every package. --all combines both groups. |
| 1:08 | Save names to a file. Read it back, one package per line. |
| 1:17 | Review before Enter. |
| 1:25 | Install by name. List the packages after -i. |
| 1:31 | Read names from a file. Capital -I takes one name per line. |
| 1:38 | Remove by name. Review each name after -r. |
| 1:44 | Remove from a file. Capital -R reads the reviewed list. |
| 1:51 | Request a system update. The -u command syncs and upgrades. |
| 1:57 | Refresh mirrors first. --mirrors uses your configured countries. |
| 2:04 | Read unread Arch news. The output includes the original URL. |
| 2:12 | Choose read articles. Use -a with --news for both groups. |
| 2:19 | Choose a config directory. The override includes caches and logs. |
| 2:26 | Set debug logging. Verbose (-v) selects the same level. |
| 2:32 | Disable update colors. This still requests a real update. |
| 2:38 | Check the cache option. Submitting it clears cached data. |
| 2:44 | Rehearse in the TUI. Direct installs and removals are live. |
| 2:53 | Choose what runs. |

### 04 · Review a package transaction

Review an official package and an AUR package together in Pacsea. Follow their dependencies, files and service choices through Preflight, select from seven optional AUR checks, then compare a dry-run preview with the prompts and summary of an example install.

| Start | What viewers see |
| ---: | --- |
| 0:00 | Pacsea: Review a package transaction |
| 0:09 | Enable Preflight Set skip_preflight to false. |
| 0:15 | Enter reviews one result Esc returns to the package browser. |
| 0:20 | Space adds syncthing The official package appears in Install List. |
| 0:26 | Add the AUR package Select fieldnotes-git and press Space. |
| 0:31 | Focus the install list Enter reviews both queued packages. |
| 0:36 | Read the risk factors One AUR package contributes two points. |
| 0:44 | Open Deps Space reveals the missing build tool. |
| 0:51 | Open Files Expand syncthing to inspect its installed paths. |
| 0:58 | Space defers restart The service can keep running until later. |
| 1:05 | Inspect Sandbox Expand the AUR package’s build dependencies. |
| 1:12 | Scan the AUR target |
| 1:20 | Press s in Preflight Choose which of seven AUR checks to run. |
| 1:27 | Space toggles VirusTotal This example skips the hash lookup. |
| 1:33 | Enter starts selected checks Pacsea scans the first AUR target. |
| 1:39 | Read the separate terminal aur-sleuth has its own audit output. |
| 1:44 | Read the scan summary Check which tools contributed to the score. |
| 1:52 | Enter returns to Preflight The selected AUR package stays in view. |
| 1:57 | Press d for dry-run The mode changes before you proceed. |
| 2:03 | Press p to preview Both packages appear in the simulated plan. |
| 2:09 | Review a live transaction |
| 2:17 | Check a reinstall The same syncthing version is already installed. |
| 2:22 | Read partial-update warnings This separate example updates openssl alone. |
| 2:28 | Authenticate a live install Read the package list in the prompt. |
| 2:33 | Interactive authentication The terminal handles the prompt through PAM. |
| 2:39 | Press l for verbosity Follow both package changes and their hooks. |
| 2:45 | Read the transaction summary Check file changes and deferred restarts. |
| 2:53 | Read the changes |

### 05 · Maintain an Arch installation

Use Pacsea to review a removal with a dependent package, choose an older package version, and select individual upgrades. Open Update System to compare official and AUR updates, cache cleanup and the mirror tools used by different Arch-based distributions.

| Start | What viewers see |
| ---: | --- |
| 0:00 | Pacsea: Maintain an Arch installation |
| 0:09 | Include explicit installs Use all to include packages with dependents. |
| 0:14 | Open Options Shift+O opens the menu from Normal mode. |
| 0:19 | Press 1 Browse the installed package list. |
| 0:24 | Space queues a removal fieldnotes enters the Remove List. |
| 0:29 | Ctrl+Space queues a downgrade Put syncthing in the other list. |
| 0:34 | Focus Remove Tab until the Remove List has focus. |
| 0:38 | Read the removal impact fieldnotes-export still needs fieldnotes. |
| 0:45 | Press m to compare Inspect the flags and affected packages. |
| 0:52 | Focus Downgrade Leave the removal plan and select syncthing. |
| 0:56 | Enter opens Preflight Check the installed version before proceeding. |
| 1:01 | Choose an older version The downgrade tool lists available builds. |
| 1:08 | Choose a package batch |
| 1:16 | Open Available Updates Shift+U shows installed and offered versions. |
| 1:22 | Move down with j Compare the language pack with Firefox. |
| 1:26 | Filter with / Type firefox to keep the matching rows. |
| 1:31 | Space marks each row Select the browser and its language pack. |
| 1:36 | Enter reviews both upgrades Their old and new versions appear together. |
| 1:42 | Plan a full update |
| 1:50 | Open Update System Shift+O then 2 opens the task list. |
| 1:56 | Choose the sync mode Left or Right switches to a forced refresh. |
| 2:01 | Choose cache cleanup Space changes the selected cache action. |
| 2:06 | Enable mirror updates Select the mirror task before choosing a region. |
| 2:11 | Choose country and count Use the arrows and + to adjust the list. |
| 2:16 | Arch uses reflector Germany limits the search to one country. |
| 2:21 | Manjaro uses pacman-mirrors Choose worldwide or country-specific ranking. |
| 2:26 | Artix uses rate-mirrors Pick a single entry country for ranking. |
| 2:32 | Rank distribution mirrors Each tool ranks its distribution’s mirrors. |
| 2:37 | Enter runs selected tasks This example shows the official update first. |
| 2:42 | Continue with AUR updates Pacsea uses paru or yay after the official update. |
| 2:47 | If pacman fails Choose whether the AUR update should still run. |
| 2:54 | Decide what runs next |

### 06 · Use the Pi Scan workspace

Preview the Pi Scan workspace in Pacsea. Configure a provider route and spending limits, follow selected AUR packages through the queue, inspect the evidence behind a finding, and recheck the package identity before returning to install review.

| Start | What viewers see |
| ---: | --- |
| 0:00 | Pacsea: Explore Pi Scan (feature preview) |
| 0:09 | Options then 5 Open Pi Scan without adding a target. |
| 0:14 | Shift+A on fieldnotes-git Open setup for the selected AUR package. |
| 0:19 | Verify Pi with Enter Read the installed version before any scan. |
| 0:24 | Choose an exact route Select a provider and model advertised by Pi. |
| 0:29 | Read the provider disclosure Check where package data goes and its cost. |
| 0:35 | Allow foreground scans Paid runs you start need separate consent. |
| 0:40 | Allow observation Pacsea may watch for package updates. |
| 0:44 | Choose paid background access This is a separate decision from observation. |
| 0:49 | Choose a fallback This example leaves backup models off. |
| 0:53 | Set a token limit Zero removes the per-scan token ceiling. |
| 0:58 | Review the draft Check the route and permissions before applying. |
| 1:03 | Press a to apply Pacsea rechecks the route, pricing and consent. |
| 1:08 | Follow a scan |
| 1:16 | Tick the scan targets Space selects each AUR package. |
| 1:21 | Preview a dry run Enter previews the command without charges. |
| 1:27 | Enter queues checked targets Live mode freezes each package version. |
| 1:32 | Open Progress with 2 Follow the active package and its current phase. |
| 1:37 | Press p to pause The active scan may finish first. |
| 1:41 | Press x twice Confirm cancellation of the active scan. |
| 1:45 | Press r to retry Queue checked, cancelled targets again. |
| 1:49 | Read the evidence |
| 1:57 | Open Results with 3 Select fieldnotes-git to inspect its finding. |
| 2:02 | Enter opens the report Read the cited PKGBUILD line and finding. |
| 2:09 | Press t for details Read the commit in the validated record. |
| 2:14 | Check the second report fieldnotes-cli has incomplete file coverage. |
| 2:20 | Open Status with 4 Check the hourly background scan limit. |
| 2:25 | Press b to adjust Review which blocked limits would change. |
| 2:30 | Shift+B toggles background An active background scan may finish. |
| 2:35 | Toggle Pi BG The package browser shows the same switch. |
| 2:39 | Reopen guided setup Press w on Settings to revisit configuration. |
| 2:43 | Press c to continue Pacsea will recheck the source identities. |
| 2:49 | A changed identity stops here Review again or rescan the changed package. |
| 2:55 | Confirm a matching version The reviewed package enters the usual install list. |
| 3:01 | Ready for install review |

### 07 · Set up tools and manage repositories

Choose first-launch tasks in Pacsea, set up optional tools and tune sudo, doas or fingerprint authentication. Read a repository recipe, inspect its signing key and pacman status, and review both kinds of AUR name collision before choosing whether to migrate or install.

| Start | What viewers see |
| ---: | --- |
| 0:00 | Pacsea: Set up tools and manage repositories |
| 0:08 | Choose startup tasks with the arrow keys. |
| 0:13 | Press Space to include the VirusTotal setup. |
| 0:18 | Press Enter to run the checked tasks. |
| 0:23 | Open AUR with O. Add your public key to enable voting. |
| 0:29 | Add your VirusTotal key. Scans can then look up file hashes. |
| 0:35 | Esc postpones setup. R hides the startup selector permanently. |
| 0:40 | Open Optional Deps with Shift+O, then 3. |
| 0:45 | Check the installed tools. Review your editor, terminal and AUR helper. |
| 0:51 | Match your clipboard tool to KDE, Wayland or X11. Space selects it. |
| 0:56 | Set up aur-sleuth. Supply a server address, model and API key. |
| 1:03 | Click Wizard to reopen the startup selector. |
| 1:07 | Set the sudo timeout to cover a longer build. |
| 1:12 | Using doas? Choose a user-scoped persist rule. |
| 1:17 | For PAM fingerprints, set auth_mode to interactive. |
| 1:22 | Press F1 for Help. Find the keys for moving between panes. |
| 1:27 | Shift+C opens config files. Shift+P controls which panes appear. |
| 1:32 | Pacsea |
| 1:40 | Open Repositories with Shift+O, then 4. Read the four columns. |
| 1:46 | Press S for the template. Shipped recipes start disabled. |
| 1:51 | Press O for repos.conf. Give the custom source a name and server. |
| 1:57 | Press R to refresh a repository's declared signing key. |
| 2:02 | Space toggles and applies an eligible managed source on Linux. |
| 2:07 | Enter applies changes on Linux. Windows supports browsing. |
| 2:12 | Read the Pacman column. The managed source now shows active. |
| 2:17 | Click the Repos filter. It changes results independently of pacman. |
| 2:22 | Check for name collisions. The new source also ships this AUR package. |
| 2:28 | Esc at the first warning disables this source and queues an apply. |
| 2:33 | Enter continues to migration. Review the remove and reinstall commands. |
| 2:40 | Press Space to select which packages to migrate. |
| 2:45 | Enter opens final review. Y opens sudo. Esc returns to selection. |
| 2:51 | Starting an AUR install? Esc cancels if a repository has the same name. |
| 2:57 | Pacsea |

### 08 · Edit Pacsea’s configuration

Use Pacsea’s config editor to change search behavior, record a shortcut and arrange the terminal panes. Then see how the feature-branch editor handles cross-tab searches, starred settings, recent queries and value suggestions.

| Start | What viewers see |
| ---: | --- |
| 0:00 | Pacsea: Edit settings and shortcuts |
| 0:08 | Open the config editor with Shift+C, then 5. |
| 0:13 | Choose settings.conf and press Enter to browse its keys. |
| 0:18 | Type fuzzy to find the search matching switch. |
| 0:24 | Press Enter to edit. Space toggles fuzzy matching on. |
| 0:29 | Reset with Ctrl+Z to restore the value you opened. |
| 0:34 | Toggle on and save with Ctrl+S. The search pane now says FUZZY. |
| 0:40 | Clear the filter, then press B to bookmark the selected key. |
| 0:45 | Record a shortcut in keybinds.conf. Press Ctrl+K, then your new chord. |
| 0:51 | Esc cancels the edit. The original shortcut stays in place. |
| 0:56 | Export with Ctrl+E. The exported settings redact secret values. |
| 1:02 | Change the pane order to put Search above Results. |
| 1:08 | Set the pane widths. The three percentages must total 100. |
| 1:14 | Shift+P controls panels in both package and news views. |
| 1:19 | Check when changes apply. Search startup mode needs a restart. |
| 1:24 | Set the selection marker. For colors, open theme.conf. |
| 1:29 | Use interactive authentication for PAM fingerprint prompts. |
| 1:34 | Choose a language or leave it empty for the system language. |
| 1:39 | Filter by topic to find news preferences or individual scans. |
| 1:44 | Set the AUR helper in settings.conf. Ctrl+R reloads external edits. |
| 1:50 | Pacsea |
| 1:58 | Tab cycles through four tabs. Starred collects your bookmarked settings. |
| 2:03 | Switch tabs with a query. The search text carries across. |
| 2:08 | Read the mirror count details. Check its current value, default and range. |
| 2:14 | Press Ctrl+B to star a key. Find it again under Starred. |
| 2:20 | Enter edits the value. Press Enter again to save the new count. |
| 2:26 | Ctrl+R lists recent searches. Choose a query and press Enter. |
| 2:32 | Tab completes the last country. Earlier entries stay in the list. |
| 2:38 | Type D in the locale picker to narrow the choices to German. |
| 2:43 | Preview the color swatch. Enter saves and applies the heading color. |
| 2:49 | Press Ctrl+G on a Pi Scan key to open its guided setup. |
| 2:55 | Pacsea |

## Rendering the series

Each episode links to its full storyboard. The `production.render` entry names the command to rebuild the film, including the music and embedded chapters. Run it from that episode’s directory with the sibling episode folders available. The arrangements use the original *Quiet Index* score from the discovery pilot.

The PNG is the upload thumbnail. The text file contains the video description and chapter timestamps. Package versions, news items, account state and scan or transaction results are illustrative. Episode 08 switches to the editor preview at 1:50.
