//! `InputEvent` layer — extends `Event` (mirrors the boa runtime).

use crate::events::{UiEventLayer, base::EventInit};
use napi::{Result, bindgen_prelude::FnArgs};
use napi_helpers::inherits::{Constructed, Super, proc::layer};

/// Own block of the `InputEvent` class.
#[layer(js_name = "InputEvent")]
pub struct InputEventLayer {
    pub(crate) data: String,
}

#[layer]
impl InputEventLayer {
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
