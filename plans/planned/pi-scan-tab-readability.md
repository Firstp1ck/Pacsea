# Pi Scan tab readability and visual hierarchy

**Status:** Awaiting approval
**Integration owner:** Parent Pi session
**Classification:** Complex feature
**Report:** `reports/pi-scan-tab-readability.html`

## Goal

Give Setup, Overview, Targets, Progress, and Results one coherent, keyboard-first visual language with clearer grouping, shorter human-facing identifiers, and accessible semantic color coding. Preserve the existing scanner behavior, trust boundaries, actions, and compact-terminal support.

## Classification rationale

The preliminary lightweight classification is superseded by repository evidence. The request spans five tabs, the guided setup wizard, shared rendering helpers, three locale files, and cross-tab rendering tests. It has two meaningful implementation slices with separate ownership and benefits from independent visual and correctness review. The complex feature contract therefore applies.

## Approved decisions

The user selected these options:

- Balanced sections: compact status rows with headings and deliberate whitespace.
- Short package commit hashes outside Technical Details. Show the first 12 characters; keep exact values in Details.
- Apply the visual redesign to both advanced Setup and the guided setup wizard.
- Use accessible semantic colors paired with text or symbols. Color must never be the only status signal.

## Design system

### Visual hierarchy

1. Page-level hint or purpose line, when needed.
2. Mauve bold section headings.
3. Indented label/value rows with muted labels and semantically colored values.
4. Blank lines between sections, not between every row.
5. Sapphire bold selection and active-work emphasis.
6. Muted supporting metadata such as reservations, package bases, and short hashes.

### Semantic colors

| Meaning | Theme color | Required textual or symbolic cue |
| --- | --- | --- |
| Selected, active, actionable | Sapphire | Selection marker, active wording, or key hint |
| Complete, confirmed, current | Green | Complete/confirmed/current wording or check mark |
| Pending, paused, incomplete, warning | Yellow | Pending/paused/incomplete/warning wording or pause marker |
| Failed, invalid, disconnected, critical/high | Red | Failed/error/disconnected wording or alert marker |
| Section heading | Mauve | Heading text |
| Secondary metadata | Overlay/subtext | Label or metadata context |

### Identity presentation

- Outside Technical Details, render commit OIDs with a shared deterministic 12-character short form.
- Package names remain primary. Package base appears only when it adds information.
- Do not truncate package names or status wording merely to preserve a full hash.
- Exact commit OIDs remain available in the expanded Details technical section.

## Scope

### In scope

- `src/ui/pi_scan/mod.rs`: reusable visual helpers and top-bar availability colors.
- `src/ui/pi_scan/setup.rs`: advanced Setup grouping, status wording, and consent readability.
- `src/ui/pi_scan/wizard.rs`: stronger progress, control, validation, and state emphasis without changing the seven-step flow.
- `src/ui/pi_scan/overview.rs`: grouped runtime, budget, permission, and notice summary.
- `src/ui/pi_scan/targets.rs`: readable target rows, status markers/colors, short identity, and preserved hit rectangles.
- `src/ui/pi_scan/progress.rs`: clearer section grouping, active/queued emphasis, short identity, and preserved truthful progress semantics.
- `src/ui/pi_scan/results.rs`: scan-friendly result rows with status/severity color, concise wording, and short identity.
- English, German, and Hungarian locale entries required by the redesign.
- Focused TUI integration and style assertions.

### Non-goals

- No scanner execution, networking, acquisition, result-schema, persistence, budget, consent-policy, or acknowledgement changes.
- No new keybindings or mouse behavior.
- No changes to Details behavior beyond consuming a safe shared presentation helper if needed.
- No dependency changes, README edits, wiki edits, deployment, or publication.
- No color-only information.

## Success criteria

- Every in-scope tab has a clear visual hierarchy at 80x24 and remains usable without panic at 20x10.
- Advanced Setup is grouped into runtime, route/cost, safety, and permissions instead of one undifferentiated list.
- The setup wizard keeps its current flow while selected controls, progress, validation, warning, and success states use the semantic color system.
- Overview separates current activity, budget use, permissions, and notices.
- Targets and Results expose package name and status first; long commit IDs no longer dominate rows.
- Progress separates session summary, current work or waiting state, and queue entries.
- Selected rows and every status category have both a textual or symbolic cue and a semantic color.
- Existing scrolling, selection, hit rectangles, actions, and narrow-terminal behavior remain intact.
- Exact commit IDs remain reachable through Details technical data.
- English and German copy is complete. New Hungarian entries use English text with the required translation TODO marker.
- Focused render tests verify layout text, short identities, and representative buffer colors.
- Required repository checks pass.

## Execution DAG and ownership

The repository is dirty with approved in-progress branch work, so implementation workers run sequentially in the shared worktree. The parent remains the sole integration owner.

### WS1: shared system, Setup, wizard, Overview, and localization

**Owner:** implementation worker 1

**Prerequisites:** This approved plan.

**Write boundary:**

- `src/ui/pi_scan/mod.rs`
- `src/ui/pi_scan/setup.rs`
- `src/ui/pi_scan/wizard.rs`
- `src/ui/pi_scan/overview.rs`
- `config/locales/en-US.yml`
- `config/locales/de-DE.yml`
- `config/locales/hu-HU.yml`

**Deliverables:**

- Shared short-identity and styled section/status helpers where they reduce duplication.
- Accessible availability color in the workspace top bar.
- Structured advanced Setup, guided wizard emphasis, and Overview layout.
- All locale keys needed by both workstreams, including keys consumed by WS2.
- Focused module tests where helper behavior is pure.

**Validation:** `cargo fmt --all`, focused Pi Scan UI/unit tests, and `cargo check`.

**Handoff:** `reports/handoffs/pi-scan-tab-readability-ws1.md`

### WS2: Targets, Progress, Results, and cross-tab acceptance tests

**Owner:** implementation worker 2

**Prerequisites:** Parent has inspected and accepted WS1, including shared helper signatures and locale keys.

**Write boundary:**

- `src/ui/pi_scan/targets.rs`
- `src/ui/pi_scan/progress.rs`
- `src/ui/pi_scan/results.rs`
- `tests/pi_scan/ws4_tui.rs`

**Deliverables:**

- Structured and color-coded Targets, Progress, and Results layouts.
- Short identity use through the approved shared helper.
- Preserved list viewport and hit-rectangle behavior.
- Cross-tab text, narrow-render, and representative style tests.

**Validation:** `cargo fmt --all`, `cargo test --test pi_scan ws4_tui -- --test-threads=1` or the narrowest valid focused filter, and `cargo check`.

**Handoff:** `reports/handoffs/pi-scan-tab-readability-ws2.md`

### Integration and review

1. Parent inspects WS1 files, scope, handoff, and focused checks.
2. Parent launches WS2 only after WS1 is accepted.
3. Parent runs affected and full repository checks on the integrated worktree.
4. Two fresh-context read-only reviewers from distinct provider families assess correctness, user-flow readability, accessibility, tests, maintainability, and plan compliance.
5. Parent records one evidence-backed disposition for every finding.
6. One bounded fix worker applies only accepted findings if needed; affected checks run again.
7. Parent creates and validates the final self-contained HTML report.

## Acceptance checks

- Render each in-scope tab at 80x24 and assert its intended section hierarchy.
- Render representative states at 20x10 and assert no panic plus bounded scroll state.
- Inspect TestBackend cell styles for selected/active, success, warning, and failure examples.
- Verify Targets and Results still register correct row hit rectangles.
- Verify hashes are 12 characters outside Details and full in toggled Technical Details.
- Run from repository root in order:
  1. `cargo fmt --all`
  2. `cargo clippy --all-targets --all-features -- -D warnings`
  3. `cargo check`
  4. `cargo test -- --test-threads=1`
  5. `git diff --check`

## Risks and mitigations

- **Color assumptions:** Pair every color with wording or a symbol and test representative styles.
- **Narrow-terminal wrapping:** Keep headings short, avoid rigid columns, and retain 20-column render coverage.
- **Hit-rectangle drift:** Preserve one rendered row per target/result and assert recorded coordinates.
- **Localization expansion:** Avoid fixed-width label columns and test all locale key presence.
- **Scope drift into scanner behavior:** Workers may only edit the declared UI, locale, and test files.
- **Dirty shared worktree:** Run workers sequentially, inspect after each, and never use automatic worktree fanout.

## Rollback

Revert the files in WS2, then the files in WS1, remove this plan and its report, and rerun the existing Pi Scan render tests. No state migration or durable data rollback is required.

## Decision and progress record

- Repository exploration identified the five tab renderers, setup wizard, shared workspace renderer, and `tests/pi_scan/ws4_tui.rs` as the relevant seams.
- No live peer owns overlapping files. Persisted summaries describe completed earlier work only.
- User approved balanced sections, 12-character hashes outside Details, both Setup modes, and accessible semantic colors.
- Awaiting user approval of this canonical plan before implementation.

## Review findings and dispositions

Pending implementation and independent review.
