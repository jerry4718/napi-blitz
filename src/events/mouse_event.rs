//! `MouseEvent` layer — parent of Wheel/Pointer.

use crate::events::{
    base::{DispatchTarget, EventInit, EventTargetLayer},
    ui_event::UIEventLayer,
};
use blitz::traits::events::{BlitzPointerEvent, MouseEventButton};
use napi::{Env, Result, bindgen_prelude::FnArgs};
use napi_helpers::inherits::{Constructed, LayerRef, Super, proc::layer};

/// Own block of the `MouseEvent` class.
#[layer]
pub struct MouseEventLayer {
    #[layer(getter)]
    pub screen_x: i32,
    #[layer(getter)]
    pub screen_y: i32,
    #[layer(getter)]
    pub client_x: i32,
    #[layer(getter)]
    pub client_y: i32,
    #[layer(getter)]
    pub ctrl_key: bool,
    #[layer(getter)]
    pub shift_key: bool,
    #[layer(getter)]
    pub alt_key: bool,
    #[layer(getter)]
    pub meta_key: bool,
    #[layer(getter)]
    pub button: i16,
    #[layer(getter)]
    pub buttons: u16,
    pub related_target: DispatchTarget,
}

impl Default for MouseEventLayer {
    fn default() -> Self {
        Self {
            screen_x: 0,
            screen_y: 0,
            client_x: 0,
            client_y: 0,
            ctrl_key: false,
            shift_key: false,
            alt_key: false,
            meta_key: false,
            button: 0,
            buttons: 0,
            related_target: DispatchTarget::None,
        }
    }
}

impl MouseEventLayer {
    pub(crate) fn from_blitz(e: &BlitzPointerEvent) -> Self {
        Self {
            screen_x: e.coords.screen_x as i32,
            screen_y: e.coords.screen_y as i32,
            client_x: e.coords.client_x as i32,
            client_y: e.coords.client_y as i32,
            ctrl_key: e.mods.ctrl(),
            shift_key: e.mods.shift(),
            alt_key: e.mods.alt(),
            meta_key: e.mods.meta(),
            button: match e.button {
                MouseEventButton::Main => 0,
                MouseEventButton::Auxiliary => 1,
                MouseEventButton::Secondary => 2,
                MouseEventButton::Fourth => 3,
                MouseEventButton::Fifth => 4,
            },
            buttons: e.buttons.bits() as u16,
            related_target: DispatchTarget::None,
        }
    }

    /// Static coords + modifiers from a wheel event (Mouse fields of `Wheel`).
    pub(crate) fn from_blitz_wheel(e: &blitz::traits::events::BlitzWheelEvent) -> Self {
        Self {
            screen_x: e.coords.screen_x as i32,
            screen_y: e.coords.screen_y as i32,
            client_x: e.coords.client_x as i32,
            client_y: e.coords.client_y as i32,
            ctrl_key: e.mods.ctrl(),
            shift_key: e.mods.shift(),
            alt_key: e.mods.alt(),
            meta_key: e.mods.meta(),
            button: 0,
            buttons: e.buttons.bits() as u16,
            related_target: DispatchTarget::None,
        }
    }
}

#[layer(js_name = "MouseEvent")]
impl MouseEventLayer {
    #[layer(parent)]
    type Parent = UIEventLayer;

    #[layer(constructor)]
    fn build(
        type_: String,
        init: Option<EventInit>,
        sup: Super<UIEventLayer>,
    ) -> Result<Constructed<Self>> {
        let done = sup.call(FnArgs::from((type_, init)))?;
        Ok(Constructed::new(done, Self::default()))
    }

    #[layer(getter)]
    fn related_target(&self, env: &Env) -> Result<Option<LayerRef<EventTargetLayer>>> {
        self.related_target.resolve(env)
    }
}
