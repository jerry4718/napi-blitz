//! The `HTMLTextAreaElement` layer — value/focused members for
//! `<textarea>` elements.

use std::rc::Rc;

use blitz::dom::NodeId;
use napi::{Env, Error, Result, bindgen_prelude::Object};
use napi_helpers::inherit as napi_inherit;
use napi_helpers::inherit::layer::{Constructed, Super};
use napi_helpers::inherit::proc::layer;
use wintertc_events::event::EventLayer;

use crate::layers::element::ElementLayer;
use crate::layers::html_element::HTMLElementLayer;
use crate::layers::node::NodeLayer;
use crate::shared::doc::SharedDoc;
use crate::shared::ops::make_qual_name;

/// Own block of the `HTMLTextAreaElement` class.
#[layer(js_name = "HTMLTextAreaElement", parent = HTMLElementLayer)]
pub struct HTMLTextAreaElementLayer {
    pub(crate) node_id: NodeId,
    pub(crate) doc: Rc<SharedDoc>,
}

impl HTMLTextAreaElementLayer {
    fn value_of(&self) -> String {
        let base = self.doc.base.borrow();
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
        let mut base = self.doc.base.borrow_mut();
        let qual = make_qual_name("value", None);
        if !crate::shared::ops::set_detached_attribute(
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

        let mut base = self.doc.base.borrow_mut();
        base.with_text_input(self.node_id, |mut driver| {
            driver.editor.set_text(&value);
            driver.refresh_layout();
        });
        drop(base);
        self.doc.mark_host_dirty();
    }

    fn focused_of(&self) -> bool {
        self.doc.base.borrow().get_focussed_node_id() == Some(self.node_id)
    }
}

#[layer]
impl HTMLTextAreaElementLayer {
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
}
