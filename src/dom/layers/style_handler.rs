//! The `style` Proxy handler — a CSSOM `CSSStyleDeclaration` surface
//! backed by the element's inline style block.
//!
//! `el.style` returns `new Proxy(target, handler)`; every trap below
//! talks straight to the element's style data via `node_id` + doc. The
//! CSSOM method functions are built once per element (at handler
//! construction) and stored, so `el.style.getPropertyValue` is
//! identity-stable.

use std::cell::RefCell;
use std::rc::Rc;

use napi::bindgen_prelude::{FromNapiValue, Function, FunctionCallContext, JsValue, Object};
use napi::{Env, Error, Result};
use napi_helpers::{
    anything::Anything,
    inherits::{Constructed, RootLayer, Super, proc::layer},
};

use crate::dom::{layers::element::ElementLayer, shared::doc::SharedDocument};
use blitz::dom::NodeId;

/// The spec members on `CSSStyleDeclaration` that must not be
/// intercepted as CSS properties by the proxy.
const RESERVED: &[&str] = &[
    "cssText",
    "length",
    "getPropertyValue",
    "setProperty",
    "removeProperty",
    "item",
    "toString",
    "valueOf",
    "constructor",
    "then",
];

/// camelCase JS identifiers -> kebab-case CSS property names. Leaves
/// names that already contain a hyphen, start with `--`, or carry a
/// vendor prefix untouched.
fn camel_to_kebab(name: &str) -> String {
    if name.starts_with("--") || name.contains('-') {
        return name.to_owned();
    }
    let mut out = String::with_capacity(name.len() + 4);
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            out.push('-');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    if out.starts_with("webkit-")
        || out.starts_with("moz-")
        || out.starts_with("ms-")
        || out.starts_with("o-")
    {
        out.insert(0, '-');
    }
    out
}

/// The CSSOM method functions of one element's `style` object. Each is
/// created on first access and reused, so repeated reads return the
/// same function object.
#[derive(Default)]
pub(crate) struct StyleMethods {
    get_property_value: Option<Anything>,
    set_property: Option<Anything>,
    remove_property: Option<Anything>,
    item: Option<Anything>,
}

/// One of the four CSSOM methods, selecting the slot and the builder.
enum CssomMethod {
    GetPropertyValue,
    SetProperty,
    RemoveProperty,
    Item,
}

/// Build one CSSOM method function capturing the element's identity.
fn build_method(
    env: &Env,
    which: CssomMethod,
    node_id: NodeId,
    doc: Rc<SharedDocument>,
) -> Result<Anything> {
    let wrapped = match which {
        CssomMethod::GetPropertyValue => {
            let doc = Rc::clone(&doc);
            let f: Function<'_, (), String> = env.create_function_from_closure(
                "getPropertyValue",
                move |ctx: FunctionCallContext| -> Result<String> {
                    let (prop,) = ctx.args::<(String,)>()?;
                    Ok(
                        ElementLayer::style_property(&doc, node_id, &camel_to_kebab(&prop))
                            .unwrap_or_default(),
                    )
                },
            )?;
            wrap_fn(env, f)?
        }
        CssomMethod::SetProperty => {
            let doc = Rc::clone(&doc);
            let f: Function<'_, (), ()> = env.create_function_from_closure(
                "setProperty",
                move |ctx: FunctionCallContext| -> Result<()> {
                    let (prop, value) = ctx.args::<(String, String)>()?;
                    ElementLayer::style_set(&doc, node_id, &camel_to_kebab(&prop), &value);
                    Ok(())
                },
            )?;
            wrap_fn(env, f)?
        }
        CssomMethod::RemoveProperty => {
            let doc = Rc::clone(&doc);
            let f: Function<'_, (), String> = env.create_function_from_closure(
                "removeProperty",
                move |ctx: FunctionCallContext| -> Result<String> {
                    let (prop,) = ctx.args::<(String,)>()?;
                    let css = camel_to_kebab(&prop);
                    let previous =
                        ElementLayer::style_property(&doc, node_id, &css).unwrap_or_default();
                    ElementLayer::style_remove(&doc, node_id, &css);
                    Ok(previous)
                },
            )?;
            wrap_fn(env, f)?
        }
        CssomMethod::Item => {
            let doc = Rc::clone(&doc);
            let f: Function<'_, (), String> = env.create_function_from_closure(
                "item",
                move |ctx: FunctionCallContext| -> Result<String> {
                    let (index,) = ctx.args::<(u32,)>()?;
                    Ok(ElementLayer::style_property_names(&doc, node_id)
                        .get(index as usize)
                        .cloned()
                        .unwrap_or_default())
                },
            )?;
            wrap_fn(env, f)?
        }
    };
    Ok(wrapped)
}

fn wrap_fn<R>(env: &Env, f: Function<'_, (), R>) -> Result<Anything> {
    unsafe { Anything::from_napi_value(env.raw(), JsValue::raw(&f)) }
}

/// Own block of the `style` Proxy handler. Holds the element it serves
/// and its lazily-built CSSOM method slots.
#[layer]
pub struct StyleHandlerLayer {
    pub(crate) node_id: NodeId,
    pub(crate) shared_doc: Rc<SharedDocument>,
    pub(crate) methods: RefCell<StyleMethods>,
}

#[layer(js_name = "StyleHandler")]
impl StyleHandlerLayer {
    #[layer(parent)]
    type Parent = RootLayer;

    #[layer(constructor)]
    fn build(_sup: Super<RootLayer>) -> Result<Constructed<Self>> {
        Err(Error::from_reason(
            "StyleHandler cannot be constructed directly; it backs el.style",
        ))
    }

    /// The `get` trap: spec members (`cssText`, `length`, CSSOM
    /// methods) resolve to their values; anything else is read as a CSS
    /// property, camelCase mapped to kebab-case. Missing properties
    /// read as `""`, matching CSSOM.
    #[layer]
    fn get(
        &self,
        env: &Env,
        _target: Anything,
        prop: Anything,
        _receiver: Anything,
    ) -> Result<Anything> {
        let Anything::String(name) = prop else {
            return Ok(Anything::Undefined);
        };
        Ok(self
            .property_slot(env, &name)?
            .unwrap_or(Anything::String(String::new())))
    }

    /// The `getOwnPropertyDescriptor` trap: `Object.keys` walks the
    /// `ownKeys` result and asks for each key's descriptor, so present
    /// properties must answer with an enumerable descriptor.
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
        let Some(value) = self.property_slot(env, &name)? else {
            return Ok(Anything::Undefined);
        };
        let mut desc = Object::new(env)?;
        desc.set("value", value)?;
        desc.set("writable", true)?;
        desc.set("enumerable", true)?;
        desc.set("configurable", true)?;
        unsafe { Anything::from_napi_value(env.raw(), JsValue::raw(&desc)) }
    }

    /// The `property_slot`: the value of a present property (spec member
    /// or CSS property), `None` when absent.
    fn property_slot(&self, env: &Env, name: &str) -> Result<Option<Anything>> {
        match name {
            "cssText" => Ok(Some(Anything::String(ElementLayer::style_css_text(
                &self.shared_doc,
                self.node_id,
            )))),
            "length" => Ok(Some(Anything::Number(
                ElementLayer::style_property_names(&self.shared_doc, self.node_id).len() as f64,
            ))),
            "getPropertyValue" => Ok(Some(self.cssom_method(env, CssomMethod::GetPropertyValue)?)),
            "setProperty" => Ok(Some(self.cssom_method(env, CssomMethod::SetProperty)?)),
            "removeProperty" => Ok(Some(self.cssom_method(env, CssomMethod::RemoveProperty)?)),
            "item" => Ok(Some(self.cssom_method(env, CssomMethod::Item)?)),
            _ => {
                // Numeric string indices follow `item(n)`.
                if name.chars().all(|c| c.is_ascii_digit()) {
                    if let Ok(index) = name.parse::<usize>() {
                        let names =
                            ElementLayer::style_property_names(&self.shared_doc, self.node_id);
                        return Ok(names.get(index).cloned().map(Anything::String));
                    }
                    return Ok(None);
                }
                Ok(ElementLayer::style_property(
                    &self.shared_doc,
                    self.node_id,
                    &camel_to_kebab(name),
                )
                .map(Anything::String))
            }
        }
    }

    /// The lazily-built slot of one CSSOM method: build on first
    /// access, reuse afterwards.
    fn cssom_method(&self, env: &Env, which: CssomMethod) -> Result<Anything> {
        let mut methods = self.methods.borrow_mut();
        let slot = match which {
            CssomMethod::GetPropertyValue => &mut methods.get_property_value,
            CssomMethod::SetProperty => &mut methods.set_property,
            CssomMethod::RemoveProperty => &mut methods.remove_property,
            CssomMethod::Item => &mut methods.item,
        };
        if let Some(f) = slot.as_ref() {
            return Ok(f.clone());
        }
        let f = build_method(env, which, self.node_id, Rc::clone(&self.shared_doc))?;
        *slot = Some(f.clone());
        Ok(f)
    }

    /// The `set` trap: writes a CSS property (camelCase or kebab),
    /// except `cssText`, which replaces the whole block. Spec members
    /// are read-only; setting them is ignored.
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
        if name == "cssText" {
            // Replace the entire block: clear, then re-parse.
            for existing in ElementLayer::style_property_names(&self.shared_doc, self.node_id) {
                ElementLayer::style_remove(&self.shared_doc, self.node_id, &existing);
            }
            for decl in value.split(';') {
                let Some((name, value)) = decl.split_once(':') else {
                    continue;
                };
                ElementLayer::style_set(&self.shared_doc, self.node_id, name.trim(), value.trim());
            }
            return Ok(true);
        }
        if RESERVED.contains(&name.as_str()) {
            return Ok(false);
        }
        ElementLayer::style_set(
            &self.shared_doc,
            self.node_id,
            &camel_to_kebab(&name),
            &value,
        );
        Ok(true)
    }

    /// The `has` trap: spec members are always present; a CSS property
    /// is present when it exists in the block.
    #[layer]
    fn has(&self, _target: Anything, prop: Anything) -> Result<bool> {
        let Anything::String(name) = prop else {
            return Ok(false);
        };
        if RESERVED.contains(&name.as_str()) {
            return Ok(true);
        }
        Ok(
            ElementLayer::style_property(&self.shared_doc, self.node_id, &camel_to_kebab(&name))
                .is_some(),
        )
    }

    /// The `deleteProperty` trap: removes the property's declaration.
    #[layer]
    fn delete_property(&mut self, _target: Anything, prop: Anything) -> Result<bool> {
        let Anything::String(name) = prop else {
            return Ok(false);
        };
        if RESERVED.contains(&name.as_str()) {
            return Ok(false);
        }
        ElementLayer::style_remove(&self.shared_doc, self.node_id, &camel_to_kebab(&name));
        Ok(true)
    }

    /// The `ownKeys` trap: the canonical (kebab-case) declaration
    /// names, in declaration order.
    #[layer]
    fn own_keys(&self, _target: Anything) -> Result<Vec<String>> {
        Ok(ElementLayer::style_property_names(
            &self.shared_doc,
            self.node_id,
        ))
    }
}
