# Implementation Plan: Migrating Pacsea to Use arch-toolkit

**Created:** 2025-01-XX  
**Status:** Completed  
**Target:** Replace custom AUR implementation with `arch-toolkit` crate  
**Last Verified:** 2025-01-XX (arch-toolkit v0.1.2)  
**Progress:** Phase 1 ✅ | Phase 2 ✅ | Phase 3 ✅ | Phase 4 ✅ | Phase 5 ✅ | Phase 6 ✅

## Overview

This document outlines the plan to migrate Pacsea's AUR-related functionality to use the `arch-toolkit` crate, which is now available on crates.io. This migration will reduce code duplication, improve maintainability, and leverage the robust rate limiting and caching features provided by arch-toolkit.

## Current State Analysis

### What Pacsea Currently Implements

1. **AUR Search** (`src/sources/search.rs`) ✅ **MIGRATED**
   - ✅ Now uses `arch-toolkit`'s `ArchClient::aur().search()`
   - ✅ Automatic rate limiting and retry logic
   - ✅ Memory caching enabled (5min TTL)
   - ✅ Reduced code by ~40 lines

2. **AUR Comments** (`src/sources/comments.rs`) ✅ **MIGRATED**
   - ✅ Now uses `arch-toolkit`'s `ArchClient::aur().comments()`
   - ✅ Automatic rate limiting and retry logic
   - ✅ Memory caching enabled (10min TTL)
   - ✅ Reduced from ~896 lines to 113 lines (~87% reduction)

3. **PKGBUILD Fetching** (`src/sources/pkgbuild.rs`, `src/logic/files/pkgbuild_fetch.rs`) ✅ **MIGRATED**
   - ✅ AUR: Now uses `arch-toolkit`'s `ArchClient::aur().pkgbuild()`
   - ✅ Automatic rate limiting and retry logic for AUR packages
   - ✅ Memory caching enabled (1hr TTL)
   - ✅ Official: GitLab API calls (unchanged - arch-toolkit doesn't handle official packages)
   - ✅ Local cache checking (yay/paru caches) - preserved as Pacsea-specific feature
   - ✅ Reduced code by ~50-70 lines (removed manual rate limiting)

### What arch-toolkit Provides

1. **AUR Search** (`ArchClient::aur().search()`)
   - AUR RPC v5 API integration
   - Automatic rate limiting (500ms base delay, exponential backoff)
   - Built-in retry policies with exponential backoff
   - Optional caching (memory and disk)
   - Proper error types

2. **AUR Package Info** (`ArchClient::aur().info()`)
   - Batch fetching for multiple packages
   - Same rate limiting and caching as search

3. **AUR Comments** (`ArchClient::aur().comments()`)
   - HTML scraping with proper parsing
   - Date parsing and sorting
   - Pinned comment detection
   - Rate limiting and caching support

4. **PKGBUILD Fetching** (`ArchClient::aur().pkgbuild()`)
   - AUR cgit integration
   - Automatic rate limiting (500ms base delay, exponential backoff)
   - Caching support
   - Proper error handling

## Migration Strategy

### Phase 1: Add Dependency and Setup Client ✅ **COMPLETED**

**Files Modified:**
- ✅ `Cargo.toml` - Added `arch-toolkit = "0.1.2"` dependency
- ✅ `src/sources/mod.rs` - Added `ArchClient` initialization with `OnceLock` and `Arc`
- ✅ `src/app/runtime/mod.rs` - Added `init_arch_client()` call during runtime startup

**Completed Tasks:**
1. ✅ Added `arch-toolkit = "0.1.2"` to `Cargo.toml` dependencies
2. ✅ Created shared `ArchClient` instance using `OnceLock<Option<Arc<ArchClient>>>`
3. ✅ Configured client with user agent "pacsea/{version}" and default timeout (30s)
4. ✅ Added optional caching support via `init_arch_client_with_cache()` function

**Implementation Details:**
- Uses `OnceLock` for thread-safe lazy initialization
- Provides `get_arch_client()` accessor function
- Supports optional cache configuration via `CacheConfigBuilder`
- Error handling: logs warnings but doesn't fail application startup

**Estimated Effort:** 1-2 hours  
**Actual Effort:** ~1.5 hours

### Phase 2: Replace AUR Search ✅ **COMPLETED**

**Files Modified:**
- ✅ `src/sources/search.rs` - Replaced `fetch_all_with_errors()` implementation
- ✅ No changes needed in `src/logic/` - function signature maintained for backward compatibility

**Previous Implementation:**
- Used `curl` via `spawn_blocking` for AUR RPC v5 API calls
- Manual JSON parsing with `serde_json::Value`
- Manual error handling
- ~60 lines of parsing code

**New Implementation:**
- ✅ Uses `client.aur().search(&query).await` from arch-toolkit
- ✅ Maps `AurPackage` to `PackageItem`:
  - All fields map directly (name, version, description, popularity, out_of_date, orphaned)
  - Sets `source: Source::Aur` explicitly
- ✅ Converts `Result<Vec<AurPackage>>` to `(Vec<PackageItem>, Vec<String>)` format
- ✅ Handles `ArchClient` unavailability gracefully (returns error message)

**Code Reduction:**
- Removed ~40 lines of manual JSON parsing code
- Removed unused imports (`percent_encode`, `s`)
- Simplified error handling

**Testing:**
- ✅ Updated tests to work with arch-toolkit
- ✅ Added test for uninitialized client handling
- ✅ Added integration test with `--ignored` flag for network access

**Benefits:**
- Automatic rate limiting (500ms base delay)
- Built-in retry logic with exponential backoff
- Optional caching support (can be enabled in Phase 5)
- Better error types from arch-toolkit
- No breaking changes - function signature unchanged

**Estimated Effort:** 2-3 hours  
**Actual Effort:** ~2 hours

### Phase 3: Replace AUR Comments ✅ **COMPLETED**

**Files Modified:**
- ✅ `src/sources/comments.rs` - Replaced `fetch_aur_comments()` implementation
- ✅ `src/state/types.rs` - Verified `AurComment` types are compatible (identical structure)

**Previous Implementation:**
- ~896 lines total (including ~700 lines of HTML parsing)
- HTML scraping using `reqwest` + `scraper`
- Complex HTML parsing logic for comments, dates, timezone conversion
- Manual date parsing and timezone conversion
- Complex pinned comment detection
- Markdown conversion from HTML
- Manual rate limiting (5s timeout)
- No caching

**New Implementation:**
- ✅ Uses `client.aur().comments(&pkgname).await` from arch-toolkit
- ✅ Maps `arch_toolkit::AurComment` to `pacsea::state::types::AurComment`:
  - Types are structurally identical, but Rust requires explicit conversion
  - All fields map directly (id, author, date, date_timestamp, date_url, content, pinned)
- ✅ Converts `Result<Vec<arch_toolkit::AurComment>>` to `Result<Vec<AurComment>>`
- ✅ Handles `ArchClient` unavailability gracefully (returns error message)
- ✅ Removed all HTML parsing, date parsing, timezone conversion, and pinned detection logic

**Code Reduction:**
- Removed ~783 lines of HTML parsing code (~87% reduction)
- File size: ~896 lines → 113 lines
- Removed unused imports (`scraper`, `chrono`, `reqwest::header`, `std::time::Duration`, `std::collections::HashSet`)
- Removed all helper functions:
  - `CommentExtractionContext`
  - `extract_comment_from_header`
  - `determine_pinned_status`
  - `separate_and_sort_comments`
  - `sort_comments_by_date`
  - `convert_utc_to_local_date`
  - `get_timezone_abbreviation`
  - `get_tz_abbr_from_offset`
  - `parse_date_to_timestamp`
  - `html_to_formatted_text`
  - `convert_element_to_markdown`

**Testing:**
- ✅ Updated tests to work with arch-toolkit
- ✅ Added test for uninitialized client handling
- ✅ Added integration test with `--ignored` flag for network access
- ✅ Tests verify comment structure and error handling

**Benefits:**
- Automatic rate limiting (500ms base delay)
- Built-in retry logic with exponential backoff
- Optional caching support (can be enabled in Phase 5)
- Better error types from arch-toolkit
- No breaking changes - function signature unchanged
- Massive code reduction (~783 lines removed)
- Simplified maintenance (no HTML parsing to maintain)

**Note:**
- ✅ `scraper` dependency verified as still needed in `src/sources/news/parse.rs` (Phase 6)

**Estimated Effort:** 3-4 hours  
**Actual Effort:** ~2.5 hours

### Phase 4: Replace AUR PKGBUILD Fetching ✅ **COMPLETED**

**Files Modified:**
- ✅ `src/sources/pkgbuild.rs` - Replaced `fetch_pkgbuild_fast()` AUR implementation
- ✅ `src/logic/files/pkgbuild_fetch.rs` - Replaced `fetch_pkgbuild_sync()` AUR implementation

**Previous Implementation:**
- Manual rate limiting with mutex (200ms interval for async, 500ms for sync)
- Direct curl calls to AUR cgit
- Manual error handling
- ~50-70 lines of rate limiting and curl code

**New Implementation:**
- ✅ Uses `client.aur().pkgbuild(&name).await` from arch-toolkit for AUR packages
- ✅ Sync version uses `tokio::runtime::Handle::try_current()` with fallback to call async from sync
- ✅ Removed manual rate limiting code (PKGBUILD_RATE_LIMITER, PKGBUILD_MIN_INTERVAL_MS)
- ✅ Handles `ArchClient` unavailability gracefully (returns error message)
- ✅ Converts `arch_toolkit::Error` to string format for backward compatibility
- ✅ Kept local cache checking (yay/paru) - Pacsea-specific feature
- ✅ Kept GitLab fetching for official packages - arch-toolkit doesn't handle this
- ✅ Kept curl-based fallback for backward compatibility when ArchClient unavailable

**Code Reduction:**
- Removed ~50-70 lines of manual rate limiting code
- Removed unused imports (`std::sync::Mutex`, `std::time::{Duration, Instant}`)
- Simplified error handling

**Testing:**
- ✅ Updated tests to work with arch-toolkit
- ✅ Replaced fake curl test with arch-toolkit test
- ✅ Added `--ignored` flag for network-dependent test
- ✅ Kept official package test unchanged

**Benefits:**
- Automatic rate limiting for AUR packages (no manual mutex management)
- Built-in retry logic with exponential backoff
- Optional caching support (can be enabled in Phase 5)
- Better error types from arch-toolkit
- Backward compatibility maintained (curl fallback when ArchClient unavailable)
- No breaking changes - function signatures unchanged

**Implementation Details:**
- Async version (`fetch_pkgbuild_fast`): Direct `await` on arch-toolkit call
- Sync version (`fetch_pkgbuild_sync`): Uses `Handle::try_current()` with fallback to create new runtime
- Both versions maintain curl-based fallback for compatibility

**Estimated Effort:** 2-3 hours  
**Actual Effort:** ~2.5 hours

### Phase 5: Optional - Enable Caching ✅ **COMPLETED**

**Files Modified:**
- ✅ `src/sources/mod.rs` - Added `create_cache_config()` function
- ✅ `src/app/runtime/mod.rs` - Updated initialization to use caching

**Previous Implementation:**
- Caching infrastructure existed but was disabled by default
- `init_arch_client()` called with `None` (no caching)
- No cache configuration

**New Implementation:**
- ✅ Created `create_cache_config()` function with recommended TTLs:
  - Search: 5 minutes (300s) - search results change frequently
  - Comments: 10 minutes (600s) - comments are updated less frequently
  - PKGBUILD: 1 hour (3600s) - PKGBUILDs change infrequently
  - Memory cache size: 200 entries (reasonable for typical usage)
- ✅ Updated runtime initialization to use `init_arch_client_with_cache()` with cache config
- ✅ Memory cache enabled (fast, no persistence)
- ✅ Disk cache disabled (requires `cache-disk` feature flag, can be enabled later if needed)

**Cache Configuration:**
- TTLs chosen to balance freshness vs performance
- Aligns with arch-toolkit defaults and Pacsea's existing cache patterns
- Memory-only cache (no disk persistence) for simplicity

**Testing:**
- ✅ All tests pass (609 passed, 7 ignored)
- ✅ Code compiles successfully
- ✅ Clippy passes with no warnings
- ✅ Cache is transparent to existing code (no breaking changes)

**Benefits:**
- **Performance**: Faster responses for repeated queries
- **Reduced Network Load**: Fewer requests to AUR servers
- **Better User Experience**: Instant results for cached queries
- **Rate Limiting**: Caching helps avoid hitting rate limits
- **Transparent**: Existing code doesn't need changes

**Implementation Details:**
- Cache configuration is hardcoded but can be made configurable via settings in the future
- Memory cache with 200 entries is reasonable for typical usage
- TTLs are conservative to ensure data freshness while providing performance benefits

**Estimated Effort:** 1-2 hours  
**Actual Effort:** ~1 hour

### Phase 6: Cleanup and Testing ✅ **COMPLETED**

**Files Modified:**
- ✅ `src/sources/comments.rs` - Updated documentation comments to remove references to HTML scraping
- ✅ `src/sources/pkgbuild.rs` - Updated module comment to reflect arch-toolkit integration
- ✅ `src/sources/mod.rs` - Fixed doctest import path issue

**Completed Tasks:**
1. ✅ Verified no unused code remains:
   - ✅ No old AUR search implementation (curl) in `src/sources/search.rs`
   - ✅ No old AUR comments HTML parsing in `src/sources/comments.rs`
   - ✅ No manual rate limiting code in `src/sources/pkgbuild.rs` or `src/logic/files/pkgbuild_fetch.rs`
   - ✅ All migrated files use arch-toolkit correctly

2. ✅ Verified dependencies:
   - ✅ `scraper` - Still needed in `src/sources/news/parse.rs` for Arch news HTML parsing - **KEEP**
   - ✅ `chrono` - Used extensively throughout codebase for date/time operations - **KEEP**
   - ✅ `percent_encode` - Still needed in `src/sources/pkgbuild.rs` for official package GitLab URLs - **KEEP**
   - ✅ All dependencies are justified and documented

3. ✅ Updated documentation comments:
   - ✅ Updated `src/sources/comments.rs` to remove references to HTML scraping
   - ✅ Updated `src/sources/pkgbuild.rs` module comment to reflect arch-toolkit integration
   - ✅ Fixed doctest in `src/sources/mod.rs` to avoid import errors

4. ✅ Ran full test suite:
   - ✅ `cargo fmt --all` - No changes needed
   - ✅ `cargo clippy --all-targets --all-features -- -D warnings` - All clean
   - ✅ `cargo check` - Compiles successfully
   - ✅ `cargo test -- --test-threads=1` - 609 passed, 7 ignored
   - ✅ `cargo test --doc` - 16 passed, 1 ignored

5. ✅ Final code review:
   - ✅ All migrated functions use arch-toolkit correctly
   - ✅ Error handling is consistent
   - ✅ No breaking changes to public APIs
   - ✅ All tests are updated and passing
   - ✅ Code follows project conventions

**Code Verification:**
- ✅ No old AUR search code remains
- ✅ No old AUR comments HTML parsing code remains
- ✅ No manual rate limiting code for AUR operations remains
- ✅ All dependencies are justified
- ✅ All documentation comments are accurate
- ✅ All tests pass
- ✅ Code compiles and lints cleanly

**Migration Summary:**
- Total code reduction: ~873-903 lines removed
  - Phase 2: ~40 lines (AUR search)
  - Phase 3: ~783 lines (AUR comments)
  - Phase 4: ~50-70 lines (PKGBUILD rate limiting)
- All AUR operations now use arch-toolkit with:
  - Automatic rate limiting
  - Built-in retry logic
  - Memory caching enabled
  - Better error handling

**Estimated Effort:** 3-4 hours  
**Actual Effort:** ~2 hours

## Detailed Implementation Notes

### Type Compatibility

**AUR Search:**
- `arch_toolkit::AurPackage` → `pacsea::state::PackageItem`
  - `name: String` → `name: String` ✓
  - `version: String` → `version: String` ✓
  - `description: String` → `description: String` ✓
  - `popularity: Option<f64>` → `popularity: Option<f64>` ✓
  - `out_of_date: Option<u64>` → `out_of_date: Option<u64>` ✓
  - `orphaned: bool` → `orphaned: bool` ✓ (arch-toolkit already has this!)
  - `maintainer: Option<String>` → can derive `orphaned` from this if needed
  - `source: Source::Aur` (need to set explicitly)

**AUR Comments:**
- ✅ **Types are IDENTICAL!** `arch_toolkit::AurComment` matches `pacsea::state::types::AurComment` exactly:
  - `id: Option<String>` ✓
  - `author: String` ✓
  - `date: String` ✓
  - `date_timestamp: Option<i64>` ✓
  - `date_url: Option<String>` ✓
  - `content: String` ✓
  - `pinned: bool` ✓
- **No conversion needed!** Can use arch-toolkit's type directly.

**PKGBUILD:**
- Simple: `String` → `String` ✓

### Error Handling

**Current Pattern:**
```rust
pub async fn fetch_all_with_errors(query: String) -> (Vec<PackageItem>, Vec<String>)
```

**arch-toolkit Pattern:**
```rust
pub async fn search(&self, query: &str) -> Result<Vec<AurPackage>>
```

**Options:**
1. Keep current pattern (convert errors to strings)
2. Migrate to `Result` pattern (breaking change)
3. Hybrid: Use `Result` internally, convert at boundary

**Recommendation:** Option 1 for backward compatibility, consider Option 2 in future refactor.

### Rate Limiting

**Current:**
- Manual mutex-based rate limiting
- Different intervals for different operations (200ms, 500ms, 5s)

**arch-toolkit:**
- Automatic rate limiting (500ms base delay for archlinux.org, with exponential backoff)
- Exponential backoff on failures (starts at 500ms, doubles up to 60s max)
- Configurable retry policies (default: 3 retries, 1s initial delay, 30s max delay)
- Request serialization via semaphore (only 1 concurrent request to archlinux.org)

**Impact:**
- Can remove manual rate limiting code
- May need to adjust retry policies if current behavior differs

### Caching

**Current:**
- No network-level caching
- Only local file system caching (yay/paru caches)

**arch-toolkit:**
- Memory cache (in-process)
- Disk cache (persistent, requires `cache-disk` feature)
- Configurable TTLs per operation

**Recommendation:**
- Start without caching (Phase 1-4)
- Enable caching in Phase 5 if desired
- Keep local cache checking (yay/paru) as fallback

## Dependencies Impact

### New Dependencies
- `arch-toolkit = "0.1.2"` (add - current version on crates.io)

### Potentially Removable Dependencies
- `scraper = "0.25.0"` - ✅ **VERIFIED: Still needed**
  - Used in `src/sources/news/parse.rs` for Arch news HTML parsing
  - Not removable - required for news feed functionality

### No Change
- `reqwest` - Still needed for other operations
- `serde_json` - Still needed for other JSON parsing

## Testing Strategy

### Unit Tests
1. Mock `ArchClient` for isolated testing
2. Test type conversions (`AurPackage` → `PackageItem`)
3. Test error handling conversions

### Integration Tests
1. Test AUR search with real API (with `--ignored` flag)
2. Test AUR comments with real API
3. Test PKGBUILD fetching
4. Verify rate limiting behavior
5. Test caching (if enabled)

### Regression Tests
1. Verify search results match previous implementation
2. Verify comments display correctly
3. Verify PKGBUILD fetching works for both AUR and official packages
4. Test error cases (network failures, invalid packages)

## Risk Assessment

### Low Risk
- AUR search replacement (straightforward API mapping)
- PKGBUILD fetching (simple string return)

### Medium Risk
- AUR comments (complex HTML parsing, need to verify output compatibility)
- Error handling changes (may affect error messages)

### High Risk
- None identified

## Rollback Plan

If issues arise:
1. Keep old implementations in separate modules
2. Use feature flag to switch between old/new
3. Or revert commits if needed

## Success Criteria

1. ✅ All existing functionality works as before
2. ✅ Code reduction (fewer lines, less complexity)
3. ✅ Improved error handling
4. ✅ Better rate limiting (automatic)
5. ✅ All tests pass
6. ✅ No performance regression
7. ✅ Clippy and fmt pass

## Timeline Estimate

- **Phase 1:** 1-2 hours
- **Phase 2:** 2-3 hours
- **Phase 3:** 3-4 hours
- **Phase 4:** 2-3 hours
- **Phase 5:** 1-2 hours (optional)
- **Phase 6:** 3-4 hours

**Total:** 12-18 hours (1.5-2.5 days)

## Next Steps

1. ✅ Review this plan
2. ✅ Check `AurComment` type compatibility - **CONFIRMED: Types are identical!**
3. ✅ Phase 1 (add dependency) - **COMPLETED**
4. ✅ Phase 2 (replace AUR search) - **COMPLETED**
5. ✅ Phase 3 (replace AUR comments) - **COMPLETED**
6. ✅ Phase 4 (replace AUR PKGBUILD fetching) - **COMPLETED**
7. ✅ Phase 5 (optional - enable caching) - **COMPLETED**
8. ✅ Phase 6 (cleanup and testing) - **COMPLETED**
9. ✅ Migration complete - All phases finished successfully
10. ✅ All tests pass, code is production-ready

## Progress Summary

### Completed Phases

**Phase 1: Add Dependency and Setup Client** ✅
- Added arch-toolkit dependency
- Created shared ArchClient instance
- Added initialization in runtime
- Added optional caching support

**Phase 2: Replace AUR Search** ✅
- Replaced curl-based search with arch-toolkit
- Mapped AurPackage to PackageItem
- Maintained backward compatibility
- Updated tests
- Reduced code by ~40 lines

**Phase 3: Replace AUR Comments** ✅
- Replaced ~896 lines (including ~700 lines of HTML parsing) with arch-toolkit
- Types are identical - explicit mapping required for Rust type system
- Mapped arch_toolkit::AurComment to pacsea::state::types::AurComment
- Removed all HTML parsing, date parsing, timezone conversion logic
- Updated tests to work with arch-toolkit
- Reduced code by ~783 lines (~87% reduction)

**Phase 4: Replace AUR PKGBUILD Fetching** ✅
- Replaced manual rate limiting with arch-toolkit for AUR packages
- Updated both async (`fetch_pkgbuild_fast`) and sync (`fetch_pkgbuild_sync`) versions
- Used `Handle::try_current()` pattern for sync version to call async from sync
- Kept local cache checking (yay/paru) - Pacsea-specific feature
- Kept GitLab fetching for official packages - arch-toolkit doesn't handle this
- Maintained curl-based fallback for backward compatibility
- Reduced code by ~50-70 lines (removed manual rate limiting)

**Phase 5: Optional - Enable Caching** ✅
- Created `create_cache_config()` function with recommended TTLs
- Enabled memory caching for search (5min), comments (10min), and PKGBUILD (1hr)
- Updated runtime initialization to use caching by default
- Memory cache size: 200 entries (reasonable for typical usage)
- Disk cache disabled (can be enabled later with `cache-disk` feature)
- Caching is transparent to existing code (no breaking changes)

**Phase 6: Cleanup and Testing** ✅
- Verified no unused code remains from old AUR implementations
- Verified all dependencies are still needed (scraper, chrono, percent_encode)
- Updated documentation comments to remove references to old implementations
- Fixed doctest import path issue
- Ran full test suite: all tests pass (609 passed, 7 ignored)
- Final code review: migration is complete and production-ready
- Total code reduction: ~873-903 lines removed across all phases

## Notes

- arch-toolkit only handles AUR operations, not official packages
- Local cache checking (yay/paru) should remain as it's Pacsea-specific
- Official package PKGBUILD fetching must remain in Pacsea
- Consider enabling caching after initial migration is stable
- May want to contribute improvements back to arch-toolkit if needed

## Verification Status (2025-01-XX)

✅ **Verified against arch-toolkit v0.1.2:**

1. **API Methods:** All match as described
   - `ArchClient::aur().search(&str) -> Result<Vec<AurPackage>>` ✓
   - `ArchClient::aur().info(&[&str]) -> Result<Vec<AurPackageDetails>>` ✓
   - `ArchClient::aur().comments(&str) -> Result<Vec<AurComment>>` ✓
   - `ArchClient::aur().pkgbuild(&str) -> Result<String>` ✓

2. **Type Compatibility:**
   - `AurComment` types are **IDENTICAL** between arch-toolkit and Pacsea ✓
   - `AurPackage` → `PackageItem` mapping is straightforward (all fields match) ✓

3. **Rate Limiting:**
   - **CORRECTION:** Base delay is **500ms** (not 200ms as originally documented)
   - Uses exponential backoff (500ms → 1s → 2s → 4s, max 60s)
   - Request serialization via semaphore (1 concurrent request)

4. **Client Configuration:**
   - `ArchClient::new()` - default configuration ✓
   - `ArchClient::builder()` - builder pattern for customization ✓
   - Supports timeout, user agent, retry policy, cache config ✓
   - Environment variable configuration available ✓

5. **Caching:**
   - `CacheConfigBuilder` for configuration ✓
   - Memory cache (default: disabled) ✓
   - Disk cache (requires `cache-disk` feature) ✓
   - Per-operation TTL configuration ✓

6. **Dependencies:**
   - Current version: **0.1.2** (plan originally said 0.1.0)
   - Feature flags: `aur` (default), `cache-disk` (optional) ✓

