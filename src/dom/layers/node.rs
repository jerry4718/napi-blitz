//! The `Node` layer — root of the DOM node hierarchy (`EventTarget` is the
//! parent slot, provided by `wintertc-events`). Holds the immutable
//! `node_id` / `doc` pair plus a `state` block (mirroring `EventLayer`'s
//! config/state split); derived layers inherit the tree-walking methods.

use std::rc::Rc;

use blitz::dom::{NodeData, NodeId};
use napi::{Env, Error, Result, bindgen_prelude::Object};
use napi_helpers::inherit as napi_inherit;
use napi_helpers::inherit::layer::{Constructed, Super};
use napi_helpers::inherit::proc::layer;
use wintertc_events::event_target::EventTargetLayer;

use crate::shared::doc::{SharedDoc, wrap_node};

const NODE_TYPE_ELEMENT: u32 = 1;
const NODE_TYPE_TEXT: u32 = 3;
const NODE_TYPE_COMMENT: u32 = 8;
const NODE_TYPE_DOCUMENT: u32 = 9;
const NODE_TYPE_OTHER: u32 = 0;

/// Mutable per-node state. Deliberately empty for now; kept as a separate
/// field so the split matches `EventLayer` (config vs `state`) and the
/// block can be wrapped in a `RefCell` later.
#[derive(Default)]
pub struct NodeState {}

/// Own block of the `Node` class.
#[layer(js_name = "Node", parent = EventTargetLayer)]
pub struct NodeLayer {
    pub(crate) node_id: NodeId,
    pub(crate) doc: Rc<SharedDoc>,
    pub(crate) state: NodeState,
}

#[layer]
impl NodeLayer {
    #[layer(constructor)]
    fn build(_sup: Super<EventTargetLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "Node is abstract; create nodes via document APIs",
        ))
    }

    #[layer(getter)]
    fn node_type(&self) -> u32 {
        let base = self.doc.base.borrow();
        let Some(node) = base.get_node(self.node_id) else {
            return NODE_TYPE_OTHER;
        };
        match &node.data {
            NodeData::Document(_) => NODE_TYPE_DOCUMENT,
            NodeData::Element(_) => NODE_TYPE_ELEMENT,
            NodeData::Text(_) => NODE_TYPE_TEXT,
            NodeData::Comment { .. } => NODE_TYPE_COMMENT,
            _ => NODE_TYPE_OTHER,
        }
    }

    #[layer]
    fn parent_node<'a>(&self, env: &'a Env) -> Option<Object<'a>> {
        let parent_id = self
            .doc
            .base
            .borrow()
            .get_node(self.node_id)
            .and_then(|n| n.parent)?;
        wrap_node(&self.doc, env, parent_id).ok()
    }

    #[layer]
    fn first_child<'a>(&self, env: &'a Env) -> Option<Object<'a>> {
        let child_id = self
            .doc
            .base
            .borrow()
            .get_node(self.node_id)
            .and_then(|n| n.children.first().copied())?;
        wrap_node(&self.doc, env, child_id).ok()
    }

    #[layer]
    fn last_child<'a>(&self, env: &'a Env) -> Option<Object<'a>> {
        let child_id = self
            .doc
            .base
            .borrow()
            .get_node(self.node_id)
            .and_then(|n| n.children.last().copied())?;
        wrap_node(&self.doc, env, child_id).ok()
    }

    #[layer]
    fn next_sibling<'a>(&self, env: &'a Env) -> Option<Object<'a>> {
        let sibling_id = {
            let base = self.doc.base.borrow();
            base.get_node(self.node_id)
                .and_then(|n| n.forward(1))
                .map(|n| n.id)
        }?;
        wrap_node(&self.doc, env, sibling_id).ok()
    }

    #[layer]
    fn previous_sibling<'a>(&self, env: &'a Env) -> Option<Object<'a>> {
        let sibling_id = {
            let base = self.doc.base.borrow();
            base.get_node(self.node_id)
                .and_then(|n| n.backward(1))
                .map(|n| n.id)
        }?;
        wrap_node(&self.doc, env, sibling_id).ok()
    }

    #[layer]
    fn child_nodes<'a>(&self, env: &'a Env) -> Vec<Object<'a>> {
        let children: Vec<NodeId> = self
            .doc
            .base
            .borrow()
            .get_node(self.node_id)
            .map(|n| n.children.iter().copied().collect())
            .unwrap_or_default();
        children
            .into_iter()
            .filter_map(|id| wrap_node(&self.doc, env, id).ok())
            .collect()
    }

    #[layer(getter)]
    fn text_content(&self) -> Option<String> {
        let base = self.doc.base.borrow();
        base.get_node(self.node_id).map(|n| n.text_content())
    }

    #[layer]
    fn set_text_content(&mut self, text: String, env: &Env) {
        let mut base = self.doc.base.borrow_mut();
        let is_text = base
            .get_node(self.node_id)
            .map(|n| n.is_text_node())
            .unwrap_or(false);
        if is_text {
            let mut mutator = base.mutate();
            mutator.set_node_text(self.node_id, &text);
            drop(mutator);
            drop(base);
            self.doc.mark_host_dirty();
            return;
        }

        drop(base);
        self.doc.detach_children(self.node_id, env).ok();
        let mut base = self.doc.base.borrow_mut();
        let mut mutator = base.mutate();
        let text_id = mutator.create_text_node(&text);
        mutator.append_children(self.node_id, &[text_id]);
        drop(mutator);
        drop(base);
        self.doc.mark_host_dirty();
    }

    #[layer]
    fn append_child<'a>(&mut self, env: &'a Env, child: Object) -> Result<Object<'a>> {
        let child_id = napi_inherit::own::with_own::<NodeLayer, _>(&child, |d| d.node_id)?;
        let mut base = self.doc.base.borrow_mut();
        let mut mutator = base.mutate();
        mutator.append_children(self.node_id, &[child_id]);
        drop(mutator);
        drop(base);
        self.doc.mark_host_dirty();
        self.doc
            .make_in_document_subtree_strong(self.node_id, child_id, env)?;
        wrap_node(&self.doc, env, child_id)
    }

    #[layer]
    fn insert_before<'a>(
        &mut self,
        env: &'a Env,
        node: Object,
        anchor: Option<Object>,
    ) -> Result<Object<'a>> {
        let node_id = napi_inherit::own::with_own::<NodeLayer, _>(&node, |d| d.node_id)?;
        let anchor_id = match &anchor {
            Some(a) => Some(napi_inherit::own::with_own::<NodeLayer, _>(a, |d| {
                d.node_id
            })?),
            None => None,
        };
        let mut base = self.doc.base.borrow_mut();
        let mut mutator = base.mutate();
        match anchor_id {
            Some(anchor_id) => {
                mutator.insert_nodes_before(anchor_id, &[node_id]);
            }
            None => {
                mutator.append_children(self.node_id, &[node_id]);
            }
        }
        drop(mutator);
        drop(base);
        self.doc.mark_host_dirty();
        self.doc
            .make_in_document_subtree_strong(self.node_id, node_id, env)?;
        wrap_node(&self.doc, env, node_id)
    }

    #[layer]
    fn remove(&mut self, env: &Env) {
        // Switch to weak before removing, while parent chain is intact.
        if let Err(e) = self.doc.make_in_document_subtree_weak(self.node_id, env) {
            eprintln!("napi-blitz-dom: make_in_document_subtree_weak failed: {e}");
        }
        let mut base = self.doc.base.borrow_mut();
        let mut mutator = base.mutate();
        mutator.remove_node(self.node_id);
        drop(mutator);
        drop(base);
        self.doc.mark_host_dirty();
    }

    #[layer]
    fn replace_with<'a>(&mut self, env: &'a Env, node: Object) -> Result<Object<'a>> {
        let removed_id = self.node_id;
        let node_id = napi_inherit::own::with_own::<NodeLayer, _>(&node, |d| d.node_id)?;
        // Switch the removed node to weak before detaching, while parent chain is intact.
        if let Err(e) = self.doc.make_in_document_subtree_weak(removed_id, env) {
            eprintln!("napi-blitz-dom: make_in_document_subtree_weak failed: {e}");
        }
        let mut base = self.doc.base.borrow_mut();
        let mut mutator = base.mutate();
        mutator.replace_node_with(removed_id, &[node_id]);
        drop(mutator);
        drop(base);
        self.doc.mark_host_dirty();
        // The new node is now in document -> strong.
        self.doc
            .make_in_document_subtree_strong(node_id, node_id, env)?;
        wrap_node(&self.doc, env, node_id)
    }

    #[layer]
    fn clone_node<'a>(&self, env: &'a Env, deep: bool) -> Result<Object<'a>> {
        let new_id = if deep {
            let mut base = self.doc.base.borrow_mut();
            let mut mutator = base.mutate();
            let clone_id = mutator.deep_clone_node(self.node_id);
            drop(mutator);
            drop(base);
            clone_id
        } else {
            let mut base = self.doc.base.borrow_mut();
            let Some(data) = base.get_node(self.node_id).map(|node| node.data.clone()) else {
                return wrap_node(&self.doc, env, self.node_id);
            };
            let clone_id = base.create_node(data);
            drop(base);
            clone_id
        };
        self.doc.mark_host_dirty();
        wrap_node(&self.doc, env, new_id)
    }
}
