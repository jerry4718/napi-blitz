//! NodeCache - weak-reference cache of JS Node objects, keyed by blitz node id.
//!
//! All `napi_ref` unsafe calls are concentrated in this module so they can be
//! audited and later converged onto napi-rs safe APIs
//! (`Reference::downgrade` / `WeakReference::upgrade` / `ObjectFinalize`).
//!
//! The cache stores `napi_ref` handles with an initial refcount of **0**
//! (weak reference). This means the cached reference does **not** prevent
//! V8 from garbage-collecting the JS Node object. When `get` is called and
//! the underlying object has been collected, `napi_get_reference_value`
//! returns a null `napi_value` and we report a cache miss.
//!
//! ## GC finalizer
//!
//! In addition to the weak reference, we attach a **finalizer** to each
//! cached JS object via `napi_add_finalizer`. When V8 collects the JS
//! object, the finalizer fires and we:
//!   1. Remove the entry from the NodeCache (by `node_id`).
//!   2. Delete the `napi_ref` (frees the V8 reference slot).
//!   3. Check if the blitz node is detached (no parent). If so, call
//!      `remove_and_drop_node` on the blitz document to reclaim the
//!      Rust-side node storage.

use std::{collections::HashMap, ptr, rc::Weak};

use blitz::dom::{BaseDocument, Node, NodeId};
use napi::{Env, JsValue, Result, bindgen_prelude::Object, check_status, sys};

use crate::dom::doc::SharedDoc;

/// Data passed to the finalizer as `finalize_data`.
/// Boxed and leaked; the finalizer reclaims it via `Box::from_raw`.
struct FinalizerHint {
    node_id: NodeId,
    /// Weak reference to the `SharedDoc`. At finalize time `upgrade()`
    /// returns `None` if the document has already been dropped, in which
    /// case we only delete the napi_ref.
    doc_weak: Weak<SharedDoc>,
}

/// Weak-reference cache: `blitz_node_id -> napi_ref (refcount=0)`.
pub struct NodeCache {
    pub entries: HashMap<NodeId, sys::napi_ref>,
}

impl NodeCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Try to retrieve a cached JS Node object.
    ///
    /// Returns `None` if the cache has no entry for `node_id` **or** if the
    /// weak reference is dead (the JS object has been garbage-collected).
    /// Stale entries are NOT removed here (use `sweep` for that).
    #[allow(unused)]
    pub fn get(&self, node_id: NodeId, env: &Env) -> Option<Object<'_>> {
        let napi_ref = *self.entries.get(&node_id)?;
        let mut value = ptr::null_mut();
        // SAFETY: `napi_ref` was created by `napi_create_reference` on the same
        // env/thread. `napi_get_reference_value` is safe to call with a valid
        // ref; it returns null when the referenced object has been collected.
        let status = unsafe { sys::napi_get_reference_value(env.raw(), napi_ref, &mut value) };
        if status != sys::Status::napi_ok || value.is_null() {
            return None;
        }
        // `Object::from_raw` is a safe constructor that copies raw pointers.
        // The returned Object<'env> does not actually borrow from `self`.
        Some(Object::from_raw(env.raw(), value))
    }

    /// Try to retrieve a cached JS Node object by raw napi_ref lookup.
    /// This avoids the `&self` borrow by taking the HashMap directly.
    pub fn get_from_map<'a>(
        entries: &HashMap<NodeId, sys::napi_ref>,
        node_id: NodeId,
        env: &'a Env,
    ) -> Option<Object<'a>> {
        let napi_ref = *entries.get(&node_id)?;
        let mut value = ptr::null_mut();
        let status = unsafe { sys::napi_get_reference_value(env.raw(), napi_ref, &mut value) };
        if status != sys::Status::napi_ok || value.is_null() {
            return None;
        }
        Some(Object::from_raw(env.raw(), value))
    }

    /// Cache a freshly created JS Node object as a **weak** reference.
    ///
    /// If an entry already exists for `node_id` it is replaced (the old
    /// `napi_ref` is deleted first).
    ///
    /// Also attaches a finalizer to `obj` so that when V8 collects it we
    /// can eagerly remove the cache entry, delete the `napi_ref`, and
    /// potentially reclaim blitz-side node storage for detached nodes.
    /// `doc_weak` is a `Weak<SharedDoc>` used by the finalizer to reach
    /// the NodeCache and the blitz document.
    pub fn insert(
        &mut self,
        node_id: NodeId,
        obj: &Object,
        env: &Env,
        doc_weak: Weak<SharedDoc>,
    ) -> Result<()> {
        // Remove any existing entry first to avoid leaking the old ref.
        if self.entries.contains_key(&node_id) {
            self.remove_internal(node_id, env);
        }
        let mut napi_ref = ptr::null_mut();
        // SAFETY: `obj.raw()` is a valid napi_value on the current env.
        // `initial_refcount = 0` creates a weak reference that does not
        // prevent GC.
        check_status!(
            unsafe { sys::napi_create_reference(env.raw(), obj.raw(), 0, &mut napi_ref) },
            "NodeCache: failed to create weak reference"
        )?;
        self.entries.insert(node_id, napi_ref);

        // Attach finalizer for eager cleanup.
        let hint = Box::new(FinalizerHint { node_id, doc_weak });
        let hint_ptr = Box::into_raw(hint);
        let status = unsafe {
            sys::napi_add_finalizer(
                env.raw(),
                obj.raw(),
                hint_ptr.cast(),
                Some(node_finalizer),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        if status != sys::Status::napi_ok {
            // Finalizer registration failed - safe to reclaim the hint.
            // The cache entry still works via weak-ref polling; we just
            // lose eager cleanup.
            let _ = unsafe { Box::from_raw(hint_ptr) };
        }

        Ok(())
    }

    /// Explicitly remove a cache entry and delete the underlying `napi_ref`.
    #[allow(unused)]
    pub fn remove(&mut self, node_id: NodeId, env: &Env) {
        self.remove_internal(node_id, env);
    }

    /// Remove all entries whose weak reference is dead (JS object collected).
    ///
    /// Intended to be called periodically (e.g. after a dispatch cycle) to
    /// keep the HashMap from growing without bound. With the finalizer in
    /// place this is less necessary but still useful as a backstop.
    pub fn sweep(&mut self, env: &Env) {
        let stale: Vec<NodeId> = self
            .entries
            .iter()
            .filter_map(|(&id, &napi_ref)| {
                let mut value = ptr::null_mut();
                let status =
                    unsafe { sys::napi_get_reference_value(env.raw(), napi_ref, &mut value) };
                if status != sys::Status::napi_ok || value.is_null() {
                    Some(id)
                } else {
                    None
                }
            })
            .collect();
        for id in stale {
            self.remove_internal(id, env);
        }
    }

    /// Number of entries currently in the cache (including potentially stale ones).
    #[allow(unused)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    #[allow(unused)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    // ---- internal helpers ----

    fn remove_internal(&mut self, node_id: NodeId, env: &Env) {
        if let Some(napi_ref) = self.entries.remove(&node_id) {
            // SAFETY: `napi_ref` was created by `napi_create_reference` and has
            // not been deleted yet. `napi_delete_reference` is safe to call
            // on a valid ref with refcount 0.
            let _ = unsafe { sys::napi_delete_reference(env.raw(), napi_ref) };
        }
    }
}

impl Default for NodeCache {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for NodeCache {
    fn drop(&mut self) {
        // We cannot call `napi_delete_reference` here because we don't have
        // an `Env`. In practice the cache lives as long as the `SharedDoc`,
        // which outlives any single JS callback. The napi_refs are weak
        // (refcount=0) so they don't keep JS objects alive - leaking the
        // ref struct itself is a minor memory cost that only matters if
        // the process creates and destroys many documents.
        //
        // If this becomes a concern we can add an explicit `shutdown(env)`
        // method that deletes all refs, called before the DocHandle is
        // dropped.
    }
}

/// Finalizer callback for cached JS Node objects.
///
/// # Safety
/// This is an `unsafe extern "C" fn` called by V8 when the JS object is
/// garbage-collected. `finalize_data` is a raw pointer to a `FinalizerHint`
/// that we created via `Box::into_raw` in `insert`. We reclaim it here.
///
/// When fired, the finalizer:
///   1. Reclaims the `FinalizerHint` box.
///   2. Upgrades the `Weak<SharedDoc>` to reach the NodeCache + blitz doc.
///   3. Removes the NodeCache entry and deletes the `napi_ref`.
///   4. Checks if the blitz node is detached (no parent). If detached,
///      calls `remove_and_drop_node` to reclaim Rust-side node storage.
unsafe extern "C" fn node_finalizer(
    env: sys::napi_env,
    finalize_data: *mut std::ffi::c_void,
    _finalize_hint: *mut std::ffi::c_void,
) {
    // Reclaim the hint.
    let hint = unsafe { Box::from_raw(finalize_data as *mut FinalizerHint) };

    // Try to upgrade the weak ref to the SharedDoc. If the document has
    // been dropped already, `upgrade` returns `None` and there's nothing
    // we can do (the napi_ref will be leaked at refcount=0, minor cost).
    let Some(doc_rc) = hint.doc_weak.upgrade() else {
        #[cfg(debug_assertions)]
        println!("node_finalizer: {} doc_rc was None", hint.node_id);
        return;
    };

    // `doc_rc` is `Rc<SharedDoc>`. SharedDoc's fields are individually
    // RefCell-protected, so we access them through the Rc directly.
    let doc = &*doc_rc;

    // 1. Remove the NodeCache entry and delete the napi_ref.
    {
        let mut cache = doc.node_cache.borrow_mut();
        if let Some(napi_ref) = cache.entries.remove(&hint.node_id) {
            let _ = unsafe { sys::napi_delete_reference(env, napi_ref) };
        }
    }

    let mut doc_mut = doc.base.borrow_mut();

    let Some(hint_node) = doc_mut.get_node_mut(hint.node_id) else {
        #[cfg(debug_assertions)]
        println!("node_finalizer: {} was None", hint.node_id);
        return;
    };

    // 2. Check if the blitz node is detached. If so, drop it to reclaim
    //    Rust-side storage. A node with no parent is detached - it's
    //    not in the document tree and no JS wrapper references it
    //    (the finalizer just fired), so it's safe to reclaim.
    let is_detached = hint_node.parent.is_none();

    #[cfg(debug_assertions)]
    let node_tree = node_tree_string(Some(hint_node), 1, 4);

    if is_detached {
        // Check if any descendant still has a live JS wrapper.
        // If none do, the entire subtree can be safely dropped.
        let cache = doc.node_cache.borrow();
        if !has_live_descendant(&doc_mut, &cache, hint.node_id) {
            drop(cache);
            #[cfg(debug_assertions)]
            {
                println!(
                    "node_finalizer: {} is_detached: {}",
                    hint.node_id, is_detached
                );
                print!("{}", node_tree);
            }
            doc_mut.mutate().remove_and_drop_node(hint.node_id);
            return;
        }
    }

    cleanup_detached_subtree(&mut doc_mut, &doc.node_cache.borrow(), hint.node_id);
}

/// Plan A: from a detached node, walk up to find the topmost ancestor
/// that still exists in the slab, then check if that subtree has no
/// live JS wrapper. If so, drop the entire subtree.
///
/// Called from `NodeHandle::remove` and `node_finalizer`.
pub fn cleanup_detached_subtree(doc: &mut BaseDocument, cache: &NodeCache, node_id: NodeId) {
    // Walk up to find the topmost node in the detached chain.
    let mut top = node_id;
    while let Some(p) = doc.get_node(top).and_then(|n| n.parent) {
        if doc.get_node(p).is_none() {
            break;
        }
        top = p;
    }

    if top == node_id {
        return;
    }

    if cache.entries.contains_key(&top) {
        return;
    }

    if has_live_descendant(doc, cache, top) {
        return;
    }

    #[cfg(debug_assertions)]
    println!(
        "cleanup_detached_subtree: node_id: {}, top: {} ",
        node_id, top
    );
    doc.mutate().remove_and_drop_node(top);
}

/// Like `Node::print_tree` but returns a `String`. Needed because the
/// finalizer holds a mutable borrow on the document and cannot call
/// `print_tree` (which would re-borrow the tree immutably).
#[cfg(debug_assertions)]
fn node_tree_string(node: Option<&Node>, level: usize, max_level: usize) -> String {
    if level > max_level {
        return format!("{} ... (max_level)\n", "  ".repeat(level));
    }
    let Some(node) = node else {
        return format!("{} (missing)\n", "  ".repeat(level));
    };
    let mut out = format!(
        "{} {} {:?} {} {:?}\n",
        "  ".repeat(level),
        node.id,
        node.parent,
        node.node_debug_str().replace('\n', ""),
        node.children
    );
    for &child_id in &*node.children {
        out.push_str(&node_tree_string(
            node.try_with(child_id),
            level + 1,
            max_level,
        ));
    }
    out
}

/// Recursively check if any descendant of `node_id` has an entry in
/// `node_cache` (i.e. still has a live JS wrapper).  Returns true if
/// at least one descendant is still referenced from JS.
pub fn has_live_descendant(doc: &BaseDocument, cache: &NodeCache, node_id: NodeId) -> bool {
    let child_ids: Vec<_> = doc
        .get_node(node_id)
        .map(|n| n.children.to_vec())
        .unwrap_or_default();
    for child_id in child_ids {
        if cache.entries.contains_key(&child_id) {
            return true;
        }
        if has_live_descendant(doc, cache, child_id) {
            return true;
        }
    }
    false
}
