# Pacsea Architecture Documentation

This document describes the current architecture of Pacsea and explores alternative architectural approaches that could be considered for future development.

**Last Updated:** 2025-12-01  
**Codebase Version:** feat/integrated-process branch

---

## Table of Contents

1. [Current Architecture Overview](#current-architecture-overview)
2. [Layer Breakdown](#layer-breakdown)
3. [Data Flow](#data-flow)
4. [Key Patterns Used](#key-patterns-used)
5. [Alternative Architectures](#alternative-architectures)
6. [Architecture Comparison](#architecture-comparison)
7. [Recommendations](#recommendations)

---

## Current Architecture Overview

Pacsea follows a **Message-Passing Actor-like Architecture** with a centralized event loop and background workers communicating via Tokio channels.

### High-Level Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              main.rs                                     │
│  - CLI argument parsing (clap)                                          │
│  - Logging initialization (tracing)                                     │
│  - Calls app::run()                                                     │
└───────────────────────────────┬─────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         app/runtime/                                     │
│                                                                          │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────────────┐ │
│  │   init.rs   │───▶│ event_loop  │◀───│  Background Workers         │ │
│  │ (startup)   │    │   .rs       │    │  (tokio::spawn tasks)       │ │
│  └─────────────┘    │             │    │                             │ │
│                     │  select! {  │    │  - search_worker            │ │
│                     │    event    │    │  - details_worker           │ │
│                     │    search   │    │  - preflight_workers (5)    │ │
│                     │    details  │    │  - executor_worker          │ │
│                     │    files    │◀──▶│  - pkgbuild_worker          │ │
│                     │    deps     │    │  - comments_worker          │ │
│                     │    ...      │    │  - auxiliary_worker         │ │
│                     │  }          │    │                             │ │
│                     └─────────────┘    └─────────────────────────────┘ │
│                           │                                             │
│                           ▼                                             │
│                    ┌─────────────┐                                      │
│                    │  AppState   │ (centralized mutable state)         │
│                    └─────────────┘                                      │
│                           │                                             │
│                           ▼                                             │
│                    ┌─────────────┐                                      │
│                    │   ui.rs     │ (ratatui rendering)                 │
│                    └─────────────┘                                      │
└─────────────────────────────────────────────────────────────────────────┘
```

### Core Components

| Component | Location | Responsibility |
|-----------|----------|----------------|
| **Entry Point** | `main.rs` | CLI parsing, logging setup, launches runtime |
| **Runtime** | `app/runtime/` | Event loop, worker coordination, initialization |
| **State** | `state/` | Centralized `AppState` struct, data types, modals |
| **Events** | `events/` | Keyboard/mouse input handling, modal handlers |
| **UI** | `ui/` | Ratatui-based rendering, widgets, layouts |
| **Logic** | `logic/` | Business logic (deps, files, sandbox, services) |
| **Sources** | `sources/` | Network data fetching (AUR, news, status) |
| **Install** | `install/` | Package installation/removal orchestration |
| **Index** | `index/` | Package index management and caching |
| **Theme** | `theme/` | Configuration loading, theming, keybinds |
| **i18n** | `i18n/` | Internationalization and locale detection |
| **Util** | `util/` | Shared utilities (curl, pacman, config parsing) |

---

## Layer Breakdown

### 1. Presentation Layer (`ui/`, `events/`)

**Purpose:** Handle user input and render the terminal UI.

```
ui/
├── modals/          # Modal dialog renderers
│   ├── preflight/   # Preflight modal (complex, multi-tab)
│   ├── help.rs      # Help overlay
│   ├── confirm.rs   # Confirmation dialogs
│   └── ...
├── details/         # Package details pane
├── middle/          # Middle pane (install/search lists)
├── results/         # Search results list
└── helpers/         # UI utilities and formatters

events/
├── search/          # Search-mode event handling
├── modals/          # Modal-specific event handling
├── preflight/       # Preflight modal events
├── mouse/           # Mouse event handling
└── global.rs        # Global keyboard shortcuts
```

**Key Characteristics:**
- Immediate-mode rendering (ratatui)
- Event handlers return `bool` (continue/exit)
- Modals are enum variants with associated data
- Keyboard navigation follows Vim conventions

### 2. Application Layer (`app/runtime/`, `state/`)

**Purpose:** Orchestrate the application lifecycle and manage state.

```
app/runtime/
├── event_loop.rs    # Main select! loop over channels
├── channels.rs      # Channel definitions and worker spawning
├── handlers/        # Result handlers for each worker type
├── workers/         # Background task implementations
├── init.rs          # Application initialization
├── tick_handler.rs  # Periodic task handler
└── cleanup.rs       # Shutdown and cleanup logic

state/
├── app_state/       # AppState struct and defaults
├── modal.rs         # Modal enum definitions
└── types.rs         # Shared type definitions
```

**Key Characteristics:**
- Single `AppState` struct (~600 lines) holds all application state
- Channels (`mpsc::UnboundedSender/Receiver`) for worker communication
- `select!` macro for concurrent channel polling
- Dirty flags for lazy persistence

### 3. Domain/Business Logic Layer (`logic/`)

**Purpose:** Implement package management business logic.

```
logic/
├── deps/            # Dependency resolution
├── files/           # File change detection
├── sandbox/         # AUR sandbox analysis
├── services/        # Systemd service impact
├── preflight/       # Pre-installation checks
└── ...
```

**Key Characteristics:**
- Pure functions where possible
- Synchronous logic wrapped in `spawn_blocking` for async context
- Trait-based abstractions (e.g., `CommandRunner`)

### 4. Data Access Layer (`sources/`, `index/`, `install/`)

**Purpose:** External data fetching, system interaction, and persistence.

```
sources/
├── search.rs        # AUR/official package search
├── details.rs       # Package details fetching
├── comments.rs      # AUR comments fetching
├── news.rs          # Arch news fetching
└── status/          # Arch status page parsing

index/
├── fetch.rs         # Index downloading
├── persist.rs       # Index serialization
├── query.rs         # Index querying
└── mirrors.rs       # Mirror list management

install/
├── executor.rs      # Shell command execution
├── batch.rs         # Batch installation
├── shell.rs         # Terminal spawning
└── scan/            # PKGBUILD scanning
```

**Key Characteristics:**
- `curl` via shell for HTTP requests (no reqwest dependency)
- JSON serialization with serde
- Pacman/paru/yay invocation via `Command`

---

## Data Flow

### Search Flow

```
User Input → events/search/ → query_tx channel → search_worker
                                                       │
                                                       ▼
                                              sources/search.rs
                                                       │
                                                       ▼
results_rx channel ← SearchResults ←──────────────────┘
       │
       ▼
handle_search_results() → AppState.results → UI re-render
```

### Preflight Installation Flow

```
User triggers install → preflight modal opened
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
   deps_worker          files_worker         services_worker
        │                     │                     │
        ▼                     ▼                     ▼
   deps_res_rx          files_res_rx        services_res_rx
        │                     │                     │
        └─────────────────────┼─────────────────────┘
                              ▼
                    Preflight modal tabs populated
                              │
                              ▼
                    User confirms installation
                              │
                              ▼
                    executor_worker → shell execution
                              │
                              ▼
                    post_summary_worker → result display
```

### State Persistence Flow

```
State Change → dirty flag set (e.g., cache_dirty = true)
                              │
                              ▼
              tick_handler (periodic) checks dirty flags
                              │
                              ▼
              maybe_flush_*() serializes to JSON
                              │
                              ▼
              ~/.config/pacsea/*.json written
```

---

## Key Patterns Used

### 1. Message-Passing / Actor-like Pattern

**Implementation:** Tokio `mpsc` unbounded channels connect the main event loop to background workers.

```rust
// Worker spawning
tokio::spawn(async move {
    while let Some(request) = req_rx.recv().await {
        let result = process_request(request);
        let _ = res_tx.send(result);
    }
});

// Event loop consumption
select! {
    Some(result) = res_rx.recv() => {
        handle_result(&mut app, result);
    }
    // ... other channels
}
```

**Pros:**
- Non-blocking UI during background operations
- Clear separation between request/response
- Easy to add new worker types

**Cons:**
- Channel proliferation (40+ channels currently)
- Complex flow tracing
- No built-in backpressure (unbounded channels)

### 2. Centralized Mutable State

**Implementation:** Single `AppState` struct passed via mutable reference.

```rust
pub struct AppState {
    pub input: String,
    pub results: Vec<PackageItem>,
    pub modal: Modal,
    // ... 100+ fields
}

fn handle_event(app: &mut AppState, ...) -> bool { ... }
```

**Pros:**
- Simple to understand
- Easy debugging (all state in one place)
- No synchronization overhead in single-threaded context

**Cons:**
- Large struct (~600 lines)
- All handlers see all state (no encapsulation)
- Testing requires full state setup

### 3. Immediate-Mode UI (Ratatui)

**Implementation:** UI rebuilds entire frame each render cycle.

```rust
loop {
    terminal.draw(|f| ui(f, &mut app))?;
    if process_events(&mut app).await { break; }
}
```

**Pros:**
- Simple mental model
- No UI state synchronization issues
- Great for terminal UIs

**Cons:**
- No differential updates
- Complex UI requires many function calls

### 4. Handler Trait Abstraction

**Implementation:** `HandlerConfig` trait for result handlers.

```rust
pub trait HandlerConfig {
    type Result: Clone;
    fn get_resolving(&self, app: &AppState) -> bool;
    fn update_cache(&self, app: &mut AppState, results: &[Self::Result]);
    // ...
}
```

**Pros:**
- Eliminates duplication in handler implementations
- Type-safe customization points
- Clear contract for new handlers
