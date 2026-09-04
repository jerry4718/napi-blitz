//! Class registry: `TypeId -> (constructor, prototype)`.
//!
//! Keyed by `TypeId` rather than the class name so two chains that happen to
//! share a `CLASS_NAME` string (e.g. the macro-generated and the hand-written
//! test chains) never collide. `CLASS_NAME` is still used for the JS class
//! name and error messages.
//!
//! Thread-local because `napi_ref` handles belong to the single JS thread
//! that created them. References live for the process lifetime - the
//! registry is the Rust-side root set keeping constructors and prototypes
//! alive, which is what `link_prototype` and `new_from_chain` resolve
//! against.

use std::{any::TypeId, cell::RefCell, collections::HashMap};

use napi::{
    Env, Error, JsValue, Result, Status,
    bindgen_prelude::{FromNapiValue, Object, ObjectRef},
};

use crate::class::build_class;
use crate::layer::{ExtendLayer, LayerAccessors, LayerBuild, RootLayer};

/// Resolve a layer's (constructor, prototype) handles. `RootLayer` has no
/// JS class, so it resolves to `None` - the only `None` there is. Every
/// other layer must already be registered; an unregistered parent class is
/// a registration-order bug and fails loudly instead of silently skipping
/// the prototype link.
pub trait HasClassRef {
    fn class_handles(env: &Env) -> Result<Option<(Object<'_>, Object<'_>)>>;

    /// Ensure the class is registered, building it lazily if not. `RootLayer`
    /// (the chain terminator) is the no-op default; every other layer builds
    /// itself through `build_class`.
    fn ensure_class_built(_env: &Env) -> Result<()> {
        Ok(())
    }
}

impl HasClassRef for RootLayer {
    fn class_handles(_env: &Env) -> Result<Option<(Object<'_>, Object<'_>)>> {
        Ok(None)
    }
}

impl<T: ExtendLayer + LayerAccessors> HasClassRef for T
where
    <T as LayerBuild>::Args: FromNapiValue,
{
    fn class_handles(env: &Env) -> Result<Option<(Object<'_>, Object<'_>)>> {
        require(env, TypeId::of::<T>()).map(Some)
    }

    fn ensure_class_built(env: &Env) -> Result<()> {
        if contains(TypeId::of::<T>()) {
            return Ok(());
        }
        build_class::<T>(env)
    }
}

/// Root-set references: the constructor and prototype are kept alive here
/// for the whole addon lifetime and never unref'd; LEAK_CHECK=false is
/// deliberate - they are not a forgotten release.
type Entry = (ObjectRef<false>, ObjectRef<false>);

thread_local! {
    static REGISTRY: RefCell<HashMap<TypeId, Entry>> = RefCell::new(HashMap::new());
}

pub fn insert(env: &Env, type_id: TypeId, ctor: &Object, proto: &Object) -> Result<()> {
    let env_raw = env.raw();
    let ctor_ref = unsafe { ObjectRef::<false>::from_napi_value(env_raw, JsValue::raw(ctor))? };
    let proto_ref = unsafe { ObjectRef::<false>::from_napi_value(env_raw, JsValue::raw(proto))? };
    REGISTRY.with(|r| {
        r.borrow_mut().insert(type_id, (ctor_ref, proto_ref));
    });
    Ok(())
}

pub fn get(env: &Env, type_id: TypeId) -> Result<Option<(Object<'_>, Object<'_>)>> {
    REGISTRY.with(|r| {
        let registry = r.borrow();
        let Some((ctor_ref, proto_ref)) = registry.get(&type_id) else {
            return Ok(None);
        };
        let ctor = ctor_ref.get_value(env)?;
        let proto = proto_ref.get_value(env)?;
        Ok(Some((ctor, proto)))
    })
}

pub fn contains(type_id: TypeId) -> bool {
    REGISTRY.with(|r| r.borrow().contains_key(&type_id))
}

/// Fetch a registered class or fail loudly.
pub fn require(env: &Env, type_id: TypeId) -> Result<(Object<'_>, Object<'_>)> {
    get(env, type_id)?.ok_or_else(|| {
        Error::new(
            Status::GenericFailure,
            format!("class {type_id:?} not registered"),
        )
    })
}
