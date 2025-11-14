<!-- 0bda0a1e-17b8-4f2d-9702-b19aba9fd976 7a81ff9b-6f42-4a6f-ac03-99ed23f103b1 -->
# Code Duplication Reduction Plan

## Overview

This plan identifies major duplication patterns in the Pacsea codebase and proposes refactoring strategies using Rust's advanced features (traits, generics, macros, associated types) to eliminate redundancy while maintaining type safety and performance.

## Identified Duplication Patterns

### 1. Cache Module Duplication (High Priority)

**Location**: `src/app/deps_cache.rs`, `src/app/files_cache.rs`, `src/app/services_cache.rs`, `src/app/sandbox_cache.rs`

**Duplication**:

- Identical `compute_signature()` function across all 4 modules
- Nearly identical `load_cache()` and `save_cache()` functions
- Similar cache structs with `install_list_signature` + data field pattern
- Duplicate test helper functions (`temp_path`, `sample_packages`)

**Solution**: Create a generic cache trait system

- Define `Cacheable` trait with associated type for cached data
- Create generic `Cache<T>` struct using generics
- Implement `compute_signature` once as a generic function
- Use trait bounds to ensure cached types are `Serialize + Deserialize`
- Consolidate test helpers into shared test utilities

**Files to modify**:

- Create `src/app/cache.rs` with generic cache implementation
- Refactor `deps_cache.rs`, `files_cache.rs`, `services_cache.rs`, `sandbox_cache.rs` to use generic cache
- Update all call sites in `src/ui/modals/preflight.rs` and `src/app/mod.rs`

### 2. Details Refresh Function Duplication (Medium Priority)

**Location**: `src/events/utils.rs` - `refresh_install_details()`, `refresh_remove_details()`, `refresh_downgrade_details()`

**Duplication**:

- Nearly identical logic for setting `details_focus`, populating placeholders, checking cache
- Only differences are: selection state field, list field, and index access

**Solution**: Generic function with trait-based selection access

- Create `SelectionAccess` trait to abstract over different selection states
- Create `ListAccess` trait to abstract over different list types
- Implement single `refresh_details()` generic function
- Use associated types to maintain type safety

**Files to modify**:

- `src/events/utils.rs` - refactor three functions into one generic
- Update call sites in event handlers

### 3. Cross-Platform Command Execution Duplication (Medium Priority)

**Location**: `src/util.rs` - `open_file()` and `open_url()`

**Duplication**:

- Similar Windows/Unix branching logic
- Similar fallback patterns
- Similar `stdin/stdout/stderr` nullification

**Solution**: Generic command builder with platform-specific strategies

- Create `CommandBuilder` trait for platform-specific command construction
- Implement Windows and Unix variants
- Use generic function with trait bounds
- Consolidate common command setup logic

**Files to modify**:

- `src/util.rs` - refactor `open_file` and `open_url` to use shared builder
- Consider extracting to `src/util/command.rs` if it grows

### 4. Find Function Duplication (Low Priority)

**Location**: `src/events/utils.rs` - `find_in_recent()` and `find_in_install()`

**Duplication**:

- Similar wrapping logic
- Similar pattern matching logic
- Only difference is the matching criteria (string vs package name/description)

**Solution**: Generic find function with predicate closure

- Create generic `find_in_list()` function
- Accept closure for matching predicate
- Use higher-order functions to abstract matching logic

**Files to modify**:

- `src/events/utils.rs` - consolidate find functions

### 5. Command Setup Pattern Duplication (Low Priority)

**Location**: Multiple files using `.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())`

**Duplication**:

- Repeated command setup patterns in `src/logic/deps/status.rs`, `src/logic/sandbox.rs`, `src/logic/deps/query.rs`

**Solution**: Extension trait for `Command` builder pattern

- Create `CommandExt` trait with helper methods
- Implement `with_null_stdio()`, `with_piped_output()` methods
- Use trait methods to reduce boilerplate

**Files to modify**:

- Create `src/util/command_ext.rs`
- Update command setup sites across logic modules

## Implementation Strategy

### Phase 1: Cache Module Refactoring (Highest Impact)

1. Create `src/app/cache.rs` with generic cache infrastructure
2. Define `Cacheable` trait with associated types
3. Implement generic `Cache<T>` struct
4. Migrate one cache module first (e.g., `deps_cache.rs`) as proof of concept
5. Migrate remaining cache modules
6. Update all call sites
7. Run tests and verify behavior

### Phase 2: Details Refresh Consolidation

1. Create selection/list access traits
2. Refactor `refresh_install_details` to generic version
3. Update call sites incrementally
4. Remove old functions after migration

### Phase 3: Cross-Platform Command Utilities

1. Extract command builder pattern
2. Refactor `open_file` and `open_url`
3. Test on both Windows and Unix platforms

### Phase 4: Minor Duplications

1. Consolidate find functions
2. Add command extension trait
3. Update command setup sites

## Benefits

- Reduced code duplication (~200-300 lines eliminated)
- Improved maintainability (single source of truth)
- Type safety preserved through generics and traits
- Easier to add new cache types or selection sources
- Better testability through trait-based abstractions

## Risks and Mitigation

- **Risk**: Breaking changes during refactoring
- **Mitigation**: Incremental migration, comprehensive tests, verify each phase before proceeding
- **Risk**: Performance impact from trait indirection
- **Mitigation**: Use zero-cost abstractions (monomorphization), benchmark critical paths
- **Risk**: Increased complexity from generics
- **Mitigation**: Clear documentation, type aliases for common cases, examples in doc comments

## Testing Strategy

- Run existing test suite after each phase
- Add integration tests for generic implementations
- Verify cache behavior matches original implementation
- Test cross-platform command execution on both Windows and Unix

### To-dos

- [ ] Design Cacheable trait with associated types for generic cache implementation
- [ ] Implement generic Cache<T> struct and compute_signature function in src/app/cache.rs
- [ ] Migrate deps_cache.rs to use generic cache as proof of concept
- [ ] Migrate files_cache.rs, services_cache.rs, and sandbox_cache.rs to generic cache
- [ ] Update all cache call sites in src/ui/modals/preflight.rs and src/app/mod.rs
- [ ] Create SelectionAccess and ListAccess traits for generic details refresh
- [ ] Refactor refresh_install_details, refresh_remove_details, refresh_downgrade_details into single generic function
- [ ] Create CommandBuilder trait and platform-specific implementations for cross-platform commands
- [ ] Refactor open_file and open_url to use CommandBuilder pattern
- [ ] Consolidate find_in_recent and find_in_install into generic find function with predicate closure
- [ ] Create CommandExt trait with helper methods for common command setup patterns
- [ ] Apply CommandExt to command setup sites in logic modules