//! `#[layer(...)]` attribute parsing — impl-level flags and member flags.
//! Each flag set implements `syn::parse::Parse` so the attribute arguments
//! are consumed through the standard `attr.parse_args` path instead of
//! hand-rolled `Punctuated` loops.

use syn::{
    Attribute, Meta, Token,
    parse::{Parse, ParseStream, Parser},
    punctuated::Punctuated,
};

/// The `#[layer(...)]` attribute on an impl block: the JS class name
/// (`#[layer(js_name = "...")]`, defaulting to the Rust ident), and the two
/// alias types exported next to the class — the constructor wrapper
/// (`class_type`, defaulting to `ClassType`) and the instance type
/// (`instance_type`, defaulting to the built-in `InstanceType`).
#[derive(Default)]
pub(crate) struct LayerAttrs {
    pub js_name: Option<String>,
    pub class_type: Option<String>,
    pub instance_type: Option<String>,
}

impl Parse for LayerAttrs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut out = Self::default();
        let metas = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;
        for meta in metas {
            let Meta::NameValue(nv) = meta else { continue };
            let Some(ident) = nv.path.get_ident() else {
                continue;
            };
            let Some(s) = (match &nv.value {
                syn::Expr::Lit(expr) => match &expr.lit {
                    syn::Lit::Str(s) => Some(s),
                    _ => None,
                },
                _ => None,
            }) else {
                continue;
            };
            match ident.to_string().as_str() {
                "js_name" => out.js_name = Some(s.value()),
                "class_type" => out.class_type = Some(s.value()),
                "instance_type" => out.instance_type = Some(s.value()),
                _ => {}
            }
        }
        Ok(out)
    }
}

/// What a `#[layer(...)]`-annotated impl member is.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum MemberKind {
    Constructor,
    Getter,
    Setter,
    Method,
    /// `#[layer(generator)]` — the member's snapshot becomes the class's
    /// `Symbol.iterator`, so `for...of` and spread iterate the instance.
    Generator,
    /// `#[layer(async_generator)]` — same snapshot shape, exposed as
    /// `Symbol.asyncIterator` for `for await...of`.
    AsyncGenerator,
}

/// The parsed flags of one `#[layer(...)]` member attribute.
struct MemberFlags {
    kind: MemberKind,
    /// `#[layer(this)]` — inject the JS instance although there is no
    /// receiver. Without the flag a receiver-less member is a plain static.
    this_injectable: bool,
    /// `#[layer(getter, js_name = "...")]` — explicit JS member name for
    /// names `to_camel` cannot express (e.g. `innerHTML`).
    js_name: Option<String>,
}

impl Parse for MemberFlags {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut kind = MemberKind::Method;
        let mut this_injectable = false;
        let mut js_name = None;
        let metas = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;
        for meta in metas {
            match &meta {
                Meta::Path(p) => {
                    if let Some(i) = p.get_ident() {
                        match i.to_string().as_str() {
                            "constructor" => kind = MemberKind::Constructor,
                            "getter" => kind = MemberKind::Getter,
                            "setter" => kind = MemberKind::Setter,
                            "generator" => kind = MemberKind::Generator,
                            "async_generator" => kind = MemberKind::AsyncGenerator,
                            "this" => this_injectable = true,
                            _ => {}
                        }
                    }
                }
                Meta::NameValue(nv) => {
                    if nv.path.is_ident("js_name")
                        && let syn::Expr::Lit(lit) = &nv.value
                        && let syn::Lit::Str(s) = &lit.lit
                    {
                        js_name = Some(s.value());
                    }
                }
                _ => {}
            }
        }
        Ok(Self {
            kind,
            this_injectable,
            js_name,
        })
    }
}

/// The parsed flags of a `#[layer(...)]`-annotated impl member.
pub(crate) struct MemberInfo {
    pub kind: MemberKind,
    pub this_injectable: bool,
    pub js_name: Option<String>,
}

impl MemberInfo {
    /// Look for a `#[layer(...)]` attribute on a method, record its kind, and
    /// remove the attribute so the item passes through cleanly.
    pub fn parse(attrs: &mut Vec<Attribute>) -> Option<Self> {
        let mut out = None;
        attrs.retain(|a| {
            if !a.path().is_ident("layer") {
                return true;
            }
            match &a.meta {
                Meta::List(_) => {
                    if let Ok(flags) = a.parse_args::<MemberFlags>() {
                        out = Some(Self {
                            kind: flags.kind,
                            this_injectable: flags.this_injectable,
                            js_name: flags.js_name,
                        });
                    }
                }
                // `#[layer]` with no arguments is a plain method member.
                _ => {
                    out = Some(Self {
                        kind: MemberKind::Method,
                        this_injectable: false,
                        js_name: None,
                    })
                }
            }
            false
        });
        out
    }
}

/// `#[layer(getter, setter, js_name = "...")]` on a field.
struct FieldFlags {
    getter: bool,
    setter: bool,
    js_name: Option<String>,
}

impl Parse for FieldFlags {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut getter = false;
        let mut setter = false;
        let mut js_name = None;
        let metas = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;
        for meta in metas {
            match meta {
                Meta::Path(p) => match p.get_ident().map(|i| i.to_string()).as_deref() {
                    Some("getter") => getter = true,
                    Some("setter") => setter = true,
                    _ => {}
                },
                Meta::NameValue(nv)
                    if nv.path.get_ident().map(|i| i == "js_name").unwrap_or(false) =>
                {
                    if let syn::Expr::Lit(expr) = &nv.value
                        && let syn::Lit::Str(s) = &expr.lit
                    {
                        js_name = Some(s.value());
                    }
                }
                _ => {}
            }
        }
        Ok(Self {
            getter,
            setter,
            js_name,
        })
    }
}

/// Parse a field's `#[layer(getter)]` / `#[layer(setter)]` /
/// `#[layer(getter, setter)]` attribute plus an optional `js_name = "..."`
/// (shared by the getter/setter pair) and remove it (an unparsed
/// `#[layer(...)]` would otherwise be an unknown attribute on the field).
/// Absent `#[layer(...)]` means the field is not exposed at all.
pub(crate) fn field_flags(attrs: &mut Vec<Attribute>) -> (bool, bool, Option<String>) {
    let mut getter = false;
    let mut setter = false;
    let mut js_name = None;
    attrs.retain(|a| {
        if !a.path().is_ident("layer") {
            return true;
        }
        if let Ok(flags) = a.parse_args::<FieldFlags>() {
            getter |= flags.getter;
            setter |= flags.setter;
            if let Some(n) = flags.js_name {
                js_name = Some(n);
            }
        }
        false
    });
    (getter, setter, js_name)
}

/// Whether an attribute is `#[layer(flag)]` with the given flag ident.
pub(crate) fn layer_attr_has_flag(attr: &Attribute, flag: &str) -> bool {
    if !attr.path().is_ident("layer") {
        return false;
    }
    let Meta::List(list) = &attr.meta else {
        return false;
    };
    Punctuated::<Meta, Token![,]>::parse_terminated
        .parse2(list.tokens.clone())
        .map(|metas| {
            metas.iter().any(|m| {
                matches!(
                    m,
                    Meta::Path(p) if p.get_ident().map(|i| i == flag).unwrap_or(false)
                )
            })
        })
        .unwrap_or(false)
}
