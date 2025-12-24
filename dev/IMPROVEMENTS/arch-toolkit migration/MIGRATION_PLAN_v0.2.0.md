# Implementation Plan: Migrating Pacsea to arch-toolkit v0.2.0

**Created:** 2025-01-XX  
**Status:** Planning  
**Target:** Migrate dependency management functionality to `arch-toolkit` v0.2.0  
**Current Version:** arch-toolkit v0.1.2  
**Target Version:** arch-toolkit v0.2.0  
**Progress:** Phase 1 ⏳ | Phase 2 ⏳ | Phase 3 ⏳ | Phase 4 ⏳ | Phase 5 ⏳ | Phase 6 ⏳

## Overview

This document outlines the plan to migrate Pacsea's dependency management functionality to use the `arch-toolkit` crate v0.2.0, which introduces comprehensive dependency resolution, reverse dependency analysis, version comparison, package querying, and source determination capabilities. This migration will reduce code duplication, improve maintainability, and leverage the robust dependency management features provided by arch-toolkit.

## What's New in arch-toolkit v0.2.0

### New Features

1. **Dependency Resolution** (`DependencyResolver`)
   - Resolve dependencies for official, AUR, and local packages
   - Configurable resolution (include optional, make, check dependencies)
   - Batch dependency fetching for efficient queries
   - Conflict detection and status determination
   - Support for PKGBUILD cache callbacks

2. **Reverse Dependency Analysis** (`ReverseDependencyAnalyzer`)
   - Find all packages that depend on packages being removed
   - Distinguish direct vs transitive dependents
   - Generate conflict status with detailed reasons
   - Helper functions for quick dependency checks

3. **Version Comparison**
   - Pacman-compatible version comparison algorithm
   - Version requirement satisfaction checking
   - Major version bump detection
   - Extract major version components

4. **Package Querying**
   - Query installed packages
   - Query upgradable packages
   - Get installed and available package versions
   - Check if packages are installed or provided
   - Graceful degradation when pacman is unavailable

5. **Source Determination**
   - Determine package source (official, AUR, local)
   - Identify core repository packages
   - Detect critical system packages

### API Reference

Based on [arch-toolkit v0.2.0 documentation](https://docs.rs/arch-toolkit/0.2.0/arch_toolkit/):

**Dependency Resolution:**
```rust
use arch_toolkit::deps::DependencyResolver;
use arch_toolkit::{PackageRef, PackageSource};

let resolver = DependencyResolver::new();
let packages = vec![PackageRef { ... }];
let result = resolver.resolve(&packages)?;
```

**Reverse Dependency Analysis:**
```rust
use arch_toolkit::deps::ReverseDependencyAnalyzer;

let analyzer = ReverseDependencyAnalyzer::new();
let report = analyzer.analyze(&packages)?;
```

**Version Comparison:**
```rust
use arch_toolkit::deps::{compare_versions, version_satisfies};
use std::cmp::Ordering;

assert_eq!(compare_versions("1.2.3", "1.2.4"), Ordering::Less);
assert!(version_satisfies("2.0", ">=1.5"));
```

**Package Querying:**
```rust
use arch_toolkit::deps::{
    get_installed_packages, get_upgradable_packages,
    get_installed_version, get_available_version,
};

let installed = get_installed_packages()?;
let upgradable = get_upgradable_packages()?;
```

**Source Determination:**
```rust
use arch_toolkit::deps::{determine_dependency_source, is_system_package};

let installed = get_installed_packages()?;
let (source, is_core) = determine_dependency_source("bash", &installed);
let is_system = is_system_package("bash");
```

## Current State Analysis

### What Pacsea Currently Implements

Pacsea has a comprehensive dependency management system in `src/logic/deps/` with the following modules:

1. **Dependency Parsing** (`src/logic/deps/parse.rs`)
   - ✅ `parse_dep_spec()` - Parse dependency specifications with version constraints
   - ✅ `parse_pacman_si_deps()` - Parse dependencies from `pacman -Si` output
   - ✅ `parse_pacman_si_conflicts()` - Parse conflicts from `pacman -Si` output
   - ⚠️ Uses i18n for localized labels (arch-toolkit uses English-only)

2. **Package Querying** (`src/logic/deps/query.rs`)
   - ✅ `get_installed_packages()` - Query installed packages via `pacman -Qq`
   - ✅ `get_upgradable_packages()` - Query upgradable packages via `pacman -Qu`
   - ✅ `get_provided_packages()` - Get packages provided by installed packages
   - ✅ `is_package_installed_or_provided()` - Check if package is installed or provided
   - ⚠️ Custom implementation with pacman command execution

3. **Source Determination** (`src/logic/deps/source.rs`)
   - ✅ `determine_dependency_source()` - Determine package source (official, AUR, local)
   - ✅ `is_system_package()` - Check if package is a critical system package
   - ⚠️ Custom implementation

4. **Version Comparison** (`src/logic/deps/status.rs`)
   - ✅ `version_satisfies()` - Check if version satisfies requirement
   - ✅ `get_installed_version()` - Get installed version of a package
   - ⚠️ Custom implementation

5. **.SRCINFO Parsing** (`src/logic/deps/srcinfo.rs`)
   - ✅ `parse_srcinfo_deps()` - Parse dependencies from .SRCINFO
   - ✅ `parse_srcinfo_conflicts()` - Parse conflicts from .SRCINFO
   - ✅ `fetch_srcinfo()` - Fetch .SRCINFO from AUR
   - ⚠️ Custom implementation (arch-toolkit v0.1.2 already has this)

6. **Dependency Resolution** (`src/logic/deps/resolve.rs`)
   - ✅ `resolve_package_deps()` - Resolve dependencies for a single package
   - ✅ `batch_fetch_official_deps()` - Batch fetch official package dependencies
   - ✅ `fetch_package_conflicts()` - Fetch conflicts for a package
   - ⚠️ Complex custom implementation with pacman command execution

7. **Reverse Dependency Analysis** (`src/logic/deps/reverse.rs`)
   - ✅ `resolve_reverse_dependencies()` - Find packages that depend on given packages
   - ✅ `get_installed_required_by()` - Get installed packages that require a package
   - ✅ `has_installed_required_by()` - Check if package has reverse dependencies
   - ⚠️ Custom implementation

8. **AUR Dependency Resolution** (`src/logic/deps/aur.rs`)
   - ✅ AUR-specific dependency resolution logic
   - ⚠️ Custom implementation

9. **Dependency Status** (`src/logic/deps/status.rs`)
   - ✅ `determine_status()` - Determine dependency status (installed, missing, upgradable, conflict)
   - ⚠️ Custom implementation

10. **Main Resolution Logic** (`src/logic/deps.rs`)
    - ✅ `resolve_dependencies()` - Main entry point for dependency resolution
    - ✅ Conflict detection and merging
    - ✅ Dependency consolidation and deduplication
    - ⚠️ Complex custom implementation

### Comparison: Pacsea vs arch-toolkit v0.2.0

| Feature | Pacsea Implementation | arch-toolkit v0.2.0 | Migration Status |
|---------|----------------------|---------------------|------------------|
| **Dependency Parsing** | `parse_dep_spec()`, `parse_pacman_si_deps()` | `parse_dep_spec()`, `parse_pacman_si_deps()` | ✅ Available in v0.1.2 |
| **.SRCINFO Parsing** | `parse_srcinfo_deps()`, `fetch_srcinfo()` | `parse_srcinfo_deps()`, `fetch_srcinfo()` | ✅ Available in v0.1.2 |
| **PKGBUILD Parsing** | Custom in `src/logic/sandbox/` | `parse_pkgbuild_deps()` | ✅ Available in v0.1.2 |
| **Package Querying** | `get_installed_packages()`, `get_upgradable_packages()` | `get_installed_packages()`, `get_upgradable_packages()` | 🆕 Available in v0.2.0 |
| **Version Comparison** | `version_satisfies()`, `compare_versions()` | `version_satisfies()`, `compare_versions()` | 🆕 Available in v0.2.0 |
| **Source Determination** | `determine_dependency_source()`, `is_system_package()` | `determine_dependency_source()`, `is_system_package()` | 🆕 Available in v0.2.0 |
| **Dependency Resolution** | `resolve_package_deps()`, `resolve_dependencies()` | `DependencyResolver::resolve()` | 🆕 Available in v0.2.0 |
| **Reverse Dependencies** | `resolve_reverse_dependencies()` | `ReverseDependencyAnalyzer::analyze()` | 🆕 Available in v0.2.0 |
| **Conflict Detection** | Custom in `resolve_dependencies()` | Built into `DependencyResolver` | 🆕 Available in v0.2.0 |

### Key Differences

1. **API Design:**
   - Pacsea: Function-based API with custom types (`DependencyInfo`, `DependencyStatus`)
   - arch-toolkit: Struct-based API with `DependencyResolver` and `ReverseDependencyAnalyzer`, uses `PackageRef`, `Dependency`, `DependencyStatus`

2. **Type System:**
   - Pacsea: Custom `DependencyInfo` struct with `required_by`, `depends_on`, `is_core`, `is_system`
   - arch-toolkit: `Dependency` type with `source`, `status`, `spec`, but different structure

3. **Resolution Strategy:**
   - Pacsea: Direct dependencies only (non-recursive), custom conflict detection
   - arch-toolkit: Configurable recursive resolution, built-in conflict detection

4. **i18n Support:**
   - Pacsea: Uses i18n for localized pacman output parsing
   - arch-toolkit: English-only (no i18n dependency)

5. **Error Handling:**
   - Pacsea: Returns `Result<Vec<DependencyInfo>, String>`
   - arch-toolkit: Returns `Result<DependencyResolution, ArchToolkitError>`

## Migration Strategy

### Phase 1: Update Dependency and Enable `deps` Feature ⏳

**Files to Modify:**
- `Cargo.toml` - Update arch-toolkit version and enable `deps` feature

**Tasks:**
1. Update `arch-toolkit = "0.1.2"` to `arch-toolkit = { version = "0.2.0", features = ["deps", "aur"] }`
2. Verify compilation with new version
3. Run tests to ensure no breaking changes in AUR operations

**Estimated Effort:** 30 minutes  
**Risk:** Low (backward compatible release)

### Phase 2: Migrate Package Querying Functions ⏳

**Files to Modify:**
- `src/logic/deps/query.rs` - Replace with arch-toolkit functions
- `src/logic/deps.rs` - Update imports

**Current Implementation:**
- `get_installed_packages()` - Uses `pacman -Qq`
- `get_upgradable_packages()` - Uses `pacman -Qu`
- `get_provided_packages()` - Uses `pacman -Qq` + `pacman -Qi` for each package
- `is_package_installed_or_provided()` - Checks installed and provided sets

**New Implementation:**
- ✅ Use `arch_toolkit::deps::get_installed_packages()` directly
- ✅ Use `arch_toolkit::deps::get_upgradable_packages()` directly
- ⚠️ `get_provided_packages()` - Need to check if arch-toolkit has this (may need to keep custom)
- ⚠️ `is_package_installed_or_provided()` - May need to keep custom wrapper

**Type Compatibility:**
- arch-toolkit returns `Result<Vec<PackageRef>>` or `Result<HashSet<String>>`
- Pacsea expects `HashSet<String>`
- Need to convert types

**Testing:**
- Update tests to work with arch-toolkit
- Verify behavior matches current implementation
- Test error handling when pacman is unavailable

**Estimated Effort:** 2-3 hours  
**Risk:** Medium (type conversion needed)

### Phase 3: Migrate Version Comparison Functions ⏳

**Files to Modify:**
- `src/logic/deps/status.rs` - Replace version comparison functions
- `src/logic/deps.rs` - Update imports

**Current Implementation:**
- `version_satisfies()` - Custom version comparison logic
- `get_installed_version()` - Uses `pacman -Q` to get installed version
- `compare_versions()` - Custom comparison (if exists)

**New Implementation:**
- ✅ Use `arch_toolkit::deps::version_satisfies()` directly
- ✅ Use `arch_toolkit::deps::compare_versions()` directly
- ✅ Use `arch_toolkit::deps::get_installed_version()` directly

**Type Compatibility:**
- Functions should be compatible (same signatures)

**Testing:**
- Update tests to use arch-toolkit functions
- Verify version comparison behavior matches
- Test edge cases (empty versions, invalid versions)

**Estimated Effort:** 1-2 hours  
**Risk:** Low (direct function replacement)

### Phase 4: Migrate Source Determination Functions ⏳

**Files to Modify:**
- `src/logic/deps/source.rs` - Replace with arch-toolkit functions
- `src/logic/deps.rs` - Update imports

**Current Implementation:**
- `determine_dependency_source()` - Determines source (official, AUR, local) and core flag
- `is_system_package()` - Checks if package is critical system package

**New Implementation:**
- ✅ Use `arch_toolkit::deps::determine_dependency_source()` directly
- ✅ Use `arch_toolkit::deps::is_system_package()` directly

**Type Compatibility:**
- arch-toolkit returns `(PackageSource, bool)` where bool is `is_core`
- Pacsea expects `(Source, bool)` where `Source` is Pacsea's custom enum
- Need to convert `PackageSource` to `Source`

**Testing:**
- Update tests to work with arch-toolkit
- Verify source determination matches current behavior
- Test with official, AUR, and local packages

**Estimated Effort:** 2-3 hours  
**Risk:** Medium (type conversion needed)

### Phase 5: Migrate Dependency Resolution ⏳

**Files to Modify:**
- `src/logic/deps/resolve.rs` - Replace resolution logic
- `src/logic/deps.rs` - Update main resolution function
- `src/logic/deps/aur.rs` - May be replaced entirely

**Current Implementation:**
- Complex custom resolution with:
  - Batch fetching for official packages
  - Individual resolution for AUR and local packages
  - Conflict detection
  - Dependency merging and deduplication
  - Status determination

**New Implementation:**
- Use `DependencyResolver::new()` and `resolver.resolve(&packages)`
- Convert `PackageItem` to `PackageRef` for arch-toolkit
- Convert `DependencyResolution` result back to `Vec<DependencyInfo>`
- May need to keep some custom logic for:
  - Conflict detection against install list
  - Dependency merging (if arch-toolkit doesn't handle this)
  - Custom `DependencyInfo` structure

**Type Compatibility:**
- arch-toolkit uses `PackageRef` with `PackageSource`
- Pacsea uses `PackageItem` with `Source`
- arch-toolkit returns `DependencyResolution` with `Vec<Dependency>`
- Pacsea expects `Vec<DependencyInfo>`
- Need comprehensive type conversion layer

**Challenges:**
1. **Different Resolution Strategy:**
   - Pacsea: Direct dependencies only (non-recursive)
   - arch-toolkit: Configurable recursive resolution
   - Need to configure arch-toolkit to only resolve direct dependencies

2. **Custom DependencyInfo Structure:**
   - Pacsea's `DependencyInfo` has `required_by`, `depends_on`, `is_core`, `is_system`
   - arch-toolkit's `Dependency` has different fields
   - Need to map between structures

3. **Conflict Detection:**
   - Pacsea checks conflicts against install list and installed packages
   - arch-toolkit has built-in conflict detection but may need custom logic for install list

4. **Dependency Merging:**
   - Pacsea merges dependencies by name, keeping worst status
   - Need to verify if arch-toolkit handles this or if custom logic is needed

**Testing:**
- Comprehensive testing with official, AUR, and local packages
- Test conflict detection
- Test dependency merging
- Test error handling

**Estimated Effort:** 8-12 hours  
**Risk:** High (complex type conversion and logic differences)

### Phase 6: Migrate Reverse Dependency Analysis ⏳

**Files to Modify:**
- `src/logic/deps/reverse.rs` - Replace with arch-toolkit
- `src/logic/deps.rs` - Update imports

**Current Implementation:**
- `resolve_reverse_dependencies()` - Finds packages that depend on given packages
- `get_installed_required_by()` - Gets installed packages that require a package
- `has_installed_required_by()` - Checks if package has reverse dependencies

**New Implementation:**
- Use `ReverseDependencyAnalyzer::new()` and `analyzer.analyze(&packages)`
- Convert `ReverseDependencyReport` to Pacsea's format
- May need to keep custom wrappers for compatibility

**Type Compatibility:**
- arch-toolkit returns `ReverseDependencyReport` with `dependents`, `conflicts`, etc.
- Pacsea expects custom `ReverseDependencyReport` structure
- Need type conversion

**Testing:**
- Test reverse dependency analysis
- Verify results match current implementation
- Test with packages that have many dependents

**Estimated Effort:** 3-4 hours  
**Risk:** Medium (type conversion needed)

### Phase 7: Cleanup and Testing ⏳

**Files to Modify:**
- Remove unused code from old implementations
- Update documentation comments
- Verify all tests pass

**Tasks:**
1. Remove old implementation files if fully replaced:
   - `src/logic/deps/parse.rs` - May keep if i18n support needed
   - `src/logic/deps/query.rs` - Remove if fully migrated
   - `src/logic/deps/source.rs` - Remove if fully migrated
   - `src/logic/deps/status.rs` - May keep if custom logic needed
   - `src/logic/deps/resolve.rs` - May keep wrapper/adapter code
   - `src/logic/deps/reverse.rs` - May keep wrapper/adapter code
   - `src/logic/deps/aur.rs` - May be removed if arch-toolkit handles AUR

2. Update imports throughout codebase
3. Update documentation comments
4. Run full test suite:
   - `cargo fmt --all`
   - `cargo clippy --all-targets --all-features -- -D warnings`
   - `cargo check`
   - `cargo test -- --test-threads=1`

5. Verify no functionality regression:
   - Test dependency resolution with various package types
   - Test reverse dependency analysis
   - Test conflict detection
   - Test version comparison

**Estimated Effort:** 4-6 hours  
**Risk:** Low (cleanup and verification)

## Type Conversion Strategy

### PackageItem → PackageRef

```rust
fn package_item_to_ref(item: &PackageItem) -> PackageRef {
    PackageRef {
        name: item.name.clone(),
        version: item.version.clone(),
        source: source_to_package_source(&item.source),
    }
}

fn source_to_package_source(source: &Source) -> PackageSource {
    match source {
        Source::Official { repo, arch } => PackageSource::Official {
            repo: repo.clone(),
            arch: arch.clone(),
        },
        Source::Aur => PackageSource::Aur,
        Source::Local => PackageSource::Local,
    }
}
```

### Dependency → DependencyInfo

```rust
fn dependency_to_info(
    dep: &Dependency,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
) -> DependencyInfo {
    let (source, is_core) = determine_dependency_source(&dep.name, installed);
    let is_system = is_core || is_system_package(&dep.name);
    
    DependencyInfo {
        name: dep.name.clone(),
        version: dep.spec.version_req.clone(),
        status: dependency_status_to_pacsea_status(&dep.status),
        source: package_source_to_source(&dep.source),
        required_by: vec![], // Will be populated by merging logic
        depends_on: vec![], // Will be populated by resolution
        is_core,
        is_system,
    }
}
```

### DependencyStatus Conversion

```rust
fn dependency_status_to_pacsea_status(status: &arch_toolkit::DependencyStatus) -> DependencyStatus {
    match status {
        arch_toolkit::DependencyStatus::Installed => DependencyStatus::Installed,
        arch_toolkit::DependencyStatus::Missing => DependencyStatus::Missing,
        arch_toolkit::DependencyStatus::Upgradable { .. } => DependencyStatus::Upgradable,
        arch_toolkit::DependencyStatus::Conflict { reason } => DependencyStatus::Conflict {
            reason: reason.clone(),
        },
        // Handle other statuses
    }
}
```

## Potential Challenges and Solutions

### Challenge 1: i18n Support for Pacman Output Parsing

**Issue:** Pacsea uses i18n for parsing localized pacman output labels, but arch-toolkit uses English-only.

**Solution Options:**
1. Keep custom parsing functions for i18n support
2. Use arch-toolkit and accept English-only limitation
3. Contribute i18n support to arch-toolkit (future enhancement)

**Recommendation:** Keep custom parsing functions if i18n is critical, otherwise migrate to arch-toolkit.

### Challenge 2: Different Dependency Resolution Strategy

**Issue:** Pacsea resolves only direct dependencies (non-recursive), while arch-toolkit supports recursive resolution.

**Solution:** Configure `DependencyResolver` to only resolve direct dependencies, or use arch-toolkit's recursive resolution and filter to direct dependencies only.

### Challenge 3: Custom DependencyInfo Structure

**Issue:** Pacsea's `DependencyInfo` has fields (`required_by`, `depends_on`, `is_core`, `is_system`) that may not map directly to arch-toolkit's `Dependency`.

**Solution:** Create conversion functions and populate custom fields from arch-toolkit's data where possible.

### Challenge 4: Conflict Detection Against Install List

**Issue:** Pacsea checks conflicts against both installed packages and packages in the install list, which may not be built into arch-toolkit.

**Solution:** Keep custom conflict detection logic for install list, use arch-toolkit for installed package conflicts.

### Challenge 5: Dependency Merging Logic

**Issue:** Pacsea has custom logic for merging dependencies by name, keeping worst status, which may not be in arch-toolkit.

**Solution:** Keep custom merging logic if arch-toolkit doesn't handle this, or adapt arch-toolkit's output to match Pacsea's needs.

## Testing Strategy

### Unit Tests
1. Test type conversion functions
2. Test individual migrated functions (querying, version comparison, source determination)
3. Mock arch-toolkit for isolated testing

### Integration Tests
1. Test dependency resolution with real packages
2. Test reverse dependency analysis
3. Test conflict detection
4. Test version comparison edge cases
5. Test source determination for all package types

### Regression Tests
1. Verify dependency resolution results match previous implementation
2. Verify reverse dependency analysis matches
3. Test error handling when pacman is unavailable
4. Test with various package combinations

## Success Criteria

1. ✅ All existing functionality works as before
2. ✅ Code reduction (fewer lines, less complexity)
3. ✅ Improved error handling (arch-toolkit's error types)
4. ✅ Better maintainability (using shared crate)
5. ✅ All tests pass
6. ✅ No performance regression
7. ✅ Clippy and fmt pass
8. ✅ Graceful degradation when pacman is unavailable

## Timeline Estimate

- **Phase 1:** 30 minutes (update dependency)
- **Phase 2:** 2-3 hours (package querying)
- **Phase 3:** 1-2 hours (version comparison)
- **Phase 4:** 2-3 hours (source determination)
- **Phase 5:** 8-12 hours (dependency resolution) ⚠️ Most complex
- **Phase 6:** 3-4 hours (reverse dependencies)
- **Phase 7:** 4-6 hours (cleanup and testing)

**Total:** 21-31 hours (2.5-4 days)

## Risk Assessment

### Low Risk
- Phase 1 (dependency update) - backward compatible
- Phase 3 (version comparison) - direct function replacement

### Medium Risk
- Phase 2 (package querying) - type conversion needed
- Phase 4 (source determination) - type conversion needed
- Phase 6 (reverse dependencies) - type conversion needed

### High Risk
- Phase 5 (dependency resolution) - complex logic differences, extensive type conversion, potential behavior changes

## Rollback Plan

If issues arise:
1. Keep old implementations in separate modules
2. Use feature flag to switch between old/new
3. Or revert commits if needed
4. Can migrate incrementally (one phase at a time)

## Next Steps

1. Review this plan
2. Verify arch-toolkit v0.2.0 API matches expectations
3. Start with Phase 1 (low risk)
4. Proceed incrementally through phases
5. Test thoroughly after each phase
6. Document any deviations from plan

## Notes

- arch-toolkit v0.2.0 is backward compatible with v0.1.2, so AUR operations migration remains intact
- Some custom logic may need to be preserved (i18n, install list conflict detection, custom merging)
- Type conversion layer will be necessary but should be straightforward
- Consider contributing improvements back to arch-toolkit if needed

## References

- [arch-toolkit v0.2.0 Documentation](https://docs.rs/arch-toolkit/0.2.0/arch_toolkit/)
- [arch-toolkit v0.2.0 Release Notes](../arch-toolkit/docs/RELEASE_v0.2.0.md)
- [arch-toolkit CHANGELOG](../arch-toolkit/CHANGELOG.md)
- [Previous AUR Operations Migration Plan](./MIGRATION_PLAN_aur-operations.md)

