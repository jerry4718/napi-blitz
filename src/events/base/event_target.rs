//! The `EventTarget` class — root of the event-target side of the DOM.
//!
//! Holds the registered listeners in its own block (a `RefCell<Vec<..>>`
//! so the list can be mutated from within a dispatched callback). The
//! three-phase walk in `dispatch` drives `dispatchEvent`; the standalone
//! dispatch here covers the plain per-target case.

use std::cell::RefCell;

use napi::{
    Env, Result,
    bindgen_prelude::{FnArgs, FromNapiValue, FunctionRef, JsValue, Object, Unknown},
};
use napi_helpers::any_value::AnyValue;
use napi_inherit::layer::{Constructed, RootLayer, Super};
use napi_inherit_proc::layer;

use super::event::EventLayer;

/// One registered listener.
pub struct ListenerEntry {
    pub event_type: String,
    pub callback: FunctionRef<FnArgs<(AnyValue,)>, AnyValue>,
    pub capture: bool,
    pub once: bool,
    pub removed: bool,
}

/// Own block of the `EventTarget` class.
#[layer(js_name = "EventTarget")]
pub struct EventTargetLayer {
    listeners: RefCell<Vec<ListenerEntry>>,
}

#[layer]
impl EventTargetLayer {
    #[layer(constructor)]
    fn build(sup: Super<RootLayer>) -> Result<Constructed<Self>> {
        let done = sup.call(FnArgs::from(()))?;
        Ok(Constructed::new(
            done,
            Self {
                listeners: RefCell::new(Vec::new()),
            },
        ))
    }

    /// `target.addEventListener(type, callback, capture?)`.
    #[layer]
    fn add_event_listener(
        &self,
        env: &Env,
        event_type: String,
        callback: FunctionRef<FnArgs<(AnyValue,)>, AnyValue>,
        capture: Option<bool>,
    ) -> Result<()> {
        let mut listeners = self.listeners.borrow_mut();
        let capture = capture.unwrap_or(false);
        for l in listeners.iter() {
            if l.removed || l.event_type != event_type || l.capture != capture {
                continue;
            }
            // Same (type, callback, capture) triple — a duplicate registration.
            // The callback is resolved from its reference and compared by
            // identity.
            let existing = l.callback.borrow_back(env)?;
            let incoming = callback.borrow_back(env)?;
            if env.strict_equals(existing, incoming)? {
                return Ok(());
            }
        }
        listeners.push(ListenerEntry {
            event_type,
            callback,
            capture,
            once: false,
            removed: false,
        });
        Ok(())
    }

    /// `target.removeEventListener(type, callback, capture?)`.
    #[layer]
    fn remove_event_listener(
        &self,
        event_type: String,
        _callback: FunctionRef<FnArgs<(AnyValue,)>, AnyValue>,
        capture: Option<bool>,
    ) -> Result<()> {
        let mut listeners = self.listeners.borrow_mut();
        let capture = capture.unwrap_or(false);
        for l in listeners.iter_mut() {
            if !l.removed && l.event_type == event_type && l.capture == capture {
                l.removed = true;
                break;
            }
        }
        Ok(())
    }

    /// `target.dispatchEvent(event) -> boolean`. Invokes the matching
    /// listeners, honouring `stopImmediatePropagation`; returns whether the
    /// default was NOT prevented.
    #[layer]
    fn dispatch_event(&self, env: &Env, event: Object) -> Result<bool> {
        let event_type = napi_inherit::own::with_own::<EventLayer, _>(&event, |d| d.type_name())?;
        let event_value = AnyValue::from_unknown(unsafe {
            Unknown::from_napi_value(env.raw(), JsValue::raw(&event))?
        })?;

        {
            let listeners = self.listeners.borrow();
            for listener in listeners.iter() {
                let stop_immediate = napi_inherit::own::with_own::<EventLayer, _>(&event, |d| {
                    d.state.stop_immediate
                })?;
                if listener.removed || listener.capture || listener.event_type != event_type {
                    continue;
                }
                if stop_immediate {
                    break;
                }
                let f = listener.callback.borrow_back(env)?;
                let _ = f.call(FnArgs::from((event_value.clone(),)));
            }
        }

        let canceled = napi_inherit::own::with_own::<EventLayer, _>(&event, |d| d.state.canceled)?;
        Ok(!canceled)
    }
}
