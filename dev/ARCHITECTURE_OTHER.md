## Alternative Architectures

### Alternative 1: Elm Architecture (TEA)

**Overview:** Unidirectional data flow with pure update functions.

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│    ┌─────────┐      ┌─────────┐      ┌─────────┐          │
│    │  Model  │──────│ Update  │──────│  View   │          │
│    │ (State) │      │ (Pure)  │      │ (Pure)  │          │
│    └────┬────┘      └────┬────┘      └────┬────┘          │
│         │                │                │                │
│         └────────────────┴────────────────┘                │
│                          │                                 │
│                          ▼                                 │
│                    ┌─────────┐                            │
│                    │ Message │                            │
│                    │  Queue  │                            │
│                    └─────────┘                            │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

**Implementation Sketch:**

```rust
enum Msg {
    SearchInput(String),
    SearchResults(Vec<PackageItem>),
    KeyPress(KeyCode),
    DepsResolved(Vec<DependencyInfo>),
    // ...
}

fn update(model: &mut Model, msg: Msg) -> Vec<Cmd> {
    match msg {
        Msg::SearchInput(s) => {
            model.input = s.clone();
            vec![Cmd::Search(s)]
        }
        Msg::SearchResults(results) => {
            model.results = results;
            vec![]
        }
        // ...
    }
}

fn view(model: &Model, frame: &mut Frame) { /* render */ }
```

**Pros:**
- Pure update functions (highly testable)
- Predictable state transitions
- Time-travel debugging possible

**Cons:**
- Boilerplate for message types
- Indirect control flow
- May not fit Rust idioms as naturally

**Suitability for Pacsea:** ⭐⭐⭐ (Good)
- Would improve testability significantly
- Requires major refactoring

---

### Alternative 2: Component-Based Architecture

**Overview:** Decompose UI into self-contained components with local state.

```
┌─────────────────────────────────────────────────────────────┐
│                        App                                  │
│  ┌────────────────────────────────────────────────────────┐│
│  │                    SearchPane                          ││
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ ││
│  │  │ SearchInput  │  │ ResultsList  │  │ PackageCard  │ ││
│  │  │ (local state)│  │ (local state)│  │ (local state)│ ││
│  │  └──────────────┘  └──────────────┘  └──────────────┘ ││
│  └────────────────────────────────────────────────────────┘│
│  ┌────────────────────────────────────────────────────────┐│
│  │                   InstallPane                          ││
│  │  ┌──────────────┐  ┌──────────────┐                   ││
│  │  │ PackageList  │  │ ActionBar    │                   ││
│  │  └──────────────┘  └──────────────┘                   ││
│  └────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

**Implementation Sketch:**

```rust
trait Component {
    type Props;
    type State;
    type Message;
    
    fn new(props: Self::Props) -> Self;
    fn update(&mut self, msg: Self::Message);
    fn render(&self, frame: &mut Frame, area: Rect);
}

struct SearchInput {
    value: String,
    cursor: usize,
}

impl Component for SearchInput {
    type Props = ();
    type State = (String, usize);
    type Message = SearchInputMsg;
    
    fn update(&mut self, msg: Self::Message) {
        match msg {
            SearchInputMsg::Char(c) => self.value.push(c),
            SearchInputMsg::Backspace => { self.value.pop(); }
        }
    }
    // ...
}
```

**Pros:**
- Encapsulated state per component
- Reusable UI components
- Easier to reason about individual parts

**Cons:**
- Cross-component communication complexity
- Overhead for simple components
- Less common pattern in Rust TUI ecosystem

**Suitability for Pacsea:** ⭐⭐ (Moderate)
- Would require significant framework development
- May add unnecessary complexity for a TUI

---

### Alternative 3: Command Pattern / CQRS-lite

**Overview:** Separate command execution from state queries with explicit command objects.

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│  Events ──▶ Commands ──▶ CommandHandler ──▶ State          │
│                              │                              │
│                              ▼                              │
│                         SideEffects                         │
│                     (spawn workers, IO)                     │
│                                                             │
│  Queries ◀── State ◀── QueryHandler ◀── UI                 │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Implementation Sketch:**

```rust
enum Command {
    Search { query: String },
    AddToInstallList { package: PackageItem },
    StartPreflight { packages: Vec<PackageItem> },
    ExecuteInstall { dry_run: bool },
}

struct CommandHandler {
    state: AppState,
    worker_channels: Channels,
}

impl CommandHandler {
    fn execute(&mut self, cmd: Command) -> Vec<SideEffect> {
        match cmd {
            Command::Search { query } => {
                self.state.input = query.clone();
                vec![SideEffect::SpawnSearch(query)]
            }
            Command::AddToInstallList { package } => {
                self.state.install_list.push(package);
                self.state.install_dirty = true;
                vec![]
            }
            // ...
        }
    }
}
```

**Pros:**
- Explicit, auditable actions
- Easy to add undo/redo
- Clear separation of concerns
- Commands can be serialized (macros, scripting)

**Cons:**
- Additional indirection
- More boilerplate
- May overcomplicate simple actions

**Suitability for Pacsea:** ⭐⭐⭐⭐ (Very Good)
- Aligns well with existing channel-based architecture
- Would improve testability
- Could enable scripting/macro recording

---

### Alternative 4: Redux-like Store

**Overview:** Single immutable state tree with reducer functions and middleware.

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│  Action ──▶ Middleware ──▶ Reducer ──▶ New State           │
│                │                            │               │
│                ▼                            ▼               │
│          Side Effects                   Subscribers         │
│         (async workers)                   (UI)             │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Implementation Sketch:**

```rust
#[derive(Clone)]
struct State {
    input: String,
    results: Vec<PackageItem>,
    modal: Modal,
    // ... immutable
}

enum Action {
    SetInput(String),
    SetResults(Vec<PackageItem>),
    OpenModal(Modal),
}

fn reducer(state: State, action: Action) -> State {
    match action {
        Action::SetInput(s) => State { input: s, ..state },
        Action::SetResults(r) => State { results: r, ..state },
        Action::OpenModal(m) => State { modal: m, ..state },
    }
}

// Middleware for side effects
fn search_middleware(store: &Store, action: &Action) {
    if let Action::SetInput(query) = action {
        store.dispatch_async(async move {
            let results = search(query).await;
            Action::SetResults(results)
        });
    }
}
```

**Pros:**
- Predictable state management
- Time-travel debugging
- Middleware for cross-cutting concerns
- Immutable state (thread-safe)

**Cons:**
- Clone overhead for large state
- Verbose action definitions
- May fight Rust's ownership model

**Suitability for Pacsea:** ⭐⭐ (Moderate)
- Immutable state cloning could be expensive
- Benefits may not outweigh costs for TUI

---

### Alternative 5: Event Sourcing

**Overview:** Persist state changes as a sequence of events.

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│  Commands ──▶ Event Store ──▶ Projections ──▶ Read Model   │
│                    │                                        │
│                    ▼                                        │
│              [PackageAdded,                                 │
│               SearchPerformed,                              │
│               InstallStarted,                               │
│               ...]                                          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Pros:**
- Complete audit trail
- Rebuild state from events
- Easy temporal queries

**Cons:**
- Overkill for a TUI application
- Storage overhead
- Complexity

**Suitability for Pacsea:** ⭐ (Low)
- Benefits don't justify complexity
- Not appropriate for this use case

---

## Architecture Comparison

| Criteria | Current | TEA | Component | Command/CQRS | Redux |
|----------|---------|-----|-----------|--------------|-------|
| **Testability** | ⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| **Simplicity** | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ | ⭐⭐ |
| **Rust Idiomatic** | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐ |
| **Refactor Effort** | N/A | High | Very High | Medium | High |
| **Extensibility** | ⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ |
| **Performance** | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ |

---

## Recommendations

### Short-Term (Current Architecture Improvements)

1. **Split `AppState`** into domain-specific substates:
   ```rust
   struct AppState {
       search: SearchState,
       install: InstallState,
       preflight: PreflightState,
       ui: UiState,
   }
   ```

2. **Reduce Channel Sprawl** by grouping related channels:
   ```rust
   struct PreflightChannels {
       deps: (Sender, Receiver),
       files: (Sender, Receiver),
       services: (Sender, Receiver),
       sandbox: (Sender, Receiver),
   }
   ```

3. **Extract Pure Business Logic** from handlers into separate modules.

### Medium-Term (Incremental Architecture Evolution)

1. **Introduce Command Pattern** for user actions:
   - Define explicit `Command` enum
   - Route all state mutations through commands
   - Enables undo/redo, macro recording

2. **Add Query Layer** for state access:
   - Computed properties as methods
   - Memoization where beneficial
   - Cleaner separation from mutation

### Long-Term (Major Refactor Options)

If a major refactor is warranted:

1. **TEA (Elm Architecture)** - Best for testability and predictability
2. **Command/CQRS-lite** - Best for extensibility and scripting support

### Not Recommended

- **Event Sourcing** - Overkill for this application
- **Full Redux** - Immutable cloning overhead not justified
- **Component Architecture** - Would require custom framework

---

## Conclusion

The current architecture is **pragmatic and well-suited** for a terminal-based package manager:

- Message-passing provides good async handling
- Centralized state is simple and debuggable
- Ratatui immediate-mode UI works well

**Primary improvement opportunities:**
1. Split `AppState` for better organization
2. Introduce Command pattern for better testability
3. Reduce channel complexity through grouping

The architecture serves its purpose well. Incremental improvements (Command pattern, state splitting) would provide the most benefit with the least disruption.

