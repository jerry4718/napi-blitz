//! `CompositionEvent` layer — extends `UiEvent` (IME composition).

use crate::events::{base::EventInit, ui_event::UiEventLayer};
use napi::{Result, bindgen_prelude::FnArgs};
use napi_helpers::inherits::{Constructed, Super, proc::layer};

/// Own block of the `CompositionEvent` class.
#[layer(js_name = "CompositionEvent")]
pub struct CompositionEventLayer {
    pub(crate) data: String,
}

#[layer]
impl CompositionEventLayer {
    #[layer(parent)]
    type Parent = UiEventLayer;

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
