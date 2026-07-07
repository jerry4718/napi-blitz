use blitz::dom::{LocalName, NodeData};
use napi::{Error, Result, bindgen_prelude::BigInt};
use napi_derive::napi;
use style::properties::PropertyId;

use crate::dom::doc::SharedBaseDoc;
use crate::dom::ops::{
    AttrInit, make_qual_name, mark_inline_style_mutated, remove_detached_attribute,
    set_detached_attribute,
};

const NODE_TYPE_ELEMENT: u32 = 1;
const NODE_TYPE_TEXT: u32 = 3;
const NODE_TYPE_COMMENT: u32 = 8;
const NODE_TYPE_DOCUMENT: u32 = 9;
const NODE_TYPE_OTHER: u32 = 0;

#[napi]
pub struct NodeHandle {
    pub(crate) base: SharedBaseDoc,
    pub(crate) node_id: usize,
}

impl NodeHandle {
    pub(crate) fn new(base: SharedBaseDoc, node_id: usize) -> Self {
        Self { base, node_id }
    }
}

#[napi]
impl NodeHandle {
    #[napi]
    pub fn node_id(&self) -> u64 {
        self.node_id as u64
    }

    #[napi]
    pub fn node_type(&self) -> u32 {
        let state = self.base.doc.borrow();
        let Some(node) = state.get_node(self.node_id) else {
            return NODE_TYPE_OTHER;
        };
        match &node.data {
            NodeData::Document => NODE_TYPE_DOCUMENT,
            NodeData::Element(_) => NODE_TYPE_ELEMENT,
            NodeData::Text(_) => NODE_TYPE_TEXT,
            NodeData::Comment => NODE_TYPE_COMMENT,
            _ => NODE_TYPE_OTHER,
        }
    }

    #[napi]
    pub fn parent_id(&self) -> Option<u64> {
        self.base
            .doc
            .borrow()
            .get_node(self.node_id)
            .and_then(|n| n.parent)
            .map(|id| id as u64)
    }

    #[napi]
    pub fn first_child_id(&self) -> Option<u64> {
        self.base
            .doc
            .borrow()
            .get_node(self.node_id)
            .and_then(|n| n.children.first().copied())
            .map(|id| id as u64)
    }

    #[napi]
    pub fn last_child_id(&self) -> Option<u64> {
        self.base
            .doc
            .borrow()
            .get_node(self.node_id)
            .and_then(|n| n.children.last().copied())
            .map(|id| id as u64)
    }

    #[napi]
    pub fn child_ids(&self) -> Vec<u64> {
        self.base
            .doc
            .borrow()
            .get_node(self.node_id)
            .map(|n| n.children.iter().map(|id| *id as u64).collect())
            .unwrap_or_default()
    }

    #[napi]
    pub fn next_sibling_id(&self) -> Option<u64> {
        self.base
            .doc
            .borrow()
            .get_node(self.node_id)
            .and_then(|n| n.forward(1))
            .map(|n| n.id as u64)
    }

    #[napi]
    pub fn previous_sibling_id(&self) -> Option<u64> {
        self.base
            .doc
            .borrow()
            .get_node(self.node_id)
            .and_then(|n| n.backward(1))
            .map(|n| n.id as u64)
    }

    #[napi]
    pub fn text_content(&self) -> Option<String> {
        let state = self.base.doc.borrow();
        state.get_node(self.node_id).map(|n| n.text_content())
    }

    #[napi]
    pub fn set_text_content(&mut self, text: String) {
        let mut state = self.base.doc.borrow_mut();
        let is_text = state
            .get_node(self.node_id)
            .map(|n| n.is_text_node())
            .unwrap_or(false);
        if is_text {
            let mut mutator = state.mutate();
            mutator.set_node_text(self.node_id, &text);
            drop(mutator);
            drop(state);
            self.base.mark_host_dirty();
            return;
        }

        let children: Vec<usize> = state
            .get_node(self.node_id)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        {
            let mut mutator = state.mutate();
            for child_id in &children {
                mutator.remove_and_drop_node(*child_id);
            }
            let text_id = mutator.create_text_node(&text);
            mutator.append_children(self.node_id, &[text_id]);
        }
        drop(state);
        self.base.mark_host_dirty();
    }

    #[napi]
    pub fn tag_name(&self) -> Option<String> {
        let state = self.base.doc.borrow();
        state
            .get_node(self.node_id)
            .and_then(|n| n.element_data())
            .map(|el| el.name.local.to_string())
    }

    #[napi]
    pub fn get_attribute(&self, name: String) -> Option<String> {
        let state = self.base.doc.borrow();
        let node = state.get_node(self.node_id)?;
        node.attr(LocalName::from(name.as_str()))
            .map(|s| s.to_string())
    }

    #[napi]
    pub fn get_attributes(&self) -> Vec<AttrInit> {
        let state = self.base.doc.borrow();
        let Some(node) = state.get_node(self.node_id) else {
            return Vec::new();
        };
        let Some(attrs) = node.attrs() else {
            return Vec::new();
        };
        attrs
            .iter()
            .map(|attr| AttrInit {
                name: attr.name.local.to_string(),
                value: attr.value.clone(),
                namespace: Some(attr.name.ns.to_string()),
            })
            .collect()
    }

    #[napi]
    pub fn set_attribute(&mut self, name: String, value: String, namespace: Option<String>) {
        let mut state = self.base.doc.borrow_mut();
        let name = make_qual_name(&name, namespace.as_deref());
        if set_detached_attribute(&mut state, self.node_id, name.clone(), &value) {
            drop(state);
            self.base.mark_host_dirty();
            return;
        }
        let mut mutator = state.mutate();
        mutator.set_attribute(self.node_id, name, &value);
        drop(mutator);
        drop(state);
        self.base.mark_host_dirty();
    }

    #[napi]
    pub fn remove_attribute(&mut self, name: String, namespace: Option<String>) {
        let mut state = self.base.doc.borrow_mut();
        let name = make_qual_name(&name, namespace.as_deref());
        if remove_detached_attribute(&mut state, self.node_id, &name) {
            drop(state);
            self.base.mark_host_dirty();
            return;
        }
        let mut mutator = state.mutate();
        mutator.clear_attribute(self.node_id, name);
        drop(mutator);
        drop(state);
        self.base.mark_host_dirty();
    }

    #[napi]
    pub fn get_style_property(&self, name: String) -> Option<String> {
        let state = self.base.doc.borrow();
        let element_data = state.get_node(self.node_id)?.element_data()?;
        let block = element_data.style_attribute.as_ref()?;
        let property_id = PropertyId::parse_enabled_for_all_content(&name).ok()?;

        let guard = state.guard().read();
        let block = block.read_with(&guard);
        let mut buf = String::new();
        block.property_value_to_css(&property_id, &mut buf).ok()?;
        if buf.is_empty() { None } else { Some(buf) }
    }

    #[napi]
    pub fn set_style_property(&mut self, name: String, value: String) {
        let mut state = self.base.doc.borrow_mut();
        mark_inline_style_mutated(&mut state, self.node_id);
        state.set_style_property(self.node_id, &name, &value);
        drop(state);
        self.base.mark_host_dirty();
    }

    #[napi]
    pub fn remove_style_property(&mut self, name: String) {
        let mut state = self.base.doc.borrow_mut();
        mark_inline_style_mutated(&mut state, self.node_id);
        state.remove_style_property(self.node_id, &name);
        drop(state);
        self.base.mark_host_dirty();
    }

    #[napi]
    pub fn get_style_property_names(&self) -> Vec<String> {
        let state = self.base.doc.borrow();
        let Some(element_data) = state.get_node(self.node_id).and_then(|n| n.element_data()) else {
            return Vec::new();
        };
        let Some(block) = element_data.style_attribute.as_ref() else {
            return Vec::new();
        };
        let guard = state.guard().read();
        let block = block.read_with(&guard);
        block
            .declarations()
            .iter()
            .map(|declaration| declaration.id().name().into_owned())
            .collect()
    }

    #[napi]
    pub fn get_style_attribute(&self) -> String {
        let state = self.base.doc.borrow();
        let Some(element_data) = state.get_node(self.node_id).and_then(|n| n.element_data()) else {
            return String::new();
        };
        let Some(block) = element_data.style_attribute.as_ref() else {
            return String::new();
        };
        let guard = state.guard().read();
        let block = block.read_with(&guard);
        let mut buf = String::new();
        let _ = block.to_css(&mut buf);
        buf
    }

    #[napi]
    pub fn append_child(&mut self, child_id: BigInt) {
        let mut state = self.base.doc.borrow_mut();
        let mut mutator = state.mutate();
        mutator.append_children(self.node_id, &[child_id.get_u64().1 as usize]);
        drop(mutator);
        drop(state);
        self.base.mark_host_dirty();
    }

    #[napi]
    pub fn insert_before(&mut self, node_id: BigInt, anchor_id: Option<BigInt>) {
        let mut state = self.base.doc.borrow_mut();
        let mut mutator = state.mutate();
        let node_id = node_id.get_u64().1 as usize;
        match anchor_id {
            Some(anchor_id) => {
                mutator.insert_nodes_before(anchor_id.get_u64().1 as usize, &[node_id]);
            }
            None => {
                mutator.append_children(self.node_id, &[node_id]);
            }
        }
        drop(mutator);
        drop(state);
        self.base.mark_host_dirty();
    }

    #[napi]
    pub fn remove(&mut self) {
        let mut state = self.base.doc.borrow_mut();
        let mut mutator = state.mutate();
        mutator.remove_node(self.node_id);
        drop(mutator);
        drop(state);
        self.base.mark_host_dirty();
    }

    #[napi]
    pub fn replace_with(&mut self, node_id: BigInt) {
        let mut state = self.base.doc.borrow_mut();
        let mut mutator = state.mutate();
        mutator.replace_node_with(self.node_id, &[node_id.get_u64().1 as usize]);
        drop(mutator);
        drop(state);
        self.base.mark_host_dirty();
    }

    #[napi]
    pub fn deep_clone_node(&self) -> u64 {
        let mut state = self.base.doc.borrow_mut();
        let mut mutator = state.mutate();
        let clone_id = mutator.deep_clone_node(self.node_id);
        drop(mutator);
        drop(state);
        self.base.mark_host_dirty();
        clone_id as u64
    }

    #[napi]
    pub fn shallow_clone_node(&self) -> u64 {
        let mut state = self.base.doc.borrow_mut();
        let Some(data) = state.get_node(self.node_id).map(|node| node.data.clone()) else {
            return 0;
        };
        let clone_id = state.create_node(data);
        drop(state);
        self.base.mark_host_dirty();
        clone_id as u64
    }

    #[napi]
    pub fn set_inner_html(&mut self, html: String) {
        let mut state = self.base.doc.borrow_mut();
        let mut mutator = state.mutate();
        mutator.set_inner_html(self.node_id, &html);
        drop(mutator);
        drop(state);
        self.base.mark_host_dirty();
    }

    #[napi]
    pub fn inner_html(&self) -> Option<String> {
        let state = self.base.doc.borrow();
        let node = state.get_node(self.node_id)?;
        let mut out = String::new();
        for &child_id in &node.children {
            if let Some(child) = state.get_node(child_id) {
                child.write_outer_html(&mut out);
            }
        }
        Some(out)
    }

    #[napi]
    pub fn outer_html(&self) -> Option<String> {
        let state = self.base.doc.borrow();
        state.get_node(self.node_id).map(|node| node.outer_html())
    }

    #[napi]
    pub fn query_selector(&self, selector: String) -> Result<Option<u64>> {
        let state = self.base.doc.borrow();
        let selector_list = state
            .try_parse_selector_list(&selector)
            .map_err(|err| Error::from_reason(format!("query_selector: {err:?}")))?;
        let Some(root_node) = state.get_node(self.node_id) else {
            return Ok(None);
        };
        use blitz::dom::Node;
        let mut result: Option<&Node> = None;
        style::dom_apis::query_selector::<&Node, style::dom_apis::QueryFirst>(
            root_node,
            &selector_list,
            &mut result,
            style::dom_apis::MayUseInvalidation::Yes,
        );
        Ok(result.map(|node| node.id as u64))
    }

    #[napi]
    pub fn query_selector_all(&self, selector: String) -> Result<Vec<u64>> {
        let state = self.base.doc.borrow();
        let selector_list = state
            .try_parse_selector_list(&selector)
            .map_err(|err| Error::from_reason(format!("query_selector_all: {err:?}")))?;
        let Some(root_node) = state.get_node(self.node_id) else {
            return Ok(Vec::new());
        };
        use blitz::dom::Node;
        let mut results: style::dom_apis::QuerySelectorAllResult<&Node> = Default::default();
        style::dom_apis::query_selector::<&Node, style::dom_apis::QueryAll>(
            root_node,
            &selector_list,
            &mut results,
            style::dom_apis::MayUseInvalidation::Yes,
        );
        Ok(results.iter().map(|node| node.id as u64).collect())
    }
}
