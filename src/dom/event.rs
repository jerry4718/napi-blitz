//! Event dispatch: Rust drives the three-phase chain walk.
//!
//! When blitz produces a `DomEvent`, `JsEventHandler` receives the event
//! chain (root → target) and the raw event data. It:
//!
//! 1. Builds a JS `Event` object by calling the registered event factory.
//! 2. Sets `event.target` to a lazy getter (only wraps the target node
//!    when JS code actually reads `event.target`).
//! 3. Walks the chain in capture → target → bubble order. For each
//!    receiver it calls `wrap_node(node_id)` to get the JS Node object,
//!    sets `event.currentTarget` / `event.eventPhase` to lazy getters,
//!    and invokes `node.dispatchEvent(event)`.
//! 4. Reads back `event.defaultPrevented` / `event.cancelBubble` and
//!    writes them to blitz's `EventState`.
//!
//! This replaces the old `JsBridge` per-step callback model: instead of
//! serializing an `EventPayload` and calling into JS for every receiver,
//! we build the Event once and call `dispatchEvent` directly.

use crate::dom::{
    doc::{SharedDoc, wrap_node},
    global_creators as gc,
    payload::{EventPayload, ImeData, InputData, KeyData, PointerData, WheelData},
};
use blitz::{
    dom::{Document as BlitzDocument, EventHandler, NodeData, NodeId},
    traits::events::{
        BlitzImeEvent, BlitzPointerId, BlitzWheelDelta, DomEvent, DomEventData, EventState,
        KeyState,
    },
};
use napi::{
    Env, JsValue, Result,
    bindgen_prelude::{
        FnArgs, FromNapiValue, Function, JsObjectValue, Object, ToNapiValue, Unknown,
    },
};
use std::rc::{Rc, Weak};

const CAPTURING_PHASE: u32 = 1;
const AT_TARGET: u32 = 2;
const BUBBLING_PHASE: u32 = 3;

/// `EventHandler` impl that drives the three-phase dispatch from Rust.
/// Holds a `Weak<SharedDoc>` so it doesn't conflict with `&mut self`
/// in `EventDriver::new(self, handler)`.
pub struct JsEventHandler {
    pub doc: Weak<SharedDoc>,
}

/// Normalize the event target: if it's an `AnonymousBlock` (blitz internal
/// layout node), walk up to the first non-anonymous ancestor.
fn normalize_event_target(doc: &dyn BlitzDocument, target: NodeId) -> NodeId {
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

/// Check if a node is an `AnonymousBlock` (blitz internal layout node).
fn is_anonymous(doc: &dyn BlitzDocument, node_id: NodeId) -> bool {
    doc.inner()
        .get_node(node_id)
        .map(|n| matches!(n.data, NodeData::AnonymousBlock(_)))
        .unwrap_or(false)
}

impl EventHandler for JsEventHandler {
    fn handle_event(
        &mut self,
        chain: &[NodeId],
        event: &mut DomEvent,
        doc: &mut dyn BlitzDocument,
        event_state: &mut EventState,
    ) {
        let env = match gc::env() {
            Ok(e) => e,
            Err(_) => return,
        };

        // Upgrade weak ref to get the SharedDoc.
        let Some(shared_doc) = self.doc.upgrade() else {
            return;
        };

        // 1. Build the JS Event object via the registered factory.
        let event_factory_ref = match gc::get_event_factory() {
            Some(r) => r,
            None => return, // No factory registered yet.
        };

        let payload = serialize_event(event);
        let mut event_obj = match call_event_factory(event_factory_ref, payload, &env) {
            Ok(obj) => obj,
            Err(e) => {
                eprintln!("napi-blitz: event factory call failed: {e}");
                return;
            }
        };

        // 2. Normalize target (skip AnonymousBlock).
        let target_nid = normalize_event_target(doc, event.target);

        // 3. Set lazy target getter on the event.
        let _ = set_lazy_target(&event_obj, target_nid, &shared_doc, &env);

        // 4. Filter anonymous nodes from the chain.
        let clean_chain: Vec<NodeId> = chain
            .iter()
            .copied()
            .filter(|&nid| !is_anonymous(doc, nid))
            .collect();

        let mut propagation_stopped = false;

        // 5. Capture phase (root → target's parent).
        for &nid in clean_chain.iter().skip(1).rev() {
            if propagation_stopped {
                break;
            }
            propagation_stopped =
                self.dispatch_to_node(nid, &event_obj, CAPTURING_PHASE, &shared_doc, &env);
        }

        // 6. Target phase.
        if !propagation_stopped {
            propagation_stopped =
                self.dispatch_to_node(target_nid, &event_obj, AT_TARGET, &shared_doc, &env);
        }

        // 7. Bubble phase (target's parent → root).
        if event.bubbles && !propagation_stopped {
            for &nid in clean_chain.iter().skip(1) {
                if propagation_stopped {
                    break;
                }
                propagation_stopped =
                    self.dispatch_to_node(nid, &event_obj, BUBBLING_PHASE, &shared_doc, &env);
            }
        }

        // 8. Reset transient dispatch state: currentTarget → null,
        //    eventPhase → NONE (0). Per DOM spec, after dispatch ends
        //    these values are cleared so async callbacks see null.
        reset_dispatch_state(&mut event_obj, &env);

        // 9. Dispatch pointer events to the window-level EventTarget.
        //    JS code may register `pointermove`/`pointerup` listeners on
        //    the Window object (e.g. for drag tracking). The DOM chain
        //    walk above only reaches DOM nodes, so we explicitly forward
        //    here.
        if is_pointer_event(event) {
            let _ = dispatch_to_window(&event_obj, &shared_doc, &env);
        }

        // 10. Read back flags and write to blitz EventState.
        let default_prevented: bool = event_obj
            .get_named_property("defaultPrevented")
            .unwrap_or(false);
        if default_prevented {
            event_state.prevent_default();
        }
        if propagation_stopped {
            event_state.stop_propagation();
        }

        // 11. Sweep stale cache entries periodically.
        shared_doc.node_cache.borrow_mut().sweep(&env);
    }
}

impl JsEventHandler {
    /// Dispatch the event to a single node. Returns `true` if propagation
    /// was stopped (stopPropagation / stopImmediatePropagation).
    fn dispatch_to_node(
        &self,
        node_id: NodeId,
        event: &Object,
        phase: u32,
        doc: &Rc<SharedDoc>,
        env: &Env,
    ) -> bool {
        // 1. Wrap the node (NodeCache lookup or create).
        let node = match wrap_node(doc, node_id, env) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("napi-blitz: wrap_node failed for node {node_id}: {e}");
                return false;
            }
        };

        // 2. Set lazy currentTarget + eventPhase.
        let _ = set_lazy_current_target(event, node_id, phase, doc, env);

        // 3. Call node.dispatchEvent(event) and read back cancelBubble.
        call_dispatch_event(&node, event, env).unwrap_or(false)
    }
}

/// Check if the event is a pointer-type event that should also be
/// forwarded to the window-level EventTarget.
fn is_pointer_event(event: &DomEvent) -> bool {
    matches!(
        event.data,
        DomEventData::PointerMove(_)
            | DomEventData::PointerDown(_)
            | DomEventData::PointerUp(_)
            | DomEventData::PointerEnter(_)
            | DomEventData::PointerLeave(_)
            | DomEventData::PointerOver(_)
            | DomEventData::PointerOut(_)
            | DomEventData::MouseMove(_)
            | DomEventData::MouseDown(_)
            | DomEventData::MouseUp(_)
            | DomEventData::MouseEnter(_)
            | DomEventData::MouseLeave(_)
            | DomEventData::MouseOver(_)
            | DomEventData::MouseOut(_)
            | DomEventData::Click(_)
            | DomEventData::ContextMenu(_)
            | DomEventData::DoubleClick(_)
    )
}

/// Dispatch the event to the JS Window's `dispatchEvent` function.
/// This lets `window.addEventListener('pointermove', ...)` work.
fn dispatch_to_window(event: &Object, doc: &Rc<SharedDoc>, env: &Env) -> Result<()> {
    let dispatch_ref = doc.window_dispatch_ref.borrow();
    let Some(napi_ref) = *dispatch_ref else {
        return Ok(());
    };
    drop(dispatch_ref);

    unsafe {
        let mut dispatch_fn = std::ptr::null_mut();
        napi::check_status!(napi::sys::napi_get_reference_value(
            env.raw(),
            napi_ref,
            &mut dispatch_fn
        ))?;

        let event_val = JsValue::raw(event);
        let args = [event_val];
        let mut result = std::ptr::null_mut();
        napi::check_status!(napi::sys::napi_call_function(
            env.raw(),
            {
                let mut undef = std::ptr::null_mut();
                napi::sys::napi_get_undefined(env.raw(), &mut undef);
                undef
            },
            dispatch_fn,
            args.len(),
            args.as_ptr(),
            &mut result,
        ))?;
    }
    Ok(())
}

// ── NAPI call helpers (concentrated unsafe) ───────────────────────────
//
// These functions use raw `sys::napi_*` calls to invoke JS functions
// stored as `napi_ref`s. They are the unsafe counterpart to the safe
// `FunctionRef::borrow_back + call` pattern, which we cannot use because
// `Object<'env>` carries a lifetime that prevents storing
// `FunctionRef<..., Object<'env>>` in struct fields.

/// Call the JS event factory: `factory(payload) → Object`.
fn call_event_factory(
    factory_ref: napi::sys::napi_ref,
    payload: EventPayload,
    env: &Env,
) -> Result<Object<'_>> {
    unsafe {
        // Get the factory function.
        let mut factory_val = std::ptr::null_mut();
        napi::check_status!(napi::sys::napi_get_reference_value(
            env.raw(),
            factory_ref,
            &mut factory_val
        ))?;

        // Convert payload to napi_value.
        let payload_val = <EventPayload as ToNapiValue>::to_napi_value(env.raw(), payload)?;

        // Call factory(payload).
        let args = [payload_val];
        let mut result = std::ptr::null_mut();
        napi::check_status!(napi::sys::napi_call_function(
            env.raw(),
            // `this` = undefined
            {
                let mut undef = std::ptr::null_mut();
                napi::sys::napi_get_undefined(env.raw(), &mut undef);
                undef
            },
            factory_val,
            args.len(),
            args.as_ptr(),
            &mut result,
        ))?;

        Ok(Object::from_raw(env.raw(), result))
    }
}

/// Call `node.dispatchEvent(event)` and return `true` if propagation stopped.
fn call_dispatch_event(node: &Object, event: &Object, _env: &Env) -> Result<bool> {
    let dispatch_fn: Function<Object, bool> = node.get_named_property("dispatchEvent")?;
    let _ = dispatch_fn.apply(*node, *event)?;
    let cancel: bool = event.get_named_property("cancelBubble")?;
    Ok(cancel)
}

// ── Dispatch state reset ──────────────────────────────────────────────

/// Reset `currentTarget` to `null` and `eventPhase` to `0` (NONE) after
/// dispatch completes, per DOM spec.
fn reset_dispatch_state(event: &mut Object, _env: &Env) {
    let _ = event.set_named_property("currentTarget", ());
    let _ = event.set_named_property("eventPhase", 0u32);
}

// ── Lazy target/currentTarget setters ─────────────────────────────────

/// Set `event.target` to a lazy getter that calls `wrap_node` only when
/// JS code reads the property.
fn set_lazy_target(event: &Object, node_id: NodeId, doc: &Rc<SharedDoc>, env: &Env) -> Result<()> {
    let doc_clone = doc.clone();
    let getter: Function<(), Unknown> =
        env.create_function_from_closure("target_getter", move |ctx| {
            let env_raw = ctx.env.raw();
            let n = wrap_node(&doc_clone, node_id, ctx.env)?;
            let raw = JsValue::raw(&n);
            unsafe { Unknown::from_napi_value(env_raw, raw) }
        })?;
    let setter: Function<Unknown, Unknown> = event.get_named_property("__setLazyTarget")?;
    let getter_raw = JsValue::raw(&getter);
    let getter_unknown = unsafe { Unknown::from_napi_value(env.raw(), getter_raw) }?;
    setter.call(getter_unknown)?;
    Ok(())
}

/// Set `event.currentTarget` to a lazy getter and `event.eventPhase` to
/// the given phase value.
fn set_lazy_current_target(
    event: &Object,
    node_id: NodeId,
    phase: u32,
    doc: &Rc<SharedDoc>,
    env: &Env,
) -> Result<()> {
    let doc_clone = doc.clone();
    let getter: Function<(), Unknown> =
        env.create_function_from_closure("currentTarget_getter", move |ctx| {
            let env_raw = ctx.env.raw();
            let n = wrap_node(&doc_clone, node_id, ctx.env)?;
            let raw = JsValue::raw(&n);
            unsafe { Unknown::from_napi_value(env_raw, raw) }
        })?;
    let setter: Function<FnArgs<(Unknown, u32)>, Unknown> =
        event.get_named_property("__setLazyCurrentTarget")?;
    let getter_raw = JsValue::raw(&getter);
    let getter_unknown = unsafe { Unknown::from_napi_value(env.raw(), getter_raw) }?;
    setter.call(FnArgs {
        data: (getter_unknown, phase),
    })?;
    Ok(())
}

// ── Event serialization ───────────────────────────────────────────────
//
// Build an `EventPayload` from a blitz `DomEvent`. This is called once
// per event (not per receiver) and passed to the JS event factory.

fn serialize_event(event: &DomEvent) -> EventPayload {
    EventPayload {
        event_type: event.name().to_string(),
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
    let p = match data {
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
        | DomEventData::DoubleClick(p) => p,
        _ => return None,
    };
    let (kind, pointer_id) = match p.id {
        BlitzPointerId::Mouse => ("mouse", 1.0),
        BlitzPointerId::Pen => ("pen", 1.0),
        BlitzPointerId::Finger(id) => ("finger", id as f64),
    };
    Some(PointerData {
        inner: std::sync::Arc::new(p.clone()),
        kind: kind.to_string(),
        pointer_id,
    })
}

fn wheel_from(data: &DomEventData) -> Option<WheelData> {
    let DomEventData::Wheel(w) = data else {
        return None;
    };
    let (mode, delta_x, delta_y) = match w.delta {
        BlitzWheelDelta::Lines(x, y) => ("lines", x, y),
        BlitzWheelDelta::Pixels(x, y) => ("pixels", x, y),
    };
    Some(WheelData {
        inner: std::sync::Arc::new(w.clone()),
        mode: mode.to_string(),
        delta_x,
        delta_y,
    })
}

fn key_from(data: &DomEventData) -> Option<KeyData> {
    let k = match data {
        DomEventData::KeyDown(k) | DomEventData::KeyUp(k) | DomEventData::KeyPress(k) => k,
        _ => return None,
    };
    let state = match k.state {
        KeyState::Pressed => "pressed",
        KeyState::Released => "released",
    };
    Some(KeyData {
        inner: std::sync::Arc::new(k.clone()),
        state: state.to_string(),
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
