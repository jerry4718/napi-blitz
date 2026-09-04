//! Small naming and parsing helpers shared across the layer expansions.

use proc_macro2::Ident;
use syn::{Attribute, Lit, Meta, Type};

/// CamelCase a snake_case Rust name (`set_foo` -> `setFoo`).
pub(crate) fn to_camel(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut upper = false;
    for c in s.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.push(c.to_ascii_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// The JS property name of a setter: the `set_` prefix (if any) is dropped
/// so the setter lands on the same property as its getter (named after the
/// property itself, e.g. `counter` / `set_counter`).
pub(crate) fn setter_js_name(name: &Ident) -> String {
    let n = name.to_string();
    to_camel(n.strip_prefix("set_").unwrap_or(&n))
}

/// Doc comments on an item, in order.
pub(crate) fn extract_doc(attrs: &[Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter_map(|a| {
            let Meta::NameValue(nv) = &a.meta else {
                return None;
            };
            let name = nv.path.get_ident()?.to_string();
            if name != "doc" {
                return None;
            }
            let syn::Expr::Lit(expr) = &nv.value else {
                return None;
            };
            let Lit::Str(s) = &expr.lit else { return None };
            Some(s.value())
        })
        .collect()
}

/// Last path segment of a (possibly reference) type, e.g. `Object` for
/// `&Object` and `Result<String>` for `napi::Result<String>`.
pub(crate) fn type_last_ident(ty: &Type) -> Option<Ident> {
    let mut ty = ty;
    while let Type::Reference(r) = ty {
        ty = &r.elem;
    }
    let Type::Path(p) = ty else { return None };
    p.path.segments.last().map(|s| s.ident.clone())
}

/// The self type's ident of a `#[layer] impl` block.
pub(crate) fn self_ident(ty: &Type) -> syn::Result<Ident> {
    let Type::Path(p) = ty else {
        return Err(syn::Error::new_spanned(
            ty,
            "#[layer] impl must have a simple self type",
        ));
    };
    p.path
        .segments
        .last()
        .map(|s| s.ident.clone())
        .ok_or_else(|| syn::Error::new_spanned(ty, "#[layer] impl must name a layer struct"))
}
