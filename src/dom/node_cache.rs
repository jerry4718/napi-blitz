//! NodeCache — Rust-side weak reference cache for JS Node wrapper objects.
//!
//! Solves the "identity stability" problem: when `querySelector("div")`
//! is called multiple times, each call must return the *same* JS object
//! as long as the previous one hasn't been garbage-collected.
//!
//! ## Design
//!
//! Each cache entry is a weak `napi_ref` with refcount=0. This does not
//! prevent V8 from GCing the JS object. When the JS object is GCed, the
//! finalizer callback fires and removes the entry from the cache.
//!
//! This replaces the JS-side `Map<bigint, WeakRef<Node>>` +
//! `FinalizationRegistry` pattern with a Rust-side equivalent that:
//! - Can be accessed directly from Rust (no JS callback round-trip)
//! - Integrates with the ListenerStore (check if a node has listeners
//!   before deciding to dispatch)
//! - Avoids the GC-timing uncertainty of JS `FinalizationRegistry`
//!
//! ## Unsafe operations
//!
//! All `napi_ref` and `napi_add_finalizer` calls go through the `raw`
//! module. The `NodeCache` itself contains no `unsafe` code.
//!
//! ## Future convergence
//!
//! When Node wrappers become `#[napi]` classes, we can switch from
//! raw `napi_ref` to the safe `WeakReference<T>` / `Reference<T>` API
//! from `napi::bindgen_runtime`. The `raw` module calls will then be
//! removed.

use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr;

use napi::{sys, Env, Result};

use crate::dom::raw;

/// A weak reference to a JS object, backed by a `napi_ref` with
/// refcount=0. Does not prevent GC. When the JS object is GCed, the
/// finalizer fires and the cache entry is removed.
pub struct WeakNodeEntry {
    /// `napi_ref` with refcount=0 (weak). Deleted on removal.
    napi_ref: sys::napi_ref,
    /// The `napi_env` at creation time. Used for `delete_reference`
    /// and `get_reference_value`.
    env: sys::napi_env,
}

impl WeakNodeEntry {
    /// Retrieve the JS object's `napi_value`, or `None` if GC'd.
    pub fn get_value(&self) -> Result<Option<sys::napi_value>> {
        raw::get_reference_value(self.env, self.napi_ref)
    }
}

impl Drop for WeakNodeEntry {
    fn drop(&mut self) {
        // Best-effort cleanup; ignore errors (env may be shutting down).
        let _ = raw::delete_reference(self.env, self.napi_ref);
    }
}

/// Per-document cache: `node_id → WeakNodeEntry`.
///
/// Stored on `SharedBridge` (or `SharedBaseDoc`) so both `DocHandle`
/// and `WindowDocument` can access it.
pub struct NodeCache {
    entries: HashMap<usize, WeakNodeEntry>,
}

impl Default for NodeCache {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Look up a cached JS Node object by node_id.
    ///
    /// Returns the `napi_value` if the cache has a live (non-GC'd)
    /// entry, or `None` if not cached or already GC'd.
    ///
    /// If the entry was GC'd (napi_ref returns null), it is removed
    /// from the cache lazily.
    pub fn get(&mut self, env: &Env, node_id: usize) -> Result<Option<sys::napi_value>> {
        let Some(entry) = self.entries.get(&node_id) else {
            return Ok(None);
        };
        let value = entry.get_value()?;
        if value.is_none() {
            // Object was GC'd. Remove the stale entry.
            self.entries.remove(&node_id);
        }
        Ok(value)
    }

    /// Cache a newly created JS Node object for `node_id`.
    ///
    /// Creates a weak `napi_ref` (refcount=0) and registers a finalizer
    /// that removes the entry when the JS object is GC'd.
    ///
    /// If an entry already exists (e.g. race), it is replaced.
    pub fn insert(
        &mut self,
        env: &Env,
        node_id: usize,
        js_value: sys::napi_value,
    ) -> Result<()> {
        // Clean up any existing entry for this node_id.
        self.entries.remove(&node_id);

        // Create a weak reference (refcount=0 → does not prevent GC).
        let napi_ref = raw::create_reference(env.raw(), js_value, 0)?;

        // Register a finalizer that removes this entry when the JS
        // object is GC'd. We pass `node_id` as the finalize data.
        //
        // The finalizer callback is a module-level `extern "C"` fn that
        // calls `NodeCache::on_finalize`. We can't pass `&mut self` to
        // an extern "C" callback, so we use a thread-local pointer to
        // the active NodeCache. This is safe because Node.js is
        // single-threaded and the finalizer runs on the main thread.
        let finalize_data = Box::into_raw(Box::new(node_id)) as *mut c_void;
        let _finalizer_ref = raw::add_finalizer(
            env.raw(),
            js_value,
            finalize_data,
            Some(node_finalize_callback),
            ptr::null_mut(),
        )?;

        self.entries.insert(
            node_id,
            WeakNodeEntry {
                napi_ref,
                env: env.raw(),
            },
        );
        Ok(())
    }

    /// Remove a cache entry (called from finalizer or manual cleanup).
    pub fn remove(&mut self, node_id: usize) {
        self.entries.remove(&node_id);
    }

    /// Check whether a node_id has a live cached entry.
    pub fn contains(&mut self, env: &Env, node_id: usize) -> Result<bool> {
        Ok(self.get(env, node_id)?.is_some())
    }

    /// Clear all entries (doc teardown).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Sweep: remove entries whose JS objects have been GC'd.
    pub fn sweep(&mut self, env: &Env) -> Result<()> {
        let mut stale = Vec::new();
        for (&node_id, entry) in &self.entries {
            if entry.get_value()?.is_none() {
                stale.push(node_id);
            }
        }
        for node_id in stale {
            self.entries.remove(&node_id);
        }
        Ok(())
    }

    /// Number of cached entries (including possibly-stale ones).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ── Finalizer callback ──────────────────────────────────────────────

/// The finalizer callback registered via `napi_add_finalizer`.
///
/// When V8 GCs a JS Node object, this function fires. It reads the
/// `node_id` from `finalize_data`, then removes the corresponding
/// entry from the thread-local `NodeCache`.
///
/// **Thread-local approach:** Because `napi_add_finalizer` callbacks
/// are plain `extern "C"` functions (no closure captures), we use a
/// thread-local `RefCell<Option<*mut NodeCache>>` to reach the active
/// cache. This is safe because:
/// 1. Node.js is single-threaded (JS + GC + finalizers all run on the
///    main thread).
/// 2. The pointer is set before any JS object with a finalizer is
///    created, and cleared only after all such objects are dropped.
thread_local! {
    static ACTIVE_CACHE: std::cell::RefCell<Option<*mut NodeCache>> = std::cell::RefCell::new(None);
}

/// Set the thread-local active NodeCache pointer. Called before any
/// JS code runs that might create or GC Node objects.
///
/// # Safety
/// The caller must ensure `cache` outlives all finalizer-registered
/// JS objects. In practice, `cache` lives on `SharedBridge` which
/// outlives the `DocHandle`.
pub unsafe fn set_active_cache(cache: &mut NodeCache) {
    ACTIVE_CACHE.with(|cell| {
        *cell.borrow_mut() = Some(cache as *mut NodeCache);
    });
}

/// Clear the thread-local active NodeCache pointer.
pub fn clear_active_cache() {
    ACTIVE_CACHE.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// The `extern "C"` finalizer callback. Registered on each cached JS
/// Node object via `napi_add_finalizer`.
extern "C" fn node_finalize_callback(
    _env: sys::napi_env,
    finalize_data: *mut c_void,
    _finalize_hint: *mut c_void,
) {
    // Reclaim the node_id from the boxed pointer.
    let node_id = unsafe { *Box::from_raw(finalize_data as *mut usize) };

    // Try to remove the entry from the active cache.
    ACTIVE_CACHE.with(|cell| {
        if let Some(cache_ptr) = *cell.borrow() {
            // SAFETY: the pointer was set by `set_active_cache` and is
            // valid as long as the SharedBridge is alive. The
            // finalizer only fires while the env is alive, which means
            // the SharedBridge is also alive.
            let cache = unsafe { &mut *cache_ptr };
            cache.remove(node_id);
        }
    });
}
