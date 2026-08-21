//! Global addon-level state: JS constructor refs, event factory, napi env.
//!
//! These are registered once during addon init and never change.
//! All documents share them. Accessed only from the JS thread.
//!
//! Uses `thread_local!` because `Env` and `FunctionRef` are not
//! `Send`/`Sync`. Node.js runs JS on a single thread, so thread-local
//! storage is correct and avoids unsafe `Sync` impls.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use crate::dom::{node_handle::NativeNode, payload::EventPayload};
use blitz::dom::{LocalName, Namespace};
use napi::{
    Env, Error, Result, Status, UnknownRef,
    bindgen_prelude::{FnArgs, FunctionRef, ObjectRef},
};

// ── Type aliases ──────────────────────────────────────────────────────

type NodeConstructor = FunctionRef<FnArgs<(NativeNode, ObjectRef)>, ObjectRef>;
type ElementConstructor =
    FunctionRef<FnArgs<(NativeNode, ObjectRef, Option<ObjectRef>)>, ObjectRef>;
type EventFactory = FunctionRef<FnArgs<(EventPayload,)>, ObjectRef>;
type DispatchFn = FunctionRef<FnArgs<(ObjectRef, ObjectRef)>, UnknownRef>;
type CancelBubbleGetter = FunctionRef<FnArgs<(ObjectRef,)>, bool>;
type DefaultPreventedGetter = FunctionRef<FnArgs<(ObjectRef,)>, bool>;
type LazyTargetSetter = FunctionRef<FnArgs<(ObjectRef, ObjectRef)>, UnknownRef>;
type LazyCurrentTargetSetter = FunctionRef<FnArgs<(ObjectRef, ObjectRef, u32)>, UnknownRef>;

struct GlobalRegistry {
    /// nodeType -> JS constructor function: `new (handle, doc) -> Node`
    node_constructors: RefCell<HashMap<u32, Rc<NodeConstructor>>>,
    /// (namespace, local name) -> JS constructor function
    element_constructors: RefCell<HashMap<(Namespace, LocalName), Rc<ElementConstructor>>>,
    /// JS event factory function: `(payload) -> Event`
    event_factory_ref: RefCell<Option<Rc<EventFactory>>>,
    /// JS dispatchEvent function: `(target, event) -> void`
    dispatch_fn_ref: RefCell<Option<Rc<DispatchFn>>>,
    /// JS cancelBubble getter: `(event) -> bool`
    cancel_bubble_getter_ref: RefCell<Option<Rc<CancelBubbleGetter>>>,
    /// JS defaultPrevented getter: `(event) -> bool`
    default_prevented_getter_ref: RefCell<Option<Rc<DefaultPreventedGetter>>>,
    /// JS lazy target setter: `(event, getter) -> void`
    lazy_target_setter_ref: RefCell<Option<Rc<LazyTargetSetter>>>,
    /// JS lazy currentTarget setter: `(event, getter, phase) -> void`
    lazy_current_target_setter_ref: RefCell<Option<Rc<LazyCurrentTargetSetter>>>,
    /// napi env (stable for addon lifetime in Node.js)
    env: Cell<Option<Env>>,
}

thread_local! {
    static GLOBAL_REGISTRY: GlobalRegistry = GlobalRegistry {
        node_constructors: RefCell::new(HashMap::new()),
        element_constructors: RefCell::new(HashMap::new()),
        event_factory_ref: RefCell::new(None),
        dispatch_fn_ref: RefCell::new(None),
        cancel_bubble_getter_ref: RefCell::new(None),
        default_prevented_getter_ref: RefCell::new(None),
        lazy_target_setter_ref: RefCell::new(None),
        lazy_current_target_setter_ref: RefCell::new(None),
        env: Cell::new(None),
    };
}

pub fn set_env(env: Env) {
    GLOBAL_REGISTRY.with(|g| g.env.set(Some(env)));
}

pub fn env() -> Result<Env> {
    GLOBAL_REGISTRY.with(|g| {
        g.env
            .get()
            .ok_or_else(|| Error::new(Status::GenericFailure, "GlobalCreators env not initialized"))
    })
}

pub fn insert_node_constructor(node_type: u32, ctor: NodeConstructor) {
    GLOBAL_REGISTRY.with(|g| {
        g.node_constructors
            .borrow_mut()
            .insert(node_type, Rc::new(ctor))
    });
}

pub fn get_node_constructor(node_type: u32) -> Option<Rc<NodeConstructor>> {
    GLOBAL_REGISTRY.with(|g| g.node_constructors.borrow().get(&node_type).cloned())
}

pub fn insert_element_constructor(ns: Namespace, local: LocalName, ctor: ElementConstructor) {
    GLOBAL_REGISTRY.with(|g| {
        g.element_constructors
            .borrow_mut()
            .insert((ns, local), Rc::new(ctor))
    });
}

pub fn get_element_constructor(
    ns: &Namespace,
    local: &LocalName,
) -> Option<Rc<ElementConstructor>> {
    GLOBAL_REGISTRY.with(|g| {
        g.element_constructors
            .borrow()
            .get(&(ns.clone(), local.clone()))
            .cloned()
    })
}

pub fn set_event_factory(factory: EventFactory) {
    GLOBAL_REGISTRY.with(|g| {
        *g.event_factory_ref.borrow_mut() = Some(Rc::new(factory));
    });
}

pub fn get_event_factory() -> Option<Rc<EventFactory>> {
    GLOBAL_REGISTRY.with(|g| g.event_factory_ref.borrow().as_ref().cloned())
}

pub fn set_dispatch_fn(dispatch_fn: DispatchFn) {
    GLOBAL_REGISTRY.with(|g| {
        *g.dispatch_fn_ref.borrow_mut() = Some(Rc::new(dispatch_fn));
    });
}

pub fn get_dispatch_fn() -> Option<Rc<DispatchFn>> {
    GLOBAL_REGISTRY.with(|g| g.dispatch_fn_ref.borrow().as_ref().cloned())
}

pub fn set_cancel_bubble_getter(fn_ref: CancelBubbleGetter) {
    GLOBAL_REGISTRY.with(|g| {
        *g.cancel_bubble_getter_ref.borrow_mut() = Some(Rc::new(fn_ref));
    });
}

pub fn get_cancel_bubble_getter() -> Option<Rc<CancelBubbleGetter>> {
    GLOBAL_REGISTRY.with(|g| g.cancel_bubble_getter_ref.borrow().as_ref().cloned())
}

pub fn set_default_prevented_getter(fn_ref: DefaultPreventedGetter) {
    GLOBAL_REGISTRY.with(|g| {
        *g.default_prevented_getter_ref.borrow_mut() = Some(Rc::new(fn_ref));
    });
}

pub fn get_default_prevented_getter() -> Option<Rc<DefaultPreventedGetter>> {
    GLOBAL_REGISTRY.with(|g| g.default_prevented_getter_ref.borrow().as_ref().cloned())
}

pub fn set_lazy_target_setter(fn_ref: LazyTargetSetter) {
    GLOBAL_REGISTRY.with(|g| {
        *g.lazy_target_setter_ref.borrow_mut() = Some(Rc::new(fn_ref));
    });
}

pub fn get_lazy_target_setter() -> Option<Rc<LazyTargetSetter>> {
    GLOBAL_REGISTRY.with(|g| g.lazy_target_setter_ref.borrow().as_ref().cloned())
}

pub fn set_lazy_current_target_setter(fn_ref: LazyCurrentTargetSetter) {
    GLOBAL_REGISTRY.with(|g| {
        *g.lazy_current_target_setter_ref.borrow_mut() = Some(Rc::new(fn_ref));
    });
}

pub fn get_lazy_current_target_setter() -> Option<Rc<LazyCurrentTargetSetter>> {
    GLOBAL_REGISTRY.with(|g| g.lazy_current_target_setter_ref.borrow().as_ref().cloned())
}

// ── Global registration functions (no DocHandle instance needed) ──────

/// One-time env injection. JS calls this during addon init (before any
/// register_* calls) so that `global::env()` works in callbacks that don't
/// receive an `Env` parameter (e.g. `EventHandler::handle_event`).
#[napi]
pub fn init_env(env: Env) -> Result<()> {
    set_env(env);
    Ok(())
}

#[napi(
    ts_args_type = "nodeType: number, constructor: { new (handle: NativeNode, document: object): object }"
)]
pub fn register_node_constructor(node_type: u32, constructor: NodeConstructor) -> Result<()> {
    insert_node_constructor(node_type, constructor);
    Ok(())
}

#[napi(
    ts_args_type = "namespace: string, tagName: string, constructor: { new (handle: NativeNode, document: object, extra?: InputDataHandle): object }"
)]
pub fn register_element_constructor(
    namespace: String,
    tag_name: String,
    constructor: ElementConstructor,
) -> Result<()> {
    let ns = Namespace::from(namespace.as_str());
    let local = LocalName::from(tag_name.to_lowercase().as_str());
    insert_element_constructor(ns, local, constructor);
    Ok(())
}

#[napi(ts_args_type = "factory: (payload: EventPayload) => Event")]
pub fn register_event_factory(factory: EventFactory) -> Result<()> {
    set_event_factory(factory);
    Ok(())
}

#[napi(ts_args_type = "dispatchFn: (target: EventTarget, event: Event) => unknown")]
pub fn register_dispatch_fn(dispatch_fn: DispatchFn) -> Result<()> {
    set_dispatch_fn(dispatch_fn);
    Ok(())
}

#[napi(ts_args_type = "getter: (event: Event) => boolean")]
pub fn register_cancel_bubble_getter(getter: CancelBubbleGetter) -> Result<()> {
    set_cancel_bubble_getter(getter);
    Ok(())
}

#[napi(ts_args_type = "getter: (event: Event) => boolean")]
pub fn register_default_prevented_getter(getter: DefaultPreventedGetter) -> Result<()> {
    set_default_prevented_getter(getter);
    Ok(())
}

#[napi(ts_args_type = "setter: (event: Event, getter: () => EventTarget | null) => void")]
pub fn register_lazy_target_setter(setter: LazyTargetSetter) -> Result<()> {
    set_lazy_target_setter(setter);
    Ok(())
}

#[napi(ts_args_type = "setter: (event: Event, getter: () => EventTarget | null, phase: number) => void")]
pub fn register_lazy_current_target_setter(setter: LazyCurrentTargetSetter) -> Result<()> {
    set_lazy_current_target_setter(setter);
    Ok(())
}
