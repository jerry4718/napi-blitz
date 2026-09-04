//! Class construction: plain function constructor + prototype, ES style.
//!
//! `build_class` creates the constructor (`napi_create_function`), its
//! prototype object, registers the layer's members, wires the spec-shaped
//! `constructor` / `prototype` properties, and links the prototype chain to
//! the parent class (`Object.setPrototypeOf`).

use std::{any::TypeId, cell::RefCell, ffi::CStr, ptr};

use napi::{
    Env, Error, JsError, JsSymbol, JsValue, Property, PropertyAttributes, Result, Status,
    bindgen_prelude::{
        FnArgs, FromNapiValue, Function, FunctionCallContext, JsObjectValue, Object, ObjectRef,
        This, ToNapiValue,
    },
    check_status, sys,
};

use crate::{
    layer::{EmitOwn, ExtendLayer, LayerAccessors, LayerArgs, LayerBuild, LayerChain},
    own::attach_registry,
    registry,
};

/// Build and register a layer's JS class. Idempotent: repeated calls for an
/// already-registered class are no-ops. The parent class is built first
/// through `ensure_class_built` (`RootLayer` terminates the chain), so the
/// prototype link below always finds the parent's handles.
pub fn build_class<T: ExtendLayer + LayerAccessors>(env: &Env) -> Result<()>
where
    <T as LayerBuild>::Args: FromNapiValue,
{
    if registry::contains(TypeId::of::<T>()) {
        return Ok(());
    }
    <T::Parent as registry::HasClassRef>::ensure_class_built(env)?;
    let mut proto = Object::new(env)?;
    let mut ctor = create_constructor::<T>(env)?;
    T::define_accessors(env, &mut proto)?;
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
    // WARNING: calling this with a hand-assembled `LayerChain` hurts
    // readability. Always prefer the `from_chain!` macro; call this directly
    // only when the chain must be built programmatically.
    let (_, proto) = registry::require(env, TypeId::of::<T>())?;
    let mut this = object_create(env, &proto)?;
    attach_registry::<T>(&mut this)?;
    T::populate_chain(env, &this, chain)?;
    Ok(this)
}

// ── member-definition helpers used by define_members implementations ─────

/// Run a JS-invoked callback, turning a panic into a JS-visible error
/// instead of letting it unwind across the `extern "C"` trampoline into
/// `rtabort` (which kills the process without even printing the message).
fn catch_panic<R>(f: impl FnOnce() -> Result<R>) -> Result<R> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|payload| {
        Err(Error::new(
            Status::GenericFailure,
            format!("rust panic: {}", panic_payload_message(&payload)),
        ))
    })
}

fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_owned()
    }
}

/// A getter on the prototype (non-enumerable, configurable - WebIDL shape).
/// The closure's `this` arrives as an `Object`.
pub fn define_getter<R, F>(proto: &mut Object, name: &str, getter: F) -> Result<()>
where
    R: ToNapiValue,
    F: 'static + Fn(Env, This) -> Result<R>,
{
    let prop = Property::new()
        .with_utf8_name(name)?
        .with_getter_closure(move |env, this| catch_panic(|| getter(env, this)))
        .with_property_attributes(PropertyAttributes::Configurable);
    proto.define_properties(&[prop])?;
    Ok(())
}

/// A getter keyed by a well-known symbol, so protocol members land on the
/// prototype.
pub fn define_symbol_getter<R, F>(
    env: &Env,
    proto: &mut Object,
    description: &str,
    getter: F,
) -> Result<()>
where
    R: ToNapiValue,
    F: 'static + Fn(Env, This) -> Result<R>,
{
    let sym = wellknown_symbol(env, description)?;
    let prop = Property::new()
        .with_name(env, sym)?
        .with_getter_closure(move |env, this| catch_panic(|| getter(env, this)))
        .with_property_attributes(PropertyAttributes::Configurable);
    proto.define_properties(&[prop])?;
    Ok(())
}

/// `Symbol.iterator` / `Symbol.asyncIterator` as a method returning an
/// iterator object driven by `next_item(env, this, index)`.
pub fn define_generator<I, F>(
    env: &Env,
    proto: &mut Object,
    description: &str,
    next_item: F,
) -> Result<()>
where
    I: ToNapiValue + 'static,
    F: 'static + Clone + Fn(Env, This, u32) -> Result<Option<I>>,
{
    let sym = wellknown_symbol(env, description)?;
    let iter_fn: Function<'_, (), ObjectRef<false>> = env.create_function_from_closure(
        "[Symbol.iterator]",
        move |ctx: FunctionCallContext| -> Result<ObjectRef<false>> {
            catch_panic(|| {
                let this: This = ctx.this()?;
                let set_ref: ObjectRef<false> =
                    unsafe { ObjectRef::from_napi_value(ctx.env.raw(), JsValue::raw(&this)) }?;
                let idx = std::rc::Rc::new(std::cell::Cell::new(0u32));
                let next_item = next_item.clone();
                let next: Function<'_, (), Object> = ctx.env.create_function_from_closure(
                    "next",
                    move |next_ctx: FunctionCallContext| -> Result<Object> {
                        catch_panic(|| {
                            let env = next_ctx.env;
                            let i = idx.get();
                            let raw = set_ref.get_value(env)?;
                            let this_obj = raw;
                            let result = match next_item(*env, this_obj.into(), i)? {
                                Some(value) => {
                                    idx.set(i + 1);
                                    let mut obj = Object::new(env)?;
                                    obj.set("value", value)?;
                                    obj.set("done", false)?;
                                    obj
                                }
                                None => {
                                    let mut obj = Object::new(env)?;
                                    obj.set("done", true)?;
                                    obj
                                }
                            };
                            Ok(result)
                        })
                    },
                )?;
                let mut iter = Object::new(ctx.env)?;
                iter.set_named_property("next", next)?;
                unsafe { ObjectRef::from_napi_value(ctx.env.raw(), JsValue::raw(&iter)) }
            })
        },
    )?;
    let prop = Property::new()
        .with_name(env, sym)?
        .with_value(&iter_fn)
        .with_property_attributes(PropertyAttributes::Writable | PropertyAttributes::Configurable);
    proto.define_properties(&[prop])?;
    Ok(())
}

/// Resolve a well-known symbol by its description through the global
/// `Symbol` constructor. `get_named_property_unchecked` because the
/// `Symbol` constructor is a function, not a plain object.
fn wellknown_symbol<'env>(env: &'env Env, description: &str) -> Result<JsSymbol<'env>> {
    let global = env.get_global()?;
    let sym_ctor: Object = global.get_named_property_unchecked("Symbol")?;
    sym_ctor.get_named_property(description)
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
        .with_setter_closure(move |env, this, v: V| catch_panic(|| setter(env, this, v)))
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
        .with_getter_closure(move |env, _this: This| catch_panic(|| getter(env)))
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
        .with_setter_closure(move |env, _this: This, v: V| catch_panic(|| setter(env, v)))
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
    let f: Function<'_, (), Return> =
        env.create_function_from_closure(name, move |ctx| catch_panic(|| method(ctx)))?;
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
    let f: Function<'_, (), Return> =
        env.create_function_from_closure(name, move |ctx| catch_panic(|| method(ctx)))?;
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
    match catch_panic(|| construct::<T>(env, info)) {
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
        c"setPrototypeOf",
        &[JsValue::raw(obj), JsValue::raw(proto)],
    )?;
    Ok(())
}

/// `Object.create(proto)`, with the `Object` constructor and its `create`
/// method resolved once per env: node wraps and event factories go through
/// here on every dispatch.
fn object_create<'env>(env: &'env Env, proto: &Object<'env>) -> Result<Object<'env>> {
    let env_raw = env.raw();
    let (ctor, create) = object_statics(env)?;
    let argv = [JsValue::raw(proto)];
    let mut result = ptr::null_mut();
    check_status!(
        unsafe {
            sys::napi_call_function(
                env_raw,
                JsValue::raw(&ctor),
                JsValue::raw(&create),
                argv.len(),
                argv.as_ptr(),
                &mut result,
            )
        },
        "call Object.create failed"
    )?;
    unsafe { Object::from_napi_value(env_raw, result) }
}

/// Per-env handles of the `Object` constructor and its `create` method.
struct ObjectStatics {
    env: sys::napi_env,
    object_ctor: ObjectRef<false>,
    create: ObjectRef<false>,
}

thread_local! {
    /// thread_local because napi_ref handles belong to the thread that
    /// created them; the env check rebuilds the entry if a thread ever
    /// hosts more than one env. The refs are never deleted - they root
    /// process-lifetime builtins, the same trade the class registry makes.
    static OBJECT_STATICS: RefCell<Option<ObjectStatics>> = const { RefCell::new(None) };
}

/// Resolve (Object constructor, Object.create) for `env`, from the cache
/// when it matches.
fn object_statics(env: &Env) -> Result<(Object<'_>, Object<'_>)> {
    let env_raw = env.raw();
    OBJECT_STATICS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.as_ref().is_none_or(|s| s.env != env_raw) {
            *cache = Some(resolve_object_statics(env)?);
        }
        let entry = cache.as_ref().unwrap();
        Ok((
            entry.object_ctor.get_value(env)?,
            entry.create.get_value(env)?,
        ))
    })
}

fn resolve_object_statics(env: &Env) -> Result<ObjectStatics> {
    let env_raw = env.raw();
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
    let mut create = ptr::null_mut();
    check_status!(
        unsafe {
            sys::napi_get_named_property(env_raw, object_ctor, c"create".as_ptr(), &mut create)
        },
        "get Object.create failed"
    )?;
    unsafe {
        Ok(ObjectStatics {
            env: env_raw,
            object_ctor: ObjectRef::from_napi_value(env_raw, object_ctor)?,
            create: ObjectRef::from_napi_value(env_raw, create)?,
        })
    }
}

fn call_object_static(
    env_raw: sys::napi_env,
    name: &CStr,
    argv: &[sys::napi_value],
) -> Result<sys::napi_value> {
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
        unsafe { sys::napi_get_named_property(env_raw, object_ctor, name.as_ptr(), &mut method) },
        "get Object.{} failed",
        name.to_string_lossy()
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
        "call Object.{} failed",
        name.to_string_lossy()
    )?;
    Ok(result)
}
