//! AUR package voting via SSH.
//!
//! ## Contract (verified from aurweb v6.3.4)
//!
//! **Transport**: `ssh aur@aur.archlinux.org {vote|unvote} <pkgbase>`
//!
//! **Auth**: User's SSH public key must be uploaded to their AUR account.
//! No passwords, cookies, or tokens are involved.
//!
//! **Success**: exit code 0, empty stdout/stderr.
//!
//! **Failure signals** (exit code 1, message on stderr):
//! - `"already voted for package base: {name}"` — duplicate vote
//! - `"missing vote for package base: {name}"` — unvote when not voted
//! - `"package base not found: {name}"` — invalid pkgbase
//! - `"The AUR is down due to maintenance"` — maintenance window
//! - `"The SSH interface is disabled for your IP address"` — IP ban
//!
//! **SSH-level failures**: exit code 255 for auth/connection issues.
//!
//! See `dev/ROADMAP/PRIORITY_ssh_aur_voting.md` "Phase 0 verified contract" for
//! the full mapping table.

use std::fmt;
use std::process::{Command, Output, Stdio};

/// AUR host used for SSH voting commands.
const AUR_SSH_HOST: &str = "aur@aur.archlinux.org";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// What: Voting action to perform on an AUR package base.
///
/// Details:
/// - `Vote` adds a vote for the package base.
/// - `Unvote` removes an existing vote.
/// - AUR does not support downvotes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoteAction {
    /// Add a vote for the package base.
    Vote,
    /// Remove an existing vote from the package base.
    Unvote,
}

impl VoteAction {
    /// What: Return the SSH subcommand string for this action.
    ///
    /// Output:
    /// - `"vote"` or `"unvote"`.
    const fn as_ssh_arg(self) -> &'static str {
        match self {
            Self::Vote => "vote",
            Self::Unvote => "unvote",
        }
    }
}

impl fmt::Display for VoteAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Vote => write!(f, "Vote"),
            Self::Unvote => write!(f, "Unvote"),
        }
    }
}

/// What: Live vote-state of the current user for one AUR package base.
///
/// Details:
/// - `Voted` means the account currently has an active vote.
/// - `NotVoted` means no vote exists for the package base.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AurPackageVoteState {
    /// User has voted for this package base.
    Voted,
    /// User has not voted for this package base.
    NotVoted,
}

/// What: Successful outcome of an AUR vote operation.
///
/// Details:
/// - Carries the action performed and the target package base name.
/// - `message()` returns a user-facing confirmation string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AurVoteOutcome {
    /// The action that was performed.
    pub action: VoteAction,
    /// The package base name that was voted on.
    pub pkgbase: String,
    /// Whether this was a dry-run (simulated) operation.
    pub dry_run: bool,
}

impl AurVoteOutcome {
    /// What: Build a user-facing message describing the outcome.
    ///
    /// Output:
    /// - A human-readable string suitable for toast/modal display.
    #[must_use]
    pub fn message(&self) -> String {
        if self.dry_run {
            return match self.action {
                VoteAction::Vote => {
                    format!("[dry-run] Would vote for '{}'", self.pkgbase)
                }
                VoteAction::Unvote => {
                    format!("[dry-run] Would remove vote for '{}'", self.pkgbase)
                }
            };
        }
        match self.action {
            VoteAction::Vote => format!("Voted for '{}'", self.pkgbase),
            VoteAction::Unvote => format!("Removed vote for '{}'", self.pkgbase),
        }
    }
}

/// What: Typed error variants for AUR vote failures.
///
/// Details:
/// - Each variant maps to a specific upstream failure signal.
/// - `Display` impl produces user-facing actionable messages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AurVoteError {
    /// User has already voted for this package base.
    AlreadyVoted(String),
    /// User has not voted for this package base (cannot unvote).
    NotVoted(String),
    /// Package base does not exist on AUR.
    NotFound(String),
    /// SSH key authentication failed.
    AuthFailed(String),
    /// AUR is under maintenance.
    Maintenance,
    /// SSH interface is disabled for the user's IP address.
    Banned,
    /// Connection timed out.
    Timeout(String),
    /// Network/DNS resolution failure.
    NetworkError(String),
    /// SSH binary was not found on the system.
    SshNotFound(String),
    /// Unexpected error with raw stderr.
    Unexpected(String),
}

impl fmt::Display for AurVoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyVoted(pkg) => {
                write!(f, "You have already voted for '{pkg}'")
            }
            Self::NotVoted(pkg) => {
                write!(f, "You haven't voted for '{pkg}'")
            }
            Self::NotFound(pkg) => {
                write!(f, "Package base '{pkg}' not found on AUR")
            }
            Self::AuthFailed(detail) => {
                write!(
                    f,
                    "SSH auth failed. Ensure your SSH key is uploaded to your AUR account \
                     at https://aur.archlinux.org/account ({detail})"
                )
            }
            Self::Maintenance => {
                write!(f, "AUR is under maintenance. Try again later")
            }
            Self::Banned => {
                write!(f, "SSH interface disabled for your IP. Contact AUR support")
            }
            Self::Timeout(detail) => {
                write!(
                    f,
                    "Connection to aur.archlinux.org timed out. Check network ({detail})"
                )
            }
            Self::NetworkError(detail) => {
                write!(
                    f,
                    "Could not connect to aur.archlinux.org. Check connectivity ({detail})"
                )
            }
            Self::SshNotFound(cmd) => {
                write!(
                    f,
                    "SSH binary '{cmd}' not found. Install openssh or configure \
                     aur_vote_ssh_command in settings"
                )
            }
            Self::Unexpected(detail) => {
                write!(f, "AUR vote failed unexpectedly: {detail}")
            }
        }
    }
}

impl std::error::Error for AurVoteError {}

/// What: Configuration context for an AUR vote operation.
///
/// Details:
/// - `dry_run`: when true, no SSH subprocess is spawned.
/// - `ssh_timeout_secs`: passed as `-o ConnectTimeout=N` to SSH.
/// - `ssh_command`: path or name of the SSH binary (default: `"ssh"`).
#[derive(Clone, Debug)]
pub struct AurVoteContext {
    /// If true, simulate the vote without network activity.
    pub dry_run: bool,
    /// SSH connect timeout in seconds.
    pub ssh_timeout_secs: u32,
    /// SSH binary path or name.
    pub ssh_command: String,
}

impl Default for AurVoteContext {
    fn default() -> Self {
        Self {
            dry_run: false,
            ssh_timeout_secs: 10,
            ssh_command: "ssh".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Transport trait + implementations
// ---------------------------------------------------------------------------

/// What: Abstraction over SSH subprocess execution for testability.
///
/// Details:
/// - `RealSshTransport` spawns the actual SSH process.
/// - Test code uses `MockSshTransport` to return configurable results.
trait SshVoteTransport {
    /// What: Execute an SSH vote/unvote command.
    ///
    /// Inputs:
    /// - `action`: vote or unvote.
    /// - `pkgbase`: target package base name.
    /// - `ctx`: configuration (SSH binary, timeout).
    ///
    /// Output:
    /// - `Ok(Output)` on process completion (even if exit != 0).
    /// - `Err` if the process could not be spawned.
    fn execute(
        &self,
        action: VoteAction,
        pkgbase: &str,
        ctx: &AurVoteContext,
    ) -> std::io::Result<Output>;
}

/// What: Real SSH transport that spawns `ssh aur@aur.archlinux.org`.
///
/// Details:
/// - Uses `-o BatchMode=yes` to prevent interactive password prompts.
/// - Uses `-o ConnectTimeout=N` to bound connection time.
/// - stdin is null; stdout and stderr are piped for capture.
struct RealSshTransport;

impl SshVoteTransport for RealSshTransport {
    fn execute(
        &self,
        action: VoteAction,
        pkgbase: &str,
        ctx: &AurVoteContext,
    ) -> std::io::Result<Output> {
        let timeout_arg = format!("ConnectTimeout={}", ctx.ssh_timeout_secs);
        Command::new(&ctx.ssh_command)
            .args([
                "-o",
                &timeout_arg,
                "-o",
                "BatchMode=yes",
                AUR_SSH_HOST,
                action.as_ssh_arg(),
                pkgbase,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
    }
}

// ---------------------------------------------------------------------------
// SSH result parsing
// ---------------------------------------------------------------------------

/// SSH exit code 255 indicates an SSH-level failure (auth, connection, etc.).
const SSH_ERROR_EXIT_CODE: i32 = 255;
/// aurweb stderr fragment indicating unsupported `list-votes` command.
const LIST_VOTES_UNSUPPORTED_PATTERN: &str = "invalid command: list-votes";

/// What: Parse SSH subprocess output into a typed result.
///
/// Inputs:
/// - `output`: captured process output (exit code, stdout, stderr).
/// - `action`: the vote action that was attempted.
/// - `pkgbase`: the target package base name.
///
/// Output:
/// - `Ok(AurVoteOutcome)` on success (exit 0).
/// - `Err(AurVoteError)` with a specific variant for each failure mode.
///
/// Details:
/// - Exit 0 is the only success signal.
/// - Exit 1 with known stderr patterns maps to specific error variants.
/// - Exit 255 is an SSH-level auth/connection failure.
/// - Other failures are matched by stderr content, then fall through to
///   `Unexpected`.
fn parse_ssh_result(
    output: &Output,
    action: VoteAction,
    pkgbase: &str,
) -> Result<AurVoteOutcome, AurVoteError> {
    let exit_code = output.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr_trimmed = stderr.trim();

    if exit_code == 0 {
        return Ok(AurVoteOutcome {
            action,
            pkgbase: pkgbase.to_string(),
            dry_run: false,
        });
    }

    if exit_code == 1 {
        if stderr_trimmed.contains("already voted for package base") {
            return Err(AurVoteError::AlreadyVoted(pkgbase.to_string()));
        }
        if stderr_trimmed.contains("missing vote for package base") {
            return Err(AurVoteError::NotVoted(pkgbase.to_string()));
        }
        if stderr_trimmed.contains("package base not found") {
            return Err(AurVoteError::NotFound(pkgbase.to_string()));
        }
        if stderr_trimmed.contains("AUR is down due to maintenance") {
            return Err(AurVoteError::Maintenance);
        }
        if stderr_trimmed.contains("SSH interface is disabled") {
            return Err(AurVoteError::Banned);
        }
    }

    // Check timeout/network patterns before generic SSH 255, since SSH
    // connection failures also exit 255 but carry distinguishable stderr.
    if stderr_trimmed.contains("Connection timed out")
        || stderr_trimmed.contains("Connection refused")
    {
        return Err(AurVoteError::Timeout(sanitize_stderr(stderr_trimmed)));
    }

    if stderr_trimmed.contains("Could not resolve hostname")
        || stderr_trimmed.contains("Network is unreachable")
        || stderr_trimmed.contains("No route to host")
    {
        return Err(AurVoteError::NetworkError(sanitize_stderr(stderr_trimmed)));
    }

    if exit_code == SSH_ERROR_EXIT_CODE {
        return Err(AurVoteError::AuthFailed(sanitize_stderr(stderr_trimmed)));
    }

    Err(AurVoteError::Unexpected(sanitize_stderr(stderr_trimmed)))
}

/// What: Parse `list-votes` SSH output into a package vote-state result.
///
/// Inputs:
/// - `output`: Captured process output for `ssh aur@aur.archlinux.org list-votes`.
/// - `pkgbase`: Package base to resolve against the returned vote list.
///
/// Output:
/// - `Ok(AurPackageVoteState::Voted)` when `pkgbase` appears in the vote list.
/// - `Ok(AurPackageVoteState::NotVoted)` when command succeeds but package is absent.
/// - `Err(AurVoteError)` for auth/network/maintenance and other failures.
///
/// Details:
/// - Successful `list-votes` output is parsed as whitespace-delimited package names.
/// - Error mapping reuses existing vote-flow error variants for consistent UX handling.
fn parse_list_votes_result(
    output: &Output,
    pkgbase: &str,
) -> Result<AurPackageVoteState, AurVoteError> {
    let exit_code = output.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr_trimmed = stderr.trim();

    if exit_code == 0 {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let is_voted = stdout.split_whitespace().any(|name| name == pkgbase);
        return Ok(if is_voted {
            AurPackageVoteState::Voted
        } else {
            AurPackageVoteState::NotVoted
        });
    }

    if stderr_trimmed.contains("AUR is down due to maintenance") {
        return Err(AurVoteError::Maintenance);
    }
    if stderr_trimmed.contains(LIST_VOTES_UNSUPPORTED_PATTERN) {
        return Err(AurVoteError::Unexpected(
            "AUR SSH server does not support vote-state lookup.".to_string(),
        ));
    }
    if stderr_trimmed.contains("SSH interface is disabled") {
        return Err(AurVoteError::Banned);
    }
    if stderr_trimmed.contains("Connection timed out")
        || stderr_trimmed.contains("Connection refused")
    {
        return Err(AurVoteError::Timeout(sanitize_stderr(stderr_trimmed)));
    }
    if stderr_trimmed.contains("Could not resolve hostname")
        || stderr_trimmed.contains("Network is unreachable")
        || stderr_trimmed.contains("No route to host")
    {
        return Err(AurVoteError::NetworkError(sanitize_stderr(stderr_trimmed)));
    }
    if exit_code == SSH_ERROR_EXIT_CODE {
        return Err(AurVoteError::AuthFailed(sanitize_stderr(stderr_trimmed)));
    }

    Err(AurVoteError::Unexpected(sanitize_stderr(stderr_trimmed)))
}

/// What: Sanitize SSH stderr output before including in user-facing errors.
///
/// Inputs:
/// - `raw`: raw stderr text from the SSH subprocess.
///
/// Output:
/// - Cleaned string with sensitive paths redacted and length bounded.
///
/// Details:
/// - Removes lines containing identity file paths (`/home/.../.ssh/`).
/// - Truncates excessively long output to keep error messages readable.
fn sanitize_stderr(raw: &str) -> String {
    const MAX_LEN: usize = 200;
    let filtered: String = raw
        .lines()
        .filter(|line| !line.contains("/.ssh/") && !line.contains("identity file"))
        .collect::<Vec<_>>()
        .join("; ");

    if filtered.len() > MAX_LEN {
        format!("{}...", &filtered[..MAX_LEN])
    } else {
        filtered
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// What: Vote or unvote an AUR package base via SSH.
///
/// Inputs:
/// - `pkgbase`: the AUR package base name (e.g. `"pacsea-bin"`).
/// - `action`: `VoteAction::Vote` or `VoteAction::Unvote`.
/// - `ctx`: configuration (dry-run, SSH timeout, SSH binary).
///
/// Output:
/// - `Ok(AurVoteOutcome)` on success, with a user-facing `.message()`.
/// - `Err(AurVoteError)` on failure, with a user-facing `Display` impl.
///
/// Details:
/// - In dry-run mode, returns a simulated outcome without spawning SSH.
/// - Uses `-o BatchMode=yes` to prevent interactive prompts.
/// - Uses `-o ConnectTimeout=N` to bound connection time.
/// - Never logs SSH key paths or identity file contents.
///
/// # Errors
///
/// Returns `AurVoteError` for auth failure, package not found, network
/// issues, SSH binary missing, AUR maintenance, IP ban, or unexpected
/// upstream errors. Each variant has a user-facing `Display` message.
pub fn aur_vote(
    pkgbase: &str,
    action: VoteAction,
    ctx: &AurVoteContext,
) -> Result<AurVoteOutcome, AurVoteError> {
    aur_vote_with_transport(&RealSshTransport, pkgbase, action, ctx)
}

/// What: Check whether the current AUR account has voted for a package base.
///
/// Inputs:
/// - `pkgbase`: Target AUR package base name.
/// - `ctx`: SSH execution context (timeout and SSH command).
///
/// Output:
/// - `Ok(AurPackageVoteState)` on successful state retrieval.
/// - `Err(AurVoteError)` for SSH/auth/network/maintenance and unexpected failures.
///
/// Details:
/// - Uses `ssh aur@aur.archlinux.org list-votes` and checks membership client-side.
/// - This is read-only and does not mutate vote state.
///
/// # Errors
///
/// Returns `AurVoteError` when SSH command execution fails, authentication is
/// invalid, network connectivity fails, AUR is in maintenance mode, the caller
/// IP is blocked, or upstream output cannot be mapped.
pub fn aur_vote_state(
    pkgbase: &str,
    ctx: &AurVoteContext,
) -> Result<AurPackageVoteState, AurVoteError> {
    let timeout_arg = format!("ConnectTimeout={}", ctx.ssh_timeout_secs);
    let output = Command::new(&ctx.ssh_command)
        .args([
            "-o",
            &timeout_arg,
            "-o",
            "BatchMode=yes",
            AUR_SSH_HOST,
            "list-votes",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => AurVoteError::SshNotFound(ctx.ssh_command.clone()),
            _ => AurVoteError::NetworkError(e.to_string()),
        })?;

    parse_list_votes_result(&output, pkgbase)
}

/// What: Determine whether a vote-state error indicates unsupported upstream command.
///
/// Inputs:
/// - `error`: Typed vote error returned by `aur_vote_state`.
///
/// Output:
/// - `true` if the upstream SSH endpoint rejected `list-votes` as invalid.
///
/// Details:
/// - Used by UI event-loop mapping to degrade gracefully to `Unknown` instead of
///   showing persistent inline errors for unsupported read-only lookups.
#[must_use]
pub fn is_vote_state_unsupported_error(error: &AurVoteError) -> bool {
    match error {
        AurVoteError::Unexpected(detail) => detail.contains("does not support vote-state lookup"),
        _ => false,
    }
}

/// What: Internal entry point parameterised on transport for testability.
///
/// Inputs:
/// - `transport`: the SSH transport implementation.
/// - `pkgbase`: the AUR package base name.
/// - `action`: vote or unvote.
/// - `ctx`: configuration context.
///
/// Output:
/// - Same as `aur_vote`.
///
/// Details:
/// - Dry-run is checked before transport is invoked.
/// - `io::Error` from subprocess spawn is mapped to `SshNotFound` or `NetworkError`.
fn aur_vote_with_transport<T: SshVoteTransport>(
    transport: &T,
    pkgbase: &str,
    action: VoteAction,
    ctx: &AurVoteContext,
) -> Result<AurVoteOutcome, AurVoteError> {
    if ctx.dry_run {
        return Ok(AurVoteOutcome {
            action,
            pkgbase: pkgbase.to_string(),
            dry_run: true,
        });
    }

    let output = transport
        .execute(action, pkgbase, ctx)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => AurVoteError::SshNotFound(ctx.ssh_command.clone()),
            _ => AurVoteError::NetworkError(e.to_string()),
        })?;

    parse_ssh_result(&output, action, pkgbase)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    /// Mock transport that returns a configurable exit code and stderr.
    struct MockSshTransport {
        exit_code: i32,
        stderr: String,
    }

    impl MockSshTransport {
        fn new(exit_code: i32, stderr: &str) -> Self {
            Self {
                exit_code,
                stderr: stderr.to_string(),
            }
        }
    }

    impl SshVoteTransport for MockSshTransport {
        fn execute(
            &self,
            _action: VoteAction,
            _pkgbase: &str,
            _ctx: &AurVoteContext,
        ) -> std::io::Result<Output> {
            Ok(Output {
                status: ExitStatus::from_raw(self.exit_code << 8),
                stdout: Vec::new(),
                stderr: self.stderr.as_bytes().to_vec(),
            })
        }
    }

    /// Mock transport that returns an `io::Error` on execute.
    struct FailingTransport {
        kind: std::io::ErrorKind,
    }

    impl SshVoteTransport for FailingTransport {
        fn execute(
            &self,
            _action: VoteAction,
            _pkgbase: &str,
            _ctx: &AurVoteContext,
        ) -> std::io::Result<Output> {
            Err(std::io::Error::new(self.kind, "mock io error"))
        }
    }

    fn default_ctx() -> AurVoteContext {
        AurVoteContext::default()
    }

    fn dry_run_ctx() -> AurVoteContext {
        AurVoteContext {
            dry_run: true,
            ..AurVoteContext::default()
        }
    }

    #[test]
    fn test_dry_run_vote() {
        let transport = MockSshTransport::new(0, "");
        let result =
            aur_vote_with_transport(&transport, "pacsea-bin", VoteAction::Vote, &dry_run_ctx());
        let outcome = result.expect("dry-run vote should succeed");
        assert!(outcome.dry_run);
        assert_eq!(outcome.action, VoteAction::Vote);
        assert!(outcome.message().contains("[dry-run]"));
        assert!(outcome.message().contains("pacsea-bin"));
    }

    #[test]
    fn test_dry_run_unvote() {
        let transport = MockSshTransport::new(0, "");
        let result =
            aur_vote_with_transport(&transport, "pacsea-bin", VoteAction::Unvote, &dry_run_ctx());
        let outcome = result.expect("dry-run unvote should succeed");
        assert!(outcome.dry_run);
        assert_eq!(outcome.action, VoteAction::Unvote);
        assert!(outcome.message().contains("[dry-run]"));
        assert!(outcome.message().contains("remove vote"));
    }

    #[test]
    fn test_success_vote() {
        let transport = MockSshTransport::new(0, "");
        let result =
            aur_vote_with_transport(&transport, "yay-bin", VoteAction::Vote, &default_ctx());
        let outcome = result.expect("success vote should return outcome");
        assert!(!outcome.dry_run);
        assert_eq!(outcome.action, VoteAction::Vote);
        assert_eq!(outcome.pkgbase, "yay-bin");
        assert!(outcome.message().contains("Voted for"));
    }

    #[test]
    fn test_success_unvote() {
        let transport = MockSshTransport::new(0, "");
        let result =
            aur_vote_with_transport(&transport, "yay-bin", VoteAction::Unvote, &default_ctx());
        let outcome = result.expect("success unvote should return outcome");
        assert_eq!(outcome.action, VoteAction::Unvote);
        assert!(outcome.message().contains("Removed vote"));
    }

    #[test]
    fn test_already_voted() {
        let transport = MockSshTransport::new(1, "vote: already voted for package base: yay-bin\n");
        let result =
            aur_vote_with_transport(&transport, "yay-bin", VoteAction::Vote, &default_ctx());
        match result {
            Err(AurVoteError::AlreadyVoted(pkg)) => assert_eq!(pkg, "yay-bin"),
            other => panic!("expected AlreadyVoted, got {other:?}"),
        }
    }

    #[test]
    fn test_not_voted() {
        let transport =
            MockSshTransport::new(1, "unvote: missing vote for package base: yay-bin\n");
        let result =
            aur_vote_with_transport(&transport, "yay-bin", VoteAction::Unvote, &default_ctx());
        match result {
            Err(AurVoteError::NotVoted(pkg)) => assert_eq!(pkg, "yay-bin"),
            other => panic!("expected NotVoted, got {other:?}"),
        }
    }

    #[test]
    fn test_package_not_found() {
        let transport = MockSshTransport::new(1, "vote: package base not found: nonexistent-pkg\n");
        let result = aur_vote_with_transport(
            &transport,
            "nonexistent-pkg",
            VoteAction::Vote,
            &default_ctx(),
        );
        match result {
            Err(AurVoteError::NotFound(pkg)) => assert_eq!(pkg, "nonexistent-pkg"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn test_auth_failure() {
        let transport = MockSshTransport::new(
            255,
            "Permission denied (publickey).\r\nfatal: Could not read from remote repository.",
        );
        let result = aur_vote_with_transport(&transport, "foo", VoteAction::Vote, &default_ctx());
        match result {
            Err(AurVoteError::AuthFailed(detail)) => {
                assert!(detail.contains("Permission denied"));
            }
            other => panic!("expected AuthFailed, got {other:?}"),
        }
    }

    #[test]
    fn test_maintenance() {
        let transport = MockSshTransport::new(
            1,
            "The AUR is down due to maintenance. We will be back soon.\n",
        );
        let result = aur_vote_with_transport(&transport, "foo", VoteAction::Vote, &default_ctx());
        match result {
            Err(AurVoteError::Maintenance) => {}
            other => panic!("expected Maintenance, got {other:?}"),
        }
    }

    #[test]
    fn test_banned() {
        let transport =
            MockSshTransport::new(1, "The SSH interface is disabled for your IP address.\n");
        let result = aur_vote_with_transport(&transport, "foo", VoteAction::Vote, &default_ctx());
        match result {
            Err(AurVoteError::Banned) => {}
            other => panic!("expected Banned, got {other:?}"),
        }
    }

    #[test]
    fn test_ssh_not_found() {
        let transport = FailingTransport {
            kind: std::io::ErrorKind::NotFound,
        };
        let result = aur_vote_with_transport(&transport, "foo", VoteAction::Vote, &default_ctx());
        match result {
            Err(AurVoteError::SshNotFound(cmd)) => assert_eq!(cmd, "ssh"),
            other => panic!("expected SshNotFound, got {other:?}"),
        }
    }

    #[test]
    fn test_timeout() {
        let transport = MockSshTransport::new(
            255,
            "ssh: connect to host aur.archlinux.org port 22: Connection timed out\n",
        );
        let result = aur_vote_with_transport(&transport, "foo", VoteAction::Vote, &default_ctx());
        match result {
            Err(AurVoteError::Timeout(_)) => {}
            other => panic!("expected Timeout, got {other:?}"),
        }
    }

    #[test]
    fn test_network_error() {
        let transport = MockSshTransport::new(
            255,
            "ssh: Could not resolve hostname aur.archlinux.org: Name or service not known\n",
        );
        let result = aur_vote_with_transport(&transport, "foo", VoteAction::Vote, &default_ctx());
        match result {
            Err(AurVoteError::NetworkError(_)) => {}
            other => panic!("expected NetworkError, got {other:?}"),
        }
    }

    #[test]
    fn test_unexpected_error() {
        let transport = MockSshTransport::new(99, "something completely unexpected happened\n");
        let result = aur_vote_with_transport(&transport, "foo", VoteAction::Vote, &default_ctx());
        match result {
            Err(AurVoteError::Unexpected(msg)) => {
                assert!(msg.contains("something completely unexpected"));
            }
            other => panic!("expected Unexpected, got {other:?}"),
        }
    }

    #[test]
    fn test_sanitize_stderr_redacts_ssh_paths() {
        let raw = "debug1: Offering public key: /home/user/.ssh/id_ed25519\n\
                   Permission denied (publickey).";
        let sanitized = sanitize_stderr(raw);
        assert!(!sanitized.contains("/.ssh/"));
        assert!(sanitized.contains("Permission denied"));
    }

    #[test]
    fn test_sanitize_stderr_truncates_long_output() {
        let raw = "x".repeat(500);
        let sanitized = sanitize_stderr(&raw);
        assert!(sanitized.len() <= 203); // 200 + "..."
        assert!(sanitized.ends_with("..."));
    }

    #[test]
    fn test_vote_action_display() {
        assert_eq!(format!("{}", VoteAction::Vote), "Vote");
        assert_eq!(format!("{}", VoteAction::Unvote), "Unvote");
    }

    #[test]
    fn test_vote_action_ssh_arg() {
        assert_eq!(VoteAction::Vote.as_ssh_arg(), "vote");
        assert_eq!(VoteAction::Unvote.as_ssh_arg(), "unvote");
    }

    #[test]
    fn test_error_display_messages() {
        let err = AurVoteError::AlreadyVoted("foo".into());
        let msg = format!("{err}");
        assert!(msg.contains("already voted"));
        assert!(msg.contains("foo"));

        let err = AurVoteError::SshNotFound("ssh".into());
        let msg = format!("{err}");
        assert!(msg.contains("not found"));
        assert!(msg.contains("openssh"));
    }

    #[test]
    fn test_context_default() {
        let ctx = AurVoteContext::default();
        assert!(!ctx.dry_run);
        assert_eq!(ctx.ssh_timeout_secs, 10);
        assert_eq!(ctx.ssh_command, "ssh");
    }

    #[test]
    fn test_parse_list_votes_result_voted() {
        let output = Output {
            status: ExitStatus::from_raw(0),
            stdout: b"pacsea-bin\nyay-bin\n".to_vec(),
            stderr: Vec::new(),
        };
        let state = parse_list_votes_result(&output, "pacsea-bin")
            .expect("list-votes parsing should succeed");
        assert_eq!(state, AurPackageVoteState::Voted);
    }

    #[test]
    fn test_parse_list_votes_result_not_voted() {
        let output = Output {
            status: ExitStatus::from_raw(0),
            stdout: b"yay-bin\nparu-bin\n".to_vec(),
            stderr: Vec::new(),
        };
        let state = parse_list_votes_result(&output, "pacsea-bin")
            .expect("list-votes parsing should succeed");
        assert_eq!(state, AurPackageVoteState::NotVoted);
    }

    #[test]
    fn test_parse_list_votes_result_auth_failed() {
        let output = Output {
            status: ExitStatus::from_raw(255 << 8),
            stdout: Vec::new(),
            stderr: b"Permission denied (publickey).".to_vec(),
        };
        let result = parse_list_votes_result(&output, "pacsea-bin");
        match result {
            Err(AurVoteError::AuthFailed(detail)) => {
                assert!(detail.contains("Permission denied"));
            }
            other => panic!("expected AuthFailed, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_list_votes_result_unsupported_command() {
        let output = Output {
            status: ExitStatus::from_raw(1 << 8),
            stdout: Vec::new(),
            stderr: b"list-votes: invalid command: list-votes".to_vec(),
        };
        let result = parse_list_votes_result(&output, "pacsea-bin");
        match result {
            Err(AurVoteError::Unexpected(detail)) => {
                assert!(detail.contains("does not support vote-state lookup"));
            }
            other => panic!("expected Unexpected unsupported-command error, got {other:?}"),
        }
    }

    #[test]
    fn test_aur_vote_state_missing_ssh_binary_error() {
        let ctx = AurVoteContext {
            dry_run: false,
            ssh_timeout_secs: 10,
            ssh_command: "__pacsea_missing_ssh__".to_string(),
        };
        let result = aur_vote_state("pacsea-bin", &ctx);
        match result {
            Err(AurVoteError::SshNotFound(cmd)) => {
                assert_eq!(cmd, "__pacsea_missing_ssh__");
            }
            other => panic!("expected SshNotFound, got {other:?}"),
        }
    }
}
