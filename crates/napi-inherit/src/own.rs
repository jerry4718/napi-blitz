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
    bindgen_prelude::{JsObjectValue, Object},
};

use crate::layer::{ExtendLayer, OwnBlock};

/// The per-instance own-data store. One slice slot per real layer in the
/// chain (RootLayer takes none); `IDX` addresses a layer's slot directly, so
/// there is no bounds check or growth logic - the slice is sized to the leaf
/// layer's `DEPTH` at attach time.
pub struct OwnDataRegistry {
    slots: Box<[RefCell<Option<Box<dyn Any>>>]>,
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
    this.wrap(registry, None)
}

/// Borrow the instance's `OwnDataRegistry`. Shared access is enough: each
/// slot is interior-mutable through its own `RefCell`.
#[inline]
fn own_registry<'a>(this: &'a Object<'_>) -> Result<&'a OwnDataRegistry> {
    this.unwrap::<OwnDataRegistry>()
        .map(|r| r as &OwnDataRegistry)
        .map_err(|_| {
            Error::new(
                Status::GenericFailure,
                "instance has no OwnDataRegistry attached",
            )
        })
}

/// Write a layer's data into its slot.
#[inline]
pub fn set_own_block<T: ExtendLayer + OwnBlock>(this: &Object, data: T) -> Result<()> {
    let registry = own_registry(this)?;
    registry.set_slot_for::<T>(data);
    Ok(())
}

/// Read a layer's own block by shared reference.
#[inline]
pub fn with_own<T, R>(this: &Object, f: impl FnOnce(&T) -> R) -> Result<R>
where
    T: ExtendLayer + OwnBlock,
{
    let registry = own_registry(this)?;
    registry.with::<T, R>(f)
}

/// Mutate a layer's own block in place through the callback. The `&mut T`
/// borrow lives only inside `f` - do not touch the same own block again
/// before it returns.
#[inline]
pub fn with_own_mut<T, R>(this: &Object, f: impl FnOnce(&mut T) -> R) -> Result<R>
where
    T: ExtendLayer + OwnBlock,
{
    let registry = own_registry(this)?;
    registry.with_mut::<T, R>(f)
}
