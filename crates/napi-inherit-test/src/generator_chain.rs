//! `#[layer(generator)]` / `#[layer(async_generator)]` end-to-end case:
//! every iteration step calls back into Rust, so values are read from the
//! instance at loop time (a mid-loop push is visible) and each callback
//! logs its index on the Rust side.

use std::cell::RefCell;

use napi::{
    Error, Result,
    bindgen_prelude::{FnArgs, Object},
};
use napi_derive::napi;

use napi_helpers::inherits::{Constructed, RootLayer, Super, from_chain, proc::layer};
use napi_helpers::native_log;

#[layer]
pub struct GenSourceLayer {
    pub label: String,
    pub items: RefCell<Vec<u32>>,
}

#[layer(js_name = "GenSource")]
impl GenSourceLayer {
    #[layer(constructor)]
    fn build(label: String, sup: Super<RootLayer>) -> Result<Constructed<Self>> {
        let done = sup.call(FnArgs::from(()))?;
        Ok(Constructed::new(
            done,
            Self {
                label,
                items: RefCell::new(vec![10, 20, 30]),
            },
        ))
    }

    #[layer(generator)]
    fn values(&self, index: u32) -> Result<Option<u32>> {
        let v = self
            .items
            .try_borrow()
            .map_err(|e| Error::from_reason(e.to_string()))
            .map(|r| r.get(index as usize).copied());
        native_log!("[rust] values(index={}) -> {:?}", index, v);
        v
    }

    #[layer(async_generator)]
    fn async_values(&self, index: u32) -> Result<Option<u32>> {
        let v = self
            .items
            .try_borrow()
            .map_err(|e| Error::from_reason(e.to_string()))
            .map(|r| r.get(index as usize).copied());
        native_log!("[rust] async_values(index={}) -> {:?}", index, v);
        v
    }

    #[layer(method)]
    fn push(&self, v: u32) -> Result<()> {
        native_log!("[rust] push({})", v);
        self.items
            .try_borrow_mut()
            .map_err(|e| Error::from_reason(e.to_string()))
            .map(|mut r| r.push(v))
    }
}

#[napi]
pub fn make_gen_source<'env>(env: &'env napi::Env) -> Result<Object<'env>> {
    from_chain!(
        (GenSourceLayer, env),
        GenSourceLayer {
            label: "src".into(),
            items: RefCell::new(vec![10, 20, 30]),
        }
    )
}
