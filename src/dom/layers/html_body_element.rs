//! The `HTMLBodyElement` layer — `<body>` elements. All data lives in the
//! `Node` layer; this layer only contributes the window-forwarding
//! `on<event>` accessors to the prototype chain (mirrors
//! `blitz-vibey-script`'s `BodyLayer`).

use std::any::TypeId;

use napi::bindgen_prelude::{FnArgs, FromNapiValue, Function, This};
use napi::{Env, Error, Result, bindgen_prelude::Object};
use napi_helpers::anything::Anything;
use napi_helpers::inherits::{
    Constructed, Super, define_getter, define_setter, proc::layer, require, with_own,
};

use crate::{
    dom::layers::{html_element::HTMLElementLayer, node::NodeLayer},
    events::base::EventTargetLayer,
    helpers::resolve_window,
};

/// Event handler IDL attributes on `<body>` elements that are aliases for
/// the corresponding window event handlers. The whole set is NOT
/// dispatched: each accessor forwards to the window's attribute listener,
/// and the window lifecycle currently has no source for these events (the
/// window only receives `close` / `closed` and DOM events that bubble out
/// of the document, none of which are in this set).
const WINDOW_REFLECTING_BODY_EVENTS: &[&str] = &[
    "afterprint",         // NOT dispatched — no window lifecycle source
    "beforeprint",        // NOT dispatched — no window lifecycle source
    "beforeunload",       // NOT dispatched — no window lifecycle source
    "blur",               // NOT dispatched — forwards to window; focus/blur does not bubble
    "error",              // NOT dispatched — no window lifecycle source
    "focus",              // NOT dispatched — forwards to window; focus/blur does not bubble
    "hashchange",         // NOT dispatched — no window lifecycle source
    "languagechange",     // NOT dispatched — no window lifecycle source
    "load",               // NOT dispatched — no window lifecycle source
    "message",            // NOT dispatched — no window lifecycle source
    "messageerror",       // NOT dispatched — no window lifecycle source
    "offline",            // NOT dispatched — no window lifecycle source
    "online",             // NOT dispatched — no window lifecycle source
    "pagehide",           // NOT dispatched — no window lifecycle source
    "pageshow",           // NOT dispatched — no window lifecycle source
    "popstate",           // NOT dispatched — no window lifecycle source
    "rejectionhandled",   // NOT dispatched — no window lifecycle source
    "resize",             // NOT dispatched — no window lifecycle source
    "scroll",             // NOT dispatched — no window lifecycle source (no Scroll event)
    "storage",            // NOT dispatched — no window lifecycle source
    "unhandledrejection", // NOT dispatched — no window lifecycle source
    "unload",             // NOT dispatched — no window lifecycle source
];

/// Own block of the `HTMLBodyElement` class.
#[layer]
pub struct HTMLBodyElementLayer {}

#[layer(js_name = "HTMLBodyElement")]
impl HTMLBodyElementLayer {
    #[layer(parent)]
    type Parent = HTMLElementLayer;

    #[layer(constructor)]
    fn build(_sup: Super<HTMLElementLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "HTMLBodyElement cannot be constructed directly; create via document APIs",
        ))
    }
}

/// Define the window-reflecting `on<event>` IDL-style attributes: reading
/// one forwards to the window's attribute listener for the type, assigning
/// a callable installs it there, assigning anything else removes it.
pub(crate) fn define_window_reflecting_attributes(proto: &mut Object) -> Result<()> {
    for event_type in WINDOW_REFLECTING_BODY_EVENTS {
        let name = format!("on{event_type}");
        define_getter(
            proto,
            &name,
            move |env: Env, this: This| -> Result<Option<Anything>> {
                let shared_doc = with_own::<NodeLayer, _>(&this, |n| n.shared_doc.clone())?;
                let Some(window) = resolve_window(&shared_doc, &env) else {
                    return Ok(None);
                };
                with_own::<EventTargetLayer, _>(&window, |w| w.attribute_listener(&env, event_type))
            },
        )?;
        define_setter(
            proto,
            &name,
            move |env: Env, this: This, value: Anything| -> Result<()> {
                let shared_doc = with_own::<NodeLayer, _>(&this, |n| n.shared_doc.clone())?;
                let Some(window) = resolve_window(&shared_doc, &env) else {
                    return Ok(());
                };
                with_own::<EventTargetLayer, _>(&window, |w| match value {
                    Anything::Function(reference) => {
                        let handler = unsafe {
                            Function::<FnArgs<(Anything,)>, Anything>::from_napi_value(
                                env.raw(),
                                reference.raw_value(&env)?,
                            )?
                        };
                        w.set_attribute_listener(&env, &window, event_type, handler)
                    }
                    _ => w.remove_attribute_listener(&env, &window, event_type),
                })?
            },
        )?;
    }
    Ok(())
}

/// Define the window-reflecting attributes on `HTMLBodyElement.prototype`.
/// Called once from the JS bootstrap, like `defineNodeOnEventAttributes`.
#[napi]
pub fn define_html_body_event_attributes(env: &Env) -> Result<()> {
    let (_, mut proto) = require(env, TypeId::of::<HTMLBodyElementLayer>())?;
    define_window_reflecting_attributes(&mut proto)
}
