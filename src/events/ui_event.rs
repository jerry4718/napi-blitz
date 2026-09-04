//! `UiEvent` layer — parent of Mouse/Wheel/Keyboard/Composition/Focus.

use napi::{Result, bindgen_prelude::FnArgs};
use napi_helpers::inherit as napi_inherit;
use napi_helpers::inherit::layer::{Constructed, Super};
use napi_helpers::inherit::proc::layer;
use wintertc_events::event::{EventInit, EventLayer};

/// Own block of the `UiEvent` class.
#[layer(js_name = "UiEvent", parent = EventLayer)]
pub struct UiEventLayer {
    #[layer(getter)]
    pub detail: i32,
}

#[layer]
impl UiEventLayer {
    #[layer(constructor)]
    fn build(
        type_: String,
        init: Option<EventInit>,
        sup: Super<EventLayer>,
    ) -> Result<Constructed<Self>> {
        let done = sup.call(FnArgs::from((type_, init)))?;
        Ok(Constructed::new(done, Self { detail: 0 }))
    }
}

/// Argument-less own block; used in chains where `detail` is 0.
impl Default for UiEventLayer {
    fn default() -> Self {
        Self { detail: 0 }
    }
}
