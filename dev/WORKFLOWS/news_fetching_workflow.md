# News Fetching Workflow

This document describes the complete news fetching workflow in Pacsea, including startup news fetch, aggregated news feed fetch, and news content fetching.

## Overview

Pacsea fetches news from multiple sources:
- **Arch News**: Official Arch Linux news feed from `archlinux.org/news/feed`
- **Security Advisories**: Security advisories from `security.archlinux.org`
- **Package Updates**: Updates for installed packages from `archlinux.org/packages/`
- **AUR Comments**: Comments on AUR packages from `aur.archlinux.org`

The system uses coordination mechanisms to prevent concurrent requests to `archlinux.org` which can cause rate limiting or blocking.

## Main Workflow Diagram

```mermaid
graph TB
    Start([App Startup]) --> Init[Initialize Auxiliary Workers]
    Init --> Check{Startup News<br/>Configured?}
    
    Check -->|No| Skip[Skip News Fetching]
    Check -->|Yes| CreateChannel[Create Oneshot Channel<br/>for Coordination]
    
    CreateChannel --> StartupWorker[Spawn Startup News Worker]
    CreateChannel --> AggWorker[Spawn Aggregated Feed Worker]
    
    StartupWorker --> StartupJitter[Random Jitter<br/>0-500ms]
    StartupJitter --> StartupFetch[Fetch Startup News Feed]
    
    AggWorker --> WaitForStartup[Wait for Startup<br/>Completion Signal]
    WaitForStartup --> AggDelay[Additional Delay<br/>500-1500ms]
    AggDelay --> AggFetch[Fetch Aggregated News Feed]
    
    StartupFetch --> FilterStartup[Filter by Source/Age/Read]
    FilterStartup --> SendStartup[Send to News Channel]
    SendStartup --> SignalComplete[Send Completion Signal]
    
    SignalComplete -.->|Unblocks| WaitForStartup
    
    AggFetch --> SendAgg[Send to News Feed Channel]
    
    SendStartup --> UIUpdate1[Update UI with<br/>Startup News]
    SendAgg --> UIUpdate2[Update UI with<br/>Full News Feed]
    
    UIUpdate1 --> End([End])
    UIUpdate2 --> End
    Skip --> End
```

## Startup News Fetch Workflow

```mermaid
sequenceDiagram
    participant App as App Startup
    participant Worker as Startup News Worker
    participant Feed as fetch_news_feed
    participant Arch as archlinux.org
    participant Security as security.archlinux.org
    participant AUR as aur.archlinux.org
    participant Channel as News Channel
    
    App->>Worker: Spawn with completion_tx
    Worker->>Worker: Random jitter (0-500ms)
    Worker->>Worker: Optimize max_age based on last_startup
    Worker->>Worker: Ensure installed packages set
    
    Worker->>Feed: fetch_news_feed(ctx)
    
    Note over Feed: Sequential fetch for archlinux.org sources
    Feed->>Arch: Fetch Arch News Feed
    Arch-->>Feed: News items
    Feed->>Security: Fetch Security Advisories
    Security-->>Feed: Advisory items
    
    Note over Feed: Parallel fetch for other sources
    par Package Updates
        Feed->>Arch: Fetch package update info
        Arch-->>Feed: Update items
    and AUR Comments
        Feed->>AUR: Fetch AUR comments
        AUR-->>Feed: Comment items
    end
    
    Feed-->>Worker: Combined news items
    Worker->>Worker: Filter by source preferences
    Worker->>Worker: Filter by max_age_days
    Worker->>Worker: Filter unread items
    Worker->>Channel: Send filtered items
    Worker->>App: Send completion signal
    Channel->>App: Update UI with news
```

## Aggregated News Feed Fetch Workflow

```mermaid
sequenceDiagram
    participant App as App Startup
    participant Worker as Aggregated Feed Worker
    participant Signal as Completion Signal
    participant Feed as fetch_news_feed
    participant Arch as archlinux.org
    participant Security as security.archlinux.org
    participant AUR as aur.archlinux.org
    participant Channel as News Feed Channel
    
    App->>Worker: Spawn with completion_rx
    Worker->>Signal: Wait for startup completion
    Note over Worker,Signal: Blocks until startup fetch completes
    Signal-->>Worker: Startup fetch completed
    Worker->>Worker: Additional delay (500-1500ms)
    Worker->>Worker: Ensure installed packages set
    
    Worker->>Feed: fetch_news_feed(ctx)
    
    Note over Feed: Sequential fetch for archlinux.org sources
    Feed->>Arch: Fetch Arch News Feed
    Arch-->>Feed: News items
    Feed->>Security: Fetch Security Advisories
    Security-->>Feed: Advisory items
    
    Note over Feed: Parallel fetch for other sources
    par Package Updates
        Feed->>Arch: Fetch package update info
        Arch-->>Feed: Update items
    and AUR Comments
        Feed->>AUR: Fetch AUR comments
        AUR-->>Feed: Comment items
    end
    
    Feed-->>Worker: Combined news items
    Worker->>Channel: Send full feed payload
    Channel->>App: Update UI with full feed
```

## News Content Fetching Workflow

```mermaid
sequenceDiagram
    participant UI as User Interface
    participant Event as Event Handler
    participant Worker as News Content Worker
    participant Cache as Content Cache
    participant RateLimit as Rate Limiter
    participant Arch as archlinux.org
    participant AUR as aur.archlinux.org
    
    UI->>Event: User selects news item
    Event->>Event: Debounce timer (prevents rapid requests)
    Event->>Worker: Send URL request
    
    Worker->>Worker: Drain stale requests<br/>(keep most recent)
    
    alt URL is AUR package
        Worker->>AUR: Fetch AUR comments
        AUR-->>Worker: Comments HTML
        Worker->>Worker: Render comments
    else URL is Arch news/article
        Worker->>Cache: Check in-memory cache (15min TTL)
        alt Cache Hit
            Cache-->>Worker: Cached content
        else Cache Miss
            Worker->>Cache: Check disk cache (14day TTL)
            alt Disk Cache Hit
                Cache-->>Worker: Cached content
                Worker->>Cache: Populate in-memory cache
            else Disk Cache Miss
                Worker->>RateLimit: Check circuit breaker
                alt Circuit Breaker Open
                    RateLimit-->>Worker: Error (use stale cache if available)
                else Circuit Breaker Closed
                    Worker->>RateLimit: Acquire rate limit permit
                    RateLimit-->>Worker: Permit acquired
                    Worker->>Arch: Fetch article content
                    Arch-->>Worker: HTML content
                    Worker->>Worker: Parse and extract content
                    Worker->>Cache: Store in memory cache
                    Worker->>Cache: Store in disk cache
                end
            end
        end
    end
    
    Worker->>UI: Send content
    UI->>UI: Display article/comments
```

## Coordination Mechanism

The coordination between startup and aggregated news fetches prevents concurrent requests to `archlinux.org`:

```mermaid
graph LR
    subgraph "App Startup"
        A[Create Oneshot Channel] --> B[completion_tx]
        A --> C[completion_rx]
    end
    
    subgraph "Startup News Worker"
        B --> D[Startup Fetch]
        D --> E[Filter & Send]
        E --> F[Send completion signal]
    end
    
    subgraph "Aggregated Feed Worker"
        C --> G[Wait for signal]
        G --> H[Receive signal]
        H --> I[Additional delay]
        I --> J[Aggregated Fetch]
    end
    
    F -.->|Unblocks| G
    
    style F fill:#90EE90
    style G fill:#FFB6C1
    style H fill:#90EE90
```

## Fetch Sources Details

### Arch News Fetch

```mermaid
graph TB
    Start[Fetch Arch News] --> CheckCache{Check Disk Cache}
    CheckCache -->|Cache Hit & Valid| ReturnCache[Return Cached Items]
    CheckCache -->|Cache Miss or Expired| RateLimit[Apply Rate Limiting]
    RateLimit --> CircuitBreaker{Circuit Breaker<br/>Status}
    CircuitBreaker -->|Open| UseStale[Use Stale Cache if Available]
    CircuitBreaker -->|Closed| Fetch[Fetch from archlinux.org/news/feed]
    Fetch --> Parse[Parse RSS Feed]
    Parse --> Filter[Filter by Date if max_age set]
    Filter --> Cache[Update Disk Cache]
    Cache --> Return[Return News Items]
    ReturnCache --> Return
    UseStale --> Return
```

### Package Updates Fetch

```mermaid
graph TB
    Start[Fetch Package Updates] --> LoadUpdates[Load available_updates.txt]
    LoadUpdates --> Scan[Scan Installed Packages]
    Scan --> CheckVersions{Compare Versions}
    CheckVersions -->|New Version| FetchDate[Fetch Package Date<br/>from archlinux.org]
    CheckVersions -->|Same Version| Skip[Skip Package]
    FetchDate --> CheckSeen{Already Seen?}
    CheckSeen -->|Yes| Skip
    CheckSeen -->|No| CreateItem[Create Update Item]
    CreateItem --> AddToList[Add to Updates List]
    AddToList --> Limit{Reached Limit?}
    Limit -->|No| Scan
    Limit -->|Yes| Return[Return Updates]
    Skip --> Limit
```

### AUR Comments Fetch

```mermaid
graph TB
    Start[Fetch AUR Comments] --> GetAUR[Get Installed AUR Packages]
    GetAUR --> ForEach[For Each AUR Package]
    ForEach --> FetchComments[Fetch Comments from AUR API]
    FetchComments --> ParseComments[Parse Comment Data]
    ParseComments --> CheckSeen{Comment Already Seen?}
    CheckSeen -->|Yes| Skip[Skip Comment]
    CheckSeen -->|No| CreateItem[Create Comment Item]
    CreateItem --> AddToList[Add to Comments List]
    AddToList --> Limit{Reached Limit?}
    Limit -->|No| ForEach
    Limit -->|Yes| Sort[Sort by Date Desc]
    Sort --> Return[Return Comments]
    Skip --> Limit
```

## Rate Limiting and Circuit Breaker

To prevent overwhelming `archlinux.org` and getting blocked:

```mermaid
stateDiagram-v2
    [*] --> Closed: Initial State
    
    Closed --> Open: Consecutive Failures >= Threshold
    Open --> HalfOpen: Backoff Timeout Expired
    HalfOpen --> Closed: Success
    HalfOpen --> Open: Failure
    
    note right of Closed
        Normal operation
        Requests allowed
    end note
    
    note right of Open
        Blocking requests
        Using cached data
        Exponential backoff
    end note
    
    note right of HalfOpen
        Testing connection
        Single request allowed
    end note
```

## Caching Strategy

```mermaid
graph TB
    Request[News Content Request] --> MemoryCache{In-Memory Cache<br/>15min TTL}
    MemoryCache -->|Hit| Return[Return Content]
    MemoryCache -->|Miss| DiskCache{Disk Cache<br/>14day TTL}
    DiskCache -->|Hit| PopulateMem[Populate Memory Cache]
    PopulateMem --> Return
    DiskCache -->|Miss| Network[Fetch from Network]
    Network --> Parse[Parse Content]
    Parse --> StoreMem[Store in Memory Cache]
    StoreMem --> StoreDisk[Store in Disk Cache]
    StoreDisk --> Return
```

## Error Handling

```mermaid
graph TB
    Fetch[Fetch Operation] --> Success{Success?}
    Success -->|Yes| Process[Process Results]
    Success -->|No| ErrorType{Error Type}
    
    ErrorType -->|Network Timeout| Retry{Retries<br/>Available?}
    ErrorType -->|Rate Limited| Backoff[Exponential Backoff]
    ErrorType -->|Circuit Breaker| UseCache[Use Cached Data]
    ErrorType -->|Parse Error| LogError[Log Error & Continue]
    
    Retry -->|Yes| RetryFetch[Retry Fetch]
    RetryFetch --> Fetch
    Retry -->|No| UseCache
    
    Backoff --> Wait[Wait Backoff Period]
    Wait --> Fetch
    
    UseCache --> Process
    LogError --> Process
    Process --> Continue[Continue with Available Data]
```

## Key Components

### Workers

1. **Startup News Worker** (`spawn_startup_news_worker`)
   - Fetches news on app startup
   - Uses startup news preferences
   - Filters by source, age, and read status
   - Sends completion signal when done

2. **Aggregated Feed Worker** (`spawn_aggregated_news_feed_worker`)
   - Fetches full news feed for main UI
   - Waits for startup fetch to complete
   - Always fetches all sources (arch news, advisories, updates, comments)

3. **News Content Worker** (`spawn_news_content_worker`)
   - Fetches individual article/package content on demand
   - Uses debouncing to prevent rapid requests
   - Implements caching (memory + disk)

### Coordination

- **Oneshot Channel**: Used to signal completion between startup and aggregated fetches
- **Random Delays**: Jitter prevents thundering herd problems
- **Rate Limiting**: Semaphore-based limiting for archlinux.org requests
- **Circuit Breaker**: Prevents repeated failures from overwhelming the server

### Data Flow

1. **Startup**: App initializes → Workers spawned → Startup fetch begins
2. **Coordination**: Startup fetch completes → Signal sent → Aggregated fetch unblocks
3. **Fetching**: Sources fetched sequentially (archlinux.org) or in parallel (others)
4. **Processing**: Items filtered, sorted, and deduplicated
5. **Delivery**: Items sent via channels to UI components
6. **Caching**: Successful fetches cached for future use

## Configuration

News fetching behavior is controlled by settings in `settings.conf`:

- `startup_news_show_arch_news`: Enable/disable Arch news in startup popup
- `startup_news_show_advisories`: Enable/disable security advisories
- `startup_news_show_pkg_updates`: Enable/disable package updates
- `startup_news_show_aur_comments`: Enable/disable AUR comments
- `startup_news_max_age_days`: Maximum age of news items to show
- `startup_news_configured`: Whether startup news is configured

## Performance Optimizations

1. **Caching**: Multi-level caching (memory + disk) reduces network requests
2. **Parallel Fetching**: Non-archlinux.org sources fetched in parallel
3. **Sequential Fetching**: archlinux.org sources fetched sequentially to prevent blocking
4. **Debouncing**: User interactions debounced to prevent rapid requests
5. **Request Draining**: Stale requests discarded, only most recent processed
6. **Incremental Updates**: Uses last startup timestamp to optimize fetch window
7. **Circuit Breaker**: Prevents cascading failures during outages

## Troubleshooting

### Issue: Getting blocked by archlinux.org

**Symptoms**: Timeout errors, rate limiting warnings

**Causes**:
- Concurrent requests to archlinux.org
- Too many requests in short time
- Network issues causing retries

**Solutions**:
- Ensure coordination mechanism is working (check logs for completion signals)
- Check circuit breaker status
- Verify rate limiting is active
- Review backoff delays

### Issue: News not updating

**Symptoms**: Old news items displayed, no new items

**Causes**:
- Cache not expiring
- Network failures
- Filter settings too restrictive

**Solutions**:
- Clear cache files
- Check network connectivity
- Review filter settings (max_age_days, source preferences)

### Issue: Slow news loading

**Symptoms**: Long delays before news appears

**Causes**:
- Network latency
- Large number of installed packages
- Rate limiting delays

**Solutions**:
- Check network connection
- Reduce number of sources enabled
- Review installed package count
- Check for circuit breaker backoff

