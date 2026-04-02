//! Background worker for AUR vote/unvote execution.

use tokio::sync::mpsc;

use crate::sources::{
    AurPackageVoteState, AurVoteContext, AurVoteError, AurVoteOutcome, VoteAction, aur_vote,
    aur_vote_state,
};

/// What: Runtime request payload for AUR vote execution.
///
/// Inputs:
/// - `pkgbase`: AUR package base name to vote on.
/// - `action`: Vote or unvote operation.
/// - `dry_run`: Whether to simulate instead of executing SSH.
/// - `ssh_timeout_secs`: SSH connect timeout in seconds.
/// - `ssh_command`: SSH binary path or command name.
///
/// Output:
/// - This struct is sent over runtime channels to the worker.
///
/// Details:
/// - Keeps all execution inputs immutable and explicit so worker handling
///   remains deterministic and testable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AurVoteRequest {
    /// Target package base.
    pub pkgbase: String,
    /// Requested operation.
    pub action: VoteAction,
    /// Dry-run flag from app runtime state.
    pub dry_run: bool,
    /// SSH connect timeout in seconds.
    pub ssh_timeout_secs: u32,
    /// SSH command path/name.
    pub ssh_command: String,
}

/// What: Runtime response payload for AUR vote execution.
///
/// Inputs:
/// - Produced by worker after processing [`AurVoteRequest`].
///
/// Output:
/// - Carries either a successful outcome or a typed error.
///
/// Details:
/// - Response is sent back to the main runtime event loop for UI-safe
///   toast/alert presentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AurVoteResponse {
    /// Operation result from `sources::aur_vote`.
    pub result: Result<AurVoteOutcome, AurVoteError>,
}

/// What: Runtime request payload for live AUR vote-state checks.
///
/// Inputs:
/// - `pkgbase`: AUR package base to check.
/// - `ssh_timeout_secs`: SSH connect timeout in seconds.
/// - `ssh_command`: SSH binary path or command name.
///
/// Output:
/// - This struct is sent over runtime channels to the vote-state worker.
///
/// Details:
/// - Keeps check requests explicit and immutable for deterministic worker behavior.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AurVoteStateRequest {
    /// Target package base.
    pub pkgbase: String,
    /// SSH connect timeout in seconds.
    pub ssh_timeout_secs: u32,
    /// SSH command path/name.
    pub ssh_command: String,
}

/// What: Runtime response payload for live AUR vote-state checks.
///
/// Inputs:
/// - Produced by worker after processing [`AurVoteStateRequest`].
///
/// Output:
/// - Carries package base and either resolved vote-state or typed failure.
///
/// Details:
/// - Response is consumed in the event loop to update UI state in a thread-safe way.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AurVoteStateResponse {
    /// Target package base.
    pub pkgbase: String,
    /// Live vote-state result from `sources::aur_vote_state`.
    pub result: Result<AurPackageVoteState, AurVoteError>,
}

/// What: Process one vote request using the core AUR vote service.
///
/// Inputs:
/// - `request`: Vote request payload containing pkgbase/action/context.
///
/// Output:
/// - Returns an [`AurVoteResponse`] with success or typed failure.
///
/// Details:
/// - Builds `AurVoteContext` from request fields.
/// - Delegates protocol handling and error mapping to `sources::aur_vote`.
fn process_vote_request(request: AurVoteRequest) -> AurVoteResponse {
    let context = AurVoteContext {
        dry_run: request.dry_run,
        ssh_timeout_secs: request.ssh_timeout_secs,
        ssh_command: request.ssh_command,
    };
    AurVoteResponse {
        result: aur_vote(&request.pkgbase, request.action, &context),
    }
}

/// What: Process one vote-state request using the core AUR vote-state service.
///
/// Inputs:
/// - `request`: Vote-state request payload.
///
/// Output:
/// - Returns an [`AurVoteStateResponse`] with state or typed failure.
///
/// Details:
/// - Builds `AurVoteContext` from request fields.
/// - Delegates SSH protocol and error mapping to `sources::aur_vote_state`.
fn process_vote_state_request(request: AurVoteStateRequest) -> AurVoteStateResponse {
    let context = AurVoteContext {
        dry_run: false,
        ssh_timeout_secs: request.ssh_timeout_secs,
        ssh_command: request.ssh_command,
    };
    AurVoteStateResponse {
        pkgbase: request.pkgbase.clone(),
        result: aur_vote_state(&request.pkgbase, &context),
    }
}

/// What: Spawn background worker for AUR vote requests.
///
/// Inputs:
/// - `aur_vote_req_rx`: Request channel receiver.
/// - `aur_vote_res_tx`: Response channel sender.
///
/// Output:
/// - None (spawns async task).
///
/// Details:
/// - Uses `tokio::task::spawn_blocking` for each request to keep the async
///   runtime and TUI render loop responsive even when SSH calls are slow.
pub fn spawn_aur_vote_worker(
    mut aur_vote_req_rx: mpsc::UnboundedReceiver<AurVoteRequest>,
    aur_vote_res_tx: mpsc::UnboundedSender<AurVoteResponse>,
) {
    tokio::spawn(async move {
        while let Some(request) = aur_vote_req_rx.recv().await {
            let res_tx = aur_vote_res_tx.clone();
            tokio::task::spawn_blocking(move || {
                let response = process_vote_request(request);
                let _ = res_tx.send(response);
            });
        }
    });
}

/// What: Spawn background worker for AUR vote-state requests.
///
/// Inputs:
/// - `aur_vote_state_req_rx`: Vote-state request channel receiver.
/// - `aur_vote_state_res_tx`: Vote-state response channel sender.
///
/// Output:
/// - None (spawns async task).
///
/// Details:
/// - Uses `spawn_blocking` for each request to keep UI rendering responsive.
pub fn spawn_aur_vote_state_worker(
    mut aur_vote_state_req_rx: mpsc::UnboundedReceiver<AurVoteStateRequest>,
    aur_vote_state_res_tx: mpsc::UnboundedSender<AurVoteStateResponse>,
) {
    tokio::spawn(async move {
        while let Some(request) = aur_vote_state_req_rx.recv().await {
            let res_tx = aur_vote_state_res_tx.clone();
            tokio::task::spawn_blocking(move || {
                let response = process_vote_state_request(request);
                let _ = res_tx.send(response);
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What: Build a request fixture for worker tests.
    ///
    /// Inputs:
    /// - `dry_run`: Whether request should execute in dry-run mode.
    /// - `ssh_command`: SSH command to include in request context.
    ///
    /// Output:
    /// - `AurVoteRequest` with stable package/action defaults.
    ///
    /// Details:
    /// - Uses `pacsea-bin` and `Vote` for deterministic assertions.
    fn test_request(dry_run: bool, ssh_command: &str) -> AurVoteRequest {
        AurVoteRequest {
            pkgbase: "pacsea-bin".to_string(),
            action: VoteAction::Vote,
            dry_run,
            ssh_timeout_secs: 10,
            ssh_command: ssh_command.to_string(),
        }
    }

    #[test]
    /// What: Ensure dry-run requests return a successful simulated outcome.
    ///
    /// Inputs:
    /// - `AurVoteRequest` with `dry_run=true`.
    ///
    /// Output:
    /// - Response is `Ok` and outcome marks dry-run.
    ///
    /// Details:
    /// - Verifies worker request processing preserves dry-run behavior from
    ///   the core vote service.
    fn process_vote_request_dry_run_success() {
        let response = process_vote_request(test_request(true, "ssh"));
        let outcome = response
            .result
            .expect("dry-run vote request should succeed");
        assert!(outcome.dry_run);
        assert_eq!(outcome.pkgbase, "pacsea-bin");
        assert_eq!(outcome.action, VoteAction::Vote);
    }

    #[test]
    /// What: Ensure missing SSH command is mapped to `SshNotFound`.
    ///
    /// Inputs:
    /// - Non-dry-run request with a guaranteed missing command.
    ///
    /// Output:
    /// - Response contains `AurVoteError::SshNotFound`.
    ///
    /// Details:
    /// - Uses a command name that should not exist to get deterministic
    ///   failure without network dependency.
    fn process_vote_request_missing_ssh_binary_error() {
        let response = process_vote_request(test_request(false, "__pacsea_missing_ssh__"));
        match response.result {
            Err(AurVoteError::SshNotFound(cmd)) => {
                assert_eq!(cmd, "__pacsea_missing_ssh__");
            }
            other => panic!("expected SshNotFound, got {other:?}"),
        }
    }

    #[test]
    /// What: Ensure vote-state checks map missing SSH command to `SshNotFound`.
    ///
    /// Inputs:
    /// - Vote-state request with a guaranteed missing command.
    ///
    /// Output:
    /// - Response contains `AurVoteError::SshNotFound`.
    ///
    /// Details:
    /// - Matches vote worker mapping behavior for consistency.
    fn process_vote_state_request_missing_ssh_binary_error() {
        let response = process_vote_state_request(AurVoteStateRequest {
            pkgbase: "pacsea-bin".to_string(),
            ssh_timeout_secs: 10,
            ssh_command: "__pacsea_missing_ssh__".to_string(),
        });
        assert_eq!(response.pkgbase, "pacsea-bin");
        match response.result {
            Err(AurVoteError::SshNotFound(cmd)) => {
                assert_eq!(cmd, "__pacsea_missing_ssh__");
            }
            other => panic!("expected SshNotFound, got {other:?}"),
        }
    }
}
