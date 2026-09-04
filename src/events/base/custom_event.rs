//! The `CustomEvent` class — inherits from `Event`.
//!
//! Only registers its own `detail` accessor (read from the own block).
//! Event properties are inherited via the prototype chain.

use napi::{
    Result, UnknownRef,
    bindgen_prelude::{FromNapiValue, JsValue, Unknown},
};
use napi_inherit_proc::layer;

/// Own block of the `CustomEvent` class.
#[layer(js_name = "CustomEvent")]
pub struct CustomEventLayer {
    detail: UnknownRef,
}

#[layer]
impl CustomEventLayer {
    /// `new CustomEvent(type, detail)`.
    #[layer(constructor)]
    fn build(detail: Option<Unknown<'static>>) -> Result<Self> {
        let env = super::dispatch::env()?;
        let detail = match detail {
            None => super::dispatch::null_unknown(&env)?,
            Some(v) => v,
        };
        let r = unsafe { UnknownRef::from_napi_value(env.raw(), JsValue::raw(&detail)) }?;
        Ok(Self { detail: r })
    }

    #[layer(getter)]
    fn detail(&self) -> Result<Unknown<'static>> {
        let env = super::dispatch::env()?;
        let v = self.detail.get_value(&env)?;
        super::dispatch::to_unknown(&env, &v)
    }
}
