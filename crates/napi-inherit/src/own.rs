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
    Env, Error, Result, Status,
    bindgen_prelude::{FromNapiValue, JsObjectValue, JsValue, Object, Unknown},
};

use crate::layer::{ExtendLayer, OwnBlock};

/// Parse one constructor argument at `idx` from the `Unknown` slice into `T`.
/// Used by the generated `ExtendLayer::build` to convert the constructor's
/// declared parameter list from raw JS values.
pub fn arg_from_napi<T: FromNapiValue>(env: &Env, args: &[Unknown], idx: usize) -> Result<T> {
    let Some(v) = args.get(idx) else {
        return Err(Error::new(
            Status::GenericFailure,
            format!("missing constructor arg {idx}"),
        ));
    };
    unsafe { T::from_napi_value(env.raw(), JsValue::raw(v)) }
}

/// The per-instance own-data store. One slice slot per real layer in the
/// chain (RootLayer takes none); `IDX` addresses a layer's slot directly, so
/// there is no bounds check or growth logic - the slice is sized to the leaf
/// layer's `DEPTH` at attach time.
pub struct OwnDataRegistry {
    slots: Box<[RefCell<Box<dyn Any>>]>,
}

impl OwnDataRegistry {
    /// Build a registry sized for a chain ending at `T` (the leaf layer).
    /// `slots.len() = T::DEPTH` is a compile-time constant, so the whole
    /// slice is allocated in one shot.
    fn new<T: ExtendLayer + OwnBlock>() -> Self {
        let slots = (0..T::DEPTH)
            .map(|_| RefCell::new(Box::new(()) as Box<dyn Any>))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self { slots }
    }

    #[inline]
    fn set<T: ExtendLayer + OwnBlock>(&self, data: T) {
        *self.slots[T::IDX].borrow_mut() = Box::new(data);
    }

    /// Resolve the `RefCell` for `T`'s slot. Fails if `T::IDX` is out of
    /// range (the receiver was built for a shorter chain - e.g. a child
    /// layer's getter invoked on a parent-only instance).
    #[inline]
    fn slot_for<T: ExtendLayer + OwnBlock>(&self) -> Result<&RefCell<Box<dyn Any>>> {
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
        let data = slot.downcast_ref::<T>().ok_or_else(|| {
            Error::new(
                Status::GenericFailure,
                format!("{}: own block missing or type mismatch", T::CLASS_NAME),
            )
        })?;
        Ok(f(data))
    }

    #[inline]
    fn with_mut<T: ExtendLayer + OwnBlock, R>(&self, f: impl FnOnce(&mut T) -> R) -> Result<R> {
        let mut slot = self.slot_for::<T>()?.borrow_mut();
        let data = slot.downcast_mut::<T>().ok_or_else(|| {
            Error::new(
                Status::GenericFailure,
                format!("{}: own block missing or type mismatch", T::CLASS_NAME),
            )
        })?;
        Ok(f(data))
    }
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

/// Write a layer's data into its slot. `T::IDX` is a compile-time constant,
/// so this is a plain slice assignment with no bounds check.
pub fn set_own_block<T: ExtendLayer + OwnBlock>(this: &Object, data: T) -> Result<()> {
    let registry = own_registry(this)?;
    registry.set::<T>(data);
    Ok(())
}

/// Read a layer's own block by shared reference.
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
pub fn with_own_mut<T, R>(this: &Object, f: impl FnOnce(&mut T) -> R) -> Result<R>
where
    T: ExtendLayer + OwnBlock,
{
    let registry = own_registry(this)?;
    registry.with_mut::<T, R>(f)
}
