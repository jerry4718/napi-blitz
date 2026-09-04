//! The `EventTarget` class — root of the event-target side of the DOM.
//!
//! Holds the registered listeners in its own block (a `RefCell<Vec<..>>`
//! so the list can be mutated from within a dispatched callback).

use std::cell::RefCell;

use napi::{
    Env, Error, Result, Status,
    bindgen_prelude::{Either, FnArgs, FromNapiValue, Function, JsValue, Object},
    check_status, sys,
};
use napi_derive::napi;
use napi_helpers::anything::{Anything, OtherRef};
use napi_inherit::layer::{Constructed, RootLayer, Super};
use napi_inherit_proc::layer;

use super::event::EventLayer;

/// `dictionary EventListenerOptions { boolean capture = false; }`
#[napi(object)]
#[derive(Default)]
pub struct EventListenerOptions {
    pub capture: Option<bool>,
}

/// `dictionary AddEventListenerOptions : EventListenerOptions`.
/// The `signal` member is not implemented yet.
#[napi(object)]
#[derive(Default)]
pub struct AddEventListenerOptions {
    pub capture: Option<bool>,
    pub passive: Option<bool>,
    pub once: Option<bool>,
}

/// One registered listener.
pub struct ListenerEntry {
    pub event_type: String,
    pub callback: OtherRef,
    pub capture: bool,
    pub passive: bool,
    pub once: bool,
    pub removed: bool,
}

/// Whether a live callback value and a stored callback refer to the same
/// JS value under strict equality. Only the stored side needs
/// re-materializing from its reference; the incoming value is the current
/// call's.
fn same_callback(env: &Env, incoming: sys::napi_value, stored: &OtherRef) -> Result<bool> {
    let stored = stored.raw_value(env)?;
    let mut result = false;
    check_status!(unsafe { sys::napi_strict_equals(env.raw(), incoming, stored, &mut result) })?;
    Ok(result)
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

    /// `target.addEventListener(type, callback)` — `callback` may also be
    /// registered as
    /// `addEventListener(type, callback, options?)`, where `options` is an
    /// `AddEventListenerOptions` object or a `useCapture` boolean.
    #[layer]
    fn add_event_listener<'env>(
        &self,
        env: &'env Env,
        event_type: String,
        callback: Anything,
        options: Option<Either<AddEventListenerOptions, bool>>,
    ) -> Result<()> {
        let Anything::Function(callback) = callback else {
            return Err(Error::new(
                Status::FunctionExpected,
                "parameter 2 expected a Function",
            ));
        };
        let callback_value = callback.raw_value(env)?;

        let mut capture = false;
        let mut once = false;
        let mut passive = false;

        if let Some(options) = options {
            match options {
                Either::A(o) => {
                    capture = o.capture.unwrap_or(false);
                    once = o.once.unwrap_or(false);
                    passive = o.passive.unwrap_or(false);
                }
                Either::B(use_capture) => capture = use_capture,
            }
        }

        let mut listeners = self.listeners.borrow_mut();
        for l in listeners.iter() {
            if l.removed || l.event_type != event_type || l.capture != capture {
                continue;
            }
            // Same (type, callback, capture) triple — a duplicate registration.
            if same_callback(env, callback_value, &l.callback)? {
                return Ok(());
            }
        }

        listeners.push(ListenerEntry {
            event_type,
            callback,
            capture,
            passive,
            once,
            removed: false,
        });

        Ok(())
    }

    /// `target.removeEventListener(type, callback)` — `callback` may also be
    /// unregistered as
    /// `removeEventListener(type, callback, options?)`, where `options` is an
    /// `EventListenerOptions` object or a `useCapture` boolean.
    #[layer]
    fn remove_event_listener(
        &self,
        env: &Env,
        event_type: String,
        callback: Anything,
        options: Option<Either<EventListenerOptions, bool>>,
    ) -> Result<()> {
        let Anything::Function(callback) = callback else {
            return Err(Error::new(
                Status::FunctionExpected,
                "parameter 2 expected a Function",
            ));
        };
        let callback_value = callback.raw_value(env)?;

        let capture = match options {
            Some(Either::A(o)) => o.capture.unwrap_or(false),
            Some(Either::B(use_capture)) => use_capture,
            None => false,
        };

        let mut listeners = self.listeners.borrow_mut();
        // Remove the exact (type, callback, capture) entry; dropping it
        // releases the reference to the callback.
        let Some(index) = listeners.iter().position(|l| {
            !l.removed
                && l.event_type == event_type
                && l.capture == capture
                && same_callback(env, callback_value, &l.callback).unwrap_or(false)
        }) else {
            return Ok(());
        };

        listeners.remove(index);

        Ok(())
    }

    /// `target.dispatchEvent(event) -> boolean`. Invokes the matching
    /// listeners, honouring `stopImmediatePropagation`; returns whether the
    /// default was NOT prevented.
    #[layer]
    fn dispatch_event(&self, env: &Env, event: Object) -> Result<bool> {
        let event_type = napi_inherit::own::with_own::<EventLayer, _>(&event, |d| d.type_name())?;
        let event_value = unsafe { Anything::from_napi_value(env.raw(), JsValue::raw(&event))? };

        // The callback never runs while the listener list is borrowed, so a
        // listener may re-enter this target from within its own invocation.
        // New registrations appended during dispatch do not fire for the
        // current event.
        let total = self.listeners.borrow().len();
        let mut cursor = 0;
        loop {
            let stop_immediate =
                napi_inherit::own::with_own::<EventLayer, _>(&event, |d| d.state.stop_immediate)?;
            if stop_immediate {
                break;
            }
            let next = {
                let listeners = self.listeners.borrow();
                (cursor..total).find(|&j| {
                    let l = &listeners[j];
                    !l.removed && !l.capture && l.event_type == event_type
                })
            };
            let Some(idx) = next else { break };
            let (once, f) = {
                let listeners = self.listeners.borrow();
                let callback = listeners[idx].callback.raw_value(env)?;
                (listeners[idx].once, unsafe {
                    Function::<FnArgs<(Anything,)>, Anything>::from_napi_value(env.raw(), callback)?
                })
            };
            let _ = f.call(FnArgs::from((event_value.clone(),)));
            if once {
                self.listeners.borrow_mut()[idx].removed = true;
            }
            cursor = idx + 1;
        }

        // Drop once-fired entries left behind by the walk; releasing them
        // here frees the references they hold.
        self.listeners.borrow_mut().retain(|l| !l.removed);

        let canceled = napi_inherit::own::with_own::<EventLayer, _>(&event, |d| d.state.canceled)?;
        Ok(!canceled)
    }
}
