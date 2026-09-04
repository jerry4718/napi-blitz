//! `#[layer]` procedural macro.
//!
//! Declares one layer of a napi inheritance chain. On expansion it emits the
//! layer's TypeScript type definitions into the napi-rs type-def JSONL file
//! that `@napi-rs/cli` renders into `index.d.ts`:
//!
//! - a class declaration (`NapiStruct`, kind `struct`) - getters come from the
//!   layer struct's public fields;
//! - the impl block (`NapiImpl`, kind `impl`) - constructor / getter / method /
//!   static members annotated with `#[layer(...)]`; the CLI merges this into
//!   the class declaration;
//! - an `extends` link (`TypeDef` kind `extends`) pointing at the parent
//!   layer's JS name, when `parent = "..."` is given.
//!
//! The item itself is passed through unchanged (minus the `#[layer]` attribute).

use std::{
    collections::HashMap,
    env, fs,
    io::Write,
    path::PathBuf,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use napi_derive_backend::{
    FnKind, FnSelf, NapiClass, NapiFn, NapiFnArg, NapiFnArgKind, NapiImpl, NapiStruct,
    NapiStructField, NapiStructKind, ToTypeDef, TypeDef,
};
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::{ToTokens, format_ident, quote};
use syn::{
    Attribute, FnArg, ImplItem, ItemImpl, ItemStruct, Lit, Meta, ReturnType, Type, Visibility,
    parse::Parser,
};

// ── type-def file output (mirrors napi-derive's private output_type_def) ──

static BUILT_FLAG: AtomicBool = AtomicBool::new(false);

fn type_def_file() -> Option<PathBuf> {
    let folder = env::var("NAPI_TYPE_DEF_TMP_FOLDER").ok()?;
    let pkg = env::var("CARGO_PKG_NAME").ok()?;
    // Independent JSONL so napi-derive's own file (same `CARGO_PKG_NAME`)
    // and this macro's "clear on first expansion" never clobber each other.
    Some(PathBuf::from(folder).join(format!("{pkg}.layer")))
}

/// Append one serialized `TypeDef` to the CLI's intermediate JSONL file.
/// The first expansion of a build clears the stale file.
fn output_type_def(def: &TypeDef) {
    let Some(file) = type_def_file() else { return };
    if BUILT_FLAG
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_ok()
    {
        let _ = fs::remove_file(&file);
    }
    if let Ok(mut f) = fs::OpenOptions::new().append(true).create(true).open(&file) {
        let _ = writeln!(f, "{}", def);
    }
}

// ── layer registry: rust ident -> compile-time layer metadata ────────────

/// One public field of a layer struct and whether it is exposed to JS as a
/// getter and/or setter (both `false` = not exposed at all). Defaults to
/// nothing; the user opts in per-field with `#[layer(getter)]` /
/// `#[layer(setter)]` / `#[layer(getter, setter)]`.
#[derive(Clone)]
struct FieldMeta {
    name: String,
    ty: String,
    getter: bool,
    setter: bool,
}

/// Everything the macro learns about one layer across its two expansions
/// (struct first, then impl): the JS class name, the parent layer's Rust
/// type, and the public fields. Types are stored as strings (`syn` items
/// are not `Send` and cannot live in a `static`).
#[derive(Clone)]
struct LayerMeta {
    js_name: String,
    parent_ty: Option<String>,
}

static LAYER_REGISTRY: OnceLock<Mutex<HashMap<String, LayerMeta>>> = OnceLock::new();

fn layer_registry() -> &'static Mutex<HashMap<String, LayerMeta>> {
    LAYER_REGISTRY.get_or_init(Default::default)
}

/// Resolve a layer reference written as a Rust ident (or already a JS name)
/// to its JS class name through the registry.
fn resolve_js_name(rust_or_js: &str) -> String {
    layer_registry()
        .lock()
        .unwrap()
        .get(rust_or_js)
        .map(|m| m.js_name.clone())
        .unwrap_or_else(|| rust_or_js.to_owned())
}

fn type_last_ident(ty: &Type) -> Option<Ident> {
    let mut ty = ty;
    while let Type::Reference(r) = ty {
        ty = &r.elem;
    }
    let Type::Path(p) = ty else { return None };
    p.path.segments.last().map(|s| s.ident.clone())
}

// ── attribute parsing ────────────────────────────────────────────────────

#[derive(Default)]
struct LayerAttrs {
    js_name: Option<String>,
}

impl LayerAttrs {
    fn parse(ts: TokenStream2) -> syn::Result<Self> {
        let mut out = Self::default();
        if ts.is_empty() {
            return Ok(out);
        }
        let metas =
            syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated.parse2(ts)?;
        for meta in metas {
            let Meta::NameValue(nv) = meta else { continue };
            let Some(name) = nv.path.get_ident().map(|i| i.to_string()) else {
                continue;
            };
            if name == "js_name"
                && let syn::Expr::Lit(expr) = &nv.value
                && let Lit::Str(s) = &expr.lit
            {
                out.js_name = Some(s.value());
            }
        }
        Ok(out)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum MemberKind {
    Constructor,
    Getter,
    Setter,
    Method,
}

/// Parse a field's `#[layer(getter)]` / `#[layer(setter)]` /
/// `#[layer(getter, setter)]` attribute and remove it (an unparsed
/// `#[layer(...)]` would otherwise be an unknown attribute on the field).
/// Absent `#[layer(...)]` means the field is not exposed at all.
fn field_flags(attrs: &mut Vec<Attribute>) -> (bool, bool) {
    let mut getter = false;
    let mut setter = false;
    attrs.retain(|a| {
        if !a.path().is_ident("layer") {
            return true;
        }
        if let Meta::List(list) = &a.meta
            && let Ok(nested) =
                syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated
                    .parse2(list.tokens.clone())
        {
            for meta in nested {
                if let Meta::Path(p) = meta
                    && let Some(i) = p.get_ident()
                {
                    match i.to_string().as_str() {
                        "getter" => getter = true,
                        "setter" => setter = true,
                        _ => {}
                    }
                }
            }
        }
        false
    });
    (getter, setter)
}

/// Whether an attribute is `#[layer(flag)]` with the given flag ident.
fn layer_attr_has_flag(attr: &Attribute, flag: &str) -> bool {
    if !attr.path().is_ident("layer") {
        return false;
    }
    let Meta::List(list) = &attr.meta else {
        return false;
    };
    syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated
        .parse2(list.tokens.clone())
        .map(|metas| {
            metas.iter().any(
                |m| matches!(m, Meta::Path(p) if p.get_ident().map(|i| i == flag).unwrap_or(false)),
            )
        })
        .unwrap_or(false)
}

/// A `#[layer(...)]`-annotated impl member.
struct MemberInfo {
    kind: MemberKind,
}

impl MemberKind {
    /// Look for a `#[layer(...)]` attribute on a method, record its kind, and
    /// remove the attribute so the item passes through cleanly.
    fn parse(attrs: &mut Vec<Attribute>) -> Option<MemberInfo> {
        let mut out = None;
        attrs.retain(|a| {
            if !a.path().is_ident("layer") {
                return true;
            }
            let mut kind = MemberKind::Method;
            if let Meta::List(list) = &a.meta
                && let Ok(nested) =
                    syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated
                        .parse2(list.tokens.clone())
            {
                for meta in nested {
                    if let Meta::Path(p) = meta
                        && let Some(i) = p.get_ident()
                    {
                        kind = match i.to_string().as_str() {
                            "constructor" => MemberKind::Constructor,
                            "getter" => MemberKind::Getter,
                            "setter" => MemberKind::Setter,
                            _ => kind,
                        };
                    }
                }
            }
            out = Some(MemberInfo { kind });
            false
        });
        out
    }
}

// ── small helpers ────────────────────────────────────────────────────────

fn to_camel(s: &str) -> String {
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
fn setter_js_name(name: &Ident) -> String {
    let n = name.to_string();
    to_camel(n.strip_prefix("set_").unwrap_or(&n))
}

fn extract_doc(attrs: &[Attribute]) -> Vec<String> {
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

fn self_ident(ty: &Type) -> syn::Result<Ident> {
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

// ── type def construction ────────────────────────────────────────────────

fn build_class_def(s: &ItemStruct, js_name: &str, fields: &[FieldMeta]) -> NapiStruct {
    let fields = fields
        .iter()
        .map(|fm| {
            let field_ident = Ident::new(&fm.name, Span::call_site());
            NapiStructField {
                name: syn::Member::Named(field_ident.clone()),
                js_name: to_camel(&fm.name),
                ty: syn::parse_str(&fm.ty).expect("invalid field type"),
                getter: fm.getter,
                setter: fm.setter,
                writable: false,
                enumerable: false,
                configurable: false,
                comments: vec![],
                skip_typescript: false,
                ts_type: None,
                has_lifetime: false,
            }
        })
        .collect();
    NapiStruct {
        // `name` feeds `original_name` in the backend's `to_type_def`; if it
        // differs from `js_name` the CLI also exports the Rust alias, which
        // has no runtime binding (`module.exports.BaseLayer` would be
        // `undefined`). Using the JS name keeps `original_name == name` so no
        // alias export is emitted.
        name: format_ident!("{}", js_name),
        js_name: js_name.to_owned(),
        comments: extract_doc(&s.attrs),
        js_mod: None,
        use_nullable: false,
        register_name: format_ident!("__layer_placeholder"),
        kind: NapiStructKind::Class(NapiClass {
            fields,
            ctor: false,
            implement_iterator: false,
            implement_async_iterator: false,
            is_tuple: false,
            use_custom_finalize: false,
        }),
        has_lifetime: false,
        is_generator: false,
        is_async_generator: false,
        type_tag: None,
    }
}

/// The constructor's declared signature. `&Env` and `&Object` parameters are
/// bound as the optional `env` and `this` (the instance, which may be a JS
/// subclass instance); the remaining parameters are this layer's constructor
/// args - typed.
/// One entry of the `#[layer(constructor)]` build signature: a real
/// constructor parameter (ancestor's or this layer's own, in argument
/// order), or one of the injected handles (`env`, `sup`).
enum BuildArg {
    Param(Ident, Box<Type>),
    Env,
    Sup,
}

struct BuildParams {
    /// The full signature in order, injected handles included.
    args: Vec<BuildArg>,
}

impl BuildParams {
    /// The real constructor parameters, injected handles removed.
    fn params(&self) -> Vec<(Ident, Box<Type>)> {
        self.args
            .iter()
            .filter_map(|a| match a {
                BuildArg::Param(id, ty) => Some((id.clone(), ty.clone())),
                _ => None,
            })
            .collect()
    }
}

fn analyze_build(f: &syn::ImplItemFn) -> BuildParams {
    let mut args = vec![];
    for a in &f.sig.inputs {
        let FnArg::Typed(pat) = a else { continue };
        let is = |expected: &str| {
            type_last_ident(&pat.ty)
                .map(|i| i == expected)
                .unwrap_or(false)
        };
        if is("Env") {
            args.push(BuildArg::Env);
        } else if is("Super") {
            args.push(BuildArg::Sup);
        } else {
            let name = match &*pat.pat {
                syn::Pat::Ident(pi) => pi.ident.clone(),
                _ => format_ident!("p{}", args.len()),
            };
            args.push(BuildArg::Param(name, Box::from((*pat.ty).clone())));
        }
    }
    BuildParams { args }
}

fn member_napi_fn(
    f: &syn::ImplItemFn,
    kind: MemberKind,
    ts_params: &[(Ident, Box<Type>)],
    parent: &Ident,
    parent_js: &str,
) -> NapiFn {
    let name = f.sig.ident.clone();
    let args = if kind == MemberKind::Constructor {
        ts_params
            .iter()
            .map(|(pname, ty)| NapiFnArg {
                kind: NapiFnArgKind::PatType(Box::new(syn::PatType {
                    attrs: vec![],
                    pat: Box::new(syn::Pat::Ident(syn::PatIdent {
                        attrs: vec![],
                        by_ref: None,
                        mutability: None,
                        ident: pname.clone(),
                        subpat: None,
                    })),
                    colon_token: Default::default(),
                    ty: Box::new(*ty.clone()),
                })),
                ts_arg_type: None,
            })
            .collect()
    } else {
        f.sig
            .inputs
            .iter()
            .filter_map(|a| match a {
                FnArg::Typed(pat) => Some(NapiFnArg {
                    kind: NapiFnArgKind::PatType(Box::new(pat.clone())),
                    ts_arg_type: None,
                }),
                FnArg::Receiver(_) => None,
            })
            .collect()
    };
    let ret = match &f.sig.output {
        ReturnType::Default => None,
        ReturnType::Type(_, ty) => Some((**ty).clone()),
    };
    let (kind_, js_name) = match kind {
        MemberKind::Constructor => (FnKind::Constructor, "constructor".to_owned()),
        MemberKind::Getter => (FnKind::Getter, to_camel(&name.to_string())),
        MemberKind::Setter => (FnKind::Setter, setter_js_name(&name)),
        MemberKind::Method => (FnKind::Normal, to_camel(&name.to_string())),
    };
    // Instance vs static is derived from the signature: `&self`/`&mut self`
    // receiver means an instance member, no receiver means a static one.
    let fn_self = f.sig.receiver().map(|recv| {
        if recv.reference.is_some() {
            if recv.mutability.is_some() {
                FnSelf::MutRef
            } else {
                FnSelf::Ref
            }
        } else {
            FnSelf::Value
        }
    });
    NapiFn {
        name,
        js_name,
        module_exports: false,
        attrs: vec![],
        args,
        ret,
        is_ret_result: false,
        is_async: false,
        within_async_runtime: false,
        fn_self,
        kind: kind_,
        vis: f.vis.clone(),
        parent: Some(parent.clone()),
        parent_js_name: Some(parent_js.to_owned()),
        strict: false,
        return_if_invalid: false,
        js_mod: None,
        ts_generic_types: None,
        ts_type: None,
        ts_args_type: None,
        ts_return_type: None,
        skip_typescript: false,
        comments: extract_doc(&f.attrs),
        parent_is_generator: false,
        parent_is_async_generator: false,
        writable: false,
        enumerable: false,
        configurable: false,
        catch_unwind: false,
        unsafe_: false,
        register_name: format_ident!("__layer_placeholder"),
        no_export: false,
    }
}

// ── expansion ────────────────────────────────────────────────────────────

fn expand_struct(s: &ItemStruct, attrs: &LayerAttrs) -> syn::Result<TokenStream2> {
    let js_name = attrs.js_name.clone().unwrap_or_else(|| s.ident.to_string());
    let mut s = s.clone();
    let fields: Vec<FieldMeta> = s
        .fields
        .iter_mut()
        .filter(|f| matches!(f.vis, Visibility::Public(_)))
        .map(|f| {
            let id = f.ident.clone().expect("named field");
            let (getter, setter) = field_flags(&mut f.attrs);
            FieldMeta {
                name: id.to_string(),
                ty: f.ty.to_token_stream().to_string(),
                getter,
                setter,
            }
        })
        .collect();
    if let Some(def) = build_class_def(&s, &js_name, &fields).to_type_def() {
        output_type_def(&def);
    }
    // The struct block self-sufficiently generates the LayerAccessors
    // implementation: field getter/setter pairs are decided solely by the
    // fields and their `#[layer(getter)]` / `#[layer(setter)]` flags here,
    // so the impl block's expansion never has to read the field list.
    let self_ty = s.ident.clone();
    let field_getters = fields.iter().filter(|f| f.getter).map(|f| {
        let ident = Ident::new(&f.name, Span::call_site());
        let field_js = to_camel(&f.name);
        quote! {
            napi_helpers::inherits::define_getter(proto, #field_js, |_ctx, this| {
                napi_helpers::inherits::with_own::<#self_ty, _>(&this, |d| d.#ident)
            })?;
        }
    });
    let field_setters = fields.iter().filter(|f| f.setter).map(|f| {
        let ident = Ident::new(&f.name, Span::call_site());
        let field_js = to_camel(&f.name);
        let ty: Type = syn::parse_str(&f.ty).expect("invalid field type");
        quote! {
            napi_helpers::inherits::define_setter(proto, #field_js, |_env, this, value: #ty| {
                napi_helpers::inherits::with_own_mut::<#self_ty, _>(&this, |d| d.#ident = value)
            })?;
        }
    });
    let accessors = quote! {
        impl napi_helpers::inherits::LayerAccessors for #self_ty {
            fn define_accessors(
                _env: &napi::Env,
                proto: &mut napi::bindgen_prelude::Object,
            ) -> napi::Result<()> {
                #(#field_getters)*
                #(#field_setters)*
                Ok(())
            }
        }
    };
    layer_registry().lock().unwrap().insert(
        s.ident.to_string(),
        LayerMeta {
            js_name: js_name.clone(),
            parent_ty: None,
        },
    );
    Ok(quote! { #s #accessors })
}

// ── runtime codegen ──────────────────────────────────────────────────────

/// One member's define_members call. Instance members read their layer data
/// through `with_own`; static members (no receiver) call the implementation
/// directly. Everything here lands in `LayerMembers::define_members` - the
/// `LayerAccessors` side is generated by the struct block alone.
fn member_define_tokens(self_ty: &Ident, f: &syn::ImplItemFn, kind: MemberKind) -> TokenStream2 {
    let name = f.sig.ident.clone();
    let js = to_camel(&name.to_string());
    // Split the signature into JS args and the injected receiver. A parameter
    // named `this` of type `Object` is filled by the runtime with the instance
    // object, so the body can reach any layer's own slot through
    // `with_own`/`with_own_mut` (the current layer's data is already handed to
    // the method as `&self`). `this` never participates in the JS args.
    //
    // A parameter named `env` of type `&Env` is filled by the runtime with the
    // current napi environment, so a getter/method can create JS values or
    // call back into JS without a global env. It also never participates in
    // the JS args.
    let mut normal: Vec<(usize, Type)> = vec![];
    let mut call: Vec<TokenStream2> = vec![];
    let mut has_this = false;
    let mut has_env = false;
    for a in &f.sig.inputs {
        let FnArg::Typed(pat) = a else { continue };
        let is_this = matches!(&*pat.pat, syn::Pat::Ident(pi) if pi.ident == "this")
            && type_last_ident(&pat.ty)
                .map(|i| i == "Object")
                .unwrap_or(false);
        let is_env = matches!(&*pat.pat, syn::Pat::Ident(pi) if pi.ident == "env")
            && type_last_ident(&pat.ty)
                .map(|i| i == "Env")
                .unwrap_or(false);
        if is_this {
            has_this = true;
            call.push(quote! { &this });
        } else if is_env {
            has_env = true;
            // The closure param `env` is an `Env` value; user methods declare
            // `env: &Env`, so hand over a reference.
            call.push(quote! { &env });
        } else {
            let i = normal.len();
            normal.push((i, (*pat.ty).clone()));
            let bind = format_ident!("a{}", i);
            call.push(quote! { #bind });
        }
    }
    let arg_binds = normal
        .iter()
        .map(|(i, ty)| {
            let bind = format_ident!("a{}", i);
            let is_option = type_last_ident(ty).map(|i| i == "Option").unwrap_or(false);
            if is_option {
                quote! {
                    let #bind: #ty = if #i < ctx.length() {
                        ctx.get(#i)?
                    } else {
                        None
                    };
                }
            } else {
                quote! { let #bind: #ty = ctx.get(#i)?; }
            }
        })
        .collect::<Vec<_>>();

    let ret_is_result = match &f.sig.output {
        ReturnType::Type(_, ty) => type_last_ident(ty).map(|i| i == "Result").unwrap_or(false),
        ReturnType::Default => false,
    };
    let result_tail = if ret_is_result {
        quote! { ? }
    } else {
        quote! {}
    };

    match kind {
        MemberKind::Getter => {
            if f.sig.receiver().is_some() {
                quote! {
                    napi_helpers::inherits::define_getter(proto, #js, |env, this| {
                        napi_helpers::inherits::with_own::<#self_ty, _>(&this, |d| #self_ty::#name(d, #(#call),*))#result_tail
                    })?;
                }
            } else if !has_this {
                let static_call = if ret_is_result {
                    quote! { #self_ty::#name(#(#call),*) }
                } else {
                    quote! { Ok(#self_ty::#name(#(#call),*)) }
                };
                quote! {
                    napi_helpers::inherits::define_static_getter(ctor, #js, |env| {
                        #static_call
                    })?;
                }
            } else {
                quote! {
                    napi_helpers::inherits::define_getter(proto, #js, |env, this| {
                        #self_ty::#name(#(#call),*)
                    })?;
                }
            }
        }
        MemberKind::Setter => {
            let js = setter_js_name(&name);
            let Some(value_ty) = normal.first().map(|(_, ty)| ty.clone()) else {
                return syn::Error::new_spanned(
                    f,
                    "a #[layer(setter)] method needs exactly one value parameter",
                )
                .into_compile_error();
            };
            let env_arg = if has_env {
                quote! { &env, }
            } else {
                quote! {}
            };
            if f.sig.receiver().is_some() {
                quote! {
                    napi_helpers::inherits::define_setter(proto, #js, |env, this, value: #value_ty| {
                        napi_helpers::inherits::with_own_mut::<#self_ty, _>(&this, |d| #self_ty::#name(d, #env_arg value))#result_tail
                    })?;
                }
            } else if !has_this {
                // A setter's method already returns `Result<()>`; the static
                // closure returns it as-is (no `?` - that would unwrap to
                // `()` and break the closure's `Result<()>` type).
                let static_call = if ret_is_result {
                    quote! { #self_ty::#name(#env_arg value) }
                } else {
                    quote! { Ok(#self_ty::#name(#env_arg value)) }
                };
                quote! {
                    napi_helpers::inherits::define_static_setter(ctor, #js, |env, value: #value_ty| {
                        #static_call
                    })?;
                }
            } else {
                let call = if ret_is_result {
                    quote! { #self_ty::#name(&this, #env_arg value) }
                } else {
                    quote! { Ok(#self_ty::#name(&this, #env_arg value)) }
                };
                quote! {
                    napi_helpers::inherits::define_setter(proto, #js, |env, this, value: #value_ty| {
                        #call
                    })?;
                }
            }
        }
        MemberKind::Constructor => unreachable!("constructor handled separately"),
        MemberKind::Method => {
            if f.sig.receiver().is_none() && !has_this {
                let static_call = if ret_is_result {
                    quote! { #self_ty::#name(#(#call),*) }
                } else {
                    quote! { Ok(#self_ty::#name(#(#call),*)) }
                };
                quote! {
                    napi_helpers::inherits::define_static_method(env, ctor, #js, |ctx| {
                        let env = *ctx.env;
                        #(#arg_binds)*
                        #static_call
                    })?;
                }
            } else if f.sig.receiver().is_some() {
                let takes_mut = f
                    .sig
                    .receiver()
                    .map(|r| r.mutability.is_some())
                    .unwrap_or(false);
                let slot = if takes_mut {
                    quote! { napi_helpers::inherits::with_own_mut::<#self_ty, _> }
                } else {
                    quote! { napi_helpers::inherits::with_own::<#self_ty, _> }
                };
                quote! {
                    napi_helpers::inherits::define_method(env, proto, #js, |ctx| {
                        let env = *ctx.env;
                        let this: napi::bindgen_prelude::Object = ctx.this()?;
                        #(#arg_binds)*
                        #slot(&this, |d| #self_ty::#name(d, #(#call),*))#result_tail
                    })?;
                }
            } else {
                // `this`-injected instance method: no receiver, the instance
                // object is handed over and the body reaches any layer's slot
                // through `with_own`/`with_own_mut` itself. No outer borrow,
                // so same-slot mutable access is fine.
                quote! {
                    napi_helpers::inherits::define_method(env, proto, #js, |ctx| {
                        let env = *ctx.env;
                        let this: napi::bindgen_prelude::Object = ctx.this()?;
                        #(#arg_binds)*
                        #self_ty::#name(#(#call),*)
                    })?;
                }
            }
        }
    }
}

fn gen_extend_layer_impl(
    self_ty: &Ident,
    js_name: &str,
    parent_ty: &Option<Type>,
    ctor_name: &Ident,
    build: &BuildParams,
    full_params_ty: &Vec<Box<Type>>,
    members: &[MemberFn],
    consts: &[(String, syn::Expr)],
    ctor_ret_is_result: bool,
) -> TokenStream2 {
    let parent_tokens = match parent_ty {
        Some(ty) => quote! { #ty },
        None => quote! { napi_helpers::inherits::RootLayer },
    };
    let member_tokens = members
        .iter()
        .map(|m| member_define_tokens(self_ty, &m.f, m.kind));
    let const_tokens = consts.iter().map(|(cname, cexpr)| {
        quote! {
            napi_helpers::inherits::define_static_value(env, ctor, #cname, #cexpr)?;
        }
    });
    // The full argument tuple comes straight from the build signature's real
    // parameters (ancestors' first). The generated `build` only destructures
    // it and forwards to the user's build, which receives the injected
    // `env` / `sup` handles and drives `sup.call` itself.
    let total = full_params_ty.len();
    let binds: Vec<Ident> = (0..total).map(|i| format_ident!("p{}", i)).collect();
    let args_tuple_ty = match total {
        0 => quote! { () },
        1 => {
            let t = &full_params_ty[0];
            quote! { (#t,) }
        }
        _ => quote! { ( #(#full_params_ty),* ) },
    };
    let destructure = match total {
        0 => quote! { let _ = args.data; },
        1 => {
            let b = &binds[0];
            quote! { let ( #b, ) = args.data; }
        }
        _ => quote! { let ( #(#binds),* ) = args.data; },
    };
    let has_env = build.args.iter().any(|a| matches!(a, BuildArg::Env));
    let env_param = if has_env {
        quote! { env: &'env napi::Env }
    } else {
        quote! { _env: &'env napi::Env }
    };
    let mut forward: Vec<TokenStream2> = vec![];
    let mut pidx = 0usize;
    for a in &build.args {
        match a {
            BuildArg::Env => forward.push(quote! { env }),
            BuildArg::Sup => forward.push(quote! { sup }),
            BuildArg::Param(_, _) => {
                let b = &binds[pidx];
                forward.push(quote! { #b });
                pidx += 1;
            }
        }
    }
    let ctor_call = if ctor_ret_is_result {
        quote! { #self_ty::#ctor_name(#(#forward),*) }
    } else {
        quote! { Ok(#self_ty::#ctor_name(#(#forward),*)) }
    };
    quote! {
        impl napi_helpers::inherits::LayerMembers for #self_ty {
            type Parent = #parent_tokens;
            const CLASS_NAME: &'static str = #js_name;

            fn define_members(
                env: &napi::Env,
                proto: &mut napi::bindgen_prelude::Object,
                ctor: &mut napi::bindgen_prelude::Object,
            ) -> napi::Result<()> {
                #(#member_tokens)*
                #(#const_tokens)*
                Ok(())
            }
        }

        impl napi_helpers::inherits::LayerBuild for #self_ty {
            type Args = #args_tuple_ty;

            fn build<'env>(
                #env_param,
                args: napi::bindgen_prelude::FnArgs<Self::Args>,
                sup: napi_helpers::inherits::Super<'_, 'env, Self::Parent>,
            ) -> napi::Result<napi_helpers::inherits::Constructed<Self>> {
                #destructure
                #ctor_call
            }
        }
    }
}

/// Mount the built class onto `module.exports`. `build_class` is idempotent
/// and builds the parent class lazily, so registration order needs no
/// ancestor handling here - `.init_array` ctor order does not matter.
fn gen_register(self_ty: &Ident, js_name: &str) -> TokenStream2 {
    let lower = self_ty.to_string().to_lowercase();
    let register_fn = format_ident!("__layer_register_{lower}");
    let export_fn = format_ident!("__layer_export_{lower}");
    quote! {
        #[cfg(all(not(test), not(target_family = "wasm")))]
        napi::ctor::declarative::ctor! {
            #[doc(hidden)]
            #[allow(clippy::all)]
            #[allow(non_snake_case)]
            #[ctor(unsafe)]
            fn #register_fn() {
                // The name must be NUL-terminated: napi parses it with
                // `CStr::from_bytes_with_nul_unchecked`.
                napi::bindgen_prelude::register_module_export(None, concat!(#js_name, "\0"), #export_fn);
            }
        }

        #[doc(hidden)]
        #[allow(clippy::all)]
        #[allow(non_snake_case)]
        unsafe fn #export_fn(
            env: napi::bindgen_prelude::sys::napi_env,
        ) -> napi::bindgen_prelude::Result<napi::bindgen_prelude::sys::napi_value> {
            let env = napi::Env::from(env);
            napi_helpers::inherits::build_class::<#self_ty>(&env)?;
            let (ctor, _) = napi_helpers::inherits::require(&env, std::any::TypeId::of::<#self_ty>())?;
            Ok(napi::bindgen_prelude::JsValue::raw(&ctor))
        }
    }
}

struct MemberFn {
    kind: MemberKind,
    f: syn::ImplItemFn,
}

fn expand_impl(i: &ItemImpl) -> syn::Result<TokenStream2> {
    let self_ty = self_ident(&i.self_ty)?;
    // rustc expands structs before impls, so a real build always finds the
    // registered struct; the fallback only serves the IDE's on-demand
    // expansion of an impl in isolation (diagnostic-only, never compiled).
    let mut meta = layer_registry()
        .lock()
        .unwrap()
        .get(&self_ty.to_string())
        .cloned()
        .unwrap_or_else(|| LayerMeta {
            js_name: self_ty.to_string(),
            parent_ty: None,
        });

    let mut i = i.clone();
    // The parent layer is declared inside the impl block as
    // `#[layer(parent)] type Parent = X;`. It is consumed here (and the
    // item removed - an inherent `type Parent` would collide with the
    // generated `LayerMembers::Parent`), so the struct block carries only
    // the class name and the fields.
    let mut parent_ann: Option<Type> = None;
    i.items.retain(|item| {
        let ImplItem::Type(t) = item else { return true };
        let is_parent = t.attrs.iter().any(|a| layer_attr_has_flag(a, "parent"));
        if is_parent {
            parent_ann = Some(t.ty.clone());
        }
        !is_parent
    });
    meta.parent_ty = parent_ann.as_ref().map(|t| t.to_token_stream().to_string());
    layer_registry()
        .lock()
        .unwrap()
        .insert(self_ty.to_string(), meta.clone());
    let mut napi_fns = vec![];
    let mut members: Vec<MemberFn> = vec![];
    let mut consts: Vec<(String, syn::Expr)> = vec![];
    let mut ctor: Option<(syn::ImplItemFn, BuildParams)> = None;
    for item in &mut i.items {
        match item {
            ImplItem::Fn(f) => {
                let Some(info) = MemberKind::parse(&mut f.attrs) else {
                    continue;
                };
                if info.kind == MemberKind::Constructor {
                    ctor = Some((f.clone(), analyze_build(f)));
                } else {
                    napi_fns.push(member_napi_fn(f, info.kind, &[], &self_ty, &meta.js_name));
                    members.push(MemberFn {
                        kind: info.kind,
                        f: f.clone(),
                    });
                }
            }
            ImplItem::Const(c) if c.attrs.iter().any(|a| a.path().is_ident("layer")) => {
                // The const is emitted as a JS static value, so the Rust
                // impl never reads it; keep the item (callers may still
                // reference it in Rust) but silence the dead_code lint.
                c.attrs.retain(|a| !a.path().is_ident("layer"));
                c.attrs.push(syn::parse_quote!(#[allow(dead_code)]));
                // Static constants keep their Rust name verbatim: JS
                // convention is already UPPER_SNAKE (to_camel would mangle
                // `BASE_CONST` into `BASECONST`).
                consts.push((c.ident.to_string(), c.expr.clone()));
            }
            _ => {}
        }
    }
    let (ctor_f, ctor_build) = ctor.ok_or_else(|| {
        syn::Error::new_spanned(
            &i.self_ty,
            "#[layer] impl must declare a constructor with #[layer(constructor)]",
        )
    })?;

    // The build signature's real parameters are the whole chain's
    // constructor arguments, ancestors' first. They feed both the TS
    // constructor signature and `LayerBuild::Args`.
    let ts_params: Vec<(Ident, Box<Type>)> = ctor_build.params();
    let full_params_ty: Vec<Box<Type>> = ts_params.iter().map(|(_, t)| t.clone()).collect();
    napi_fns.push(member_napi_fn(
        &ctor_f,
        MemberKind::Constructor,
        &ts_params,
        &self_ty,
        &meta.js_name,
    ));

    let impl_def = NapiImpl {
        name: self_ty.clone(),
        js_name: meta.js_name.clone(),
        has_lifetime: false,
        items: napi_fns,
        task_output_type: None,
        iterator_yield_type: None,
        iterator_next_type: None,
        iterator_return_type: None,
        async_iterator_yield_type: None,
        async_iterator_next_type: None,
        async_iterator_return_type: None,
        js_mod: None,
        comments: vec![],
        register_name: format_ident!("__layer_placeholder"),
    };
    if let Some(def) = impl_def.to_type_def() {
        output_type_def(&def);
    }

    // The TS `extends` link is emitted right next to the parent declaration
    // that lives in the impl block.
    let parent_js = parent_ann
        .as_ref()
        .and_then(type_last_ident)
        .map(|id| resolve_js_name(&id.to_string()));
    if let Some(parent) = &parent_js {
        output_type_def(&TypeDef {
            kind: "extends".to_owned(),
            name: meta.js_name.clone(),
            def: parent.clone(),
            ..Default::default()
        });
    }

    let parent_ty = parent_ann;

    let ctor_ret_is_result = match &ctor_f.sig.output {
        ReturnType::Type(_, ty) => type_last_ident(ty).map(|i| i == "Result").unwrap_or(false),
        ReturnType::Default => false,
    };
    let extend_layer = gen_extend_layer_impl(
        &self_ty,
        &meta.js_name,
        &parent_ty,
        &ctor_name_of(&ctor_f),
        &ctor_build,
        &full_params_ty,
        &members,
        &consts,
        ctor_ret_is_result,
    );
    let register = gen_register(&self_ty, &meta.js_name);
    Ok(quote! { #i #extend_layer #register })
}

fn ctor_name_of(f: &syn::ImplItemFn) -> Ident {
    f.sig.ident.clone()
}

fn expand(attr: TokenStream2, input: TokenStream2) -> syn::Result<TokenStream2> {
    let attrs = LayerAttrs::parse(attr)?;
    let item: syn::Item = syn::parse2(input)?;
    match item {
        syn::Item::Struct(mut s) => {
            s.attrs.retain(|a| !a.path().is_ident("layer"));
            expand_struct(&s, &attrs)
        }
        syn::Item::Impl(mut i) => {
            i.attrs.retain(|a| !a.path().is_ident("layer"));
            expand_impl(&i)
        }
        other => Err(syn::Error::new_spanned(
            other,
            "#[layer] can only be applied to a struct or an impl block",
        )),
    }
}

#[proc_macro_attribute]
pub fn layer(attr: TokenStream, input: TokenStream) -> TokenStream {
    match expand(attr.into(), input.into()) {
        Ok(ts) => ts.into(),
        Err(e) => e.into_compile_error().into(),
    }
}
