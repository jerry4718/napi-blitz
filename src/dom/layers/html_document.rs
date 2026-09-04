//! The `HTMLDocument` layer — the concrete document class returned by
//! `createDocument`.

use napi::{Error, Result};
use napi_helpers::inherits::{Constructed, Super, proc::layer};

use crate::layers::document::DocumentLayer;

/// Own block of the `HTMLDocument` class.
#[layer(js_name = "HTMLDocument")]
pub struct HTMLDocumentLayer {}

#[layer]
impl HTMLDocumentLayer {
    #[layer(parent)]
    type Parent = DocumentLayer;

    #[layer(constructor)]
    fn build(_sup: Super<DocumentLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "HTMLDocument cannot be constructed directly; use createDocument",
        ))
    }
}
