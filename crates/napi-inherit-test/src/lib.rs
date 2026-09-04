//! End-to-end verification of `#[layer]`: a three-layer chain
//! `InheritBase -> InheritMid -> InheritLeaf`. The macro generates the
//! `LayerDef` + `LayerBridge` impls (the bridge dispatches the typed
//! constructor params and calls the user's pure-data `#[layer(constructor)]`
//! method) and mounts each class onto `module.exports` via
//! `register_module_export`, so a `napi build` produces both `index.d.ts`
//! (extends-typed, constructor signature concatenated from the chain) and
//! `index.cjs` with `module.exports.InheritBase = nativeBinding.InheritBase`
//! etc.

#[macro_use]
extern crate napi_derive;

mod manual_chain;
mod proc_chain;
