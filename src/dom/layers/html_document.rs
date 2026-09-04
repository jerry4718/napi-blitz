//! The `HTMLDocument` layer — the concrete document class returned by
//! `createDocument`.

use napi::{Error, Result};
use napi_helpers::inherits::{Constructed, Super, proc::layer};

use crate::layers::document::DocumentLayer;

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
