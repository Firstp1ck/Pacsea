use tokio::sync::mpsc;

use crate::state::AppState;

/// Send the current query text over the search channel with a fresh id.
///
/// Side effects on `app`:
/// - Increments and records `next_query_id`
/// - Updates `latest_query_id` to the id sent
///
/// The id allows the receiver to tag results so the UI can discard any stale
/// responses that arrive out of order.
pub fn send_query(app: &mut AppState, query_tx: &mpsc::UnboundedSender<crate::state::QueryInput>) {
    let id = app.next_query_id;
    app.next_query_id += 1;
    app.latest_query_id = id;
    let _ = query_tx.send(crate::state::QueryInput {
        id,
        text: app.input.clone(),
    });
}
