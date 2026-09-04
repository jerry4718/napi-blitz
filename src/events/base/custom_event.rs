//! The `CustomEvent` class — inherits from `Event`.
//!
//! Only registers its own `detail` accessor (read from the own block).
//! Event properties are inherited via the prototype chain.

use napi::{Env, Result, bindgen_prelude::Unknown};
use napi_inherit_proc::layer;

use super::{dispatch::StoredValue, event::EventLayer};

/// Own block of the `CustomEvent` class.
#[layer(js_name = "CustomEvent", parent = EventLayer)]
pub struct CustomEventLayer {
    detail: StoredValue,
}

#[layer]
impl CustomEventLayer {
    /// `new CustomEvent(type, detail)`.
    #[layer(constructor)]
    fn build(env: &Env, detail: Option<Unknown<'static>>) -> Result<Self> {
        let detail = match detail {
            None => super::dispatch::null_unknown(env)?,
            Some(v) => v,
        };
        Ok(Self {
            detail: StoredValue::from_value(&detail)?,
        })
    }

    #[layer(getter)]
    fn detail(&self, env: &Env) -> Result<Unknown<'static>> {
        self.detail.to_value(env)
    }
}
