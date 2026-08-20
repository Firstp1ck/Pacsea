//! Production adapter and runtime bridge for the optional Pi-backed AUR scanner.
//!
//! Construction is inert. Network, Git, `GnuPG`, and Pi are reached only after the explicit
//! scanner/setup gates permit an observation or execution operation.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[cfg(test)]
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::app::runtime::workers::pi_scan::{
    PiScanCancelMessage, PiScanNoticeProvenance, PiScanNoticeSource, PiScanPolicyAcknowledgement,
    PiScanProgressMessage, PiScanRequestMessage, PiScanResultMessage, PiScanRuntimeAction,
    PiScanRuntimeChannels, PiScanRuntimeNotice, PiScanRuntimeOptions, PiScanSessionRegistration,
    PiScanShutdownAck, PiScanShutdownMessage,
};
use crate::install::resolve_command_on_path;
use crate::logic::pi_scan::acquisition::{
    AcquisitionError, AcquisitionLimits, AcquisitionOutcome, AcquisitionRequest, AurRpcData,
    acquire_package, acquire_package_with_https_proxy, fetch_commit_invocation,
    init_repository_invocation, mutable_source_identity_changed, resolve_package_base,
};
use crate::logic::pi_scan::identity::{AurRepoUrl, CommitOid, PackageName};
use crate::logic::pi_scan::network::{
    SystemNetworkAdapter, fetch_aur_rpc_package_base, fetch_aur_rpc_package_base_with_timeout,
};
use crate::logic::pi_scan::observer::{
    ExplicitHttpsProxyGitRunner, GitCommandRunner, GitInvocation, ObservationCycle,
    SystemGitRunner, head_query_invocation, observe_package_base, parse_head_oid,
};
#[cfg(test)]
use crate::logic::pi_scan::pricing::{pricing_from_pi_model_cost, reserve_worst_case_microusd};
use crate::logic::pi_scan::result::Coverage;
#[cfg(test)]
use crate::logic::pi_scan::result::UsageAccounting;
use crate::logic::pi_scan::result_store::{
    StoredResultBatch, StoredResultSummary, cleanup_expired_results, load_all_results, load_result,
    plan_retention,
};
use crate::logic::pi_scan::signature::IsolatedSignatureVerifier;
use crate::logic::pi_scan::source::AcquisitionStatus;
use crate::pi_agent::scan_engine::{
    ExecutionError, ProductionScanRequest, ScanExecutionInput, execute_scan,
};
use crate::pi_agent::session::ModelChoice;
use crate::pi_agent::setup_probe::{
    PiSetupProbeRequest, SETUP_PROBE_MAXIMUM_PRICING_AGE, SETUP_PROBE_RESERVATION_TOKENS,
    probe_pi_setup,
};
use crate::pi_scan_orchestrator::{
    DiscoveredPackage, DryRunAcquisitionReceipt, ExecutionFailure, ExecutionReceipt,
    FrozenScanIdentity, ObservationCommit, ObservationPackage, OrchestrationAdapter,
    OrchestrationConfig, OrchestrationError, PiScanOrchestrator, PiScanSequentialRunner,
    PiScanSetupConsentState, SetupSnapshot, UpdateCandidate,
};
use crate::state::pi_scan::{
    PiScanActualUsage, PiScanBudgetLimits, PiScanReservation, PiScanTerminalRecord,
    PiScanTerminalStatus,
};

/// Monotonic suffix preventing same-process observation workspace collisions.
static OBSERVATION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// What: Immutable production seams and user-confirmed route policy.
///
/// Inputs:
/// - Resolved executable paths, private workspace, exact model order, deadlines, and reservation.
///
/// Output:
/// - Construction data for [`ProductionOrchestrationAdapter`].
///
/// Details:
/// - All paths must be absolute. The primary route must be first in `models`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionAdapterConfig {
    /// Resolved absolute Pi executable.
    pub pi_executable: PathBuf,
    /// Resolved absolute Git executable.
    pub git_executable: PathBuf,
    /// Private parent for ephemeral observation/acquisition/Pi workspaces.
    pub workspace_parent: PathBuf,
    /// Exact confirmed primary and fallback model order.
    pub models: Vec<ModelChoice>,
    /// Explicit Pi thinking level applied to every model attempt.
    pub thinking: String,
    /// Per-model-attempt deadline.
    pub model_attempt_timeout: Duration,
    /// Whole logical-scan deadline.
    pub logical_timeout: Duration,
    /// Per-observation Git query deadline.
    pub head_query_timeout: Duration,
    /// Whole observation-cycle deadline.
    pub observation_deadline: Duration,
    /// Conservative reservation supplied by the setup disclosure.
    pub reservation: PiScanReservation,
    /// Explicit credential-free HTTPS proxy, or direct-only when absent.
    pub https_proxy: Option<String>,
}

/// What: Validated runtime settings needed to construct the production service lazily.
///
/// Inputs:
/// - Parsed scanner settings after bounds validation.
///
/// Output:
/// - Optional production payload carried by the default-off runtime options.
///
/// Details:
/// - Executables are resolved only when the enabled service is spawned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionRuntimeSettings {
    /// Configured Pi executable name or absolute path.
    pub binary: String,
    /// Exact confirmed primary and fallback route order.
    pub models: Vec<ModelChoice>,
    /// Explicit unattended paid-execution gate.
    pub background_execution: bool,
    /// Explicit Pi thinking level applied to every model attempt.
    pub thinking: String,
    /// Observation interval in seconds.
    pub observation_interval_seconds: u64,
    /// Per-model-attempt deadline.
    pub model_attempt_timeout: Duration,
    /// Whole logical-scan deadline.
    pub logical_timeout: Duration,
    /// Per-head-query deadline.
    pub head_query_timeout: Duration,
    /// Whole observation-cycle deadline.
    pub observation_deadline: Duration,
    /// Retention window for superseded validated results.
    pub result_retention_days: u32,
    /// Conservative worst-case reservation.
    pub reservation: PiScanReservation,
    /// Authoritative rolling unattended limits.
    pub budget_limits: PiScanBudgetLimits,
    /// Explicit credential-free HTTPS proxy text, empty for direct-only.
    pub https_proxy: String,
}

/// What: Resolve production executables without invoking them.
///
/// Inputs:
/// - `binary`: Configured Pi executable name or absolute path.
/// - `workspace_parent`: Private config-relative scanner root.
/// - `models`: Exact primary/fallback route order.
/// - Deadlines and reservation from validated settings.
///
/// Output:
/// - Validated production adapter configuration.
///
/// Details:
/// - Missing Pi is an optional-feature error; Pacsea itself remains usable.
///
/// # Errors
/// - Returns actionable missing-tool or invalid-path guidance.
#[allow(clippy::too_many_arguments)]
pub fn resolve_production_adapter_config(
    binary: &str,
    workspace_parent: PathBuf,
    models: Vec<ModelChoice>,
    thinking: &str,
    model_attempt_timeout: Duration,
    logical_timeout: Duration,
    head_query_timeout: Duration,
    observation_deadline: Duration,
    reservation: PiScanReservation,
    https_proxy: &str,
    dry_run: bool,
) -> Result<ProductionAdapterConfig, String> {
    let pi_executable = if dry_run {
        workspace_parent.join("pi-not-launched-in-dry-run")
    } else {
        resolve_configured_executable(binary).ok_or_else(|| {
            format!("Pi executable {binary:?} was not found; install Pi or disable pi_scan_enabled")
        })?
    };
    let git_executable = resolve_command_on_path("git").ok_or_else(|| {
        "Git was not found; install git before enabling the optional Pi scanner".to_string()
    })?;
    if models.is_empty() {
        return Err("select an exact Pi provider/model before enabling the scanner".to_string());
    }
    let https_proxy = if https_proxy.trim().is_empty() {
        None
    } else {
        SystemNetworkAdapter::with_https_proxy(https_proxy)
            .map_err(|error| format!("invalid explicit Pi scan HTTPS proxy: {error}"))?;
        Some(https_proxy.trim().to_string())
    };
    Ok(ProductionAdapterConfig {
        pi_executable,
        git_executable,
        workspace_parent,
        models,
        thinking: thinking.to_string(),
        model_attempt_timeout,
        logical_timeout,
        head_query_timeout,
        observation_deadline,
        reservation,
        https_proxy,
    })
}

/// Resolve a configured absolute path or executable name through the existing PATH policy.
fn resolve_configured_executable(binary: &str) -> Option<PathBuf> {
    let candidate = PathBuf::from(binary.trim());
    if candidate.is_absolute() && candidate.is_file() {
        Some(candidate)
    } else if candidate.components().count() == 1 {
        resolve_command_on_path(binary.trim())
    } else {
        None
    }
}

/// What: Concrete official-AUR/acquisition/Pi adapter used by the central orchestrator.
///
/// Inputs:
/// - Constructed from [`ProductionAdapterConfig`].
///
/// Output:
/// - [`OrchestrationAdapter`] implementation with no shell or ambient credential use.
///
/// Details:
/// - A fresh bounded observation cycle starts whenever installed packages are enumerated.
#[derive(Debug)]
pub struct ProductionOrchestrationAdapter {
    /// Immutable production configuration.
    config: ProductionAdapterConfig,
    /// Direct-argv bounded Git runner.
    git: SystemGitRunner,
    /// AUR RPC facts cached only for the active process.
    rpc_by_base: BTreeMap<String, AurRpcData>,
    /// Typed current/candidate versions keyed by installed package name.
    update_candidates: BTreeMap<String, UpdateCandidate>,
    /// Shared cap and whole-cycle deadline across sequential package observation.
    observation_cycle: ObservationCycle,
    /// Verified Pi version cached after the no-model setup probe.
    pi_version: Option<String>,
}

impl ProductionOrchestrationAdapter {
    /// What: Construct an inert production adapter.
    ///
    /// Inputs:
    /// - `config`: Resolved executable/model/deadline policy.
    ///
    /// Output:
    /// - Adapter that performs no external operation until a trait method is invoked.
    ///
    /// Details:
    /// - Setup, observation, acquisition, and model execution remain separate calls.
    #[must_use]
    pub fn new(config: ProductionAdapterConfig) -> Self {
        let observation_cycle = ObservationCycle::new(config.head_query_timeout);
        Self {
            config,
            git: SystemGitRunner::new(),
            rpc_by_base: BTreeMap::new(),
            update_candidates: BTreeMap::new(),
            observation_cycle,
            pi_version: None,
        }
    }

    /// Build the direct-only or explicitly proxied adapter from validated network policy.
    fn network(&self) -> Result<SystemNetworkAdapter, String> {
        self.config.https_proxy.as_deref().map_or_else(
            || Ok(SystemNetworkAdapter::new()),
            |proxy| {
                SystemNetworkAdapter::with_https_proxy(proxy).map_err(|error| error.to_string())
            },
        )
    }

    /// Run one production Git invocation through direct-only or explicit proxy policy.
    fn run_git(
        &mut self,
        invocation: &GitInvocation,
    ) -> Result<
        crate::logic::pi_scan::observer::GitOutput,
        crate::logic::pi_scan::observer::ObserverError,
    > {
        if let Some(proxy) = self.config.https_proxy.as_deref() {
            ExplicitHttpsProxyGitRunner::new(&mut self.git, proxy).run(invocation)
        } else {
            self.git.run(invocation)
        }
    }

    /// Require one production Git invocation to exit successfully.
    fn run_git_success(
        &mut self,
        invocation: &GitInvocation,
        operation: &str,
    ) -> Result<(), String> {
        if let Some(proxy) = self.config.https_proxy.as_deref() {
            let mut runner = ExplicitHttpsProxyGitRunner::new(&mut self.git, proxy);
            run_git_success(&mut runner, invocation, operation)
        } else {
            run_git_success(&mut self.git, invocation, operation)
        }
    }
}

/// Exact no-model record retained from Pi's available-model response.
#[cfg(test)]
#[derive(Debug, Clone)]
struct ProbedModel {
    /// Exact provider identifier.
    provider: String,
    /// Exact model identifier.
    model: String,
    /// Exact Pi-reported cost object, when present.
    cost: Option<Value>,
}

/// Derive an exact worst-case reservation from Pi-reported route prices and user caps.
#[cfg(test)]
fn reservation_from_probed_models(
    available: &[ProbedModel],
    choices: &[ModelChoice],
    cap: PiScanReservation,
) -> Result<PiScanReservation, String> {
    if cap.tokens == 0 {
        return Err(
            "Pi scan token cap is zero; set a positive cap before enabling model execution"
                .to_string(),
        );
    }
    let usage = UsageAccounting {
        rpc_bytes: 0,
        reported_tokens: Some(cap.tokens),
    };
    let mut worst_cost = 0u64;
    for choice in choices {
        let model = available
            .iter()
            .find(|model| model.provider == choice.provider && model.model == choice.model)
            .ok_or_else(|| {
                format!(
                    "configured Pi route {}/{} is not advertised",
                    choice.provider, choice.model
                )
            })?;
        let cost = model.cost.as_ref().ok_or_else(|| {
            format!(
                "Pi route {}/{} reported no exact pricing; choose a priced route",
                choice.provider, choice.model
            )
        })?;
        let pricing = pricing_from_pi_model_cost(&choice.provider, &choice.model, cost, &[])
            .map_err(|error| error.to_string())?;
        worst_cost = worst_cost.max(reserve_worst_case_microusd(&pricing, usage));
    }
    if worst_cost > cap.cost_microusd {
        return Err(format!(
            "worst-case Pi scan reservation is {worst_cost} micro-USD, above the configured {} micro-USD cap",
            cap.cost_microusd
        ));
    }
    Ok(PiScanReservation {
        tokens: cap.tokens,
        cost_microusd: worst_cost,
    })
}

/// Create one private observation root and bare repository path.
fn create_observation_workspace(parent: &Path) -> Result<(PathBuf, PathBuf), String> {
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create observation parent: {error}"))?;
    let suffix = OBSERVATION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = parent.join(format!("observe-{}-{suffix}", std::process::id()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .map_err(|error| format!("could not create private observation workspace: {error}"))?;
    }
    #[cfg(not(unix))]
    fs::create_dir(&root)
        .map_err(|error| format!("could not create private observation workspace: {error}"))?;
    let repository = root.join("repo");
    fs::create_dir(&repository)
        .map_err(|error| format!("could not create private observation repository: {error}"))?;
    Ok((root, repository))
}

/// Require one direct-argv Git invocation to exit successfully.
fn run_git_success(
    runner: &mut dyn GitCommandRunner,
    invocation: &GitInvocation,
    operation: &str,
) -> Result<(), String> {
    let output = runner.run(invocation).map_err(|error| error.to_string())?;
    if output.success {
        Ok(())
    } else {
        Err(format!(
            "isolated Git {operation} exited unsuccessfully; retry observation"
        ))
    }
}

/// What: Classify one foreign-package AUR lookup for discovery.
///
/// Inputs:
/// - `result`: Typed exact-name AUR RPC outcome.
///
/// Output:
/// - `Some` mapping for AUR packages, `None` for non-AUR foreign packages, or a fatal error.
///
/// Details:
/// - Only an explicit unresolved-package classification is skippable; transport, schema, and
///   identity failures remain fail-closed for the complete observation cycle.
fn classify_foreign_rpc_result(
    result: Result<AurRpcData, AcquisitionError>,
) -> Result<Option<AurRpcData>, String> {
    match result {
        Ok(rpc) => Ok(Some(rpc)),
        Err(AcquisitionError::PackageBaseUnresolved { .. }) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

/// What: Find selected package names absent from the installed foreign-package inventory.
///
/// Inputs:
/// - `selected`: Exact package names selected in the Targets view.
/// - `foreign`: Read-only `pacman -Qm` rows.
///
/// Output:
/// - Sorted selected names that are outside the scanner's installed-package scope.
///
/// Details:
/// - This prevents an uninstalled search result from silently becoming a broad system scan.
fn missing_selected_foreign_packages(
    selected: &BTreeSet<String>,
    foreign: &[(String, String)],
) -> Vec<String> {
    selected
        .iter()
        .filter(|name| {
            !foreign
                .iter()
                .any(|(foreign_name, _)| foreign_name == *name)
        })
        .cloned()
        .collect()
}

/// What: Resolve all or selected foreign packages into exact official AUR package bases.
///
/// Inputs:
/// - `adapter`: Production adapter owning bounded network/cache state.
/// - `selected`: Optional exact installed package-name filter.
///
/// Output:
/// - Deduplication-ready AUR package records; non-AUR foreign packages are omitted.
///
/// Details:
/// - Filtering happens before network access so a selected foreground scan cannot be blocked by
///   an unrelated foreign package. Actual transport/schema failures remain fatal.
fn enumerate_foreign_filtered(
    adapter: &mut ProductionOrchestrationAdapter,
    selected: Option<&BTreeSet<String>>,
) -> Result<Vec<DiscoveredPackage>, String> {
    adapter.observation_cycle = ObservationCycle::with_deadline(
        adapter.config.head_query_timeout,
        adapter.config.observation_deadline,
    );
    adapter.rpc_by_base.clear();
    let foreign = crate::logic::repos::list_foreign_packages()?;
    if let Some(package_names) = selected {
        let missing = missing_selected_foreign_packages(package_names, &foreign);
        if !missing.is_empty() {
            return Err(format!(
                "selected Pi Scan target(s) are not installed foreign packages: {}. Pi Scan currently supports installed AUR packages and update candidates; install the package or choose an installed AUR target",
                missing.join(", ")
            ));
        }
    }
    let mut grouped: BTreeMap<String, DiscoveredPackage> = BTreeMap::new();
    for (name, version) in foreign {
        if selected.is_some_and(|package_names| !package_names.contains(&name)) {
            continue;
        }
        let package_name = PackageName::new(name.clone()).map_err(|error| error.to_string())?;
        let mut network = adapter.network()?;
        let timeout = adapter
            .observation_cycle
            .remaining_deadline()
            .map_err(|error| error.to_string())?;
        let rpc_result =
            fetch_aur_rpc_package_base_with_timeout(&mut network, package_name.as_str(), timeout);
        let Some(rpc) = classify_foreign_rpc_result(rpc_result)? else {
            continue;
        };
        let package_base =
            resolve_package_base(&package_name, &rpc).map_err(|error| error.to_string())?;
        adapter
            .rpc_by_base
            .entry(package_base.as_str().to_string())
            .or_insert_with(|| rpc.clone());
        let entry = grouped
            .entry(package_base.as_str().to_string())
            .or_insert_with(|| {
                let candidate = adapter.update_candidates.get(&name);
                DiscoveredPackage {
                    package_base: package_base.clone(),
                    installed_names: Vec::new(),
                    installed_version: candidate
                        .map_or_else(|| version.clone(), |item| item.current_version.clone()),
                    candidate_version: candidate.map(|item| item.candidate_version.clone()),
                }
            });
        if !entry.installed_names.contains(&name) {
            if entry.candidate_version.is_none() {
                entry.candidate_version = adapter
                    .update_candidates
                    .get(&name)
                    .map(|candidate| candidate.candidate_version.clone());
            }
            entry.installed_names.push(name);
            entry.installed_names.sort();
        }
    }
    Ok(grouped.into_values().collect())
}

impl OrchestrationAdapter for ProductionOrchestrationAdapter {
    fn probe_setup(&mut self) -> Result<SetupSnapshot, String> {
        let probe = probe_pi_setup(&PiSetupProbeRequest {
            binary: self.config.pi_executable.to_string_lossy().into_owned(),
            workspace_parent: self.config.workspace_parent.join("setup-probe"),
            reservation_tokens: SETUP_PROBE_RESERVATION_TOKENS,
            now_unix_seconds: unix_now(),
            maximum_pricing_age: SETUP_PROBE_MAXIMUM_PRICING_AGE,
        })
        .map_err(|error| error.to_string())?;
        for choice in &self.config.models {
            probe
                .exact_route(&choice.provider, &choice.model)
                .map_err(|error| error.to_string())?;
        }
        let primary = self
            .config
            .models
            .first()
            .cloned()
            .ok_or_else(|| "no primary Pi model was configured".to_string())?;
        let mut reservation = PiScanReservation {
            tokens: SETUP_PROBE_RESERVATION_TOKENS,
            cost_microusd: 0,
        };
        for choice in &self.config.models {
            let route = probe
                .exact_route(&choice.provider, &choice.model)
                .map_err(|error| error.to_string())?;
            reservation.cost_microusd = reservation
                .cost_microusd
                .max(route.reservation.cost_microusd);
        }
        self.config.reservation = reservation;
        self.pi_version = Some(probe.pi_version.to_string());
        Ok(SetupSnapshot {
            pi_version: probe.pi_version.to_string(),
            available_models: probe
                .routes
                .iter()
                .map(|route| (route.provider.clone(), route.model.clone()))
                .collect(),
            selected_provider: primary.provider,
            selected_model: primary.model,
            reservation,
            route_reservations: probe
                .routes
                .iter()
                .map(|route| {
                    (
                        route.provider.clone(),
                        route.model.clone(),
                        route.reservation,
                    )
                })
                .collect(),
            pricing_binding: probe.pricing_binding,
            pricing_observed_at_unix_seconds: probe.pricing_observed_at_unix_seconds,
            maximum_pricing_age_seconds: probe.maximum_pricing_age.as_secs(),
            pricing_summary: probe
                .routes
                .iter()
                .map(|route| {
                    format!(
                        "{}/{} · input={} output={} micro-USD/million · {}",
                        route.provider,
                        route.model,
                        route.pricing.rates.input_microusd_per_million,
                        route.pricing.rates.output_microusd_per_million,
                        route.pricing_provenance
                    )
                })
                .collect(),
        })
    }

    fn dry_run_setup(&mut self) -> Result<SetupSnapshot, String> {
        let primary = self.config.models.first().ok_or_else(|| {
            "select an exact provider/model before running acquisition preview".to_string()
        })?;
        Ok(SetupSnapshot {
            pi_version: "dry-run-pi-not-launched".to_string(),
            available_models: self
                .config
                .models
                .iter()
                .map(|model| (model.provider.clone(), model.model.clone()))
                .collect(),
            selected_provider: primary.provider.clone(),
            selected_model: primary.model.clone(),
            reservation: self.config.reservation,
            route_reservations: self
                .config
                .models
                .iter()
                .map(|model| {
                    (
                        model.provider.clone(),
                        model.model.clone(),
                        self.config.reservation,
                    )
                })
                .collect(),
            pricing_binding: "dry-run-no-pricing".to_string(),
            pricing_observed_at_unix_seconds: 0,
            maximum_pricing_age_seconds: 0,
            pricing_summary: vec!["dry-run: pricing was not probed".to_string()],
        })
    }

    fn set_update_candidates(&mut self, candidates: Vec<UpdateCandidate>) {
        self.update_candidates = candidates
            .into_iter()
            .map(|candidate| (candidate.package_name.clone(), candidate))
            .collect();
    }

    fn enumerate_foreign(&mut self) -> Result<Vec<DiscoveredPackage>, String> {
        enumerate_foreign_filtered(self, None)
    }

    fn enumerate_selected(
        &mut self,
        package_names: &BTreeSet<String>,
    ) -> Result<Vec<DiscoveredPackage>, String> {
        enumerate_foreign_filtered(self, Some(package_names))
    }

    fn observe_package(
        &mut self,
        package: &DiscoveredPackage,
        cursor: Option<&CommitOid>,
    ) -> Result<ObservationPackage, String> {
        let (root, repository) = create_observation_workspace(&self.config.workspace_parent)?;
        let observed = self.observe_in_workspace(package, cursor, &repository);
        if let Err(error) = fs::remove_dir_all(&root)
            && observed.is_ok()
        {
            return Err(format!(
                "could not remove private observation workspace: {error}"
            ));
        }
        observed
    }

    fn execute(
        &mut self,
        target: &FrozenScanIdentity,
        cancelled: &AtomicBool,
    ) -> Result<ExecutionReceipt, ExecutionFailure> {
        if cancelled.load(Ordering::SeqCst) {
            return Err(ExecutionFailure::Cancelled);
        }
        self.execute_frozen(target, cancelled)
    }

    fn revalidate_service(&mut self, target: &FrozenScanIdentity) -> Result<(), String> {
        self.probe_setup()?;
        let receipt = self.dry_run_acquisition(target)?;
        if receipt.status == "failed" {
            Err(
                "service revalidation repeated a failed immutable acquisition; the scanner remains paused"
                    .to_string(),
            )
        } else {
            Ok(())
        }
    }

    fn dry_run_acquisition(
        &mut self,
        target: &FrozenScanIdentity,
    ) -> Result<DryRunAcquisitionReceipt, String> {
        let package_name =
            PackageName::new(target.package_name.clone()).map_err(|error| error.to_string())?;
        let rpc = if let Some(rpc) = self.rpc_by_base.get(target.package_base.as_str()) {
            rpc.clone()
        } else {
            let mut network = self.network()?;
            fetch_aur_rpc_package_base(&mut network, package_name.as_str())
                .map_err(|error| error.to_string())?
        };
        let request = AcquisitionRequest {
            scan_id: target.scan_id.clone(),
            package_name,
            commit_oid: target.commit_oid.clone(),
            rpc,
            limits: AcquisitionLimits::default(),
            dry_run: true,
        };
        let mut http = self.network()?;
        let mut resolver = http.clone();
        let mut verifier = IsolatedSignatureVerifier::production_with_network(
            self.config.workspace_parent.clone(),
            http.clone(),
        );
        let outcome = if let Some(proxy) = self.config.https_proxy.as_deref() {
            acquire_package_with_https_proxy(
                &request,
                &self.config.workspace_parent,
                &self.config.git_executable,
                &mut http,
                &mut resolver,
                &mut self.git,
                &mut verifier,
                proxy,
            )
        } else {
            acquire_package(
                &request,
                &self.config.workspace_parent,
                &self.config.git_executable,
                &mut http,
                &mut resolver,
                &mut self.git,
                &mut verifier,
            )
        }
        .map_err(|error| error.to_string())?;
        let status = match outcome.status {
            AcquisitionStatus::Complete => "complete",
            AcquisitionStatus::Incomplete => "incomplete",
            AcquisitionStatus::Failed => "failed",
        };
        Ok(DryRunAcquisitionReceipt {
            key: crate::state::pi_scan::PiScanQueueKey {
                package_base: target.package_base.clone(),
                commit_oid: target.commit_oid.clone(),
            },
            status: status.to_string(),
            manifest_count: 2,
            coverage_notes: outcome.coverage_notes,
        })
    }

    fn recheck_mutable_sources(
        &mut self,
        sources: &[crate::logic::pi_scan::acquisition::MutableSourceIdentity],
    ) -> Result<bool, String> {
        for identity in sources {
            let mut resolver = self.network()?;
            if mutable_source_identity_changed(
                identity,
                &self.config.git_executable,
                &mut resolver,
                &mut self.git,
                self.config.https_proxy.as_deref(),
                self.config.head_query_timeout,
            )
            .map_err(|error| error.to_string())?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn recheck_continuation(
        &mut self,
        package_base: &crate::logic::pi_scan::identity::PackageBase,
        observed_head_oid: &CommitOid,
    ) -> Result<bool, String> {
        let repo_url = AurRepoUrl::for_package_base(package_base);
        let invocation = head_query_invocation(
            self.config.git_executable.as_os_str(),
            &repo_url,
            self.config.head_query_timeout,
        );
        let output = self
            .run_git(&invocation)
            .map_err(|error| error.to_string())?;
        let current = parse_head_oid(&output).map_err(|error| error.to_string())?;
        Ok(&current != observed_head_oid)
    }
}

impl ProductionOrchestrationAdapter {
    /// Observe one package after preparing an ephemeral bare repository with exact HEAD history.
    fn observe_in_workspace(
        &mut self,
        package: &DiscoveredPackage,
        cursor: Option<&CommitOid>,
        repository: &Path,
    ) -> Result<ObservationPackage, String> {
        let executable = self.config.git_executable.as_os_str().to_os_string();
        let repo_url = AurRepoUrl::for_package_base(&package.package_base);
        let head_invocation = self
            .observation_cycle
            .bounded_invocation(head_query_invocation(
                executable.as_os_str(),
                &repo_url,
                self.config.head_query_timeout,
            ))
            .map_err(|error| error.to_string())?;
        let head_output = self
            .run_git(&head_invocation)
            .map_err(|error| error.to_string())?;
        let head = parse_head_oid(&head_output).map_err(|error| error.to_string())?;
        let init_invocation = self
            .observation_cycle
            .bounded_invocation(init_repository_invocation(
                executable.as_os_str(),
                repository.as_os_str(),
                self.config.head_query_timeout,
            ))
            .map_err(|error| error.to_string())?;
        self.run_git_success(&init_invocation, "init")?;
        let fetch_invocation = self
            .observation_cycle
            .bounded_invocation(fetch_commit_invocation(
                executable.as_os_str(),
                repository.as_os_str(),
                &repo_url,
                &head,
                self.config.head_query_timeout,
            ))
            .map_err(|error| error.to_string())?;
        self.run_git_success(&fetch_invocation, "fetch")?;
        let observation = if let Some(proxy) = self.config.https_proxy.as_deref() {
            let mut runner = ExplicitHttpsProxyGitRunner::new(&mut self.git, proxy);
            observe_package_base(
                &mut runner,
                executable.as_os_str(),
                repository.as_os_str(),
                &package.package_base,
                cursor,
                &mut self.observation_cycle,
            )
        } else {
            observe_package_base(
                &mut self.git,
                executable.as_os_str(),
                repository.as_os_str(),
                &package.package_base,
                cursor,
                &mut self.observation_cycle,
            )
        }
        .map_err(|error| error.to_string())?;
        Ok(ObservationPackage {
            package_base: observation.package_base,
            head_oid: observation.head_oid,
            commits: observation
                .commits
                .into_iter()
                .map(|commit| ObservationCommit {
                    oid: commit.oid,
                    relevance: commit.relevance,
                })
                .collect(),
            truncated: observation.truncated,
            paused_for_rebaseline: observation.paused_for_rebaseline,
        })
    }

    /// Perform one bounded immutable acquisition attempt before any model can run.
    fn acquire_once(
        &mut self,
        request: &AcquisitionRequest,
    ) -> Result<AcquisitionOutcome, AcquisitionError> {
        let mut http = self.network().map_err(|reason| AcquisitionError::Network {
            url: "Pi scanner network adapter".to_string(),
            reason,
        })?;
        let mut resolver = http.clone();
        let mut verifier = IsolatedSignatureVerifier::production_with_network(
            self.config.workspace_parent.clone(),
            http.clone(),
        );
        if let Some(proxy) = self.config.https_proxy.as_deref() {
            acquire_package_with_https_proxy(
                request,
                &self.config.workspace_parent,
                &self.config.git_executable,
                &mut http,
                &mut resolver,
                &mut self.git,
                &mut verifier,
                proxy,
            )
        } else {
            acquire_package(
                request,
                &self.config.workspace_parent,
                &self.config.git_executable,
                &mut http,
                &mut resolver,
                &mut self.git,
                &mut verifier,
            )
        }
    }

    /// Return whether a pre-model acquisition failure is plausibly transient.
    fn is_transient_pre_model(error: &AcquisitionError) -> bool {
        let reason = match error {
            AcquisitionError::Network { reason, .. } => reason.as_str(),
            AcquisitionError::Git { source } => {
                return source.to_string().to_ascii_lowercase().contains("timeout");
            }
            AcquisitionError::PackageBaseUnresolved { .. }
            | AcquisitionError::MembershipUnproven { .. }
            | AcquisitionError::RecipeInvalid { .. }
            | AcquisitionError::Workspace { .. }
            | AcquisitionError::Identity { .. } => return false,
        }
        .to_ascii_lowercase();
        [
            "timeout",
            "timed out",
            "temporar",
            "connection reset",
            "connection refused",
            "dns",
            "status 500",
            "status 502",
            "status 503",
            "status 504",
        ]
        .iter()
        .any(|marker| reason.contains(marker))
    }

    /// Wait exactly one minute before the sole pre-model transient retry.
    fn wait_for_pre_model_retry(cancelled: &AtomicBool) -> Result<(), ExecutionFailure> {
        let deadline = Instant::now() + Duration::from_mins(1);
        while Instant::now() < deadline {
            if cancelled.load(Ordering::SeqCst) {
                return Err(ExecutionFailure::Cancelled);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    }

    /// Acquire immutable snapshots, run the restricted Pi engine, and recheck exact AUR HEAD.
    fn execute_frozen(
        &mut self,
        target: &FrozenScanIdentity,
        cancelled: &AtomicBool,
    ) -> Result<ExecutionReceipt, ExecutionFailure> {
        let package_name =
            PackageName::new(target.package_name.clone()).map_err(service_failure)?;
        let mut pre_model_retry_used = false;
        let rpc = if let Some(rpc) = self.rpc_by_base.get(target.package_base.as_str()) {
            rpc.clone()
        } else {
            let mut network = self.network().map_err(service_failure)?;
            match fetch_aur_rpc_package_base(&mut network, package_name.as_str()) {
                Ok(rpc) => rpc,
                Err(first_error) if Self::is_transient_pre_model(&first_error) => {
                    Self::wait_for_pre_model_retry(cancelled)?;
                    pre_model_retry_used = true;
                    let mut retry_network = self.network().map_err(service_failure)?;
                    fetch_aur_rpc_package_base(&mut retry_network, package_name.as_str()).map_err(
                        |second_error| {
                            ExecutionFailure::Service(format!(
                                "pre-model AUR startup failed twice after the one-minute retry: {first_error}; {second_error}"
                            ))
                        },
                    )?
                }
                Err(error) => return Err(service_failure(error)),
            }
        };
        let request = AcquisitionRequest {
            scan_id: target.scan_id.clone(),
            package_name,
            commit_oid: target.commit_oid.clone(),
            rpc,
            limits: AcquisitionLimits::default(),
            dry_run: false,
        };
        let acquisition = match self.acquire_once(&request) {
            Ok(acquisition) => acquisition,
            Err(first_error)
                if !pre_model_retry_used && Self::is_transient_pre_model(&first_error) =>
            {
                Self::wait_for_pre_model_retry(cancelled)?;
                self.acquire_once(&request).map_err(|second_error| {
                    ExecutionFailure::Service(format!(
                        "pre-model acquisition failed twice after the one-minute retry: {first_error}; {second_error}"
                    ))
                })?
            }
            Err(error) => return Err(service_failure(error)),
        };
        if acquisition.status == AcquisitionStatus::Failed {
            return Err(ExecutionFailure::Service(
                "immutable acquisition failed an integrity check; no Pi scan was accepted"
                    .to_string(),
            ));
        }
        if cancelled.load(Ordering::SeqCst) {
            return Err(ExecutionFailure::Cancelled);
        }
        let pi_version = self.pi_version.as_deref().ok_or_else(|| {
            ExecutionFailure::Service(
                "Pi setup was not verified before execution; re-run scanner setup".to_string(),
            )
        })?;
        let output = execute_scan(ProductionScanRequest {
            workspace_parent: &self.config.workspace_parent,
            executable: &self.config.pi_executable,
            snapshots: &acquisition.snapshots,
            input: ScanExecutionInput {
                prompt: &acquisition.prompt,
                identity: &acquisition.identity,
                evidence: &acquisition.evidence,
                models: &self.config.models,
                thinking: &self.config.thinking,
                pi_version,
                model_attempt_timeout: self.config.model_attempt_timeout,
                logical_timeout: self.config.logical_timeout,
                cancelled,
            },
        })
        .map_err(|error| map_execution_failure(&error))?;
        let mut result = output.result;
        if acquisition.status != AcquisitionStatus::Complete {
            result.coverage = Coverage::Incomplete;
            result
                .limitations
                .extend(acquisition.coverage_notes.clone());
            result.limitations.sort();
            result.limitations.dedup();
        }
        let mutable_changed = self
            .recheck_mutable_sources(&acquisition.mutable_sources)
            .map_err(ExecutionFailure::Service)?;
        let stale = self.recheck_head(target)?
            || mutable_changed
            || acquisition.status != AcquisitionStatus::Complete;
        let tokens = output.provenance.total_tokens();
        Ok(ExecutionReceipt {
            result,
            observed_head_oid: target.observed_head_oid.clone(),
            provenance: output.provenance,
            manifests: vec![acquisition.recipe_manifest, acquisition.source_manifest],
            usage: PiScanActualUsage {
                tokens,
                cost_microusd: target.reservation.cost_microusd,
            },
            stale,
            mutable_sources: acquisition.mutable_sources,
        })
    }

    /// Requery official AUR HEAD and compare it to the frozen observed identity.
    fn recheck_head(&mut self, target: &FrozenScanIdentity) -> Result<bool, ExecutionFailure> {
        let repo_url = AurRepoUrl::for_package_base(&target.package_base);
        let invocation = head_query_invocation(
            self.config.git_executable.as_os_str(),
            &repo_url,
            self.config.head_query_timeout,
        );
        let output = self.run_git(&invocation).map_err(service_failure)?;
        let current = parse_head_oid(&output).map_err(service_failure)?;
        Ok(current != target.observed_head_oid)
    }
}

/// Convert a displayable production seam failure into an orchestration service failure.
fn service_failure(error: impl std::fmt::Display) -> ExecutionFailure {
    ExecutionFailure::Service(error.to_string())
}

/// Preserve sticky cancellation while reducing other engine failures to bounded guidance.
fn map_execution_failure(error: &ExecutionError) -> ExecutionFailure {
    if matches!(error, ExecutionError::Cancelled) {
        ExecutionFailure::Cancelled
    } else {
        ExecutionFailure::Service(error.to_string())
    }
}

/// What: Spawn the production single-owner orchestration service behind the existing UI channels.
///
/// Inputs:
/// - `options`: Explicit feature/dry-run paths plus validated production settings.
///
/// Output:
/// - The same typed channel surface used by the inert WS3 worker.
///
/// Details:
/// - Blocking observation/acquisition/Pi work runs through [`PiScanSequentialRunner`].
/// - Cancellation and shutdown are handled on independent async tasks so an active model call can
///   receive its sticky abort flag while the sequential request owner awaits completion.
///
/// # Errors
/// - Returns actionable configuration, executable, or durable-state failures without affecting
///   the rest of Pacsea.
pub(crate) fn spawn_production_pi_scan_worker(
    options: &PiScanRuntimeOptions,
) -> Result<PiScanRuntimeChannels, String> {
    let settings = options
        .production
        .clone()
        .ok_or_else(|| "production Pi scanner settings were not supplied".to_string())?;
    let root = options
        .state_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Pi scanner state path has no private parent".to_string())?;
    let adapter_config = resolve_production_adapter_config(
        &settings.binary,
        root.join("workspaces"),
        settings.models.clone(),
        &settings.thinking,
        settings.model_attempt_timeout,
        settings.logical_timeout,
        settings.head_query_timeout,
        settings.observation_deadline,
        settings.reservation,
        &settings.https_proxy,
        options.dry_run,
    )?;
    let mut restored = if options.dry_run {
        StoredResultBatch {
            documents: Vec::new(),
            warnings: Vec::new(),
        }
    } else {
        load_all_results(
            &root.join("results-v1"),
            &root.join("quarantine").join("results"),
            unix_now(),
        )
        .map_err(|error| error.to_string())?
    };
    if !options.dry_run {
        restored.warnings.extend(apply_result_retention(
            &root.join("results-v1"),
            &root.join("quarantine").join("results"),
            &restored.documents,
            settings.result_retention_days,
            unix_now(),
        ));
    }
    let orchestrator = PiScanOrchestrator::new(
        OrchestrationConfig {
            enabled: options.effective_enabled(),
            setup_confirmed: false,
            background_execution: settings.background_execution,
            initial_consent: crate::state::pi_scan::PiScanConsentState {
                background_observation: false,
                paid_execution: false,
            },
            consent_binding: production_consent_binding(&settings),
            consent_path: root.join("consent-v1.json"),
            consent_quarantine_dir: root.join("quarantine").join("consent"),
            dry_run: options.dry_run,
            state_path: root.join("orchestration-v1.json"),
            results_root: root.join("results-v1"),
            result_quarantine_dir: root.join("quarantine").join("results"),
            quarantine_dir: root.join("quarantine").join("orchestration"),
            baseline_path: root.join("baseline-v1.json"),
            baseline_quarantine_dir: root.join("quarantine").join("baseline"),
            observation_interval_seconds: settings.observation_interval_seconds,
            budget_limits: settings.budget_limits,
        },
        ProductionOrchestrationAdapter::new(adapter_config),
    )
    .map_err(|error| error.to_string())?;
    let runner = PiScanSequentialRunner::new(orchestrator);
    Ok(spawn_production_channels(
        runner,
        settings,
        options.dry_run,
        restored,
    ))
}

/// Apply configured retention only after semantically validated result loads produced proof.
fn apply_result_retention(
    results_root: &Path,
    quarantine_dir: &Path,
    documents: &[crate::logic::pi_scan::result_store::StoredScanResult],
    retention_days: u32,
    now_unix: u64,
) -> Vec<String> {
    let mut by_base: BTreeMap<String, Vec<_>> = BTreeMap::new();
    for document in documents {
        by_base
            .entry(document.package_base.clone())
            .or_default()
            .push(document);
    }
    let mut warnings = Vec::new();
    for (package_base, package_documents) in by_base {
        let summaries = package_documents
            .iter()
            .map(|document| StoredResultSummary {
                scan_id: document.scan_id.clone(),
                stored_at_unix: document.stored_at_unix,
                accepted_baseline: document.accepted_baseline,
            })
            .collect::<Vec<_>>();
        let plan = plan_retention(&summaries, now_unix, u64::from(retention_days));
        if plan.delete.is_empty() {
            continue;
        }
        let Some(proof_document) = package_documents
            .iter()
            .max_by_key(|document| document.stored_at_unix)
        else {
            continue;
        };
        let outcome = load_result(
            results_root,
            quarantine_dir,
            &package_base,
            &proof_document.scan_id,
            now_unix,
        )
        .and_then(|(_, receipt)| {
            cleanup_expired_results(results_root, &package_base, &plan, &receipt)
        });
        if let Err(error) = outcome {
            warnings.push(format!(
                "Pi result retention for {package_base} was skipped: {error}"
            ));
        }
    }
    warnings
}

/// Hash material provider/model/privacy/pricing configuration for consent invalidation.
pub(crate) fn production_consent_binding(settings: &ProductionRuntimeSettings) -> String {
    let material = serde_json::json!({
        "binary": settings.binary,
        "models": settings.models.iter().map(|model| {
            serde_json::json!({"provider": model.provider, "model": model.model})
        }).collect::<Vec<_>>(),
        "background_execution": settings.background_execution,
        "thinking": settings.thinking,
        "https_proxy": settings.https_proxy,
        "budget_starts_per_hour": settings.budget_limits.starts_per_hour,
        "budget_tokens_per_24h": settings.budget_limits.tokens_per_24h,
        "budget_cost_microusd_per_24h": settings.budget_limits.cost_microusd_per_24h,
        "extension_sha256": crate::pi_agent::process::EMBEDDED_EXTENSION_SHA256,
        "tool_contract": crate::pi_agent::TOOL_CONTRACT_VERSION,
        "prompt_version": crate::logic::pi_scan::prompt::PROMPT_VERSION,
        "result_schema": crate::logic::pi_scan::prompt::SCHEMA_VERSION,
    });
    let bytes = serde_json::to_vec(&material).unwrap_or_default();
    sha256_hex(&bytes)
}

/// Compute lowercase SHA-256 hexadecimal without introducing a formatting dependency.
fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

/// Build channel pairs and launch the production request, cancellation, and shutdown owners.
fn spawn_production_channels(
    runner: PiScanSequentialRunner<ProductionOrchestrationAdapter>,
    settings: ProductionRuntimeSettings,
    dry_run: bool,
    restored: StoredResultBatch,
) -> PiScanRuntimeChannels {
    let (request_tx, request_rx) = tokio::sync::mpsc::unbounded_channel();
    let (cancel_tx, cancel_rx) = tokio::sync::mpsc::unbounded_channel();
    let (session_tx, session_rx) = tokio::sync::mpsc::unbounded_channel();
    let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::unbounded_channel();
    let (progress_tx, progress_rx) = tokio::sync::mpsc::unbounded_channel();
    let (result_tx, result_rx) = tokio::sync::mpsc::unbounded_channel();
    let (notice_tx, notice_rx) = tokio::sync::mpsc::unbounded_channel();
    let (execution_tx, execution_rx) = tokio::sync::mpsc::unbounded_channel();

    if !restored.documents.is_empty() {
        drop(progress_tx.send(PiScanProgressMessage::RestoredResults {
            documents: restored.documents,
        }));
    }
    for warning in restored.warnings {
        reject(&result_tx, warning);
    }

    let request_senders = ProductionRequestSenders {
        progress: progress_tx.clone(),
        result: result_tx.clone(),
        execution: execution_tx,
        notice: notice_tx,
    };
    tokio::spawn(run_production_requests(
        runner.clone(),
        request_rx,
        request_senders,
        settings.clone(),
        dry_run,
    ));
    tokio::spawn(run_production_execution(
        runner.clone(),
        execution_rx,
        progress_tx.clone(),
        result_tx.clone(),
        settings,
    ));
    tokio::spawn(run_production_cancellations(
        runner.clone(),
        cancel_rx,
        progress_tx.clone(),
        result_tx.clone(),
    ));
    tokio::spawn(run_production_shutdown(runner, shutdown_rx, progress_tx));
    tokio::spawn(drain_unused_session_registrations(session_rx, result_tx));

    PiScanRuntimeChannels {
        request_tx,
        cancel_tx,
        session_tx,
        shutdown_tx,
        progress_rx,
        result_rx,
        notice_rx,
    }
}

/// Runtime request-loop output channels grouped to keep protocol ownership cohesive.
#[derive(Clone)]
struct ProductionRequestSenders {
    /// Progress projection sender.
    progress: tokio::sync::mpsc::UnboundedSender<PiScanProgressMessage>,
    /// Terminal and legacy rejection sender.
    result: tokio::sync::mpsc::UnboundedSender<PiScanResultMessage>,
    /// Sequential execution wake-up sender.
    execution: tokio::sync::mpsc::UnboundedSender<()>,
    /// Provenance-bearing runtime notice sender.
    notice: tokio::sync::mpsc::UnboundedSender<PiScanRuntimeNotice>,
}

/// Session-owned consent projection controlling when external work may start.
#[derive(Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "observation lifecycle, foreground payment, and background payment are independent runtime gates"
)]
struct RuntimeConsentProjection {
    /// Read-only background observation is currently consented.
    observation_enabled: bool,
    /// At least one consented observation completed successfully this runtime.
    observation_started: bool,
    /// Foreground paid execution is currently consented.
    paid_execution: bool,
    /// Paid background execution is independently consented.
    background_paid_execution: bool,
}

/// Return whether the periodic timer may attempt consented background observation.
#[must_use]
const fn background_observation_due(consent: &RuntimeConsentProjection) -> bool {
    consent.observation_enabled
}

/// Own sequential observation, queue promotion, runtime policy, and execution requests.
async fn run_production_requests(
    runner: PiScanSequentialRunner<ProductionOrchestrationAdapter>,
    mut request_rx: tokio::sync::mpsc::UnboundedReceiver<PiScanRequestMessage>,
    senders: ProductionRequestSenders,
    settings: ProductionRuntimeSettings,
    dry_run: bool,
) {
    let progress_tx = senders.progress.clone();
    let result_tx = senders.result.clone();
    let execution_tx = senders.execution.clone();
    let restored_state = match runner.state_snapshot().await {
        Ok(state) => state,
        Err(error) => {
            reject(&result_tx, error.to_string());
            return;
        }
    };
    drop(
        progress_tx.send(PiScanProgressMessage::RestoredRuntime(Box::new(
            restored_state,
        ))),
    );
    let (mut runtime_consent, mut setup_consent) = match runner.consent_snapshot().await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            reject(&result_tx, error.to_string());
            return;
        }
    };
    let has_persisted_setup_consent = setup_consent.disclosure_confirmed
        || setup_consent.fallback_confirmed
        || setup_consent.readiness_warning_confirmed
        || !setup_consent.confirmed_pi_version.is_empty()
        || !setup_consent.confirmed_pricing_binding.is_empty();
    if !dry_run && has_persisted_setup_consent {
        let current_setup = match runner.setup_snapshot().await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                reject(&result_tx, error.to_string());
                return;
            }
        };
        drop(progress_tx.send(PiScanProgressMessage::SetupVerified(current_setup.clone())));
        let material_changed = setup_consent.confirmed_pi_version != current_setup.pi_version
            || setup_consent.confirmed_pricing_binding != current_setup.pricing_binding;
        if material_changed {
            runtime_consent = crate::state::pi_scan::PiScanConsentState::default();
            setup_consent = crate::pi_scan_orchestrator::PiScanSetupConsentState {
                configuration_binding: setup_consent.configuration_binding,
                confirmed_pi_version: current_setup.pi_version.clone(),
                confirmed_pricing_binding: current_setup.pricing_binding.clone(),
                ..crate::pi_scan_orchestrator::PiScanSetupConsentState::default()
            };
            let reset = runner
                .update_runtime_policy(Some(runtime_consent), None, false, None)
                .await
                .and(
                    runner
                        .update_setup_consent(PiScanSetupConsentState {
                            configuration_binding: String::new(),
                            disclosure_confirmed: false,
                            fallback_confirmed: false,
                            background_paid_execution: false,
                            readiness_warning_confirmed: false,
                            confirmed_pi_version: current_setup.pi_version,
                            confirmed_pricing_binding: current_setup.pricing_binding,
                        })
                        .await,
                );
            if let Err(error) = reset {
                reject(&result_tx, error.to_string());
                return;
            }
            reject(
                &result_tx,
                "Pi version or exact route pricing changed; persisted scanner consent was reset and must be confirmed again"
                    .to_string(),
            );
        }
    }
    let background_paid_execution = setup_consent.background_paid_execution;
    drop(progress_tx.send(PiScanProgressMessage::RestoredConsent {
        consent: runtime_consent,
        setup: setup_consent,
    }));
    let mut consent = RuntimeConsentProjection {
        observation_enabled: runtime_consent.background_observation,
        observation_started: false,
        paid_execution: runtime_consent.paid_execution,
        background_paid_execution,
    };
    if consent.observation_enabled {
        let observed = runner.startup_observation(unix_now()).await;
        consent.observation_started = observed.is_ok();
        publish_observation(
            observed,
            "background startup observation",
            &progress_tx,
            &result_tx,
        );
        if consent.observation_started
            && settings.background_execution
            && consent.paid_execution
            && consent.background_paid_execution
            && !dry_run
        {
            request_execution(&execution_tx);
        }
    }
    let mut interval =
        tokio::time::interval(Duration::from_secs(settings.observation_interval_seconds));
    interval.tick().await;
    loop {
        tokio::select! {
            request = request_rx.recv() => {
                let Some(request) = request else { break; };
                handle_production_request(
                    &runner,
                    request,
                    &senders,
                    &settings,
                    &mut consent,
                    dry_run,
                ).await;
            }
            _ = interval.tick(), if background_observation_due(&consent) => {
                let observed = runner.periodic_observation(unix_now()).await;
                let observation_succeeded = observed.is_ok();
                consent.observation_started |= observation_succeeded;
                publish_observation(
                    observed,
                    "background periodic observation",
                    &progress_tx,
                    &result_tx,
                );
                if observation_succeeded
                    && settings.background_execution
                    && consent.paid_execution
                    && consent.background_paid_execution
                    && !dry_run
                {
                    request_execution(&execution_tx);
                }
            }
        }
    }
}

/// Optional setup confirmations accompanying one explicit consent update.
#[derive(Clone, Copy)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "disclosure, fallback, background payment, and readiness are independent consent decisions"
)]
struct ProductionSetupConsentUpdate {
    /// Disclosure confirmation.
    disclosure_confirmed: bool,
    /// Ordered fallback confirmation.
    fallback_confirmed: bool,
    /// Independent paid background-execution confirmation.
    background_paid_execution_confirmed: bool,
    /// Readiness-warning confirmation.
    readiness_warning_confirmed: bool,
}

/// Runtime consent plus optional setup confirmations from one explicit caller action.
#[derive(Clone, Copy)]
struct ProductionConsentUpdate {
    /// Independent observation and paid-execution consent.
    consent: crate::state::pi_scan::PiScanConsentState,
    /// Material-bound setup confirmations when changed by the UI.
    setup: Option<ProductionSetupConsentUpdate>,
}

/// Apply one typed UI/runtime request to the central production owner.
#[allow(
    clippy::too_many_lines,
    reason = "the exhaustive typed request dispatcher keeps each security gate explicit"
)]
async fn handle_production_request(
    runner: &PiScanSequentialRunner<ProductionOrchestrationAdapter>,
    request: PiScanRequestMessage,
    senders: &ProductionRequestSenders,
    settings: &ProductionRuntimeSettings,
    consent_state: &mut RuntimeConsentProjection,
    dry_run: bool,
) {
    let progress_tx = &senders.progress;
    let result_tx = &senders.result;
    let execution_tx = &senders.execution;
    let notice_tx = &senders.notice;
    match request {
        PiScanRequestMessage::ProbeSetup if dry_run => reject(
            result_tx,
            "dry-run setup is inert and never probes Pi; select a target to preview bounded acquisition"
                .to_string(),
        ),
        PiScanRequestMessage::ProbeSetup => match runner.setup_snapshot().await {
            Ok(snapshot) => {
                drop(progress_tx.send(PiScanProgressMessage::SetupVerified(snapshot)));
            }
            Err(error) => reject(result_tx, error.to_string()),
        },
        PiScanRequestMessage::ManualObservation { package_names } => {
            publish_observation(
                runner
                    .manual_observation_selected(unix_now(), package_names)
                    .await,
                "selected-target observation",
                progress_tx,
                result_tx,
            );
        }
        PiScanRequestMessage::Enqueue(request) => {
            handle_production_enqueue(
                runner,
                request,
                progress_tx,
                result_tx,
                dry_run,
                execution_tx,
            )
            .await;
        }
        PiScanRequestMessage::SetConsentDetails {
            consent,
            disclosure_confirmed,
            fallback_confirmed,
            background_paid_execution_confirmed,
            readiness_warning_confirmed,
        } => {
            apply_production_consent(
                runner,
                ProductionConsentUpdate {
                    consent,
                    setup: Some(ProductionSetupConsentUpdate {
                        disclosure_confirmed,
                        fallback_confirmed,
                        background_paid_execution_confirmed,
                        readiness_warning_confirmed,
                    }),
                },
                senders,
                settings,
                consent_state,
                dry_run,
            )
            .await;
        }
        PiScanRequestMessage::SetConsent(consent) => {
            apply_production_consent(
                runner,
                ProductionConsentUpdate {
                    consent,
                    setup: None,
                },
                senders,
                settings,
                consent_state,
                dry_run,
            )
            .await;
        }
        PiScanRequestMessage::SetUserPaused(paused) => {
            handle_user_pause(runner, paused, notice_tx, result_tx).await;
        }
        PiScanRequestMessage::PauseForService => {
            publish_policy_result(
                runner.update_runtime_policy(None, None, true, None).await,
                result_tx,
            );
        }
        PiScanRequestMessage::ClearServicePause { .. } => reject(
            result_tx,
            "production service pauses clear only after exact setup and acquisition revalidation during an explicit target retry"
                .to_string(),
        ),
        PiScanRequestMessage::UpdateCandidates(candidates) => {
            match runner.set_update_candidates(candidates).await {
                Ok(())
                    if consent_state.observation_enabled && consent_state.observation_started =>
                {
                    publish_observation(
                        runner.update_candidate_observation(unix_now()).await,
                        "background update-candidate observation",
                        progress_tx,
                        result_tx,
                    );
                }
                Ok(()) => {}
                Err(error) => reject(result_tx, error.to_string()),
            }
        }
        PiScanRequestMessage::AcceptBaseline {
            package_base,
            commit_oid,
            scan_id,
            result_binding,
        } => match runner
            .accept_baseline(
                package_base,
                commit_oid,
                scan_id,
                result_binding.clone(),
                unix_now(),
            )
            .await
        {
            Ok(()) => {
                drop(result_tx.send(PiScanResultMessage::BaselineAccepted { result_binding }));
            }
            Err(error) => reject(result_tx, error.to_string()),
        },
        PiScanRequestMessage::ValidateContinuation {
            package_base,
            observed_head_oid,
            mutable_sources,
            result_binding,
        } => match runner
            .validate_continuation_with_sources(
                package_base.clone(),
                observed_head_oid,
                mutable_sources,
            )
            .await
        {
            Ok(stale) => {
                drop(result_tx.send(PiScanResultMessage::ContinuationValidated {
                    package_base,
                    result_binding,
                    stale,
                }));
            }
            Err(error) => reject(result_tx, error.to_string()),
        },
        PiScanRequestMessage::RevalidateBudgets { .. } => {}
        PiScanRequestMessage::Complete { .. } => reject(
            result_tx,
            "production Pi completion is accepted only from the central orchestrator".to_string(),
        ),
    }
}

/// Dispatch one foreground request through dry-run acquisition or durable promotion.
async fn handle_production_enqueue(
    runner: &PiScanSequentialRunner<ProductionOrchestrationAdapter>,
    request: crate::state::pi_scan::PiScanJobRequest,
    progress_tx: &tokio::sync::mpsc::UnboundedSender<PiScanProgressMessage>,
    result_tx: &tokio::sync::mpsc::UnboundedSender<PiScanResultMessage>,
    dry_run: bool,
    execution_tx: &tokio::sync::mpsc::UnboundedSender<()>,
) {
    if dry_run {
        match runner.dry_run_acquisition(request.key.clone()).await {
            Ok(receipt) => {
                drop(progress_tx.send(PiScanProgressMessage::DryRunPreview(request)));
                drop(result_tx.send(PiScanResultMessage::DryRunAcquired {
                    key: receipt.key,
                    status: receipt.status,
                    manifest_count: receipt.manifest_count,
                    coverage_notes: receipt.coverage_notes,
                }));
            }
            Err(error) => reject(result_tx, error.to_string()),
        }
        return;
    }
    match runner.promote_queued(request.key.clone()).await {
        Ok(request_id) => {
            let mut request = request;
            request.request_id = request_id;
            drop(progress_tx.send(PiScanProgressMessage::Queued {
                request,
                queue_len: 1,
            }));
            request_execution(execution_tx);
        }
        Err(error) => reject(result_tx, error.to_string()),
    }
}

/// Persist explicit consent and start newly consented observation when setup permits.
async fn apply_production_consent(
    runner: &PiScanSequentialRunner<ProductionOrchestrationAdapter>,
    update: ProductionConsentUpdate,
    senders: &ProductionRequestSenders,
    settings: &ProductionRuntimeSettings,
    consent_state: &mut RuntimeConsentProjection,
    dry_run: bool,
) {
    let progress_tx = &senders.progress;
    let result_tx = &senders.result;
    let execution_tx = &senders.execution;
    let setup_identity = if let Some(setup) = update.setup {
        let needs_verification = setup.disclosure_confirmed
            || setup.fallback_confirmed
            || setup.readiness_warning_confirmed
            || update.consent.background_observation
            || update.consent.paid_execution;
        if needs_verification {
            match runner.setup_snapshot().await {
                Ok(snapshot) => {
                    drop(progress_tx.send(PiScanProgressMessage::SetupVerified(snapshot.clone())));
                    Some(snapshot)
                }
                Err(error) => {
                    reject(result_tx, error.to_string());
                    return;
                }
            }
        } else {
            None
        }
    } else {
        None
    };
    let runtime_policy = runner
        .update_runtime_policy(Some(update.consent), None, false, None)
        .await;
    let setup_policy = if let Some(setup) = update.setup {
        let (pi_version, pricing_binding) = setup_identity.map_or_else(
            || (String::new(), String::new()),
            |snapshot| (snapshot.pi_version, snapshot.pricing_binding),
        );
        runner
            .update_setup_consent(PiScanSetupConsentState {
                configuration_binding: String::new(),
                disclosure_confirmed: setup.disclosure_confirmed,
                fallback_confirmed: setup.fallback_confirmed,
                background_paid_execution: setup.background_paid_execution_confirmed,
                readiness_warning_confirmed: setup.readiness_warning_confirmed,
                confirmed_pi_version: pi_version,
                confirmed_pricing_binding: pricing_binding,
            })
            .await
    } else {
        Ok(())
    };
    if let Err(error) = runtime_policy.and(setup_policy) {
        reject(result_tx, error.to_string());
        return;
    }
    consent_state.observation_enabled = update.consent.background_observation;
    consent_state.paid_execution = update.consent.paid_execution;
    if let Some(setup) = update.setup {
        consent_state.background_paid_execution = setup.background_paid_execution_confirmed;
    }
    let disclosure_allows_observation = update.setup.is_none_or(|setup| setup.disclosure_confirmed);
    if !disclosure_allows_observation
        || !update.consent.background_observation
        || consent_state.observation_started
    {
        return;
    }
    let observed = runner.startup_observation(unix_now()).await;
    consent_state.observation_started = observed.is_ok();
    publish_observation(
        observed,
        "background startup observation",
        progress_tx,
        result_tx,
    );
    if consent_state.observation_started
        && settings.background_execution
        && update.consent.paid_execution
        && consent_state.background_paid_execution
        && !dry_run
    {
        request_execution(execution_tx);
    }
}

/// Publish a completed observation or reduce its failure to an optional-feature rejection.
fn publish_observation(
    observed: Result<Vec<FrozenScanIdentity>, OrchestrationError>,
    context: &str,
    progress_tx: &tokio::sync::mpsc::UnboundedSender<PiScanProgressMessage>,
    result_tx: &tokio::sync::mpsc::UnboundedSender<PiScanResultMessage>,
) {
    match observed {
        Ok(targets) => {
            drop(progress_tx.send(PiScanProgressMessage::Observed { targets }));
        }
        Err(error) => reject(result_tx, format!("{context} failed: {error}")),
    }
}

/// Queue one execution wake-up without blocking the runtime request owner.
fn request_execution(execution_tx: &tokio::sync::mpsc::UnboundedSender<()>) {
    let _ = execution_tx.send(());
}

/// What: Own all production execution starts independently from policy request reception.
///
/// Inputs:
/// - Serialized runner, coalescing wake-up receiver, projection channels, and route settings.
///
/// Output:
/// - Sequential queue drain after each wake-up.
///
/// Details:
/// - The runner lock remains the sole execution owner. Separating this loop lets Pause/Resume
///   publish a truthful queued acknowledgement while a model call is still active.
async fn run_production_execution(
    runner: PiScanSequentialRunner<ProductionOrchestrationAdapter>,
    mut execution_rx: tokio::sync::mpsc::UnboundedReceiver<()>,
    progress_tx: tokio::sync::mpsc::UnboundedSender<PiScanProgressMessage>,
    result_tx: tokio::sync::mpsc::UnboundedSender<PiScanResultMessage>,
    settings: ProductionRuntimeSettings,
) {
    while execution_rx.recv().await.is_some() {
        drain_eligible_queue(&runner, &progress_tx, &result_tx, &settings).await;
        while execution_rx.try_recv().is_ok() {}
    }
}

/// What: Publish a truthful pause acknowledgement and persist it at the correct boundary.
///
/// Inputs:
/// - Runner, requested pause value, typed notice sender, and legacy error sender.
///
/// Output:
/// - Immediate `Queued` while active, followed by `Persisted` or `Failed`.
///
/// Details:
/// - Idle changes use the ordinary owner lock. Active changes are completed by `run_next` before
///   releasing its active registration, which prevents another job from starting first.
async fn handle_user_pause(
    runner: &PiScanSequentialRunner<ProductionOrchestrationAdapter>,
    paused: bool,
    notice_tx: &tokio::sync::mpsc::UnboundedSender<PiScanRuntimeNotice>,
    result_tx: &tokio::sync::mpsc::UnboundedSender<PiScanResultMessage>,
) {
    let queued = match runner.queue_user_pause_if_active(paused) {
        Ok(queued) => queued,
        Err(error) => {
            publish_policy_notice(notice_tx, paused, None, Err(error.to_string()));
            reject(result_tx, error.to_string());
            return;
        }
    };
    if let Some((correlation_id, completion)) = queued {
        publish_policy_notice(
            notice_tx,
            paused,
            Some(correlation_id),
            Ok(PiScanPolicyAcknowledgement::Queued),
        );
        let result = completion.await.map_err(|_| {
            "queued Pi pause persistence completion was lost; retry the policy action".to_string()
        });
        let result = result.and_then(|outcome| outcome.map_err(|error| error.to_string()));
        publish_policy_notice(
            notice_tx,
            paused,
            Some(correlation_id),
            result.map(|()| PiScanPolicyAcknowledgement::Persisted),
        );
        return;
    }
    let result = runner
        .update_runtime_policy(None, Some(paused), false, None)
        .await
        .map_err(|error| error.to_string());
    publish_policy_notice(
        notice_tx,
        paused,
        None,
        result.map(|()| PiScanPolicyAcknowledgement::Persisted),
    );
}

/// Publish one provenance-bearing pause policy notice.
fn publish_policy_notice(
    notice_tx: &tokio::sync::mpsc::UnboundedSender<PiScanRuntimeNotice>,
    paused: bool,
    correlation_id: Option<u64>,
    acknowledgement: Result<PiScanPolicyAcknowledgement, String>,
) {
    let action = if paused {
        PiScanRuntimeAction::Pause
    } else {
        PiScanRuntimeAction::Resume
    };
    let acknowledgement =
        acknowledgement.unwrap_or_else(|reason| PiScanPolicyAcknowledgement::Failed { reason });
    drop(notice_tx.send(PiScanRuntimeNotice {
        provenance: PiScanNoticeProvenance {
            source: PiScanNoticeSource::Foreground,
            action: Some(action),
            correlation_id,
        },
        user_paused: paused,
        acknowledgement,
    }));
}

/// Drain sequential eligible work until queue, pause, consent, or budget blocks the next start.
async fn drain_eligible_queue(
    runner: &PiScanSequentialRunner<ProductionOrchestrationAdapter>,
    progress_tx: &tokio::sync::mpsc::UnboundedSender<PiScanProgressMessage>,
    result_tx: &tokio::sync::mpsc::UnboundedSender<PiScanResultMessage>,
    settings: &ProductionRuntimeSettings,
) {
    if settings.models.len() > 1 {
        match runner.consent_snapshot().await {
            Ok((_, setup)) if setup.fallback_confirmed => {}
            Ok(_) => {
                reject(
                    result_tx,
                    "ordered Pi fallback routes require explicit confirmation before execution"
                        .to_string(),
                );
                return;
            }
            Err(error) => {
                reject(result_tx, error.to_string());
                return;
            }
        }
    }
    while execute_one(runner, progress_tx, result_tx).await {}
}

/// Execute at most one queued item while publishing its exact active correlation.
async fn execute_one(
    runner: &PiScanSequentialRunner<ProductionOrchestrationAdapter>,
    progress_tx: &tokio::sync::mpsc::UnboundedSender<PiScanProgressMessage>,
    result_tx: &tokio::sync::mpsc::UnboundedSender<PiScanResultMessage>,
) -> bool {
    let started_at = unix_now();
    let (started_tx, mut started_rx) = tokio::sync::mpsc::unbounded_channel();
    let run = {
        let runner = runner.clone();
        tokio::spawn(async move { runner.run_next_with_started(started_at, started_tx).await })
    };
    let active = started_rx.recv().await;
    if let Some(item) = active.as_ref() {
        drop(progress_tx.send(PiScanProgressMessage::Started(item.clone())));
    }
    let had_active = active.is_some();
    match run.await {
        Ok(Ok(Some(receipt))) => {
            drop(result_tx.send(PiScanResultMessage::Validated(Box::new(receipt))));
            true
        }
        Ok(Ok(None)) => false,
        Ok(Err(OrchestrationError::Cancelled)) => {
            publish_cancelled(active, result_tx);
            had_active
        }
        Ok(Err(error)) => {
            reject(result_tx, error.to_string());
            false
        }
        Err(error) => {
            reject(
                result_tx,
                format!("Pi orchestration execution task failed: {error}"),
            );
            false
        }
    }
}

/// Publish a terminal cancellation from the exact active item when it was registered.
fn publish_cancelled(
    active: Option<crate::state::pi_scan::PiScanActiveItem>,
    result_tx: &tokio::sync::mpsc::UnboundedSender<PiScanResultMessage>,
) {
    let Some(active) = active else {
        reject(
            result_tx,
            "Pi scan was cancelled before an active identity could be projected".to_string(),
        );
        return;
    };
    drop(result_tx.send(PiScanResultMessage::Cancelled {
        record: PiScanTerminalRecord {
            request: active.request,
            correlation_id: active.correlation_id,
            status: PiScanTerminalStatus::Cancelled,
            finished_at_unix: unix_now(),
        },
        warning: None,
    }));
}

/// Apply exact correlated cancellation while execution continues on the blocking pool.
async fn run_production_cancellations(
    runner: PiScanSequentialRunner<ProductionOrchestrationAdapter>,
    mut cancel_rx: tokio::sync::mpsc::UnboundedReceiver<PiScanCancelMessage>,
    progress_tx: tokio::sync::mpsc::UnboundedSender<PiScanProgressMessage>,
    result_tx: tokio::sync::mpsc::UnboundedSender<PiScanResultMessage>,
) {
    while let Some(cancel) = cancel_rx.recv().await {
        if runner.cancel(cancel.correlation_id) {
            drop(progress_tx.send(PiScanProgressMessage::Cancelling {
                correlation_id: cancel.correlation_id,
            }));
        } else {
            reject(
                &result_tx,
                format!(
                    "Pi cancellation correlation {} is no longer active",
                    cancel.correlation_id
                ),
            );
        }
    }
}

/// Cancel/reap the active service and acknowledge the durable shutdown boundary.
async fn run_production_shutdown(
    runner: PiScanSequentialRunner<ProductionOrchestrationAdapter>,
    mut shutdown_rx: tokio::sync::mpsc::UnboundedReceiver<PiScanShutdownMessage>,
    progress_tx: tokio::sync::mpsc::UnboundedSender<PiScanProgressMessage>,
) {
    while let Some(shutdown) = shutdown_rx.recv().await {
        let active_interrupted = runner.active_correlation().is_some();
        let result = runner.shutdown(unix_now()).await;
        let ack = PiScanShutdownAck {
            persisted: result.is_ok(),
            active_interrupted,
            warning: result.err().map(|error| error.to_string()),
        };
        drop(shutdown.acknowledge.send(ack.clone()));
        drop(progress_tx.send(PiScanProgressMessage::Shutdown(ack)));
    }
}

/// Reject unexpected legacy WS2 session registrations without leaking a child process.
async fn drain_unused_session_registrations(
    mut session_rx: tokio::sync::mpsc::UnboundedReceiver<PiScanSessionRegistration>,
    result_tx: tokio::sync::mpsc::UnboundedSender<PiScanResultMessage>,
) {
    while let Some(mut registration) = session_rx.recv().await {
        let warning = registration.target.abort_and_reap().err();
        reject(
            &result_tx,
            warning.map_or_else(
                || {
                    "external Pi session registration is not accepted by the production orchestrator"
                        .to_string()
                },
                |reason| format!("external Pi session was rejected and cleanup failed: {reason}"),
            ),
        );
    }
}

/// Publish a runtime-policy failure while successful updates remain quiet.
fn publish_policy_result(
    result: Result<(), OrchestrationError>,
    result_tx: &tokio::sync::mpsc::UnboundedSender<PiScanResultMessage>,
) {
    if let Err(error) = result {
        reject(result_tx, error.to_string());
    }
}

/// Send one bounded rejection without panicking when the UI has already closed.
fn reject(result_tx: &tokio::sync::mpsc::UnboundedSender<PiScanResultMessage>, reason: String) {
    drop(result_tx.send(PiScanResultMessage::Rejected { reason }));
}

/// Return current Unix seconds for orchestration accounting and persistence.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::{
        ProbedModel, ProductionOrchestrationAdapter, RuntimeConsentProjection,
        background_observation_due, classify_foreign_rpc_result, missing_selected_foreign_packages,
        publish_observation, publish_policy_notice, reservation_from_probed_models,
        resolve_production_adapter_config,
    };
    use crate::app::runtime::workers::pi_scan::{
        PiScanNoticeSource, PiScanPolicyAcknowledgement, PiScanResultMessage, PiScanRuntimeAction,
    };
    use crate::logic::pi_scan::acquisition::AcquisitionError;
    use crate::pi_agent::session::ModelChoice;
    use crate::pi_scan_orchestrator::{OrchestrationAdapter, OrchestrationError};
    use crate::state::pi_scan::PiScanReservation;
    use std::collections::BTreeSet;
    use std::time::Duration;

    /// A transient startup failure must not permanently disable consented periodic observation.
    #[test]
    fn background_observation_remains_due_after_startup_failure() {
        let consent = RuntimeConsentProjection {
            observation_enabled: true,
            observation_started: false,
            paid_execution: true,
            background_paid_execution: true,
        };

        assert!(background_observation_due(&consent));
    }

    /// Background observation failures must identify their trigger in the status notice.
    #[test]
    fn background_observation_failure_names_its_trigger() {
        let (progress_tx, _progress_rx) = tokio::sync::mpsc::unbounded_channel();
        let (result_tx, mut result_rx) = tokio::sync::mpsc::unbounded_channel();

        publish_observation(
            Err(OrchestrationError::Observation(
                "network unavailable".to_string(),
            )),
            "background startup observation",
            &progress_tx,
            &result_tx,
        );

        assert!(matches!(
            result_rx.try_recv(),
            Ok(PiScanResultMessage::Rejected { reason })
                if reason == "background startup observation failed: network unavailable"
        ));
    }

    /// Policy acknowledgement must carry typed foreground provenance independent of UI history.
    #[test]
    fn policy_notice_carries_action_and_correlation_provenance() {
        let (notice_tx, mut notice_rx) = tokio::sync::mpsc::unbounded_channel();

        publish_policy_notice(
            &notice_tx,
            true,
            Some(42),
            Ok(PiScanPolicyAcknowledgement::Queued),
        );

        let notice = notice_rx.try_recv().expect("typed notice");
        assert_eq!(notice.provenance.source, PiScanNoticeSource::Foreground);
        assert_eq!(notice.provenance.action, Some(PiScanRuntimeAction::Pause));
        assert_eq!(notice.provenance.correlation_id, Some(42));
        assert_eq!(notice.acknowledgement, PiScanPolicyAcknowledgement::Queued);
    }

    /// An uninstalled search result must produce scoped guidance instead of scanning everything.
    #[test]
    fn selected_target_must_be_an_installed_foreign_package() {
        let selected = BTreeSet::from(["pacsea-bin".to_string()]);
        let foreign = vec![("qml-vulkan".to_string(), "1.0-1".to_string())];

        assert_eq!(
            missing_selected_foreign_packages(&selected, &foreign),
            ["pacsea-bin"]
        );
    }

    /// Non-AUR foreign packages must not block discovery of later exact AUR packages.
    #[test]
    fn unresolved_foreign_package_is_skipped_during_discovery() {
        let classified =
            classify_foreign_rpc_result(Err(AcquisitionError::PackageBaseUnresolved {
                package_name: "qml-vulkan".to_string(),
                reason: "AUR returned no exact result".to_string(),
            }))
            .expect("non-AUR foreign package is skippable during discovery");

        assert!(classified.is_none());
    }

    /// Verify Pi native per-million prices produce an exact conservative reservation.
    #[test]
    fn probed_model_prices_bound_the_reservation_exactly() {
        let available = vec![ProbedModel {
            provider: "anthropic".to_string(),
            model: "claude-x".to_string(),
            cost: Some(serde_json::json!({ "input": 3.0, "output": 15.0 })),
        }];
        let choices = vec![ModelChoice {
            provider: "anthropic".to_string(),
            model: "claude-x".to_string(),
        }];
        let reservation = reservation_from_probed_models(
            &available,
            &choices,
            PiScanReservation {
                tokens: 10_000,
                cost_microusd: 150_000,
            },
        )
        .expect("exact cap accepts");
        assert_eq!(reservation.tokens, 10_000);
        assert_eq!(reservation.cost_microusd, 150_000);
        assert!(
            reservation_from_probed_models(
                &available,
                &choices,
                PiScanReservation {
                    tokens: 10_000,
                    cost_microusd: 149_999,
                },
            )
            .is_err(),
            "one micro-USD below the exact reservation must fail closed"
        );
    }

    /// Verify acquisition-only dry-run neither resolves nor probes the configured Pi binary.
    #[test]
    fn dry_run_configuration_keeps_pi_unresolved_and_uses_inert_setup_identity() {
        let temp = tempfile::tempdir().expect("temp");
        let config = resolve_production_adapter_config(
            "definitely-missing-pi-binary",
            temp.path().join("workspaces"),
            vec![ModelChoice {
                provider: "provider".to_string(),
                model: "model".to_string(),
            }],
            "medium",
            Duration::from_mins(5),
            Duration::from_mins(12),
            Duration::from_secs(15),
            Duration::from_secs(90),
            PiScanReservation {
                tokens: 10_000,
                cost_microusd: 1_000_000,
            },
            "",
            true,
        )
        .expect("dry-run config");
        assert!(!config.pi_executable.exists());
        let mut adapter = ProductionOrchestrationAdapter::new(config);
        let setup = adapter.dry_run_setup().expect("inert setup");
        assert_eq!(setup.pi_version, "dry-run-pi-not-launched");
        assert_eq!(setup.selected_provider, "provider");
        assert_eq!(setup.selected_model, "model");
    }

    /// Verify explicit proxy policy is credential-free HTTPS and retained exactly.
    #[test]
    fn production_proxy_policy_rejects_credentials_and_retains_explicit_https() {
        let temp = tempfile::tempdir().expect("temp");
        let build = |proxy: &str| {
            resolve_production_adapter_config(
                "unused-in-dry-run",
                temp.path().join("workspaces"),
                vec![ModelChoice {
                    provider: "provider".to_string(),
                    model: "model".to_string(),
                }],
                "medium",
                Duration::from_mins(5),
                Duration::from_mins(12),
                Duration::from_secs(15),
                Duration::from_secs(90),
                PiScanReservation {
                    tokens: 10_000,
                    cost_microusd: 1_000_000,
                },
                proxy,
                true,
            )
        };
        assert!(build("https://user:secret@proxy.example").is_err());
        assert!(build("http://proxy.example").is_err());
        let config = build("https://proxy.example:8443").expect("explicit proxy");
        assert_eq!(
            config.https_proxy.as_deref(),
            Some("https://proxy.example:8443")
        );
    }

    /// Verify the real installed Pi can complete the isolated production setup without a model call.
    #[test]
    #[ignore = "requires installed Pi >=0.84.0 and one configured local model route"]
    fn live_production_setup_probe_verifies_tools_models_and_pricing() {
        let temp = tempfile::tempdir().expect("temp");
        let config = resolve_production_adapter_config(
            "pi",
            temp.path().join("workspaces"),
            vec![ModelChoice {
                provider: "openai-codex".to_string(),
                model: "gpt-5.6-luna".to_string(),
            }],
            "medium",
            Duration::from_mins(5),
            Duration::from_mins(12),
            Duration::from_secs(15),
            Duration::from_secs(90),
            PiScanReservation {
                tokens: 10_000,
                cost_microusd: 1_000_000,
            },
            "",
            false,
        )
        .expect("installed tools");
        let mut adapter = ProductionOrchestrationAdapter::new(config);
        let setup = adapter.probe_setup().expect("isolated no-model setup");
        assert_eq!(setup.pi_version, "0.84.0");
        assert_eq!(setup.selected_provider, "openai-codex");
        assert_eq!(setup.selected_model, "gpt-5.6-luna");
        assert!(setup.reservation.cost_microusd <= 1_000_000);
    }
}
