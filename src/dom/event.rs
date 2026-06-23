//! Event bridge between blitz and JS.
//!
//! `JsEventHandler` plugs into blitz's `EventDriver`. Whenever blitz produces
//! a `DomEvent`, we call the document's `__dispatchFromNative(payload)` JS
//! function (saved as a `FunctionRef` on the document). JS runs capture +
//! bubble using its own `EventTarget`s and returns a `DispatchResult` that we
//! translate back into the blitz `EventState`.

use std::{cell::RefCell, collections::HashSet};

use blitz::{
    dom::{Document as BlitzDocument, NodeData},
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

fn debug_event_kind(event: &DomEvent) -> &'static str {
    event.name()
}

fn should_log_event(event: &DomEvent) -> bool {
    matches!(event.data, DomEventData::PointerDown(_) | DomEventData::PointerUp(_))
}

fn describe_node(doc: &dyn BlitzDocument, node_id: usize) -> String {
    let inner = doc.inner();
    let Some(node) = inner.get_node(node_id) else {
        return format!("{}:<missing>", node_id);
    };
    let parent = node.parent.map_or("-".to_string(), |id| id.to_string());
    let layout_parent = node.layout_parent.get().map_or("-".to_string(), |id| id.to_string());
    match &node.data {
        NodeData::Document => format!("{}:Document(parent={},layout_parent={})", node_id, parent, layout_parent),
        NodeData::Text(text) => {
            let snippet: String = text.content.chars().take(24).collect();
            format!(
                "{}:Text(\"{}\",parent={},layout_parent={})",
                node_id,
                snippet.replace('\n', "\\n"),
                parent,
                layout_parent
            )
        }
        NodeData::Comment => format!("{}:Comment(parent={},layout_parent={})", node_id, parent, layout_parent),
        NodeData::Element(el) | NodeData::AnonymousBlock(el) => {
            let id_attr = el
                .attrs()
                .iter()
                .find(|attr| attr.name.local.as_ref() == "id")
                .map(|attr| attr.value.as_ref())
                .unwrap_or("");
            let class_attr = el
                .attrs()
                .iter()
                .find(|attr| attr.name.local.as_ref() == "class")
                .map(|attr| attr.value.as_ref())
                .unwrap_or("");
            let kind = match &node.data {
                NodeData::AnonymousBlock(_) => "AnonymousBlock",
                _ => "Element",
            };
            format!(
                "{}:{}<{} id=\"{}\" class=\"{}\">(parent={},layout_parent={})",
                node_id,
                kind,
                el.name.local,
                id_attr,
                class_attr,
                parent,
                layout_parent
            )
        }
    }
}

fn describe_chain(doc: &mut dyn BlitzDocument, chain: &[usize]) -> Vec<String> {
    chain.iter().map(|&id| describe_node(doc, id)).collect()
}

/// Trait object stored on the document, holds the JS callback that dispatches
/// events into the JS-side document instance.
///
/// `listened_nodes` lives in a separate `RefCell<HashSet<usize>>` on
/// `SharedBridge` so that `DocHandle` methods (which need `borrow_mut` on
/// the listened set) and `WindowDocument::handle_ui_event` (which holds a
/// `RefMut<JsBridge>` across the JS callback) never collide on the same
/// `RefCell`.
pub struct JsBridge {
    pub env: Env,
    pub callback: FunctionRef<EventPayload, DispatchResult>,
}

impl JsBridge {
    pub fn new(env: Env, callback: FunctionRef<EventPayload, DispatchResult>) -> Self {
        Self { env, callback }
    }
}

/// `EventHandler` impl that funnels every event through the JS bridge.
pub struct JsEventHandler<'a> {
    pub bridge: &'a mut JsBridge,
    /// The listened-nodes set, in a *separate* `RefCell` from `JsBridge`.
    /// We borrow it transiently for `chain_is_observed` and release before
    /// calling into JS, so JS event handlers that mutate the set (via
    /// `DocHandle::add_listened_node`) don't collide.
    pub listened: &'a RefCell<HashSet<usize>>,
}

/// Returns true if any node along the chain has a live JS wrapper that
/// might be listening for this event. We err on the side of dispatching
/// when in doubt.
fn chain_is_observed(listened: &HashSet<usize>, chain: &[usize]) -> bool {
    if listened.is_empty() {
        // Until JS has registered any listeners we still dispatch, because
        // the document itself may want to observe events.
        return true;
    }
    chain.iter().any(|id| listened.contains(id))
}

fn normalize_event_target(doc: &dyn BlitzDocument, target: usize) -> usize {
    let inner = doc.inner();
    let mut node_id = target;

    while let Some(node) = inner.get_node(node_id) {
        match node.data {
            NodeData::AnonymousBlock(_) => {
                let Some(parent_id) = node.parent else {
                    return target;
                };
                node_id = parent_id;
            }
            NodeData::Element(_) => return node_id,
            _ => return target,
        }
    }

    target
}

impl<'a> blitz::dom::EventHandler for JsEventHandler<'a> {
    fn handle_event(
        &mut self,
        chain: &[usize],
        event: &mut DomEvent,
        _doc: &mut dyn BlitzDocument,
        event_state: &mut EventState,
    ) {
        // Borrow listened_nodes only for the check, then release the
        // borrow so JS callbacks (which may call DocHandle methods that
        // borrow_mut the same RefCell) don't panic.
        {
            let nodes = self.listened.borrow();
            let observed = chain_is_observed(&nodes, chain);
            if should_log_event(event) {
                let chain_desc = describe_chain(_doc, chain);
                eprintln!(
                    "napi-blitz[event]: kind={} target={} chain={:?} chain_desc={:?} listened_len={} observed={}",
                    debug_event_kind(event),
                    event.target,
                    chain,
                    chain_desc,
                    nodes.len(),
                    observed,
                );
            }
            if !observed {
                if should_log_event(event) {
                    eprintln!(
                        "napi-blitz[event]: skipped_unobserved kind={} target={} chain={:?}",
                        debug_event_kind(event),
                        event.target,
                        chain,
                    );
                }
                return;
            }
        }

        let normalized_target = normalize_event_target(_doc, event.target);
        let payload = serialize_event(event, normalized_target, chain);

        let callback = match self.bridge.callback.borrow_back(&self.bridge.env) {
            Ok(cb) => cb,
            Err(err) => {
                eprintln!("napi-blitz: failed to borrow dispatch callback: {err}");
                return;
            }
        };

        println!("napi-blitz: success to borrow dispatch callback");

        let result: DispatchResult = match call_dispatch(callback, payload) {
            Ok(r) => r,
            Err(err) => {
                eprintln!("napi-blitz: dispatch callback failed: {err}");
                return;
            }
        };

        println!("napi-blitz: dispatch callback success");

        if should_log_event(event) {
            eprintln!(
                "napi-blitz[event]: dispatched kind={} target={} default_prevented={} propagation_stopped={} request_redraw={}",
                debug_event_kind(event),
                normalized_target,
                result.default_prevented,
                result.propagation_stopped,
                result.request_redraw,
            );
        }

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
    println!("napi-blitz: callback call start");
    let x = callback.call(payload);
    println!("napi-blitz: callback call end");
    x
}

/// Translate a blitz `DomEvent` into the napi-friendly `EventPayload`.
pub fn serialize_event(event: &DomEvent, target: usize, chain: &[usize]) -> EventPayload {
    EventPayload {
        event_type: event.name().to_string(),
        target: BigInt::from(target as u64),
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
