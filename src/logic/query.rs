use tokio::sync::mpsc;

use crate::state::AppState;

/// What: Send the current query text over the search channel with a fresh id.
///
/// Inputs:
/// - `app`: Mutable application state; updates `next_query_id` and `latest_query_id`
/// - `query_tx`: Channel to send the `QueryInput`
///
/// Output:
/// - Sends a `QueryInput` with incremented id and current text; updates ids in `app`.
///
/// Details:
/// - The id allows correlating responses so the UI can discard stale results.
pub fn send_query(app: &mut AppState, query_tx: &mpsc::UnboundedSender<crate::state::QueryInput>) {
    let id = app.next_query_id;
    app.next_query_id += 1;
    app.latest_query_id = id;
    let _ = query_tx.send(crate::state::QueryInput {
        id,
        text: app.input.clone(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    /// What: Ensure `send_query` increments identifiers and forwards the current input text.
    ///
    /// Inputs:
    /// - `AppState` whose `input` is set to `"hello"`.
    ///
    /// Output:
    /// - `latest_query_id` advances to `1` and the channel receives a matching `QueryInput`.
    ///
    /// Details:
    /// - Uses a short timeout to guarantee the send occurs asynchronously.
    async fn send_query_increments_and_sends() {
        let mut app = AppState {
            input: "hello".into(),
            ..Default::default()
        };
        let (tx, mut rx) = mpsc::unbounded_channel();
        send_query(&mut app, &tx);
        assert_eq!(app.latest_query_id, 1);
        let q = tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv())
            .await
            .ok()
            .flatten()
            .expect("query sent");
        assert_eq!(q.id, app.latest_query_id);
        assert_eq!(q.text, "hello");
    }
}
