//! The process-wide layer registry (the struct block hands its fields over
//! to the impl block) and the type-def JSONL file that `@napi-rs/cli`
//! renders into `index.d.ts`.

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

use napi_derive_backend::TypeDef;
use proc_macro2::Ident;

/// One public field of a layer struct and whether it is exposed to JS as a
/// getter and/or setter (both `false` = not exposed at all). Defaults to
/// nothing; the user opts in per-field with `#[layer(getter)]` /
/// `#[layer(setter)]` / `#[layer(getter, setter)]`.
#[derive(Clone)]
pub(crate) struct FieldMeta {
    pub name: String,
    pub ty: String,
    pub getter: bool,
    pub setter: bool,
    /// The JS property name; getter and setter share it. Defaults to the
    /// camelCased field name.
    pub js_name: String,
}

/// Everything the macro learns about one layer across its two expansions
/// (struct first, then impl). The struct block registers only its own data
/// (public fields, doc comments); the impl block adds the JS class name and
/// forwards the struct-registered fields into the class type def it emits.
/// Types are stored as strings (`syn` items are not `Send` and cannot live
/// in a `static`).
#[derive(Clone)]
pub(crate) struct LayerMeta {
    pub js_name: String,
    pub fields: Vec<FieldMeta>,
    pub comments: Vec<String>,
}

static LAYER_REGISTRY: OnceLock<Mutex<HashMap<String, LayerMeta>>> = OnceLock::new();

fn layer_registry() -> &'static Mutex<HashMap<String, LayerMeta>> {
    LAYER_REGISTRY.get_or_init(Default::default)
}

/// Read the layer metadata registered by the struct block.
pub(crate) fn read_layer(name: &str) -> Option<LayerMeta> {
    layer_registry().lock().unwrap().get(name).cloned()
}

/// Register (or overwrite) the layer metadata under its Rust ident.
pub(crate) fn write_layer(name: &str, meta: LayerMeta) {
    layer_registry()
        .lock()
        .unwrap()
        .insert(name.to_owned(), meta);
}

/// Resolve a layer reference written as a Rust ident (or already a JS name)
/// to its JS class name through the registry.
pub(crate) fn resolve_js_name(rust_or_js: &str) -> String {
    read_layer(rust_or_js)
        .map(|m| m.js_name)
        .unwrap_or_else(|| rust_or_js.to_owned())
}

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
pub(crate) fn output_type_def(def: &TypeDef) {
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

/// The rust ident under which `FieldMeta` is stored.
pub(crate) fn field_ident(fm: &FieldMeta) -> Ident {
    Ident::new(&fm.name, proc_macro2::Span::call_site())
}
