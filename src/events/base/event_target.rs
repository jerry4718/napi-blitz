//! The `EventTarget` class — root of the event-target side of the DOM.
//!
//! Holds the registered listeners in its own block (a `RefCell<Vec<..>>`
//! so the list can be mutated from within a dispatched callback).

use std::cell::RefCell;

use super::EventLayer;
use napi::{
    Env, Error, Result, Status,
    bindgen_prelude::{
        Either, FnArgs, FromNapiValue, Function, FunctionRef, JsObjectValue, JsValue, Object,
    },
    check_status, sys,
};
use napi_derive::napi;
use napi_helpers::inherits::LayerRef;
use napi_helpers::{
    JsWeakRef,
    anything::Anything,
    inherits::{Constructed, RootLayer, Super, proc::layer, with_own},
};

thread_local! {
    static LISTENER_OPS: RefCell<Option<ListenerOps>> = const { RefCell::new(None) };
}

#[napi(object, object_to_js = false)]
pub struct ListenerOps {
    #[napi(
        ts_type = "(target: EventTarget, listener: Function | { handleEvent: Function }, spec: ListenerSpec) => boolean"
    )]
    pub insert_listener:
        FunctionRef<FnArgs<(LayerRef<EventTargetLayer>, Anything, ListenerSpec)>, bool>,
    #[napi(
        ts_type = "(target: EventTarget, listener: Function | { handleEvent: Function }, spec: ListenerSpec) => boolean"
    )]
    pub delete_listener:
        FunctionRef<FnArgs<(LayerRef<EventTargetLayer>, Anything, ListenerSpec)>, bool>,
}

#[napi(object, object_from_js = false)]
#[derive(Clone)]
pub struct ListenerSpec {
    pub r#type: String,
    pub capture: bool,
    pub kind: String,
}

#[napi]
pub fn set_listener_ops(_env: &Env, ops: ListenerOps) -> Result<()> {
    LISTENER_OPS.with(|cell| *cell.borrow_mut() = Some(ops));
    Ok(())
}

/// Run `f` with the JS-registered listener ops. Errors when the JS side has
/// not registered them: without the registry there is no strong anchor for
/// callbacks, so Rust-side weak holding alone would misbehave silently.
fn with_listener_ops<R>(f: impl FnOnce(&ListenerOps) -> Result<R>) -> Result<R> {
    LISTENER_OPS.with(|cell| {
        let borrow = cell.borrow();
        let ops = borrow
            .as_ref()
            .ok_or_else(|| Error::from_reason("listener ops are not registered"))?;
        f(ops)
    })
}

/// Strongly anchor `callback` on the JS-side registry for `target`'s
/// lifetime. The registry keeps the callback alive while the target lives
/// and drops it with the target, so a callback closure capturing JS objects
/// cannot form a native-rooted reference cycle.
fn insert_js_listener(
    env: &Env,
    target: &Object,
    callback: Anything,
    spec: ListenerSpec,
) -> Result<()> {
    let target_value = unsafe { LayerRef::from_napi_value(env.raw(), JsValue::raw(target))? };
    with_listener_ops(|ops| {
        let insert = ops.insert_listener.borrow_back(env)?;
        insert.call(FnArgs::from((target_value, callback, spec)))
    })?;
    Ok(())
}

/// Release the JS-side anchor for a removed or once-fired listener.
fn delete_js_listener(
    env: &Env,
    target: &Object,
    callback: Anything,
    spec: ListenerSpec,
) -> Result<()> {
    let target_value = unsafe { LayerRef::from_napi_value(env.raw(), JsValue::raw(target))? };
    with_listener_ops(|ops| {
        let delete = ops.delete_listener.borrow_back(env)?;
        delete.call(FnArgs::from((target_value, callback, spec)))
    })?;
    Ok(())
}

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

/// A listener callback held weakly by Rust for dispatch; its strong
/// ownership lives in the JS-side listener registry, anchored by the
/// target's own lifetime.
pub enum ListenerCallback {
    /// `addEventListener(type, fn)`.
    Function(JsWeakRef),
    /// `addEventListener(type, listenerObject)` — invoke `handleEvent`.
    HandlerObject(JsWeakRef),
    /// An `on<event>` attribute handler, always a function.
    #[allow(dead_code)]
    AttributeFunction(JsWeakRef),
}

impl ListenerCallback {
    fn weak(&self) -> &JsWeakRef {
        match self {
            Self::Function(callback)
            | Self::HandlerObject(callback)
            | Self::AttributeFunction(callback) => callback,
        }
    }

    /// Registry kind reported to the JS-side anchor.
    fn kind(&self) -> &'static str {
        match self {
            Self::AttributeFunction(_) => "attribute",
            Self::Function(_) | Self::HandlerObject(_) => "basic",
        }
    }

    fn raw_value(&self, env: &Env) -> Result<sys::napi_value> {
        self.weak()
            .get_value(env)
            .map(|value| value.raw())
            .ok_or_else(|| Error::from_reason("event listener callback was collected"))
    }
}

/// One registered listener.
pub struct ListenerEntry {
    pub event_type: String,
    pub callback: ListenerCallback,
    pub capture: bool,
    #[allow(dead_code)]
    pub passive: bool,
    pub once: bool,
    pub removed: bool,
}

unsafe fn same_callback(
    env: &Env,
    incoming: sys::napi_value,
    stored: &ListenerCallback,
) -> Result<bool> {
    // An entry exists only while the JS registry anchors its callback, so
    // resolution cannot fail for a live entry.
    let Some(stored) = stored.weak().get_value(env) else {
        unreachable!("listener callback was collected while its target lives");
    };
    let stored = stored.raw();
    let mut result = false;
    check_status!(unsafe { sys::napi_strict_equals(env.raw(), incoming, stored, &mut result) })?;
    Ok(result)
}

/// Own block of the `EventTarget` class.
#[layer]
pub struct EventTargetLayer {
    listeners: RefCell<Vec<ListenerEntry>>,
}

impl EventTargetLayer {
    /// Fresh own block with an empty listener list, for Rust-side
    /// data-chain construction (the parent slot of derived layers such as
    /// a DOM `Node`).
    pub fn fresh() -> Self {
        Self {
            listeners: RefCell::new(Vec::new()),
        }
    }
}

#[layer(js_name = "EventTarget")]
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
    fn add_event_listener(
        &self,
        this: &Object,
        env: &Env,
        event_type: String,
        callback: Anything,
        options: Option<Either<AddEventListenerOptions, bool>>,
    ) -> Result<()> {
        let callback_value = match &callback {
            Anything::Function(reference) | Anything::Object(reference) => unsafe {
                reference.raw_value(env)?
            },
            _ => {
                return Err(Error::new(
                    Status::FunctionExpected,
                    "parameter 2 expected a Function or EventListener object",
                ));
            }
        };
        let callback = match callback {
            Anything::Function(_) => ListenerCallback::Function(JsWeakRef::new(
                &Object::from_raw(env.raw(), callback_value),
                env,
            )?),
            Anything::Object(_) => ListenerCallback::HandlerObject(JsWeakRef::new(
                &Object::from_raw(env.raw(), callback_value),
                env,
            )?),
            _ => {
                return Err(Error::new(
                    Status::InvalidArg,
                    "parameter 2 expected a Function or EventListener object",
                ));
            }
        };

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

        {
            let listeners = self.listeners.borrow();
            for l in listeners.iter() {
                if l.removed
                    || l.event_type != event_type
                    || l.capture != capture
                    || l.callback.kind() != callback.kind()
                {
                    continue;
                }
                // Same (type, callback, capture, kind) quadruple — a duplicate
                // registration. `on<event> = fn` and
                // `addEventListener(event, fn)` may coexist for one function,
                // so the registry kind disambiguates.
                if unsafe { same_callback(env, callback_value, &l.callback) }? {
                    return Ok(());
                }
            }
        }

        // Anchor the callback on the JS-side registry before recording it:
        // the registry holds the callback's only strong reference for the
        // target's lifetime, so the Rust entry can stay weak.
        let kind = callback.kind();
        let spec = ListenerSpec {
            r#type: event_type.clone(),
            capture,
            kind: kind.to_string(),
        };
        let callback_for_js = unsafe { Anything::from_napi_value(env.raw(), callback_value)? };
        insert_js_listener(env, this, callback_for_js, spec)?;

        self.listeners.borrow_mut().push(ListenerEntry {
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
        this: &Object,
        env: &Env,
        event_type: String,
        callback: Anything,
        options: Option<Either<EventListenerOptions, bool>>,
    ) -> Result<()> {
        let callback_value = match callback {
            Anything::Function(callback) | Anything::Object(callback) => unsafe {
                callback.raw_value(env)?
            },
            _ => {
                return Err(Error::new(
                    Status::InvalidArg,
                    "parameter 2 expected a Function or EventListener object",
                ));
            }
        };

        let capture = match options {
            Some(Either::A(o)) => o.capture.unwrap_or(false),
            Some(Either::B(use_capture)) => use_capture,
            None => false,
        };

        // Remove the exact (type, callback, capture) basic entry and release
        // its JS-side registry anchor. `removeEventListener` never touches
        // attribute entries (`on<event> = fn`), which are cleared by the
        // attribute setter path.
        let (removed_callback, removed_kind) = {
            let mut listeners = self.listeners.borrow_mut();
            let Some(index) = listeners.iter().position(|l| {
                !l.removed
                    && l.callback.kind() == "basic"
                    && l.event_type == event_type
                    && l.capture == capture
                    && unsafe { same_callback(env, callback_value, &l.callback) }.unwrap_or(false)
            }) else {
                return Ok(());
            };
            let entry = listeners.remove(index);
            let kind = entry.callback.kind().to_string();
            (entry.callback, kind)
        };

        if let Ok(raw) = removed_callback.raw_value(env) {
            let callback_for_js = unsafe { Anything::from_napi_value(env.raw(), raw)? };
            let spec = ListenerSpec {
                r#type: event_type,
                capture,
                kind: removed_kind,
            };
            delete_js_listener(env, this, callback_for_js, spec)?;
        }

        Ok(())
    }

    /// `target.dispatchEvent(event) -> boolean`. Invokes the matching
    /// listeners, honouring `stopImmediatePropagation`; returns whether the
    /// default was NOT prevented.
    #[layer]
    /// Dispatch a single event to this target's listeners. The event must
    /// be an `Event`-derived layer instance; the canceled flag is read
    /// back from its `EventLayer` state. `pub` so the Rust dispatch driver
    /// (`napi-blitz-dom`) can invoke it directly.
    pub fn dispatch_event(&self, this: &Object, env: &Env, event: Object) -> Result<bool> {
        let event_type = with_own::<EventLayer, _>(&event, |d| d.type_name())?;
        let event_value = unsafe { Anything::from_napi_value(env.raw(), JsValue::raw(&event))? };

        // The callback never runs while the listener list is borrowed, so a
        // listener may re-enter this target from within its own invocation.
        // New registrations appended during dispatch do not fire for the
        // current event.
        let total = self.listeners.borrow().len();
        let mut cursor = 0;
        let mut once_fired: Vec<(sys::napi_value, &'static str)> = Vec::new();
        loop {
            let stop_immediate = with_own::<EventLayer, _>(&event, |d| d.state.stop_immediate)?;
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
            let (once, callback_kind, callback) = {
                let listeners = self.listeners.borrow();
                (
                    listeners[idx].once,
                    matches!(listeners[idx].callback, ListenerCallback::HandlerObject(_)),
                    listeners[idx].callback.raw_value(env)?,
                )
            };
            if callback_kind {
                let object = Object::from_raw(env.raw(), callback);
                let handle_event: Function<FnArgs<(Anything,)>, Anything> =
                    object.get_named_property("handleEvent")?;
                let _ = handle_event.apply(object, FnArgs::from((event_value.clone(),)));
            } else {
                let function = unsafe {
                    Function::<FnArgs<(Anything,)>, Anything>::from_napi_value(env.raw(), callback)?
                };
                let _ = function.call(FnArgs::from((event_value.clone(),)));
            }
            if once {
                self.listeners.borrow_mut()[idx].removed = true;
                let kind = {
                    let listeners = self.listeners.borrow();
                    listeners[idx].callback.kind()
                };
                once_fired.push((callback, kind));
            }
            cursor = idx + 1;
        }

        // Drop once-fired entries left behind by the walk and release their
        // JS-side registry anchors.
        {
            let mut listeners = self.listeners.borrow_mut();
            listeners.retain(|l| !l.removed);
        }
        for (callback, kind) in once_fired {
            let callback_for_js = unsafe { Anything::from_napi_value(env.raw(), callback)? };
            let spec = ListenerSpec {
                r#type: event_type.clone(),
                capture: false,
                kind: kind.to_string(),
            };
            delete_js_listener(env, this, callback_for_js, spec)?;
        }

        let canceled = with_own::<EventLayer, _>(&event, |d| d.state.canceled)?;
        Ok(!canceled)
    }
}
