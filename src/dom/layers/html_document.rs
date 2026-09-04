//! The `HTMLDocument` layer — the concrete document class returned by
//! `createDocument`.

use napi::{Env, Error, Result, bindgen_prelude::Object};
use napi_helpers::inherits::{Constructed, LayerRef, Super, proc::layer, with_own};

use crate::dom::{
    fonts::FontFaceSetLayer,
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
    fn create(env: &Env, config: Option<DocHandleConfig>) -> Result<LayerRef<HTMLDocumentLayer>> {
        let document = create_document(env, config)?;
        LayerRef::new(&document, env)
    }

    /// `document.fonts` — the document's `FontFaceSet`, created when the
    /// document is initialized.
    #[layer(getter, ts_return_type = "FontFaceSet | null")]
    fn fonts(&self, this: &Object, env: &Env) -> Result<Option<LayerRef<FontFaceSetLayer>>> {
        let shared = with_own::<DocumentLayer, _>(this, |d| d.shared.clone())?;
        let fonts = shared.fonts().clone();
        match fonts {
            Some(r) => {
                let raw = unsafe { r.raw_value(env)? };
                let obj = Object::from_raw(env.raw(), raw);
                Ok(Some(LayerRef::new(&obj, env)?))
            }
            None => Ok(None),
        }
    }
}
