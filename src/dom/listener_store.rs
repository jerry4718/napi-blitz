//! ListenerStore — Rust-side event listener storage indexed by node id.
//!
//! Inspired by blitz-boa-gui's `ListenerStore`. Listeners are stored in
//! Rust, independent of JS Node wrapper lifetimes. Each listener's
//! callback is held as a strong `napi_ref` (prevents GC until
//! `remove_listener` or doc teardown).
//!
//! **Note on callback storage:** We store callbacks as raw `napi_ref`
//! (via `raw::create_reference`) rather than `FunctionRef<Args, Return>`
//! because `FunctionRef`'s generic type parameters require `'static`
//! types, and our Event object is not yet a `#[napi]` class. Once we
//! have a `#[napi] struct BlitzEvent`, we can converge to the safe
//! `FunctionRef<BlitzEvent, ()>` API. All unsafe operations are
//! encapsulated in the `raw` module.

use std::collections::HashMap;

use napi::{Env, Result};
use napi_derive::napi;

use crate::dom::raw;

/// Options for `addEventListener`, mirroring the DOM spec.
#[napi(object)]
#[derive(Clone, Debug, Default)]
pub struct AddEventListenerOptions {
    /// Capture phase listener.
    pub capture: bool,
    /// Automatically removed after first invocation.
    pub once: bool,
    /// Listener will never call `preventDefault` (hint to the engine).
    pub passive: bool,
}

/// A unique id for a registered listener.
pub type ListenerId = u64;

static LISTENER_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn next_listener_id() -> ListenerId {
    LISTENER_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// One registered listener. Holds a strong reference to the JS callback
/// via `napi_ref` (refcount=1), preventing GC until removed.
pub struct StoredListener {
    pub id: ListenerId,
    pub event_type: String,
    /// Strong reference to the JS function. refcount=1, prevents GC.
    /// Created via `raw::create_reference(env, value, 1)`.
    /// Deleted via `raw::delete_reference` on Drop.
    callback_ref: napi::sys::napi_ref,
    /// Cached `napi_env` for operations on the ref.
    env: napi::sys::napi_env,
    pub options: AddEventListenerOptions,
    pub removed: bool,
}

impl Drop for StoredListener {
    fn drop(&mut self) {
        // Release the napi_ref when the listener is dropped.
        let _ = raw::delete_reference(self.env, self.callback_ref);
    }
}

impl std::fmt::Debug for StoredListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredListener")
            .field("id", &self.id)
            .field("event_type", &self.event_type)
            .field("options", &self.options)
            .field("removed", &self.removed)
            .finish()
    }
}

impl StoredListener {
    /// Invoke the callback with the given event `napi_value`.
    /// `this` is set to `undefined`.
    pub fn invoke(&self, env: &Env, event_value: napi::sys::napi_value) -> Result<()> {
        let callback_value = raw::get_reference_value(env.raw(), self.callback_ref)?;
        let Some(cb) = callback_value else {
            return Ok(()); // callback was GC'd (shouldn't happen with strong ref)
        };

        let args = [event_value];
        raw::call_function(env.raw(), cb, &args)?;
        Ok(())
    }

    /// Get the callback's `napi_value` for identity comparison.
    fn callback_value(&self, env: &Env) -> Result<napi::sys::napi_value> {
        raw::get_reference_value(env.raw(), self.callback_ref)?
            .ok_or_else(|| {
                napi::Error::new(
                    napi::Status::GenericFailure,
                    "listener callback was garbage collected".to_string(),
                )
            })
    }
}

/// Per-node listener storage: `node_id → Vec<StoredListener>`.
pub struct ListenerStore {
    entries: HashMap<usize, Vec<StoredListener>>,
}

impl Default for ListenerStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ListenerStore {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Register a listener for `node_id`. Returns `false` if an identical
    /// (same event_type + same callback + same capture) listener already
    /// exists (DOM spec: addEventListener is idempotent).
    pub fn add(
        &mut self,
        env: &Env,
        node_id: usize,
        event_type: String,
        callback_napi_value: napi::sys::napi_value,
        options: AddEventListenerOptions,
    ) -> Result<bool> {
        let listeners = self.entries.entry(node_id).or_default();

        // Dedup: same event_type + same callback object + same capture
        for l in listeners.iter() {
            if l.removed || l.event_type != event_type || l.options.capture != options.capture {
                continue;
            }
            let stored_value = l.callback_value(env)?;
            let is_same =
                raw::strict_equals(env.raw(), stored_value, callback_napi_value)?;
            if is_same {
                return Ok(false);
            }
        }

        // Create a strong reference (refcount=1) to the callback.
        let callback_ref = raw::create_reference(env.raw(), callback_napi_value, 1)?;

        listeners.push(StoredListener {
            id: next_listener_id(),
            event_type,
            callback_ref,
            env: env.raw(),
            options,
            removed: false,
        });
        Ok(true)
    }

    /// Remove a listener by identity (same event_type + same callback +
    /// same capture). Marks as removed; the `StoredListener` is dropped
    /// (releasing the napi_ref) during `compact()`.
    pub fn remove(
        &mut self,
        env: &Env,
        node_id: usize,
        event_type: &str,
        callback_napi_value: napi::sys::napi_value,
        capture: bool,
    ) -> Result<bool> {
        let Some(listeners) = self.entries.get_mut(&node_id) else {
            return Ok(false);
        };
        for l in listeners.iter_mut() {
            if l.removed || l.event_type != event_type || l.options.capture != capture {
                continue;
            }
            let stored_value = l.callback_value(env)?;
            let is_same =
                raw::strict_equals(env.raw(), stored_value, callback_napi_value)?;
            if is_same {
                l.removed = true;
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Return all matching (non-removed) listener ids for a given node,
    /// event type, and capture flag.
    pub fn matching_ids(
        &self,
        node_id: usize,
        event_type: &str,
        capture: bool,
    ) -> Vec<ListenerId> {
        self.entries
            .get(&node_id)
            .map(|listeners| {
                listeners
                    .iter()
                    .filter(|l| !l.removed && l.event_type == event_type && l.options.capture == capture)
                    .map(|l| l.id)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Check whether *any* non-removed listener exists for the given node.
    /// Used for the `chain_is_observed` fast-path.
    pub fn has_listeners(&self, node_id: usize) -> bool {
        self.entries
            .get(&node_id)
            .map(|listeners| listeners.iter().any(|l| !l.removed))
            .unwrap_or(false)
    }

    /// Check whether *any* node in the chain has listeners.
    pub fn chain_has_listeners(&self, chain: &[usize]) -> bool {
        chain.iter().any(|&nid| self.has_listeners(nid))
    }

    /// Invoke all matching listeners for a node. Handles `once` removal.
    ///
    /// `event_value` is the raw `napi_value` of the JS Event object to
    /// pass to each listener callback.
    ///
    /// **Note:** `stopPropagation` / `stopImmediatePropagation` /
    /// `preventDefault` flags are read back from the Event object's Rust
    /// state. This will be wired when the `#[napi] struct BlitzEvent`
    /// is implemented. For now, we return a basic `InvokeResult`.
    pub fn invoke_listeners(
        &mut self,
        env: &Env,
        node_id: usize,
        event_type: &str,
        capture: bool,
        event_value: napi::sys::napi_value,
    ) -> Result<InvokeResult> {
        let ids = self.matching_ids(node_id, event_type, capture);

        let result = InvokeResult::default();
        for id in ids {
            if result.immediate_stopped {
                break;
            }

            // Check if the listener still exists and is not removed.
            let listener_info = self.find_listener(node_id, id).map(|l| {
                (l.removed, l.options.once, l.callback_ref, l.env)
            });
            let Some((removed, once, callback_ref, callback_env)) = listener_info else {
                continue;
            };
            if removed {
                continue;
            }

            // Handle `once`: mark removed before invoking.
            if once {
                self.mark_removed(node_id, id);
            }

            // Invoke the callback via a temporary StoredListener reference.
            // We construct a temporary view to call invoke without borrowing self.
            let temp = StoredListener {
                id,
                event_type: String::new(), // not used by invoke
                callback_ref,
                env: callback_env,
                options: AddEventListenerOptions::default(), // not used by invoke
                removed: false,
            };
            if let Err(e) = temp.invoke(env, event_value) {
                eprintln!("napi-blitz: listener invoke error: {e}");
            }

            // Note: temp is a view, not the real stored listener.
            // We must NOT drop it (it would delete the napi_ref).
            std::mem::forget(temp);

            // TODO: read back stopPropagation / preventDefault from the
            // Event object's Rust state once BlitzEvent is implemented.
        }

        Ok(result)
    }

    /// Find a listener by id within a node's list (immutable).
    fn find_listener(&self, node_id: usize, listener_id: ListenerId) -> Option<&StoredListener> {
        self.entries
            .get(&node_id)?
            .iter()
            .find(|l| l.id == listener_id)
    }

    /// Mark a listener as removed by id (for `once` cleanup).
    pub fn mark_removed(&mut self, node_id: usize, listener_id: ListenerId) {
        if let Some(listeners) = self.entries.get_mut(&node_id) {
            if let Some(l) = listeners.iter_mut().find(|l| l.id == listener_id) {
                l.removed = true;
            }
        }
    }

    /// Remove all listeners for a node (e.g. when the node is dropped).
    pub fn remove_all_for_node(&mut self, node_id: usize) {
        self.entries.remove(&node_id);
    }

    /// Clear all listeners (doc teardown).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Compact: drop all `removed` entries and clean up empty node lists.
    pub fn compact(&mut self) {
        self.entries.retain(|_, listeners| {
            listeners.retain(|l| !l.removed);
            !listeners.is_empty()
        });
    }
}

/// Result of invoking listeners at one node.
#[derive(Debug, Default, Clone, Copy)]
pub struct InvokeResult {
    /// `stopImmediatePropagation` was called.
    pub immediate_stopped: bool,
}
