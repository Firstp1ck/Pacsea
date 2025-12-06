# Performance Implementation Priority List

This document tracks the status of performance suggestions from `PREFORMANCE_SUGGESTIONS.md` and prioritizes remaining optimizations.

---

## Status Legend
- ✅ **Implemented** - Already in codebase
- ⚠️ **Partially Implemented** - Some aspects exist
- ❌ **Not Implemented** - Candidate for optimization

---

## 1. Search & Filtering Performance

| Suggestion | Status | Current Implementation | Notes |
|------------|--------|------------------------|-------|
| Hash-based indexing for package names | ✅ | `OfficialIndex.name_to_idx: HashMap<String, usize>` | O(1) lookup via `find_package_by_name()` |
| Fuzzy search with trie/BK-tree | ⚠️ | Uses `fuzzy-matcher::SkimMatcherV2` (fzf-style) | Good for fuzzy, but linear scan |
| Search result memoization | ✅ | `search_cache_query`, `search_cache_results` in `AppState` | Caches last query/results pair |
| Reuse matcher instance | ✅ | `fuzzy_match_rank_with_matcher()` accepts shared matcher | `src/util/mod.rs:192` |

**Location**: `src/index/query.rs`, `src/util/mod.rs`

---

## 2. AUR/Repo Data Synchronization

| Suggestion | Status | Current Implementation | Notes |
|------------|--------|------------------------|-------|
| Use BTreeMap/HashMap | ❌ | `OfficialIndex.pkgs: Vec<OfficialPkg>` | Repeated sorting on every search |
| Stream results incrementally | ❌ | Collects all results before rendering | Blocking on full response |
| Lazy loading | ⚠️ | Loads from disk when empty | But not on-demand paging |

**Location**: `src/index/mod.rs`, `src/sources/search.rs`

---

## 3. Installed-Only Mode & Package Removal

| Suggestion | Status | Current Implementation | Notes |
|------------|--------|------------------------|-------|
| HashSet for installed names | ✅ | `INSTALLED_SET: RwLock<HashSet<String>>` | O(1) membership test |
| HashSet for explicit names | ✅ | `EXPLICIT_SET: RwLock<HashSet<String>>` | O(1) membership test |
| Use Vec::retain() efficiently | ⚠️ | Mixed - some use `.iter().any()` | Could optimize list operations |

**Location**: `src/index/mod.rs`, `src/index/installed.rs`

---

## 4. PKGBUILD Parsing & Rendering

| Suggestion | Status | Current Implementation | Notes |
|------------|--------|------------------------|-------|
| Cache PKGBUILDs from yay/paru | ✅ | `get_pkgbuild_from_cache()` tries offline first | Fast local lookup |
| Rate limiting for fetches | ✅ | `PKGBUILD_RATE_LIMITER` with 200ms interval | Prevents server overload |
| Cache parsed PKGBUILD ASTs | ❌ | Re-parses on each access | Could cache parse results |
| Incremental rendering | ❌ | Full render each frame | Syntect highlighting is expensive |

**Location**: `src/sources/pkgbuild.rs`, `src/logic/files/pkgbuild_parse.rs`

---

## 5. Queue Management (Install/Remove/Downgrade Lists)

| Suggestion | Status | Current Implementation | Notes |
|------------|--------|------------------------|-------|
| HashSet for O(1) removal | ✅ | `install_list_names`, `remove_list_names`, `downgrade_list_names: HashSet<String>` | O(1) membership check |
| Efficient deduplication | ✅ | Uses `HashSet::insert()` for O(1) deduplication | Case-insensitive via lowercase keys |

**Current implementation** (`src/logic/lists.rs`):
```rust
// O(1) membership check via HashSet
let name_lower = item.name.to_lowercase();
if !app.install_list_names.insert(name_lower) {
    return; // Already present
}
app.install_list.insert(0, item);
```

---

## 6. Recent Searches Persistence

| Suggestion | Status | Current Implementation | Notes |
|------------|--------|------------------------|-------|
| Bounded LRU cache | ❌ | `Vec<String>` clamped to 20 | Manual truncate |
| O(1) access | ⚠️ | Linear scan for dedup | Uses `.position()` |
| Debounced persistence | ✅ | 2-second debounce window | `src/app/recent.rs` |

**Location**: `src/app/recent.rs`

---

## 7. Sorting Options (best_matches, popularity, alphabetical)

| Suggestion | Status | Current Implementation | Notes |
|------------|--------|------------------------|-------|
| Pre-sort during initial load | ⚠️ | Infrastructure added but not fully utilized | Cache fields exist, invalidation integrated |
| Cache multiple sort orders | ⚠️ | `sort_cache_repo_name`, `sort_cache_aur_popularity` fields added | Cache invalidation works; reordering not implemented |

**Location**: `src/logic/sort.rs`

**Current behavior**: Every call to `sort_results_preserve_selection()` performs a full O(n log n) sort. Cache fields are cleared to maintain consistency. Future optimization: implement cache-based reordering for O(1) sort mode switching.

---

## 8. Ring Prefetch for Details

| Suggestion | Status | Current Implementation | Notes |
|------------|--------|------------------------|-------|
| Proactive detail fetching | ✅ | `ring_prefetch_from_selected()` | Radius of 30 items |
| Cache-aware | ✅ | Skips items in `details_cache` | Avoids redundant fetches |
| Details cache | ✅ | `HashMap<String, PackageDetails>` | O(1) lookup |

**Location**: `src/logic/prefetch.rs`, `src/state/app_state/mod.rs`

---

## Priority Implementation Ranking

Based on **user-facing impact** and **implementation complexity**:

### 🔴 High Priority (High Impact, Moderate Effort)

| # | Optimization | Estimated Impact | Effort | Rationale | Status |
|---|--------------|------------------|--------|-----------|--------|
| 1 | **Hash-based package index** | 🟢 High | Medium | Most user-facing latency is search; O(1) lookup vs O(n) | ✅ **Implemented** |
| 2 | **Install list HashSet** | 🟢 High | Low | Frequent user operation; simple change | ✅ **Implemented** |
| 3 | **Search result memoization** | 🟢 High | Medium | Repeated queries common; cache last N results | ✅ **Implemented** |

### 🟡 Medium Priority (Medium Impact, Low-Medium Effort)

| # | Optimization | Estimated Impact | Effort | Rationale | Status |
|---|--------------|------------------|--------|-----------|--------|
| 4 | **Pre-cached sort orders** | 🟡 Medium | Medium | Sort mode switching is common | ⚠️ **Partially Implemented** (infrastructure added) |
| 5 | **LRU cache for recent searches** | 🟡 Medium | Low | Add `lru` crate; cleaner semantics | ❌ Not Implemented |
| 6 | **Incremental PKGBUILD rendering** | 🟡 Medium | High | Syntect highlighting bottleneck | ❌ Not Implemented |

### 🟢 Low Priority (Lower Impact or Higher Effort)

| # | Optimization | Estimated Impact | Effort | Rationale |
|---|--------------|------------------|--------|-----------|
| 7 | **Stream AUR results incrementally** | 🟢 Low | High | Network latency dominates |
| 8 | **Cache parsed PKGBUILD ASTs** | 🟢 Low | Medium | Parsing is fast; rendering is slower |
| 9 | **Trie/BK-tree for fuzzy search** | 🟢 Low | High | SkimMatcher is already optimized |

---

## Implementation Recommendations

### ✅ Completed Optimizations

1. **✅ Install list name HashSet** (`src/logic/lists.rs`)
   - Added `install_list_names`, `remove_list_names`, `downgrade_list_names: HashSet<String>` to `AppState`
   - Updated `add_to_install_list()`, `add_to_remove_list()`, `add_to_downgrade_list()` to use O(1) HashSet checks
   - Integrated HashSet updates in all removal/clear operations

2. **✅ Search result cache** (`src/state/app_state/mod.rs`)
   - Added `search_cache_query`, `search_cache_fuzzy`, `search_cache_results` fields
   - Implemented cache hit detection in `handle_search_results()`
   - Cache invalidation on fuzzy mode toggle

3. **✅ Package name index HashMap** (`src/index/mod.rs`)
   - Added `name_to_idx: HashMap<String, usize>` to `OfficialIndex`
   - Implemented `rebuild_name_index()` method
   - Updated `find_package_by_name()` to use O(1) HashMap lookup
   - Integrated rebuild into `load_from_disk()`, index fetch/update operations

4. **⚠️ Pre-sorted views** (`src/logic/sort.rs`) - **Infrastructure Added**
   - Added `sort_cache_repo_name`, `sort_cache_aur_popularity`, `sort_cache_signature` fields to `AppState`
   - Implemented `invalidate_sort_caches()` function
   - Integrated cache invalidation into filter/search result changes
   - **Note**: Cache-based reordering not yet implemented (would require storing sorted copies or more complex index mapping)

### Remaining Optimizations

5. **LRU cache for recent searches** - Replace `Vec<String>` with `lru::LruCache`
6. **Incremental PKGBUILD rendering** - Requires architectural changes
7. **Stream AUR results incrementally** - Needs UI redesign

### Long-Term (1+ week)

5. **Incremental UI rendering** - Requires architectural changes to track dirty regions
6. **Streaming AUR results** - Needs UI redesign for progressive display

---

## Benchmarking Approach

Before implementing, establish baselines using `criterion.rs`:

```rust
// benches/search_bench.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_search(c: &mut Criterion) {
    // Seed with 1K, 10K, 100K packages
    c.bench_function("search_1k", |b| {
        b.iter(|| search_official(black_box("ripgrep"), false))
    });
}

criterion_group!(benches, bench_search);
criterion_main!(benches);
```

**Expected scaling patterns:**
- O(n): Time ∝ 10x when packages ∝ 10x ✅ Acceptable
- O(n²): Time ∝ 100x when packages ∝ 10x ⚠️ Optimize
- O(1)/O(log n): Time stable or minimal growth ✅ Ideal

---

## Dependencies to Add (Optional)

| Crate | Purpose | Size Impact |
|-------|---------|-------------|
| `lru` | LRU cache for recent searches | Minimal |
| `hashbrown` | Faster HashMap (optional) | Minimal |
| `criterion` | Benchmarking (dev only) | Dev dependency |

---

## Summary

| Category | Implemented | Partially | Not Implemented |
|----------|-------------|-----------|-----------------|
| Search & Filtering | 3 | 1 | 0 |
| AUR/Repo Sync | 0 | 1 | 2 |
| Installed Mode | 2 | 1 | 0 |
| PKGBUILD | 2 | 0 | 2 |
| Queue Management | 2 | 0 | 0 |
| Recent Searches | 1 | 1 | 1 |
| Sorting | 0 | 2 | 0 |
| Prefetch | 3 | 0 | 0 |
| **Total** | **13** | **6** | **5** |

**Recent implementations**:
- ✅ Hash-based package index (O(1) lookups via `HashMap<String, usize>`)
- ✅ Install/Remove/Downgrade list HashSet optimization (O(1) membership checks)
- ✅ Search result caching (last query/results pair)
- ⚠️ Sort cache infrastructure (fields added, invalidation integrated; reordering not yet implemented)

**Focus areas for maximum impact**: Remaining optimizations include LRU cache for recent searches, incremental PKGBUILD rendering, and completing sort cache-based reordering.

