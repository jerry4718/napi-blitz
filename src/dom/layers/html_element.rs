//! The `HTMLElement` layer — parent of the concrete HTML element classes.

use napi::{Error, Result};
use napi_helpers::inherits::{Constructed, Super, proc::layer};

use crate::dom::layers::element::ElementLayer;

/// Own block of the `HTMLElement` class.
#[layer]
pub struct HTMLElementLayer {}

#[layer(js_name = "HTMLElement")]
impl HTMLElementLayer {
    #[layer(parent)]
    type Parent = ElementLayer;

    #[layer(constructor)]
    fn build(_sup: Super<ElementLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "HTMLElement is abstract; create elements via document APIs",
        ))
    }
}
