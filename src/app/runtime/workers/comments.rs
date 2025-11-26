//! Background worker for fetching AUR package comments.

use tokio::sync::mpsc;

use crate::sources;
use crate::state::types::AurComment;

/// What: Spawn background worker for AUR comments fetching.
///
/// Inputs:
/// - `comments_req_rx`: Channel receiver for comments requests (package name as String)
/// - `comments_res_tx`: Channel sender for comments responses
///
/// Output:
/// - None (spawns async task)
///
/// Details:
/// - Listens for package name requests on the channel
/// - Fetches comments asynchronously using `fetch_aur_comments`
/// - Sends results as `(String, Result<Vec<AurComment>, String>)` matching PKGBUILD pattern
/// - Handles errors gracefully (sends error message instead of panicking)
pub fn spawn_comments_worker(
    mut comments_req_rx: mpsc::UnboundedReceiver<String>,
    comments_res_tx: mpsc::UnboundedSender<(String, Result<Vec<AurComment>, String>)>,
) {
    tokio::spawn(async move {
        while let Some(pkgname) = comments_req_rx.recv().await {
            let name = pkgname.clone();
            match sources::fetch_aur_comments(pkgname).await {
                Ok(comments) => {
                    let _ = comments_res_tx.send((name, Ok(comments)));
                }
                Err(e) => {
                    let _ =
                        comments_res_tx.send((name, Err(format!("Failed to fetch comments: {e}"))));
                }
            }
        }
    });
}
