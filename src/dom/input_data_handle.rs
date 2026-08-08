//! `InputDataHandle`: a JS-facing handle for `<input>` / `<textarea>` specific
//! data that cannot be expressed through the generic attribute interface.
//!
//! Created in `wrap_node` when the element tag is "input" or "textarea" and
//! passed as the third constructor argument. The JS subclass stores this
//! reference and forwards only the properties that need native-side access
//! (value syncs with the parley editor; checked syncs with special_data).
//!
//! Pure attribute-backed properties (type, disabled, placeholder, readOnly,
//! required, name, rows, cols) are handled in JS via the inherited
//! getAttribute/setAttribute from Element, not here.

use std::rc::Rc;

use blitz::dom::{LocalName, NodeId};

use crate::dom::doc::SharedDoc;
use crate::dom::ops::{make_qual_name, remove_detached_attribute, set_detached_attribute};

#[napi]
pub struct InputDataHandle {
    pub(crate) node_id: NodeId,
    pub(crate) doc: Rc<SharedDoc>,
}

impl InputDataHandle {
    pub(crate) fn new(node_id: NodeId, doc: Rc<SharedDoc>) -> Self {
        Self { node_id, doc }
    }
}

#[napi]
impl InputDataHandle {
    // ---- value ----------------------------------------------------------

    /// Current text value of the input/textarea.
    ///
    /// If a live `TextInputData` exists (created during layout), returns
    /// the editor's text. Otherwise falls back to the `value` attribute.
    #[napi(getter)]
    pub fn value(&self) -> String {
        let base = self.doc.base.borrow();
        let Some(node) = base.get_node(self.node_id) else {
            return String::new();
        };
        if let Some(el) = node.element_data()
            && let Some(ti) = el.text_input_data()
        {
            return ti.editor.text().to_string();
        }
        node.attr(LocalName::from("value"))
            .map(|s| s.to_string())
            .unwrap_or_default()
    }

    #[napi(setter)]
    pub fn set_value(&mut self, value: String) {
        // 1. Update the value attribute.
        let qual = make_qual_name("value", None);
        let mut base = self.doc.base.borrow_mut();
        if set_detached_attribute(&mut base, self.node_id, qual, &value) {
            drop(base);
        } else {
            let mut mutator = base.mutate();
            mutator.set_attribute(self.node_id, make_qual_name("value", None), &value);
            drop(mutator);
            drop(base);
        }
        self.doc.mark_host_dirty();

        // 2. Sync the editor text if TextInputData exists.
        let mut base = self.doc.base.borrow_mut();
        base.with_text_input(self.node_id, |mut driver| {
            driver.editor.set_text(&value);
            driver.refresh_layout();
        });
    }

    // ---- checked --------------------------------------------------------

    /// Checked state for checkbox/radio inputs.
    ///
    /// Reads from `CheckboxInput` special data if present, otherwise
    /// falls back to the `checked` attribute.
    #[napi(getter)]
    pub fn checked(&self) -> bool {
        let base = self.doc.base.borrow();
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

    #[napi(setter)]
    pub fn set_checked(&mut self, checked: bool) {
        // 1. Update special_data if it's a checkbox.
        {
            let mut base = self.doc.base.borrow_mut();
            if let Some(node) = base.get_node_mut(self.node_id)
                && let Some(el) = node.element_data_mut()
                && matches!(
                    el.special_data,
                    blitz::dom::node::SpecialElementData::CheckboxInput(_)
                )
            {
                el.special_data = blitz::dom::node::SpecialElementData::CheckboxInput(checked);
            }
        }

        // 2. Sync the checked attribute.
        let qual = make_qual_name("checked", None);
        let mut base = self.doc.base.borrow_mut();
        if checked {
            if !set_detached_attribute(&mut base, self.node_id, qual.clone(), "") {
                let mut mutator = base.mutate();
                mutator.set_attribute(self.node_id, qual, "");
            }
        } else {
            remove_detached_attribute(&mut base, self.node_id, &qual);
        }
        drop(base);
        self.doc.mark_host_dirty();
    }

    // ---- focus ----------------------------------------------------------

    /// Whether this input currently has focus.
    #[napi(getter)]
    pub fn focused(&self) -> bool {
        let base = self.doc.base.borrow();
        base.get_focussed_node_id() == Some(self.node_id)
    }
}
