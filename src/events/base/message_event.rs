//! The `MessageEvent` class — inherits from `Event`.

use napi::{
    Result, UnknownRef,
    bindgen_prelude::{FromNapiValue, JsValue, Unknown},
};
use napi_inherit_proc::layer;

/// Own block of the `MessageEvent` class.
#[layer(js_name = "MessageEvent")]
pub struct MessageEventLayer {
    data: UnknownRef,
    origin: String,
}

#[layer]
impl MessageEventLayer {
    /// `new MessageEvent(type, data, origin)`.
    #[layer(constructor)]
    fn build(data: Option<Unknown<'static>>, origin: Option<String>) -> Result<Self> {
        let env = super::dispatch::env()?;
        let data = match data {
            None => super::dispatch::null_unknown(&env)?,
            Some(v) => v,
        };
        let r = unsafe { UnknownRef::from_napi_value(env.raw(), JsValue::raw(&data)) }?;
        Ok(Self {
            data: r,
            origin: origin.unwrap_or_default(),
        })
    }

    #[layer(getter)]
    fn data(&self) -> Result<Unknown<'static>> {
        let env = super::dispatch::env()?;
        let v = self.data.get_value(&env)?;
        super::dispatch::to_unknown(&env, &v)
    }

    #[layer(getter)]
    fn origin(&self) -> String {
        self.origin.clone()
    }
}
