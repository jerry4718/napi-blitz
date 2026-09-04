//! The `MessageEvent` class — inherits from `Event`.

use napi::{Env, Result, bindgen_prelude::Unknown};
use napi_inherit_proc::layer;

use super::{EventLayer, dispatch::StoredValue};

/// Own block of the `MessageEvent` class.
#[layer(js_name = "MessageEvent", parent = EventLayer)]
pub struct MessageEventLayer {
    data: StoredValue,
    origin: String,
}

#[layer]
impl MessageEventLayer {
    /// `new MessageEvent(type, data, origin)`.
    #[layer(constructor)]
    fn build(env: &Env, data: Option<Unknown<'static>>, origin: Option<String>) -> Result<Self> {
        let data = match data {
            None => super::dispatch::null_unknown(env)?,
            Some(v) => v,
        };
        Ok(Self {
            data: StoredValue::from_value(&data)?,
            origin: origin.unwrap_or_default(),
        })
    }

    #[layer(getter)]
    fn data(&self, env: &Env) -> Result<Unknown<'static>> {
        self.data.to_value(env)
    }

    #[layer(getter)]
    fn origin(&self) -> String {
        self.origin.clone()
    }
}
