//! Own data blocks: a single per-instance `OwnDataRegistry` holds every
//! layer's Rust struct in a fixed-size slice of independently-borrowed
//! slots, indexed by [`OwnBlock::IDX`]. The registry is attached to the
//! instance once via `napi_wrap`; reads and writes are pure Rust-side slot
//! borrow + slice index + `TypeId` downcast.
//!
//! Each slot is its own `RefCell`, so borrowing layer A's slot never blocks
//! layer B's - only concurrent shared/mutable access to the same layer's
//! slot conflicts.
//!
//! The slot layout is derived at compile time by [`OwnBlock`]; see its docs
//! for the `DEPTH` / `IDX` derivation.

use std::any::Any;
use std::cell::RefCell;

use napi::{
    Error, Result, Status,
    bindgen_prelude::{JsObjectValue, Object, Property, PropertyAttributes},
};

use crate::layer::{ExtendLayer, OwnBlock};

/// Key under which every layer instance stores a self-reference. A
/// receiver-passing proxy (e.g. Vue reactivity's
/// `Reflect.get(target, key, receiver)`) forwards property reads to the
/// target's own value, so reading this key *through* a proxy yields the
/// raw instance; accessors unwrap that instead of the receiver. Read only
/// on the slow path, after a direct unwrap of the receiver failed.
const REAL_INSTANCE_KEY: &str = "__napi_blitz_real_instance";

/// One independently borrowed slot per real layer in the chain, indexed at
/// compile time by `OwnBlock::IDX`.
type OwnSlots = Box<[RefCell<Option<Box<dyn Any>>>]>;

/// The per-instance own-data store. One slice slot per real layer in the
/// chain (RootLayer takes none); `IDX` addresses a layer's slot directly, so
/// there is no bounds check or growth logic - the slice is sized to the leaf
/// layer's `DEPTH` at attach time.
pub struct OwnDataRegistry {
    slots: OwnSlots,
}

impl OwnDataRegistry {
    /// Build a registry sized for a chain ending at `T` (the leaf layer).
    /// `slots.len() = T::DEPTH` is a compile-time constant, so the whole
    /// slice is allocated in one shot. Slots start empty; `set_slot_for`
    /// fills them layer by layer during construction.
    fn new<T: ExtendLayer + OwnBlock>() -> Self {
        let slots = (0..T::DEPTH)
            .map(|_| RefCell::new(None))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self { slots }
    }

    /// Write `T`'s data into its slot. `T::IDX` is a compile-time constant
    /// and a registry is always sized for the chain `T` is constructed on,
    /// so this is a plain slice assignment with no bounds check.
    #[inline]
    fn set_slot_for<T: ExtendLayer + OwnBlock>(&self, data: T) {
        *self.slots[T::IDX].borrow_mut() = Some(Box::new(data));
    }

    /// Resolve the `RefCell` for `T`'s slot. Fails if `T::IDX` is out of
    /// range (the receiver was built for a shorter chain - e.g. a child
    /// layer's getter invoked on a parent-only instance).
    #[inline]
    fn slot_for<T: ExtendLayer + OwnBlock>(&self) -> Result<&RefCell<Option<Box<dyn Any>>>> {
        self.slots.get(T::IDX).ok_or_else(|| {
            Error::new(
                Status::GenericFailure,
                format!("{}: slot index {} out of range", T::CLASS_NAME, T::IDX),
            )
        })
    }

    #[inline]
    fn with<T: ExtendLayer + OwnBlock, R>(&self, f: impl FnOnce(&T) -> R) -> Result<R> {
        let slot = self.slot_for::<T>()?.borrow();
        let Some(any) = slot.as_deref() else {
            return Err(slot_error::<T>("not constructed"));
        };
        let data = any
            .downcast_ref::<T>()
            .ok_or_else(|| slot_error::<T>("type mismatch"))?;
        Ok(f(data))
    }

    #[inline]
    fn with_mut<T: ExtendLayer + OwnBlock, R>(&self, f: impl FnOnce(&mut T) -> R) -> Result<R> {
        let mut slot = self.slot_for::<T>()?.borrow_mut();
        let Some(any) = slot.as_deref_mut() else {
            return Err(slot_error::<T>("not constructed"));
        };
        let data = any
            .downcast_mut::<T>()
            .ok_or_else(|| slot_error::<T>("type mismatch"))?;
        Ok(f(data))
    }
}

fn slot_error<T: ExtendLayer + OwnBlock>(reason: &str) -> Error {
    Error::new(
        Status::GenericFailure,
        format!("{}: own block {reason}", T::CLASS_NAME),
    )
}

/// Attach a fresh `OwnDataRegistry` to `this` via `napi_wrap`. Sized for the
/// chain ending at `T`. Called once at the entry of each instantiation path
/// (JS `new`, Rust data chain) - the recursive layer code then reaches the
/// same registry through [`own_registry`].
pub fn attach_registry<T: ExtendLayer + OwnBlock>(this: &mut Object) -> Result<()> {
    let registry = OwnDataRegistry::new::<T>();
    this.wrap(registry, None)?;
    // Self-reference for the receiver re-resolution in `with_registry`.
    // Empty attribute bits: not writable, not enumerable, not configurable.
    let prop = Property::new()
        .with_utf8_name(REAL_INSTANCE_KEY)?
        .with_value(this)
        .with_property_attributes(PropertyAttributes::empty());
    this.define_properties(&[prop])
}

/// Run `f` with the instance's `OwnDataRegistry`. Shared access is enough:
/// each slot is interior-mutable through its own `RefCell`.
///
/// Fast path: unwrap the receiver directly - the common case, where the
/// receiver is the raw instance, costs the same as the pre-proxy design.
/// Slow path (unwrap failed, so the receiver is not the raw instance -
/// e.g. a Vue reactivity proxy): resolve the raw instance through its
/// self-reference key and unwrap that instead.
#[inline]
fn with_registry<R>(this: &Object, f: impl FnOnce(&OwnDataRegistry) -> Result<R>) -> Result<R> {
    if let Ok(registry) = this.unwrap::<OwnDataRegistry>() {
        return f(registry);
    }
    match this.get_named_property::<Option<Object>>(REAL_INSTANCE_KEY) {
        Ok(Some(real)) => {
            let registry = real
                .unwrap::<OwnDataRegistry>()
                .map_err(|_| no_registry_error())?;
            f(registry)
        }
        _ => Err(no_registry_error()),
    }
}

fn no_registry_error() -> Error {
    Error::new(
        Status::GenericFailure,
        "instance has no OwnDataRegistry attached",
    )
}

/// Write a layer's data into its slot.
#[inline]
pub fn set_own_block<T: ExtendLayer + OwnBlock>(this: &Object, data: T) -> Result<()> {
    with_registry(this, |registry| {
        registry.set_slot_for::<T>(data);
        Ok(())
    })
}

/// Read a layer's own block by shared reference.
#[inline]
pub fn with_own<T, R>(this: &Object, f: impl FnOnce(&T) -> R) -> Result<R>
where
    T: ExtendLayer + OwnBlock,
{
    with_registry(this, |registry| registry.with::<T, R>(f))
}

/// Mutate a layer's own block in place through the callback. The `&mut T`
/// borrow lives only inside `f` - do not touch the same own block again
/// before it returns.
#[inline]
pub fn with_own_mut<T, R>(this: &Object, f: impl FnOnce(&mut T) -> R) -> Result<R>
where
    T: ExtendLayer + OwnBlock,
{
    with_registry(this, |registry| registry.with_mut::<T, R>(f))
}
