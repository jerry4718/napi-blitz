//! `FocusEvent` layer — extends `UiEvent`.

use napi::{Env, Result, bindgen_prelude::FnArgs};
use napi_helpers::anything::Anything;
use napi_helpers::inherit as napi_inherit;
use napi_helpers::inherit::layer::{Constructed, Super};
use napi_helpers::inherit::proc::layer;
use wintertc_events::event::{DispatchTarget, EventInit, EventLayer};

use crate::events::ui_event::UiEventLayer;

/// Own block of the `FocusEvent` class.
#[layer(js_name = "FocusEvent", parent = UiEventLayer)]
pub struct FocusEventLayer {
    pub related_target: DispatchTarget,
}

#[layer]
impl FocusEventLayer {
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
                related_target: DispatchTarget::None,
            },
        ))
    }

    #[layer(getter)]
    fn related_target(&self, env: &Env) -> Result<Anything> {
        self.related_target.resolve(env)
    }
}
