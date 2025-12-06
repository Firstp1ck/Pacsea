//! Disk-persisted LRU cache for parsed PKGBUILD data.

use crate::logic::files::pkgbuild_parse::{
    parse_backup_from_pkgbuild, parse_install_paths_from_pkgbuild,
};
use crate::state::Source;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;
use std::path::PathBuf;
#[cfg(test)]
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};
#[cfg(test)]
use std::thread::ThreadId;

const CACHE_CAPACITY: usize = 200;
const CACHE_PATH_ENV: &str = "PACSEA_PKGBUILD_CACHE_PATH";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PkgbuildSourceKind {
    Aur,
    Official,
    Unknown,
}

impl From<&Source> for PkgbuildSourceKind {
    fn from(src: &Source) -> Self {
        match src {
            Source::Aur => Self::Aur,
            Source::Official { .. } => Self::Official,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PkgbuildParseEntry {
    pub name: String,
    pub version: String,
    pub source: PkgbuildSourceKind,
    pub pkgbuild_signature: u64,
    pub backup_files: Vec<String>,
    pub install_paths: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PkgbuildCacheDisk {
    entries: Vec<PkgbuildParseEntry>,
}

#[derive(Debug)]
struct PkgbuildCacheState {
    lru: LruCache<String, PkgbuildParseEntry>,
    path: PathBuf,
    dirty: bool,
}

impl PkgbuildCacheState {
    fn new(path: PathBuf) -> Self {
        Self {
            lru: LruCache::new(
                NonZeroUsize::new(CACHE_CAPACITY)
                    .unwrap_or_else(|| NonZeroUsize::new(1).expect("non-zero capacity")),
            ),
            path,
            dirty: false,
        }
    }

    fn load_from_disk(&mut self) {
        let raw = match fs::read_to_string(&self.path) {
            Ok(raw) => raw,
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!(
                        path = %self.path.display(),
                        error = %e,
                        "[PKGBUILD cache] Failed to read cache file"
                    );
                }
                return;
            }
        };

        let parsed: PkgbuildCacheDisk = match serde_json::from_str(&raw) {
            Ok(cache) => cache,
            Err(e) => {
                tracing::warn!(
                    path = %self.path.display(),
                    error = %e,
                    "[PKGBUILD cache] Failed to parse cache file"
                );
                return;
            }
        };

        // Insert from least-recent to most-recent to preserve order when iterating.
        for entry in parsed.entries.into_iter().rev() {
            let key = cache_key(&entry.name, &entry.version, entry.source);
            let _ = self.lru.put(key, entry);
        }
        tracing::info!(
            path = %self.path.display(),
            count = self.lru.len(),
            "[PKGBUILD cache] Loaded cache entries"
        );
    }

    fn flush_if_dirty(&mut self) {
        if !self.dirty {
            return;
        }

        let payload = PkgbuildCacheDisk {
            entries: self.lru.iter().map(|(_, v)| v.clone()).collect(),
        };

        let Ok(serialized) = serde_json::to_string(&payload) else {
            tracing::warn!("[PKGBUILD cache] Failed to serialize cache payload");
            return;
        };

        if let Some(parent) = self.path.parent()
            && let Err(e) = fs::create_dir_all(parent)
        {
            tracing::warn!(
                path = %self.path.display(),
                error = %e,
                "[PKGBUILD cache] Failed to create parent directory"
            );
            return;
        }

        match fs::write(&self.path, serialized) {
            Ok(()) => {
                tracing::debug!(
                    path = %self.path.display(),
                    entries = self.lru.len(),
                    "[PKGBUILD cache] Persisted cache to disk"
                );
                self.dirty = false;
            }
            Err(e) => {
                tracing::warn!(
                    path = %self.path.display(),
                    error = %e,
                    "[PKGBUILD cache] Failed to write cache to disk"
                );
            }
        }
    }
}

fn cache_path() -> PathBuf {
    if let Ok(path) = std::env::var(CACHE_PATH_ENV) {
        return PathBuf::from(path);
    }
    crate::theme::lists_dir().join("pkgbuild_parse_cache.json")
}

fn cache_state() -> &'static Mutex<PkgbuildCacheState> {
    static STATE: OnceLock<Mutex<PkgbuildCacheState>> = OnceLock::new();
    STATE.get_or_init(|| {
        let path = cache_path();
        let mut state = PkgbuildCacheState::new(path);
        state.load_from_disk();
        Mutex::new(state)
    })
}

fn compute_signature(contents: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    contents.hash(&mut hasher);
    hasher.finish()
}

fn cache_key(name: &str, version: &str, source: PkgbuildSourceKind) -> String {
    format!("{name}::{version}::{source:?}")
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::logic::files) enum CacheTestHookPoint {
    AfterLookup,
}

#[cfg(test)]
pub(in crate::logic::files) type CacheTestHook = dyn Fn(CacheTestHookPoint) + Send + Sync + 'static;

#[cfg(test)]
#[derive(Clone)]
struct CacheTestHookEntry {
    hook: Arc<CacheTestHook>,
    thread_id: ThreadId,
}

#[cfg(test)]
fn cache_test_hook_slot() -> &'static Mutex<Option<CacheTestHookEntry>> {
    static HOOK: OnceLock<Mutex<Option<CacheTestHookEntry>>> = OnceLock::new();
    HOOK.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
/// What: Temporarily register a cache test hook for synchronization.
///
/// Inputs:
/// - `hook`: Callback executed when a cache hook point is reached.
/// - `thread_id`: Thread id to match before invoking the hook.
///
/// Output:
/// - Guard that clears the hook on drop to restore default behavior.
///
/// Details:
/// - Only compiled in tests; the hook is global and not re-entrant.
pub fn set_cache_test_hook(hook: Arc<CacheTestHook>, thread_id: ThreadId) -> CacheTestHookGuard {
    if let Ok(mut slot) = cache_test_hook_slot().lock() {
        *slot = Some(CacheTestHookEntry { hook, thread_id });
    }
    CacheTestHookGuard
}

#[cfg(test)]
/// What: RAII guard that removes the active cache test hook on drop.
///
/// Inputs: None.
///
/// Output:
/// - Clears any registered test hook when dropped.
///
/// Details:
/// - Scope the guard to the duration the hook should stay active.
pub struct CacheTestHookGuard;

#[cfg(test)]
impl Drop for CacheTestHookGuard {
    fn drop(&mut self) {
        if let Ok(mut slot) = cache_test_hook_slot().lock() {
            slot.take();
        }
    }
}

#[cfg(test)]
fn invoke_cache_test_hook(point: CacheTestHookPoint) {
    // Clone hook entry and release slot mutex before invoking so that other threads
    // can still check the hook slot while this thread is blocked inside the callback.
    let entry = cache_test_hook_slot()
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    if let Some(hook) = entry
        && std::thread::current().id() == hook.thread_id
    {
        (hook.hook)(point);
    }
}

/// What: Parse PKGBUILD data while leveraging a disk-backed LRU cache.
///
/// Inputs:
/// - `name`: Package name used for keying and install path inference.
/// - `version`: Package version (fall back to `"unknown"` if empty).
/// - `source`: Source kind for keying (Aur/Official/Unknown).
/// - `pkgbuild`: Raw PKGBUILD text to parse.
///
/// Output:
/// - Parsed entry containing backup files and install paths. On cache hit with matching
///   signature, returns the cached entry. On cache miss or signature mismatch, parses
///   fresh data, updates the cache, and returns the new entry.
///
/// Details:
/// - Uses a signature of the PKGBUILD text to detect staleness even when version is unchanged.
/// - Cache is bounded to 200 entries and persists to disk via `flush_pkgbuild_cache()`.
pub fn parse_pkgbuild_cached(
    name: &str,
    version: Option<&str>,
    source: PkgbuildSourceKind,
    pkgbuild: &str,
) -> PkgbuildParseEntry {
    let normalized_version = version
        .filter(|v| !v.is_empty())
        .map_or_else(|| "unknown".to_string(), ToString::to_string);
    let signature = compute_signature(pkgbuild);
    let key = cache_key(name, &normalized_version, source);
    let prior_signature = if let Ok(mut guard) = cache_state().lock()
        && let Some(entry) = guard.lru.get(&key)
    {
        if entry.pkgbuild_signature == signature {
            return entry.clone();
        }
        Some(entry.pkgbuild_signature)
    } else {
        None
    };

    #[cfg(test)]
    invoke_cache_test_hook(CacheTestHookPoint::AfterLookup);

    let parsed = PkgbuildParseEntry {
        name: name.to_string(),
        version: normalized_version,
        source,
        pkgbuild_signature: signature,
        backup_files: parse_backup_from_pkgbuild(pkgbuild),
        install_paths: parse_install_paths_from_pkgbuild(pkgbuild, name),
    };

    let mut guard = match cache_state().lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            tracing::warn!(
                "[PKGBUILD cache] Cache mutex poisoned; continuing with recovered state"
            );
            poisoned.into_inner()
        }
    };

    if let Some(entry) = guard.lru.get(&key) {
        if entry.pkgbuild_signature == signature {
            return entry.clone();
        }

        if prior_signature.is_some() && prior_signature == Some(entry.pkgbuild_signature) {
            let _ = guard.lru.put(key, parsed.clone());
            guard.dirty = true;
            return parsed;
        }

        return entry.clone();
    }

    let _ = guard.lru.put(key, parsed.clone());
    guard.dirty = true;

    parsed
}

/// What: Persist the PKGBUILD parse cache to disk when dirty.
///
/// Inputs: None.
///
/// Output:
/// - Best-effort disk write of the cache file; clears the dirty flag on success.
///
/// Details:
/// - Safe to call frequently; returns immediately when nothing has changed.
pub fn flush_pkgbuild_cache() {
    if let Ok(mut guard) = cache_state().lock() {
        guard.flush_if_dirty();
    }
}

#[cfg(test)]
pub fn reset_cache_for_tests(path: PathBuf) {
    if let Ok(mut guard) = cache_state().lock() {
        let mut state = PkgbuildCacheState::new(path);
        state.load_from_disk();
        *guard = state;
    }
}

#[cfg(test)]
pub fn peek_cache_entry_for_tests(
    name: &str,
    version: &str,
    source: PkgbuildSourceKind,
) -> Option<PkgbuildParseEntry> {
    let key = cache_key(name, version, source);
    cache_state()
        .lock()
        .ok()
        .and_then(|mut guard| guard.lru.get(&key).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Barrier, mpsc};
    use std::time::Duration;

    fn sample_pkgbuild() -> String {
        r#"
pkgname=sample
pkgver=1.2.3
pkgrel=1
backup=('etc/sample.conf' '/etc/sample.d/more.conf')
package() {
  install -Dm755 "$srcdir/sample" "$pkgdir/usr/bin/sample"
  install -Dm644 "$srcdir/sample.conf" "$pkgdir/etc/sample.conf"
}
"#
        .to_string()
    }

    fn temp_cache_path(label: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_pkgb_cache_{label}_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time ok")
                .as_nanos()
        ));
        path
    }

    #[test]
    fn cache_hit_returns_same_signature_entry() {
        let path = temp_cache_path("hit");
        reset_cache_for_tests(path);
        let text = sample_pkgbuild();
        let entry = parse_pkgbuild_cached("sample", Some("1.2.3"), PkgbuildSourceKind::Aur, &text);
        assert!(entry.backup_files.contains(&"etc/sample.conf".to_string()));
        assert!(entry.install_paths.contains(&"/usr/bin/sample".to_string()));
        let hit = parse_pkgbuild_cached("sample", Some("1.2.3"), PkgbuildSourceKind::Aur, &text);
        assert_eq!(hit.pkgbuild_signature, entry.pkgbuild_signature);
        assert_eq!(hit.install_paths, entry.install_paths);
    }

    #[test]
    fn cache_miss_on_signature_change_reparses() {
        let path = temp_cache_path("miss");
        reset_cache_for_tests(path);
        let text = sample_pkgbuild();
        let _ = parse_pkgbuild_cached("sample", Some("1.2.3"), PkgbuildSourceKind::Official, &text);
        let modified = format!("{text}\n# change");
        let updated = parse_pkgbuild_cached(
            "sample",
            Some("1.2.3"),
            PkgbuildSourceKind::Official,
            &modified,
        );
        assert!(updated.pkgbuild_signature != compute_signature(&text));
    }

    #[test]
    fn flush_and_reload_persists_entries() {
        let path = temp_cache_path("persist");
        reset_cache_for_tests(path.clone());
        let text = sample_pkgbuild();
        let entry = parse_pkgbuild_cached("sample", Some("1.2.3"), PkgbuildSourceKind::Aur, &text);
        flush_pkgbuild_cache();
        reset_cache_for_tests(path);
        let cached = peek_cache_entry_for_tests("sample", "1.2.3", PkgbuildSourceKind::Aur)
            .expect("entry should reload");
        assert_eq!(cached.pkgbuild_signature, entry.pkgbuild_signature);
        assert_eq!(cached.backup_files, entry.backup_files);
    }

    #[test]
    fn cache_evicts_oldest_when_capacity_exceeded() {
        let path = temp_cache_path("evict");
        reset_cache_for_tests(path);
        let text = sample_pkgbuild();
        for i in 0..(CACHE_CAPACITY + 5) {
            let name = format!("pkg{i}");
            parse_pkgbuild_cached(&name, Some("1"), PkgbuildSourceKind::Unknown, &text);
        }
        assert!(
            peek_cache_entry_for_tests("pkg0", "1", PkgbuildSourceKind::Unknown).is_none(),
            "oldest entry should be evicted past capacity"
        );
    }

    #[test]
    fn concurrent_parse_does_not_overwrite_newer_entry() {
        let path = temp_cache_path("concurrent");
        reset_cache_for_tests(path);
        let name = "racepkg";
        let stale_pkgbuild = sample_pkgbuild();
        let newer_pkgbuild = r#"
pkgname=sample
pkgver=9.9.9
pkgrel=1
backup=('etc/sample.conf')
package() {
  install -Dm755 "$srcdir/sample" "$pkgdir/usr/bin/sample"
  install -Dm644 "$srcdir/sample.conf" "$pkgdir/etc/sample.conf"
}
"#
        .to_string();

        let (reached_tx, reached_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let resume_rx = Arc::new(Mutex::new(resume_rx));
        let hook_consumed = Arc::new(AtomicBool::new(false));
        let hook_flag = Arc::clone(&hook_consumed);
        let hook_resume = Arc::clone(&resume_rx);
        let hook = Arc::new(move |point: CacheTestHookPoint| {
            if point == CacheTestHookPoint::AfterLookup
                && hook_flag
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
            {
                let _ = reached_tx.send(());
                hook_resume
                    .lock()
                    .expect("resume_rx lock poisoned")
                    .recv()
                    .expect("resume signal should arrive");
            }
        });
        let start_barrier = Arc::new(Barrier::new(2));

        let stale_pkgbuild_for_thread = stale_pkgbuild.clone();
        let stale_start = Arc::clone(&start_barrier);
        let stale_handle = std::thread::spawn(move || {
            stale_start.wait();
            parse_pkgbuild_cached(
                name,
                Some("1.2.3"),
                PkgbuildSourceKind::Aur,
                &stale_pkgbuild_for_thread,
            )
        });

        let stale_thread_id = stale_handle.thread().id();
        let _guard = set_cache_test_hook(hook, stale_thread_id);
        start_barrier.wait();

        reached_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("stale thread should reach hook before proceeding");

        let newer_pkgbuild_for_thread = newer_pkgbuild.clone();
        let new_handle = std::thread::spawn(move || {
            parse_pkgbuild_cached(
                name,
                Some("1.2.3"),
                PkgbuildSourceKind::Aur,
                &newer_pkgbuild_for_thread,
            )
        });

        let new_entry = new_handle
            .join()
            .expect("new parsing thread should finish without panic");
        resume_tx
            .send(())
            .expect("should release stale thread after new parse completes");
        let stale_entry = stale_handle
            .join()
            .expect("stale parsing thread should finish without panic");

        let cached = peek_cache_entry_for_tests(name, "1.2.3", PkgbuildSourceKind::Aur)
            .expect("cache entry should exist after concurrent parses");
        let stale_signature = compute_signature(&stale_pkgbuild);
        let new_signature = compute_signature(&newer_pkgbuild);

        assert_eq!(
            cached.pkgbuild_signature, new_signature,
            "newer entry must remain in cache"
        );
        assert_eq!(
            cached.pkgbuild_signature, new_entry.pkgbuild_signature,
            "cache entry should match result of newer parse"
        );
        assert_ne!(
            cached.pkgbuild_signature, stale_signature,
            "stale parse must not overwrite newer cache entry"
        );
        // When the stale thread loses the race, it should return the cached (newer)
        // entry rather than its own stale parse result.
        assert_eq!(
            stale_entry.pkgbuild_signature, new_entry.pkgbuild_signature,
            "stale thread should return cached newer entry after losing race"
        );
        assert_ne!(
            stale_signature, new_signature,
            "test setup should use distinct PKGBUILD contents"
        );
    }
}
