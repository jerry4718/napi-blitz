//! The `Node` layer — root of the DOM node hierarchy (`EventTarget` is the
//! parent slot, provided by `wintertc-events`). Holds the immutable
//! `node_id` / `doc` pair plus a `state` block (mirroring `EventLayer`'s
//! config/state split); derived layers inherit the tree-walking methods.

use std::rc::Rc;

use crate::{
    dom::shared::{doc::SharedDocument, wrap_node},
    events::base::EventTargetLayer,
};
use blitz::dom::{NodeData, NodeId};
use napi::{Env, Error, Result, bindgen_prelude::Object};
use napi_helpers::inherits::{Constructed, LayerRef, Super, proc::layer, with_own};
use napi_helpers::native_log;

#[napi(js_name = "NodeTypes")]
mod node_types {
    #[napi]
    pub const ELEMENT_NODE: u8 = 1;
    #[napi]
    pub const TEXT_NODE: u8 = 3;
    #[napi]
    pub const COMMENT_NODE: u8 = 8;
    #[napi]
    pub const DOCUMENT_NODE: u8 = 9;
    #[napi]
    pub const OTHER_NODE: u8 = 0;
}

/// Own block of the `Node` class.
#[layer]
pub struct NodeLayer {
    pub(crate) node_id: NodeId,
    pub(crate) shared_doc: Rc<SharedDocument>,
}

#[layer(js_name = "Node")]
impl NodeLayer {
    #[layer(parent)]
    type Parent = EventTargetLayer;

    #[layer(constructor)]
    fn build(_sup: Super<EventTargetLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "Node is abstract; create nodes via document APIs",
        ))
    }

    #[layer(getter)]
    fn node_type(&self) -> u8 {
        let base = self.shared_doc.base();
        let Some(node) = base.get_node(self.node_id) else {
            return node_types::OTHER_NODE;
        };
        match &node.data {
            NodeData::Document(_) => node_types::DOCUMENT_NODE,
            NodeData::Element(_) => node_types::ELEMENT_NODE,
            NodeData::Text(_) => node_types::TEXT_NODE,
            NodeData::Comment { .. } => node_types::COMMENT_NODE,
            _ => node_types::OTHER_NODE,
        }
    }

    #[layer(getter)]
    fn parent_node(&self, env: &Env) -> Result<Option<LayerRef<NodeLayer>>> {
        let Some(parent_id) = self
            .shared_doc
            .base()
            .get_node(self.node_id)
            .and_then(|n| n.parent)
        else {
            return Ok(None);
        };
        let node = wrap_node(&self.shared_doc, env, parent_id)?;
        Ok(Some(LayerRef::new(&node, env)?))
    }

    #[layer(getter)]
    fn first_child(&self, env: &Env) -> Result<Option<LayerRef<NodeLayer>>> {
        let Some(child_id) = self
            .shared_doc
            .base()
            .get_node(self.node_id)
            .and_then(|n| n.children.first().copied())
        else {
            return Ok(None);
        };
        let node = wrap_node(&self.shared_doc, env, child_id)?;
        Ok(Some(LayerRef::new(&node, env)?))
    }

    #[layer(getter)]
    fn last_child(&self, env: &Env) -> Result<Option<LayerRef<NodeLayer>>> {
        let Some(child_id) = self
            .shared_doc
            .base()
            .get_node(self.node_id)
            .and_then(|n| n.children.last().copied())
        else {
            return Ok(None);
        };
        let node = wrap_node(&self.shared_doc, env, child_id)?;
        Ok(Some(LayerRef::new(&node, env)?))
    }

    #[layer(getter)]
    fn next_sibling(&self, env: &Env) -> Result<Option<LayerRef<NodeLayer>>> {
        let Some(sibling_id) = ({
            let base = self.shared_doc.base();
            base.get_node(self.node_id)
                .and_then(|n| n.forward(1))
                .map(|n| n.id)
        }) else {
            return Ok(None);
        };
        let node = wrap_node(&self.shared_doc, env, sibling_id)?;
        Ok(Some(LayerRef::new(&node, env)?))
    }

    #[layer(getter)]
    fn previous_sibling(&self, env: &Env) -> Result<Option<LayerRef<NodeLayer>>> {
        let Some(sibling_id) = ({
            let base = self.shared_doc.base();
            base.get_node(self.node_id)
                .and_then(|n| n.backward(1))
                .map(|n| n.id)
        }) else {
            return Ok(None);
        };
        let node = wrap_node(&self.shared_doc, env, sibling_id)?;
        Ok(Some(LayerRef::new(&node, env)?))
    }

    #[layer(getter)]
    fn child_nodes(&self, env: &Env) -> Result<Vec<LayerRef<NodeLayer>>> {
        let children: Vec<NodeId> = self
            .shared_doc
            .base()
            .get_node(self.node_id)
            .map(|n| n.children.iter().copied().collect())
            .unwrap_or_default();
        let mut out = Vec::with_capacity(children.len());
        for id in children {
            out.push(LayerRef::new(&wrap_node(&self.shared_doc, env, id)?, env)?);
        }
        Ok(out)
    }

    /// `node.contains(other)` — true for the node itself and its
    /// descendants. Non-`Node` arguments are false, per spec.
    #[layer]
    fn contains(&self, other: Object) -> Result<bool> {
        let Ok(other_id) = with_own::<NodeLayer, _>(&other, |n| n.node_id) else {
            return Ok(false);
        };
        if other_id == self.node_id {
            return Ok(true);
        }
        let base = self.shared_doc.base();
        let mut ancestor = base.get_node(other_id).and_then(|n| n.parent);
        while let Some(id) = ancestor {
            if id == self.node_id {
                return Ok(true);
            }
            ancestor = base.get_node(id).and_then(|n| n.parent);
        }
        Ok(false)
    }

    #[layer(getter)]
    fn text_content(&self) -> Option<String> {
        let base = self.shared_doc.base();
        base.get_node(self.node_id).map(|n| n.text_content())
    }

    #[layer(setter)]
    fn set_text_content(&mut self, env: &Env, text: String) {
        let mut base = self.shared_doc.base_mut();
        let is_text = base
            .get_node(self.node_id)
            .map(|n| n.is_text_node())
            .unwrap_or(false);
        if is_text {
            let mut mutator = base.mutate();
            mutator.set_node_text(self.node_id, &text);
            drop(mutator);
            drop(base);
            self.shared_doc.mark_host_dirty();
            return;
        }

        drop(base);
        self.shared_doc.detach_children(self.node_id, env).ok();
        let mut base = self.shared_doc.base_mut();
        let mut mutator = base.mutate();
        let text_id = mutator.create_text_node(&text);
        mutator.append_children(self.node_id, &[text_id]);
        drop(mutator);
        drop(base);
        self.shared_doc.mark_host_dirty();
    }

    #[layer]
    fn append_child(&mut self, env: &Env, child: Object) -> Result<LayerRef<NodeLayer>> {
        let child_id = with_own::<NodeLayer, _>(&child, |d| d.node_id)?;
        let mut base = self.shared_doc.base_mut();
        let mut mutator = base.mutate();
        mutator.append_children(self.node_id, &[child_id]);
        drop(mutator);
        drop(base);
        self.shared_doc.mark_host_dirty();
        self.shared_doc
            .make_in_document_subtree_strong(self.node_id, child_id, env)?;
        LayerRef::new(&wrap_node(&self.shared_doc, env, child_id)?, env)
    }

    #[layer]
    fn insert_before(
        &mut self,
        env: &Env,
        node: Object,
        anchor: Option<Object>,
    ) -> Result<LayerRef<NodeLayer>> {
        let node_id = with_own::<NodeLayer, _>(&node, |d| d.node_id)?;
        let anchor_id = match &anchor {
            Some(a) => Some(with_own::<NodeLayer, _>(a, |d| d.node_id)?),
            None => None,
        };
        let mut base = self.shared_doc.base_mut();
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
        self.shared_doc.mark_host_dirty();
        self.shared_doc
            .make_in_document_subtree_strong(self.node_id, node_id, env)?;
        LayerRef::new(&wrap_node(&self.shared_doc, env, node_id)?, env)
    }

    /// `parent.removeChild(child)` — detach `child` and return it.
    #[layer]
    fn remove_child(&mut self, env: &Env, child: Object) -> Result<LayerRef<NodeLayer>> {
        let child_id = with_own::<NodeLayer, _>(&child, |d| d.node_id)?;
        // Switch to weak before removing, while parent chain is intact.
        if let Err(e) = self.shared_doc.make_in_document_subtree_weak(child_id, env) {
            native_log!("napi-blitz-dom: make_in_document_subtree_weak failed: {e}");
        }
        let mut base = self.shared_doc.base_mut();
        let mut mutator = base.mutate();
        mutator.remove_node(child_id);
        drop(mutator);
        drop(base);
        self.shared_doc.mark_host_dirty();
        LayerRef::new(&child, env)
    }

    #[layer]
    fn remove(&mut self, env: &Env) {
        // Switch to weak before removing, while parent chain is intact.
        if let Err(e) = self
            .shared_doc
            .make_in_document_subtree_weak(self.node_id, env)
        {
            native_log!("napi-blitz-dom: make_in_document_subtree_weak failed: {e}");
        }
        let mut base = self.shared_doc.base_mut();
        let mut mutator = base.mutate();
        mutator.remove_node(self.node_id);
        drop(mutator);
        drop(base);
        self.shared_doc.mark_host_dirty();
    }

    #[layer]
    fn replace_with(&mut self, env: &Env, node: Object) -> Result<LayerRef<NodeLayer>> {
        let removed_id = self.node_id;
        let node_id = with_own::<NodeLayer, _>(&node, |d| d.node_id)?;
        // Switch the removed node to weak before detaching, while parent chain is intact.
        if let Err(e) = self
            .shared_doc
            .make_in_document_subtree_weak(removed_id, env)
        {
            native_log!("napi-blitz-dom: make_in_document_subtree_weak failed: {e}");
        }
        let mut base = self.shared_doc.base_mut();
        let mut mutator = base.mutate();
        mutator.replace_node_with(removed_id, &[node_id]);
        drop(mutator);
        drop(base);
        self.shared_doc.mark_host_dirty();
        // The new node is now in document -> strong.
        self.shared_doc
            .make_in_document_subtree_strong(node_id, node_id, env)?;
        LayerRef::new(&wrap_node(&self.shared_doc, env, node_id)?, env)
    }

    #[layer]
    fn clone_node(&self, env: &Env, deep: bool) -> Result<LayerRef<NodeLayer>> {
        let new_id = if deep {
            let mut base = self.shared_doc.base_mut();
            let mut mutator = base.mutate();
            let clone_id = mutator.deep_clone_node(self.node_id);
            drop(mutator);
            drop(base);
            clone_id
        } else {
            let mut base = self.shared_doc.base_mut();
            let Some(data) = base.get_node(self.node_id).map(|node| node.data.clone()) else {
                return LayerRef::new(&wrap_node(&self.shared_doc, env, self.node_id)?, env);
            };
            let clone_id = base.create_node(data);
            drop(base);
            clone_id
        };
        self.shared_doc.mark_host_dirty();
        LayerRef::new(&wrap_node(&self.shared_doc, env, new_id)?, env)
    }
}
