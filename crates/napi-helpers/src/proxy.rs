//! Helpers for constructing JS `Proxy` objects from Rust.

use napi::{
    Env, Result,
    bindgen_prelude::{FromNapiValue, JsObjectValue, JsValue, Object},
    check_status, sys,
};

use crate::anything::Anything;

/// Construct a JS `Proxy` around an empty target object with the given
/// handler, via the global `Proxy` constructor.
pub fn new_proxy(env: &Env, handler: &Object) -> Result<Anything> {
    let global = env.get_global()?;
    let proxy_ctor = global.get_named_property_unchecked::<Object>("Proxy")?;
    let target = Object::new(env)?;
    let mut argv = [JsValue::raw(&target), JsValue::raw(handler)];
    let mut result = std::ptr::null_mut();
    check_status!(unsafe {
        sys::napi_new_instance(
            env.raw(),
            JsValue::raw(&proxy_ctor),
            argv.len(),
            argv.as_mut_ptr(),
            &mut result,
        )
    })?;
    unsafe { Anything::from_napi_value(env.raw(), result) }
}
