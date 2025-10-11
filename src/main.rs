//! Pacsea binary entrypoint kept minimal. The full runtime lives in `app`.

mod app;
mod events;
mod index;
mod install;
mod logic;
mod sources;
mod state;
mod theme;
mod ui;
mod ui_helpers;
mod util;

#[tokio::main]
async fn main() {
    let dry_run_flag = std::env::args().any(|a| a == "--dry-run");
    if let Err(err) = app::run(dry_run_flag).await {
        eprintln!("Error: {err:?}");
    }
}
