//! The `FontFaceSet` layer — the collection backing `document.fonts`.
//!
//! Adding a `FontFace` ships its bytes into the set's `FontContext` (a
//! clone shared with the document's engine context), so subsequent
//! layout/paint can shape with it. Registration is synchronous: `status`
//! is always `"loaded"` and `ready` is always already resolved.

use std::{cell::RefCell, ptr, sync::Arc};

use napi::{
    Env, Error, JsValue, Result, Status,
    bindgen_prelude::{Object, ToNapiValue},
    check_status, sys,
};
use napi_helpers::{
    Deferred,
    anything::{Anything, OtherRef},
    inherits::{Constructed, Super, layer_chain, new_from_chain, proc::layer, with_own},
};
use parley::{
    FontContext,
    fontique::{Blob, FontInfoOverride, FontStyle, FontWeight, FontWidth},
};

use super::font_face::FontFaceLayer;
use crate::events::base::EventTargetLayer;

fn parse_descriptor<T>(
    label: &str,
    raw: Option<&str>,
    parse: impl FnOnce(&str) -> Option<T>,
) -> Result<Option<T>> {
    let Some(s) = raw else {
        return Ok(None);
    };
    parse(s).map(Some).ok_or_else(|| {
        Error::new(
            Status::InvalidArg,
            format!("registerFont: invalid CSS `{label}` descriptor: {s:?}"),
        )
    })
}

/// Own block of the `FontFaceSet` class.
#[layer]
pub struct FontFaceSetLayer {
    pub(crate) font_ctx: FontContext,
    faces: RefCell<Vec<OtherRef>>,
    ready_promise: Deferred,
}

#[layer(js_name = "FontFaceSet")]
impl FontFaceSetLayer {
    #[layer(parent)]
    type Parent = EventTargetLayer;

    #[layer(constructor)]
    fn build(_sup: Super<EventTargetLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "FontFaceSet cannot be constructed directly; use document.fonts",
        ))
    }

    /// Always `"loaded"`: registration is synchronous.
    #[layer(getter)]
    fn status(&self) -> String {
        "loaded".into()
    }

    /// Already-resolved promise of this set.
    #[layer(getter)]
    fn ready(&self) -> Anything {
        self.ready_promise.value()
    }

    /// Number of faces currently in the set.
    #[layer(getter)]
    fn size(&self) -> u32 {
        self.faces.borrow().len() as u32
    }

    /// Add `face` to the set, registering its bytes with the underlying
    /// font cache. Returns the set (per spec).
    #[layer]
    fn add(&mut self, env: &Env, this: &Object, face: Object) -> Result<Anything> {
        let set_value = || -> Result<Anything> {
            Ok(Anything::Object(unsafe {
                OtherRef::new(env.raw(), JsValue::raw(this))?
            }))
        };
        if with_own::<FontFaceLayer, _>(&face, |_| ()).is_err() {
            return Err(Error::new(
                Status::InvalidArg,
                "FontFaceSet.add: argument must be a FontFace",
            ));
        }
        let face_raw = JsValue::raw(&face);
        if self.has_face(env, face_raw)? {
            return set_value();
        }

        let (bytes, family, weight, style, stretch) = with_own::<FontFaceLayer, _>(&face, |f| {
            (
                f.bytes().to_vec(),
                f.family.clone(),
                f.weight.clone(),
                f.style.clone(),
                f.stretch.clone(),
            )
        })?;
        if bytes.is_empty() {
            return Err(Error::new(
                Status::InvalidArg,
                "FontFaceSet.add: face has no source data to register",
            ));
        }

        if let Err(err) = self.register_font(&bytes, &family, &weight, &style, &stretch) {
            let reason = err.reason.clone();
            with_own::<FontFaceLayer, _>(&face, |f| f.mark_error(env, err))?;
            return Err(Error::new(Status::GenericFailure, reason));
        }
        with_own::<FontFaceLayer, _>(&face, |f| f.mark_loaded(env, face_raw))?;

        self.faces
            .borrow_mut()
            .push(unsafe { OtherRef::new(env.raw(), face_raw)? });
        set_value()
    }

    /// Remove `face` from the set. The engine-side registration has no
    /// unregister path, so the bytes stay resolvable inside the engine;
    /// iteration stops yielding the face.
    #[layer]
    fn delete(&mut self, env: &Env, face: Object) -> Result<bool> {
        let raw = JsValue::raw(&face);
        let mut faces = self.faces.borrow_mut();
        let Some(index) = faces.iter().position(|stored| {
            unsafe { stored.raw_value(env) }
                .ok()
                .and_then(|value| same_js_value(env, raw, value).ok())
                .unwrap_or(false)
        }) else {
            return Ok(false);
        };
        faces.remove(index);
        Ok(true)
    }

    /// Whether `face` is currently in the set.
    #[layer]
    fn has(&self, env: &Env, face: Object) -> Result<bool> {
        self.has_face(env, JsValue::raw(&face))
    }

    /// Drop every face (same engine-side caveat as `delete`).
    #[layer]
    fn clear(&self) {
        self.faces.borrow_mut().clear();
    }

    /// Iterate over registered faces in insertion order.
    #[layer]
    fn for_each(
        &self,
        env: &Env,
        this: &Object,
        callback: Anything,
        this_arg: Option<Anything>,
    ) -> Result<()> {
        let Anything::Function(callback) = callback else {
            return Err(Error::new(
                Status::FunctionExpected,
                "FontFaceSet.forEach: callback must be a Function",
            ));
        };
        let callback_raw = unsafe { callback.raw_value(env)? };
        let recv_raw = unsafe {
            ToNapiValue::to_napi_value(env.raw(), this_arg.unwrap_or(Anything::Undefined))?
        };
        let set_raw = JsValue::raw(this);
        // Snapshot: a callback may mutate the set during iteration.
        let snapshot = self.faces.borrow().clone();
        for stored in &snapshot {
            let face_raw = unsafe { stored.raw_value(env)? };
            let argv = [face_raw, face_raw, set_raw];
            check_status!(unsafe {
                sys::napi_call_function(
                    env.raw(),
                    recv_raw,
                    callback_raw,
                    3,
                    argv.as_ptr(),
                    ptr::null_mut(),
                )
            })?;
        }
        Ok(())
    }

    #[layer(generator)]
    fn values(&self, index: u32) -> Option<Anything> {
        self.face_values().into_iter().nth(index as usize)
    }

    #[layer]
    fn keys(&self) -> Vec<Anything> {
        self.face_values()
    }

    #[layer]
    fn entries(&self) -> Vec<Vec<Anything>> {
        self.face_values()
            .into_iter()
            .map(|face| vec![face.clone(), face])
            .collect()
    }

    /// Not implemented: needs a CSS font-shorthand parser plus shaping
    /// queries. Throwing keeps the gap obvious.
    #[layer]
    fn load(&self, _font: String, _text: Option<String>) -> Result<()> {
        Err(Error::from_reason(
            "FontFaceSet.load(font, text?) is not yet implemented",
        ))
    }

    /// Same gap as `load`.
    #[layer]
    fn check(&self, _font: String, _text: Option<String>) -> Result<()> {
        Err(Error::from_reason(
            "FontFaceSet.check(font, text?) is not yet implemented",
        ))
    }
}

impl FontFaceSetLayer {
    /// Build the set for a document (`document.fonts`), its `ready`
    /// promise already resolved to the instance.
    pub(crate) fn init<'env>(env: &'env Env, font_ctx: FontContext) -> Result<Object<'env>> {
        let set = new_from_chain::<FontFaceSetLayer>(
            env,
            layer_chain!(
                EventTargetLayer::fresh(),
                FontFaceSetLayer {
                    font_ctx,
                    faces: RefCell::new(Vec::new()),
                    ready_promise: Deferred::new(env)?,
                },
            ),
        )?;
        let raw = JsValue::raw(&set);
        with_own::<FontFaceSetLayer, _>(&set, |d| d.ready_promise.resolve(env, raw))?;
        Ok(set)
    }

    /// Register font bytes under the face's descriptors; returns the
    /// number of faces the blob contained (the old `registerFont` path).
    fn register_font(
        &mut self,
        bytes: &[u8],
        family: &str,
        weight: &str,
        style: &str,
        stretch: &str,
    ) -> Result<u32> {
        let blob = Blob::new(Arc::new(bytes.to_vec()) as _);
        let weight = parse_descriptor("weight", non_normal(weight), FontWeight::parse_css)?;
        let style = parse_descriptor("style", non_normal(style), FontStyle::parse_css)?;
        let width = parse_descriptor("stretch", non_normal(stretch), FontWidth::parse_css)?;
        let info_override = Some(FontInfoOverride {
            family_name: Some(family),
            weight,
            style,
            width,
            ..Default::default()
        });
        let registered = self.font_ctx.collection.register_fonts(blob, info_override);
        let face_count: usize = registered.iter().map(|(_, fonts)| fonts.len()).sum();
        Ok(face_count as u32)
    }

    fn has_face(&self, env: &Env, raw: sys::napi_value) -> Result<bool> {
        let faces = self.faces.borrow();
        for stored in faces.iter() {
            let value = unsafe { stored.raw_value(env)? };
            if same_js_value(env, raw, value)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn face_values(&self) -> Vec<Anything> {
        self.faces
            .borrow()
            .iter()
            .map(|r| Anything::Object(r.clone()))
            .collect()
    }
}

/// A descriptor counts as an override unless it is the `"normal"` default.
fn non_normal(value: &str) -> Option<&str> {
    (value != "normal").then_some(value)
}

/// Strict JS equality between two raw values.
fn same_js_value(env: &Env, left: sys::napi_value, right: sys::napi_value) -> Result<bool> {
    let mut equal = false;
    check_status!(unsafe { sys::napi_strict_equals(env.raw(), left, right, &mut equal) })?;
    Ok(equal)
}
