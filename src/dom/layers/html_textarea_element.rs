//! The `HTMLTextAreaElement` layer — value/focused members for
//! `<textarea>` elements.

use std::rc::Rc;

use blitz::dom::NodeId;
use napi::{Error, Result};
use napi_helpers::inherits::{Constructed, Super, proc::layer};

use crate::dom::{
    layers::html_element::HTMLElementLayer,
    shared::{doc::SharedDocument, ops::make_qual_name},
};

/// Own block of the `HTMLTextAreaElement` class.
#[layer]
pub struct HTMLTextAreaElementLayer {
    pub(crate) node_id: NodeId,
    pub(crate) shared_doc: Rc<SharedDocument>,
}

impl HTMLTextAreaElementLayer {
    fn value_of(&self) -> String {
        let base = self.shared_doc.base();
        let Some(node) = base.get_node(self.node_id) else {
            return String::new();
        };
        if let Some(el) = node.element_data()
            && let Some(ti) = el.text_input_data()
        {
            return ti.editor.text().to_string();
        }
        node.attr(blitz::dom::LocalName::from("value"))
            .map(|s| s.to_string())
            .unwrap_or_default()
    }

    fn apply_value(&self, value: String) {
        let mut base = self.shared_doc.base_mut();
        let qual = make_qual_name("value", None);
        if !crate::dom::shared::ops::set_detached_attribute(
            &mut base,
            self.node_id,
            qual.clone(),
            &value,
        ) {
            let mut mutator = base.mutate();
            mutator.set_attribute(self.node_id, qual, &value);
            drop(mutator);
        }
        drop(base);

        let mut base = self.shared_doc.base_mut();
        base.with_text_input(self.node_id, |mut driver| {
            driver.editor.set_text(&value);
            driver.refresh_layout();
        });
        drop(base);
        self.shared_doc.mark_host_dirty();
    }

    fn focused_of(&self) -> bool {
        self.shared_doc.base().get_focussed_node_id() == Some(self.node_id)
    }

    // ---- Attribute-backed property helpers (mirror the old JS scheme) ----

    fn attr_str(&self, name: &str) -> String {
        self.shared_doc
            .base()
            .get_node(self.node_id)
            .and_then(|n| {
                n.attr(blitz::dom::LocalName::from(name))
                    .map(|s| s.to_string())
            })
            .unwrap_or_default()
    }

    fn attr_num(&self, name: &str, fallback: i32) -> i32 {
        self.attr_str(name).trim().parse().unwrap_or(fallback)
    }

    fn attr_flag(&self, name: &str) -> bool {
        self.shared_doc
            .base()
            .get_node(self.node_id)
            .map(|n| n.attr(blitz::dom::LocalName::from(name)).is_some())
            .unwrap_or(false)
    }

    fn apply_attr(&self, name: &str, value: &str) {
        let mut base = self.shared_doc.base_mut();
        let qual = make_qual_name(name, None);
        if !crate::dom::shared::ops::set_detached_attribute(
            &mut base,
            self.node_id,
            qual.clone(),
            value,
        ) {
            let mut mutator = base.mutate();
            mutator.set_attribute(self.node_id, qual, value);
            drop(mutator);
        }
        drop(base);
        self.shared_doc.mark_host_dirty();
    }

    fn remove_attr(&self, name: &str) {
        let mut base = self.shared_doc.base_mut();
        let qual = make_qual_name(name, None);
        if !crate::dom::shared::ops::remove_detached_attribute(&mut base, self.node_id, &qual) {
            let mut mutator = base.mutate();
            mutator.clear_attribute(self.node_id, qual);
            drop(mutator);
        }
        drop(base);
        self.shared_doc.mark_host_dirty();
    }

    /// Boolean content attribute: present means true.
    fn set_flag(&self, name: &str, on: bool) {
        if on {
            self.apply_attr(name, "");
        } else {
            self.remove_attr(name);
        }
    }
}

#[layer(js_name = "HTMLTextAreaElement")]
impl HTMLTextAreaElementLayer {
    #[layer(parent)]
    type Parent = HTMLElementLayer;

    #[layer(constructor)]
    fn build(_sup: Super<HTMLElementLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "HTMLTextAreaElement cannot be constructed directly; create via document.createElement",
        ))
    }

    #[layer(getter)]
    fn value(&self) -> String {
        self.value_of()
    }

    #[layer(setter)]
    fn set_value(&mut self, value: String) {
        self.apply_value(value);
    }

    #[layer(getter)]
    fn focused(&self) -> bool {
        self.focused_of()
    }

    /// `rows` defaults to 2 per the old implementation.
    #[layer(getter)]
    fn rows(&self) -> i32 {
        self.attr_num("rows", 2)
    }

    #[layer(setter)]
    fn set_rows(&mut self, value: i32) {
        self.apply_attr("rows", &value.to_string());
    }

    /// `cols` defaults to 20 per the old implementation.
    #[layer(getter)]
    fn cols(&self) -> i32 {
        self.attr_num("cols", 20)
    }

    #[layer(setter)]
    fn set_cols(&mut self, value: i32) {
        self.apply_attr("cols", &value.to_string());
    }

    #[layer(getter)]
    fn placeholder(&self) -> String {
        self.attr_str("placeholder")
    }

    #[layer(setter)]
    fn set_placeholder(&mut self, value: String) {
        self.apply_attr("placeholder", &value);
    }

    /// `readOnly` mirrors the `readonly` content attribute.
    #[layer(getter)]
    fn read_only(&self) -> bool {
        self.attr_flag("readonly")
    }

    #[layer(setter)]
    fn set_read_only(&mut self, value: bool) {
        self.set_flag("readonly", value);
    }

    #[layer(getter)]
    fn required(&self) -> bool {
        self.attr_flag("required")
    }

    #[layer(setter)]
    fn set_required(&mut self, value: bool) {
        self.set_flag("required", value);
    }

    #[layer(getter)]
    fn name(&self) -> String {
        self.attr_str("name")
    }

    #[layer(setter)]
    fn set_name(&mut self, value: String) {
        self.apply_attr("name", &value);
    }

    #[layer(getter)]
    fn disabled(&self) -> bool {
        self.attr_flag("disabled")
    }

    #[layer(setter)]
    fn set_disabled(&mut self, value: bool) {
        self.set_flag("disabled", value);
    }

    /// `defaultValue` mirrors the `value` content attribute.
    #[layer(getter)]
    fn default_value(&self) -> String {
        self.attr_str("value")
    }

    #[layer(setter)]
    fn set_default_value(&mut self, value: String) {
        self.apply_attr("value", &value);
    }
}
