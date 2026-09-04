//! The `FontFace` layer — a loadable font face handed to
//! `document.fonts.add(face)` to make its bytes available to layout/paint.
//!
//! Construction does not register anything: `FontFaceSet.add` ships the
//! bytes into the set's `FontContext`.

use std::cell::Cell;

use napi::{
    Env, Error, JsValue, Result, Status,
    bindgen_prelude::{FnArgs, Object, Uint8Array},
    sys,
};
use napi_helpers::{
    Deferred,
    anything::Anything,
    inherits::{Constructed, RootLayer, Super, proc::layer},
};

/// `dictionary FontFaceDescriptors` — the descriptor subset we honor.
#[napi(object)]
#[derive(Default)]
pub struct FontFaceDescriptors {
    pub style: Option<String>,
    pub weight: Option<String>,
    pub stretch: Option<String>,
    pub unicode_range: Option<String>,
    pub variant: Option<String>,
    pub feature_settings: Option<String>,
    pub display: Option<String>,
}

/// Standard `FontFace.status` values.
#[derive(Clone, Copy, PartialEq)]
enum FaceStatus {
    Unloaded,
    Loading,
    Loaded,
    Error,
}

impl FaceStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Unloaded => "unloaded",
            Self::Loading => "loading",
            Self::Loaded => "loaded",
            Self::Error => "error",
        }
    }
}

/// Own block of the `FontFace` class.
#[layer]
pub struct FontFaceLayer {
    pub(crate) family: String,
    pub(crate) style: String,
    pub(crate) weight: String,
    pub(crate) stretch: String,
    unicode_range: String,
    variant: String,
    feature_settings: String,
    display: String,
    face_status: Cell<FaceStatus>,
    bytes: Vec<u8>,
    loaded_promise: Deferred,
}

#[layer(js_name = "FontFace")]
impl FontFaceLayer {
    /// `new FontFace(family, source, descriptors?)` — the bytes are kept
    /// until `FontFaceSet.add` registers them with the engine.
    #[layer(constructor)]
    fn build(
        family: String,
        source: Uint8Array,
        descriptors: Option<FontFaceDescriptors>,
        env: &Env,
        sup: Super<RootLayer>,
    ) -> Result<Constructed<Self>> {
        if family.is_empty() {
            return Err(Error::new(
                Status::InvalidArg,
                "FontFace: family must be a non-empty string",
            ));
        }
        let done = sup.call(FnArgs::from(()))?;
        let d = descriptors.unwrap_or_default();
        Ok(Constructed::new(
            done,
            Self {
                family,
                style: d.style.unwrap_or_else(|| "normal".into()),
                weight: d.weight.unwrap_or_else(|| "normal".into()),
                stretch: d.stretch.unwrap_or_else(|| "normal".into()),
                unicode_range: d.unicode_range.unwrap_or_else(|| "U+0-10FFFF".into()),
                variant: d.variant.unwrap_or_else(|| "normal".into()),
                feature_settings: d.feature_settings.unwrap_or_else(|| "normal".into()),
                display: d.display.unwrap_or_else(|| "auto".into()),
                face_status: Cell::new(FaceStatus::Unloaded),
                bytes: source.to_vec(),
                loaded_promise: Deferred::new(env)?,
            },
        ))
    }

    #[layer(getter)]
    fn family(&self) -> String {
        self.family.clone()
    }

    #[layer(setter)]
    fn set_family(&mut self, value: String) {
        self.family = value;
    }

    #[layer(getter)]
    fn style(&self) -> String {
        self.style.clone()
    }

    #[layer(setter)]
    fn set_style(&mut self, value: String) {
        self.style = value;
    }

    #[layer(getter)]
    fn weight(&self) -> String {
        self.weight.clone()
    }

    #[layer(setter)]
    fn set_weight(&mut self, value: String) {
        self.weight = value;
    }

    #[layer(getter)]
    fn stretch(&self) -> String {
        self.stretch.clone()
    }

    #[layer(setter)]
    fn set_stretch(&mut self, value: String) {
        self.stretch = value;
    }

    #[layer(getter)]
    fn unicode_range(&self) -> String {
        self.unicode_range.clone()
    }

    #[layer(setter)]
    fn set_unicode_range(&mut self, value: String) {
        self.unicode_range = value;
    }

    #[layer(getter)]
    fn variant(&self) -> String {
        self.variant.clone()
    }

    #[layer(setter)]
    fn set_variant(&mut self, value: String) {
        self.variant = value;
    }

    #[layer(getter)]
    fn feature_settings(&self) -> String {
        self.feature_settings.clone()
    }

    #[layer(setter)]
    fn set_feature_settings(&mut self, value: String) {
        self.feature_settings = value;
    }

    #[layer(getter)]
    fn display(&self) -> String {
        self.display.clone()
    }

    #[layer(setter)]
    fn set_display(&mut self, value: String) {
        self.display = value;
    }

    /// `"unloaded" | "loading" | "loaded" | "error"`.
    #[layer(getter)]
    fn status(&self) -> String {
        self.face_status.get().as_str().into()
    }

    /// Promise resolving to this face once loaded.
    #[layer(getter)]
    fn loaded(&self) -> Anything {
        self.loaded_promise.value()
    }

    /// Trigger loading. Buffer-backed faces complete synchronously, so the
    /// returned promise is already resolved.
    #[layer]
    fn load(&self, env: &Env, this: &Object) -> Result<Anything> {
        if self.face_status.get() != FaceStatus::Loaded {
            self.face_status.set(FaceStatus::Loaded);
            self.loaded_promise.resolve(env, JsValue::raw(this))?;
        }
        Ok(self.loaded_promise.value())
    }
}

impl FontFaceLayer {
    /// Bytes stashed for the `FontFaceSet.add` registration path.
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Force the loaded state and resolve `loaded` (used by `add`).
    pub(crate) fn mark_loaded(&self, env: &Env, face: sys::napi_value) -> Result<()> {
        if self.face_status.get() != FaceStatus::Loaded {
            self.face_status.set(FaceStatus::Loaded);
            self.loaded_promise.resolve(env, face)?;
        }
        Ok(())
    }

    /// Force an error state and reject `loaded` with `err`.
    pub(crate) fn mark_error(&self, env: &Env, err: Error) -> Result<()> {
        self.face_status.set(FaceStatus::Error);
        self.loaded_promise.reject(env, err)
    }
}
