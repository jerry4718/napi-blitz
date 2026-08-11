//! NodeCache - weak-reference cache of JS Node objects, keyed by blitz node id.
//!
//! The cache stores [`JsWeakRef`] handles (refcount-0 `napi_ref`). This means
//! the cached reference does **not** prevent V8 from garbage-collecting the JS
//! Node object. When `get` is called and the underlying object has been
//! collected, `JsWeakRef::get_value` returns `None` and we report a cache miss.
//!
//! All `napi_ref` unsafe operations are encapsulated in [`JsWeakRef`].
//!
//! ## GC finalizer
//!
//! In addition to the weak reference, we attach a **finalizer** to each
//! cached JS object via `napi_add_finalizer`. When V8 collects the JS
//! object, the finalizer fires and we:
//!   1. Remove the entry from the NodeCache (by `node_id`).
//!   2. Check if the blitz node is detached (no parent). If so, call
//!      `remove_and_drop_node` on the blitz document to reclaim the
//!      Rust-side node storage.

use std::{collections::HashMap, rc::Weak};

use blitz::dom::{BaseDocument, Node, NodeId};
use napi::{Env, Result, bindgen_prelude::Object};

use crate::{
    dom::doc::SharedDoc,
    helpers::{Finalize, JsWeakRef},
};

/// Weak-reference cache: `blitz_node_id -> JsWeakRef`.
pub struct NodeCache {
    entries: HashMap<NodeId, JsWeakRef>,
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
    pub fn get<'env>(&self, node_id: NodeId, env: &'env Env) -> Option<Object<'env>> {
        self.entries.get(&node_id)?.get_value(env)
    }

    /// Cache a freshly created JS Node object as a **weak** reference.
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
        let weak_ref = JsWeakRef::new(obj, env)?;
        weak_ref.add_finalizer(env, NodeFinalizer { node_id, doc_weak })?;
        self.entries.insert(node_id, weak_ref);
        Ok(())
    }

    /// Explicitly remove a cache entry and delete the underlying `napi_ref`.
    #[allow(unused)]
    pub fn remove(&mut self, node_id: NodeId) {
        self.entries.remove(&node_id);
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
            .filter_map(|(&id, weak_ref)| (!weak_ref.is_alive(env)).then_some(id))
            .collect();
        for id in stale {
            self.entries.remove(&id);
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

    fn is_alive(&self, node_id: NodeId, env: &Env) -> bool {
        self.entries
            .get(&node_id)
            .is_some_and(|weak_ref| weak_ref.is_alive(env))
    }
}

impl Default for NodeCache {
    fn default() -> Self {
        Self::new()
    }
}

struct NodeFinalizer {
    node_id: NodeId,
    doc_weak: Weak<SharedDoc>,
}

impl Finalize for NodeFinalizer {
    fn finalize(&self, env: Env) {
        // Try to upgrade the weak ref to the SharedDoc. If the document has
        // already been dropped, its NodeCache and JsWeakRefs were dropped too.
        let Some(doc_rc) = self.doc_weak.upgrade() else {
            #[cfg(debug_assertions)]
            println!("[finalize] node_id={} doc_rc was None", self.node_id);
            return;
        };

        let doc = &*doc_rc;

        doc.node_cache.borrow_mut().remove(self.node_id);

        let mut doc_mut = doc.base.borrow_mut();
        let doc_id = doc_mut.id();

        let Some(hint_node) = doc_mut.get_node_mut(self.node_id) else {
            #[cfg(debug_assertions)]
            println!(
                "[finalize] doc_id={doc_id} node_id={} node not found",
                self.node_id
            );
            return;
        };

        let is_detached = hint_node.parent.is_none();

        if is_detached {
            #[cfg(debug_assertions)]
            let node_tree = node_tree_string(Some(hint_node), 1, 4);

            let cache = doc.node_cache.borrow();
            if !has_live_descendant(&doc_mut, &cache, self.node_id, &env) {
                drop(cache);
                #[cfg(debug_assertions)]
                {
                    println!(
                        "[finalize] doc_id={} node_id={} detached, remove_and_drop_node",
                        doc_mut.id(),
                        self.node_id
                    );
                    print!("{}", node_tree);
                }
                doc_mut.mutate().remove_and_drop_node(self.node_id);
                return;
            }
        }

        cleanup_detached_subtree(&mut doc_mut, &doc.node_cache.borrow(), self.node_id, &env);
    }
}

/// Plan A: from a detached node, walk up to find the topmost ancestor
/// that still exists in the slab, then check if that subtree has no
/// live JS wrapper. If so, drop the entire subtree.
///
/// Called from `NodeHandle::remove` and `NodeFinalizerState::finalize`.
pub fn cleanup_detached_subtree(
    doc: &mut BaseDocument,
    cache: &NodeCache,
    node_id: NodeId,
    env: &Env,
) {
    let doc_id = doc.id();
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

    if cache.is_alive(top, env) {
        return;
    }

    if has_live_descendant(doc, cache, top, env) {
        return;
    }

    #[cfg(debug_assertions)]
    {
        println!(
            "[cleanup] doc_id={} node_id={} top={} remove_and_drop_node",
            doc_id, node_id, top
        );
    }
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
pub fn has_live_descendant(
    doc: &BaseDocument,
    cache: &NodeCache,
    node_id: NodeId,
    env: &Env,
) -> bool {
    find_live_descendant(doc, cache, node_id, env).is_some()
}

fn find_live_descendant(
    doc: &BaseDocument,
    cache: &NodeCache,
    node_id: NodeId,
    env: &Env,
) -> Option<NodeId> {
    let child_ids: Vec<_> = doc
        .get_node(node_id)
        .map(|n| n.children.to_vec())
        .unwrap_or_default();
    for child_id in child_ids {
        if cache.is_alive(child_id, env) {
            return Some(child_id);
        }
        if let Some(live_descendant) = find_live_descendant(doc, cache, child_id, env) {
            return Some(live_descendant);
        }
    }
    None
}
