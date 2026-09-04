use napi::bindgen_prelude::{FnArgs, Object};
use napi::{Error, Result, Status};
use napi_derive::napi;

use napi_inherit::{
    from_chain,
    layer::{Constructed, RootLayer, Super},
    own::{with_own, with_own_mut},
};

use napi_inherit_proc::layer;

use std::sync::atomic::{AtomicU32, Ordering};

/// Backing store for the static `counter` accessor pair.
static SHARED_COUNTER: AtomicU32 = AtomicU32::new(10);

// ── InheritBase ──────────────────────────────────────────────────────────

#[layer(js_name = "InheritBase")]
pub struct BaseLayer {
    #[layer(getter, setter)]
    pub base_value: u32,
}

#[layer]
impl BaseLayer {
    #[layer]
    const BASE_CONST: u32 = 1;

    #[layer(constructor)]
    fn build(base_value: u32, sup: Super<RootLayer>) -> Result<Constructed<Self>> {
        let done = sup.call(FnArgs::from(()))?;
        Ok(Constructed::new(done, Self { base_value }))
    }

    #[layer]
    fn base_greet(&self) -> String {
        format!("base:{}", self.base_value)
    }

    #[layer]
    fn bump_base(this: &Object, delta: u32) -> Result<u32> {
        with_own_mut::<BaseLayer, _>(this, |d| {
            d.base_value += delta;
            d.base_value
        })
    }

    /// Static accessor pair on `InheritBase.counter`, backed by a Rust
    /// atomic (there is no instance slot for static state).
    #[layer(getter)]
    fn counter() -> u32 {
        SHARED_COUNTER.load(Ordering::Relaxed)
    }

    #[layer(setter)]
    fn set_counter(v: u32) {
        SHARED_COUNTER.store(v, Ordering::Relaxed);
    }

    /// Result-returning instance accessor pair: reads/writes the base slot
    /// and fails loudly on a zero value.
    #[layer(getter)]
    fn checked_value(&self) -> Result<u32> {
        if self.base_value == 0 {
            Err(Error::new(Status::GenericFailure, "base_value is zero"))
        } else {
            Ok(self.base_value)
        }
    }

    #[layer(setter)]
    fn set_checked_value(&mut self, v: u32) -> Result<()> {
        if v == 0 {
            return Err(Error::new(Status::GenericFailure, "cannot set to zero"));
        }
        self.base_value = v;
        Ok(())
    }

    /// Result-returning static accessor pair over the shared counter.
    #[layer(getter)]
    fn checked_counter() -> Result<u32> {
        let v = SHARED_COUNTER.load(Ordering::Relaxed);
        if v == 0 {
            Err(Error::new(Status::GenericFailure, "counter is zero"))
        } else {
            Ok(v)
        }
    }

    #[layer(setter)]
    fn set_checked_counter(v: u32) -> Result<()> {
        if v == 0 {
            return Err(Error::new(
                Status::GenericFailure,
                "cannot set counter to zero",
            ));
        }
        SHARED_COUNTER.store(v, Ordering::Relaxed);
        Ok(())
    }

    /// Result-returning `&self` method (with_own outer + `?`): errors when
    /// the base value is zero.
    #[layer]
    fn guarded_greet(&self) -> Result<String> {
        if self.base_value == 0 {
            return Err(Error::new(Status::GenericFailure, "base is zero"));
        }
        Ok(format!("base:{}", self.base_value))
    }

    /// Result-returning `this`-injected method.
    #[layer]
    fn guard(this: &Object, v: u32) -> Result<String> {
        if v == 0 {
            return Err(Error::new(Status::GenericFailure, "guard rejects zero"));
        }
        let base = with_own::<BaseLayer, _>(this, |d| d.base_value)?;
        Ok(format!("guard:{v}/base:{base}"))
    }

    /// Result-returning static method.
    #[layer]
    fn static_guard(v: u32) -> Result<u32> {
        if v == 0 {
            return Err(Error::new(
                Status::GenericFailure,
                "static guard rejects zero",
            ));
        }
        Ok(v * 2)
    }
}

// ── InheritMid ───────────────────────────────────────────────────────────

#[layer(js_name = "InheritMid", parent = BaseLayer)]
pub struct MidLayer {
    #[layer(getter)]
    pub mid_value: u32,
    /// The parent data read through `this`.
    #[layer(getter)]
    pub base_seen_after_super: u32,
}

#[layer]
impl MidLayer {
    #[layer]
    const MID_CONST: u32 = 2;

    #[layer(constructor)]
    fn build(base_value: u32, mid_value: u32, sup: Super<BaseLayer>) -> Result<Constructed<Self>> {
        let done = sup.call(FnArgs::from((base_value,)))?;
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

    #[layer]
    fn mid_describe(this: &Object) -> Result<String> {
        let mid = with_own::<MidLayer, _>(this, |d| d.mid_value)?;
        let base = with_own::<BaseLayer, _>(this, |d| d.base_value)?;
        Ok(format!("mid:{mid}/base:{base}"))
    }
}

// ── InheritLeaf ──────────────────────────────────────────────────────────

#[layer(js_name = "InheritLeaf", parent = MidLayer)]
pub struct LeafLayer {
    #[layer(getter)]
    pub leaf_value: u32,
    #[layer(getter)]
    pub mid_seen_after_super: u32,
}

#[layer]
impl LeafLayer {
    #[layer]
    const LEAF_CONST: u32 = 3;

    #[layer(constructor)]
    fn build(
        base_value: u32,
        mid_value: u32,
        leaf_value: u32,
        sup: Super<MidLayer>,
    ) -> Result<Constructed<Self>> {
        let done = sup.call(FnArgs::from((base_value, mid_value)))?;
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

    #[layer]
    fn leaf_shout(this: &Object) -> Result<String> {
        let leaf = with_own::<LeafLayer, _>(this, |d| d.leaf_value)?;
        let mid = with_own::<MidLayer, _>(this, |d| d.mid_value)?;
        let base = with_own::<BaseLayer, _>(this, |d| d.base_value)?;
        Ok(format!("leaf:{leaf}+mid:{mid}+base:{base}"))
    }

    #[layer]
    fn leaf_const() -> u32 {
        99
    }
}

// ── Exports ──────────────────────────────────────────────────────────────

/// Build an `InheritLeaf` from a Rust-side data chain, bypassing the JS
/// `new` path. Distinct Rust/js name from the manual chain's
/// `makeInheritLeafFromChain`.
#[napi]
pub fn make_proc_leaf_from_chain<'env>(env: &'env napi::Env) -> Result<Object<'env>> {
    from_chain!((
        LeafLayer,
        env
    ) BaseLayer { base_value: 100 }, MidLayer { mid_value: 200, base_seen_after_super: 100 }, LeafLayer { leaf_value: 300, mid_seen_after_super: 200 })
}
