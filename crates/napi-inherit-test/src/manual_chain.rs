//! A three-layer test chain exercising the whole infrastructure:
//! `InheritBase -> InheritMid -> InheritLeaf`.
//!
//! Each layer records the parent data it reads after `sup.call`, so the JS
//! tests can assert the super ordering from the outside. What is visible
//! before `sup.call` is not testable here on purpose: `build` receives no
//! instance, so the type system itself forbids touching `this` before
//! super.

use napi::{
    Env, Error, JsValue, Result, Status,
    bindgen_prelude::{FromNapiValue, JsObjectValue, Object, Unknown},
};
use std::{
    any::TypeId,
    sync::atomic::{AtomicU32, Ordering},
};

use napi_inherit::{
    class::{
        build_class, define_getter, define_method, define_setter, define_static_getter,
        define_static_method, define_static_setter, define_static_value, new_from_chain,
    },
    layer::{Constructed, LayerChain, LayerComposed, RootLayer, Super},
    own::{with_own, with_own_mut},
    registry,
};

/// Backing store for the static `counter` accessor pair.
static SHARED_COUNTER: AtomicU32 = AtomicU32::new(10);

fn arg_u32(env: &Env, args: &[Unknown], idx: usize) -> Result<u32> {
    let Some(v) = args.get(idx) else {
        return Err(Error::new(
            Status::GenericFailure,
            format!("missing arg {idx}"),
        ));
    };
    let n = unsafe { f64::from_napi_value(env.raw(), JsValue::raw(v)) }?;
    Ok(n as u32)
}

// ── InheritBase ──────────────────────────────────────────────────────────

pub struct BaseLayer {
    pub base_value: u32,
}

impl LayerComposed for BaseLayer {
    type Parent = RootLayer;
    const CLASS_NAME: &'static str = "InheritBase";

    fn build<'env>(
        env: &'env Env,
        args: &[Unknown<'env>],
        sup: Super<'_, 'env, RootLayer>,
    ) -> Result<Constructed<Self>> {
        let base_value = arg_u32(env, args, 0)?;
        let done = sup.call(&[])?;
        Ok(Constructed::new(done, Self { base_value }))
    }

    fn define_members(env: &Env, proto: &mut Object, ctor: &mut Object) -> Result<()> {
        define_getter(proto, "baseValue", |_ctx, this| {
            with_own::<BaseLayer, _>(&this, |d| d.base_value)
        })?;
        define_method(env, proto, "baseGreet", |ctx| {
            let this: Object = ctx.this()?;
            let v = with_own::<BaseLayer, _>(&this, |d| d.base_value)?;
            Ok(format!("base:{v}"))
        })?;
        define_method(env, proto, "bumpBase", |ctx| {
            let this: Object = ctx.this()?;
            let delta: f64 = ctx.get(0)?;
            with_own_mut::<BaseLayer, _>(&this, |d| {
                d.base_value += delta as u32;
                d.base_value
            })
        })?;
        define_static_value(env, ctor, "BASE_CONST", 1u32)?;
        define_setter(proto, "baseValue", |_env, this, value: u32| {
            with_own_mut::<BaseLayer, _>(&this, |d| d.base_value = value)
        })?;
        define_static_getter(ctor, "counter", |_env| {
            Ok(SHARED_COUNTER.load(Ordering::Relaxed))
        })?;
        define_static_setter(ctor, "counter", |_env, value: u32| {
            SHARED_COUNTER.store(value, Ordering::Relaxed);
            Ok(())
        })?;
        define_getter(proto, "checkedValue", |_env, this| {
            with_own::<BaseLayer, _>(&this, |d| {
                if d.base_value == 0 {
                    Err(Error::new(Status::GenericFailure, "base_value is zero"))
                } else {
                    Ok(d.base_value)
                }
            })?
        })?;
        define_setter(proto, "checkedValue", |_env, this, value: u32| {
            with_own_mut::<BaseLayer, _>(&this, |d| {
                if value == 0 {
                    return Err(Error::new(Status::GenericFailure, "cannot set to zero"));
                }
                d.base_value = value;
                Ok(())
            })?
        })?;
        define_static_getter(ctor, "checkedCounter", |_env| {
            let v = SHARED_COUNTER.load(Ordering::Relaxed);
            if v == 0 {
                Err(Error::new(Status::GenericFailure, "counter is zero"))
            } else {
                Ok(v)
            }
        })?;
        define_static_setter(ctor, "checkedCounter", |_env, value: u32| {
            if value == 0 {
                return Err(Error::new(
                    Status::GenericFailure,
                    "cannot set counter to zero",
                ));
            }
            SHARED_COUNTER.store(value, Ordering::Relaxed);
            Ok(())
        })?;
        define_method(env, proto, "guardedGreet", |ctx| {
            let this: Object = ctx.this()?;
            with_own::<BaseLayer, _>(&this, |d| {
                if d.base_value == 0 {
                    Err(Error::new(Status::GenericFailure, "base is zero"))
                } else {
                    Ok(format!("base:{}", d.base_value))
                }
            })?
        })?;
        define_method(env, proto, "guard", |ctx| {
            let this: Object = ctx.this()?;
            let v: u32 = ctx.get(0)?;
            if v == 0 {
                return Err(Error::new(Status::GenericFailure, "guard rejects zero"));
            }
            let base = with_own::<BaseLayer, _>(&this, |d| d.base_value)?;
            Ok(format!("guard:{v}/base:{base}"))
        })?;
        define_static_method(env, ctor, "staticGuard", |ctx| {
            let v: u32 = ctx.get(0)?;
            if v == 0 {
                return Err(Error::new(
                    Status::GenericFailure,
                    "static guard rejects zero",
                ));
            }
            Ok(v * 2)
        })?;
        Ok(())
    }
}

// ── InheritMid ───────────────────────────────────────────────────────────

pub struct MidLayer {
    pub mid_value: u32,
    /// The parent data read after `sup.call`.
    pub base_seen_after_super: u32,
}

impl LayerComposed for MidLayer {
    type Parent = BaseLayer;
    const CLASS_NAME: &'static str = "InheritMid";

    fn build<'env>(
        env: &'env Env,
        args: &[Unknown<'env>],
        sup: Super<'_, 'env, BaseLayer>,
    ) -> Result<Constructed<Self>> {
        let mid_value = arg_u32(env, args, 1)?;
        let done = sup.call(args)?;
        let this = done.this();
        let base_seen_after_super = with_own::<BaseLayer, _>(this, |d| d.base_value)?;
        Ok(Constructed::new(
            done,
            Self {
                mid_value,
                base_seen_after_super,
            },
        ))
    }

    fn define_members(env: &Env, proto: &mut Object, ctor: &mut Object) -> Result<()> {
        define_getter(proto, "midValue", |_ctx, this| {
            with_own::<MidLayer, _>(&this, |d| d.mid_value)
        })?;
        define_getter(proto, "baseSeenAfterSuper", |_ctx, this| {
            with_own::<MidLayer, _>(&this, |d| d.base_seen_after_super)
        })?;
        define_method(env, proto, "midDescribe", |ctx| {
            let this: Object = ctx.this()?;
            let mid = with_own::<MidLayer, _>(&this, |d| d.mid_value)?;
            let base = with_own::<BaseLayer, _>(&this, |d| d.base_value)?;
            Ok(format!("mid:{mid}/base:{base}"))
        })?;
        define_static_value(env, ctor, "MID_CONST", 2u32)?;
        Ok(())
    }
}

// ── InheritLeaf ──────────────────────────────────────────────────────────

pub struct LeafLayer {
    pub leaf_value: u32,
    pub mid_seen_after_super: u32,
}

impl LayerComposed for LeafLayer {
    type Parent = MidLayer;
    const CLASS_NAME: &'static str = "InheritLeaf";

    fn build<'env>(
        env: &'env Env,
        args: &[Unknown<'env>],
        sup: Super<'_, 'env, MidLayer>,
    ) -> Result<Constructed<Self>> {
        let leaf_value = arg_u32(env, args, 2)?;
        let done = sup.call(args)?;
        let this = done.this();
        let mid_seen_after_super = with_own::<MidLayer, _>(this, |d| d.mid_value)?;
        if leaf_value > 100 {
            return Err(Error::new(Status::GenericFailure, "leaf_value too large"));
        }
        Ok(Constructed::new(
            done,
            Self {
                leaf_value,
                mid_seen_after_super,
            },
        ))
    }

    fn define_members(env: &Env, proto: &mut Object, ctor: &mut Object) -> Result<()> {
        define_getter(proto, "leafValue", |_ctx, this| {
            with_own::<LeafLayer, _>(&this, |d| d.leaf_value)
        })?;
        define_getter(proto, "midSeenAfterSuper", |_ctx, this| {
            with_own::<LeafLayer, _>(&this, |d| d.mid_seen_after_super)
        })?;
        define_method(env, proto, "leafShout", |ctx| {
            let this: Object = ctx.this()?;
            let leaf = with_own::<LeafLayer, _>(&this, |d| d.leaf_value)?;
            let mid = with_own::<MidLayer, _>(&this, |d| d.mid_value)?;
            let base = with_own::<BaseLayer, _>(&this, |d| d.base_value)?;
            Ok(format!("leaf:{leaf}+mid:{mid}+base:{base}"))
        })?;
        define_static_value(env, ctor, "LEAF_CONST", 3u32)?;
        Ok(())
    }
}

// ── Exports ──────────────────────────────────────────────────────────────

/// Build the three classes and hand the constructors to JS. Idempotent via
/// the registry; the returned constructors are the same objects on repeat
/// calls.
#[napi]
pub fn build_inherit_test_classes<'env>(env: &'env Env) -> Result<Object<'env>> {
    build_class::<BaseLayer>(env)?;
    build_class::<MidLayer>(env)?;
    build_class::<LeafLayer>(env)?;

    let (base_ctor, _) = registry::require(env, TypeId::of::<BaseLayer>())?;
    let (mid_ctor, _) = registry::require(env, TypeId::of::<MidLayer>())?;
    let (leaf_ctor, _) = registry::require(env, TypeId::of::<LeafLayer>())?;

    let mut out = Object::new(env)?;
    out.set_named_property("Base", base_ctor)?;
    out.set_named_property("Mid", mid_ctor)?;
    out.set_named_property("Leaf", leaf_ctor)?;
    Ok(out)
}

/// Build an `InheritLeaf` instance from a Rust-side data chain, bypassing
/// the JS `new` path entirely.
#[napi]
pub fn make_inherit_leaf_from_chain<'env>(env: &'env Env) -> Result<Object<'env>> {
    let chain = LayerChain {
        parent: LayerChain {
            parent: LayerChain {
                parent: (),
                own: BaseLayer { base_value: 100 },
            },
            own: MidLayer {
                mid_value: 200,
                base_seen_after_super: 100,
            },
        },
        own: LeafLayer {
            leaf_value: 300,
            mid_seen_after_super: 200,
        },
    };
    new_from_chain(env, chain)
}
