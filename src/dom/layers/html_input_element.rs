//! The `HTMLInputElement` layer — value/checked/focused members for
//! `<input>` elements, read through the blitz text-input / special data.

use std::rc::Rc;

use blitz::dom::{LocalName, NodeId};
use napi::{Error, Result};
use napi_helpers::inherits::{Constructed, Super, proc::layer};

use crate::dom::{
    layers::html_element::HTMLElementLayer,
    shared::{
        doc::SharedDocument,
        ops::{make_qual_name, remove_detached_attribute, set_detached_attribute},
    },
};

/// Own block of the `HTMLInputElement` class.
#[layer]
pub struct HTMLInputElementLayer {
    pub(crate) node_id: NodeId,
    pub(crate) shared_doc: Rc<SharedDocument>,
}

impl HTMLInputElementLayer {
    fn value_of(&self) -> Result<String> {
        let base = self.shared_doc.base();
        let Some(node) = base.get_node(self.node_id) else {
            return Ok(String::new());
        };
        if let Some(el) = node.element_data()
            && let Some(ti) = el.text_input_data()
        {
            return Ok(ti.editor.text().to_string());
        }
        Ok(node
            .attr(LocalName::from("value"))
            .map(|s| s.to_string())
            .unwrap_or_default())
    }

    fn apply_value(&self, value: String) {
        let mut state = self.shared_doc.base_mut();
        let qual = make_qual_name("value", None);
        if !set_detached_attribute(&mut state, self.node_id, qual.clone(), &value) {
            let mut mutator = state.mutate();
            mutator.set_attribute(self.node_id, qual, &value);
            drop(mutator);
        }
        drop(state);

        let mut state = self.shared_doc.base_mut();
        state.with_text_input(self.node_id, |mut driver| {
            driver.editor.set_text(&value);
            driver.refresh_layout();
        });
        drop(state);
        self.shared_doc.mark_host_dirty();
    }

    fn checked_of(&self) -> bool {
        let base = self.shared_doc.base();
        let Some(node) = base.get_node(self.node_id) else {
            return false;
        };
        let Some(el) = node.element_data() else {
            return false;
        };
        match &el.special_data {
            blitz::dom::node::SpecialElementData::CheckboxInput(c) => *c,
            _ => node.attr(LocalName::from("checked")).is_some(),
        }
    }

    fn apply_checked(&self, checked: bool) {
        {
            let mut base = self.shared_doc.base_mut();
            if let Some(node) = base.get_node_mut(self.node_id)
                && let Some(el) = node.element_data_mut()
            {
                el.special_data = blitz::dom::node::SpecialElementData::CheckboxInput(checked);
            }
        }
        let mut base = self.shared_doc.base_mut();
        let qual = make_qual_name("checked", None);
        if checked {
            if !set_detached_attribute(&mut base, self.node_id, qual.clone(), "") {
                let mut mutator = base.mutate();
                mutator.set_attribute(self.node_id, qual, "");
            }
        } else {
            remove_detached_attribute(&mut base, self.node_id, &qual);
        }
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
            .and_then(|n| n.attr(LocalName::from(name)).map(|s| s.to_string()))
            .unwrap_or_default()
    }

    fn attr_flag(&self, name: &str) -> bool {
        self.shared_doc
            .base()
            .get_node(self.node_id)
            .map(|n| n.attr(LocalName::from(name)).is_some())
            .unwrap_or(false)
    }

    fn apply_attr(&self, name: &str, value: &str) {
        let mut state = self.shared_doc.base_mut();
        let qual = make_qual_name(name, None);
        if !set_detached_attribute(&mut state, self.node_id, qual.clone(), value) {
            let mut mutator = state.mutate();
            mutator.set_attribute(self.node_id, qual, value);
            drop(mutator);
        }
        drop(state);
        self.shared_doc.mark_host_dirty();
    }

    fn remove_attr(&self, name: &str) {
        let mut state = self.shared_doc.base_mut();
        let qual = make_qual_name(name, None);
        if !remove_detached_attribute(&mut state, self.node_id, &qual) {
            let mut mutator = state.mutate();
            mutator.clear_attribute(self.node_id, qual);
            drop(mutator);
        }
        drop(state);
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

#[layer(js_name = "HTMLInputElement")]
impl HTMLInputElementLayer {
    #[layer(parent)]
    type Parent = HTMLElementLayer;

    #[layer(constructor)]
    fn build(_sup: Super<HTMLElementLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "HTMLInputElement cannot be constructed directly; create via document.createElement",
        ))
    }

    #[layer(getter)]
    fn value(&self) -> Result<String> {
        self.value_of()
    }

    #[layer(setter)]
    fn set_value(&mut self, value: String) {
        self.apply_value(value);
    }

    #[layer(getter)]
    fn checked(&self) -> bool {
        self.checked_of()
    }

    #[layer(setter)]
    fn set_checked(&mut self, checked: bool) {
        self.apply_checked(checked);
    }

    #[layer(getter)]
    fn focused(&self) -> bool {
        self.focused_of()
    }

    #[layer(getter)]
    fn type_(&self) -> String {
        let v = self.attr_str("type");
        if v.is_empty() { "text".to_string() } else { v }
    }

    #[layer(setter)]
    fn set_type(&mut self, value: String) {
        self.apply_attr("type", &value);
    }

    #[layer(getter)]
    fn disabled(&self) -> bool {
        self.attr_flag("disabled")
    }

    #[layer(setter)]
    fn set_disabled(&mut self, value: bool) {
        self.set_flag("disabled", value);
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
