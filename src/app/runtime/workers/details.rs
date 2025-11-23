use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};

use crate::sources;
use crate::sources::fetch_details;
use crate::state::{PackageDetails, PackageItem, Source};

/// What: Spawn background worker for batched package details fetching.
///
/// Inputs:
/// - `net_err_tx`: Channel sender for network errors
/// - `details_req_rx`: Channel receiver for detail requests
/// - `details_res_tx`: Channel sender for detail responses
///
/// Details:
/// - Batches requests within a 120ms window to reduce network calls
/// - Deduplicates requests by package name
/// - Filters out disallowed packages
pub fn spawn_details_worker(
    net_err_tx: &mpsc::UnboundedSender<String>,
    mut details_req_rx: mpsc::UnboundedReceiver<PackageItem>,
    details_res_tx: mpsc::UnboundedSender<PackageDetails>,
) {
    use std::collections::HashSet;
    let net_err_tx_details = net_err_tx.clone();
    tokio::spawn(async move {
        const DETAILS_BATCH_WINDOW_MS: u64 = 120;
        loop {
            let Some(first) = details_req_rx.recv().await else {
                break;
            };
            let mut batch: Vec<PackageItem> = vec![first];
            loop {
                tokio::select! {
                    Some(next) = details_req_rx.recv() => { batch.push(next); }
                    () = sleep(Duration::from_millis(DETAILS_BATCH_WINDOW_MS)) => { break; }
                }
            }
            let mut seen: HashSet<String> = HashSet::new();
            let mut ordered: Vec<PackageItem> = Vec::with_capacity(batch.len());
            for it in batch {
                if seen.insert(it.name.clone()) {
                    ordered.push(it);
                }
            }
            for it in ordered {
                if !crate::logic::is_allowed(&it.name) {
                    continue;
                }
                match fetch_details(it.clone()).await {
                    Ok(details) => {
                        let _ = details_res_tx.send(details);
                    }
                    Err(e) => {
                        let msg = match it.source {
                            Source::Official { .. } => format!(
                                "Official package details unavailable for {}: {}",
                                it.name, e
                            ),
                            Source::Aur => {
                                format!("AUR package details unavailable for {}: {e}", it.name)
                            }
                        };
                        let _ = net_err_tx_details.send(msg);
                    }
                }
            }
        }
    });
}

/// What: Spawn background worker for PKGBUILD fetching.
///
/// Inputs:
/// - `pkgb_req_rx`: Channel receiver for PKGBUILD requests
/// - `pkgb_res_tx`: Channel sender for PKGBUILD responses
pub fn spawn_pkgbuild_worker(
    mut pkgb_req_rx: mpsc::UnboundedReceiver<PackageItem>,
    pkgb_res_tx: mpsc::UnboundedSender<(String, String)>,
) {
    tokio::spawn(async move {
        while let Some(item) = pkgb_req_rx.recv().await {
            let name = item.name.clone();
            match sources::fetch_pkgbuild_fast(&item).await {
                Ok(txt) => {
                    let _ = pkgb_res_tx.send((name, txt));
                }
                Err(e) => {
                    let _ = pkgb_res_tx.send((name, format!("Failed to fetch PKGBUILD: {e}")));
                }
            }
        }
    });
}
