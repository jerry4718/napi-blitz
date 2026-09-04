//! The `CustomEvent` class — inherits from `Event`.
//!
//! Only registers its own `detail` accessor (read from the own block).
//! Event properties are inherited via the prototype chain.

use napi::Result;
use napi::bindgen_prelude::FnArgs;
use napi_helpers::any_value::AnyValue;
use napi_inherit::layer::{Constructed, Super};
use napi_inherit_proc::layer;

/// Own block of the `CustomEvent` class.
#[layer(js_name = "CustomEvent", parent = super::EventLayer)]
pub struct CustomEventLayer {
    detail: AnyValue,
}

#[layer]
impl CustomEventLayer {
    /// `new CustomEvent(type, detail)`.
    #[layer(constructor)]
    fn build(
        type_: String,
        detail: Option<AnyValue>,
        sup: Super<crate::event::EventLayer>,
    ) -> Result<Constructed<Self>> {
        let done = sup.call(FnArgs::from((type_,)))?;
        Ok(Constructed::new(
            done,
            Self {
                detail: detail.unwrap_or(AnyValue::Null),
            },
        ))
    }

    #[layer(getter)]
    fn detail(&self) -> AnyValue {
        self.detail.clone()
    }
}
