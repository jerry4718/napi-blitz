//! The layer trait system.
//!
//! [`LayerDef`] describes one level of an inheritance chain: its parent
//! layer, its class name, and which members live on its prototype.
//! [`LayerBuild`] builds this layer's own data from constructor arguments,
//! enforcing the ES super-before-return rule through [`Super`]. [`ExtendLayer`]
//! aggregates the two and is implemented automatically for any type that has
//! both. [`EmitOwn`] drives the recursive write of own blocks onto an
//! instance, along both the JS `new` path (`emit_own`) and the Rust
//! data-chain path (`populate_chain`).

use std::marker::PhantomData;

use napi::{
    Env, Result,
    bindgen_prelude::{Object, Unknown},
};

use crate::{own::set_own_block, registry::HasClassRef};

/// Compile-time layout of a layer's own data inside the per-instance
/// `OwnDataRegistry`. `RootLayer` is the chain terminator with `DEPTH = 0`;
/// every `ExtendLayer` derives `Parent::DEPTH + 1`. `DEPTH` is the number of
/// real layers on the chain (so it is also the exact registry size), and
/// `IDX` is the layer's slot position, `DEPTH - 1`, evaluated at compile
/// time.
pub trait OwnBlock: 'static {
    /// Number of real layers from the chain root down to and including
    /// `Self`. `RootLayer::DEPTH = 0` - it occupies no slot.
    const DEPTH: usize;

    /// The slot position of `Self`'s own data: `DEPTH - 1`, evaluated at
    /// compile time.
    const IDX: usize = Self::DEPTH - 1;
}

/// One level of an inheritance chain: its parent layer, class name, and the
/// members it registers on its prototype and constructor.
pub trait LayerDef: Sized + 'static {
    /// The parent layer. `RootLayer` terminates the chain.
    type Parent: EmitOwn + HasClassRef + OwnBlock;

    const CLASS_NAME: &'static str;

    /// Register this layer's members (getters, methods on `proto`; static
    /// constants on `ctor`).
    fn define_members(env: &Env, proto: &mut Object, ctor: &mut Object) -> Result<()>;
}

/// The constructor bridge. `sup.call(...)` must be invoked before returning:
/// `Constructed` can only be assembled from the resulting `SuperDone`
/// receipt, so the type system forces the parent to be constructed first -
/// the ES super-before-return rule. The instance is not a parameter: it only
/// becomes reachable through the `SuperDone` receipt after `sup.call`, the
/// ES "no `this` before super" rule expressed at the signature level.
///
/// The macro generates this from a `#[layer(constructor)]` method: it
/// dispatches the method's typed arguments out of the raw JS slice, calls
/// `sup`, and hands `env` / `this` to the method (which may be a JS subclass
/// instance).
pub trait LayerBuild: LayerDef + Sized + 'static {
    fn build<'env>(
        env: &'env Env,
        args: &[Unknown<'env>],
        sup: Super<'_, 'env, Self::Parent>,
    ) -> Result<Constructed<Self>>;
}

pub trait LayerComposed: Sized + 'static {
    /// The parent layer. `RootLayer` terminates the chain.
    type Parent: EmitOwn + HasClassRef + OwnBlock;

    const CLASS_NAME: &'static str;

    fn build<'env>(
        env: &'env Env,
        args: &[Unknown<'env>],
        sup: Super<'_, 'env, Self::Parent>,
    ) -> Result<Constructed<Self>>;

    /// Register this layer's members (getters, methods on `proto`; static
    /// constants on `ctor`).
    fn define_members(env: &Env, proto: &mut Object, ctor: &mut Object) -> Result<()>;
}

/// The composed path: a hand-written layer implements this single trait
/// (description + constructor bridge) instead of `LayerDef` + `LayerBuild`
/// separately; the blanket impls below split it back into the two focused
/// traits, which is what the rest of the runtime consumes.
pub trait ExtendLayer: LayerDef + LayerBuild {}

impl<T: LayerDef + LayerBuild> ExtendLayer for T {}

impl<T: LayerComposed> LayerDef for T {
    type Parent = <T as LayerComposed>::Parent;
    const CLASS_NAME: &'static str = <T as LayerComposed>::CLASS_NAME;

    fn define_members(env: &Env, proto: &mut Object, ctor: &mut Object) -> Result<()> {
        <T as LayerComposed>::define_members(env, proto, ctor)
    }
}

impl<T: LayerComposed> LayerBuild for T {
    fn build<'env>(
        env: &'env Env,
        args: &[Unknown<'env>],
        sup: Super<'_, 'env, Self::Parent>,
    ) -> Result<Constructed<Self>> {
        <T as LayerComposed>::build(env, args, sup)
    }
}

/// Recursively writes own blocks onto an instance.
pub trait EmitOwn: 'static {
    /// The chain type of the Rust data path. `RootLayer::Chain` is `()`;
    /// `T::Chain` is `LayerChain<T>`.
    type Chain;

    /// JS `new` path: build each layer's own data from the constructor
    /// arguments and write it onto the instance.
    fn emit_own<'env>(env: &'env Env, this: &Object<'env>, args: &[Unknown<'env>]) -> Result<()>;

    /// Write an existing data chain onto the instance, parent layer first.
    fn populate_chain<'env>(env: &'env Env, this: &Object<'env>, chain: Self::Chain) -> Result<()>;
}

/// Chain terminator: no parent, no own data.
#[derive(Debug, Default, Clone)]
pub struct RootLayer;

impl EmitOwn for RootLayer {
    type Chain = ();

    fn emit_own<'env>(
        _env: &'env Env,
        _this: &Object<'env>,
        _args: &[Unknown<'env>],
    ) -> Result<()> {
        Ok(())
    }

    fn populate_chain<'env>(_env: &'env Env, _this: &Object<'env>, _chain: ()) -> Result<()> {
        Ok(())
    }
}

impl OwnBlock for RootLayer {
    const DEPTH: usize = 0;
}

/// A node of the Rust data chain: this layer's own data plus the parent
/// chain. Values move straight into the own blocks.
pub struct LayerChain<T: ExtendLayer> {
    pub parent: <T::Parent as EmitOwn>::Chain,
    pub own: T,
}

impl<T: LayerDef + LayerBuild> EmitOwn for T {
    type Chain = LayerChain<T>;

    fn emit_own<'env>(env: &'env Env, this: &Object<'env>, args: &[Unknown<'env>]) -> Result<()> {
        let sup = Super::<T::Parent> {
            instance: this,
            env,
            _parent: PhantomData,
        };
        let constructed = T::build(env, args, sup)?;
        set_own_block(this, constructed.own)
    }

    fn populate_chain<'env>(
        env: &'env Env,
        this: &Object<'env>,
        chain: LayerChain<T>,
    ) -> Result<()> {
        T::Parent::populate_chain(env, this, chain.parent)?;
        set_own_block(this, chain.own)
    }
}

/// The super handle: `call` triggers the parent layer's recursive
/// construction on the instance.
pub struct Super<'a, 'env, P: EmitOwn> {
    instance: &'a Object<'env>,
    env: &'env Env,
    _parent: PhantomData<fn() -> P>,
}

impl<'a, 'env, P: EmitOwn> Super<'a, 'env, P> {
    /// Construct the parent layers now. Once this returns, every ancestor
    /// layer's own block is visible on the instance.
    pub fn call(self, parent_args: &[Unknown<'env>]) -> Result<SuperDone<'a, 'env>> {
        P::emit_own(self.env, self.instance, parent_args)?;
        Ok(SuperDone {
            this: self.instance,
        })
    }
}

/// Receipt proving that `super` has run. Only `Super::call` can produce
/// one. It hands back the instance - the only way a `bridge_build` gets an
/// instance handle, and only after the parent layers are constructed.
pub struct SuperDone<'a, 'env> {
    this: &'a Object<'env>,
}

impl<'a, 'env> SuperDone<'a, 'env> {
    /// The instance, valid once every parent layer's own block is in place.
    pub fn this(&self) -> &'a Object<'env> {
        self.this
    }
}

/// The return value of `bridge_build`. Assemblable only from a `SuperDone`,
/// which is what makes "call super before returning" a type-level guarantee.
pub struct Constructed<T> {
    pub(crate) own: T,
}

impl<T> Constructed<T> {
    pub fn new(_done: SuperDone<'_, '_>, own: T) -> Self {
        Self { own }
    }
}

impl<T: LayerDef> OwnBlock for T {
    const DEPTH: usize = <T::Parent as OwnBlock>::DEPTH + 1;
}
