//! The `HTMLDocument` layer — the concrete document class returned by
//! `createDocument`.

use napi::{Env, Error, JsValue, Result, bindgen_prelude::Object};
use napi_helpers::{
    anything::{Anything, OtherRef},
    inherits::{Constructed, Super, proc::layer, with_own},
};

use crate::dom::{
    layers::document::DocumentLayer,
    shared::{doc::DocHandleConfig, doc::create_document},
};

/// Own block of the `HTMLDocument` class.
#[layer]
pub struct HTMLDocumentLayer {}

#[layer(js_name = "HTMLDocument")]
impl HTMLDocumentLayer {
    #[layer(parent)]
    type Parent = DocumentLayer;

    #[layer(constructor)]
    fn build(_sup: Super<DocumentLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "HTMLDocument cannot be constructed directly; use createDocument",
        ))
    }

    #[layer]
    fn create(env: &Env, config: Option<DocHandleConfig>) -> Result<Anything> {
        let document = create_document(env, config)?;
        unsafe { OtherRef::new(env.raw(), document.raw()) }.map(Anything::Object)
    }

    /// `document.fonts` — the document's `FontFaceSet`, created when the
    /// document is initialized.
    #[layer(getter)]
    fn fonts(&self, this: &Object) -> Result<Anything> {
        let shared = with_own::<DocumentLayer, _>(this, |d| d.shared.clone())?;
        let fonts = shared.fonts().clone();
        Ok(fonts.map(Anything::Object).unwrap_or(Anything::Null))
    }
}
