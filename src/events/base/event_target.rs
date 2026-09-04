//! The `EventTarget` class — root of the event-target side of the DOM.
//!
//! Holds the registered listeners in its own block (a `RefCell<Vec<..>>`
//! so the list can be mutated from within a dispatched callback). The
//! three-phase walk in `dispatch` drives `dispatchEvent`; the standalone
//! dispatch here covers the plain per-target case.

use std::cell::RefCell;

use napi::{
    Env, Result,
    bindgen_prelude::{FnArgs, FunctionRef, JsValue, Object, Unknown},
};
use napi_inherit_proc::layer;

use super::{dispatch, event::EventLayer};

/// One registered listener.
pub struct ListenerEntry {
    pub event_type: String,
    pub callback: FunctionRef<FnArgs<(Unknown<'static>,)>, Unknown<'static>>,
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
    fn build() -> Self {
        Self {
            listeners: RefCell::new(Vec::new()),
        }
    }

    /// `target.addEventListener(type, callback, capture?)`.
    #[layer]
    fn add_event_listener(
        &self,
        event_type: String,
        callback: FunctionRef<FnArgs<(Unknown<'static>,)>, Unknown<'static>>,
        capture: Option<bool>,
    ) -> Result<()> {
        let mut listeners = self.listeners.borrow_mut();
        let capture = capture.unwrap_or(false);
        if listeners
            .iter()
            .any(|l| !l.removed && l.event_type == event_type && l.capture == capture)
        {
            return Ok(());
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
        _callback: FunctionRef<FnArgs<(Unknown<'static>,)>, Unknown<'static>>,
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
    fn dispatch_event(&self, event: Object) -> Result<bool> {
        let event_type = napi_inherit::own::with_own::<EventLayer, _>(&event, |d| d.type_name())?;

        let env = dispatch::env()?;
        let event_unknown = to_unknown(&env, &event)?;

        {
            let listeners = self.listeners.borrow();
            for listener in listeners.iter() {
                if listener.removed || listener.capture || listener.event_type != event_type {
                    continue;
                }
                if napi_inherit::own::with_own::<EventLayer, _>(&event, |d| d.state.stop_immediate)?
                {
                    break;
                }
                let f = listener.callback.borrow_back(&env)?;
                let _ = f.call(FnArgs::from((event_unknown.clone(),)));
            }
        }

        let canceled = napi_inherit::own::with_own::<EventLayer, _>(&event, |d| d.state.canceled)?;
        Ok(!canceled)
    }
}

/// Convert a JS object to an `Unknown` value (no conversion/ref creation).
fn to_unknown(env: &Env, obj: &Object) -> Result<Unknown<'static>> {
    use napi::bindgen_prelude::FromNapiValue;
    unsafe { Unknown::from_napi_value(env.raw(), JsValue::raw(obj)) }
}
