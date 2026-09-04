//! `#[layer(generator)]` / `#[layer(async_generator)]` end-to-end case:
//! every iteration step calls back into Rust, so values are read from the
//! instance at loop time (a mid-loop push is visible) and each callback
//! `eprintln!`s its index on the Rust side.

use std::cell::RefCell;

use napi::Result;
use napi::bindgen_prelude::{FnArgs, Object};
use napi_derive::napi;

use napi_helpers::inherits::{Constructed, RootLayer, Super, from_chain, proc::layer};

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
    fn values(&self, index: u32) -> Option<u32> {
        let v = self.items.borrow().get(index as usize).copied();
        eprintln!("[rust] values(index={}) -> {:?}", index, v);
        v
    }

    #[layer(async_generator)]
    fn async_values(&self, index: u32) -> Option<u32> {
        let v = self.items.borrow().get(index as usize).copied();
        eprintln!("[rust] async_values(index={}) -> {:?}", index, v);
        v
    }

    #[layer(method)]
    fn push(&self, v: u32) {
        eprintln!("[rust] push({})", v);
        self.items.borrow_mut().push(v);
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
