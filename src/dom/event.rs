//! Event bridge between blitz and JS.
//!
//! `JsEventHandler` plugs into blitz's `EventDriver`. Whenever blitz produces
//! a `DomEvent`, we call the document's `__dispatchFromNative(payload)` JS
//! function (saved as a `FunctionRef` on the document). JS runs capture +
//! bubble using its own `EventTarget`s and returns a `DispatchResult` that we
//! translate back into the blitz `EventState`.

use std::collections::HashSet;

use blitz::{
    dom::Document as BlitzDocument,
    traits::events::{
        BlitzImeEvent, BlitzPointerEvent, BlitzPointerId, BlitzWheelDelta, DomEvent, DomEventData,
        DomEventKind, EventState, KeyState,
    },
};
use napi::{
    Env,
    bindgen_prelude::{BigInt, Function, FunctionRef},
};

use crate::dom::payload::{
    DispatchResult, EventPayload, ImeData, InputData, KeyData, PointerData, WheelData,
};

/// Trait object stored on the document, holds the JS callback that dispatches
/// events into the JS-side document instance.
pub struct JsBridge {
    pub env: Env,
    pub callback: FunctionRef<EventPayload, DispatchResult>,
    /// node ids that JS currently has live wrappers for. We use this to skip
    /// dispatching events to nodes JS no longer cares about (the JS side
    /// updates this set via `DocHandle::set_listened_nodes`).
    pub listened_nodes: HashSet<usize>,
}

impl JsBridge {
    pub fn new(env: Env, callback: FunctionRef<EventPayload, DispatchResult>) -> Self {
        Self {
            env,
            callback,
            listened_nodes: HashSet::new(),
        }
    }

    /// Returns true if any node along the chain has a live JS wrapper that
    /// might be listening for this event. We err on the side of dispatching
    /// when in doubt.
    fn chain_is_observed(&self, chain: &[usize]) -> bool {
        if self.listened_nodes.is_empty() {
            // Until JS has registered any listeners we still dispatch, because
            // the document itself may want to observe events.
            return true;
        }
        chain.iter().any(|id| self.listened_nodes.contains(id))
    }
}

/// `EventHandler` impl that funnels every event through the JS bridge.
pub struct JsEventHandler<'a> {
    pub bridge: &'a mut JsBridge,
}

impl<'a> blitz::dom::EventHandler for JsEventHandler<'a> {
    fn handle_event(
        &mut self,
        chain: &[usize],
        event: &mut DomEvent,
        _doc: &mut dyn BlitzDocument,
        event_state: &mut EventState,
    ) {
        if !self.bridge.chain_is_observed(chain) {
            return;
        }

        let payload = serialize_event(event, chain);

        let callback = match self.bridge.callback.borrow_back(&self.bridge.env) {
            Ok(cb) => cb,
            Err(err) => {
                eprintln!("napi-blitz: failed to borrow dispatch callback: {err}");
                return;
            }
        };

        let result: DispatchResult = match call_dispatch(callback, payload) {
            Ok(r) => r,
            Err(err) => {
                eprintln!("napi-blitz: dispatch callback failed: {err}");
                return;
            }
        };

        if result.default_prevented {
            event_state.prevent_default();
        }
        if result.propagation_stopped {
            event_state.stop_propagation();
        }
        if result.request_redraw {
            event_state.request_redraw();
        }
    }
}

fn call_dispatch(
    callback: Function<EventPayload, DispatchResult>,
    payload: EventPayload,
) -> napi::Result<DispatchResult> {
    callback.call(payload)
}

/// Translate a blitz `DomEvent` into the napi-friendly `EventPayload`.
pub fn serialize_event(event: &DomEvent, chain: &[usize]) -> EventPayload {
    EventPayload {
        event_type: event.name().to_string(),
        target: BigInt::from(event.target as u64),
        chain: chain.iter().map(|id| BigInt::from(*id as u64)).collect(),
        bubbles: event.bubbles,
        cancelable: event.cancelable,
        pointer: pointer_from(&event.data),
        wheel: wheel_from(&event.data),
        key: key_from(&event.data),
        input: input_from(&event.data),
        ime: ime_from(&event.data),
    }
}

fn pointer_from(data: &DomEventData) -> Option<PointerData> {
    match data {
        DomEventData::PointerMove(p)
        | DomEventData::PointerDown(p)
        | DomEventData::PointerUp(p)
        | DomEventData::PointerEnter(p)
        | DomEventData::PointerLeave(p)
        | DomEventData::PointerOver(p)
        | DomEventData::PointerOut(p)
        | DomEventData::MouseMove(p)
        | DomEventData::MouseDown(p)
        | DomEventData::MouseUp(p)
        | DomEventData::MouseEnter(p)
        | DomEventData::MouseLeave(p)
        | DomEventData::MouseOver(p)
        | DomEventData::MouseOut(p)
        | DomEventData::Click(p)
        | DomEventData::ContextMenu(p)
        | DomEventData::DoubleClick(p) => Some(serialize_pointer(p)),
        _ => None,
    }
}

fn serialize_pointer(p: &BlitzPointerEvent) -> PointerData {
    let (kind, pointer_id) = match p.id {
        BlitzPointerId::Mouse => ("mouse", 1.0),
        BlitzPointerId::Pen => ("pen", 1.0),
        BlitzPointerId::Finger(id) => ("finger", id as f64),
    };
    PointerData {
        kind: kind.to_string(),
        pointer_id,
        is_primary: p.is_primary,
        page_x: p.coords.page_x as f64,
        page_y: p.coords.page_y as f64,
        client_x: p.coords.client_x as f64,
        client_y: p.coords.client_y as f64,
        screen_x: p.coords.screen_x as f64,
        screen_y: p.coords.screen_y as f64,
        button: p.button as i32,
        buttons: p.buttons.bits() as u32,
        pressure: p.details.pressure,
        tilt_x: p.details.tilt_x as i32,
        tilt_y: p.details.tilt_y as i32,
        twist: p.details.twist as u32,
        mods_bits: p.mods.bits(),
    }
}

fn wheel_from(data: &DomEventData) -> Option<WheelData> {
    let DomEventData::Wheel(w) = data else {
        return None;
    };
    let (mode, dx, dy) = match w.delta {
        BlitzWheelDelta::Lines(x, y) => ("lines", x, y),
        BlitzWheelDelta::Pixels(x, y) => ("pixels", x, y),
    };
    Some(WheelData {
        mode: mode.to_string(),
        delta_x: dx,
        delta_y: dy,
        page_x: w.coords.page_x as f64,
        page_y: w.coords.page_y as f64,
        client_x: w.coords.client_x as f64,
        client_y: w.coords.client_y as f64,
        buttons: w.buttons.bits() as u32,
        mods_bits: w.mods.bits(),
    })
}

fn key_from(data: &DomEventData) -> Option<KeyData> {
    let (k, kind) = match data {
        DomEventData::KeyDown(k) => (k, DomEventKind::KeyDown),
        DomEventData::KeyUp(k) => (k, DomEventKind::KeyUp),
        DomEventData::KeyPress(k) => (k, DomEventKind::KeyPress),
        _ => return None,
    };
    let _ = kind;
    Some(KeyData {
        key: k.key.to_string(),
        code: k.code.to_string(),
        location: k.location as u32,
        mods_bits: k.modifiers.bits(),
        repeat: k.is_auto_repeating,
        is_composing: k.is_composing,
        state: match k.state {
            KeyState::Pressed => "pressed".to_string(),
            KeyState::Released => "released".to_string(),
        },
        text: k.text.as_ref().map(|s| s.to_string()),
    })
}

fn input_from(data: &DomEventData) -> Option<InputData> {
    let DomEventData::Input(i) = data else {
        return None;
    };
    Some(InputData {
        value: i.value.clone(),
    })
}

fn ime_from(data: &DomEventData) -> Option<ImeData> {
    let DomEventData::Ime(ime) = data else {
        return None;
    };
    Some(match ime {
        BlitzImeEvent::Enabled => ImeData {
            kind: "enabled".to_string(),
            text: None,
            cursor_start: None,
            cursor_end: None,
            before_bytes: None,
            after_bytes: None,
        },
        BlitzImeEvent::Disabled => ImeData {
            kind: "disabled".to_string(),
            text: None,
            cursor_start: None,
            cursor_end: None,
            before_bytes: None,
            after_bytes: None,
        },
        BlitzImeEvent::Preedit(s, range) => ImeData {
            kind: "preedit".to_string(),
            text: Some(s.clone()),
            cursor_start: range.map(|(a, _)| a as u32),
            cursor_end: range.map(|(_, b)| b as u32),
            before_bytes: None,
            after_bytes: None,
        },
        BlitzImeEvent::Commit(s) => ImeData {
            kind: "commit".to_string(),
            text: Some(s.clone()),
            cursor_start: None,
            cursor_end: None,
            before_bytes: None,
            after_bytes: None,
        },
        BlitzImeEvent::DeleteSurrounding {
            before_bytes,
            after_bytes,
        } => ImeData {
            kind: "deleteSurrounding".to_string(),
            text: None,
            cursor_start: None,
            cursor_end: None,
            before_bytes: Some(*before_bytes as u32),
            after_bytes: Some(*after_bytes as u32),
        },
    })
}
