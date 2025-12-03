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
| Hash-based indexing for package names | ❌ | `OfficialIndex` uses `Vec<OfficialPkg>` with linear search | Could add `HashMap<String, usize>` index |
| Fuzzy search with trie/BK-tree | ⚠️ | Uses `fuzzy-matcher::SkimMatcherV2` (fzf-style) | Good for fuzzy, but linear scan |
| Search result memoization | ❌ | No caching layer for identical queries | Could benefit repeated searches |
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
| HashSet for O(1) removal | ❌ | `Vec<PackageItem>` with `.iter().any()` | Linear scan for dedup |
| Efficient deduplication | ⚠️ | Case-insensitive check exists but O(n) | |

**Current code** (`src/logic/lists.rs`):
```rust
// O(n) membership check
if app.install_list.iter().any(|p| p.name.eq_ignore_ascii_case(&item.name)) {
    return;
}
```

**Suggested optimization**:
```rust
// Auxiliary HashSet for O(1) check
let mut install_set: HashSet<String> = HashSet::new();
if !install_set.insert(item.name.to_lowercase()) {
    return; // Already present
}
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
| Pre-sort during initial load | ❌ | Sorts on every mode switch | O(n log n) each time |
| Cache multiple sort orders | ❌ | Single `results` Vec | Could maintain parallel sorted views |

**Location**: `src/logic/sort.rs`

**Current behavior**: Every call to `sort_results_preserve_selection()` performs a full O(n log n) sort.

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

| # | Optimization | Estimated Impact | Effort | Rationale |
|---|--------------|------------------|--------|-----------|
| 1 | **Hash-based package index** | 🟢 High | Medium | Most user-facing latency is search; O(1) lookup vs O(n) |
| 2 | **Install list HashSet** | 🟢 High | Low | Frequent user operation; simple change |
| 3 | **Search result memoization** | 🟢 High | Medium | Repeated queries common; cache last N results |

### 🟡 Medium Priority (Medium Impact, Low-Medium Effort)

| # | Optimization | Estimated Impact | Effort | Rationale |
|---|--------------|------------------|--------|-----------|
| 4 | **Pre-cached sort orders** | 🟡 Medium | Medium | Sort mode switching is common |
| 5 | **LRU cache for recent searches** | 🟡 Medium | Low | Add `lru` crate; cleaner semantics |
| 6 | **Incremental PKGBUILD rendering** | 🟡 Medium | High | Syntect highlighting bottleneck |

### 🟢 Low Priority (Lower Impact or Higher Effort)

| # | Optimization | Estimated Impact | Effort | Rationale |
|---|--------------|------------------|--------|-----------|
| 7 | **Stream AUR results incrementally** | 🟢 Low | High | Network latency dominates |
| 8 | **Cache parsed PKGBUILD ASTs** | 🟢 Low | Medium | Parsing is fast; rendering is slower |
| 9 | **Trie/BK-tree for fuzzy search** | 🟢 Low | High | SkimMatcher is already optimized |

---

## Implementation Recommendations

### Immediate Wins (< 1 day each)

1. **Add install list name HashSet** (`src/logic/lists.rs`)
   ```rust
   // Add to AppState
   pub install_names: HashSet<String>,
   
   // Update add_to_install_list()
   if !app.install_names.insert(item.name.to_lowercase()) {
       return; // Already present
   }
   app.install_list.insert(0, item);
   ```

2. **Add search result cache** (`src/state/app_state/mod.rs`)
   ```rust
   // Simple last-query cache
   pub last_search_query: Option<String>,
   pub last_search_results: Option<Vec<PackageItem>>,
   ```

### Medium-Term (1-3 days each)

3. **Package name index HashMap**
   ```rust
   // In OfficialIndex
   pub name_to_idx: HashMap<String, usize>,
   
   // Build during index load
   for (i, pkg) in pkgs.iter().enumerate() {
       name_to_idx.insert(pkg.name.to_lowercase(), i);
   }
   ```

4. **Pre-sorted views**
   ```rust
   // In AppState or a separate cache
   pub sorted_by_name: Vec<usize>,      // indices into results
   pub sorted_by_popularity: Vec<usize>,
   pub sorted_by_match: Vec<usize>,
   ```

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
| Search & Filtering | 1 | 1 | 2 |
| AUR/Repo Sync | 0 | 1 | 2 |
| Installed Mode | 2 | 1 | 0 |
| PKGBUILD | 2 | 0 | 2 |
| Queue Management | 0 | 1 | 1 |
| Recent Searches | 1 | 1 | 1 |
| Sorting | 0 | 0 | 2 |
| Prefetch | 3 | 0 | 0 |
| **Total** | **9** | **5** | **10** |

**Focus areas for maximum impact**: Search indexing, install list optimization, and sort caching.

