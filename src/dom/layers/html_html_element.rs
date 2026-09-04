//! The `HTMLHtmlElement` layer — the `<html>` root element.

use napi::{Error, Result};
use napi_helpers::inherits::{Constructed, Super, proc::layer};

use crate::dom::layers::html_element::HTMLElementLayer;

/// Own block of the `HTMLHtmlElement` class.
#[layer]
pub struct HTMLHtmlElementLayer {}

#[layer(js_name = "HTMLHtmlElement")]
impl HTMLHtmlElementLayer {
    #[layer(parent)]
    type Parent = HTMLElementLayer;

    #[layer(constructor)]
    fn build(_sup: Super<HTMLElementLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "HTMLHtmlElement cannot be constructed directly; create via document.createElement",
        ))
    }
}
