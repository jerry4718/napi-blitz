//! The `Event` class — base of the DOM event hierarchy.
//!
//! Configuration fields (type, bubbles, cancelable, composed, timeStamp,
//! isTrusted) are immutable, exposed as read-only getters. Mutable dispatch
//! state (target, currentTarget, eventPhase, canceled, propagation flags)
//! lives in [`EventState`], written by the event methods and read by the
//! dispatch side.

use std::cell::RefCell;

use napi::{
    Env, Result,
    bindgen_prelude::{FnArgs, Object},
};
use napi_derive::napi;
use napi_helpers::inherits::{
    Constructed, LayerChain, LayerRef, RootLayer, Super, new_from_chain, proc::layer,
};

use crate::events::base::EventTargetLayer;

/// `dictionary EventInit { boolean bubbles = false; boolean cancelable = false; boolean composed = false; }`
#[napi(object)]
#[derive(Default)]
pub struct EventInit {
    pub bubbles: Option<bool>,
    pub cancelable: Option<bool>,
    pub composed: Option<bool>,
}

/// A lazily-resolved target producer; see [`DispatchTarget::Callable`].
type TargetResolver = Box<dyn Fn(&Env) -> Result<LayerRef<EventTargetLayer>>>;

/// A reference to the event's target (or current target).
///
/// The target is not always a node: it can be any object on an
/// `EventTarget` chain (window, app, element), and it may not be
/// materialized yet when dispatch starts. So the target is held either as
/// a direct value, or as a callable that produces it lazily (cached after
/// first resolution) — e.g. wrapping a node only when the JS side first
/// reads `event.target`.
#[derive(Default)]
pub enum DispatchTarget {
    /// No target assigned.
    #[default]
    None,
    /// A direct held value.
    Direct(LayerRef<EventTargetLayer>),
    /// Lazily produces the target; the result is cached after first resolve.
    Callable {
        callable: TargetResolver,
        cached: RefCell<Option<LayerRef<EventTargetLayer>>>,
    },
}

impl DispatchTarget {
    /// Hold a JS value directly.
    #[allow(dead_code)]
    pub fn from_value(v: LayerRef<EventTargetLayer>) -> Self {
        Self::Direct(v)
    }

    /// Resolve lazily via a callable; the produced value is cached.
    pub fn from_callable(callable: TargetResolver) -> Self {
        Self::Callable {
            callable,
            cached: RefCell::new(None),
        }
    }

    /// Produce the target (None when unset). Resolving a `Callable` caches
    /// its result; the cached `LayerRef` is handed out by clone, so each
    /// read shares one `napi_ref` instead of creating a fresh one.
    pub fn resolve(&self, env: &Env) -> Result<Option<LayerRef<EventTargetLayer>>> {
        match self {
            Self::None => Ok(None),
            Self::Direct(v) => Ok(Some(v.clone())),
            Self::Callable { callable, cached } => {
                if let Some(c) = cached.borrow().as_ref() {
                    return Ok(Some(c.clone()));
                }
                let v = callable(env)?;
                *cached.borrow_mut() = Some(v.clone());
                Ok(Some(v))
            }
        }
    }
}

/// Mutable per-event dispatch state. `target` / `current_target` are
/// resolved lazily — the JS side only materializes them when the getters
/// are read.
#[derive(Default)]
pub struct EventState {
    pub target: DispatchTarget,
    pub current_target: DispatchTarget,
    pub phase: u32,
    pub canceled: bool,
    pub stop_propagation: bool,
    pub stop_immediate: bool,
    #[allow(dead_code)]
    pub dispatching: bool,
}

/// Own block of the `Event` class.
#[layer]
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

#[layer(js_name = "Event")]
impl EventLayer {
    #[layer]
    const NONE: u32 = 0;
    #[layer]
    const CAPTURING_PHASE: u32 = 1;
    #[layer]
    const AT_TARGET: u32 = 2;
    #[layer]
    const BUBBLING_PHASE: u32 = 3;

    /// `new Event(type, init?)` — `init` follows `dictionary EventInit`.
    #[layer(constructor)]
    fn build(
        type_: String,
        init: Option<EventInit>,
        sup: Super<RootLayer>,
    ) -> Result<Constructed<Self>> {
        let EventInit {
            bubbles,
            cancelable,
            composed,
        } = init.unwrap_or_default();
        let done = sup.call(FnArgs::from(()))?;
        Ok(Constructed::new(
            done,
            Self {
                type_,
                bubbles: bubbles.unwrap_or(false),
                cancelable: cancelable.unwrap_or(false),
                composed: composed.unwrap_or(false),
                time_stamp: 0.0,
                is_trusted: false,
                state: EventState::default(),
            },
        ))
    }

    #[layer(getter)]
    fn type_(&self) -> String {
        self.type_.clone()
    }

    /// The event type string, readable by the dispatch side.
    pub fn type_name(&self) -> String {
        self.type_.clone()
    }

    /// Mutable access to the dispatch state block, for the Rust-side
    /// dispatch driver (`napi-blitz-dom`) to set the target /
    /// currentTarget / phase and read back the outcome flags.
    pub fn state_mut(&mut self) -> &mut EventState {
        &mut self.state
    }

    /// Shared access to the dispatch state block.
    pub fn state_ref(&self) -> &EventState {
        &self.state
    }

    /// `event.target` — resolves the target only when read.
    // event.rs expands before event_target.rs registers the layer, so the
    // automatic `LayerRef<L>` mapping cannot resolve the JS name here.
    #[layer(getter, ts_return_type = "EventTarget | null")]
    fn target(&self, env: &Env) -> Result<Option<LayerRef<EventTargetLayer>>> {
        self.state.target.resolve(env)
    }

    /// `event.currentTarget` — the current receiver during dispatch.
    #[layer(getter, ts_return_type = "EventTarget | null")]
    fn current_target(&self, env: &Env) -> Result<Option<LayerRef<EventTargetLayer>>> {
        self.state.current_target.resolve(env)
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

    /// `cancelBubble` mirrors whether `stopPropagation()` was called.
    #[layer(getter)]
    fn cancel_bubble(&self) -> bool {
        self.state.stop_propagation
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
    #[layer(ts_return_type = "Array<EventTarget>")]
    fn composed_path(&self) -> Vec<LayerRef<EventTargetLayer>> {
        Vec::new()
    }
}

impl EventLayer {
    /// Fresh own block with default configuration, for Rust-side data-chain
    /// construction (the parent slot of derived event layers such as `UIEvent`).
    #[allow(dead_code)]
    pub fn fresh() -> Self {
        Self {
            type_: String::new(),
            bubbles: false,
            cancelable: false,
            composed: false,
            time_stamp: 0.0,
            is_trusted: false,
            state: EventState::default(),
        }
    }

    /// Own block with explicit configuration, for Rust-side data-chain
    /// construction of dispatched events (`napi-blitz-dom`'s event builder).
    pub fn with_init(
        type_: impl Into<String>,
        bubbles: bool,
        cancelable: bool,
        composed: bool,
    ) -> Self {
        Self {
            type_: type_.into(),
            bubbles,
            cancelable,
            composed,
            time_stamp: 0.0,
            is_trusted: false,
            state: EventState::default(),
        }
    }
}

/// Build an `Event` from native data (Rust-side construction, bypassing the
/// JS `new` path).
#[allow(dead_code)]
pub fn create(env: &Env, type_: impl Into<String>) -> Result<Object<'_>> {
    let chain = LayerChain {
        own: EventLayer {
            type_: type_.into(),
            bubbles: false,
            cancelable: false,
            composed: false,
            time_stamp: 0.0,
            is_trusted: false,
            state: EventState::default(),
        },
        parent: (),
    };
    new_from_chain::<EventLayer>(env, chain)
}
