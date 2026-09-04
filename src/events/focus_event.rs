//! `FocusEvent` layer — extends `UiEvent`.

use crate::events::{
    base::{DispatchTarget, EventInit},
    ui_event::UIEventLayer,
};
use napi::{Env, Result, bindgen_prelude::FnArgs};
use napi_helpers::{
    anything::Anything,
    inherits::{Constructed, Super, proc::layer},
};

/// Own block of the `FocusEvent` class.
#[layer]
pub struct FocusEventLayer {
    pub related_target: DispatchTarget,
}

#[layer(js_name = "FocusEvent")]
impl FocusEventLayer {
    #[layer(parent)]
    type Parent = UIEventLayer;

    #[layer(constructor)]
    fn build(
        type_: String,
        init: Option<EventInit>,
        sup: Super<UIEventLayer>,
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
