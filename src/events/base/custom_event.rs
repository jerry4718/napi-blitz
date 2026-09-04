//! The `CustomEvent` class — inherits from `Event`.
//!
//! Only registers its own `detail` accessor (read from the own block).
//! Event properties are inherited via the prototype chain.

use napi::Result;
use napi::bindgen_prelude::FnArgs;
use napi_derive::napi;
use napi_helpers::any_value::AnyValue;
use napi_inherit::layer::{Constructed, Super};
use napi_inherit_proc::layer;

use crate::event::EventInit;

/// `dictionary CustomEventInit : EventInit { any detail = null; }`
#[napi(object)]
#[derive(Default)]
pub struct CustomEventInit {
    pub detail: Option<AnyValue>,
    pub bubbles: Option<bool>,
    pub cancelable: Option<bool>,
    pub composed: Option<bool>,
}

/// Own block of the `CustomEvent` class.
#[layer(js_name = "CustomEvent", parent = super::EventLayer)]
pub struct CustomEventLayer {
    detail: AnyValue,
}

#[layer]
impl CustomEventLayer {
    /// `new CustomEvent(type, init?)` — `init` follows
    /// `dictionary CustomEventInit`.
    #[layer(constructor)]
    fn build(
        type_: String,
        init: Option<CustomEventInit>,
        sup: Super<crate::event::EventLayer>,
    ) -> Result<Constructed<Self>> {
        let CustomEventInit {
            detail,
            bubbles,
            cancelable,
            composed,
        } = init.unwrap_or_default();
        let event_init = Some(EventInit {
            bubbles,
            cancelable,
            composed,
        });
        let done = sup.call(FnArgs::from((type_, event_init)))?;
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
