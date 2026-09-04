//! `CompositionEvent` layer — extends `UiEvent` (IME composition).

use napi::{Result, bindgen_prelude::FnArgs};
use napi_helpers::inherit as napi_inherit;
use napi_helpers::inherit::layer::{Constructed, Super};
use napi_helpers::inherit::proc::layer;
use wintertc_events::event::{EventInit, EventLayer};

use crate::events::ui_event::UiEventLayer;

/// Own block of the `CompositionEvent` class.
#[layer(js_name = "CompositionEvent", parent = UiEventLayer)]
pub struct CompositionEventLayer {
    pub(crate) data: String,
}

#[layer]
impl CompositionEventLayer {
    #[layer(constructor)]
    fn build(
        type_: String,
        init: Option<EventInit>,
        sup: Super<UiEventLayer>,
    ) -> Result<Constructed<Self>> {
        let done = sup.call(FnArgs::from((type_, init)))?;
        Ok(Constructed::new(
            done,
            Self {
                data: String::new(),
            },
        ))
    }

    #[layer(getter)]
    fn data(&self) -> String {
        self.data.clone()
    }
}
