//! Native official-AUR repository observation over an injectable direct-argv Git seam.
//!
//! Security invariants enforced here:
//!
//! - direct `argv` only; never `sh -c`, never a helper, never shell interpolation, and no
//!   quoting is required because no fragment is ever handed to a shell;
//! - Git is run with hooks, submodules, textconv, credential helpers, prompts, and proxies
//!   disabled through explicit `-c` overrides and a positive environment allowlist;
//! - HEAD queries are sequential and individually deadlined;
//! - every OID crossing this boundary is fully validated before it reaches durable state;
//! - unseen history expands oldest-first under hard per-package and per-cycle caps;
//! - non-ancestor (force-push/rewrite) history preserves the previous lineage and pauses
//!   for an explicit user rebaseline instead of fabricating continuity.
//!
//! The process seam is [`GitCommandRunner`]. Tests inject a fake runner, so no test in this
//! workstream performs a real Git invocation or touches the network.

use crate::logic::pi_scan::baseline::{CommitBuildRelevance, classify_commit_delta};
use crate::logic::pi_scan::identity::{AurRepoUrl, CommitOid, IdentityError, PackageBase};
use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::time::{Duration, Instant};

/// Maximum unseen commits expanded for one package base in one observation cycle.
pub const MAX_COMMIT_EXPANSION_PER_PACKAGE: usize = 500;

/// Maximum unseen commits expanded across every package base in one observation cycle.
pub const MAX_COMMIT_EXPANSION_PER_CYCLE: usize = 2_000;

/// Default per-HEAD-query wall-clock deadline.
pub const DEFAULT_HEAD_QUERY_TIMEOUT: Duration = Duration::from_secs(15);

/// Default whole-cycle wall-clock deadline for sequential head queries.
pub const DEFAULT_OBSERVATION_DEADLINE: Duration = Duration::from_secs(90);

/// Minimum interval between automatic observation cycles.
pub const OBSERVATION_INTERVAL_FLOOR: Duration = Duration::from_mins(15);

/// Hard cap on Git output accepted from one invocation.
pub const MAX_GIT_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

/// Environment names inherited verbatim by the Git child process.
///
/// This is a positive allowlist. Credential helpers, askpass hooks, proxies, SSH agent
/// sockets, and every future credential variable are excluded by construction.
pub const GIT_PASSTHROUGH_ENVIRONMENT: [&str; 5] = ["PATH", "HOME", "LANG", "LC_ALL", "LC_CTYPE"];

/// Environment values always forced on the Git child process.
pub const GIT_FIXED_ENVIRONMENT: [(&str, &str); 6] = [
    ("GIT_TERMINAL_PROMPT", "0"),
    ("GIT_ASKPASS", ""),
    ("SSH_ASKPASS", ""),
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_CONFIG_GLOBAL", "/dev/null"),
    ("GIT_CONFIG_SYSTEM", "/dev/null"),
];

/// Fixed `-c` overrides prepended to every Git argv built here.
///
/// These disable hook execution, submodule traversal, textconv filters, credential
/// helpers, and any inherited proxy before the subcommand is reached.
const GIT_ISOLATION_CONFIG: [&str; 22] = [
    "-c",
    "core.hooksPath=/dev/null",
    "-c",
    "core.askPass=",
    "-c",
    "credential.helper=",
    "-c",
    "diff.textconv=",
    "-c",
    "diff.external=",
    "-c",
    "protocol.allow=never",
    "-c",
    "protocol.https.allow=always",
    "-c",
    "submodule.recurse=false",
    "-c",
    "fetch.recurseSubmodules=no",
    "-c",
    "http.proxy=",
    "-c",
    "http.followRedirects=false",
];

/// What: Observation, Git-process, or validation failure with actionable guidance.
///
/// Inputs:
/// - Produced by the observer when a resolved Git binary, invocation, or output is unusable.
///
/// Output:
/// - A user-facing message that names what failed and what the user can do next.
///
/// Details:
/// - Every variant is inert: constructing an error never retries, mutates state, or logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserverError {
    /// No usable `git` executable was resolved.
    GitUnavailable {
        /// Reason resolution failed.
        reason: String,
    },
    /// A Git invocation failed to start, exited non-zero, or timed out.
    GitCommand {
        /// Human-readable operation name.
        operation: String,
        /// Reason reported by the runner or by Git.
        reason: String,
    },
    /// Git produced more output than the hard cap allows.
    OutputTooLarge {
        /// Human-readable operation name.
        operation: String,
        /// Observed byte count.
        observed: usize,
        /// Hard maximum byte count.
        limit: usize,
    },
    /// Git returned output that is not a valid full commit identity.
    InvalidOid {
        /// Human-readable operation name.
        operation: String,
        /// Underlying identity validation failure.
        source: IdentityError,
    },
    /// Observed history is not a descendant of the recorded lineage.
    HistoryDiverged {
        /// Affected canonical package base.
        package_base: String,
        /// Previously recorded cursor commit that is no longer reachable.
        previous_oid: String,
        /// Newly observed head commit.
        observed_oid: String,
    },
}

impl fmt::Display for ObserverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitUnavailable { reason } => write!(
                formatter,
                "AUR observation needs a working git executable but {reason}. Install git \
                 (pacman -S git) or disable Pi scanning in settings"
            ),
            Self::GitCommand { operation, reason } => write!(
                formatter,
                "git {operation} failed: {reason}. Check network access to \
                 aur.archlinux.org and retry the observation refresh"
            ),
            Self::OutputTooLarge {
                operation,
                observed,
                limit,
            } => write!(
                formatter,
                "git {operation} returned {observed} bytes, above the {limit}-byte safety \
                 limit; the response was discarded. Retry later or report this package base"
            ),
            Self::InvalidOid { operation, source } => write!(
                formatter,
                "git {operation} returned an unusable commit identity: {source}. Nothing was \
                 recorded; retry the observation refresh"
            ),
            Self::HistoryDiverged {
                package_base,
                previous_oid,
                observed_oid,
            } => write!(
                formatter,
                "the AUR history for '{package_base}' was rewritten: recorded commit \
                 {previous_oid} is no longer reachable from {observed_oid}. The previous \
                 lineage was kept and observation is paused; review the package and \
                 confirm an explicit rebaseline to continue"
            ),
        }
    }
}

impl std::error::Error for ObserverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidOid { source, .. } => Some(source),
            Self::GitUnavailable { .. }
            | Self::GitCommand { .. }
            | Self::OutputTooLarge { .. }
            | Self::HistoryDiverged { .. } => None,
        }
    }
}

/// What: One fully-specified direct-argv Git invocation.
///
/// Inputs:
/// - Resolved absolute executable, argv tail, environment policy, and a wall-clock deadline.
///
/// Output:
/// - Consumed by a [`GitCommandRunner`] implementation.
///
/// Details:
/// - `argv` never contains shell metacharacter handling because it is passed to `execve`
///   argument by argument. No fragment is quoted, joined, or interpolated into a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitInvocation {
    /// Resolved Git executable path.
    pub executable: OsString,
    /// Full argv tail after the executable.
    pub argv: Vec<OsString>,
    /// Environment names inherited verbatim, in fixed order.
    pub passthrough_environment: Vec<String>,
    /// Environment values forced on the child, in fixed order.
    pub fixed_environment: Vec<(String, String)>,
    /// Wall-clock deadline for this single invocation.
    pub timeout: Duration,
    /// Maximum stdout bytes accepted; more output fails the invocation as too large.
    pub stdout_limit: usize,
    /// Optional bound on bytes the child may write under one directory, such as a fetch.
    pub storage_limit: Option<StorageLimit>,
}

/// What: Bound on the bytes one Git invocation may store under a private directory.
///
/// Inputs:
/// - Set by callers whose Git command writes to disk instead of stdout (`fetch`).
///
/// Output:
/// - Enforced by [`SystemGitRunner`] while the child runs.
///
/// Details:
/// - The runner sums regular-file sizes under `dir` without following symlinks and kills the
///   process group once the total exceeds `max_bytes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageLimit {
    /// Private directory the child writes into.
    pub dir: std::path::PathBuf,
    /// Maximum total regular-file bytes allowed under `dir`.
    pub max_bytes: u64,
}

impl GitInvocation {
    /// What: Render the argv tail as lossy UTF-8 for assertions and dry-run previews.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - One string per argv element, in exact order.
    ///
    /// Details:
    /// - Rendering is display-only. The executed argv always remains the `OsString` form.
    #[must_use]
    pub fn argv_strings(&self) -> Vec<String> {
        self.argv
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    /// Add one explicit validated HTTPS proxy override after the isolation defaults.
    pub fn set_https_proxy(&mut self, proxy: &str) {
        self.argv.splice(
            GIT_ISOLATION_CONFIG.len()..GIT_ISOLATION_CONFIG.len(),
            [
                OsString::from("-c"),
                OsString::from(format!("http.proxy={proxy}")),
            ],
        );
    }
}

/// What: Captured result of one completed Git invocation.
///
/// Inputs: Produced by a [`GitCommandRunner`] implementation.
///
/// Output: Parsed by the observer after bounds and success checks.
///
/// Details:
/// - `stdout` is raw bytes so the observer can enforce its byte cap before any decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitOutput {
    /// Whether Git exited with status zero.
    pub success: bool,
    /// Raw standard output bytes.
    pub stdout: Vec<u8>,
    /// Bounded standard error text used only for actionable messages.
    pub stderr: String,
}

/// What: Injectable process seam for every Git invocation the observer performs.
///
/// Inputs:
/// - One fully-specified [`GitInvocation`].
///
/// Output:
/// - The captured [`GitOutput`], or a reason the invocation could not complete.
///
/// Details:
/// - This trait exists so observation logic stays pure and testable. Tests inject a fake
///   runner that records argv and returns canned output, so no test performs real Git or
///   network work.
///
/// # Errors
/// - Implementations return `Err` when the child cannot be spawned, exceeds its deadline,
///   or cannot be reaped.
pub trait GitCommandRunner {
    /// Execute one invocation and capture its bounded output.
    ///
    /// # Errors
    /// - Returns `Err` when spawning, waiting, or reaping the Git child fails.
    fn run(&mut self, invocation: &GitInvocation) -> Result<GitOutput, ObserverError>;
}

/// Git-runner decorator that adds one prevalidated explicit HTTPS proxy override.
pub struct ExplicitHttpsProxyGitRunner<'a> {
    /// Underlying direct-argv runner.
    inner: &'a mut dyn GitCommandRunner,
    /// Prevalidated credential-free HTTPS proxy.
    proxy: &'a str,
}

impl<'a> ExplicitHttpsProxyGitRunner<'a> {
    /// Construct a decorator from a prevalidated explicit HTTPS proxy.
    #[must_use]
    pub fn new(inner: &'a mut dyn GitCommandRunner, proxy: &'a str) -> Self {
        Self { inner, proxy }
    }
}

impl GitCommandRunner for ExplicitHttpsProxyGitRunner<'_> {
    fn run(&mut self, invocation: &GitInvocation) -> Result<GitOutput, ObserverError> {
        let mut invocation = invocation.clone();
        invocation.set_https_proxy(self.proxy);
        self.inner.run(&invocation)
    }
}

/// What: Build the isolated argv tail for one Git subcommand.
///
/// Inputs:
/// - `subcommand_args`: The subcommand and its arguments, already validated by the caller.
///
/// Output:
/// - The fixed isolation `-c` overrides followed by the supplied arguments, in order.
///
/// Details:
/// - The isolation prefix is always first so no caller-supplied argument can precede or
///   override it.
#[must_use]
pub fn git_argv(subcommand_args: &[&OsStr]) -> Vec<OsString> {
    let mut argv: Vec<OsString> = GIT_ISOLATION_CONFIG.iter().map(OsString::from).collect();
    argv.extend(subcommand_args.iter().map(|arg| (*arg).to_os_string()));
    argv
}

/// What: Build the exact sequential HEAD-query invocation for one official AUR repository.
///
/// Inputs:
/// - `executable`: Resolved Git executable path.
/// - `repo_url`: Canonical official AUR repository URL for the package base.
/// - `timeout`: Wall-clock deadline for this single query.
///
/// Output:
/// - A complete [`GitInvocation`] for `ls-remote --exit-code <url> HEAD`.
///
/// Details:
/// - The URL is produced by [`AurRepoUrl`], never by package-controlled text, and is passed
///   as its own argv element so no shell quoting is involved.
/// - `--` is not applicable to `ls-remote`; instead the URL is constrained by construction
///   to the canonical `https://aur.archlinux.org/<pkgbase>.git` form.
#[must_use]
pub fn head_query_invocation(
    executable: &OsStr,
    repo_url: &AurRepoUrl,
    timeout: Duration,
) -> GitInvocation {
    let url = OsString::from(repo_url.as_str());
    let args: [&OsStr; 4] = [
        OsStr::new("ls-remote"),
        OsStr::new("--exit-code"),
        url.as_os_str(),
        OsStr::new("HEAD"),
    ];
    build_invocation(executable, &args, timeout)
}

/// What: Build the invocation listing unseen commits oldest-first for one package base.
///
/// Inputs:
/// - `executable`: Resolved Git executable path.
/// - `repository_dir`: Private local mirror directory for this package base.
/// - `range`: Optional exclusive lower bound; `None` requests full reachable history.
/// - `head`: Observed head commit that bounds the walk.
/// - `timeout`: Wall-clock deadline for this single query.
///
/// Output:
/// - A complete [`GitInvocation`] for a reverse topological `rev-list`.
///
/// Details:
/// - `--reverse --topo-order` yields strict oldest-first order so commits can be inserted
///   and resumed without coalescing.
/// - No `--max-count`: Git applies it before `--reverse`, which would keep the newest
///   commits. [`parse_unseen_commits`] caps the oldest-first list instead; the stdout limit
///   bounds the listing at roughly 100,000 commits.
#[must_use]
pub fn unseen_commits_invocation(
    executable: &OsStr,
    repository_dir: &OsStr,
    range: Option<&CommitOid>,
    head: &CommitOid,
    timeout: Duration,
) -> GitInvocation {
    let dir = repository_dir.to_os_string();
    let revision = range.map_or_else(
        || OsString::from(head.as_str()),
        |from| OsString::from(format!("{}..{}", from.as_str(), head.as_str())),
    );
    let args: [&OsStr; 8] = [
        OsStr::new("-C"),
        dir.as_os_str(),
        OsStr::new("rev-list"),
        OsStr::new("--reverse"),
        OsStr::new("--topo-order"),
        OsStr::new("--no-walk=unsorted"),
        revision.as_os_str(),
        OsStr::new("--"),
    ];
    build_invocation(executable, &args, timeout)
}

/// What: Build the ancestry check that detects a rewritten or force-pushed history.
///
/// Inputs:
/// - `executable`: Resolved Git executable path.
/// - `repository_dir`: Private local mirror directory for this package base.
/// - `ancestor`: Previously recorded lineage commit.
/// - `descendant`: Newly observed head commit.
/// - `timeout`: Wall-clock deadline for this single query.
///
/// Output:
/// - A complete [`GitInvocation`] for `merge-base --is-ancestor`.
///
/// Details:
/// - Exit status zero means the recorded lineage is preserved; non-zero means the history
///   diverged and requires an explicit rebaseline.
#[must_use]
pub fn ancestry_invocation(
    executable: &OsStr,
    repository_dir: &OsStr,
    ancestor: &CommitOid,
    descendant: &CommitOid,
    timeout: Duration,
) -> GitInvocation {
    let dir = repository_dir.to_os_string();
    let ancestor_arg = OsString::from(ancestor.as_str());
    let descendant_arg = OsString::from(descendant.as_str());
    let args: [&OsStr; 6] = [
        OsStr::new("-C"),
        dir.as_os_str(),
        OsStr::new("merge-base"),
        OsStr::new("--is-ancestor"),
        ancestor_arg.as_os_str(),
        descendant_arg.as_os_str(),
    ];
    build_invocation(executable, &args, timeout)
}

/// What: Build the changed-path listing used for build-relevance classification.
///
/// Inputs:
/// - `executable`: Resolved Git executable path.
/// - `repository_dir`: Private local mirror directory for this package base.
/// - `commit`: Commit whose changed paths are requested.
/// - `timeout`: Wall-clock deadline for this single query.
///
/// Output:
/// - A complete [`GitInvocation`] for a name-only, textconv-free `show`.
///
/// Details:
/// - `--no-textconv` and `--no-renames` keep the output free of filter execution and of
///   rename heuristics that could hide a build-relevant path.
#[must_use]
pub fn changed_paths_invocation(
    executable: &OsStr,
    repository_dir: &OsStr,
    commit: &CommitOid,
    timeout: Duration,
) -> GitInvocation {
    let dir = repository_dir.to_os_string();
    let commit_arg = OsString::from(commit.as_str());
    let args: [&OsStr; 10] = [
        OsStr::new("-C"),
        dir.as_os_str(),
        OsStr::new("show"),
        OsStr::new("--name-only"),
        OsStr::new("--no-textconv"),
        OsStr::new("--no-renames"),
        OsStr::new("--pretty=format:"),
        OsStr::new("--no-color"),
        commit_arg.as_os_str(),
        OsStr::new("--"),
    ];
    build_invocation(executable, &args, timeout)
}

/// Assemble one invocation with the fixed isolation argv and environment policy.
fn build_invocation(executable: &OsStr, args: &[&OsStr], timeout: Duration) -> GitInvocation {
    GitInvocation {
        executable: executable.to_os_string(),
        argv: git_argv(args),
        passthrough_environment: GIT_PASSTHROUGH_ENVIRONMENT
            .iter()
            .map(|name| (*name).to_string())
            .collect(),
        fixed_environment: GIT_FIXED_ENVIRONMENT
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect(),
        timeout,
        stdout_limit: MAX_GIT_OUTPUT_BYTES,
        storage_limit: None,
    }
}

/// What: Extract and fully validate the commit identity from `ls-remote ... HEAD` output.
///
/// Inputs:
/// - `output`: Captured Git output for a head query.
///
/// Output:
/// - The validated head [`CommitOid`].
///
/// Details:
/// - Enforces the byte cap before decoding, requires success, and rejects any token that
///   is not a full 40-hex OID. Abbreviated or symbolic answers are never accepted.
///
/// # Errors
/// - Returns `ObserverError` when the invocation failed, exceeded the byte cap, produced no
///   usable line, or produced an invalid OID.
pub fn parse_head_oid(output: &GitOutput) -> Result<CommitOid, ObserverError> {
    let text = bounded_stdout(output, "ls-remote")?;
    let token = text
        .lines()
        .find_map(|line| line.split_whitespace().next())
        .ok_or_else(|| ObserverError::GitCommand {
            operation: "ls-remote".to_string(),
            reason: "the repository reported no HEAD reference".to_string(),
        })?;
    CommitOid::new(token).map_err(|source| ObserverError::InvalidOid {
        operation: "ls-remote".to_string(),
        source,
    })
}

/// What: Parse an oldest-first `rev-list` response into validated commit identities.
///
/// Inputs:
/// - `output`: Captured Git output for an unseen-commit query.
/// - `remaining_cycle_budget`: Commits still expandable in this observation cycle.
///
/// Output:
/// - Validated commits in oldest-first order plus whether expansion was truncated.
///
/// Details:
/// - Applies the per-package cap and the remaining per-cycle cap, whichever binds first.
/// - Truncation is reported rather than silently dropped so the cycle can resume exactly
///   where it stopped without coalescing skipped commits.
///
/// # Errors
/// - Returns `ObserverError` when the invocation failed, exceeded the byte cap, or produced
///   a token that is not a full OID.
pub fn parse_unseen_commits(
    output: &GitOutput,
    remaining_cycle_budget: usize,
) -> Result<UnseenCommits, ObserverError> {
    let text = bounded_stdout(output, "rev-list")?;
    let cap = MAX_COMMIT_EXPANSION_PER_PACKAGE.min(remaining_cycle_budget);
    let mut commits = Vec::new();
    let mut truncated = false;
    for line in text.lines() {
        let token = line.trim();
        if token.is_empty() {
            continue;
        }
        if commits.len() == cap {
            truncated = true;
            break;
        }
        let oid = CommitOid::new(token).map_err(|source| ObserverError::InvalidOid {
            operation: "rev-list".to_string(),
            source,
        })?;
        commits.push(oid);
    }
    Ok(UnseenCommits { commits, truncated })
}

/// What: Bounded oldest-first expansion result for one package base.
///
/// Inputs: Produced by [`parse_unseen_commits`].
///
/// Output: Durable insertion input for the backlog ledger.
///
/// Details:
/// - `truncated` means more unseen history exists and the next cycle must resume from the
///   last inserted commit rather than jumping to the observed head.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnseenCommits {
    /// Validated commits in strict oldest-first order.
    pub commits: Vec<CommitOid>,
    /// Whether a hard cap stopped expansion before history was exhausted.
    pub truncated: bool,
}

/// What: Parse changed paths and classify one commit's build relevance.
///
/// Inputs:
/// - `output`: Captured Git output for a changed-path query.
///
/// Output:
/// - The commit's [`CommitBuildRelevance`] classification.
///
/// Details:
/// - Delegates the actual classification to the WS1 classifier so the paid-scan trigger has
///   exactly one definition.
/// - Empty output classifies as `Uncertain`, which queues the commit rather than dropping it.
///
/// # Errors
/// - Returns `ObserverError` when the invocation failed or exceeded the byte cap.
pub fn classify_observed_commit(output: &GitOutput) -> Result<CommitBuildRelevance, ObserverError> {
    let text = bounded_stdout(output, "show --name-only")?;
    let paths: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    Ok(classify_commit_delta(&paths))
}

/// What: Decide whether observed history preserves the recorded lineage.
///
/// Inputs:
/// - `package_base`: Canonical package base under observation.
/// - `previous`: Previously recorded cursor commit, when one exists.
/// - `observed_head`: Newly observed head commit.
/// - `ancestry`: Captured `merge-base --is-ancestor` output, when a check was performed.
///
/// Output:
/// - `Ok(())` when the lineage is intact or when there is nothing to compare.
///
/// Details:
/// - A failed ancestry check is a rewritten or force-pushed history. The caller must keep
///   the previous lineage untouched and pause the package for explicit rebaseline.
///
/// # Errors
/// - Returns `ObserverError::HistoryDiverged` when the recorded commit is unreachable.
pub fn verify_lineage_preserved(
    package_base: &PackageBase,
    previous: Option<&CommitOid>,
    observed_head: &CommitOid,
    ancestry: Option<&GitOutput>,
) -> Result<(), ObserverError> {
    let Some(previous_oid) = previous else {
        return Ok(());
    };
    if previous_oid == observed_head {
        return Ok(());
    }
    let is_ancestor = ancestry.is_some_and(|output| output.success);
    if is_ancestor {
        return Ok(());
    }
    Err(ObserverError::HistoryDiverged {
        package_base: package_base.as_str().to_string(),
        previous_oid: previous_oid.as_str().to_string(),
        observed_oid: observed_head.as_str().to_string(),
    })
}

/// What: Outcome of observing one package base within a cycle.
///
/// Inputs: Produced by [`observe_package_base`].
///
/// Output: Ledger, cursor, and pause input for the runtime owner.
///
/// Details:
/// - `paused_for_rebaseline` is set only when lineage verification failed; in that case the
///   cursor must not advance and no commit is queued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageObservation {
    /// Canonical package base observed.
    pub package_base: PackageBase,
    /// Head commit reported by the official repository.
    pub head_oid: CommitOid,
    /// Unseen commits in oldest-first order with their classification.
    pub commits: Vec<ObservedCommit>,
    /// Whether a hard cap stopped expansion before history was exhausted.
    pub truncated: bool,
    /// Whether the package requires an explicit user rebaseline before continuing.
    pub paused_for_rebaseline: bool,
}

/// What: One observed commit with its build-relevance classification.
///
/// Inputs: Produced by [`observe_package_base`].
///
/// Output: Durable ledger entry input.
///
/// Details:
/// - `ObservedNoRecipeDelta` commits are ledgered without a paid scan; `BuildRelevant` and
///   `Uncertain` commits are queued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedCommit {
    /// Validated commit identity.
    pub oid: CommitOid,
    /// Build relevance decided from the commit's changed paths.
    pub relevance: CommitBuildRelevance,
}

impl ObservedCommit {
    /// Return whether this commit requires a paid model scan.
    #[must_use]
    pub const fn requires_scan(&self) -> bool {
        matches!(
            self.relevance,
            CommitBuildRelevance::BuildRelevant | CommitBuildRelevance::Uncertain
        )
    }
}

/// What: Sequential observation cycle budget shared by every package base.
///
/// Inputs:
/// - Constructed once per cycle by the runtime owner.
///
/// Output:
/// - Remaining per-cycle expansion budget and per-query deadlines.
///
/// Details:
/// - The budget is decremented as commits are expanded so the 2,000-commit cycle cap holds
///   across packages, not merely per package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationCycle {
    /// Commits still expandable in this cycle.
    remaining_budget: usize,
    /// Per-HEAD-query deadline.
    head_query_timeout: Duration,
    /// Monotonic start of the whole observation cycle.
    started_at: Instant,
    /// Whole-cycle deadline.
    cycle_deadline: Duration,
}

impl Default for ObservationCycle {
    fn default() -> Self {
        Self::new(DEFAULT_HEAD_QUERY_TIMEOUT)
    }
}

impl ObservationCycle {
    /// What: Start a cycle with the full per-cycle expansion budget.
    ///
    /// Inputs:
    /// - `head_query_timeout`: Per-query deadline, clamped to the compiled maximum.
    ///
    /// Output:
    /// - A cycle with `MAX_COMMIT_EXPANSION_PER_CYCLE` remaining.
    ///
    /// Details:
    /// - The timeout is clamped down only; a configured value can never raise the
    ///   compiled security maximum.
    #[must_use]
    pub fn new(head_query_timeout: Duration) -> Self {
        Self::with_deadline(head_query_timeout, Duration::from_secs(90))
    }

    /// Start a cycle with explicit clamp-down-only per-query and whole-cycle deadlines.
    #[must_use]
    pub fn with_deadline(head_query_timeout: Duration, cycle_deadline: Duration) -> Self {
        Self {
            remaining_budget: MAX_COMMIT_EXPANSION_PER_CYCLE,
            head_query_timeout: head_query_timeout.min(DEFAULT_HEAD_QUERY_TIMEOUT),
            started_at: Instant::now(),
            cycle_deadline: cycle_deadline.min(Duration::from_secs(90)),
        }
    }

    /// Clamp one Git invocation to the remaining whole-cycle deadline.
    ///
    /// # Errors
    /// - Returns a bounded command error once the whole-cycle deadline is exhausted.
    pub fn bounded_invocation(
        &self,
        mut invocation: GitInvocation,
    ) -> Result<GitInvocation, ObserverError> {
        invocation.timeout = invocation.timeout.min(self.remaining_deadline()?);
        Ok(invocation)
    }

    /// Return the remaining whole-cycle wall-clock allowance.
    ///
    /// # Errors
    /// - Returns a bounded command error once the whole-cycle deadline is exhausted.
    pub fn remaining_deadline(&self) -> Result<Duration, ObserverError> {
        self.cycle_deadline
            .checked_sub(self.started_at.elapsed())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| ObserverError::GitCommand {
                operation: "observation-cycle".to_string(),
                reason: "the 90-second whole observation deadline was exhausted".to_string(),
            })
    }

    /// Return the commits still expandable in this cycle.
    #[must_use]
    pub const fn remaining_budget(&self) -> usize {
        self.remaining_budget
    }

    /// Return the effective per-HEAD-query deadline.
    #[must_use]
    pub const fn head_query_timeout(&self) -> Duration {
        self.head_query_timeout
    }

    /// Return whether the cycle can still expand any commit.
    #[must_use]
    pub const fn is_exhausted(&self) -> bool {
        self.remaining_budget == 0
    }

    /// Consume expansion budget without wrapping below zero.
    const fn consume(&mut self, count: usize) {
        self.remaining_budget = self.remaining_budget.saturating_sub(count);
    }
}

/// What: Observe one package base end to end through the injected Git seam.
///
/// Inputs:
/// - `runner`: Injectable Git process seam.
/// - `executable`: Resolved Git executable path.
/// - `repository_dir`: Private local mirror directory for this package base.
/// - `package_base`: Canonical package base under observation.
/// - `previous_cursor`: Last observed commit, when one exists.
/// - `cycle`: Shared per-cycle expansion budget and deadlines.
///
/// Output:
/// - The package observation, including any pause for explicit rebaseline.
///
/// Details:
/// - Queries are strictly sequential: head, then ancestry, then expansion, then one
///   classification per expanded commit.
/// - A diverged history returns `paused_for_rebaseline` with no commits and never advances
///   the cursor, so the previous lineage is preserved verbatim.
///
/// # Errors
/// - Returns `ObserverError` when a Git invocation fails, exceeds a bound, or yields an
///   invalid commit identity. History divergence is reported in the observation rather
///   than as an error so the caller can persist the pause.
pub fn observe_package_base(
    runner: &mut dyn GitCommandRunner,
    executable: &OsStr,
    repository_dir: &OsStr,
    package_base: &PackageBase,
    previous_cursor: Option<&CommitOid>,
    cycle: &mut ObservationCycle,
) -> Result<PackageObservation, ObserverError> {
    let repo_url = AurRepoUrl::for_package_base(package_base);
    let head_invocation = cycle.bounded_invocation(head_query_invocation(
        executable,
        &repo_url,
        cycle.head_query_timeout(),
    ))?;
    let head_output = runner.run(&head_invocation)?;
    let head_oid = parse_head_oid(&head_output)?;

    if let Some(previous) = previous_cursor
        && previous != &head_oid
    {
        let ancestry_invocation = cycle.bounded_invocation(ancestry_invocation(
            executable,
            repository_dir,
            previous,
            &head_oid,
            cycle.head_query_timeout(),
        ))?;
        let ancestry = runner.run(&ancestry_invocation)?;
        if verify_lineage_preserved(package_base, Some(previous), &head_oid, Some(&ancestry))
            .is_err()
        {
            return Ok(PackageObservation {
                package_base: package_base.clone(),
                head_oid,
                commits: Vec::new(),
                truncated: false,
                paused_for_rebaseline: true,
            });
        }
    }

    let already_current = previous_cursor == Some(&head_oid);
    if already_current || cycle.is_exhausted() {
        return Ok(PackageObservation {
            package_base: package_base.clone(),
            head_oid,
            commits: Vec::new(),
            truncated: !already_current,
            paused_for_rebaseline: false,
        });
    }

    let list_invocation = cycle.bounded_invocation(unseen_commits_invocation(
        executable,
        repository_dir,
        previous_cursor,
        &head_oid,
        cycle.head_query_timeout(),
    ))?;
    let list_output = runner.run(&list_invocation)?;
    let unseen = parse_unseen_commits(&list_output, cycle.remaining_budget())?;
    cycle.consume(unseen.commits.len());

    let mut commits = Vec::with_capacity(unseen.commits.len());
    for oid in unseen.commits {
        let show_invocation = cycle.bounded_invocation(changed_paths_invocation(
            executable,
            repository_dir,
            &oid,
            cycle.head_query_timeout(),
        ))?;
        let show_output = runner.run(&show_invocation)?;
        let relevance = classify_observed_commit(&show_output)?;
        commits.push(ObservedCommit { oid, relevance });
    }

    Ok(PackageObservation {
        package_base: package_base.clone(),
        head_oid,
        commits,
        truncated: unseen.truncated,
        paused_for_rebaseline: false,
    })
}

/// What: Frozen installed or update-candidate identity captured inside one cycle.
///
/// Inputs:
/// - Package identity, versions, observed head, and cycle id supplied by the runtime owner.
///
/// Output:
/// - An identity that later stages compare against without re-deriving it.
///
/// Details:
/// - Version equality is deliberately absent from provenance decisions. `provenance_proven`
///   is always false because an installed version matching a commit's version does not
///   prove that commit produced the installed bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenTargetIdentity {
    /// Canonical package base.
    pub package_base: PackageBase,
    /// Installed package names covered by this base.
    pub installed_names: Vec<String>,
    /// Installed version string, recorded verbatim.
    pub installed_version: String,
    /// Update candidate version when this is an update target.
    pub candidate_version: Option<String>,
    /// Head commit observed when the identity was frozen.
    pub observed_head_oid: CommitOid,
    /// Observation cycle that froze this identity.
    pub cycle_id: String,
}

impl FrozenTargetIdentity {
    /// What: Report whether build provenance is proven for this identity.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - Always `false`.
    ///
    /// Details:
    /// - Provenance can never be inferred from version equality, so this is a constant.
    ///   Keeping it as an explicit method makes the invariant testable and prevents a
    ///   later caller from reintroducing version-equality inference.
    #[must_use]
    pub const fn provenance_proven(&self) -> bool {
        false
    }

    /// What: Report whether the frozen head still matches a freshly observed head.
    ///
    /// Inputs:
    /// - `observed_now`: Head commit observed at continuation time.
    ///
    /// Output:
    /// - `true` when the identity is stale and must be re-acknowledged.
    ///
    /// Details:
    /// - The frozen commit is still scanned; staleness only marks the result and queues the
    ///   newly observed commit separately.
    #[must_use]
    pub fn is_stale_against(&self, observed_now: &CommitOid) -> bool {
        &self.observed_head_oid != observed_now
    }
}

/// What: Deduplicate observed package bases while preserving first-seen order.
///
/// Inputs:
/// - `bases`: Package bases collected from installed names and update candidates.
///
/// Output:
/// - Each base once, in first-seen order.
///
/// Details:
/// - Split packages share a base, so this keeps one observation per repository while the
///   caller retains every affected installed name separately.
#[must_use]
pub fn deduplicate_observation_targets(bases: &[PackageBase]) -> Vec<PackageBase> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut ordered = Vec::new();
    for base in bases {
        if seen.insert(base.as_str()) {
            ordered.push(base.clone());
        }
    }
    ordered
}

/// Enforce the output byte cap and success status, then decode stdout as lossy UTF-8.
fn bounded_stdout(output: &GitOutput, operation: &str) -> Result<String, ObserverError> {
    if output.stdout.len() > MAX_GIT_OUTPUT_BYTES {
        return Err(ObserverError::OutputTooLarge {
            operation: operation.to_string(),
            observed: output.stdout.len(),
            limit: MAX_GIT_OUTPUT_BYTES,
        });
    }
    if !output.success {
        let reason = if output.stderr.trim().is_empty() {
            "the command exited with a non-zero status".to_string()
        } else {
            output.stderr.trim().to_string()
        };
        return Err(ObserverError::GitCommand {
            operation: operation.to_string(),
            reason,
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// What: Production [`GitCommandRunner`] that executes real isolated Git invocations.
///
/// Inputs:
/// - One fully-specified [`GitInvocation`] built by this module or by the acquisition adapter.
///
/// Output:
/// - The captured [`GitOutput`] after the child has been reaped.
///
/// Details:
/// - The child is spawned with direct `argv`; no shell, helper, or string interpolation is
///   ever involved, so no quoting is required for any fragment.
/// - The environment is cleared and rebuilt from the invocation's positive allowlist plus
///   its fixed values, so credential helpers, askpass hooks, proxies, and SSH-agent sockets
///   cannot be inherited even if new variables appear in the future.
/// - Stdout is captured through a pipe and bounded by [`GitInvocation::stdout_limit`];
///   output above it fails with [`ObserverError::OutputTooLarge`] so a hostile or
///   malfunctioning remote can neither exhaust memory nor pass off a prefix as complete.
/// - The invocation deadline is enforced by polling and, on timeout, the child is killed and
///   reaped so no Git process is left behind.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemGitRunner;

impl SystemGitRunner {
    /// What: Create a production Git runner.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - A stateless runner usable for one observation cycle or acquisition run.
    ///
    /// Details:
    /// - The runner holds no state, so it can be reused across invocations safely.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl GitCommandRunner for SystemGitRunner {
    fn run(&mut self, invocation: &GitInvocation) -> Result<GitOutput, ObserverError> {
        use std::process::{Command, Stdio};

        let mut command = Command::new(&invocation.executable);
        command
            .args(&invocation.argv)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.env_clear();
        for name in &invocation.passthrough_environment {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        for (name, value) in &invocation.fixed_environment {
            command.env(name, value);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            command.process_group(0);
        }

        let child = command.spawn().map_err(|error| ObserverError::GitCommand {
            operation: "spawn".to_string(),
            reason: error.to_string(),
        })?;
        wait_bounded(child, invocation)
    }
}

/// Bytes of Git stderr kept for user-facing messages.
const MAX_GIT_STDERR_BYTES: usize = 4096;

/// How often the runner re-measures a storage-limited directory.
const STORAGE_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Directory entries visited at most when measuring a storage-limited directory.
const MAX_STORAGE_WALK_ENTRIES: usize = 200_000;

/// What: Wait for one Git child while concurrently draining both bounded output pipes.
///
/// Inputs:
/// - `child`: Spawned Git process with piped stdout and stderr.
/// - `invocation`: Source of the deadline, stdout limit, and optional storage limit.
///
/// Output:
/// - Captured output, or `OutputTooLarge` when stdout or stored bytes exceeded their limit.
///
/// Details:
/// - Readers keep draining past the limit so the child never blocks on a full pipe, but
///   only `stdout_limit + 1` bytes are retained. A prefix is never returned as success.
/// - A storage-limited child is killed as soon as a poll sees the directory over its bound.
fn wait_bounded(
    mut child: std::process::Child,
    invocation: &GitInvocation,
) -> Result<GitOutput, ObserverError> {
    use std::time::Instant;

    let timeout = invocation.timeout;
    let stdout_limit = invocation.stdout_limit;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ObserverError::GitCommand {
            operation: "wait".to_string(),
            reason: "Git stdout pipe was unavailable".to_string(),
        })?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ObserverError::GitCommand {
            operation: "wait".to_string(),
            reason: "Git stderr pipe was unavailable".to_string(),
        })?;
    let stdout_reader = spawn_bounded_reader(stdout, stdout_limit.saturating_add(1));
    let stderr_reader = spawn_bounded_reader(stderr, MAX_GIT_STDERR_BYTES.saturating_add(4));
    let started = Instant::now();
    let mut last_storage_poll = started;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                terminate_git_child(&mut child);
                return Err(ObserverError::GitCommand {
                    operation: "wait".to_string(),
                    reason: error.to_string(),
                });
            }
        }
        if started.elapsed() >= timeout {
            terminate_git_child(&mut child);
            return Err(ObserverError::GitCommand {
                operation: "wait".to_string(),
                reason: format!("the command exceeded its {timeout:?} deadline"),
            });
        }
        if let Some(limit) = invocation.storage_limit.as_ref()
            && last_storage_poll.elapsed() >= STORAGE_POLL_INTERVAL
        {
            last_storage_poll = Instant::now();
            let stored = directory_bytes(&limit.dir);
            if stored > limit.max_bytes {
                terminate_git_child(&mut child);
                return Err(ObserverError::OutputTooLarge {
                    operation: "git fetch storage".to_string(),
                    observed: usize::try_from(stored).unwrap_or(usize::MAX),
                    limit: usize::try_from(limit.max_bytes).unwrap_or(usize::MAX),
                });
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let (stdout, stdout_total) = join_bounded_reader(stdout_reader, "stdout")?;
    if stdout_total > stdout_limit {
        return Err(ObserverError::OutputTooLarge {
            operation: "git output".to_string(),
            observed: stdout_total,
            limit: stdout_limit,
        });
    }
    let (stderr_bytes, _) = join_bounded_reader(stderr_reader, "stderr")?;
    let mut stderr = String::from_utf8_lossy(&stderr_bytes).into_owned();
    stderr.truncate(stderr.floor_char_boundary(MAX_GIT_STDERR_BYTES));
    Ok(GitOutput {
        success: status.success(),
        stdout,
        stderr,
    })
}

/// What: Sum regular-file bytes under one directory without following symlinks.
///
/// Inputs:
/// - `dir`: Directory to measure; missing or unreadable entries count as zero.
///
/// Output:
/// - Total regular-file bytes seen, saturating; `u64::MAX` when the walk exceeds
///   [`MAX_STORAGE_WALK_ENTRIES`], so a huge tree always counts as over budget.
///
/// Details:
/// - Uses `symlink_metadata`, so links are never followed out of the directory.
#[must_use]
pub fn directory_bytes(dir: &std::path::Path) -> u64 {
    let mut pending = vec![dir.to_path_buf()];
    let mut total = 0u64;
    let mut visited = 0usize;
    while let Some(current) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited > MAX_STORAGE_WALK_ENTRIES {
                return u64::MAX;
            }
            let Ok(metadata) = std::fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if metadata.is_dir() {
                pending.push(entry.path());
            } else if metadata.is_file() {
                total = total.saturating_add(metadata.len());
            }
        }
    }
    total
}

/// Pipe reader result: the retained prefix and the total byte count drained.
type BoundedRead = Result<(Vec<u8>, usize), String>;

/// Spawn one pipe-draining reader that retains only a bounded prefix and counts every byte.
fn spawn_bounded_reader<R>(mut reader: R, retain: usize) -> std::thread::JoinHandle<BoundedRead>
where
    R: std::io::Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut retained = Vec::with_capacity(retain.min(64 * 1024));
        let mut total = 0usize;
        let mut chunk = [0u8; 16 * 1024];
        loop {
            let read = reader.read(&mut chunk).map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            total = total.saturating_add(read);
            let remaining = retain.saturating_sub(retained.len());
            retained.extend_from_slice(&chunk[..read.min(remaining)]);
        }
        Ok((retained, total))
    })
}

/// Join one bounded reader and map panic/I/O failures to an inert Git error.
fn join_bounded_reader(
    reader: std::thread::JoinHandle<BoundedRead>,
    stream: &str,
) -> Result<(Vec<u8>, usize), ObserverError> {
    reader
        .join()
        .map_err(|_| ObserverError::GitCommand {
            operation: "wait".to_string(),
            reason: format!("Git {stream} reader panicked"),
        })?
        .map_err(|reason| ObserverError::GitCommand {
            operation: "wait".to_string(),
            reason: format!("Git {stream} read failed: {reason}"),
        })
}

/// Terminate the isolated Git process group and reap the direct child.
fn terminate_git_child(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        use nix::sys::signal::{Signal, killpg};
        use nix::unistd::Pid;

        if let Ok(pid) = i32::try_from(child.id()) {
            let _ = killpg(Pid::from_raw(pid), Signal::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    drop(child.kill());
    drop(child.wait());
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_HEAD_QUERY_TIMEOUT, GitCommandRunner, GitInvocation, GitOutput,
        MAX_COMMIT_EXPANSION_PER_PACKAGE, ObservationCycle, SystemGitRunner, git_argv,
        head_query_invocation, parse_head_oid, parse_unseen_commits, unseen_commits_invocation,
    };
    use crate::logic::pi_scan::identity::CommitOid;
    use crate::logic::pi_scan::identity::{AurRepoUrl, PackageBase};
    use std::ffi::OsStr;

    /// Build an isolated `/bin/sh -c <script>` invocation for runner-level tests.
    #[cfg(unix)]
    fn shell_invocation(script: &str, stdout_limit: usize) -> GitInvocation {
        GitInvocation {
            executable: "/bin/sh".into(),
            argv: vec!["-c".into(), script.into()],
            passthrough_environment: vec!["PATH".to_string()],
            fixed_environment: Vec::new(),
            timeout: std::time::Duration::from_secs(20),
            stdout_limit,
            storage_limit: None,
        }
    }

    /// What: Verify long stderr ending in a multibyte character is cut without panicking.
    ///
    /// Inputs:
    /// - A child writing 4095 ASCII bytes plus `é` (two bytes) to stderr.
    ///
    /// Output:
    /// - A successful output whose stderr is at most 4096 bytes and valid UTF-8.
    ///
    /// Details:
    /// - Regression: `String::truncate(4096)` panicked inside the two-byte character.
    #[test]
    #[cfg(unix)]
    fn stderr_cut_respects_utf8_boundaries() {
        let script = "head -c 4095 /dev/zero | tr '\\000' a >&2; printf '\\303\\251' >&2";
        let output = SystemGitRunner::new()
            .run(&shell_invocation(script, 1024))
            .expect("runner returns output");
        assert!(output.success);
        assert!(output.stderr.len() <= 4096);
        assert!(output.stderr.starts_with("aaaa"));
    }

    /// What: Verify a child writing past its storage limit is killed and reported too large.
    ///
    /// Inputs:
    /// - A child that writes 2 MiB into a temp directory, then sleeps for 30 seconds.
    ///
    /// Output:
    /// - `OutputTooLarge` long before the sleep or the 20 s deadline ends.
    ///
    /// Details:
    /// - Mirrors an upstream fetch writing objects to disk rather than stdout.
    #[test]
    #[cfg(unix)]
    fn storage_limit_kills_a_child_that_writes_too_much() {
        let dir = tempfile::tempdir().expect("temp dir");
        let target = dir.path().join("objects.pack");
        let script = format!(
            "head -c 2097152 /dev/zero > '{}'; exec sleep 30",
            target.display()
        );
        let mut invocation = shell_invocation(&script, 1024);
        invocation.storage_limit = Some(super::StorageLimit {
            dir: dir.path().to_path_buf(),
            max_bytes: 1024 * 1024,
        });
        let started = std::time::Instant::now();
        let outcome = SystemGitRunner::new().run(&invocation);
        assert!(
            matches!(
                outcome,
                Err(super::ObserverError::OutputTooLarge {
                    limit: 1_048_576,
                    ..
                })
            ),
            "{:?}",
            outcome.map(|output| output.success)
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
    }

    /// What: Verify stdout above the invocation's cap is an explicit error, never a prefix.
    ///
    /// Inputs:
    /// - Children writing exactly the cap and one byte more.
    ///
    /// Output:
    /// - The exact-cap run succeeds with every byte; the larger run is `OutputTooLarge`.
    ///
    /// Details:
    /// - Regression: the runner silently returned the first 4 MiB + 1 bytes as success.
    #[test]
    #[cfg(unix)]
    fn stdout_over_invocation_cap_is_too_large() {
        let mut runner = SystemGitRunner::new();
        let exact = runner
            .run(&shell_invocation("head -c 2048 /dev/zero", 2048))
            .expect("exact cap succeeds");
        assert_eq!(exact.stdout.len(), 2048);
        let over = runner.run(&shell_invocation("head -c 5000 /dev/zero", 2048));
        assert!(
            matches!(
                over,
                Err(super::ObserverError::OutputTooLarge {
                    observed: 5000,
                    limit: 2048,
                    ..
                })
            ),
            "{:?}",
            over.as_ref().map(|output| output.stdout.len())
        );
    }

    /// Build a successful output from stdout text.
    fn ok(stdout: &str) -> GitOutput {
        GitOutput {
            success: true,
            stdout: stdout.as_bytes().to_vec(),
            stderr: String::new(),
        }
    }

    #[test]
    fn isolation_config_always_precedes_the_subcommand() {
        let argv = git_argv(&[OsStr::new("ls-remote")]);
        assert_eq!(argv[0], OsStr::new("-c"));
        assert_eq!(argv.last().expect("subcommand"), OsStr::new("ls-remote"));
        assert!(
            argv.iter()
                .any(|arg| arg == OsStr::new("core.hooksPath=/dev/null")),
            "hooks must be disabled"
        );
    }

    #[test]
    fn head_query_uses_direct_argv_without_shell_forms() {
        let base = PackageBase::new("yay").expect("valid base");
        let url = AurRepoUrl::for_package_base(&base);
        let invocation =
            head_query_invocation(OsStr::new("/usr/bin/git"), &url, DEFAULT_HEAD_QUERY_TIMEOUT);
        let argv = invocation.argv_strings();
        assert!(argv.contains(&"ls-remote".to_string()));
        assert!(argv.contains(&"https://aur.archlinux.org/yay.git".to_string()));
        assert!(
            !argv
                .iter()
                .any(|arg| arg == "-c" && arg.len() == 2 && argv.contains(&"sh".to_string())),
            "no shell form is ever constructed"
        );
    }

    #[test]
    fn head_parsing_requires_a_full_oid() {
        let oid = "a".repeat(40);
        let parsed = parse_head_oid(&ok(&format!("{oid}\tHEAD\n"))).expect("valid head");
        assert_eq!(parsed.as_str(), oid);
        assert!(parse_head_oid(&ok("abc123\tHEAD\n")).is_err());
    }

    #[test]
    fn expansion_respects_the_smaller_of_package_and_cycle_caps() {
        let mut lines = String::new();
        for index in 0..MAX_COMMIT_EXPANSION_PER_PACKAGE + 10 {
            use std::fmt::Write as _;
            let _ = writeln!(lines, "{index:040x}");
        }
        let unseen = parse_unseen_commits(&ok(&lines), 5).expect("bounded");
        assert_eq!(unseen.commits.len(), 5);
        assert!(unseen.truncated);
    }

    /// Run one isolated real Git command in `dir`, returning stdout on success.
    fn real_git(dir: &std::path::Path, args: &[&str], stdin: Option<&[u8]>) -> Option<Vec<u8>> {
        use std::io::Write as _;
        let mut command = std::process::Command::new("git");
        command
            .current_dir(dir)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("HOME", dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        let mut child = command.spawn().ok()?;
        if let Some(bytes) = stdin {
            child.stdin.take()?.write_all(bytes).ok()?;
        }
        let output = child.wait_with_output().ok()?;
        output.status.success().then_some(output.stdout)
    }

    /// What: Verify a capped history walk returns the oldest unseen commits, not the newest.
    ///
    /// Inputs:
    /// - A real 600-commit linear repository built with `git fast-import`; the range starts
    ///   after commit 1, so 599 commits are unseen.
    ///
    /// Output:
    /// - 500 commits starting at commit 2, ending before HEAD, with `truncated` set.
    ///
    /// Details:
    /// - Regression: `--max-count` applied before `--reverse`, so Git returned the newest 500
    ///   and the cursor jumped to HEAD, skipping the oldest 99 forever.
    /// - Skips when `git` is not installed.
    #[test]
    fn unseen_history_cap_keeps_the_oldest_commits() {
        let temp = tempfile::tempdir().expect("temp repo");
        let dir = temp.path();
        if real_git(dir, &["init", "--quiet", "--bare"], None).is_none() {
            eprintln!("skipping: git is not available");
            return;
        }
        let mut stream = String::new();
        for index in 1..=600 {
            use std::fmt::Write as _;
            let _ = write!(
                stream,
                "commit refs/heads/main\nmark :{index}\ncommitter T <t@example.invalid> {index} +0000\ndata 1\nc\n"
            );
            if index > 1 {
                let _ = writeln!(stream, "from :{}", index - 1);
            }
            let _ = write!(
                stream,
                "M 644 inline f\ndata {}\n{index}\n\n",
                index.to_string().len()
            );
        }
        real_git(dir, &["fast-import", "--quiet"], Some(stream.as_bytes()))
            .expect("fast-import builds the repository");
        let listing = real_git(dir, &["rev-list", "--reverse", "refs/heads/main"], None)
            .expect("list commits");
        let all: Vec<CommitOid> = String::from_utf8_lossy(&listing)
            .lines()
            .map(|line| CommitOid::new(line.trim()).expect("oid"))
            .collect();
        assert_eq!(all.len(), 600);
        let mut invocation = unseen_commits_invocation(
            OsStr::new("git"),
            dir.as_os_str(),
            Some(&all[0]),
            &all[599],
            std::time::Duration::from_secs(20),
        );
        invocation
            .fixed_environment
            .push(("HOME".to_string(), dir.display().to_string()));
        let output = SystemGitRunner::new()
            .run(&invocation)
            .expect("rev-list runs");
        let unseen = parse_unseen_commits(&output, usize::MAX).expect("parsed");
        assert_eq!(unseen.commits.len(), MAX_COMMIT_EXPANSION_PER_PACKAGE);
        assert_eq!(unseen.commits[0], all[1]);
        assert_eq!(
            unseen.commits[MAX_COMMIT_EXPANSION_PER_PACKAGE - 1],
            all[500]
        );
        assert!(unseen.truncated);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn system_runner_drains_large_stdout_without_pipe_deadlock() {
        let executable = if std::path::Path::new("/usr/bin/head").is_file() {
            "/usr/bin/head"
        } else {
            "/bin/head"
        };
        let invocation = GitInvocation {
            executable: executable.into(),
            argv: vec!["-c".into(), "1000000".into(), "/dev/zero".into()],
            passthrough_environment: Vec::new(),
            fixed_environment: Vec::new(),
            timeout: std::time::Duration::from_secs(2),
            stdout_limit: super::MAX_GIT_OUTPUT_BYTES,
            storage_limit: None,
        };
        let output = SystemGitRunner::new()
            .run(&invocation)
            .expect("large pipe output drains");
        assert!(output.success);
        assert_eq!(output.stdout.len(), 1_000_000);
    }

    #[test]
    fn cycle_budget_starts_full_and_clamps_the_timeout_down() {
        let cycle = ObservationCycle::new(std::time::Duration::from_mins(10));
        assert_eq!(
            cycle.remaining_budget(),
            super::MAX_COMMIT_EXPANSION_PER_CYCLE
        );
        assert_eq!(cycle.head_query_timeout(), DEFAULT_HEAD_QUERY_TIMEOUT);
    }

    #[test]
    fn exhausted_whole_cycle_deadline_rejects_before_git_dispatch() {
        let cycle = ObservationCycle::with_deadline(
            std::time::Duration::from_secs(15),
            std::time::Duration::ZERO,
        );
        let base = PackageBase::new("yay").expect("base");
        let invocation = head_query_invocation(
            OsStr::new("/usr/bin/git"),
            &AurRepoUrl::for_package_base(&base),
            std::time::Duration::from_secs(15),
        );
        let error = cycle
            .bounded_invocation(invocation)
            .expect_err("expired cycle");
        assert!(error.to_string().contains("whole observation deadline"));
    }
}
