//! Class construction: plain function constructor + prototype, ES style.
//!
//! `build_class` creates the constructor (`napi_create_function`), its
//! prototype object, registers the layer's members, wires the spec-shaped
//! `constructor` / `prototype` properties, and links the prototype chain to
//! the parent class (`Object.setPrototypeOf`).

use std::{any::TypeId, ffi::CString, ptr};

use napi::{
    Env, Error, JsError, JsValue, Property, PropertyAttributes, Result, Status,
    bindgen_prelude::{
        FnArgs, FromNapiValue, Function, FunctionCallContext, JsObjectValue, Object, This,
        ToNapiValue,
    },
    check_status, sys,
};

use crate::{
    layer::{EmitOwn, ExtendLayer, LayerArgs, LayerBuild, LayerChain},
    own::attach_registry,
    registry,
};

/// Build and register a layer's JS class. Idempotent: repeated calls for an
/// already-registered class are no-ops.
pub fn build_class<T: ExtendLayer>(env: &Env) -> Result<()>
where
    <T as LayerBuild>::Args: FromNapiValue,
{
    if registry::contains(TypeId::of::<T>()) {
        return Ok(());
    }
    let mut proto = Object::new(env)?;
    let mut ctor = create_constructor::<T>(env)?;
    T::define_members(env, &mut proto, &mut ctor)?;
    define_constructor_props(&mut ctor, &mut proto)?;
    registry::insert(env, TypeId::of::<T>(), &ctor, &proto)?;
    link_prototype::<T>(env)?;
    Ok(())
}

fn create_constructor<T: ExtendLayer>(env: &Env) -> Result<Object<'_>>
where
    <T as LayerBuild>::Args: FromNapiValue,
{
    let env_raw = env.raw();
    let mut raw = ptr::null_mut();
    check_status!(unsafe {
        sys::napi_create_function(
            env_raw,
            T::CLASS_NAME.as_ptr().cast(),
            T::CLASS_NAME.len() as isize,
            Some(constructor_callback::<T>),
            ptr::null_mut(),
            &mut raw,
        )
    })?;
    unsafe { Object::from_napi_value(env_raw, raw) }
}

/// Link `Child.prototype.__proto__ = Parent.prototype` and the constructor
/// chain `Child.__proto__ = Parent`. A `RootLayer` parent is a no-op.
pub fn link_prototype<T: ExtendLayer>(env: &Env) -> Result<()> {
    let Some((parent_ctor, parent_proto)) =
        <T::Parent as registry::HasClassRef>::class_handles(env)?
    else {
        return Ok(());
    };
    let (ctor, proto) = registry::require(env, TypeId::of::<T>())?;
    set_prototype(env, &proto, &parent_proto)?;
    set_prototype(env, &ctor, &parent_ctor)?;
    Ok(())
}

/// Build an instance on the Rust side from an existing data chain:
/// `Object.create(proto)` followed by the recursive own-block write,
/// parent layers first.
pub fn new_from_chain<T: ExtendLayer>(env: &Env, chain: LayerChain<T>) -> Result<Object<'_>> {
    let (_, proto) = registry::require(env, TypeId::of::<T>())?;
    let mut this = object_create(env, &proto)?;
    attach_registry::<T>(&mut this)?;
    T::populate_chain(env, &this, chain)?;
    Ok(this)
}

// ── member-definition helpers used by define_members implementations ─────

/// A getter on the prototype (non-enumerable, configurable - WebIDL shape).
/// The closure's `this` arrives as an `Object`.
pub fn define_getter<R, F>(proto: &mut Object, name: &str, getter: F) -> Result<()>
where
    R: ToNapiValue,
    F: 'static + Fn(Env, This) -> Result<R>,
{
    let prop = Property::new()
        .with_utf8_name(name)?
        .with_getter_closure(getter)
        .with_property_attributes(PropertyAttributes::Configurable);
    proto.define_properties(&[prop])?;
    Ok(())
}

/// A setter on the prototype (non-enumerable, configurable - WebIDL shape).
/// The closure's `this` arrives as an `Object`; the assigned value as `V`.
pub fn define_setter<V, F>(proto: &mut Object, name: &str, setter: F) -> Result<()>
where
    V: FromNapiValue,
    F: 'static + Fn(Env, This, V) -> Result<()>,
{
    let prop = Property::new()
        .with_utf8_name(name)?
        .with_setter_closure(setter)
        .with_property_attributes(PropertyAttributes::Configurable);
    proto.define_properties(&[prop])?;
    Ok(())
}

/// A getter on the constructor itself (static; non-enumerable, configurable).
pub fn define_static_getter<R, F>(ctor: &mut Object, name: &str, getter: F) -> Result<()>
where
    R: ToNapiValue,
    F: 'static + Fn(Env) -> Result<R>,
{
    let prop = Property::new()
        .with_utf8_name(name)?
        .with_getter_closure(move |env, _this: This| getter(env))
        .with_property_attributes(PropertyAttributes::Configurable);
    ctor.define_properties(&[prop])?;
    Ok(())
}

/// A setter on the constructor itself (static; non-enumerable, configurable).
pub fn define_static_setter<V, F>(ctor: &mut Object, name: &str, setter: F) -> Result<()>
where
    V: FromNapiValue,
    F: 'static + Fn(Env, V) -> Result<()>,
{
    let prop = Property::new()
        .with_utf8_name(name)?
        .with_setter_closure(move |env, _this: This, v: V| setter(env, v))
        .with_property_attributes(PropertyAttributes::Configurable);
    ctor.define_properties(&[prop])?;
    Ok(())
}

/// A method on the prototype (non-enumerable, writable, configurable).
pub fn define_method<Return, F>(env: &Env, proto: &mut Object, name: &str, method: F) -> Result<()>
where
    Return: ToNapiValue,
    F: 'static + Fn(FunctionCallContext) -> Result<Return>,
{
    let f: Function<'_, (), Return> = env.create_function_from_closure(name, method)?;
    let prop = Property::new()
        .with_utf8_name(name)?
        .with_value(&f)
        .with_property_attributes(PropertyAttributes::Writable | PropertyAttributes::Configurable);
    proto.define_properties(&[prop])?;
    Ok(())
}

/// A read-only constant on the constructor itself. The WebIDL interface
/// shape: {writable: false, enumerable: false, configurable: false}.
pub fn define_static_value<V: ToNapiValue>(
    env: &Env,
    ctor: &mut Object,
    name: &str,
    value: V,
) -> Result<()> {
    let prop = Property::new()
        .with_utf8_name(name)?
        .with_napi_value(env, value)?
        .with_property_attributes(PropertyAttributes::Default);
    ctor.define_properties(&[prop])?;
    Ok(())
}

/// A static method on the constructor itself: a callable function value
/// (writable, configurable) matching the `static foo(): T` TS shape.
pub fn define_static_method<Return, F>(
    env: &Env,
    ctor: &mut Object,
    name: &str,
    method: F,
) -> Result<()>
where
    Return: ToNapiValue,
    F: 'static + Fn(FunctionCallContext) -> Result<Return>,
{
    let f: Function<'_, (), Return> = env.create_function_from_closure(name, method)?;
    let prop = Property::new()
        .with_utf8_name(name)?
        .with_value(&f)
        .with_property_attributes(PropertyAttributes::Writable | PropertyAttributes::Configurable);
    ctor.define_properties(&[prop])?;
    Ok(())
}

// ── constructor callback ─────────────────────────────────────────────────

unsafe extern "C" fn constructor_callback<T: ExtendLayer>(
    env: sys::napi_env,
    info: sys::napi_callback_info,
) -> sys::napi_value
where
    <T as LayerBuild>::Args: FromNapiValue,
{
    match construct::<T>(env, info) {
        // Returning undefined lets `new` fall back to the engine-created `this`.
        Ok(()) => ptr::null_mut(),
        Err(e) => {
            unsafe { JsError::from(e).throw_into(env) };
            ptr::null_mut()
        }
    }
}

fn construct<T: ExtendLayer>(env_raw: sys::napi_env, info: sys::napi_callback_info) -> Result<()>
where
    <T as LayerBuild>::Args: FromNapiValue,
{
    let mut new_target = ptr::null_mut();
    check_status!(unsafe { sys::napi_get_new_target(env_raw, info, &mut new_target) })?;
    if new_target.is_null() {
        return Err(Error::new(
            Status::GenericFailure,
            format!("constructor {} requires 'new'", T::CLASS_NAME),
        ));
    }

    const MAX_ARGS: usize = 16;
    // First call only asks for the real argument count.
    let mut actual_argc: usize = 0;
    check_status!(unsafe {
        sys::napi_get_cb_info(
            env_raw,
            info,
            &mut actual_argc,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        )
    })?;
    if actual_argc > MAX_ARGS {
        return Err(Error::new(
            Status::GenericFailure,
            format!(
                "constructor {} takes at most {MAX_ARGS} arguments, got {actual_argc}",
                T::CLASS_NAME
            ),
        ));
    }

    let mut argc: usize = actual_argc;
    let mut argv = [ptr::null_mut(); MAX_ARGS];
    let mut this = ptr::null_mut();
    check_status!(unsafe {
        sys::napi_get_cb_info(
            env_raw,
            info,
            &mut argc,
            argv.as_mut_ptr(),
            &mut this,
            ptr::null_mut(),
        )
    })?;
    if this.is_null() {
        return Err(Error::new(
            Status::GenericFailure,
            format!("constructor {} called without a receiver", T::CLASS_NAME),
        ));
    }

    let env = Env::from(env_raw);
    let mut this_obj = unsafe { Object::from_napi_value(env_raw, this) }?;
    attach_registry::<T>(&mut this_obj)?;
    // Size the array to the layer's `ARITY` so omitted trailing arguments
    // stay `undefined`; napi's `Option: FromNapiValue` parses those as
    // `None`.
    let arity = <T::Args as LayerArgs>::ARITY;
    let mut args_array = ptr::null_mut();
    check_status!(unsafe { sys::napi_create_array_with_length(env_raw, arity, &mut args_array) })?;
    for (i, v) in argv[..argc.min(arity)].iter().enumerate() {
        check_status!(unsafe { sys::napi_set_element(env_raw, args_array, i as u32, *v) })?;
    }
    let args = unsafe { <T::Args as FromNapiValue>::from_napi_value(env_raw, args_array) }?;
    T::emit_own(&env, &this_obj, FnArgs::from(args))
}

// ── spec-shaped constructor/prototype wiring ─────────────────────────────

fn define_constructor_props(ctor: &mut Object, proto: &mut Object) -> Result<()> {
    // proto.constructor = ctor, {writable: true, enumerable: false,
    // configurable: true} - the ES spec shape.
    let ctor_prop = Property::new()
        .with_utf8_name("constructor")?
        .with_value(ctor)
        .with_property_attributes(PropertyAttributes::Writable | PropertyAttributes::Configurable);
    proto.define_properties(&[ctor_prop])?;

    // ctor.prototype = proto, {writable: true, enumerable: false,
    // configurable: false} - the ES spec shape.
    let proto_prop = Property::new()
        .with_utf8_name("prototype")?
        .with_value(proto)
        .with_property_attributes(PropertyAttributes::Writable);
    ctor.define_properties(&[proto_prop])?;
    Ok(())
}

// ── Object static-method plumbing ────────────────────────────────────────

/// `Object.setPrototypeOf(obj, proto)`.
fn set_prototype(env: &Env, obj: &Object, proto: &Object) -> Result<()> {
    call_object_static(
        env.raw(),
        "setPrototypeOf",
        &[JsValue::raw(obj), JsValue::raw(proto)],
    )?;
    Ok(())
}

/// `Object.create(proto)`.
fn object_create<'env>(env: &'env Env, proto: &Object<'env>) -> Result<Object<'env>> {
    let env_raw = env.raw();
    let raw = call_object_static(env_raw, "create", &[JsValue::raw(proto)])?;
    unsafe { Object::from_napi_value(env_raw, raw) }
}

fn call_object_static(
    env_raw: sys::napi_env,
    name: &str,
    argv: &[sys::napi_value],
) -> Result<sys::napi_value> {
    let name_c = CString::new(name)?;
    let mut global = ptr::null_mut();
    check_status!(
        unsafe { sys::napi_get_global(env_raw, &mut global) },
        "get global failed"
    )?;
    let mut object_ctor = ptr::null_mut();
    check_status!(
        unsafe {
            sys::napi_get_named_property(env_raw, global, c"Object".as_ptr(), &mut object_ctor)
        },
        "get Object constructor failed"
    )?;
    let mut method = ptr::null_mut();
    check_status!(
        unsafe { sys::napi_get_named_property(env_raw, object_ctor, name_c.as_ptr(), &mut method) },
        "get Object.{name} failed"
    )?;
    let mut result = ptr::null_mut();
    check_status!(
        unsafe {
            sys::napi_call_function(
                env_raw,
                object_ctor,
                method,
                argv.len(),
                argv.as_ptr(),
                &mut result,
            )
        },
        "call Object.{name} failed"
    )?;
    Ok(result)
}
