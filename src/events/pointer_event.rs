//! `PointerEvent` layer — extends `MouseEvent`.

use blitz::traits::events::BlitzPointerEvent;
use napi::{Result, bindgen_prelude::FnArgs};
use napi_helpers::inherits::{Constructed, Super, proc::layer};
use wintertc_events::event::EventInit;

use crate::events::mouse_event::MouseEventLayer;

/// Own block of the `PointerEvent` class (pointer-specific fields).
#[layer(js_name = "PointerEvent")]
pub struct PointerEventLayer {
    #[layer(getter)]
    pub pointer_id: i32,
    #[layer(getter)]
    pub width: f64,
    #[layer(getter)]
    pub height: f64,
    #[layer(getter)]
    pub pressure: f64,
    #[layer(getter)]
    pub tangential_pressure: f64,
    #[layer(getter)]
    pub tilt_x: i32,
    #[layer(getter)]
    pub tilt_y: i32,
    #[layer(getter)]
    pub twist: i32,
    pub(crate) pointer_type: String,
    #[layer(getter)]
    pub is_primary: bool,
}

impl Default for PointerEventLayer {
    fn default() -> Self {
        Self {
            pointer_id: 0,
            width: 1.0,
            height: 1.0,
            pressure: 0.0,
            tangential_pressure: 0.0,
            tilt_x: 0,
            tilt_y: 0,
            twist: 0,
            pointer_type: String::new(),
            is_primary: false,
        }
    }
}

impl PointerEventLayer {
    pub(crate) fn from_blitz(e: &BlitzPointerEvent) -> Self {
        Self {
            pointer_id: match e.id {
                blitz::traits::events::BlitzPointerId::Mouse => 1,
                blitz::traits::events::BlitzPointerId::Pen => 2,
                blitz::traits::events::BlitzPointerId::Finger(id) => id as i32,
            },
            width: 1.0,
            height: 1.0,
            pressure: e.details.pressure,
            tangential_pressure: e.details.tangential_pressure as f64,
            tilt_x: e.details.tilt_x as i32,
            tilt_y: e.details.tilt_y as i32,
            twist: e.details.twist as i32,
            pointer_type: if e.is_mouse() {
                "mouse".to_string()
            } else if e.is_pen() {
                "pen".to_string()
            } else {
                "touch".to_string()
            },
            is_primary: e.is_primary,
        }
    }
}

#[layer]
impl PointerEventLayer {
    #[layer(parent)]
    type Parent = MouseEventLayer;

    #[layer(constructor)]
    fn build(
        type_: String,
        init: Option<EventInit>,
        sup: Super<MouseEventLayer>,
    ) -> Result<Constructed<Self>> {
        let done = sup.call(FnArgs::from((type_, init)))?;
        Ok(Constructed::new(done, Self::default()))
    }

    #[layer(getter)]
    fn pointer_type(&self) -> String {
        self.pointer_type.clone()
    }
}
