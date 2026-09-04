//! The `attributes` Proxy handler — a NamedNodeMap-ish surface over the
//! element's content attributes.
//!
//! `el.attributes` returns `new Proxy(target, handler)`; every trap
//! below reads/writes the element's content attributes via `node_id` +
//! doc.

use std::rc::Rc;

use napi::bindgen_prelude::{FromNapiValue, JsValue, Object};
use napi::{Env, Error, Result};
use napi_helpers::{
    anything::Anything,
    inherits::{Constructed, RootLayer, Super, proc::layer},
};

use crate::dom::{layers::element::ElementLayer, shared::doc::SharedDocument};
use blitz::dom::NodeId;

#[layer]
pub struct AttributesHandlerLayer {
    pub(crate) node_id: NodeId,
    pub(crate) shared_doc: Rc<SharedDocument>,
}

#[layer(js_name = "AttributesHandler")]
impl AttributesHandlerLayer {
    #[layer(parent)]
    type Parent = RootLayer;

    #[layer(constructor)]
    fn build(_sup: Super<RootLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "AttributesHandler cannot be constructed directly; it backs el.attributes",
        ))
    }

    /// The `get` trap: the attribute's value, or `undefined` when absent.
    #[layer]
    fn get(&self, _target: Anything, prop: Anything, _receiver: Anything) -> Result<Anything> {
        let Anything::String(name) = prop else {
            return Ok(Anything::Undefined);
        };
        Ok(
            ElementLayer::attribute(&self.shared_doc, self.node_id, &name)
                .map(Anything::String)
                .unwrap_or(Anything::Undefined),
        )
    }

    /// The `set` trap: sets the content attribute.
    #[layer]
    fn set(
        &mut self,
        _target: Anything,
        prop: Anything,
        value: String,
        _receiver: Anything,
    ) -> Result<bool> {
        let Anything::String(name) = prop else {
            return Ok(false);
        };
        ElementLayer::attr_set(&self.shared_doc, self.node_id, &name, &value, None);
        Ok(true)
    }

    /// The `getOwnPropertyDescriptor` trap: `Object.keys` walks the
    /// `ownKeys` result and asks for each key's descriptor, so present
    /// attributes must answer with an enumerable descriptor.
    #[layer]
    fn get_own_property_descriptor(
        &self,
        env: &Env,
        _target: Anything,
        prop: Anything,
    ) -> Result<Anything> {
        let Anything::String(name) = prop else {
            return Ok(Anything::Undefined);
        };
        let Some(value) = ElementLayer::attribute(&self.shared_doc, self.node_id, &name) else {
            return Ok(Anything::Undefined);
        };
        let mut desc = Object::new(env)?;
        desc.set("value", Anything::String(value))?;
        desc.set("writable", true)?;
        desc.set("enumerable", true)?;
        desc.set("configurable", true)?;
        unsafe { Anything::from_napi_value(env.raw(), JsValue::raw(&desc)) }
    }

    /// The `has` trap: the attribute exists.
    #[layer]
    fn has(&self, _target: Anything, prop: Anything) -> Result<bool> {
        let Anything::String(name) = prop else {
            return Ok(false);
        };
        Ok(ElementLayer::attr_has(
            &self.shared_doc,
            self.node_id,
            &name,
        ))
    }

    /// The `deleteProperty` trap: removes the content attribute.
    #[layer]
    fn delete_property(&mut self, _target: Anything, prop: Anything) -> Result<bool> {
        let Anything::String(name) = prop else {
            return Ok(false);
        };
        ElementLayer::attr_remove(&self.shared_doc, self.node_id, &name, None);
        Ok(true)
    }

    /// The `ownKeys` trap: all content attribute names.
    #[layer]
    fn own_keys(&self, _target: Anything) -> Result<Vec<String>> {
        Ok(ElementLayer::attr_list(&self.shared_doc, self.node_id)
            .into_iter()
            .map(|attr| attr.name)
            .collect())
    }
}
