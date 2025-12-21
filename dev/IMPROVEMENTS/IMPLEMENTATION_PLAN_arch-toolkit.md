# Implementation Plan: Migrating Pacsea to Use arch-toolkit

**Created:** 2025-01-XX  
**Status:** Planning  
**Target:** Replace custom AUR implementation with `arch-toolkit` crate

## Overview

This document outlines the plan to migrate Pacsea's AUR-related functionality to use the `arch-toolkit` crate, which is now available on crates.io. This migration will reduce code duplication, improve maintainability, and leverage the robust rate limiting and caching features provided by arch-toolkit.

## Current State Analysis

### What Pacsea Currently Implements

1. **AUR Search** (`src/sources/search.rs`)
   - Direct AUR RPC v5 API calls via `curl`
   - Manual JSON parsing
   - Manual error handling
   - No built-in rate limiting
   - No caching

2. **AUR Comments** (`src/sources/comments.rs`)
   - HTML scraping using `reqwest` + `scraper`
   - Complex HTML parsing logic (~700 lines)
   - Manual date parsing and timezone conversion
   - Manual rate limiting (5s timeout)
   - No caching

3. **PKGBUILD Fetching** (`src/sources/pkgbuild.rs`, `src/logic/files/pkgbuild_fetch.rs`)
   - AUR: Direct curl calls to AUR cgit
   - Official: GitLab API calls
   - Manual rate limiting with mutex (200ms/500ms intervals)
   - Local cache checking (yay/paru caches)
   - No network-level caching

### What arch-toolkit Provides

1. **AUR Search** (`ArchClient::aur().search()`)
   - AUR RPC v5 API integration
   - Automatic rate limiting (200ms minimum)
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
   - Automatic rate limiting (200ms minimum)
   - Caching support
   - Proper error handling

## Migration Strategy

### Phase 1: Add Dependency and Setup Client

**Files to Modify:**
- `Cargo.toml` - Add `arch-toolkit` dependency
- `src/sources/mod.rs` - Initialize `ArchClient` instance
- `src/app/runtime/` - Pass `ArchClient` through runtime

**Tasks:**
1. Add `arch-toolkit = "0.1.0"` to `Cargo.toml` dependencies
2. Create a shared `ArchClient` instance in the runtime
3. Configure client with appropriate timeout and user agent
4. Optionally enable caching if desired

**Estimated Effort:** 1-2 hours

### Phase 2: Replace AUR Search

**Files to Modify:**
- `src/sources/search.rs` - Replace `fetch_all_with_errors()` implementation
- `src/logic/` - Update callers if needed

**Current Implementation:**
```rust
pub async fn fetch_all_with_errors(query: String) -> (Vec<PackageItem>, Vec<String>)
```

**New Implementation:**
- Use `client.aur().search(&query).await`
- Map `AurPackage` to `PackageItem`:
  - Most fields map directly (name, version, description, popularity, out_of_date, orphaned)
  - Set `source: Source::Aur` explicitly
- Convert errors to `Vec<String>` format (for backward compatibility)

**Considerations:**
- arch-toolkit returns `Result<Vec<AurPackage>>`, not `(Vec, Vec<String>)`
- `AurPackage` already has `orphaned: bool` field - no need to derive from maintainer!
- Error handling needs to be converted to string format
- May want to keep error tuple format for backward compatibility initially

**Estimated Effort:** 2-3 hours

### Phase 3: Replace AUR Comments

**Files to Modify:**
- `src/sources/comments.rs` - Replace `fetch_aur_comments()` implementation
- `src/state/types.rs` - Check if `AurComment` types are compatible

**Current Implementation:**
- ~700 lines of HTML parsing, date parsing, timezone conversion
- Complex pinned comment detection
- Markdown conversion

**New Implementation:**
- Use `client.aur().comments(&pkgname).await`
- Check if `arch_toolkit::AurComment` matches `pacsea::state::types::AurComment`
- If types differ, create conversion function
- May need to keep some formatting logic if arch-toolkit's output differs

**Considerations:**
- ✅ `AurComment` types are identical - no conversion needed!
- Date format should be compatible (both use same parsing logic)
- Markdown rendering is handled by arch-toolkit (same approach as Pacsea)
- Can remove ~600+ lines of HTML parsing code

**Estimated Effort:** 3-4 hours

### Phase 4: Replace AUR PKGBUILD Fetching

**Files to Modify:**
- `src/sources/pkgbuild.rs` - Update AUR PKGBUILD fetching
- `src/logic/files/pkgbuild_fetch.rs` - Update `fetch_pkgbuild_fast()` for AUR packages

**Current Implementation:**
- Manual rate limiting with mutex
- Direct curl calls
- Local cache checking (yay/paru)

**New Implementation:**
- Use `client.aur().pkgbuild(&name).await` for AUR packages
- Keep local cache checking (Pacsea-specific feature)
- Keep GitLab fetching for official packages (arch-toolkit doesn't handle this)

**Considerations:**
- arch-toolkit only handles AUR PKGBUILDs
- Official package PKGBUILD fetching must remain in Pacsea
- Local cache checking (yay/paru) should remain
- Can remove manual rate limiting for AUR packages

**Estimated Effort:** 2-3 hours

### Phase 5: Optional - Enable Caching

**Files to Modify:**
- `src/app/runtime/` - Configure `ArchClient` with caching
- `src/sources/mod.rs` - Cache configuration

**Tasks:**
1. Configure `CacheConfig` with appropriate TTLs
2. Enable memory cache (fast, no persistence)
3. Optionally enable disk cache (persists across restarts)
4. Test cache behavior

**Considerations:**
- Caching may change behavior (stale data)
- Need to decide on cache TTLs per operation
- Disk cache requires `cache-disk` feature flag

**Estimated Effort:** 1-2 hours

### Phase 6: Cleanup and Testing

**Tasks:**
1. Remove unused code:
   - Old AUR search implementation
   - Old AUR comments HTML parsing
   - Manual rate limiting code (for AUR operations)
   - Unused dependencies (`scraper`? - check if still needed)
2. Update tests:
   - Mock `ArchClient` for unit tests
   - Update integration tests
   - Verify backward compatibility
3. Run full test suite:
   - `cargo fmt --all`
   - `cargo clippy --all-targets --all-features -- -D warnings`
   - `cargo check`
   - `cargo test -- --test-threads=1`

**Estimated Effort:** 3-4 hours

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
- Automatic rate limiting (200ms minimum for archlinux.org)
- Exponential backoff on failures
- Configurable retry policies

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
- `arch-toolkit = "0.1.0"` (add)

### Potentially Removable Dependencies
- `scraper = "0.25.0"` - Check if still needed after comments migration
  - May still be needed for other HTML parsing (news, advisories?)

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
3. Start with Phase 1 (add dependency)
4. Proceed phase by phase with testing after each phase
5. Document any deviations from plan

## Notes

- arch-toolkit only handles AUR operations, not official packages
- Local cache checking (yay/paru) should remain as it's Pacsea-specific
- Official package PKGBUILD fetching must remain in Pacsea
- Consider enabling caching after initial migration is stable
- May want to contribute improvements back to arch-toolkit if needed

