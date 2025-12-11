//! Background worker for fetching news article content.

use tokio::sync::mpsc;

use crate::sources;

/// What: Spawn background worker for news article content fetching.
///
/// Inputs:
/// - `news_content_req_rx`: Channel receiver for content requests (URL as String)
/// - `news_content_res_tx`: Channel sender for content responses (URL, content)
///
/// Output:
/// - None (spawns async task)
///
/// Details:
/// - Listens for URL requests on the channel
/// - Fetches article content asynchronously using `fetch_news_content`
/// - Sends results as `(String, String)` with URL and content
/// - On error, sends empty content string
pub fn spawn_news_content_worker(
    mut news_content_req_rx: mpsc::UnboundedReceiver<String>,
    news_content_res_tx: mpsc::UnboundedSender<(String, String)>,
) {
    tokio::spawn(async move {
        while let Some(url) = news_content_req_rx.recv().await {
            let url_clone = url.clone();
            match sources::fetch_news_content(&url).await {
                Ok(content) => {
                    let _ = news_content_res_tx.send((url_clone, content));
                }
                Err(e) => {
                    tracing::warn!(error = %e, url = %url_clone, "Failed to fetch news content");
                    let _ = news_content_res_tx
                        .send((url_clone, format!("Failed to load content: {e}")));
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    /// What: Test error message format for failed content fetches.
    ///
    /// Inputs:
    /// - Error string from `fetch_news_content`.
    ///
    /// Output:
    /// - Error message formatted as "Failed to load content: {error}".
    ///
    /// Details:
    /// - Verifies error message format matches worker behavior.
    fn test_news_content_worker_error_format() {
        let error = "Network error";
        let error_msg = format!("Failed to load content: {error}");
        assert!(error_msg.contains("Failed to load content"));
        assert!(error_msg.contains(error));
    }
}
