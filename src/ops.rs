//! Flat, nodeId-based DOM operations exposed to JS as methods on `DocHandle`.
//!
//! Every method here takes node ids (`u32`) as input and returns plain
//! values. Element wrapper objects live entirely in the JS layer.

use blitz::dom::{Attribute as BlitzAttribute, LocalName, Namespace, QualName, ns};
use blitz::html::DocumentHtmlParser;
use napi::{Error, Result};
use napi_derive::napi;

use crate::doc::DocHandle;

/// Plain attribute pair used by the create/insert APIs.
#[napi(object)]
pub struct AttrInit {
    pub name: String,
    pub value: String,
    pub namespace: Option<String>,
}

fn make_qual_name(local: &str, namespace: Option<&str>) -> QualName {
    QualName {
        prefix: None,
        ns: namespace.map(Namespace::from).unwrap_or(ns!(html)),
        local: LocalName::from(local),
    }
}

#[napi]
impl DocHandle {
    /// Replace document content from an HTML string. Useful for tests and
    /// initial bootstrapping when `base_html` was not enough.
    #[napi]
    pub fn load_html(&mut self, html: String) {
        let mut state = self.state.0.borrow_mut();
        {
            let mut mutator = state.base.mutate();
            DocumentHtmlParser::parse_into_mutator(&mut mutator, &html);
        }
        state.base.resolve(0.0);
    }

    /// Find a single node by CSS selector. Returns its node id or null.
    #[napi]
    pub fn query_selector(&self, selector: String) -> Result<Option<u32>> {
        let state = self.state.0.borrow();
        match state.base.query_selector(&selector) {
            Ok(Some(id)) => Ok(Some(id as u32)),
            Ok(None) => Ok(None),
            Err(err) => Err(Error::from_reason(format!("query_selector: {err:?}"))),
        }
    }

    /// Find all nodes by CSS selector. Returns a list of node ids.
    #[napi]
    pub fn query_selector_all(&self, selector: String) -> Result<Vec<u32>> {
        let state = self.state.0.borrow();
        match state.base.query_selector_all(&selector) {
            Ok(ids) => Ok(ids.into_iter().map(|id| id as u32).collect()),
            Err(err) => Err(Error::from_reason(format!("query_selector_all: {err:?}"))),
        }
    }

    /// Lookup by `id=` attribute, like `document.getElementById`.
    #[napi]
    pub fn get_element_by_id(&self, id: String) -> Option<u32> {
        self.state
            .0
            .borrow()
            .base
            .get_element_by_id(&id)
            .map(|id| id as u32)
    }

    /// True iff the given node id currently exists in the document.
    #[napi]
    pub fn has_node(&self, id: u32) -> bool {
        self.state.0.borrow().base.get_node(id as usize).is_some()
    }
}

#[napi]
impl DocHandle {
    /// Create an element node. Returns its node id. The element is detached
    /// (no parent) until inserted.
    #[napi]
    pub fn create_element(
        &mut self,
        local_name: String,
        namespace: Option<String>,
        attrs: Option<Vec<AttrInit>>,
    ) -> u32 {
        let mut state = self.state.0.borrow_mut();
        let mut mutator = state.base.mutate();
        let qn = make_qual_name(&local_name, namespace.as_deref());
        let attr_vec: Vec<BlitzAttribute> = attrs
            .unwrap_or_default()
            .into_iter()
            .map(|a| BlitzAttribute {
                name: make_qual_name(&a.name, a.namespace.as_deref()),
                value: a.value,
            })
            .collect();
        mutator.create_element(qn, attr_vec) as u32
    }

    /// Create a text node with the given content.
    #[napi]
    pub fn create_text_node(&mut self, text: String) -> u32 {
        let mut state = self.state.0.borrow_mut();
        let mut mutator = state.base.mutate();
        mutator.create_text_node(&text) as u32
    }

    /// Create an empty comment node.
    #[napi]
    pub fn create_comment_node(&mut self) -> u32 {
        let mut state = self.state.0.borrow_mut();
        let mut mutator = state.base.mutate();
        mutator.create_comment_node() as u32
    }

    /// Deep-clone an existing node and return the new node's id.
    #[napi]
    pub fn deep_clone_node(&mut self, node_id: u32) -> u32 {
        let mut state = self.state.0.borrow_mut();
        let mut mutator = state.base.mutate();
        mutator.deep_clone_node(node_id as usize) as u32
    }
}

#[napi]
impl DocHandle {
    /// Parent node id, if any.
    #[napi]
    pub fn parent_id(&self, node_id: u32) -> Option<u32> {
        self.state
            .0
            .borrow()
            .base
            .get_node(node_id as usize)
            .and_then(|n| n.parent)
            .map(|id| id as u32)
    }

    /// First child id, if any.
    #[napi]
    pub fn first_child_id(&self, node_id: u32) -> Option<u32> {
        self.state
            .0
            .borrow()
            .base
            .get_node(node_id as usize)
            .and_then(|n| n.children.first().copied())
            .map(|id| id as u32)
    }

    /// Last child id, if any.
    #[napi]
    pub fn last_child_id(&self, node_id: u32) -> Option<u32> {
        self.state
            .0
            .borrow()
            .base
            .get_node(node_id as usize)
            .and_then(|n| n.children.last().copied())
            .map(|id| id as u32)
    }

    /// All children, in document order.
    #[napi]
    pub fn child_ids(&self, node_id: u32) -> Vec<u32> {
        self.state
            .0
            .borrow()
            .base
            .get_node(node_id as usize)
            .map(|n| n.children.iter().map(|id| *id as u32).collect())
            .unwrap_or_default()
    }

    /// Next sibling id, if any.
    #[napi]
    pub fn next_sibling_id(&self, node_id: u32) -> Option<u32> {
        self.state
            .0
            .borrow()
            .base
            .get_node(node_id as usize)
            .and_then(|n| n.forward(1))
            .map(|n| n.id as u32)
    }

    /// Previous sibling id, if any.
    #[napi]
    pub fn previous_sibling_id(&self, node_id: u32) -> Option<u32> {
        self.state
            .0
            .borrow()
            .base
            .get_node(node_id as usize)
            .and_then(|n| n.backward(1))
            .map(|n| n.id as u32)
    }
}

/// Mirrors web NodeType numeric codes for the small subset blitz exposes.
const NODE_TYPE_ELEMENT: u32 = 1;
const NODE_TYPE_TEXT: u32 = 3;
const NODE_TYPE_COMMENT: u32 = 8;
const NODE_TYPE_DOCUMENT: u32 = 9;
const NODE_TYPE_OTHER: u32 = 0;

#[napi]
impl DocHandle {
    /// DOM-style `nodeType` (1=Element, 3=Text, 8=Comment, 9=Document).
    #[napi]
    pub fn node_type(&self, node_id: u32) -> u32 {
        let state = self.state.0.borrow();
        let Some(node) = state.base.get_node(node_id as usize) else {
            return NODE_TYPE_OTHER;
        };
        use blitz::dom::NodeData;
        match &node.data {
            NodeData::Document => NODE_TYPE_DOCUMENT,
            NodeData::Element(_) => NODE_TYPE_ELEMENT,
            NodeData::Text(_) => NODE_TYPE_TEXT,
            NodeData::Comment => NODE_TYPE_COMMENT,
            _ => NODE_TYPE_OTHER,
        }
    }

    /// Local element tag name (lowercased), e.g. "div". Returns None for
    /// non-element nodes.
    #[napi]
    pub fn tag_name(&self, node_id: u32) -> Option<String> {
        let state = self.state.0.borrow();
        state
            .base
            .get_node(node_id as usize)
            .and_then(|n| n.element_data())
            .map(|el| el.name.local.to_string())
    }

    /// Concatenated text content of this node and its descendants. Mirrors
    /// `Node.textContent`.
    #[napi]
    pub fn text_content(&self, node_id: u32) -> Option<String> {
        let state = self.state.0.borrow();
        state
            .base
            .get_node(node_id as usize)
            .map(|n| n.text_content())
    }

    /// Get an attribute value, or None if missing or node is not an element.
    #[napi]
    pub fn get_attribute(&self, node_id: u32, name: String) -> Option<String> {
        let state = self.state.0.borrow();
        let node = state.base.get_node(node_id as usize)?;
        let local = LocalName::from(name.as_str());
        node.attr(local).map(|s| s.to_string())
    }

    /// All attribute (name, value) pairs on this node, or empty if not an
    /// element.
    #[napi]
    pub fn get_attributes(&self, node_id: u32) -> Vec<AttrInit> {
        let state = self.state.0.borrow();
        let Some(node) = state.base.get_node(node_id as usize) else {
            return Vec::new();
        };
        let Some(attrs) = node.attrs() else {
            return Vec::new();
        };
        attrs
            .iter()
            .map(|a| AttrInit {
                name: a.name.local.to_string(),
                value: a.value.clone(),
                namespace: Some(a.name.ns.to_string()),
            })
            .collect()
    }
}

#[napi]
impl DocHandle {
    /// Set an attribute on an element.
    #[napi]
    pub fn set_attribute(
        &mut self,
        node_id: u32,
        name: String,
        value: String,
        namespace: Option<String>,
    ) {
        let mut state = self.state.0.borrow_mut();
        let mut mutator = state.base.mutate();
        mutator.set_attribute(
            node_id as usize,
            make_qual_name(&name, namespace.as_deref()),
            &value,
        );
    }

    /// Remove an attribute from an element.
    #[napi]
    pub fn remove_attribute(&mut self, node_id: u32, name: String, namespace: Option<String>) {
        let mut state = self.state.0.borrow_mut();
        let mut mutator = state.base.mutate();
        mutator.clear_attribute(
            node_id as usize,
            make_qual_name(&name, namespace.as_deref()),
        );
    }

    /// Set a single inline style property (e.g. "color", "#ff0000").
    #[napi]
    pub fn set_style_property(&mut self, node_id: u32, name: String, value: String) {
        let mut state = self.state.0.borrow_mut();
        state
            .base
            .set_style_property(node_id as usize, &name, &value);
    }

    /// Remove a single inline style property.
    #[napi]
    pub fn remove_style_property(&mut self, node_id: u32, name: String) {
        let mut state = self.state.0.borrow_mut();
        state.base.remove_style_property(node_id as usize, &name);
    }

    /// Replace this node's text content. For elements this resets to a single
    /// text-node child; for text/comment nodes this updates their content.
    #[napi]
    pub fn set_text_content(&mut self, node_id: u32, text: String) {
        let mut state = self.state.0.borrow_mut();
        // For text/comment nodes we update the existing data.
        let is_text = state
            .base
            .get_node(node_id as usize)
            .map(|n| n.is_text_node())
            .unwrap_or(false);
        if is_text {
            let mut mutator = state.base.mutate();
            mutator.set_node_text(node_id as usize, &text);
            return;
        }

        // Otherwise reset element children to a single text node.
        let children: Vec<usize> = state
            .base
            .get_node(node_id as usize)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        {
            let mut mutator = state.base.mutate();
            for c in &children {
                mutator.remove_and_drop_node(*c);
            }
            let text_id = mutator.create_text_node(&text);
            mutator.append_children(node_id as usize, &[text_id]);
        }
    }
}

#[napi]
impl DocHandle {
    /// Append `child` as the last child of `parent`. Mirrors `Node.appendChild`.
    #[napi]
    pub fn append_child(&mut self, parent_id: u32, child_id: u32) {
        let mut state = self.state.0.borrow_mut();
        let mut mutator = state.base.mutate();
        mutator.append_children(parent_id as usize, &[child_id as usize]);
    }

    /// Insert `node` immediately before `anchor`. If `anchor` is None, behaves
    /// like `appendChild`. Matches `Node.insertBefore`.
    #[napi]
    pub fn insert_before(&mut self, parent_id: u32, node_id: u32, anchor_id: Option<u32>) {
        let mut state = self.state.0.borrow_mut();
        let mut mutator = state.base.mutate();
        match anchor_id {
            Some(anchor) => {
                mutator.insert_nodes_before(anchor as usize, &[node_id as usize]);
            }
            None => {
                mutator.append_children(parent_id as usize, &[node_id as usize]);
            }
        }
    }

    /// Insert `node` immediately after `anchor`.
    #[napi]
    pub fn insert_after(&mut self, anchor_id: u32, node_id: u32) {
        let mut state = self.state.0.borrow_mut();
        let mut mutator = state.base.mutate();
        mutator.insert_nodes_after(anchor_id as usize, &[node_id as usize]);
    }

    /// Detach a node from its parent. The node is kept around (still
    /// addressable by id) so JS wrappers stay valid. Use `drop_node` to
    /// release storage.
    #[napi]
    pub fn remove(&mut self, node_id: u32) {
        let mut state = self.state.0.borrow_mut();
        let mut mutator = state.base.mutate();
        mutator.remove_node(node_id as usize);
    }

    /// Detach and free the node.
    #[napi]
    pub fn drop_node(&mut self, node_id: u32) {
        let mut state = self.state.0.borrow_mut();
        let mut mutator = state.base.mutate();
        mutator.remove_and_drop_node(node_id as usize);
    }

    /// Replace `anchor` with `node` in its parent.
    #[napi]
    pub fn replace_with(&mut self, anchor_id: u32, node_id: u32) {
        let mut state = self.state.0.borrow_mut();
        let mut mutator = state.base.mutate();
        mutator.replace_node_with(anchor_id as usize, &[node_id as usize]);
    }

    /// Replace this element's inner HTML.
    #[napi]
    pub fn set_inner_html(&mut self, node_id: u32, html: String) {
        let mut state = self.state.0.borrow_mut();
        let mut mutator = state.base.mutate();
        mutator.set_inner_html(node_id as usize, &html);
    }

    /// Serialize this node (including the node itself) to HTML. Mirrors
    /// `Element.outerHTML`. Returns None for unknown nodes.
    #[napi]
    pub fn outer_html(&self, node_id: u32) -> Option<String> {
        let state = self.state.0.borrow();
        state
            .base
            .get_node(node_id as usize)
            .map(|n| n.outer_html())
    }

    /// Serialize the children of this node to HTML, without the node's own
    /// open/close tags. Mirrors `Element.innerHTML`.
    #[napi]
    pub fn inner_html(&self, node_id: u32) -> Option<String> {
        let state = self.state.0.borrow();
        let node = state.base.get_node(node_id as usize)?;
        let mut out = String::new();
        for &child_id in &node.children {
            if let Some(child) = state.base.get_node(child_id) {
                child.write_outer_html(&mut out);
            }
        }
        Some(out)
    }
}
