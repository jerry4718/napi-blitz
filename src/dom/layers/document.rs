//! The `Document` layer — document-level queries and node creation.

use std::rc::Rc;

use blitz::dom::{Attribute as BlitzAttribute, LocalName, Node, local_name};
use napi::{Env, Error, Result};
use napi_helpers::{
    anything::Anything,
    inherits::{Constructed, Super, proc::layer},
};

use crate::dom::{
    layers::node::NodeLayer,
    shared::{
        doc::SharedDocument,
        ops::{AttrInit, make_qual_name, to_anything},
        wrap_node,
    },
};

/// Own block of the `Document` class. The blitz document node is always
/// the root node, so the members here work off `doc` alone (the parent
/// `NodeLayer` slot carries the `node_id`).
#[layer]
pub struct DocumentLayer {
    pub(crate) shared: Rc<SharedDocument>,
}

#[layer(js_name = "Document")]
impl DocumentLayer {
    #[layer(parent)]
    type Parent = NodeLayer;

    #[layer(constructor)]
    fn build(_sup: Super<NodeLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "Document is abstract; use createDocument",
        ))
    }

    /// Replace document content from an HTML string.
    #[layer]
    fn load_html(&mut self, html: String) {
        let mut state = self.shared.base_mut();
        {
            let mut mutator = state.mutate();
            blitz::html::DocumentHtmlParser::parse_into_mutator(&mut mutator, &html);
        }
        state.resolve(0.0);
        drop(state);
        self.shared.mark_host_dirty();
    }

    #[layer]
    fn query_selector(&self, selector: String, env: &Env) -> Result<Option<Anything>> {
        let state = self.shared.base();
        match state.query_selector(&selector) {
            Ok(Some(id)) => Ok(Some(to_anything(wrap_node(&self.shared, env, id)?, env)?)),
            Ok(None) => Ok(None),
            Err(err) => Err(Error::from_reason(format!("query_selector: {err:?}"))),
        }
    }

    #[layer]
    fn query_selector_all(&self, selector: String, env: &Env) -> Result<Vec<Anything>> {
        let state = self.shared.base();
        match state.query_selector_all(&selector) {
            Ok(ids) => {
                let mut result = Vec::new();
                for id in ids {
                    result.push(to_anything(wrap_node(&self.shared, env, id)?, env)?);
                }
                Ok(result)
            }
            Err(err) => Err(Error::from_reason(format!("query_selector_all: {err:?}"))),
        }
    }

    #[layer]
    fn get_element_by_id(&self, id: String, env: &Env) -> Option<Anything> {
        let node_id = self.shared.base().get_element_by_id(&id)?;
        to_anything(wrap_node(&self.shared, env, node_id).ok()?, env).ok()
    }

    #[layer]
    fn get_elements_by_tag_name(&self, name: String, env: &Env) -> Vec<Anything> {
        let doc = self.shared.clone();
        // Tag matching is ASCII case-insensitive per the HTML spec.
        let name = name.to_ascii_lowercase();
        let root = doc.base().root_node().id;
        let ids = doc.dfs(root, |n| name == "*" || is_element_with_tag(n, &name));
        ids.into_iter()
            .filter_map(|id| to_anything(wrap_node(&doc, env, id).ok()?, env).ok())
            .collect()
    }

    #[layer]
    fn get_elements_by_class_name(&self, class_name: String, env: &Env) -> Vec<Anything> {
        let doc = self.shared.clone();
        let root = doc.base().root_node().id;
        let ids = doc.dfs(root, |n| {
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
        let mut state = self.shared.base_mut();
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
        self.shared.mark_host_dirty();
        to_anything(wrap_node(&self.shared, env, node_id)?, env)
    }

    #[layer]
    fn create_text_node(&mut self, text: String, env: &Env) -> Result<Anything> {
        let mut state = self.shared.base_mut();
        let mut mutator = state.mutate();
        let node_id = mutator.create_text_node(&text);
        drop(mutator);
        drop(state);
        self.shared.mark_host_dirty();
        to_anything(wrap_node(&self.shared, env, node_id)?, env)
    }

    #[layer]
    fn create_comment(&mut self, text: String, env: &Env) -> Result<Anything> {
        let mut state = self.shared.base_mut();
        let mut mutator = state.mutate();
        let node_id = mutator.create_comment_node(&text);
        drop(mutator);
        drop(state);
        self.shared.mark_host_dirty();
        to_anything(wrap_node(&self.shared, env, node_id)?, env)
    }

    #[layer(getter)]
    fn document_element(&self, env: &Env) -> Option<Anything> {
        let id = self.shared.find_first(|n| is_element_with_tag(n, "html"))?;
        to_anything(wrap_node(&self.shared, env, id).ok()?, env).ok()
    }

    #[layer(getter)]
    fn head(&self, env: &Env) -> Option<Anything> {
        let id = self.shared.find_first(|n| is_element_with_tag(n, "head"))?;
        to_anything(wrap_node(&self.shared, env, id).ok()?, env).ok()
    }

    #[layer(getter)]
    fn body(&self, env: &Env) -> Option<Anything> {
        let id = self.shared.find_first(|n| is_element_with_tag(n, "body"))?;
        to_anything(wrap_node(&self.shared, env, id).ok()?, env).ok()
    }

    #[layer(getter)]
    fn title(&self) -> String {
        let Some(id) = self.shared.find_first(|n| is_element_with_tag(n, "title")) else {
            return String::new();
        };
        self.shared
            .base()
            .get_node(id)
            .map(|n| n.text_content())
            .unwrap_or_default()
    }

    /// `document.title = ...` — update the existing `<title>`'s text, or
    /// create one inside `<head>` when the document has none.
    #[layer(setter)]
    fn set_title(&mut self, title: String) {
        // All lookups borrow the document; run them before `mutate()`.
        let existing = self.shared.find_first(|n| is_element_with_tag(n, "title"));
        let head = self.shared.find_first(|n| is_element_with_tag(n, "head"));
        // `<title>` is an element; its text lives in the Text child.
        let text_child = existing.and_then(|id| {
            let state = self.shared.base();
            let child = state.get_node(id).and_then(|n| n.children.first().copied());
            child.filter(|&c| state.get_node(c).map(|t| t.is_text_node()).unwrap_or(false))
        });
        let mut state = self.shared.base_mut();
        let mut mutator = state.mutate();
        match existing {
            Some(id) => match text_child {
                Some(c) => mutator.set_node_text(c, &title),
                None => {
                    let text_id = mutator.create_text_node(&title);
                    mutator.append_children(id, &[text_id]);
                }
            },
            None => {
                if let Some(head) = head {
                    let title_id = mutator.create_element(make_qual_name("title", None), vec![]);
                    let text_id = mutator.create_text_node(&title);
                    mutator.append_children(title_id, &[text_id]);
                    mutator.append_children(head, &[title_id]);
                }
            }
        }
        drop(mutator);
        drop(state);
        self.shared.mark_host_dirty();
    }
}

#[inline]
fn is_element_with_tag(node: &Node, name: &str) -> bool {
    node.data.is_element_with_tag_name(&LocalName::from(name))
}
