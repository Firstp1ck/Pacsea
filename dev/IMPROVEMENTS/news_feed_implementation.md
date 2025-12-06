# News Feed Implementation Suggestions

## Existing Coverage Snapshot
- Arch news RSS already fetched (`src/sources/news.rs`) and surfaced via CLI/startup modal.
- AUR RPC v5 search/info endpoints already used for package search/details (`src/sources/search.rs`, `src/sources/details.rs`).
- Official repo JSON search API used for indexing/search (`src/index/mirrors.rs`).
- No ingestion yet for security advisories, package-update RSS feeds, AUR bulk metadata archives, or pacman db tarballs.

## Priority Focus
- Package news for installed packages only (AUR + official), including recent AUR user comments in the feed view.
- Security news with user control over scope: toggle between installed-only and all packages.

## Source Coverage and Plan
| Source | Status | What it is / Uses | Suggested implementation notes |
| --- | --- | --- | --- |
| AUR Package Info (RPC v5) | Implemented (details) | AUR’s JSON info endpoint for specific packages; returns version, maintainer, out-of-date flag, popularity, etc. Use for precise “this package updated” detection. | Reuse existing client; build feed items for installed AUR packages when version differs from cached snapshot; include maintainer/orphan/out-of-date markers. |
| AUR Package Search (RPC v5) | Implemented (search) | AUR’s search endpoint returning lightweight results (name/desc/version/popularity). Good for discovery and link-outs. | No extra ingestion needed; surface as “related/discovery” links inside feed cards when helpful. |
| AUR Metadata Bulk (.json.gz) | Not implemented | Full compressed dump of all AUR metadata. Enables fast snapshot/diff across the entire AUR without many RPC calls. | Periodic downloader with etag/if-modified-since to cache dir; parse incrementally; filter to installed names first for feed; allow “all packages” diff on demand. |
| Official Repo Packages (JSON API) | Implemented (index build) | Arch official packages JSON API used today for index. Reliable for name/version/desc per repo/arch. | Tap existing fetches to emit feed items when versions change; persist last-seen map; default to installed set, allow “all” toggle. |
| Official Repo Search (JSON) | Implemented (search) | JSON search endpoint for official repos; user-driven lookup, not a feed source. | Keep for interactive search only; no feed ingestion needed. |
| Security Advisories (security.archlinux.org JSON) | Not implemented | Structured advisory feed with IDs, severity, affected packages, fixed versions. | Poll endpoint; generate feed with severity badges; add scope filter (installed vs all); maintain per-advisory read/unread. |
| Arch News (RSS) | Implemented | Official Arch news posts (manual interventions, announcements). Good for human-readable notices. | Store GUID/pubDate to dedupe; retry/backoff; fast-fail offline. |
| Package Updates (RSS per repo/arch) | Not implemented | RSS streams listing recent package version updates per repository/architecture. Human-friendly “recent changes” view. | Optional subscription; normalize to package/version tuples; dedupe with official index; default to installed set only. |
| Pacman Database (db.tar.gz) | Not implemented | Compressed sync dbs (`$repo.db.tar.gz`) containing package metadata/versions; usable offline. | Opt-in download; parse `desc` files to detect version bumps and metadata when APIs fail; limit to installed names to save work. |
| AUR Comments (HTML/JSON) | Implemented (existing comment fetch) | Latest user comments on AUR packages; useful signal for build breaks or fixes. | Reuse `fetch_aur_comments`; show recent comments for installed AUR packages; track last-seen comment ID to avoid noise. |
| Arch BBS (bbs.archlinux.org feeds) | Not implemented | Forum threads (Announcements, Pacman/Upgrade Issues, Security, AUR). Atom/RSS via `extern.php?action=feed&type=atom|rss&fid=<forum_id>`; HTML fallback. | Optional and user-configurable; per-forum enable/disable; rate-limit; cache ETag/Last-Modified; parse titles/links/dates; default off to avoid noise. |
| Full repo snapshots (official/AUR) | Not implemented | Large mirror snapshots of official or AUR repos (packages and/or full metadata). | Opt-in only; for offline/air-gapped or reproducibility; not needed for routine feeds. Track size/bandwidth warnings. |

## Proposed Architecture
- Create `sources::feeds` with per-source fetchers returning `Vec<FeedItem>`; shared `FeedItemKind` enum (`News`, `Advisory`, `Update`, `AURChange`, etc.).
- Central scheduler: periodic async tasks with jitter; backoff on failure; honor global `--dry-run` (log planned fetches only).
- Persistence: store last-seen identifiers per source (e.g., advisory ID, RSS GUID/link, pkg+version) under `~/.config/pacsea/cache/news/`.
- Caching and diffing: compare newly fetched items against last-seen snapshot to generate incremental feed entries; keep small ring buffer to bound disk use.
- Graceful degradation: if `curl/reqwest` or network unavailable, surface actionable error in UI and continue with other sources.
- UI: single feed view with filters (source, severity, unread); actions to mark read, open link, copy URL; keyboard-first shortcuts aligned with existing patterns.
- Testing: add unit tests per fetcher using recorded fixtures; integration test that aggregates mixed sources and enforces dedupe and ordering.

## What the missing sources provide
- Package Updates (RSS per repo/arch): RSS feeds published per repository/architecture that list recent package version updates; each item usually contains package name, new version, link to package page, and publish date. Not required for update checks (we already have API/index paths), but useful for a human-readable “recent repo changes” lane and for cross-checking unexpected version bumps.
- Pacman Database (db.tar.gz): Compressed pacman sync database files (`$repo.db.tar.gz`) containing package metadata and versions; parsing them locally allows offline detection of version changes and metadata when APIs are unreachable. Typical size: a few MB for `core`, low tens of MB for `extra`/`multilib`; total usually under ~40–60 MB per arch. Opt-in download. Uses: offline verification, reproducing historical state, secondary diff source when JSON APIs are down, and deep metadata parsing (depends/optdepends/licensing) without invoking pacman on the live system.
- AUR Metadata Bulk (.json.gz): Periodic full AUR metadata dumps in compressed JSON; processing them yields a snapshot of all AUR packages (name, version, metadata) enabling fast diffing to detect updates without many RPC calls. Typical size: tens of MB compressed (varies with AUR churn); opt-in download. Uses: fast installed-only diffs, bulk analytics (orphaned/out-of-date stats), and reduced network chatter compared to many RPC calls.
- Full repo snapshots (official/AUR): Large downloads (hundreds of MB+ depending on mirror scope) that mirror package files and/or full metadata. Opt-in only; suited for offline/air-gapped environments, reproducibility, or accelerating local diff pipelines. Not necessary for normal feed consumption.
- Arch BBS feeds: Atom/RSS endpoints exposed via `extern.php` for specific forum IDs (e.g., Announcements, Pacman & Package Upgrades, Security). Useful for surfacing forum alerts about breakage, manual interventions, or security discussions; make per-forum opt-in configurable with rate limits and caching.

## News Feed Priority List (stylistic match to Feature Priority doc)

| Tier | Item | Target | Impact | Complexity |
|------|------|--------|--------|------------|
| 🔴 | Installed-package news + AUR comments | v0.7.1 | ⭐⭐⭐⭐ | Medium-High |
| 🔴 | Security advisories (installed/all toggle) | v0.7.1 | ⭐⭐⭐⭐ | Medium |
| 🟠 | AUR metadata bulk diff (installed-first) | v0.7.x | ⭐⭐⭐ | Medium |
| 🟠 | Package update RSS lane (optional) | v0.7.x | ⭐⭐ | Low-Medium |
| 🟡 | Pacman db.tar.gz fallback (opt-in) | v0.7.x | ⭐⭐ | Medium |
| 🟡 | Arch BBS feeds (per-forum opt-in) | v0.7.x | ⭐⭐ | Low-Medium |
| 🟢 | Full repo snapshots (opt-in/offline) | v0.7.x | ⭐⭐ | High |

### Tier details
- 🔴 Installed-package news + AUR comments (v0.7.1): Aggregate version bumps for installed official/AUR packages and surface latest AUR comments with last-seen tracking; unread/read state; keyboard-first filters.
- 🔴 Security advisories with scope toggle (v0.7.1): Fetch security.archlinux.org JSON; show severity, affected packages, fixed versions; filters for installed vs all; per-advisory read/unread; link-out to details.
- 🟠 AUR metadata bulk diff (v0.7.x): Periodic .json.gz sync with etag; incremental parse filtered to installed names; optional full diff for “all”; produces update events without many RPC calls.
- 🟠 Package update RSS lane (v0.7.x, optional): Subscription per repo/arch; human-friendly recent changes stream; dedupe against official index; default to installed set; low runtime risk if disabled.
- 🟡 Pacman db.tar.gz fallback (v0.7.x, opt-in): Download per repo/arch on demand; parse desc for version/metadata when APIs are down; offline verification; bandwidth-aware prompts.
- 🟡 Arch BBS feeds (v0.7.x, per-forum opt-in): Atom/RSS via `extern.php` for selected forums (Announcements, Pacman & Package Upgrades, Security). Default off; user-select forums; rate-limit and cache ETag/Last-Modified; useful for surfacing breakage/manual intervention chatter without overwhelming the feed.
- 🟢 Full repo snapshots (v0.7.x, opt-in): Large mirror snapshots (official/AUR); for offline/air-gapped/repro builds; not needed for routine feeds; require quota warnings and manual enablement.

