//! The `Event` class — base of the DOM event hierarchy.
//!
//! Configuration fields (type, bubbles, cancelable, composed, timeStamp,
//! isTrusted) are immutable, exposed as read-only getters. Mutable dispatch
//! state (target, currentTarget, eventPhase, canceled, propagation flags)
//! lives in [`EventState`], written by the dispatch walk and read back by
//! the state getters.

use std::cell::RefCell;

use napi::{
    Env, Result, UnknownRef,
    bindgen_prelude::{FromNapiValue, JsValue, Object, Unknown},
};
use napi_inherit_proc::layer;

use super::dispatch;

/// A reference to the event's target (or current target).
///
/// The target is not always a node: it can be any object (window, app,
/// element), and it may not be materialized yet when dispatch starts. So the
/// target is held either as a direct value, or as a callable that produces
/// it lazily (cached after first resolution) — e.g. wrapping a node only when
/// the JS side first reads `event.target`.
pub enum DispatchTarget {
    /// No target assigned.
    None,
    /// A direct held reference to a JS value.
    Direct(UnknownRef),
    /// Lazily produces the target; the result is cached after first resolve.
    Callable {
        callable: Box<dyn Fn(&Env) -> Result<Unknown<'static>>>,
        cached: RefCell<Option<UnknownRef>>,
    },
}

impl Default for DispatchTarget {
    fn default() -> Self {
        Self::None
    }
}

impl DispatchTarget {
    /// Hold a JS value directly.
    pub fn from_value(env: &Env, v: Unknown<'_>) -> Result<Self> {
        let r = unsafe { UnknownRef::from_napi_value(env.raw(), JsValue::raw(&v)) }?;
        Ok(Self::Direct(r))
    }

    /// Resolve lazily via a callable; the produced value is cached.
    pub fn from_callable(callable: Box<dyn Fn(&Env) -> Result<Unknown<'static>>>) -> Self {
        Self::Callable {
            callable,
            cached: RefCell::new(None),
        }
    }

    /// Produce the target JS value (null when unset). Resolving a `Callable`
    /// caches its result.
    pub fn resolve(&self, env: &Env) -> Result<Unknown<'static>> {
        match self {
            Self::None => dispatch::null_unknown(env),
            Self::Direct(r) => {
                let v = r.get_value(env)?;
                dispatch::to_unknown(env, &v)
            }
            Self::Callable { callable, cached } => {
                if let Some(c) = cached.borrow().as_ref() {
                    let v = c.get_value(env)?;
                    return dispatch::to_unknown(env, &v);
                }
                let v = callable(env)?;
                *cached.borrow_mut() =
                    Some(unsafe { UnknownRef::from_napi_value(env.raw(), JsValue::raw(&v))? });
                Ok(v)
            }
        }
    }
}

/// Mutable per-event dispatch state, set by the dispatch walk in
/// `dispatch`. `target` / `current_target` are resolved lazily — the JS
/// side only materializes them when the getters are read.
#[derive(Default)]
pub struct EventState {
    pub target: DispatchTarget,
    pub current_target: DispatchTarget,
    pub phase: u32,
    pub canceled: bool,
    pub stop_propagation: bool,
    pub stop_immediate: bool,
    pub dispatching: bool,
}

/// Own block of the `Event` class.
#[layer(js_name = "Event")]
pub struct EventLayer {
    type_: String,
    #[layer(getter)]
    pub bubbles: bool,
    #[layer(getter)]
    pub cancelable: bool,
    #[layer(getter)]
    pub composed: bool,
    #[layer(getter)]
    pub time_stamp: f64,
    #[layer(getter)]
    pub is_trusted: bool,
    pub(crate) state: EventState,
}

#[layer]
impl EventLayer {
    #[layer]
    const NONE: u32 = 0;
    #[layer]
    const CAPTURING_PHASE: u32 = 1;
    #[layer]
    const AT_TARGET: u32 = 2;
    #[layer]
    const BUBBLING_PHASE: u32 = 3;

    /// `new Event(type)`.
    #[layer(constructor)]
    fn build(type_: String) -> Self {
        Self {
            type_,
            bubbles: false,
            cancelable: false,
            composed: false,
            time_stamp: 0.0,
            is_trusted: false,
            state: EventState::default(),
        }
    }

    #[layer(getter)]
    fn type_(&self) -> String {
        self.type_.clone()
    }

    /// The event type string, readable by the dispatch side.
    pub fn type_name(&self) -> String {
        self.type_.clone()
    }

    /// `event.target` — resolves the target only when read.
    #[layer(getter)]
    fn target(&self) -> Result<Unknown<'static>> {
        let env = dispatch::env()?;
        self.state.target.resolve(&env)
    }

    /// `event.currentTarget` — the current receiver during dispatch.
    #[layer(getter)]
    fn current_target(&self) -> Result<Unknown<'static>> {
        let env = dispatch::env()?;
        self.state.current_target.resolve(&env)
    }

    #[layer(getter)]
    fn event_phase(&self) -> u32 {
        self.state.phase
    }

    #[layer(getter)]
    fn default_prevented(&self) -> bool {
        self.state.canceled
    }

    #[layer]
    fn stop_propagation(&mut self) {
        self.state.stop_propagation = true;
    }

    #[layer]
    fn stop_immediate_propagation(&mut self) {
        self.state.stop_propagation = true;
        self.state.stop_immediate = true;
    }

    #[layer]
    fn prevent_default(&mut self) {
        if self.cancelable {
            self.state.canceled = true;
        }
    }

    /// `event.composedPath()`. Placeholder: the dispatch chain is populated
    /// by the dispatch side.
    #[layer]
    fn composed_path(&self) -> Vec<Unknown<'static>> {
        Vec::new()
    }
}

/// Build an `Event` from native data (Rust-side construction, bypassing the
/// JS `new` path).
pub fn create(env: &Env, type_: impl Into<String>) -> Result<Object<'_>> {
    use napi_inherit::layer::LayerChain;
    let chain = LayerChain {
        own: EventLayer::build(type_.into()),
        parent: (),
    };
    napi_inherit::class::new_from_chain::<EventLayer>(env, chain)
}
