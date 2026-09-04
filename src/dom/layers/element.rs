//! The `Element` layer — parent of `HTMLElement`. Adds attribute, style,
//! inner/outer HTML, selector query, and layout/scroll/focus members.

use std::cell::RefCell;
use std::rc::Rc;

use blitz::dom::{NodeId, local_name};
use napi::{Env, Error, Result};
use napi_helpers::{
    anything::Anything,
    inherits::{Constructed, Super, from_chain, proc::layer},
    proxy::new_proxy,
};
use style::properties::PropertyId;

use crate::dom::{
    layers::{
        attributes_handler::AttributesHandlerLayer,
        node::NodeLayer,
        style_handler::{StyleHandlerLayer, StyleMethods},
    },
    shared::{
        doc::SharedDocument,
        ops::{
            AttrInit, make_qual_name, mark_inline_style_mutated, remove_detached_attribute,
            set_detached_attribute, to_anything,
        },
        wrap_node,
    },
};

/// Mutable per-element state placeholder (see `NodeState`).
#[derive(Default)]
pub struct ElementState {}

/// Own block of the `Element` class. Carries its own `node_id`/`doc` copy
/// (they never change once assigned) so the members here don't need to
/// re-materialize the parent `NodeLayer` slot on every call.
#[layer]
pub struct ElementLayer {
    pub(crate) node_id: NodeId,
    pub(crate) shared_doc: Rc<SharedDocument>,
    #[allow(dead_code)]
    pub(crate) state: ElementState,
}

#[layer(js_name = "Element")]
impl ElementLayer {
    #[layer(parent)]
    type Parent = NodeLayer;

    #[layer(constructor)]
    fn build(
        _type_: String,
        _init: Option<napi::bindgen_prelude::Either<(), ()>>,
        _sup: Super<NodeLayer>,
    ) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "Element is abstract; create elements via document APIs",
        ))
    }

    #[layer(getter)]
    fn tag_name(&self) -> Option<String> {
        let base = self.shared_doc.base();
        base.get_node(self.node_id)
            .and_then(|n| n.element_data())
            .map(|el| el.name.local.to_string())
    }

    /// `element.hasAttribute(name)` — presence of the content attribute.
    #[layer]
    fn has_attribute(&self, name: String) -> bool {
        Self::attr_has(&self.shared_doc, self.node_id, &name)
    }

    #[layer]
    fn get_attribute(&self, name: String) -> Option<String> {
        Self::attribute(&self.shared_doc, self.node_id, &name)
    }

    #[layer]
    fn get_attributes(&self) -> Vec<AttrInit> {
        Self::attr_list(&self.shared_doc, self.node_id)
    }

    #[layer]
    fn set_attribute(&mut self, name: String, value: String, namespace: Option<String>) {
        Self::attr_set(
            &self.shared_doc,
            self.node_id,
            &name,
            &value,
            namespace.as_deref(),
        );
    }

    #[layer]
    fn remove_attribute(&mut self, name: String, namespace: Option<String>) {
        Self::attr_remove(&self.shared_doc, self.node_id, &name, namespace.as_deref());
    }

    #[layer]
    fn get_style_property(&self, name: String) -> Option<String> {
        Self::style_property(&self.shared_doc, self.node_id, &name)
    }

    #[layer]
    fn set_style_property(&mut self, name: String, value: String) {
        Self::style_set(&self.shared_doc, self.node_id, &name, &value);
    }

    #[layer]
    fn remove_style_property(&mut self, name: String) {
        Self::style_remove(&self.shared_doc, self.node_id, &name);
    }

    #[layer]
    fn get_style_property_names(&self) -> Vec<String> {
        Self::style_property_names(&self.shared_doc, self.node_id)
    }

    #[layer]
    fn get_style_attribute(&self) -> String {
        Self::style_css_text(&self.shared_doc, self.node_id)
    }

    #[layer(setter, js_name = "innerHTML")]
    fn set_inner_html(&mut self, env: &Env, html: String) {
        self.shared_doc.detach_children(self.node_id, env).ok();
        let mut base = self.shared_doc.base_mut();
        let mut mutator = base.mutate();
        mutator.set_inner_html(self.node_id, &html);
        drop(mutator);
        drop(base);
        self.shared_doc.mark_host_dirty();
    }

    /// `element.id` — mirrors the `id` content attribute.
    #[layer(getter)]
    fn id(&self) -> Option<String> {
        let base = self.shared_doc.base();
        base.get_node(self.node_id)
            .and_then(|n| n.attr(local_name!("id")).map(str::to_string))
    }

    #[layer(setter)]
    fn set_id(&mut self, id: String) {
        let mut base = self.shared_doc.base_mut();
        let name = make_qual_name("id", None);
        if set_detached_attribute(&mut base, self.node_id, name.clone(), &id) {
            drop(base);
            self.shared_doc.mark_host_dirty();
            return;
        }
        let mut mutator = base.mutate();
        mutator.set_attribute(self.node_id, name, &id);
        drop(mutator);
        drop(base);
        self.shared_doc.mark_host_dirty();
    }

    /// `element.className` — mirrors the `class` content attribute.
    #[layer(getter)]
    fn class_name(&self) -> String {
        let base = self.shared_doc.base();
        base.get_node(self.node_id)
            .and_then(|n| n.attr(local_name!("class")).map(str::to_string))
            .unwrap_or_default()
    }

    #[layer(setter)]
    fn set_class_name(&mut self, value: String) {
        let mut base = self.shared_doc.base_mut();
        let name = make_qual_name("class", None);
        if set_detached_attribute(&mut base, self.node_id, name.clone(), &value) {
            drop(base);
            self.shared_doc.mark_host_dirty();
            return;
        }
        let mut mutator = base.mutate();
        mutator.set_attribute(self.node_id, name, &value);
        drop(mutator);
        drop(base);
        self.shared_doc.mark_host_dirty();
    }

    /// `element.getElementsByTagName` — tag-matching descendants in tree
    /// order. Tag comparison is ASCII case-insensitive per the HTML spec.
    #[layer]
    fn get_elements_by_tag_name(&self, name: String, env: &Env) -> Vec<Anything> {
        let doc = self.shared_doc.clone();
        let name = name.to_ascii_lowercase();
        let self_id = self.node_id;
        let ids = doc.dfs(self_id, |n| {
            n.id != self_id
                && (name == "*"
                    || n.data
                        .is_element_with_tag_name(&blitz::dom::LocalName::from(name.as_str())))
        });
        ids.into_iter()
            .filter_map(|id| to_anything(wrap_node(&doc, env, id).ok()?, env).ok())
            .collect()
    }

    /// `element.getElementsByClassName` — descendants whose `class`
    /// attribute contains the exact token.
    #[layer]
    fn get_elements_by_class_name(&self, class_name: String, env: &Env) -> Vec<Anything> {
        let doc = self.shared_doc.clone();
        let self_id = self.node_id;
        let ids = doc.dfs(self_id, |n| {
            n.id != self_id
                && n.attr(local_name!("class"))
                    .map(|c| c.split_whitespace().any(|w| w == class_name))
                    .unwrap_or(false)
        });
        ids.into_iter()
            .filter_map(|id| to_anything(wrap_node(&doc, env, id).ok()?, env).ok())
            .collect()
    }

    #[layer(getter, js_name = "innerHTML")]
    fn inner_html(&self) -> Option<String> {
        let base = self.shared_doc.base();
        let node = base.get_node(self.node_id)?;
        let mut out = String::new();
        for &child_id in &node.children {
            if let Some(child) = base.get_node(child_id) {
                child.write_outer_html(&mut out);
            }
        }
        Some(out)
    }

    #[layer(getter, js_name = "outerHTML")]
    fn outer_html(&self) -> Option<String> {
        let base = self.shared_doc.base();
        base.get_node(self.node_id).map(|node| node.outer_html())
    }

    #[layer]
    fn query_selector(&self, selector: String, env: &Env) -> Result<Option<Anything>> {
        let result_id = {
            let base = self.shared_doc.base();
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
            Some(id) => Ok(Some(to_anything(
                wrap_node(&self.shared_doc, env, id)?,
                env,
            )?)),
            None => Ok(None),
        }
    }

    #[layer]
    fn query_selector_all(&self, selector: String, env: &Env) -> Result<Vec<Anything>> {
        let ids: Vec<blitz::dom::NodeId> = {
            let base = self.shared_doc.base();
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
            out.push(to_anything(wrap_node(&self.shared_doc, env, id)?, env)?);
        }
        Ok(out)
    }

    #[layer]
    fn get_bounding_client_rect(&self) -> Option<crate::dom::shared::ops::DomRect> {
        let base = self.shared_doc.base();
        let node = base.get_node(self.node_id)?;
        let pos = node.absolute_position(0.0, 0.0);
        let layout = node.final_layout();
        Some(crate::dom::shared::ops::DomRect {
            x: pos.x as f64,
            y: pos.y as f64,
            width: layout.size.width as f64,
            height: layout.size.height as f64,
            top: pos.y as f64,
            left: pos.x as f64,
            bottom: (pos.y + layout.size.height) as f64,
            right: (pos.x + layout.size.width) as f64,
        })
    }

    #[layer(getter)]
    fn scroll_top(&self) -> f64 {
        let base = self.shared_doc.base();
        base.get_node(self.node_id)
            .map(|n| n.scroll_offset().y)
            .unwrap_or(0.0)
    }

    #[layer(setter)]
    fn set_scroll_top(&mut self, value: f64) {
        let mut base = self.shared_doc.base_mut();
        if let Some(node) = base.get_node_mut(self.node_id) {
            let offset = node.scroll_offset_mut();
            offset.y = value;
        }
        drop(base);
        self.shared_doc.mark_host_dirty();
    }

    #[layer(getter)]
    fn scroll_left(&self) -> f64 {
        let base = self.shared_doc.base();
        base.get_node(self.node_id)
            .map(|n| n.scroll_offset().x)
            .unwrap_or(0.0)
    }

    #[layer(setter)]
    fn set_scroll_left(&mut self, value: f64) {
        let mut base = self.shared_doc.base_mut();
        if let Some(node) = base.get_node_mut(self.node_id) {
            let offset = node.scroll_offset_mut();
            offset.x = value;
        }
        drop(base);
        self.shared_doc.mark_host_dirty();
    }

    #[layer(getter)]
    fn scroll_height(&self) -> f64 {
        let base = self.shared_doc.base();
        base.get_node(self.node_id)
            .map(|n| {
                let layout = n.final_layout();
                layout.content_box_height() as f64
            })
            .unwrap_or(0.0)
    }

    #[layer(getter)]
    fn scroll_width(&self) -> f64 {
        let base = self.shared_doc.base();
        base.get_node(self.node_id)
            .map(|n| {
                let layout = n.final_layout();
                layout.content_box_width() as f64
            })
            .unwrap_or(0.0)
    }

    #[layer(getter)]
    fn client_height(&self) -> f64 {
        let base = self.shared_doc.base();
        base.get_node(self.node_id)
            .map(|n| n.final_layout().content_box_height() as f64)
            .unwrap_or(0.0)
    }

    #[layer(getter)]
    fn client_width(&self) -> f64 {
        let base = self.shared_doc.base();
        base.get_node(self.node_id)
            .map(|n| n.final_layout().content_box_width() as f64)
            .unwrap_or(0.0)
    }

    #[layer]
    fn focus(&mut self) -> bool {
        let mut base = self.shared_doc.base_mut();
        base.set_focus_to(self.node_id)
    }

    #[layer]
    fn blur(&mut self) {
        let mut base = self.shared_doc.base_mut();
        base.clear_focus();
        drop(base);
        self.shared_doc.mark_host_dirty();
    }

    /// `element.style` — the CSSOM `CSSStyleDeclaration` Proxy over the
    /// inline style block. The proxy is cached per element, so repeated
    /// reads return the same object.
    #[layer(getter)]
    fn style(&self, env: &Env) -> Result<Anything> {
        if let Some(proxy) = self.shared_doc.style_proxy(self.node_id) {
            return Ok(proxy);
        }
        let handler = from_chain!((StyleHandlerLayer, env)
            StyleHandlerLayer {
                node_id: self.node_id,
                shared_doc: Rc::clone(&self.shared_doc),
                methods: RefCell::new(StyleMethods::default()),
            },
        )?;
        let proxy = new_proxy(env, &handler)?;
        self.shared_doc.set_style_proxy(self.node_id, proxy.clone());
        Ok(proxy)
    }

    /// `element.attributes` — the NamedNodeMap-ish Proxy over the content
    /// attributes. Cached like `style`.
    #[layer(getter)]
    fn attributes(&self, env: &Env) -> Result<Anything> {
        if let Some(proxy) = self.shared_doc.attributes_proxy(self.node_id) {
            return Ok(proxy);
        }
        let handler = from_chain!((AttributesHandlerLayer, env)
            AttributesHandlerLayer {
                node_id: self.node_id,
                shared_doc: Rc::clone(&self.shared_doc),
            },
        )?;
        let proxy = new_proxy(env, &handler)?;
        self.shared_doc
            .set_attributes_proxy(self.node_id, proxy.clone());
        Ok(proxy)
    }

    pub(crate) fn attribute(doc: &SharedDocument, node_id: NodeId, name: &str) -> Option<String> {
        let base = doc.base();
        let node = base.get_node(node_id)?;
        node.attr(blitz::dom::LocalName::from(name))
            .map(|s| s.to_string())
    }

    pub(crate) fn attr_has(doc: &SharedDocument, node_id: NodeId, name: &str) -> bool {
        doc.base()
            .get_node(node_id)
            .map(|n| n.attr(blitz::dom::LocalName::from(name)).is_some())
            .unwrap_or(false)
    }

    pub(crate) fn attr_list(doc: &SharedDocument, node_id: NodeId) -> Vec<AttrInit> {
        let base = doc.base();
        let Some(node) = base.get_node(node_id) else {
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

    pub(crate) fn attr_set(
        doc: &SharedDocument,
        node_id: NodeId,
        name: &str,
        value: &str,
        namespace: Option<&str>,
    ) {
        let mut base = doc.base_mut();
        let name = make_qual_name(name, namespace);
        if set_detached_attribute(&mut base, node_id, name.clone(), value) {
            drop(base);
            doc.mark_host_dirty();
            return;
        }
        let mut mutator = base.mutate();
        mutator.set_attribute(node_id, name, value);
        drop(mutator);
        drop(base);
        doc.mark_host_dirty();
    }

    pub(crate) fn attr_remove(
        doc: &SharedDocument,
        node_id: NodeId,
        name: &str,
        namespace: Option<&str>,
    ) {
        let mut base = doc.base_mut();
        let name = make_qual_name(name, namespace);
        if remove_detached_attribute(&mut base, node_id, &name) {
            drop(base);
            doc.mark_host_dirty();
            return;
        }
        let mut mutator = base.mutate();
        mutator.clear_attribute(node_id, name);
        drop(mutator);
        drop(base);
        doc.mark_host_dirty();
    }

    pub(crate) fn style_property(
        doc: &SharedDocument,
        node_id: NodeId,
        name: &str,
    ) -> Option<String> {
        let base = doc.base();
        let element_data = base.get_node(node_id)?.element_data()?;
        let block = element_data.style_attribute.as_ref()?;
        let property_id = PropertyId::parse_enabled_for_all_content(name).ok()?;

        let guard = base.guard().read();
        let block = block.read_with(&guard);
        let mut buf = String::new();
        block.property_value_to_css(&property_id, &mut buf).ok()?;
        if buf.is_empty() { None } else { Some(buf) }
    }

    pub(crate) fn style_set(doc: &SharedDocument, node_id: NodeId, name: &str, value: &str) {
        let mut base = doc.base_mut();
        mark_inline_style_mutated(&mut base, node_id);
        base.set_style_property(node_id, name, value);
        drop(base);
        doc.mark_host_dirty();
    }

    pub(crate) fn style_remove(doc: &SharedDocument, node_id: NodeId, name: &str) {
        let mut base = doc.base_mut();
        mark_inline_style_mutated(&mut base, node_id);
        base.remove_style_property(node_id, name);
        drop(base);
        doc.mark_host_dirty();
    }

    pub(crate) fn style_property_names(doc: &SharedDocument, node_id: NodeId) -> Vec<String> {
        let base = doc.base();
        let Some(element_data) = base.get_node(node_id).and_then(|n| n.element_data()) else {
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

    pub(crate) fn style_css_text(doc: &SharedDocument, node_id: NodeId) -> String {
        let base = doc.base();
        let Some(element_data) = base.get_node(node_id).and_then(|n| n.element_data()) else {
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
}
