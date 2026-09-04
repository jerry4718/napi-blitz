//! The `HTMLDocument` layer — the concrete document class returned by
//! `createDocument`.

use napi::{Error, Result, bindgen_prelude::Object};
use napi_helpers::inherit as napi_inherit;
use napi_helpers::inherit::layer::{Constructed, Super};
use napi_helpers::inherit::proc::layer;
use wintertc_events::event::EventLayer;

use crate::layers::document::DocumentLayer;
use crate::layers::node::NodeLayer;

/// Own block of the `HTMLDocument` class.
#[layer(js_name = "HTMLDocument", parent = DocumentLayer)]
pub struct HTMLDocumentLayer {}

#[layer]
impl HTMLDocumentLayer {
    #[layer(constructor)]
    fn build(_sup: Super<DocumentLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "HTMLDocument cannot be constructed directly; use createDocument",
        ))
    }
}
