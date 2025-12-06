# Refactoring Evaluation and Priority List

This document evaluates duplicate code patterns and refactoring opportunities in the Pacsea codebase, prioritized by impact and effort.

**Last Updated:** 2025-12-01  
**Codebase Version:** feat/integrated-process branch

## Executive Summary

The codebase shows several areas of code duplication that could benefit from refactoring. The most impactful opportunities involve:
1. **Command execution patterns** - 38 files with 108 direct `Command::new` usages and multiple wrapper functions
2. **Cache implementations** - 4 cache modules (1018 total lines) with ~350-400 lines of duplicated logic
3. **Package validation** - 6 duplicate validation functions in `args/package.rs`
4. **String parsing utilities** - Existing utility (`util/config.rs`) is unused; 13 files have inline implementations

---

## Priority 1: High Impact, Low-Medium Effort

### 1.1 Unify Command Execution Patterns

**Impact:** High - Affects error handling consistency, testability, and maintainability  
**Effort:** Medium  
**Files Affected:** ~38 files using `Command::new` directly

**Current State:**
- `logic/preflight/command.rs` - `CommandRunner` trait with `SystemCommandRunner` implementation
- `logic/services/command.rs` - `run_command()` function returning `Result<String, String>`
- `util/pacman.rs` - `run_pacman()` function with custom error handling
- `args/package.rs` - Multiple inline command executions with similar patterns

**Duplication Examples:**
```rust
// Pattern 1: logic/services/command.rs
pub(super) fn run_command(program: &str, args: &[&str], display: &str) -> Result<String, String> {
    let output = Command::new(program).args(args).output()
        .map_err(|err| format!("failed to spawn `{display}`: {err}"))?;
    if !output.status.success() {
        return Err(format!("`{display}` exited with status {}", output.status));
    }
    String::from_utf8(output.stdout)
        .map_err(|err| format!("`{display}` produced invalid UTF-8: {err}"))
}

// Pattern 2: util/pacman.rs
pub fn run_pacman(args: &[&str]) -> Result<String> {
    let out = std::process::Command::new("pacman").args(args).output()?;
    if !out.status.success() {
        return Err(format!("pacman {:?} exited with {:?}", args, out.status).into());
    }
    Ok(String::from_utf8(out.stdout)?)
}

// Pattern 3: args/package.rs (multiple instances)
match Command::new("pacman")
    .args(["-Si", package_name])
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .output()
{
    Ok(output) => output.status.success(),
    Err(_) => false,
}
```

**Recommendation:**
- Extend `CommandRunner` trait to support optional `display` parameter for better error messages
- Migrate `run_command()` and `run_pacman()` to use `CommandRunner` trait
- Create convenience wrappers for common commands (pacman, paru, yay)
- Replace inline command executions in `args/package.rs` with trait-based approach

**Benefits:**
- Consistent error handling across codebase
- Better testability through trait-based dependency injection
- Single source of truth for command execution logic
- Easier to add features like timeout, retry logic, or dry-run support

---

### 1.2 Consolidate Cache Implementations

**Impact:** High - Reduces code duplication significantly (~350-400 duplicated lines)  
**Effort:** Medium  
**Files Affected:**
- `app/deps_cache.rs` (220 lines)
- `app/files_cache.rs` (267 lines)
- `app/sandbox_cache.rs` (321 lines)
- `app/services_cache.rs` (210 lines)
- **Total:** 1018 lines across 4 files

**Current State:**
All cache modules follow nearly identical patterns:
- `compute_signature()` - **Identical** implementation in all 4 files (~10 lines each)
- `load_cache()` - Very similar implementations (~15-20 lines each)
- `load_cache_partial()` - Present in `files_cache.rs` and `sandbox_cache.rs` with similar logic
- `save_cache()` - Nearly identical serialization logic (~10 lines each)
- `temp_path()` test helper - Identical pattern in all 4 files
- `sample_packages()` test helper - Similar structure across modules

**Duplication Examples:**
```rust
// Identical in all 4 cache modules
pub fn compute_signature(packages: &[crate::state::PackageItem]) -> Vec<String> {
    let mut names: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
    names.sort();
    names
}

// Nearly identical save_cache pattern (only type name differs)
pub fn save_cache(path: &PathBuf, signature: &[String], data: &[T]) {
    let cache = CacheStruct {
        install_list_signature: signature.to_vec(),
        data: data.to_vec(),
    };
    if let Ok(s) = serde_json::to_string(&cache) {
        let _ = fs::write(path, s);
        tracing::debug!(path = %path.display(), count = data.len(), "saved cache");
    }
}
```

**Recommendation:**
- Create a generic `Cache<T>` trait or struct that handles signature computation and serialization
- Use generics to handle different cached data types (`DependencyInfo`, `PackageFileInfo`, `SandboxInfo`, `ServiceImpact`)
- Extract common cache operations into a shared `app/cache.rs` module
- Keep type-specific logic (like partial matching strategies for `sandbox_cache`) in specialized implementations
- Consolidate test utilities into a shared test helper module

**Benefits:**
- Reduce code duplication by ~60-70% (eliminate ~250-300 lines)
- Single place to fix cache bugs or add features
- Consistent cache behavior across all cache types
- Easier to add new cache types in the future

---

## Priority 2: Medium Impact, Low Effort

### 2.1 Extract Common String Parsing Utilities

**Impact:** Medium - Improves code consistency and reduces duplication  
**Effort:** Low  
**Files Affected:** 13 files with inline comment-skipping patterns

**Current State:**
- `util/config.rs` - `skip_comment_or_empty()` and `parse_key_value()` functions exist but are **NOT USED**
- 13 files implement inline comment-skipping logic with slight variations:

| File | Comment Prefixes | Locations |
|------|-----------------|-----------|
| `install/patterns.rs` | `#`, `//`, `;` | 1 |
| `theme/config/theme_loader.rs` | `#`, `//` | 1 |
| `theme/config/settings_save.rs` | `#`, `//` | 3 |
| `theme/config/settings_ensure.rs` | `#`, `//` (complex) | 5+ |
| `theme/config/tests.rs` | `#`, `//` | 3 |
| `theme/settings/parse_settings.rs` | `#`, `//` | 1 |
| `theme/settings/parse_keybinds.rs` | `#`, `//` | 1 |
| `events/modals/import.rs` | `#` only | 1 |
| `args/install.rs` | `#` only | 1 |
| `logic/faillock.rs` | `#` only | 1 |
| `logic/deps/srcinfo.rs` | `#` only | 2 |
| `logic/sandbox/parse.rs` | `#` only | 5 |
| `logic/files/pkgbuild_parse.rs` | `#` only | 3 |

**Duplication Examples:**
```rust
// Full pattern in util/config.rs (EXISTS BUT UNUSED):
pub fn skip_comment_or_empty(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.starts_with("//")
        || trimmed.starts_with(';')
}

// Inline pattern repeated in install/patterns.rs, theme/config/*, etc:
if line.is_empty()
    || line.starts_with('#')
    || line.starts_with("//")
    || line.starts_with(';')
{
    continue;
}

// Simplified pattern in logic/sandbox/parse.rs, logic/deps/srcinfo.rs:
if line.is_empty() || line.starts_with('#') {
    continue;
}
```

**Recommendation:**
1. **Immediate:** Update files using `#`, `//`, `;` pattern to import and use `util/config::skip_comment_or_empty()`
2. **Optional:** Create a simpler `skip_shell_comment_or_empty()` for PKGBUILD/srcinfo parsing that only checks `#`
3. Migrate all 13 files to use shared utilities

**Benefits:**
- Consistent comment handling across codebase
- Single place to update comment syntax support
- Reduced code duplication (~50-100 lines)
- The utility already exists - just needs adoption

---

### 2.2 Unify Package Validation Functions

**Impact:** Medium - Reduces duplication in package checking logic  
**Effort:** Low-Medium  
**Files Affected:** `args/package.rs` (259 lines)

**Current State:**
6 functions with significant duplication:

| Function | Purpose | Lines |
|----------|---------|-------|
| `is_official_package()` | Check via `pacman -Si` | ~14 |
| `is_official_package_search()` | Check via `sudo pacman -Ss` | ~26 |
| `is_aur_package()` | Check via `helper -Si` | ~14 |
| `is_aur_package_search()` | Check via `helper -Ss` | ~26 |
| `validate_and_categorize_packages()` | Uses `-Si` functions | ~26 |
| `validate_and_categorize_packages_search()` | Uses `-Ss` functions | ~26 |

**Duplication Examples:**
```rust
// is_official_package() - uses pacman -Si, returns bool based on exit status
fn is_official_package(package_name: &str) -> bool {
    match Command::new("pacman")
        .args(["-Si", package_name])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
    {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

// is_aur_package() - identical structure, just different program name
fn is_aur_package(package_name: &str, helper: &str) -> bool {
    match Command::new(helper)  // Only this line differs
        .args(["-Si", package_name])
        // ... rest identical
    }
}

// validate_and_categorize_packages() and validate_and_categorize_packages_search()
// are IDENTICAL except for which is_*_package() functions they call
```

**Recommendation:**
```rust
// Unified approach with strategy enum
pub enum PackageCheckStrategy { Info, Search }

fn check_package_exists(
    package_name: &str,
    program: &str,  // "pacman" or helper name
    strategy: PackageCheckStrategy,
) -> bool { ... }

fn validate_and_categorize_packages(
    package_names: &[String],
    aur_helper: Option<&str>,
    strategy: PackageCheckStrategy,
) -> (Vec<String>, Vec<String>, Vec<String>) { ... }
```

**Benefits:**
- Eliminate ~80 lines of duplicate code
- Single source of truth for package checking logic
- Easier to add new validation strategies
- Consider using `CommandRunner` trait (from Priority 1.1) for testability

---

## Priority 3: Medium Impact, Medium-High Effort

### 3.1 Standardize Error Type Conversions

**Impact:** Medium - Improves error handling consistency  
**Effort:** Medium  
**Files Affected:** ~22 files with `map_err` and error formatting patterns

**Current State:**
Multiple error conversion patterns:
- `format!("failed to spawn `{display}`: {err}")` in `logic/services/command.rs`
- `format!("pacman {:?} exited with {:?}", args, out.status)` in `util/pacman.rs`
- Various `map_err(|e| format!("..."))` patterns throughout codebase

**Recommendation:**
- Standardize on `CommandError` type from `logic/preflight/command.rs` where applicable
- Create error conversion helpers for common patterns
- Consider using `thiserror` or `anyhow` for better error context propagation

**Benefits:**
- Consistent error messages
- Better error context propagation
- Easier debugging

---

### 3.2 Extract Common UI Rendering Patterns

**Impact:** Medium - Reduces UI code duplication  
**Effort:** Medium-High  
**Files Affected:** Multiple files in `ui/` directory

**Current State:**
Similar widget creation and styling patterns across:
- `ui/modals/common.rs`
- `ui/middle/mod.rs`
- `ui/results/mod.rs`
- Various modal renderers

**Recommendation:**
- Create helper functions for common widget creation patterns
- Extract shared styling logic into reusable components
- Consider a widget builder pattern for complex UI elements

**Benefits:**
- Consistent UI appearance
- Easier theme updates
- Reduced code duplication

---

## Priority 4: Lower Impact, Lower Priority

### 4.1 Consolidate Similar Test Utilities

**Impact:** Low-Medium - Improves test maintainability  
**Effort:** Low  
**Files Affected:** 4 cache test modules, potentially others

**Current State:**
Identical test helper patterns across cache modules:

```rust
// temp_path() - identical in all 4 cache modules (only filename prefix differs)
fn temp_path(label: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "pacsea_{cache_type}_cache_{label}_{}_{}.json",  // Only this differs
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time is before UNIX epoch")
            .as_nanos()
    ));
    path
}

// sample_packages() - similar structure in all modules
fn sample_packages() -> Vec<PackageItem> {
    vec![PackageItem { name: "...", ... }]
}
```

**Recommendation:**
- Create `#[cfg(test)] mod test_utils` in `app/mod.rs` or a dedicated `app/test_helpers.rs`
- Extract common functions:
  - `temp_path(prefix: &str, label: &str) -> PathBuf`
  - `sample_packages() -> Vec<PackageItem>`
  - `cleanup_temp_file(path: &PathBuf)`
- Consider a `TestPackageBuilder` for creating test data

**Benefits:**
- Consistent test patterns (~80-100 lines saved)
- Easier test maintenance
- Reusable test fixtures

---

### 4.2 Unify Result Type Aliases

**Impact:** Low - Minor consistency improvement  
**Effort:** Low  
**Files Affected:** 6 files with identical type alias

**Current State:**
The same `Result<T>` type alias is defined in 6 different files:
```rust
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
```

| File | Module Purpose |
|------|----------------|
| `util/pacman.rs` | Pacman command utilities |
| `util/curl.rs` | HTTP request utilities |
| `sources/mod.rs` | Network data retrieval |
| `index/mirrors.rs` | Mirror management |
| `app/runtime/mod.rs` | Runtime execution |
| `app/terminal.rs` | Terminal handling |

**Recommendation:**
- Create a shared `type Result<T>` in a common module (e.g., `util/mod.rs` or a new `error.rs`)
- Export and re-use across all modules that need this pattern
- Consider adopting `anyhow::Result` for consistent error context

**Benefits:**
- Single definition of common Result type
- Consistent error handling patterns
- Easier error propagation
- Eliminates ~6 duplicate type definitions

---

## Implementation Strategy

### Phase 1: Quick Wins (1-2 days)
**Goal:** Adopt existing utilities and eliminate simple duplication

1. **Adopt `util/config::skip_comment_or_empty()`** (Priority 2.1)
   - Update 13 files to use existing utility
   - Add `skip_shell_comment()` for `#`-only patterns
   - Low risk, high consistency gain

2. **Unify Result type alias** (Priority 4.2)
   - Create shared type in `util/mod.rs`
   - Update 6 files to use shared type
   - Trivial change, no functional impact

3. **Consolidate test utilities** (Priority 4.1)
   - Create `app/test_helpers.rs`
   - Extract `temp_path()` and `sample_packages()`
   - Update 4 cache test modules

### Phase 2: Medium Complexity Refactoring (2-3 days)
**Goal:** Reduce function duplication with minimal API changes

1. **Unify package validation functions** (Priority 2.2)
   - Introduce `PackageCheckStrategy` enum
   - Consolidate 6 functions into 2
   - Maintain existing public API

2. **Consolidate cache implementations** (Priority 1.2)
   - Create generic `Cache<T>` trait
   - Extract `compute_signature()` and `save_cache()` to shared module
   - Keep type-specific partial matching in specialized impls

### Phase 3: Core Infrastructure (3-5 days)
**Goal:** Major architectural improvements

1. **Unify command execution patterns** (Priority 1.1)
   - Extend `CommandRunner` trait with display parameter
   - Create `PacmanRunner`, `AurHelperRunner` convenience wrappers
   - Migrate high-impact files first (38 files total)
   - Add comprehensive test coverage

2. **Standardize error type conversions** (Priority 3.1)
   - Adopt `CommandError` type more broadly
   - Consider `thiserror` for structured errors

### Phase 4: Optional Polish (1-2 days)
**Goal:** UI and minor improvements

1. **Extract common UI rendering patterns** (Priority 3.2)
   - Only if UI changes are planned
   - Lower priority than core refactoring

---

## Metrics

### Current State (Verified)
| Area | Scope | Details |
|------|-------|---------|
| **Command execution** | 38 files, 108 usages | Direct `Command::new` calls without trait abstraction |
| **Cache duplication** | 4 files, 1018 total lines | ~350-400 lines of duplicated logic |
| **String parsing** | 13 files, ~30 locations | Inline comment skipping (utility exists but unused) |
| **Package validation** | 1 file, 6 functions | ~130 lines of duplicated logic |
| **Result type aliases** | 6 files | Identical `type Result<T> = ...` definition |

### Expected Improvements
| Priority | Code Reduction | Benefit |
|----------|---------------|---------|
| 1.1 Command patterns | ~200-300 lines | Unified error handling, testability via traits |
| 1.2 Cache consolidation | ~250-300 lines | Single cache implementation, type-safe generics |
| 2.1 String parsing | ~50-100 lines | Adopt existing utility, consistency |
| 2.2 Package validation | ~80-100 lines | Strategy pattern, reduced complexity |
| 3.1 Error types | ~50 lines | Consistent error propagation |
| 4.1 Test utilities | ~80-100 lines | Shared test helpers |
| 4.2 Result aliases | ~6 definitions | Single shared type |
| **Total** | **~700-900 lines** | Improved maintainability, testability, consistency |

---

## Notes

### Implementation Guidelines
- All refactoring should maintain backward compatibility
- Existing tests should continue to pass after refactoring
- Consider incremental migration strategy to avoid large breaking changes
- Document new shared utilities with comprehensive rustdoc comments
- Follow existing code quality standards (cyclomatic complexity < 25, comprehensive tests)

### Key Findings from Analysis
1. **`util/config.rs` is underutilized** - The `skip_comment_or_empty()` utility exists but is not imported anywhere. This is the lowest-hanging fruit.
2. **Cache modules are highly uniform** - 4 modules with nearly identical structure makes generic abstraction straightforward.
3. **Command execution is widespread** - 108 usages across 38 files means careful migration planning is needed.
4. **Package validation has clear patterns** - The `-Si` vs `-Ss` difference can be cleanly abstracted.

### Risk Assessment
| Priority | Risk Level | Mitigation |
|----------|------------|------------|
| 2.1 String parsing | Very Low | Simple import addition |
| 4.2 Result aliases | Very Low | Type alias change only |
| 4.1 Test utilities | Low | Test-only code |
| 2.2 Package validation | Low | Local to single file |
| 1.2 Cache consolidation | Medium | Generics may affect compile times |
| 1.1 Command execution | Medium-High | Wide impact, needs thorough testing |
| 3.1 Error types | Medium | May require API changes |

---

## Conclusion

### Recommended Approach
Start with **Phase 1 quick wins** to build momentum and demonstrate immediate value:
1. **Adopt `util/config::skip_comment_or_empty()`** - Zero risk, utility already exists
2. **Unify Result type aliases** - Trivial change with consistency benefit
3. **Consolidate test utilities** - Improves test maintainability

Then proceed to **Phase 2** for meaningful code reduction:
1. **Package validation unification** - ~80 lines saved, low risk
2. **Cache consolidation** - ~250-300 lines saved, moderate complexity

**Phase 3** (Command execution) should be tackled when:
- Time permits for thorough testing across 38 files
- A feature requires touching command execution code anyway
- Team has bandwidth for potentially wide-reaching changes

### Summary
| Quick Wins | Lines Saved | Risk |
|------------|-------------|------|
| String parsing adoption | ~50-100 | Very Low |
| Result type unification | ~6 defs | Very Low |
| Test utilities | ~80-100 | Low |
| **Subtotal** | **~150-200** | |

| Core Refactoring | Lines Saved | Risk |
|-----------------|-------------|------|
| Package validation | ~80-100 | Low |
| Cache consolidation | ~250-300 | Medium |
| Command execution | ~200-300 | Medium-High |
| **Subtotal** | **~530-700** | |

**Total potential reduction: ~700-900 lines** with improved maintainability, testability, and consistency.

---

## Appendix: Additional Duplication Opportunities (Lower Priority)

### A.1 Persistence Functions in `app/persist.rs`

**Impact:** Medium - 8 similar functions with boilerplate  
**Effort:** Medium  
**Lines:** ~185 lines (excluding tests)

**Current State:**
8 functions with similar patterns:
- `maybe_flush_cache()` / `maybe_flush_recent()` / `maybe_flush_news_read()` / `maybe_flush_install()`
- `maybe_flush_deps_cache()` / `maybe_flush_files_cache()` / `maybe_flush_services_cache()` / `maybe_flush_sandbox_cache()`

**Pattern:**
```rust
// Simple flush pattern (4 functions)
pub fn maybe_flush_X(app: &mut AppState) {
    if !app.X_dirty { return; }
    if let Ok(s) = serde_json::to_string(&app.X_data) {
        let _ = fs::write(&app.X_path, s);
        app.X_dirty = false;
    }
}

// Cache flush pattern with empty-list cleanup (4 functions)
pub fn maybe_flush_X_cache(app: &mut AppState) {
    if app.install_list.is_empty() {
        let _ = fs::remove_file(&app.X_cache_path);
        app.X_cache_dirty = false;
        return;
    }
    if !app.X_cache_dirty { return; }
    // ... compute signature and save
}
```

**Recommendation:**
Create a macro or generic function to reduce boilerplate:
```rust
fn maybe_flush<T: Serialize>(
    dirty_flag: &mut bool,
    data: &T,
    path: &PathBuf,
) { ... }
```

---

### A.2 Config Path Resolution in `theme/paths.rs`

**Impact:** Low - 3 nearly identical functions  
**Effort:** Low  
**Lines:** ~70 lines

**Current State:**
```rust
resolve_theme_config_path()    // looks for theme.conf + legacy
resolve_settings_config_path() // looks for settings.conf + legacy
resolve_keybinds_config_path() // looks for keybinds.conf + legacy
```

All three functions:
1. Get `HOME` and `XDG_CONFIG_HOME` env vars
2. Build candidates: `{base}/{primary}.conf` and `{base}/pacsea.conf`
3. Return first existing file

**Recommendation:**
```rust
fn resolve_config_path(primary_filename: &str) -> Option<PathBuf> {
    // Unified implementation
}

pub fn resolve_theme_config_path() -> Option<PathBuf> {
    resolve_config_path("theme.conf")
}
```

---

### A.3 Temp Path Generation in Tests

**Impact:** Low - Test helper consistency  
**Effort:** Very Low  
**Lines:** ~60+ occurrences across 34 files

**Current State:**
59 occurrences of `std::env::temp_dir()` with similar patterns:
```rust
let mut path = std::env::temp_dir();
path.push(format!(
    "pacsea_{type}_{label}_{}_{}.json",
    std::process::id(),
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
));
```

**Recommendation:**
Create shared test utility:
```rust
// In a test_utils module
pub fn temp_test_path(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "pacsea_{prefix}_{}_{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("System time before UNIX epoch")
            .as_nanos()
    ))
}
```

---

### A.4 Handler Trait Implementations

**Impact:** Low - Already using trait pattern  
**Effort:** Medium  
**Files:** `app/runtime/handlers/{files,sandbox,services}.rs`

**Current State:**
The codebase already uses `HandlerConfig` trait (good!), but implementations have:
- Similar `is_resolution_complete()` logic across handlers
- Repeated HashSet creation and iteration patterns
- Similar logging patterns

**Recommendation:**
- Extract common HashSet comparison logic to helper functions
- Consider default implementations in the trait for common patterns
- Create macros for repetitive field accessor implementations

---

### A.5 JSON Serialization/Deserialization Patterns

**Impact:** Low - Scattered duplication  
**Effort:** Low  

**Current State:**
- 16 uses of `serde_json::to_string()`
- 12 uses of `serde_json::from_str()`
- 60 uses of `fs::read_to_string()` + 80 uses of `fs::write()`

Many follow similar patterns with similar error handling.

**Recommendation:**
Consider helper functions for common patterns:
```rust
fn load_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    fs::read_to_string(path).ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn save_json<T: Serialize>(path: &Path, data: &T) -> bool {
    serde_json::to_string(data)
        .map(|s| fs::write(path, s).is_ok())
        .unwrap_or(false)
}
```

---

### Summary of Additional Opportunities

| Area | Lines Affected | Effort | Priority |
|------|---------------|--------|----------|
| Persistence functions | ~50 lines | Medium | Low |
| Config path resolution | ~40 lines | Low | Low |
| Temp path in tests | ~60 occurrences | Very Low | Low |
| Handler implementations | ~100 lines | Medium | Very Low |
| JSON helpers | ~30 lines | Low | Very Low |
| **Additional Total** | **~280 lines** | | |

These are lower priority than the main recommendations because:
1. Some already use trait-based abstractions (handlers)
2. The duplication is more scattered
3. The effort-to-benefit ratio is lower
4. Some patterns benefit from explicit code for clarity (persistence)

