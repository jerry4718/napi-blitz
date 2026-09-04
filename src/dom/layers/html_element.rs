//! The `HTMLElement` layer — parent of the concrete HTML element classes.

use napi::{Error, Result, bindgen_prelude::Object};
use napi_helpers::inherit as napi_inherit;
use napi_helpers::inherit::layer::{Constructed, Super};
use napi_helpers::inherit::proc::layer;
use wintertc_events::event::EventLayer;

use crate::layers::element::ElementLayer;
use crate::layers::node::NodeLayer;

/// Own block of the `HTMLElement` class.
#[layer(js_name = "HTMLElement", parent = ElementLayer)]
pub struct HTMLElementLayer {}

#[layer]
impl HTMLElementLayer {
    #[layer(constructor)]
    fn build(_sup: Super<ElementLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "HTMLElement is abstract; create elements via document APIs",
        ))
    }
}
