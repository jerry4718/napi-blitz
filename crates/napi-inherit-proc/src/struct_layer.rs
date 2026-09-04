//! Expansion of a `#[layer]` struct block: field parsing, the generated
//! `LayerAccessors` implementation, and the registry handoff to the impl
//! block. The struct knows nothing about JS names or members — the impl
//! block owns those.

use crate::attrs::field_flags;
use crate::registry::{FieldMeta, LayerMeta, field_ident, write_layer};
use crate::util::{extract_doc, to_camel};
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{ToTokens, quote};
use syn::{ItemStruct, Type, Visibility};

pub(crate) struct StructLayer {
    /// The struct item with `#[layer]` attributes stripped, emitted verbatim.
    item: ItemStruct,
    self_ty: Ident,
    fields: Vec<FieldMeta>,
}

impl StructLayer {
    pub fn parse(s: &ItemStruct) -> syn::Result<Self> {
        let mut s = s.clone();
        s.attrs.retain(|a| !a.path().is_ident("layer"));
        // Only public fields participate; a field is exposed when its
        // `#[layer(getter)]` / `#[layer(setter)]` /
        // `#[layer(getter, setter)]` attribute says so.
        let fields: Vec<FieldMeta> = s
            .fields
            .iter_mut()
            .filter(|f| matches!(f.vis, Visibility::Public(_)))
            .map(|f| {
                let id = f.ident.clone().expect("named field");
                let (getter, setter, js_name) = field_flags(&mut f.attrs);
                FieldMeta {
                    name: id.to_string(),
                    ty: f.ty.to_token_stream().to_string(),
                    getter,
                    setter,
                    js_name: js_name.unwrap_or_else(|| to_camel(&id.to_string())),
                }
            })
            .collect();
        let self_ty = s.ident.clone();
        Ok(Self {
            item: s,
            self_ty,
            fields,
        })
    }

    /// Hand the struct's data over to the impl block. `js_name` is an ident
    /// placeholder; the impl block overwrites it with the real JS name.
    pub fn register(&self) {
        write_layer(
            &self.self_ty.to_string(),
            LayerMeta {
                js_name: self.self_ty.to_string(),
                fields: self.fields.clone(),
                comments: extract_doc(&self.item.attrs),
            },
        );
    }
}

impl ToTokens for StructLayer {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let Self {
            item,
            self_ty,
            fields,
        } = self;
        let field_getters = fields.iter().filter(|f| f.getter).map(|f| {
            let ident = field_ident(f);
            let field_js = &f.js_name;
            quote! {
                napi_helpers::inherits::define_getter(proto, #field_js, |_ctx, this| {
                    napi_helpers::inherits::with_own::<#self_ty, _>(&this, |d| d.#ident)
                })?;
            }
        });
        let field_setters = fields.iter().filter(|f| f.setter).map(|f| {
            let ident = field_ident(f);
            let field_js = &f.js_name;
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
        item.to_tokens(tokens);
        accessors.to_tokens(tokens);
    }
}
