//! Global addon-level state: JS constructor refs, event factory, napi env.
//!
//! These are registered once during addon init and never change.
//! All documents share them. Accessed only from the JS thread.
//!
//! Uses `thread_local!` because `napi_ref` and `napi_env` are raw pointers
//! that are not `Send`/`Sync`. Node.js runs JS on a single thread, so
//! thread-local storage is correct and avoids unsafe `Sync` impls.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use blitz::dom::{LocalName, Namespace};
use napi::{Env, Error, Result, Status, sys};

struct GlobalCreators {
    /// nodeType -> raw napi_ref of JS constructor function
    node_constructors: RefCell<HashMap<u32, sys::napi_ref>>,
    /// (namespace, local name) -> raw napi_ref of JS constructor function
    element_constructors: RefCell<HashMap<(Namespace, LocalName), sys::napi_ref>>,
    /// Raw napi_ref of JS event factory function
    event_factory_ref: RefCell<Option<sys::napi_ref>>,
    /// napi env (stable for addon lifetime in Node.js)
    env_raw: Cell<Option<sys::napi_env>>,
}

thread_local! {
    static GLOBAL_CREATORS: GlobalCreators = GlobalCreators {
        node_constructors: RefCell::new(HashMap::new()),
        element_constructors: RefCell::new(HashMap::new()),
        event_factory_ref: RefCell::new(None),
        env_raw: Cell::new(None),
    };
}

pub fn set_env(env: sys::napi_env) {
    GLOBAL_CREATORS.with(|g| g.env_raw.set(Some(env)));
}

pub fn env() -> Result<Env> {
    GLOBAL_CREATORS.with(|g| {
        g.env_raw
            .get()
            .map(Env::from_raw)
            .ok_or_else(|| Error::new(Status::GenericFailure, "GlobalCreators env not initialized"))
    })
}

pub fn insert_node_constructor(node_type: u32, napi_ref: sys::napi_ref) {
    GLOBAL_CREATORS.with(|g| g.node_constructors.borrow_mut().insert(node_type, napi_ref));
}

pub fn get_node_constructor(node_type: u32) -> Option<sys::napi_ref> {
    GLOBAL_CREATORS.with(|g| Some(*g.node_constructors.borrow().get(&node_type)?))
}

pub fn insert_element_constructor(ns: Namespace, local: LocalName, napi_ref: sys::napi_ref) {
    GLOBAL_CREATORS.with(|g| {
        g.element_constructors
            .borrow_mut()
            .insert((ns, local), napi_ref)
    });
}

pub fn get_element_constructor(ns: &Namespace, local: &LocalName) -> Option<sys::napi_ref> {
    GLOBAL_CREATORS.with(|g| {
        Some(
            *g.element_constructors
                .borrow()
                .get(&(ns.clone(), local.clone()))?,
        )
    })
}

pub fn set_event_factory(napi_ref: sys::napi_ref) {
    GLOBAL_CREATORS.with(|g| {
        *g.event_factory_ref.borrow_mut() = Some(napi_ref);
    });
}

pub fn get_event_factory() -> Option<sys::napi_ref> {
    GLOBAL_CREATORS.with(|g| *g.event_factory_ref.borrow())
}
