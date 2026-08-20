# Pi AUR scan package-specific details

**Status:** In progress
**Integration owner:** parent session
**Classification:** Complex feature

## Goal

Make the Pi Scan Details view clearly identify which package each detail belongs to and, when multiple validated package results exist, show a navigable list of package headers whose scan content can be expanded or collapsed.

## Classification rationale

The change crosses the Pi Scan workspace state, keyboard interaction, renderer, localization, and tests. It has two independently verifiable slices: state/input behavior and rendering/acceptance coverage. It therefore remains complex under the feature workflow contract.

## Success criteria

- Details always show the package name for every displayed scan.
- Multiple results show a package header list; each header has an unambiguous expanded/collapsed state.
- Users can select a package header and toggle its content without losing existing scrolling and acknowledgement actions.
- Single-result Details remains useful and package-labeled.
- Focused tests cover state transitions and rendered output/interaction.
- `cargo fmt`, clippy, check, and serialized tests pass.

## Scope and non-goals

- In scope: Pi Scan Details state, rendering, keyboard interaction, translations, and tests.
- Out of scope: scan execution, result schema, persistence, other workspaces, and README/wiki changes.

## Approved decisions and invariants

- Existing `selected_result` remains the package/result identity used by acknowledgement and continuation actions.
- Expansion state is session-only and keyed by result index; it is reset when entering Details if necessary to avoid stale indices.
- Package headers remain visible even when collapsed; expanded content is rendered beneath its own header.
- Existing raw-output visibility remains respected inside each package section.
- No external commands, network calls, or dependency changes.

## Execution waves and ownership

1. **WS1 — state and input contract**: add expansion state and deterministic keyboard behavior, plus focused unit tests. Owned by implementation worker 1; paths: `src/state/pi_scan_ui.rs`, `src/events/pi_scan/keys.rs`, related tests only.
2. **WS2 — rendering and localization**: render all result package sections with labels and expansion markers, update Details footer/localization, plus render tests. Owned by implementation worker 2; paths: `src/ui/pi_scan/details.rs`, `src/ui/pi_scan/mod.rs`, locale files, related tests only. Depends on WS1 API names.
3. **Integration**: parent inspects both workstreams, resolves any interface conflict, runs affected and cross-workstream checks.
4. **Review**: two fresh-context read-only reviewers from distinct provider families inspect the integrated result.

## Acceptance checks

- Unit tests assert toggling expansion and selection behavior.
- TUI tests assert package names and collapsed/expanded content are visible as intended.
- Full required Rust checks are run from repository root.

## Rollback

Revert the feature files and remove the plan/report artifacts; no migration or durable state is involved.

## Progress record

- Repository exploration identified `src/ui/pi_scan/details.rs`, `src/state/pi_scan_ui.rs`, and `src/events/pi_scan/keys.rs` as the primary seams.
- Parent session is the sole integration owner.

## Follow-up: human-readable Details report

**Classification:** Lightweight feature

### Classification rationale

This follow-up changes one existing presentation path: the Details renderer, its localized labels, and focused render assertions. It does not change scanner state, input handling, result schemas, persistence, security policy, or runtime behavior. One implementation slice is sufficient, so the preliminary lightweight classification is confirmed.

### Success criteria and decisions

- Lead with a short review summary instead of internal completion flags and booleans.
- Group limitations and findings under counted headings with whitespace between sections.
- Translate known validation limitations into plain language while retaining unknown messages unchanged.
- Deduplicate equivalent plain-language limitations without altering the validated result.
- Keep exact canonical metadata and original validation messages behind the existing `t` toggle.
- Preserve package selection, expansion, acknowledgement, continuation, scrolling, and raw-output settings.

### Validation

- Focused Details render tests: 4 passed.
- `cargo fmt --all`, Clippy with warnings denied, and `cargo check` passed.
- Full serialized test suite passed: 1,289 library tests, 10 binary tests, all integration targets, and doctests; 7 library tests remained intentionally ignored.

## Review dispositions

The original complex package-navigation feature still has its independent review pending. This lightweight presentation follow-up does not add a separate mandatory review gate.

## Report

Pending; final report must link this plan and be saved under `reports/pi-aur-scan-package-details.html`.
