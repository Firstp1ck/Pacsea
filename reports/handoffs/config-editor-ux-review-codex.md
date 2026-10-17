# Config editor UX — Review B (codex / gpt-6.1-sol, reasoning high), round 1

Reviewer run: taskId `node:delegated-task:command%3Amcp%3A59fbb81b-62ac-4473-b0b5-c803dd8d6ba5%3Adelegate-task%3Aconfig-editor-ux-review-codex-r1`, read-only, fresh context. Verbatim final output below.

---

1. The module split, shared value snapshot, and cached validation are sound, and Clippy, compilation, and the full test suite pass. The save safeguards remain intact. I found six issues: one credential-disclosure risk, three input/routing problems, and two display inaccuracies. The existing tests miss these combinations.

2. Findings, most severe first:

**B1 — high — Ambient proxy credentials become visible suggestions.** `src/state/config_editor/suggestions.rs:306`
`SuggestionContext::capture` reads `HTTPS_PROXY` verbatim, and `add_key_values` inserts it as a normal `[Detected]` suggestion (`add(out, proxy, Some(SuggestionTag::Detected));`). With `HTTPS_PROXY=https://alice:password@proxy.example`, opening `pi_scan_https_proxy` exposes those credentials without a reveal action. Selecting and saving also succeeds: this string key has no semantic validator, and its normal sensitivity permits unredacted export. This violates the secrets invariant and offers values that `PiScanSettings::validation_issues` explicitly rejects.
Fix: Filter detected proxies through the existing credential-free HTTPS validation before creating suggestions. Apply that validation to manual saves too, and protect exports from credential-bearing existing values. Add injected-context tests containing proxy userinfo.

**B2 — medium — Editor mouse actions can pass through an Alert modal.** `src/events/mouse/mod.rs:101`
`handle_modal_mouse` returns `None` for `Modal::Alert`. The new editor mouse handler then runs before the generic non-`None` modal gate, and it checks menu flags but never `app.modal`. Source-traced reproduction: display an Alert over the editor, then click an underlying tab or selected key row. The tab changes or a popup opens while the Alert remains visible. A hidden Record button can likewise start capture.
Fix: Consume remaining mouse events whenever a modal is active, immediately after modal-specific dispatch and before workspace/menu routing. Add a public-dispatch test with an Alert covering editor hit rectangles.

**B3 — medium — Integer nudging ignores the highlighted effective value.** `src/events/modals/config_editor/popup.rs:137`
`bump_int` parses `popup.buffer`, while dropdown selection deliberately leaves that buffer unchanged. For `mirror_count = 20`: open the popup, press Down twice to highlight `1`, then press `+`. The effective value becomes `21`, although immediately beforehand "Enter saves" showed `1`.
Fix: Nudge `effective_value()`, then replace the buffer and clear the suggestion highlight. Test both signs after selecting a preset.

**B4 — medium — Strict-picker Tab changes the choice instead of completing it.** `src/events/modals/config_editor/popup.rs:113`
`complete_or_cycle` advances strict pickers. The canonical plan says Tab completes the highlighted suggestion without saving. Opening a false boolean and pressing Tab changes its effective value to true; filtered enums retain their filter text instead of completing it. The updated hints describe cycling, but this deviation is not recorded among the accepted decisions.
Fix: Make Tab complete the highlighted choice for strict pickers too; retain arrows for selection. Adjust the tests currently asserting Tab cycling.

**B5 — low — Narrow tab strips can hide the active tab.** `src/ui/modals/config_editor/list.rs:71`
Tabs render from Settings onward and stop at the first overflow. At 60 columns, selecting Starred through BackTab leaves only the three file tabs visible, with no active-tab indicator.
Fix: Scroll or compact the strip so the active tab always remains visible, with matching hit rectangles. Add narrow-width tests for every active tab.

**B6 — low — Extra keybind changes are reported as unmodified.** `src/state/config_editor/values.rs:384`
`is_modified` compares `value_for` against `default_for`; both serialize only the first chord. `keybind_recent_remove = F9` appends F9 to its default bindings, leaving the first chord unchanged, so the row lacks the modified marker.
Fix: Compare complete chord slices for keybind entries. Add a snapshot test where only an extra chord differs.

3. Checks run: `cargo clippy --all-targets --all-features -- -D warnings` exit 0; `cargo check` exit 0; `cargo test -- --test-threads=1` exit 0; `git diff --cached --check` exit 0; read-only YAML duplicate-key and new-key parity checks exit 0 (all 243 newly added English editor keys exist in both other locales); confirmed `tests/pi_scan/ws4_tui.rs` has no staged changes.

4. Findings are based on source tracing; reproduction sequences were not executed in a live TUI and no regression tests were added. Ignored tests remain unverified.
