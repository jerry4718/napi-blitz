//! Event layers for the DOM hierarchy, mirroring the boa-gui-runtime
//! design: each event class holds its own scalar fields, constructed
//! directly from the blitz event payload (no JS-factory/payload shim).

pub mod base;

mod composition_event;
mod focus_event;
mod input_event;
mod keyboard_event;
mod mouse_event;
mod pointer_event;
mod ui_event;
mod wheel_event;

pub(crate) use composition_event::CompositionEventLayer;
pub(crate) use focus_event::FocusEventLayer;
pub(crate) use input_event::InputEventLayer;
pub(crate) use keyboard_event::KeyboardEventLayer;
pub(crate) use mouse_event::MouseEventLayer;
pub(crate) use pointer_event::PointerEventLayer;
pub(crate) use ui_event::UiEventLayer;
pub(crate) use wheel_event::WheelEventLayer;

use blitz::traits::events::DomEventData;
use napi::{Env, Result, bindgen_prelude::Object};
use napi_helpers::inherits::{from_chain, layer_chain};
use wintertc_events::event::{DispatchTarget, EventLayer};

/// Build the most specific `Event` layer chain for a blitz payload and
/// materialize the JS instance from Rust data (no `new Event(...)` path).
pub(crate) fn build_event<'env>(
    env: &'env Env,
    event_type: &str,
    data: &DomEventData,
    bubbles: bool,
    cancelable: bool,
) -> Result<Object<'env>> {
    let event = EventLayer::with_init(event_type, bubbles, cancelable, false);

    let base_chain = layer_chain!(event, UiEventLayer::default());

    match data {
        DomEventData::PointerMove(e)
        | DomEventData::PointerDown(e)
        | DomEventData::PointerUp(e)
        | DomEventData::PointerOver(e)
        | DomEventData::PointerOut(e)
        | DomEventData::PointerEnter(e)
        | DomEventData::PointerLeave(e)
        | DomEventData::Click(e)
        | DomEventData::ContextMenu(e)
        | DomEventData::DoubleClick(e) => {
            from_chain!(
                (PointerEventLayer, env)..base_chain,
                MouseEventLayer::from_blitz(e),
                PointerEventLayer::from_blitz(e),
            )
        }
        DomEventData::MouseMove(e)
        | DomEventData::MouseDown(e)
        | DomEventData::MouseUp(e)
        | DomEventData::MouseOver(e)
        | DomEventData::MouseOut(e)
        | DomEventData::MouseEnter(e)
        | DomEventData::MouseLeave(e) => {
            from_chain!(
                (MouseEventLayer, env)..base_chain,
                MouseEventLayer::from_blitz(e),
            )
        }
        DomEventData::Wheel(e) => {
            from_chain!(
                (WheelEventLayer, env)..base_chain,
                MouseEventLayer::from_blitz_wheel(e),
                WheelEventLayer::from_blitz(e),
            )
        }
        DomEventData::KeyDown(e) | DomEventData::KeyUp(e) | DomEventData::KeyPress(e) => {
            from_chain!(
                (KeyboardEventLayer, env)..base_chain,
                KeyboardEventLayer::from_blitz(e),
            )
        }
        DomEventData::Input(e) => {
            from_chain!(
                (InputEventLayer, env)..base_chain,
                InputEventLayer {
                    data: e.value.clone()
                },
            )
        }
        DomEventData::Ime(e) => {
            let data = match e {
                blitz::traits::events::BlitzImeEvent::Commit(s) => s.clone(),
                blitz::traits::events::BlitzImeEvent::Preedit(s, _) => s.clone(),
                _ => String::new(),
            };
            from_chain!(
                (CompositionEventLayer, env)..base_chain,
                CompositionEventLayer { data },
            )
        }
        DomEventData::Focus(_)
        | DomEventData::Blur(_)
        | DomEventData::FocusIn(_)
        | DomEventData::FocusOut(_) => {
            from_chain!(
                (FocusEventLayer, env)..base_chain,
                FocusEventLayer {
                    related_target: DispatchTarget::None,
                },
            )
        }
        _ => from_chain!((UiEventLayer, env)..base_chain),
    }
}
