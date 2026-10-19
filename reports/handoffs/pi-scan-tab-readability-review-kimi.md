# Pi Scan Readability — Independent Review

## Review

### Correct

- The integrated renderers generally follow the approved balanced-section hierarchy:
  - Setup sections: `src/ui/pi_scan/setup.rs:64-84, 91-180, 184-229`
  - Overview sections: `src/ui/pi_scan/overview.rs:23-165`
  - Progress sections: `src/ui/pi_scan/progress.rs:75-111`
- Shared 12-character identity presentation is centralized in `src/ui/pi_scan/mod.rs:47-49` and used by Targets, Progress, and Results. Exact stored identities are not modified.
- Targets and Results bound each displayed entry to one visual row and derive hit rectangles from the same compact prefix depth: `src/ui/pi_scan/targets.rs:16-101`, `src/ui/pi_scan/results.rs:17-65`.
- Semantic colors are generally paired with text or symbols rather than used alone.
- English and German section/status keys are present. New Hungarian readability keys use English text with the required translation TODO markers.
- No project or source files were modified by this review.

## Findings

### Blocker — wrapped wizard controls can activate the wrong consent

- **Severity:** Blocker / High
- **Affected symbols:**
  - `src/ui/pi_scan/wizard.rs:119-122` — body uses word wrapping.
  - `src/ui/pi_scan/wizard.rs:257-281` — `append_control_rects` calculates rows from unwrapped logical line indices.
  - `src/ui/pi_scan/wizard.rs:419-466` — Pricing and Privacy places confirmations after wrapping disclosure text.
  - `src/events/pi_scan/keys.rs:113-140` — clicking a `Control(index)` immediately activates that indexed control.
- **Violated requirement:** Preserve controls, mouse behavior, hit rectangles, and trust boundaries.
- **Failure mode/reproduction:** At an 80-column terminal, the body has approximately 78 inner columns. The English provider disclosure at `config/locales/en-US.yml:227` exceeds this width and wraps. The first Pricing confirmation therefore renders below its nominal source line. `append_control_rects` still records consecutive rows beginning at logical line five. Clicking the visibly rendered first confirmation can hit `Control(1)` and toggle foreground paid execution instead of the disclosure confirmation.
- **Test gap:** `render_records_footer_and_body_hit_targets` at `src/ui/pi_scan/wizard.rs:832-856` covers only `OptionalBehavior`, whose controls begin at line zero, and checks hard-coded coordinates rather than matching rectangles to rendered control text.
- **Smallest useful remediation:** Calculate control rows using the same display-width wrapping model as the body, including scroll offset. Add TestBackend mouse tests for Readiness, Route, and Pricing at 80 columns and a narrower width, asserting that clicking each rendered label changes only its corresponding control.

### Medium — completed wizard operations are shown as pending

- **Affected symbols:**
  - `src/ui/pi_scan/wizard.rs:139-157` — every `wizard.notice` gets a yellow hourglass.
  - `src/state/pi_scan_setup.rs:479-482` — successful readiness verification sets `readiness_verified`.
  - `src/state/pi_scan_setup.rs:642-647` — successful write-free validation sets `validation_write_free`.
- **Violated requirement:** Wizard validation, warning, and success states must use the approved semantic color system and truthful wording.
- **Failure mode/reproduction:** After readiness verification or candidate validation completes, the persistent success notice is rendered as `⏳` in yellow. It therefore communicates pending/in-progress work despite the underlying operation being complete.
- **Test gap:** The wizard style test at `src/ui/pi_scan/wizard.rs:786-796` covers progress markers only, not in-flight, successful, warning, or failed notice states.
- **Smallest useful remediation:** Classify known notice keys by semantic state. Render `verifying`/`validating` with yellow hourglass, `readiness_verified`/`validation_write_free` with green check, and route reselection or dry-run review with an appropriate warning marker. Add focused style assertions.

### Medium — Results rows remain partly English in German

- **Affected symbols:**
  - `src/ui/pi_scan/results.rs:140-150` — appends `result.completion_wording()` directly.
  - `src/logic/pi_scan/result.rs:987-994` — completion wording is fixed English text.
  - `config/locales/de-DE.yml:332-344` — coverage, identity, and severity are translated, but row completion wording has no corresponding localized keys.
- **Violated requirement:** English and German copy must be complete.
- **Failure mode/reproduction:** Rendering Results under `de-DE` produces a mixed-language row such as localized `vollständig`, `AKTUELLE IDENTITÄT`, and severity followed by English `Complete — no findings in analyzed scope` or `N finding(s) in analyzed scope`.
- **Test gap:** Readability Results assertions at `tests/pi_scan/ws4_tui.rs:1297-1311` load only English. The locale test verifies map loading/key presence but does not render Results in German.
- **Smallest useful remediation:** Derive localized completion wording from typed coverage and finding count in the UI, preserving the exact approved English strings. Add German zero-, one-, and multiple-finding render assertions.

### Medium — Advanced Setup exposes static configuration errors only in English

- **Affected symbols:**
  - `src/ui/pi_scan/setup.rs:45-52` — joins raw `validation_issues()` strings.
  - `src/theme/types.rs:126-173, 180-183` — validation issues are hard-coded English sentences.
- **Violated requirement:** English and German copy must be complete; configuration status should remain actionable.
- **Failure mode/reproduction:** Load German with an invalid setting, such as `pi_scan_head_query_timeout_seconds = 16`. The Setup section heading and label are German, but the red value is English: `pi_scan_head_query_timeout_seconds must be between 1 and 15`. The same occurs for binary, observation interval, budget, retention, and proxy validation failures.
- **Test gap:** Setup render tests cover section presence and no-panic behavior only. No non-English invalid-configuration state is rendered.
- **Smallest useful remediation:** Map the finite validation issue set to localized keys, retaining setting names and bounds as format arguments. Add one German invalid-configuration render test.

## Checks Inspected

- Approved plan: `plans/planned/pi-scan-tab-readability.md`
- Workstream handoffs:
  - `reports/handoffs/pi-scan-tab-readability-ws1.md`
  - `reports/handoffs/pi-scan-tab-readability-ws2.md`
- Current affected Rust renderers:
  - `src/ui/pi_scan/{mod,setup,wizard,overview,targets,progress,results}.rs`
- Locales:
  - `config/locales/{en-US,de-DE,hu-HU}.yml`
- Cross-tab tests: `tests/pi_scan/ws4_tui.rs`
- Supporting state/action definitions were inspected where required to verify concrete failure modes.
- Handoffs attest that formatting, Clippy, compilation, focused tests, full serialized tests, and whitespace checks passed. Those commands were not independently rerun during this read-only review.
- The available review tools did not expose Git status/diff execution. Current affected file contents were inspected directly, but the exact baseline-to-worktree Git diff was not independently generated.

## Conclusion

- **Findings present:** Yes — one blocker and three medium-severity findings.
- **No-findings statement:** Not applicable.
- **Confidence:** 94/100. Static source evidence strongly establishes the findings. Confidence is reduced because no interactive render or independent Git/test command execution was available.

## Residual Risks

- Custom-theme contrast was not manually assessed.
- German and Hungarian rendering is not covered across representative Setup, Wizard, and Results states.
- Exact current staging status and baseline diff require supervisor-side Git inspection.
- Existing narrow-render tests establish no panic, but they do not verify that all important wording remains reachable and understandable after wrapping.