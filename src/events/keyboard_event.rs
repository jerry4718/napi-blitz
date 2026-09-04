//! `KeyboardEvent` layer — extends `UiEvent`.

use blitz::traits::events::BlitzKeyEvent;
use napi::{Result, bindgen_prelude::FnArgs};
use napi_helpers::inherit as napi_inherit;
use napi_helpers::inherit::layer::{Constructed, Super};
use napi_helpers::inherit::proc::layer;
use wintertc_events::event::{EventInit, EventLayer};

use crate::events::ui_event::UiEventLayer;

/// Own block of the `KeyboardEvent` class.
#[layer(js_name = "KeyboardEvent", parent = UiEventLayer)]
pub struct KeyboardEventLayer {
    pub(crate) key: String,
    pub(crate) code: String,
    #[layer(getter)]
    pub location: u32,
    #[layer(getter)]
    pub ctrl_key: bool,
    #[layer(getter)]
    pub shift_key: bool,
    #[layer(getter)]
    pub alt_key: bool,
    #[layer(getter)]
    pub meta_key: bool,
    #[layer(getter)]
    pub repeat: bool,
    #[layer(getter)]
    pub is_composing: bool,
}

impl KeyboardEventLayer {
    pub(crate) fn from_blitz(e: &BlitzKeyEvent) -> Self {
        Self {
            key: e.key.to_string(),
            code: e.code.to_string(),
            location: e.location as u32,
            ctrl_key: e.modifiers.ctrl(),
            shift_key: e.modifiers.shift(),
            alt_key: e.modifiers.alt(),
            meta_key: e.modifiers.meta(),
            repeat: e.is_auto_repeating,
            is_composing: e.is_composing,
        }
    }
}

#[layer]
impl KeyboardEventLayer {
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
                key: String::new(),
                code: String::new(),
                location: 0,
                ctrl_key: false,
                shift_key: false,
                alt_key: false,
                meta_key: false,
                repeat: false,
                is_composing: false,
            },
        ))
    }

    #[layer(getter)]
    fn key(&self) -> String {
        self.key.clone()
    }

    #[layer(getter)]
    fn code(&self) -> String {
        self.code.clone()
    }
}
