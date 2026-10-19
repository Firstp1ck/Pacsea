//! Isolated exact-fingerprint `OpenPGP` detached-signature verification.
//!
//! Keys are retrieved only from exact full-fingerprint keys.openpgp.org URLs. Verification
//! uses private ephemeral files and direct-argv `gpg`/`gpgv` children with a cleared,
//! positive allowlist environment and bounded process-group teardown.

use crate::install::resolve_command_on_path;
use crate::logic::pi_scan::acquisition::{
    AcquisitionError, SignatureRequest, SignatureVerifier, download_static_source,
};
use crate::logic::pi_scan::network::SystemNetworkAdapter;
use crate::logic::pi_scan::source::SignatureStatus;
use std::ffi::OsString;
use std::fmt;
use std::io::{Read, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Maximum bytes accepted for one retrieved `OpenPGP` key body.
pub const MAX_SIGNING_KEY_BYTES: u64 = 10 * 1024 * 1024;

/// Default wall-clock deadline for each `GnuPG` child.
pub const DEFAULT_GPG_TIMEOUT: Duration = Duration::from_secs(30);

/// Monotonic suffix preventing same-process workspace collisions.
static WORKSPACE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// What: Build the only permitted signing-key retrieval URL.
///
/// Inputs:
/// - `fingerprint`: Full 40- or 64-hex `OpenPGP` fingerprint.
///
/// Output:
/// - Exact keys.openpgp.org HTTPS by-fingerprint URL using uppercase hex.
///
/// Details:
/// - No key id, email, search query, alternate keyserver, or userinfo is accepted.
///
/// # Errors
/// - Returns `Err` when the input is not one exact full fingerprint.
pub fn key_retrieval_url(fingerprint: &str) -> Result<String, String> {
    if !matches!(fingerprint.len(), 40 | 64)
        || !fingerprint
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err("signing key requires a full 40- or 64-hex fingerprint".to_string());
    }
    Ok(format!(
        "https://keys.openpgp.org/vks/v1/by-fingerprint/{}",
        fingerprint.to_ascii_uppercase()
    ))
}

/// What: Fetch one exact signing-key body through an injectable bounded network seam.
///
/// Inputs:
/// - Exact key URL and byte/deadline policy.
///
/// Output:
/// - Raw key body used only in private ephemeral files.
///
/// Details:
/// - Implementations must not log or persist key bodies.
pub trait SigningKeyFetcher {
    /// Retrieve one exact key body.
    ///
    /// # Errors
    /// - Returns an acquisition error for unavailable or policy-rejected key transport.
    fn fetch_key(&mut self, url: &str) -> Result<Vec<u8>, AcquisitionError>;
}

impl SigningKeyFetcher for SystemNetworkAdapter {
    fn fetch_key(&mut self, url: &str) -> Result<Vec<u8>, AcquisitionError> {
        let mut resolver = self.clone();
        download_static_source(
            self,
            &mut resolver,
            url,
            MAX_SIGNING_KEY_BYTES,
            Duration::from_secs(30),
        )
        .map(|downloaded| downloaded.bytes)
    }
}

/// What: Fully specified direct-argv `GnuPG` invocation.
///
/// Inputs:
/// - Absolute executable, fixed argv, private home, and deadline.
///
/// Output:
/// - Consumed by a [`GpgCommandRunner`].
///
/// Details:
/// - Contains no shell string and inherits no ambient environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpgInvocation {
    /// Resolved absolute `gpg` or `gpgv` executable.
    pub executable: PathBuf,
    /// Argument vector after the executable.
    pub argv: Vec<OsString>,
    /// Private mode-0700 home used as HOME and GNUPGHOME.
    pub home: PathBuf,
    /// Child wall-clock deadline.
    pub timeout: Duration,
}

impl GpgInvocation {
    /// Render argv lossily for deterministic fake-runner assertions only.
    #[must_use]
    pub fn argv_strings(&self) -> Vec<String> {
        self.argv
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect()
    }
}

/// What: Bounded process result used by exact status parsing.
///
/// Inputs:
/// - Exit status and machine-readable status-fd bytes.
///
/// Output:
/// - Imported or verified fingerprint evidence.
///
/// Details:
/// - Human-readable stderr is intentionally excluded from policy decisions and logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpgOutput {
    /// Whether the child exited successfully.
    pub success: bool,
    /// Bounded `--status-fd` output.
    pub status: Vec<u8>,
}

/// What: Injectable isolated `GnuPG` process seam.
///
/// Inputs:
/// - One complete direct-argv invocation.
///
/// Output:
/// - Bounded exit/status evidence.
///
/// Details:
/// - Production clears the environment, starts a process group, and reaps on timeout.
pub trait GpgCommandRunner: Send {
    /// Execute and reap one `GnuPG` child.
    ///
    /// # Errors
    /// - Returns `Err` when the tool cannot start, time out safely, or be reaped.
    fn run(&mut self, invocation: &GpgInvocation) -> Result<GpgOutput, String>;
}

/// Production direct-argv `GnuPG` runner.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemGpgRunner;

impl GpgCommandRunner for SystemGpgRunner {
    fn run(&mut self, invocation: &GpgInvocation) -> Result<GpgOutput, String> {
        let (status_path, status_file) = private_status_file(&invocation.home)?;
        let mut command = Command::new(&invocation.executable);
        command
            .args(&invocation.argv)
            .current_dir(&invocation.home)
            .stdin(Stdio::null())
            .stdout(Stdio::from(status_file))
            .stderr(Stdio::null())
            .env_clear()
            .env("HOME", &invocation.home)
            .env("GNUPGHOME", &invocation.home)
            .env("LANG", "C")
            .env("LC_ALL", "C");
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            command.process_group(0);
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("could not start isolated GnuPG: {error}"))?;
        let started = Instant::now();
        let exit = loop {
            if let Some(status) = child
                .try_wait()
                .map_err(|error| format!("could not wait for GnuPG: {error}"))?
            {
                break status;
            }
            if started.elapsed() >= invocation.timeout {
                terminate_group(&mut child)?;
                return Err(format!(
                    "isolated GnuPG exceeded its {:?} deadline",
                    invocation.timeout
                ));
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let status = read_status_file(&status_path, 64 * 1024)?;
        Ok(GpgOutput {
            success: exit.success(),
            status,
        })
    }
}

/// What: Production isolated verifier implementing the WS8 signature seam.
///
/// Inputs:
/// - Private workspace parent, resolved tools, bounded key fetcher, and process runner.
///
/// Output:
/// - Verified, failed, or unavailable signature policy state.
///
/// Details:
/// - One fresh private home/keyring is created per request and removed on return.
/// - Only exact imported and `VALIDSIG` fingerprints plus exit status influence the result.
pub struct IsolatedSignatureVerifier {
    /// Parent under which one-use private homes are created.
    workspace_parent: PathBuf,
    /// Resolved absolute `gpg`, absent when unavailable.
    gpg: Option<PathBuf>,
    /// Resolved absolute `gpgv`, absent when unavailable.
    gpgv: Option<PathBuf>,
    /// Bounded exact-key transport.
    key_fetcher: Box<dyn SigningKeyFetcher + Send>,
    /// Direct-argv child runner.
    runner: Box<dyn GpgCommandRunner>,
    /// Per-child wall-clock deadline.
    timeout: Duration,
}

impl fmt::Debug for IsolatedSignatureVerifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IsolatedSignatureVerifier")
            .field("workspace_parent", &self.workspace_parent)
            .field("gpg", &self.gpg)
            .field("gpgv", &self.gpgv)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl IsolatedSignatureVerifier {
    /// What: Discover production tools and construct an isolated verifier.
    ///
    /// Inputs:
    /// - `workspace_parent`: Existing private-capable temporary parent.
    ///
    /// Output:
    /// - A verifier that reports unavailable when either tool cannot be resolved.
    ///
    /// Details:
    /// - Tool paths are resolved once; every invocation uses the resulting direct path.
    #[must_use]
    pub fn production(workspace_parent: PathBuf) -> Self {
        Self::production_with_network(workspace_parent, SystemNetworkAdapter::new())
    }

    /// Construct a production verifier using the scanner's explicit network policy.
    #[must_use]
    pub fn production_with_network(
        workspace_parent: PathBuf,
        network: SystemNetworkAdapter,
    ) -> Self {
        Self {
            workspace_parent,
            gpg: resolve_command_on_path("gpg"),
            gpgv: resolve_command_on_path("gpgv"),
            key_fetcher: Box::new(network),
            runner: Box::new(SystemGpgRunner),
            timeout: DEFAULT_GPG_TIMEOUT,
        }
    }

    /// What: Construct a verifier from deterministic test or production seams.
    ///
    /// Inputs:
    /// - Explicit workspace, tool paths, fetcher, runner, and deadline.
    ///
    /// Output:
    /// - Fully configured verifier.
    ///
    /// Details:
    /// - Explicit paths allow local fake executables without consulting ambient PATH.
    #[must_use]
    pub fn with_seams(
        workspace_parent: PathBuf,
        gpg: Option<PathBuf>,
        gpgv: Option<PathBuf>,
        key_fetcher: Box<dyn SigningKeyFetcher + Send>,
        runner: Box<dyn GpgCommandRunner>,
        timeout: Duration,
    ) -> Self {
        Self {
            workspace_parent,
            gpg,
            gpgv,
            key_fetcher,
            runner,
            timeout: timeout.min(DEFAULT_GPG_TIMEOUT),
        }
    }

    /// Verify one request after creating all private one-use artifacts.
    fn verify_inner(&mut self, request: &SignatureRequest<'_>) -> SignatureStatus {
        let (Some(gpg), Some(gpgv)) = (self.gpg.clone(), self.gpgv.clone()) else {
            return SignatureStatus::Unavailable;
        };
        if !gpg.is_absolute() || !gpgv.is_absolute() || request.fingerprints.is_empty() {
            return SignatureStatus::Unavailable;
        }
        let Ok(workspace) = PrivateWorkspace::create(&self.workspace_parent) else {
            return SignatureStatus::Unavailable;
        };
        let data_path = workspace.root.join("covered-source");
        let signature_path = workspace.root.join("detached-signature");
        let keyring_path = workspace.root.join("trustedkeys.gpg");
        if write_private_file(&data_path, request.data).is_err()
            || write_private_file(&signature_path, request.signature).is_err()
            || write_private_file(&keyring_path, &[]).is_err()
        {
            return SignatureStatus::Unavailable;
        }
        let mut imported_any = false;
        for (index, fingerprint) in request.fingerprints.iter().enumerate() {
            match self.import_key(&workspace.root, &keyring_path, &gpg, fingerprint, index) {
                ImportResult::Imported => imported_any = true,
                ImportResult::WrongFingerprint => return SignatureStatus::Failed,
                ImportResult::Unavailable => {}
            }
        }
        if !imported_any {
            return SignatureStatus::Unavailable;
        }
        self.verify_signature(
            &workspace.root,
            &keyring_path,
            &gpgv,
            &signature_path,
            &data_path,
            request.fingerprints,
        )
    }

    /// Retrieve and import one exact fingerprint into the private keyring.
    fn import_key(
        &mut self,
        home: &Path,
        keyring: &Path,
        gpg: &Path,
        fingerprint: &str,
        index: usize,
    ) -> ImportResult {
        let Ok(url) = key_retrieval_url(fingerprint) else {
            return ImportResult::WrongFingerprint;
        };
        let Ok(body) = self.key_fetcher.fetch_key(&url) else {
            return ImportResult::Unavailable;
        };
        let key_path = home.join(format!("retrieved-key-{index}"));
        if write_private_file(&key_path, &body).is_err() {
            return ImportResult::Unavailable;
        }
        let invocation = import_invocation(gpg, home, keyring, &key_path, self.timeout);
        let Ok(output) = self.runner.run(&invocation) else {
            return ImportResult::Unavailable;
        };
        if !output.success {
            return ImportResult::Unavailable;
        }
        let imported = parse_status_fingerprints(&output.status, "IMPORT_OK");
        if imported.iter().any(|value| value == fingerprint) {
            ImportResult::Imported
        } else {
            ImportResult::WrongFingerprint
        }
    }

    /// Run gpgv and bind its exact primary/signing fingerprint to validpgpkeys.
    fn verify_signature(
        &mut self,
        home: &Path,
        keyring: &Path,
        gpgv: &Path,
        signature: &Path,
        data: &Path,
        fingerprints: &[String],
    ) -> SignatureStatus {
        let invocation = verify_invocation(gpgv, home, keyring, signature, data, self.timeout);
        let Ok(output) = self.runner.run(&invocation) else {
            return SignatureStatus::Unavailable;
        };
        if !output.success {
            return SignatureStatus::Failed;
        }
        let verified = parse_validsig_fingerprints(&output.status);
        if verified
            .iter()
            .any(|value| fingerprints.iter().any(|allowed| allowed == value))
        {
            SignatureStatus::Verified
        } else {
            SignatureStatus::Failed
        }
    }
}

impl SignatureVerifier for IsolatedSignatureVerifier {
    fn verify(&mut self, request: &SignatureRequest<'_>) -> SignatureStatus {
        self.verify_inner(request)
    }
}

/// Import outcome preserving failed-versus-unavailable policy.
enum ImportResult {
    /// Exact declared fingerprint was imported.
    Imported,
    /// Imported status did not bind the requested full fingerprint.
    WrongFingerprint,
    /// Key transport, tool, or workspace was unavailable.
    Unavailable,
}

/// Private one-use `GnuPG` workspace removed on drop.
struct PrivateWorkspace {
    /// Mode-0700 home path.
    root: PathBuf,
}

impl PrivateWorkspace {
    /// Create a collision-resistant mode-0700 directory without reusing existing state.
    fn create(parent: &Path) -> Result<Self, std::io::Error> {
        std::fs::create_dir_all(parent)?;
        let sequence = WORKSPACE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = parent.join(format!("gpg-{}-{sequence}", std::process::id()));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            builder.mode(0o700);
        }
        builder.create(&root)?;
        Ok(Self { root })
    }
}

impl Drop for PrivateWorkspace {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.root));
    }
}

/// Write a private mode-0600 file atomically with `create_new` semantics.
fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Build the exact isolated gpg import invocation.
fn import_invocation(
    gpg: &Path,
    home: &Path,
    keyring: &Path,
    key_file: &Path,
    timeout: Duration,
) -> GpgInvocation {
    GpgInvocation {
        executable: gpg.to_path_buf(),
        argv: vec![
            "--homedir".into(),
            home.as_os_str().into(),
            "--no-options".into(),
            "--batch".into(),
            "--no-tty".into(),
            "--no-autostart".into(),
            "--no-auto-key-retrieve".into(),
            "--no-default-keyring".into(),
            "--keyring".into(),
            keyring.as_os_str().into(),
            "--trustdb-name".into(),
            home.join("trustdb.gpg").into_os_string(),
            "--status-fd".into(),
            "1".into(),
            "--import".into(),
            key_file.as_os_str().into(),
        ],
        home: home.to_path_buf(),
        timeout,
    }
}

/// Build the exact isolated gpgv detached-signature invocation.
fn verify_invocation(
    gpgv: &Path,
    home: &Path,
    keyring: &Path,
    signature: &Path,
    data: &Path,
    timeout: Duration,
) -> GpgInvocation {
    GpgInvocation {
        executable: gpgv.to_path_buf(),
        argv: vec![
            "--homedir".into(),
            home.as_os_str().into(),
            "--keyring".into(),
            keyring.as_os_str().into(),
            "--status-fd".into(),
            "1".into(),
            signature.as_os_str().into(),
            data.as_os_str().into(),
        ],
        home: home.to_path_buf(),
        timeout,
    }
}

/// Parse exact full fingerprints from one machine-readable `GnuPG` status tag.
fn parse_status_fingerprints(status: &[u8], tag: &str) -> Vec<String> {
    let text = String::from_utf8_lossy(status);
    text.lines()
        .filter_map(|line| line.strip_prefix(&format!("[GNUPG:] {tag} ")))
        .filter_map(|tail| tail.split_whitespace().last())
        .filter(|value| valid_fingerprint(value))
        .map(str::to_string)
        .collect()
}

/// Parse signing and primary full fingerprints from exact VALIDSIG records.
fn parse_validsig_fingerprints(status: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(status);
    let mut fingerprints = Vec::new();
    for tail in text
        .lines()
        .filter_map(|line| line.strip_prefix("[GNUPG:] VALIDSIG "))
    {
        let fields: Vec<&str> = tail.split_whitespace().collect();
        if let Some(signing) = fields
            .first()
            .copied()
            .filter(|value| valid_fingerprint(value))
        {
            fingerprints.push(signing.to_string());
        }
        if let Some(primary) = fields
            .last()
            .copied()
            .filter(|value| valid_fingerprint(value))
        {
            fingerprints.push(primary.to_string());
        }
    }
    fingerprints
}

/// Return whether text is one uppercase full fingerprint accepted from status-fd.
fn valid_fingerprint(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .chars()
            .all(|character| character.is_ascii_hexdigit() && !character.is_ascii_lowercase())
}

/// Create one private status file used instead of a potentially blocking child pipe.
fn private_status_file(home: &Path) -> Result<(PathBuf, std::fs::File), String> {
    for index in 0..16_u8 {
        let path = home.join(format!("status-{index}"));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(format!(
                    "could not create private GnuPG status file: {error}"
                ));
            }
        }
    }
    Err("could not allocate a private GnuPG status file".to_string())
}

/// Read one completed status file with a strict byte ceiling.
fn read_status_file(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("could not inspect GnuPG status output: {error}"))?;
    if !metadata.file_type().is_file() || metadata.len() > limit {
        return Err("GnuPG status output is not a bounded regular file".to_string());
    }
    let file = std::fs::File::open(path)
        .map_err(|error| format!("could not open GnuPG status output: {error}"))?;
    let mut status = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut status)
        .map_err(|error| format!("could not read GnuPG status output: {error}"))?;
    if status.len() as u64 > limit {
        return Err("GnuPG status output exceeded its byte limit".to_string());
    }
    Ok(status)
}

/// Terminate and reap one timed-out `GnuPG` process group.
fn terminate_group(child: &mut std::process::Child) -> Result<(), String> {
    #[cfg(unix)]
    {
        use nix::errno::Errno;
        use nix::sys::signal::{Signal, killpg};
        use nix::unistd::Pid;
        let pid = i32::try_from(child.id()).map_err(|error| error.to_string())?;
        if let Err(error) = killpg(Pid::from_raw(pid), Signal::SIGKILL)
            && error != Errno::ESRCH
        {
            return Err(format!("could not kill timed-out GnuPG group: {error}"));
        }
    }
    #[cfg(not(unix))]
    child
        .kill()
        .map_err(|error| format!("could not kill timed-out GnuPG: {error}"))?;
    child
        .wait()
        .map_err(|error| format!("could not reap timed-out GnuPG: {error}"))?;
    Ok(())
}
