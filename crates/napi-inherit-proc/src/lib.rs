//! `#[layer]` procedural macro — one layer of a napi inheritance chain.
//!
//! A layer is declared as a pair of `#[layer]` items in the same module: a
//! struct owning the layer's data, and its impl block carrying the
//! JS-facing members. The struct item must appear before the impl in
//! source order (the impl's expansion reads the struct's registration).
//!
//! # Struct block
//!
//! ```ignore
//! #[layer]
//! pub struct EventLayer {
//!     #[layer(getter)]
//!     pub bubbles: bool,
//!     #[layer(getter, setter)]
//!     pub value: u32,
//!     #[layer(getter, setter, js_name = "timeStamp")]
//!     pub time_stamp: f64,
//!     internal: String, // private: never exposed
//! }
//! ```
//!
//! The struct expansion generates the runtime `LayerAccessors`
//! implementation — one getter/setter pair per exposed public field —
//! and registers the field list for the impl block. Only public fields
//! participate; a field is exposed when its `#[layer(getter)]` /
//! `#[layer(setter)]` / `#[layer(getter, setter)]` attribute says so.
//! The JS property name defaults to the camelCased field name; a
//! `js_name = "..."` entry inside `#[layer(...)]` overrides it for the
//! whole getter/setter pair. The struct's doc comment becomes the
//! class's JSDoc in `index.d.ts`.
//!
//! # Impl block
//!
//! ```ignore
//! #[layer(js_name = "Event")]
//! impl EventLayer {
//!     #[layer(parent)]
//!     type Parent = RootLayer;
//!
//!     #[layer(constructor)]
//!     fn build(
//!         type_: String,
//!         sup: Super<RootLayer>,
//!     ) -> napi::Result<Constructed<Self>> {
//!         let done = sup.call(napi::bindgen_prelude::FnArgs::from(()))?;
//!         Ok(Constructed::new(done, Self { type_ }))
//!     }
//!
//!     #[layer]
//!     const NONE: u32 = 0;
//!
//!     #[layer]
//!     fn method(&self, x: u32) -> u32 { x }
//!
//!     #[layer(getter)]
//!     fn counter(&self) -> u32 { self.counter }
//!
//!     #[layer(setter)]
//!     fn set_counter(&mut self, v: u32) { self.counter = v; }
//! }
//! ```
//!
//! # Member attributes
//!
//! - `#[layer(constructor)]` — the layer's constructor. Its parameters are
//!   the whole chain's constructor arguments, ancestors' first (each layer
//!   re-declares them). A `Super<Parent>` parameter receives the parent
//!   chain handle and must be called with `FnArgs` to build the parent.
//! - `#[layer(getter)]` / `#[layer(setter)]` on a method — property
//!   accessors. The JS property name is the camelCased method name; a
//!   setter's `set_` prefix is dropped so it lands on the same property as
//!   its getter. With a receiver the accessor is on the prototype; without
//!   one it is static, on the constructor.
//! - `#[layer]` on a method — an instance method when it has a receiver,
//!   a static method otherwise.
//! - `#[layer(this)]` on a member — an explicit instance member without a
//!   receiver: the JS instance is still injected, so the body reaches any
//!   layer's slot via `with_own` / `with_own_mut` on its own. The flag is
//!   also honoured on `#[layer(getter, this)]` / `#[layer(setter, this)]`
//!   accessors.
//! - `#[layer] const NAME: T = v;` — a static value on the constructor,
//!   using the Rust name verbatim (JS convention is `UPPER_SNAKE`).
//!
//! # Injected parameters
//!
//! Besides real arguments, a member may declare `env: &Env` — the current
//! napi environment — and `this: &Object` — the JS instance, letting the
//! body reach any layer's slot via `with_own` / `with_own_mut` while the
//! current layer's data is handed over as `&self`. Injected parameters
//! never take part in the JS arguments. `this` is only injectable on an
//! instance member: one with a `&self` receiver, or a receiver-less member
//! explicitly marked `#[layer(this)]`. A plain static method (no receiver,
//! no `#[layer(this)]`) declaring a `this` parameter is a compile error. A
//! `Result` return type surfaces as a JS exception; a non-`Result` return
//! is wrapped.
//!
//! # js_name, parent and the registry
//!
//! The impl block owns the JS class name (`#[layer(js_name = "...")]`,
//! defaulting to the Rust ident) and emits every type def the CLI needs:
//! the class declaration (built from the struct-registered fields), the
//! impl members, and the `extends` link. The parent layer is declared
//! inside the impl as `#[layer(parent)] type Parent = X;` (the item is
//! consumed, not emitted); the `extends` link resolves the parent's JS
//! name through the registry, so expansion order only couples impl blocks
//! to earlier impl blocks and to their own struct.
//!
//! # Expansion output
//!
//! Each expansion appends a serialized `TypeDef` to the napi-rs type-def
//! JSONL file that `@napi-rs/cli` renders into `index.d.ts`. The items
//! pass through unchanged minus the `#[layer]` attributes; the struct
//! additionally emits `LayerAccessors`, the impl additionally emits
//! `LayerMembers`, `LayerBuild`, and a constructor-registered export.
//!
//! # Implementation layout
//!
//! Each expansion is driven by a parsed struct:
//!
//! - [`struct_layer::StructLayer`] — the data side: public fields, the
//!   generated `LayerAccessors`, the registry handoff.
//! - [`impl_layer::ImplLayer`] — the JS-facing side: members, the
//!   constructor, the parent link, the type defs and the runtime glue.
//! - [`member::Member`] — one annotated impl member, quoting both its
//!   runtime registration and its TS entry.
//! - [`registry`] — the struct→impl handoff and the JSONL output.
//! - [`attrs`] — `#[layer(...)]` attribute parsing.

mod attrs;
mod impl_layer;
mod member;
mod registry;
mod struct_layer;
mod util;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use attrs::LayerAttrs;
use impl_layer::ImplLayer;
use registry::output_type_def;
use struct_layer::StructLayer;

fn expand(attr: TokenStream2, input: TokenStream2) -> syn::Result<TokenStream2> {
    let attrs = syn::parse2::<LayerAttrs>(attr)?;
    let item: syn::Item = syn::parse2(input)?;
    match item {
        syn::Item::Struct(s) => {
            let layer = StructLayer::parse(&s)?;
            layer.register();
            Ok(quote! { #layer })
        }
        syn::Item::Impl(i) => {
            let layer = ImplLayer::parse(&i, &attrs)?;
            layer.register();
            for def in layer.type_defs() {
                output_type_def(&def);
            }
            Ok(quote! { #layer })
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
