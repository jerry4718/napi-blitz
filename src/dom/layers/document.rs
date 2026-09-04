//! The `Document` layer — document-level queries and node creation.

use std::rc::Rc;

use blitz::dom::{Attribute as BlitzAttribute, NodeId, local_name};
use napi::{Env, Error, Result};
use napi_helpers::{
    anything::Anything,
    inherits::{Constructed, Super, proc::layer},
};

use crate::{
    layers::node::NodeLayer,
    shared::{
        doc::{SharedDoc, wrap_node},
        ops::{AttrInit, make_qual_name, to_anything},
    },
};

/// Own block of the `Document` class. The blitz document node is always
/// the root node, so the members here work off `doc` alone (the parent
/// `NodeLayer` slot carries the `node_id`).
#[layer(js_name = "Document", parent = NodeLayer)]
pub struct DocumentLayer {
    pub(crate) doc: Rc<SharedDoc>,
}

/// Pre-order DFS over the document tree, starting from the given node
/// (inclusive). `pred` decides which node ids are collected.
fn dfs<F>(doc: &SharedDoc, root: NodeId, pred: F) -> Vec<NodeId>
where
    F: Fn(&blitz::dom::Node) -> bool,
{
    let state = doc.base.borrow();
    let mut out = Vec::new();
    let mut stack: Vec<NodeId> = vec![root];
    while let Some(id) = stack.pop() {
        let Some(node) = state.get_node(id) else {
            continue;
        };
        if pred(node) {
            out.push(id);
        }
        for &child in node.children.iter().rev() {
            stack.push(child);
        }
    }
    out
}

fn find_first<F>(doc: &SharedDoc, pred: F) -> Option<NodeId>
where
    F: Fn(&blitz::dom::Node) -> bool,
{
    let state = doc.base.borrow();
    let mut stack: Vec<NodeId> = vec![state.root_node().id];
    while let Some(id) = stack.pop() {
        let node = state.get_node(id)?;
        if pred(node) {
            return Some(id);
        }
        for &child in node.children.iter().rev() {
            stack.push(child);
        }
    }
    None
}

fn is_element_with_tag(node: &blitz::dom::Node, name: &str) -> bool {
    node.data
        .is_element_with_tag_name(&blitz::dom::LocalName::from(name))
}

#[layer]
impl DocumentLayer {
    #[layer(constructor)]
    fn build(_sup: Super<NodeLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "Document is abstract; use createDocument",
        ))
    }

    /// Replace document content from an HTML string.
    #[layer]
    fn load_html(&mut self, html: String) {
        let mut state = self.doc.base.borrow_mut();
        {
            let mut mutator = state.mutate();
            blitz::html::DocumentHtmlParser::parse_into_mutator(&mut mutator, &html);
        }
        state.resolve(0.0);
        drop(state);
        self.doc.mark_host_dirty();
    }

    #[layer]
    fn query_selector(&self, selector: String, env: &Env) -> Result<Option<Anything>> {
        let state = self.doc.base.borrow();
        match state.query_selector(&selector) {
            Ok(Some(id)) => Ok(Some(to_anything(wrap_node(&self.doc, env, id)?, env)?)),
            Ok(None) => Ok(None),
            Err(err) => Err(Error::from_reason(format!("query_selector: {err:?}"))),
        }
    }

    #[layer]
    fn query_selector_all(&self, selector: String, env: &Env) -> Result<Vec<Anything>> {
        let state = self.doc.base.borrow();
        match state.query_selector_all(&selector) {
            Ok(ids) => {
                let mut result = Vec::new();
                for id in ids {
                    result.push(to_anything(wrap_node(&self.doc, env, id)?, env)?);
                }
                Ok(result)
            }
            Err(err) => Err(Error::from_reason(format!("query_selector_all: {err:?}"))),
        }
    }

    #[layer]
    fn get_element_by_id(&self, id: String, env: &Env) -> Option<Anything> {
        let node_id = self.doc.base.borrow().get_element_by_id(&id)?;
        to_anything(wrap_node(&self.doc, env, node_id).ok()?, env).ok()
    }

    #[layer]
    fn get_elements_by_tag_name(&self, name: String, env: &Env) -> Vec<Anything> {
        let doc = self.doc.clone();
        let root = doc.base.borrow().root_node().id;
        let ids = dfs(&doc, root, |n| is_element_with_tag(n, &name));
        ids.into_iter()
            .filter_map(|id| to_anything(wrap_node(&doc, env, id).ok()?, env).ok())
            .collect()
    }

    #[layer]
    fn get_elements_by_class_name(&self, class_name: String, env: &Env) -> Vec<Anything> {
        let doc = self.doc.clone();
        let root = doc.base.borrow().root_node().id;
        let ids = dfs(&doc, root, |n| {
            n.attr(local_name!("class"))
                .map(|c| c.split_whitespace().any(|w| w == class_name))
                .unwrap_or(false)
        });
        ids.into_iter()
            .filter_map(|id| to_anything(wrap_node(&doc, env, id).ok()?, env).ok())
            .collect()
    }

    #[layer]
    fn create_element(
        &mut self,
        local_name: String,
        namespace: Option<String>,
        attrs: Option<Vec<AttrInit>>,
        env: &Env,
    ) -> Result<Anything> {
        let mut state = self.doc.base.borrow_mut();
        let mut mutator = state.mutate();
        let qn = make_qual_name(&local_name, namespace.as_deref());
        let attr_vec: Vec<BlitzAttribute> = attrs
            .unwrap_or_default()
            .into_iter()
            .map(|a| BlitzAttribute {
                name: make_qual_name(&a.name, a.namespace.as_deref()),
                value: a.value,
            })
            .collect();
        let node_id = mutator.create_element(qn, attr_vec);
        drop(mutator);
        drop(state);
        self.doc.mark_host_dirty();
        Ok(to_anything(wrap_node(&self.doc, env, node_id)?, env)?)
    }

    #[layer]
    fn create_text_node(&mut self, text: String, env: &Env) -> Result<Anything> {
        let mut state = self.doc.base.borrow_mut();
        let mut mutator = state.mutate();
        let node_id = mutator.create_text_node(&text);
        drop(mutator);
        drop(state);
        self.doc.mark_host_dirty();
        Ok(to_anything(wrap_node(&self.doc, env, node_id)?, env)?)
    }

    #[layer]
    fn create_comment(&mut self, text: String, env: &Env) -> Result<Anything> {
        let mut state = self.doc.base.borrow_mut();
        let mut mutator = state.mutate();
        let node_id = mutator.create_comment_node(&text);
        drop(mutator);
        drop(state);
        self.doc.mark_host_dirty();
        Ok(to_anything(wrap_node(&self.doc, env, node_id)?, env)?)
    }

    #[layer]
    fn document_element(&self, env: &Env) -> Option<Anything> {
        let id = find_first(&self.doc.clone(), |n| is_element_with_tag(n, "html"))?;
        to_anything(wrap_node(&self.doc, env, id).ok()?, env).ok()
    }

    #[layer]
    fn head(&self, env: &Env) -> Option<Anything> {
        let id = find_first(&self.doc.clone(), |n| is_element_with_tag(n, "head"))?;
        to_anything(wrap_node(&self.doc, env, id).ok()?, env).ok()
    }

    #[layer]
    fn body(&self, env: &Env) -> Option<Anything> {
        let id = find_first(&self.doc.clone(), |n| is_element_with_tag(n, "body"))?;
        to_anything(wrap_node(&self.doc, env, id).ok()?, env).ok()
    }

    #[layer(getter)]
    fn title(&self) -> String {
        let Some(id) = find_first(&self.doc.clone(), |n| is_element_with_tag(n, "title")) else {
            return String::new();
        };
        self.doc
            .base
            .borrow()
            .get_node(id)
            .map(|n| n.text_content())
            .unwrap_or_default()
    }
}
