use blitz::dom::{LocalName, NodeData};
use napi::{Env, Error, Result, bindgen_prelude::Object};
use napi_derive::napi;
use style::properties::PropertyId;

use crate::dom::doc::{SharedDoc, wrap_node};
use crate::dom::ops::{
    AttrInit, make_qual_name, mark_inline_style_mutated, remove_detached_attribute,
    set_detached_attribute,
};
use std::rc::Rc;

const NODE_TYPE_ELEMENT: u32 = 1;
const NODE_TYPE_TEXT: u32 = 3;
const NODE_TYPE_COMMENT: u32 = 8;
const NODE_TYPE_DOCUMENT: u32 = 9;
const NODE_TYPE_OTHER: u32 = 0;

#[napi]
pub struct NodeHandle {
    pub(crate) node_id: usize,
    pub(crate) doc: Rc<SharedDoc>,
}

impl NodeHandle {
    pub(crate) fn new(node_id: usize, doc: Rc<SharedDoc>) -> Self {
        Self { node_id, doc }
    }
}

#[napi]
impl NodeHandle {
    #[napi]
    pub fn node_type(&self) -> u32 {
        let base = self.doc.base.borrow();
        let Some(node) = base.get_node(self.node_id) else {
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
    pub fn parent_node<'a>(&self, env: &'a Env) -> Option<Object<'a>> {
        let parent_id = self.doc.base.borrow()
            .get_node(self.node_id)
            .and_then(|n| n.parent)?;
        wrap_node(&self.doc, parent_id, env).ok()
    }

    #[napi]
    pub fn first_child<'a>(&self, env: &'a Env) -> Option<Object<'a>> {
        let child_id = self.doc.base.borrow()
            .get_node(self.node_id)
            .and_then(|n| n.children.first().copied())?;
        wrap_node(&self.doc, child_id, env).ok()
    }

    #[napi]
    pub fn last_child<'a>(&self, env: &'a Env) -> Option<Object<'a>> {
        let child_id = self.doc.base.borrow()
            .get_node(self.node_id)
            .and_then(|n| n.children.last().copied())?;
        wrap_node(&self.doc, child_id, env).ok()
    }

    #[napi]
    pub fn next_sibling<'a>(&self, env: &'a Env) -> Option<Object<'a>> {
        let sibling_id = {
            let base = self.doc.base.borrow();
            base.get_node(self.node_id)
                .and_then(|n| n.forward(1))
                .map(|n| n.id)
        }?;
        wrap_node(&self.doc, sibling_id, env).ok()
    }

    #[napi]
    pub fn previous_sibling<'a>(&self, env: &'a Env) -> Option<Object<'a>> {
        let sibling_id = {
            let base = self.doc.base.borrow();
            base.get_node(self.node_id)
                .and_then(|n| n.backward(1))
                .map(|n| n.id)
        }?;
        wrap_node(&self.doc, sibling_id, env).ok()
    }

    #[napi]
    pub fn child_nodes<'a>(&self, env: &'a Env) -> Vec<Object<'a>> {
        let children: Vec<usize> = self.doc.base.borrow()
            .get_node(self.node_id)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        children.into_iter()
            .filter_map(|id| wrap_node(&self.doc, id, env).ok())
            .collect()
    }

    #[napi]
    pub fn text_content(&self) -> Option<String> {
        let base = self.doc.base.borrow();
        base.get_node(self.node_id).map(|n| n.text_content())
    }

    #[napi]
    pub fn set_text_content(&mut self, text: String) {
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

        let children: Vec<usize> = base
            .get_node(self.node_id)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        {
            let mut mutator = base.mutate();
            for child_id in &children {
                mutator.remove_and_drop_node(*child_id);
            }
            let text_id = mutator.create_text_node(&text);
            mutator.append_children(self.node_id, &[text_id]);
        }
        drop(base);
        self.doc.mark_host_dirty();
    }

    #[napi]
    pub fn tag_name(&self) -> Option<String> {
        let base = self.doc.base.borrow();
        base.get_node(self.node_id)
            .and_then(|n| n.element_data())
            .map(|el| el.name.local.to_string())
    }

    #[napi]
    pub fn get_attribute(&self, name: String) -> Option<String> {
        let base = self.doc.base.borrow();
        let node = base.get_node(self.node_id)?;
        node.attr(LocalName::from(name.as_str()))
            .map(|s| s.to_string())
    }

    #[napi]
    pub fn get_attributes(&self) -> Vec<AttrInit> {
        let base = self.doc.base.borrow();
        let Some(node) = base.get_node(self.node_id) else {
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
        let mut base = self.doc.base.borrow_mut();
        let name = make_qual_name(&name, namespace.as_deref());
        if set_detached_attribute(&mut base, self.node_id, name.clone(), &value) {
            drop(base);
            self.doc.mark_host_dirty();
            return;
        }
        let mut mutator = base.mutate();
        mutator.set_attribute(self.node_id, name, &value);
        drop(mutator);
        drop(base);
        self.doc.mark_host_dirty();
    }

    #[napi]
    pub fn remove_attribute(&mut self, name: String, namespace: Option<String>) {
        let mut base = self.doc.base.borrow_mut();
        let name = make_qual_name(&name, namespace.as_deref());
        if remove_detached_attribute(&mut base, self.node_id, &name) {
            drop(base);
            self.doc.mark_host_dirty();
            return;
        }
        let mut mutator = base.mutate();
        mutator.clear_attribute(self.node_id, name);
        drop(mutator);
        drop(base);
        self.doc.mark_host_dirty();
    }

    #[napi]
    pub fn get_style_property(&self, name: String) -> Option<String> {
        let base = self.doc.base.borrow();
        let element_data = base.get_node(self.node_id)?.element_data()?;
        let block = element_data.style_attribute.as_ref()?;
        let property_id = PropertyId::parse_enabled_for_all_content(&name).ok()?;

        let guard = base.guard().read();
        let block = block.read_with(&guard);
        let mut buf = String::new();
        block.property_value_to_css(&property_id, &mut buf).ok()?;
        if buf.is_empty() { None } else { Some(buf) }
    }

    #[napi]
    pub fn set_style_property(&mut self, name: String, value: String) {
        let mut base = self.doc.base.borrow_mut();
        mark_inline_style_mutated(&mut base, self.node_id);
        base.set_style_property(self.node_id, &name, &value);
        drop(base);
        self.doc.mark_host_dirty();
    }

    #[napi]
    pub fn remove_style_property(&mut self, name: String) {
        let mut base = self.doc.base.borrow_mut();
        mark_inline_style_mutated(&mut base, self.node_id);
        base.remove_style_property(self.node_id, &name);
        drop(base);
        self.doc.mark_host_dirty();
    }

    #[napi]
    pub fn get_style_property_names(&self) -> Vec<String> {
        let base = self.doc.base.borrow();
        let Some(element_data) = base.get_node(self.node_id).and_then(|n| n.element_data()) else {
            return Vec::new();
        };
        let Some(block) = element_data.style_attribute.as_ref() else {
            return Vec::new();
        };
        let guard = base.guard().read();
        let block = block.read_with(&guard);
        block
            .declarations()
            .iter()
            .map(|declaration| declaration.id().name().into_owned())
            .collect()
    }

    #[napi]
    pub fn get_style_attribute(&self) -> String {
        let base = self.doc.base.borrow();
        let Some(element_data) = base.get_node(self.node_id).and_then(|n| n.element_data()) else {
            return String::new();
        };
        let Some(block) = element_data.style_attribute.as_ref() else {
            return String::new();
        };
        let guard = base.guard().read();
        let block = block.read_with(&guard);
        let mut buf = String::new();
        let _ = block.to_css(&mut buf);
        buf
    }

    #[napi]
    pub fn append_child<'a>(&mut self, child: &NodeHandle, env: &'a Env) -> Object<'a> {
        let mut base = self.doc.base.borrow_mut();
        let mut mutator = base.mutate();
        mutator.append_children(self.node_id, &[child.node_id]);
        drop(mutator);
        drop(base);
        self.doc.mark_host_dirty();
        wrap_node(&self.doc, child.node_id, env).unwrap()
    }

    #[napi]
    pub fn insert_before<'a>(&mut self, node: &NodeHandle, anchor: Option<&NodeHandle>, env: &'a Env) -> Object<'a> {
        let mut base = self.doc.base.borrow_mut();
        let mut mutator = base.mutate();
        match anchor {
            Some(anchor) => {
                mutator.insert_nodes_before(anchor.node_id, &[node.node_id]);
            }
            None => {
                mutator.append_children(self.node_id, &[node.node_id]);
            }
        }
        drop(mutator);
        drop(base);
        self.doc.mark_host_dirty();
        wrap_node(&self.doc, node.node_id, env).unwrap()
    }

    #[napi]
    pub fn remove(&mut self) {
        let mut base = self.doc.base.borrow_mut();
        let mut mutator = base.mutate();
        mutator.remove_node(self.node_id);
        drop(mutator);
        drop(base);
        self.doc.mark_host_dirty();
    }

    #[napi]
    pub fn replace_with<'a>(&mut self, node: &NodeHandle, env: &'a Env) -> Object<'a> {
        let mut base = self.doc.base.borrow_mut();
        let mut mutator = base.mutate();
        mutator.replace_node_with(self.node_id, &[node.node_id]);
        drop(mutator);
        drop(base);
        self.doc.mark_host_dirty();
        wrap_node(&self.doc, node.node_id, env).unwrap()
    }

    #[napi]
    pub fn clone_node<'a>(&self, deep: bool, env: &'a Env) -> Object<'a> {
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
                return wrap_node(&self.doc, self.node_id, env).unwrap();
            };
            let clone_id = base.create_node(data);
            drop(base);
            clone_id
        };
        self.doc.mark_host_dirty();
        wrap_node(&self.doc, new_id, env).unwrap()
    }

    #[napi]
    pub fn set_inner_html(&mut self, html: String) {
        let mut base = self.doc.base.borrow_mut();
        let mut mutator = base.mutate();
        mutator.set_inner_html(self.node_id, &html);
        drop(mutator);
        drop(base);
        self.doc.mark_host_dirty();
    }

    #[napi]
    pub fn inner_html(&self) -> Option<String> {
        let base = self.doc.base.borrow();
        let node = base.get_node(self.node_id)?;
        let mut out = String::new();
        for &child_id in &node.children {
            if let Some(child) = base.get_node(child_id) {
                child.write_outer_html(&mut out);
            }
        }
        Some(out)
    }

    #[napi]
    pub fn outer_html(&self) -> Option<String> {
        let base = self.doc.base.borrow();
        base.get_node(self.node_id).map(|node| node.outer_html())
    }

    #[napi]
    pub fn query_selector<'a>(&self, selector: String, env: &'a Env) -> Result<Option<Object<'a>>> {
        let result_id = {
            let base = self.doc.base.borrow();
            let selector_list = base
                .try_parse_selector_list(&selector)
                .map_err(|err| Error::from_reason(format!("query_selector: {err:?}")))?;
            let Some(root_node) = base.get_node(self.node_id) else {
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
            result.map(|node| node.id)
        };
        match result_id {
            Some(id) => Ok(Some(wrap_node(&self.doc, id, env)?)),
            None => Ok(None),
        }
    }

    #[napi]
    pub fn query_selector_all<'a>(&self, selector: String, env: &'a Env) -> Result<Vec<Object<'a>>> {
        let ids: Vec<usize> = {
            let base = self.doc.base.borrow();
            let selector_list = base
                .try_parse_selector_list(&selector)
                .map_err(|err| Error::from_reason(format!("query_selector_all: {err:?}")))?;
            let Some(root_node) = base.get_node(self.node_id) else {
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
            results.iter().map(|node| node.id).collect()
        };
        let mut out = Vec::new();
        for id in ids {
            out.push(wrap_node(&self.doc, id, env)?);
        }
        Ok(out)
    }
}
