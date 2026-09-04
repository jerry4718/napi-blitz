//! NodeCache - switchable-reference cache of JS Node objects, keyed by blitz node id.
//!
//! The cache stores [`SwitchableRef`] handles. Each entry's refcount can be
//! toggled between strong (1, prevents GC) and weak (0, allows GC):
//!
//! - **In-document nodes**: strong. The JS object stays alive so event
//!   listeners registered on it are never lost.
//! - **Detached nodes**: weak. V8 may collect the JS object; the finalizer
//!   then removes the cache entry and reclaims blitz-side node storage.
//!
//! ## GC finalizer
//!
//! A finalizer is attached to each cached JS object. When V8 collects it
//! (only possible while in weak mode), the finalizer fires and:
//!   1. Removes the entry from the NodeCache (by `node_id`).
//!   2. Calls `remove_and_drop_node` on the blitz document to reclaim the
//!      Rust-side node storage.

use std::{collections::HashMap, rc::Weak};

use blitz::dom::{BaseDocument, NodeId};
use napi::{Env, Result, bindgen_prelude::Object};

use crate::dom::shared::doc::SharedDocument;
use napi_helpers::{Finalize, SwitchableRef};

/// Switchable-reference cache: `blitz_node_id -> SwitchableRef`.
pub struct NodeCache {
    entries: HashMap<NodeId, SwitchableRef>,
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
    /// reference is dead (the JS object has been garbage-collected, only
    /// possible in weak mode).
    pub fn get<'env>(&self, node_id: NodeId, env: &'env Env) -> Option<Object<'env>> {
        self.entries.get(&node_id)?.get_value(env)
    }

    /// Cache a freshly created JS Node object with the given initial strength.
    ///
    /// Also attaches a finalizer to `obj` so that when V8 collects it we
    /// can remove the cache entry and reclaim blitz-side node storage.
    pub fn insert(
        &mut self,
        node_id: NodeId,
        obj: &Object,
        env: &Env,
        strong: bool,
        shared_doc: Weak<SharedDocument>,
    ) -> Result<()> {
        let switchable_ref = SwitchableRef::new(obj, env, strong)?;
        switchable_ref.add_finalizer(
            env,
            NodeFinalizer {
                node_id,
                shared_doc,
            },
        )?;
        self.entries.insert(node_id, switchable_ref);
        Ok(())
    }

    /// Explicitly remove a cache entry and delete the underlying `napi_ref`.
    pub fn remove(&mut self, node_id: NodeId) {
        self.entries.remove(&node_id);
    }

    /// Switch a cache entry to strong (refcount 1). No-op if already strong
    /// or not in cache.
    pub fn make_strong(&mut self, node_id: NodeId, env: &Env) -> Result<()> {
        if let Some(sref) = self.entries.get_mut(&node_id) {
            sref.make_strong(env)?;
        }
        Ok(())
    }

    /// Switch a cache entry to weak (refcount 0). No-op if already weak
    /// or not in cache.
    pub fn make_weak(&mut self, node_id: NodeId, env: &Env) -> Result<()> {
        if let Some(sref) = self.entries.get_mut(&node_id) {
            sref.make_weak(env)?;
        }
        Ok(())
    }

    /// Remove all entries whose reference is dead (JS object collected).
    pub fn sweep(&mut self, env: &Env) {
        let stale: Vec<NodeId> = self
            .entries
            .iter()
            .filter_map(|(&id, sref)| (!sref.is_alive(env)).then_some(id))
            .collect();
        for id in stale {
            self.entries.remove(&id);
        }
    }

    /// Number of entries currently in the cache.
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
            .is_some_and(|sref| sref.is_alive(env))
    }
}

impl Default for NodeCache {
    fn default() -> Self {
        Self::new()
    }
}

// ── Reference switching helpers ──────────────────────────────────────
//
// The subtree strong/weak switching helpers live on `SharedDocument` in
// `doc.rs`.

// ── Finalizer ────────────────────────────────────────────────────────

struct NodeFinalizer {
    node_id: NodeId,
    shared_doc: Weak<SharedDocument>,
}

impl Finalize for NodeFinalizer {
    fn finalize(&self, env: Env) {
        // The finalizer only fires in weak mode, which means the node was
        // detached from the document. Try to upgrade the weak ref to the
        // SharedDocument. If the document has already been dropped, its NodeCache
        // and SwitchableRefs were dropped too.
        let Some(doc_rc) = self.shared_doc.upgrade() else {
            #[cfg(debug_assertions)]
            println!("[finalize] node_id={} doc_rc was None", self.node_id);
            return;
        };

        let doc = &*doc_rc;

        doc.node_cache_mut().remove(self.node_id);

        let mut doc_mut = doc.base_mut();

        #[cfg(debug_assertions)]
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
            let node_tree = da::node_tree_string(Some(hint_node), 1, 4);

            let cache = doc.node_cache();
            if !has_live_descendant(&doc_mut, &cache, self.node_id, &env) {
                drop(cache);
                /*#[cfg(debug_assertions)]
                {
                    println!(
                        "[finalize] doc_id={} node_id={} detached, remove_and_drop_node",
                        doc_mut.id(),
                        self.node_id
                    );
                    print!("{}", node_tree);
                }*/
                doc_mut.mutate().remove_and_drop_node(self.node_id);
                return;
            }
        }

        cleanup_detached_subtree(&mut doc_mut, &doc.node_cache(), self.node_id, &env);
    }
}

/// From a detached node, walk up to find the topmost ancestor that still
/// exists in the slab, then check if that subtree has no live JS wrapper.
/// If so, drop the entire subtree.
pub fn cleanup_detached_subtree(
    doc: &mut BaseDocument,
    cache: &NodeCache,
    node_id: NodeId,
    env: &Env,
) {
    #[cfg(debug_assertions)]
    let doc_id = doc.id();
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

    /*#[cfg(debug_assertions)]
    {
        println!(
            "[cleanup] doc_id={} node_id={} top={} remove_and_drop_node",
            doc_id, node_id, top
        );
    }*/
    doc.mutate().remove_and_drop_node(top);
}

#[cfg(debug_assertions)]
mod da {
    use blitz_dom::Node;

    pub(crate) fn node_tree_string(node: Option<&Node>, level: usize, max_level: usize) -> String {
        node_tree_string_inner(node, level, max_level, true)
    }

    #[inline]
    pub fn node_tree_string_inner(
        node: Option<&Node>,
        level: usize,
        max_level: usize,
        first: bool,
    ) -> String {
        if level > max_level {
            return format!("{} ... (max_level)\n", "  ".repeat(level));
        }
        let Some(node) = node else {
            return format!("{} (missing)\n", "  ".repeat(level));
        };

        let mut out = if first {
            format!(
                "{} {} parent = {:?} {} {:?}\n",
                "  ".repeat(level),
                node.id,
                node.parent,
                node.node_debug_str().replace('\n', ""),
                node.children
            )
        } else {
            format!(
                "{} {} {} {:?}\n",
                "  ".repeat(level),
                node.id,
                node.node_debug_str().replace('\n', ""),
                node.children
            )
        };

        for &child_id in &*node.children {
            out.push_str(&node_tree_string_inner(
                node.try_with(child_id),
                level + 1,
                max_level,
                false,
            ));
        }
        out
    }
}

/// Recursively check if any descendant of `node_id` has a live entry in
/// `node_cache`.
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
