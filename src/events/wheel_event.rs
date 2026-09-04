//! `WheelEvent` layer — extends `MouseEvent`.

use blitz::traits::events::{BlitzWheelDelta, BlitzWheelEvent};
use napi::{Result, bindgen_prelude::FnArgs};
use napi_helpers::inherits::{Constructed, Super, proc::layer};
use std::mem::ManuallyDrop;
use wintertc_events::event::EventInit;

use crate::events::mouse_event::MouseEventLayer;

/// Own block of the `WheelEvent` class.
#[layer(js_name = "WheelEvent")]
pub struct WheelEventLayer {
    #[layer(getter)]
    pub delta_x: f64,
    #[layer(getter)]
    pub delta_y: f64,
    #[layer(getter)]
    pub delta_z: f64,
    #[layer(getter)]
    pub delta_mode: u32,
}

impl WheelEventLayer {
    /// Line deltas are normalized to pixels (DOM: 1 line = 100px).
    pub(crate) fn from_blitz(e: &BlitzWheelEvent) -> Self {
        let (delta_x, delta_y) = match e.delta {
            BlitzWheelDelta::Lines(x, y) => (x * 100.0, y * 100.0),
            BlitzWheelDelta::Pixels(x, y) => (x, y),
        };
        Self {
            delta_x,
            delta_y,
            delta_z: 0.0,
            delta_mode: 0,
        }
    }
}

#[layer]
impl WheelEventLayer {
    #[layer(parent)]
    type Parent = MouseEventLayer;

    #[layer(constructor)]
    fn build(
        type_: String,
        init: Option<EventInit>,
        sup: Super<MouseEventLayer>,
    ) -> Result<Constructed<Self>> {
        let done = sup.call(FnArgs::from((type_, init)))?;
        Ok(Constructed::new(
            done,
            Self {
                delta_x: 0.0,
                delta_y: 0.0,
                delta_z: 0.0,
                delta_mode: 0,
            },
        ))
    }
}

pub struct X<T> {
    x: ManuallyDrop<T>,
}

impl X<WheelEventLayer> {
    pub fn xxx() {}
}
