use tokio::sync::mpsc;

use crate::state::PackageItem;

/// What: Spawn background worker for dependency resolution.
///
/// Inputs:
/// - `deps_req_rx`: Channel receiver for dependency resolution requests
/// - `deps_res_tx`: Channel sender for dependency resolution responses
///
/// Details:
/// - Runs blocking dependency resolution in a thread pool
/// - Always sends a result, even if the task panics, to ensure flags are reset
pub fn spawn_dependency_worker(
    mut deps_req_rx: mpsc::UnboundedReceiver<Vec<PackageItem>>,
    deps_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::DependencyInfo>>,
) {
    let deps_res_tx_bg = deps_res_tx;
    tokio::spawn(async move {
        while let Some(items) = deps_req_rx.recv().await {
            // Run blocking dependency resolution in a thread pool
            let items_clone = items.clone();
            let res_tx = deps_res_tx_bg.clone();
            let res_tx_error = deps_res_tx_bg.clone(); // Clone for error handling
            let handle = tokio::task::spawn_blocking(move || {
                let deps = crate::logic::deps::resolve_dependencies(&items_clone);
                let _ = res_tx.send(deps);
            });
            // CRITICAL: Always await and send a result, even if task panics
            // This ensures deps_resolving flag is always reset
            tokio::spawn(async move {
                match handle.await {
                    Ok(_) => {
                        // Task completed successfully, result already sent
                        tracing::debug!("[Runtime] Dependency resolution task completed");
                    }
                    Err(e) => {
                        // Task panicked - send empty result to reset flag
                        tracing::error!("[Runtime] Dependency resolution task panicked: {:?}", e);
                        let _ = res_tx_error.send(Vec::new());
                    }
                }
            });
        }
        tracing::debug!("[Runtime] Dependency resolution worker exiting (channel closed)");
    });
}

/// What: Spawn background worker for file resolution.
///
/// Inputs:
/// - `files_req_rx`: Channel receiver for file resolution requests
/// - `files_res_tx`: Channel sender for file resolution responses
pub fn spawn_file_worker(
    mut files_req_rx: mpsc::UnboundedReceiver<Vec<PackageItem>>,
    files_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::PackageFileInfo>>,
) {
    let files_res_tx_bg = files_res_tx;
    tokio::spawn(async move {
        while let Some(items) = files_req_rx.recv().await {
            // Run blocking file resolution in a thread pool
            let items_clone = items.clone();
            let res_tx = files_res_tx_bg.clone();
            tokio::task::spawn_blocking(move || {
                let files = crate::logic::files::resolve_file_changes(
                    &items_clone,
                    crate::state::modal::PreflightAction::Install,
                );
                tracing::debug!(
                    "[Background] Sending file result: {} entries for packages: {:?}",
                    files.len(),
                    files.iter().map(|f| &f.name).collect::<Vec<_>>()
                );
                for file_info in &files {
                    tracing::debug!(
                        "[Background] Package '{}' - total={}, new={}, changed={}, removed={}, config={}",
                        file_info.name,
                        file_info.total_count,
                        file_info.new_count,
                        file_info.changed_count,
                        file_info.removed_count,
                        file_info.config_count
                    );
                }
                if let Err(e) = res_tx.send(files) {
                    tracing::error!("[Background] Failed to send file result: {}", e);
                } else {
                    tracing::debug!("[Background] Successfully sent file result");
                }
            });
        }
    });
}

/// What: Spawn background worker for service impact resolution.
///
/// Inputs:
/// - `services_req_rx`: Channel receiver for service resolution requests
/// - `services_res_tx`: Channel sender for service resolution responses
pub fn spawn_service_worker(
    mut services_req_rx: mpsc::UnboundedReceiver<Vec<PackageItem>>,
    services_res_tx: mpsc::UnboundedSender<Vec<crate::state::modal::ServiceImpact>>,
) {
    let services_res_tx_bg = services_res_tx;
    tokio::spawn(async move {
        while let Some(items) = services_req_rx.recv().await {
            // Run blocking service resolution in a thread pool
            let items_clone = items.clone();
            let res_tx = services_res_tx_bg.clone();
            tokio::task::spawn_blocking(move || {
                let services = crate::logic::services::resolve_service_impacts(
                    &items_clone,
                    crate::state::modal::PreflightAction::Install,
                );
                tracing::debug!(
                    "[Background] Sending service result: {} entries",
                    services.len()
                );
                if let Err(e) = res_tx.send(services) {
                    tracing::error!("[Background] Failed to send service result: {}", e);
                } else {
                    tracing::debug!("[Background] Successfully sent service result");
                }
            });
        }
    });
}

/// What: Spawn background worker for sandbox resolution.
///
/// Inputs:
/// - `sandbox_req_rx`: Channel receiver for sandbox resolution requests
/// - `sandbox_res_tx`: Channel sender for sandbox resolution responses
///
/// Details:
/// - Uses async version for parallel HTTP fetches
pub fn spawn_sandbox_worker(
    mut sandbox_req_rx: mpsc::UnboundedReceiver<Vec<PackageItem>>,
    sandbox_res_tx: mpsc::UnboundedSender<Vec<crate::logic::sandbox::SandboxInfo>>,
) {
    let sandbox_res_tx_bg = sandbox_res_tx;
    tokio::spawn(async move {
        while let Some(items) = sandbox_req_rx.recv().await {
            // Use async version for parallel HTTP fetches
            let items_clone = items.clone();
            let res_tx = sandbox_res_tx_bg.clone();
            tokio::spawn(async move {
                let sandbox_info =
                    crate::logic::sandbox::resolve_sandbox_info_async(&items_clone).await;
                tracing::debug!(
                    "[Background] Sending sandbox result: {} entries for packages: {:?}",
                    sandbox_info.len(),
                    sandbox_info
                        .iter()
                        .map(|s| &s.package_name)
                        .collect::<Vec<_>>()
                );
                if let Err(e) = res_tx.send(sandbox_info) {
                    tracing::error!("[Background] Failed to send sandbox result: {}", e);
                } else {
                    tracing::debug!("[Background] Successfully sent sandbox result");
                }
            });
        }
    });
}

/// What: Spawn background worker for preflight summary computation.
///
/// Inputs:
/// - `summary_req_rx`: Channel receiver for summary computation requests
/// - `summary_res_tx`: Channel sender for summary computation responses
///
/// Details:
/// - Runs blocking summary computation in a thread pool
/// - Always sends a result, even if the task panics, to avoid breaking the UI
pub fn spawn_summary_worker(
    mut summary_req_rx: mpsc::UnboundedReceiver<(
        Vec<PackageItem>,
        crate::state::modal::PreflightAction,
    )>,
    summary_res_tx: mpsc::UnboundedSender<crate::logic::preflight::PreflightSummaryOutcome>,
) {
    let summary_res_tx_bg = summary_res_tx;
    tokio::spawn(async move {
        while let Some((items, action)) = summary_req_rx.recv().await {
            // Run blocking summary computation in a thread pool
            let items_clone = items.clone();
            let res_tx = summary_res_tx_bg.clone();
            let res_tx_error = summary_res_tx_bg.clone();
            let handle = tokio::task::spawn_blocking(move || {
                let summary =
                    crate::logic::preflight::compute_preflight_summary(&items_clone, action);
                let _ = res_tx.send(summary);
            });
            // CRITICAL: Always await and send a result, even if task panics
            tokio::spawn(async move {
                match handle.await {
                    Ok(_) => {
                        // Task completed successfully, result already sent
                        tracing::debug!("[Runtime] Preflight summary computation task completed");
                    }
                    Err(e) => {
                        // Task panicked - send minimal result to reset flag
                        tracing::error!(
                            "[Runtime] Preflight summary computation task panicked: {:?}",
                            e
                        );
                        // Create a minimal summary to avoid breaking the UI
                        let minimal_summary = crate::logic::preflight::PreflightSummaryOutcome {
                            summary: crate::state::modal::PreflightSummaryData {
                                packages: Vec::new(),
                                package_count: 0,
                                aur_count: 0,
                                download_bytes: 0,
                                install_delta_bytes: 0,
                                risk_score: 0,
                                risk_level: crate::state::modal::RiskLevel::Low,
                                risk_reasons: Vec::new(),
                                major_bump_packages: Vec::new(),
                                core_system_updates: Vec::new(),
                                pacnew_candidates: 0,
                                pacsave_candidates: 0,
                                config_warning_packages: Vec::new(),
                                service_restart_units: Vec::new(),
                                summary_warnings: vec!["Summary computation failed".to_string()],
                                summary_notes: Vec::new(),
                            },
                            header: crate::state::modal::PreflightHeaderChips {
                                package_count: 0,
                                download_bytes: 0,
                                install_delta_bytes: 0,
                                aur_count: 0,
                                risk_score: 0,
                                risk_level: crate::state::modal::RiskLevel::Low,
                            },
                        };
                        let _ = res_tx_error.send(minimal_summary);
                    }
                }
            });
        }
        tracing::debug!("[Runtime] Preflight summary computation worker exiting (channel closed)");
    });
}
