//! Expansion of a `#[layer]` impl block: the JS-facing half of a layer.
//! Everything the impl learns about itself during parsing is stored on
//! `ImplLayer`; quoting the runtime glue, emitting the TS type defs and
//! registering the JS class name are then thin reads over those fields.

use crate::attrs::{LayerAttrs, MemberKind, layer_attr_has_flag};
use crate::member::Member;
use crate::registry::{FieldMeta, LayerMeta, read_layer, resolve_js_name, write_layer};
use crate::util::{self_ident, type_last_ident};
use napi_derive_backend::{
    NapiClass, NapiFn, NapiImpl, NapiStruct, NapiStructField, NapiStructKind, ToTypeDef, TypeDef,
};
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::{ToTokens, format_ident, quote};
use syn::{FnArg, ImplItem, ItemImpl, ReturnType, Type};

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

pub(crate) struct ImplLayer {
    /// The impl item with `#[layer]` attributes and the parent declaration
    /// stripped, emitted verbatim.
    item: ItemImpl,
    self_ty: Ident,
    js_name: String,
    /// The struct block's registration (fields + docs). On a real build the
    /// struct expands first; the fallback only serves the IDE's on-demand
    /// expansion of an impl in isolation (diagnostic-only, never compiled).
    meta: LayerMeta,
    /// The parent layer declared as `#[layer(parent)] type Parent = X;`.
    parent_ty: Option<Type>,
    /// The parent's JS class name, resolved through the registry.
    parent_js: Option<String>,
    members: Vec<Member>,
    consts: Vec<(String, syn::Expr)>,
    ctor: (Member, BuildParams),
}

impl ImplLayer {
    pub fn parse(i: &ItemImpl, attrs: &LayerAttrs) -> syn::Result<Self> {
        let mut i = i.clone();
        i.attrs.retain(|a| !a.path().is_ident("layer"));
        let self_ty = self_ident(&i.self_ty)?;
        let js_name = attrs.js_name.clone().unwrap_or_else(|| self_ty.to_string());
        let meta = read_layer(&self_ty.to_string()).unwrap_or_else(|| LayerMeta {
            js_name: self_ty.to_string(),
            fields: vec![],
            comments: vec![],
        });

        // The parent layer is declared inside the impl block as
        // `#[layer(parent)] type Parent = X;`. It is consumed here (and the
        // item removed - an inherent `type Parent` would collide with the
        // generated `LayerMembers::Parent`).
        let mut parent_ty = None;
        i.items.retain(|item| {
            let ImplItem::Type(t) = item else { return true };
            let is_parent = t.attrs.iter().any(|a| layer_attr_has_flag(a, "parent"));
            if is_parent {
                parent_ty = Some(t.ty.clone());
            }
            !is_parent
        });
        let parent_js = parent_ty
            .as_ref()
            .and_then(type_last_ident)
            .map(|id| resolve_js_name(&id.to_string()));

        let mut members = vec![];
        let mut consts = vec![];
        let mut ctor = None;
        for item in &mut i.items {
            match item {
                ImplItem::Fn(f) => {
                    let Some(member) = Member::parse(f) else {
                        continue;
                    };
                    if member.kind == MemberKind::Constructor {
                        let build = analyze_build(&member.f);
                        ctor = Some((member, build));
                    } else {
                        members.push(member);
                    }
                }
                ImplItem::Const(c) if c.attrs.iter().any(|a| a.path().is_ident("layer")) => {
                    // The const is emitted as a JS static value, so the Rust
                    // impl never reads it; keep the item (callers may still
                    // reference it in Rust) but silence the dead_code lint.
                    c.attrs.retain(|a| !a.path().is_ident("layer"));
                    c.attrs.push(syn::parse_quote!(#[allow(dead_code)]));
                    // Static constants keep their Rust name verbatim: JS
                    // convention is already UPPER_SNAKE (to_camel would
                    // mangle `BASE_CONST` into `BASECONST`).
                    consts.push((c.ident.to_string(), c.expr.clone()));
                }
                _ => {}
            }
        }
        let ctor = ctor.ok_or_else(|| {
            syn::Error::new_spanned(
                &i.self_ty,
                "#[layer] impl must declare a constructor with #[layer(constructor)]",
            )
        })?;

        Ok(Self {
            item: i,
            self_ty,
            js_name,
            meta,
            parent_ty,
            parent_js,
            members,
            consts,
            ctor,
        })
    }

    /// Overwrite the struct-registered placeholder JS name with this impl's
    /// real one, so later impl blocks resolve `extends` correctly.
    pub fn register(&self) {
        let mut meta = self.meta.clone();
        meta.js_name = self.js_name.clone();
        write_layer(&self.self_ty.to_string(), meta);
    }

    /// The three TS type defs this impl emits, in CLI merge order: members,
    /// class declaration, `extends` link.
    pub fn type_defs(&self) -> Vec<TypeDef> {
        let Self {
            self_ty,
            js_name,
            meta,
            parent_js,
            members,
            ctor,
            ..
        } = self;
        let (ctor_member, ctor_build) = ctor;
        let ts_params = ctor_build.params();
        let mut napi_fns: Vec<NapiFn> = members
            .iter()
            .map(|m| m.napi_fn(&[], self_ty, js_name))
            .collect();
        // The constructor leads the member list so the class declaration's
        // signature sits right after the field block instead of at the end.
        napi_fns.insert(0, ctor_member.napi_fn(&ts_params, self_ty, js_name));

        let mut defs = vec![];
        let impl_def = NapiImpl {
            name: self_ty.clone(),
            js_name: js_name.clone(),
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
            defs.push(def);
        }
        // The class type def comes from the struct-registered fields and
        // this impl's `js_name`, so both type defs that the CLI merges share
        // a single name source.
        if let Some(def) = class_def(js_name, &meta.fields, meta.comments.clone()).to_type_def() {
            defs.push(def);
        }
        // The TS `extends` link is emitted right next to the parent
        // declaration that lives in the impl block.
        if let Some(parent) = parent_js {
            defs.push(TypeDef {
                kind: "extends".to_owned(),
                name: js_name.clone(),
                def: parent.clone(),
                ..Default::default()
            });
        }
        defs
    }
}

/// The class declaration type def built from the struct-registered fields.
fn class_def(js_name: &str, fields: &[FieldMeta], comments: Vec<String>) -> NapiStruct {
    let fields = fields
        .iter()
        .map(|fm| {
            let field_ident = Ident::new(&fm.name, Span::call_site());
            NapiStructField {
                name: syn::Member::Named(field_ident.clone()),
                js_name: fm.js_name.clone(),
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
        comments,
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

impl ToTokens for ImplLayer {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let Self {
            self_ty,
            js_name,
            parent_ty,
            members,
            consts,
            ctor,
            ..
        } = self;
        let (ctor_member, ctor_build) = ctor;
        let ctor_f = &ctor_member.f;
        let ctor_name = &ctor_f.sig.ident;
        let ctor_ret_is_result = match &ctor_f.sig.output {
            ReturnType::Type(_, ty) => type_last_ident(ty).map(|i| i == "Result").unwrap_or(false),
            ReturnType::Default => false,
        };
        let parent_tokens = match parent_ty {
            Some(ty) => quote! { #ty },
            None => quote! { napi_helpers::inherits::RootLayer },
        };
        let member_tokens = members.iter().map(|m| m.runtime_tokens(self_ty));
        let const_tokens = consts.iter().map(|(cname, cexpr)| {
            quote! {
                napi_helpers::inherits::define_static_value(env, ctor, #cname, #cexpr)?;
            }
        });
        // The full argument tuple comes straight from the build signature's
        // real parameters (ancestors' first). The generated `build` only
        // destructures it and forwards to the user's build, which receives
        // the injected `env` / `sup` handles and drives `sup.call` itself.
        let full_params_ty: Vec<Type> = ctor_build.params().into_iter().map(|(_, t)| *t).collect();
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
        let has_env = ctor_build.args.iter().any(|a| matches!(a, BuildArg::Env));
        let env_param = if has_env {
            quote! { env: &'env napi::Env }
        } else {
            quote! { _env: &'env napi::Env }
        };
        let mut forward: Vec<TokenStream2> = vec![];
        let mut pidx = 0usize;
        for a in &ctor_build.args {
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

        let extend_layer = quote! {
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
        };
        let register = {
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
                        // The name must be NUL-terminated: napi parses it
                        // with `CStr::from_bytes_with_nul_unchecked`.
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
        };
        self.item.to_tokens(tokens);
        extend_layer.to_tokens(tokens);
        register.to_tokens(tokens);
    }
}
