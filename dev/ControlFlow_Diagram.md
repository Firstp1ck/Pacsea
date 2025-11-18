# Pacsea Application Control Flow Diagram

This diagram shows the complete control flow of the Pacsea TUI application, from startup to shutdown.

```mermaid
flowchart TD
    Start([main.rs: Application Start]) --> ParseArgs[Parse CLI Arguments]
    ParseArgs --> CheckFlags{Special Flags?}
    
    CheckFlags -->|--clear-cache| ClearCache[Clear Cache Files & Exit]
    CheckFlags -->|--dry-run| SetDryRun[Set Dry Run Flag]
    CheckFlags -->|Normal| InitLogging[Initialize Logging]
    
    SetDryRun --> InitLogging
    InitLogging --> AppRun[app::run]
    
    AppRun --> SetupTerm[Setup Terminal]
    SetupTerm --> InitState[Initialize App State]
    
    InitState --> LoadConfig[Migrate Legacy Configs<br/>Load Settings]
    LoadConfig --> LoadLocale[Initialize Locale System]
    LoadLocale --> LoadCaches[Load Persisted Caches:<br/>- Details Cache<br/>- Recent Queries<br/>- Install List<br/>- Dependency Cache<br/>- File Cache<br/>- Service Cache<br/>- Sandbox Cache<br/>- News Read URLs<br/>- Official Index]
    
    LoadCaches --> CheckCaches{Caches Valid?}
    CheckCaches -->|Missing/Invalid| SetInitFlags[Set Init Flags for<br/>Background Resolution]
    CheckCaches -->|Valid| CreateChannels[Create Channels]
    SetInitFlags --> CreateChannels
    
    CreateChannels --> SpawnWorkers[Spawn Background Workers:<br/>- Status Worker<br/>- News Worker<br/>- Tick Worker<br/>- Index Update Worker<br/>- Event Reading Thread]
    
    SpawnWorkers --> TriggerResolutions{Init Flags Set?}
    TriggerResolutions -->|Yes| SendResolutions[Send Resolution Requests:<br/>- Dependencies<br/>- Files<br/>- Services<br/>- Sandbox]
    TriggerResolutions -->|No| SendQuery
    SendResolutions --> SendQuery[Send Initial Query]
    
    SendQuery --> MainLoop[Main Event Loop<br/>tokio::select!]
    
    MainLoop --> RenderUI[Render UI Frame]
    RenderUI --> SelectEvents{Select on Channels}
    
    SelectEvents -->|Event Received| HandleEvent[events::handle_event]
    SelectEvents -->|Index Notify| UpdateIndex[Update Index State]
    SelectEvents -->|Search Results| HandleResults[Handle Search Results]
    SelectEvents -->|Details Update| HandleDetails[Handle Details Update]
    SelectEvents -->|Preview Update| HandlePreview[Handle Preview]
    SelectEvents -->|Add to Install| HandleAdd[Batch Add to Install List<br/>Trigger Resolutions]
    SelectEvents -->|Deps Result| HandleDeps[Handle Dependency Result]
    SelectEvents -->|Files Result| HandleFiles[Handle File Result]
    SelectEvents -->|Services Result| HandleServices[Handle Service Result]
    SelectEvents -->|Sandbox Result| HandleSandbox[Handle Sandbox Result]
    SelectEvents -->|PKGBUILD Result| HandlePKGBUILD[Handle PKGBUILD Result]
    SelectEvents -->|Summary Result| HandleSummary[Handle Summary Result]
    SelectEvents -->|Network Error| ShowAlert[Show Alert Modal]
    SelectEvents -->|Tick Event| HandleTick[Handle Tick Event]
    SelectEvents -->|News Update| HandleNews[Handle News Update]
    SelectEvents -->|Status Update| HandleStatus[Handle Status Update]
    
    HandleEvent --> CheckModal{Modal Active?}
    CheckModal -->|Yes| HandleModal[Handle Modal Events]
    CheckModal -->|No| CheckGlobal{Global Shortcut?}
    
    HandleModal --> CheckExit{Exit Requested?}
    CheckGlobal -->|Yes| HandleGlobal[Handle Global Shortcuts]
    CheckGlobal -->|No| HandlePane[Handle Pane-Specific Events:<br/>- Search Pane<br/>- Recent Pane<br/>- Install Pane]
    
    HandleGlobal --> CheckExit
    HandlePane --> CheckExit
    HandleModal --> CheckExit
    
    CheckExit -->|Yes| Shutdown
    CheckExit -->|No| RenderUI
    
    UpdateIndex --> RenderUI
    HandleResults --> RenderUI
    HandleDetails --> RenderUI
    HandlePreview --> RenderUI
    HandleAdd --> RenderUI
    HandleDeps --> RenderUI
    HandleFiles --> RenderUI
    HandleServices --> RenderUI
    HandleSandbox --> RenderUI
    HandlePKGBUILD --> RenderUI
    HandleSummary --> RenderUI
    ShowAlert --> RenderUI
    HandleNews --> RenderUI
    HandleStatus --> RenderUI
    
    HandleTick --> TickTasks[Periodic Tasks:<br/>- Flush Caches<br/>- Save Recent<br/>- Preflight Resolution<br/>- PKGBUILD Debounce<br/>- Poll Installed Cache<br/>- Ring Prefetch<br/>- Auto-close Menus<br/>- Expire Toasts]
    TickTasks --> RenderUI
    
    Shutdown[Shutdown Sequence] --> ResetFlags[Reset Resolution Flags]
    ResetFlags --> SignalEventThread[Signal Event Thread to Exit]
    SignalEventThread --> FlushAll[Flush All Caches:<br/>- Details Cache<br/>- Recent Queries<br/>- Install List<br/>- News Read URLs<br/>- Dependency Cache<br/>- File Cache<br/>- Service Cache<br/>- Sandbox Cache]
    FlushAll --> RestoreTerm[Restore Terminal]
    RestoreTerm --> End([Exit])
    
    ClearCache --> End
    
    style Start fill:#e1f5ff
    style AppRun fill:#fff4e1
    style MainLoop fill:#ffe1f5
    style Shutdown fill:#e1ffe1
    style End fill:#ffe1e1
```

## Key Components

### 1. Initialization Phase
- **CLI Argument Parsing**: Handles command-line flags (--dry-run, --clear-cache, etc.)
- **Logging Setup**: Initializes tracing logger to file
- **Terminal Setup**: Configures terminal for TUI mode
- **State Initialization**: Loads settings, caches, locale system
- **Channel Creation**: Sets up async communication channels
- **Worker Spawning**: Launches background workers for async operations

### 2. Main Event Loop
The application uses `tokio::select!` to concurrently handle multiple async channels:
- **User Input**: Keyboard and mouse events
- **Search Results**: Package search results from AUR/official repos
- **Details Updates**: Package information updates
- **Analysis Results**: Dependency, file, service, and sandbox analysis
- **PKGBUILD Content**: Package build file content
- **Preflight Summary**: Installation preflight analysis results
- **News/Status**: Arch Linux news and status updates
- **Tick Events**: Periodic background tasks

### 3. Event Handling
Events are processed in priority order:
1. **Modal Interactions**: Active modal dialogs (handled first)
2. **Global Shortcuts**: Application-wide shortcuts (help, exit, theme reload)
3. **Pane-Specific Events**: Search, Recent, and Install pane interactions

### 4. Background Workers
Asynchronous workers handle:
- **Search Worker**: AUR and official repository package search
- **Details Worker**: Package information retrieval
- **Dependency Worker**: Dependency resolution and analysis
- **File Worker**: File system impact analysis
- **Service Worker**: Systemd service impact analysis
- **Sandbox Worker**: AUR package sandbox analysis
- **News Worker**: Arch Linux news fetching
- **Status Worker**: Arch status page monitoring
- **Index Worker**: Official package index updates

### 5. Tick Handler (Periodic Tasks)
The tick handler performs periodic maintenance:
- **Cache Persistence**: Debounced writes of dirty caches
- **Preflight Resolution**: Processes queued preflight analysis requests
- **PKGBUILD Debouncing**: Manages PKGBUILD reload requests
- **Installed Cache Polling**: Refreshes installed package cache after installs/removals
- **Ring Prefetch**: Prefetches details for packages around selection
- **UI State Cleanup**: Auto-closes menus and expires toast messages

### 6. Shutdown Sequence
Graceful shutdown process:
- Reset all resolution flags
- Signal background threads to exit
- Flush all pending cache writes
- Restore terminal to original state

## Architecture Notes

- **Async Architecture**: Uses Tokio for async runtime with channels for communication
- **Event-Driven**: Main loop responds to events from multiple sources
- **Background Processing**: Heavy I/O operations run in background workers
- **State Management**: Centralized `AppState` holds all application state
- **Cache Strategy**: Multiple caches with signature-based validation
- **Debouncing**: Used for cache writes and PKGBUILD reloads to reduce I/O

## Converting to Image

To convert this Mermaid diagram to a PNG image, you can use:

1. **Mermaid CLI**: `mmdc -i ControlFlow_Diagram.md -o ControlFlow_Diagram.png`
2. **Online Tools**: Paste the mermaid code block into https://mermaid.live/
3. **VS Code Extension**: Use the "Markdown Preview Mermaid Support" extension
4. **GitHub/GitLab**: The diagram will render automatically in markdown files

