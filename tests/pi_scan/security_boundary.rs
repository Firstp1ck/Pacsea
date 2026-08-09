//! Intentionally red Wave 0 security contract markers for later WS1/WS2 implementation.
//!
//! These ignored markers preserve boundary names and ownership before production seams exist.
//! They are not regression tests: each owning workstream must replace its marker body with
//! executable adversarial assertions before removing the ignore.

/// What: Fail one intentionally red Wave 0 contract marker with an ownership label.
///
/// Inputs:
/// - `owner`: Workstream that must implement the boundary.
/// - `contract`: Exact missing behavior.
///
/// Output:
/// - Never returns; emits a deterministic test failure.
///
/// Details:
/// - Centralizes failure wording so explicit red-test output is easy to audit.
#[track_caller]
fn pending_boundary(owner: &str, contract: &str) -> ! {
    panic!("Wave 0 red contract pending {owner}: {contract}");
}

/// What: Require strict package-base validation before any AUR URL or Git argv construction.
///
/// Inputs:
/// - Leading-dash, traversal, slash, NUL/control, whitespace, and shell-metacharacter cases.
///
/// Output:
/// - WS1 must reject every adversarial package base deterministically.
///
/// Details:
/// - Remains ignored until the identity module owns the public validator.
#[test]
#[ignore = "Wave 0 red contract; WS1 identity validator not implemented"]
fn wave0_red_package_base_injection_is_rejected() {
    pending_boundary(
        "WS1",
        "reject -leading, ../, slash, NUL/control, whitespace, and shell metacharacters",
    );
}

/// What: Require canonical official-AUR recipe identity and immutable full commit OIDs.
///
/// Inputs:
/// - Userinfo, query, fragment, alternate port, non-AUR host, abbreviated/malformed OIDs.
///
/// Output:
/// - WS1 accepts only canonical official AUR URL plus full immutable identity.
///
/// Details:
/// - Covers manual mapping and observed-head acquisition contracts.
#[test]
#[ignore = "Wave 0 red contract; WS1 AUR identity binding not implemented"]
fn wave0_red_aur_commit_identity_is_canonical() {
    pending_boundary(
        "WS1",
        "canonicalize official AUR URL and reject noncanonical hosts/ports/fragments/OIDs",
    );
}

/// What: Require corrupt/newer baseline state to quarantine and fail closed.
///
/// Inputs:
/// - Invalid JSON and unsupported schema fixtures.
///
/// Output:
/// - Dedicated loader never returns an empty accepted baseline.
///
/// Details:
/// - The existing generic cache loader is intentionally not an acceptable implementation.
#[test]
#[ignore = "Wave 0 red contract; WS1 versioned quarantine loader not implemented"]
fn wave0_red_malformed_baseline_never_becomes_empty_state() {
    pending_boundary(
        "WS1",
        "atomically quarantine corrupt/newer state and keep scanner unavailable",
    );
}

/// What: Require path resolution to reject traversal, absolute paths, and symlink escapes.
///
/// Inputs:
/// - Parent components, absolute paths, link escapes, root replacement, and special files.
///
/// Output:
/// - WS2 restricted tools return bounded inert errors without reading host data.
///
/// Details:
/// - The test will later use a sentinel outside the snapshot root.
#[test]
#[ignore = "Wave 0 red contract; WS2 restricted path resolver not implemented"]
fn wave0_red_restricted_paths_cannot_escape_snapshot() {
    pending_boundary(
        "WS2",
        "reject traversal, absolute paths, symlink/root escapes, and special files",
    );
}

/// What: Require the model-visible read tool to be unable to read an outside-root sentinel.
///
/// Inputs:
/// - Private snapshot descriptor and a host sentinel adjacent to the snapshot.
///
/// Output:
/// - No sentinel bytes appear in tool output, errors, logs, or persisted state.
///
/// Details:
/// - Complements pure path normalization with an end-to-end fake-Pi boundary test.
#[test]
#[ignore = "Wave 0 red contract; WS2 restricted extension not implemented"]
fn wave0_red_pi_cannot_read_outside_root_sentinel() {
    pending_boundary(
        "WS2",
        "fake Pi cannot read or disclose a sentinel outside approved snapshot roots",
    );
}

/// What: Require strict bounded LF-only RPC framing.
///
/// Inputs:
/// - CR-only, invalid UTF-8/JSON, Unicode separators, oversized and incomplete records.
///
/// Output:
/// - WS2 accepts LF records, strips one trailing CR, and rejects every malformed case.
///
/// Details:
/// - Generic Unicode line readers are forbidden by the Pi RPC contract.
#[test]
#[ignore = "Wave 0 red contract; WS2 RPC codec not implemented"]
fn wave0_red_rpc_framing_is_lf_only_and_bounded() {
    pending_boundary(
        "WS2",
        "strict LF JSONL with one optional trailing CR and bounded invalid-record rejection",
    );
}

/// What: Require strict model-response and evidence validation against hostile output.
///
/// Inputs:
/// - Duplicate keys, trailing objects, fabricated evidence, controls, oversized values, and tool payloads.
///
/// Output:
/// - WS2 rejects the entire attempt without persisting or presenting unvalidated content.
///
/// Details:
/// - The executable assertion must bind findings to exact identity and manifest evidence.
#[test]
#[ignore = "Wave 0 red contract marker; WS2 model-output validator not implemented"]
fn wave0_red_model_output_and_evidence_are_strictly_validated() {
    pending_boundary(
        "WS2",
        "reject duplicate/trailing/oversized/control output and fabricated or mismatched evidence",
    );
}

/// What: Require the embedded restricted-tool extension hash to be verified before Pi launch.
///
/// Inputs:
/// - Untampered, modified, truncated, and replacement extension assets.
///
/// Output:
/// - WS2 launches only the compiled trusted asset and fails closed on every mismatch.
///
/// Details:
/// - The executable assertion must observe that a mismatch prevents process creation.
#[test]
#[ignore = "Wave 0 red contract marker; WS2 extension asset verification not implemented"]
fn wave0_red_extension_asset_hash_is_verified_before_launch() {
    pending_boundary(
        "WS2",
        "verify the compiled extension asset hash before materialization and Pi launch",
    );
}

/// What: Require cancellation to abort, group-kill, reap, and suppress correction/fallback.
///
/// Inputs:
/// - Fake child that forks, ignores TERM, and emits unbounded output.
///
/// Output:
/// - No zombie/orphan survives the five-second grace and ten-second shutdown deadline.
///
/// Details:
/// - Uses the approved process-group mechanism once WS2 owns it.
#[test]
#[ignore = "Wave 0 red contract; WS2 process lifecycle not implemented"]
fn wave0_red_cancellation_reaps_entire_pi_process_group() {
    pending_boundary(
        "WS2",
        "RPC abort then bounded process-group TERM/KILL/reap with no fallback",
    );
}

/// What: Require dry-run acquisition to leave durable scanner state unchanged and never launch Pi.
///
/// Inputs:
/// - Read-only AUR/source/key acquisition in an isolated config root.
///
/// Output:
/// - Preview evidence exists, all temporary data is removed, and no durable scan state changes.
///
/// Details:
/// - Reflects the post-interview dry-run contract rather than the superseded no-network draft.
#[test]
#[ignore = "Wave 0 red contract; WS1 dry-run acquisition transaction not implemented"]
fn wave0_red_dry_run_acquires_without_pi_or_durable_mutation() {
    pending_boundary(
        "WS1/WS2",
        "allow ephemeral acquisition but forbid Pi launch and durable state mutation",
    );
}

/// What: Require identity changes to invalidate results and acknowledgements.
///
/// Inputs:
/// - AUR HEAD or mutable source ref changed after the frozen scan identity.
///
/// Output:
/// - Result becomes stale and linked continuation requires separate acknowledgement/rescan.
///
/// Details:
/// - Late responses may never overwrite a newer target or baseline.
#[test]
#[ignore = "Wave 0 red contract; WS1/WS3 stale identity state machine not implemented"]
fn wave0_red_stale_head_invalidates_result_and_acknowledgement() {
    pending_boundary(
        "WS1/WS3",
        "changed AUR/source identity marks stale and invalidates prior acknowledgement",
    );
}
