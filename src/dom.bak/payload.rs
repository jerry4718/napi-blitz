//! Napi structs used to ferry event data between Rust and JS.
//!
//! All fields are `pub(crate)` with read-only getters so JS cannot
//! mutate them. Each struct holds an `Arc` to the original blitz event
//! so cloning is cheap (one refcount bump). Values that require
//! decomposition (enum-to-string etc.) are pre-computed at construction
//! time in `event.rs` and stored alongside the `Arc`.

use std::sync::Arc;

use blitz::traits::events::{BlitzKeyEvent, BlitzPointerEvent, BlitzWheelEvent};

// ── EventPayload ────────────────────────────────────────────────────

/// One DomEvent serialized for JS consumption.
///
/// Built once per event and passed to the registered JS event factory.
/// Rust drives the capture/target/bubble walk separately.
#[napi]
pub struct EventPayload {
    pub(crate) event_type: String,
    pub(crate) bubbles: bool,
    pub(crate) cancelable: bool,
    pub(crate) pointer: Option<PointerData>,
    pub(crate) wheel: Option<WheelData>,
    pub(crate) key: Option<KeyData>,
    pub(crate) input: Option<InputData>,
    pub(crate) ime: Option<ImeData>,
}

#[napi]
impl EventPayload {
    /// Event name in DOM-spec lowercased form, e.g. "click", "pointerdown".
    #[napi(getter, js_name = "type")]
    pub fn event_type(&self) -> String {
        self.event_type.clone()
    }
    /// `event.bubbles`
    #[napi(getter)]
    pub fn bubbles(&self) -> bool {
        self.bubbles
    }
    /// `event.cancelable`
    #[napi(getter)]
    pub fn cancelable(&self) -> bool {
        self.cancelable
    }
    /// Pointer/mouse details, when applicable.
    #[napi(getter)]
    pub fn pointer(&self) -> Option<PointerData> {
        self.pointer.clone()
    }
    /// Wheel delta, when applicable.
    #[napi(getter)]
    pub fn wheel(&self) -> Option<WheelData> {
        self.wheel.clone()
    }
    /// Keyboard details, when applicable.
    #[napi(getter)]
    pub fn key(&self) -> Option<KeyData> {
        self.key.clone()
    }
    /// `<input>` value carried by `Input` events.
    #[napi(getter)]
    pub fn input(&self) -> Option<InputData> {
        self.input.clone()
    }
    /// IME details, when applicable.
    #[napi(getter)]
    pub fn ime(&self) -> Option<ImeData> {
        self.ime.clone()
    }
}

// ── PointerData ─────────────────────────────────────────────────────

#[napi]
#[derive(Clone)]
pub struct PointerData {
    pub(crate) inner: Arc<BlitzPointerEvent>,
    pub(crate) kind: String,
    pub(crate) pointer_id: f64,
}

#[napi]
impl PointerData {
    /// "mouse" | "pen" | "finger"
    #[napi(getter)]
    pub fn kind(&self) -> String {
        self.kind.clone()
    }
    /// Pointer id; for mouse / pen this is 1, for finger it's the finger id.
    #[napi(getter)]
    pub fn pointer_id(&self) -> f64 {
        self.pointer_id
    }
    #[napi(getter)]
    pub fn is_primary(&self) -> bool {
        self.inner.is_primary
    }
    #[napi(getter)]
    pub fn page_x(&self) -> f64 {
        self.inner.coords.page_x as f64
    }
    #[napi(getter)]
    pub fn page_y(&self) -> f64 {
        self.inner.coords.page_y as f64
    }
    #[napi(getter)]
    pub fn client_x(&self) -> f64 {
        self.inner.coords.client_x as f64
    }
    #[napi(getter)]
    pub fn client_y(&self) -> f64 {
        self.inner.coords.client_y as f64
    }
    #[napi(getter)]
    pub fn screen_x(&self) -> f64 {
        self.inner.coords.screen_x as f64
    }
    #[napi(getter)]
    pub fn screen_y(&self) -> f64 {
        self.inner.coords.screen_y as f64
    }
    #[napi(getter)]
    pub fn button(&self) -> i32 {
        self.inner.button as i32
    }
    #[napi(getter)]
    pub fn buttons(&self) -> u32 {
        self.inner.buttons.bits() as u32
    }
    #[napi(getter)]
    pub fn pressure(&self) -> f64 {
        self.inner.details.pressure
    }
    #[napi(getter)]
    pub fn tilt_x(&self) -> i32 {
        self.inner.details.tilt_x as i32
    }
    #[napi(getter)]
    pub fn tilt_y(&self) -> i32 {
        self.inner.details.tilt_y as i32
    }
    #[napi(getter)]
    pub fn twist(&self) -> u32 {
        self.inner.details.twist as u32
    }
    #[napi(getter)]
    pub fn mods_bits(&self) -> u32 {
        self.inner.mods.bits()
    }
}

// ── WheelData ───────────────────────────────────────────────────────

#[derive(Clone)]
#[napi]
pub struct WheelData {
    pub(crate) inner: Arc<BlitzWheelEvent>,
    pub(crate) mode: String,
    pub(crate) delta_x: f64,
    pub(crate) delta_y: f64,
}

#[napi]
impl WheelData {
    /// "lines" | "pixels"
    #[napi(getter)]
    pub fn mode(&self) -> String {
        self.mode.clone()
    }
    #[napi(getter)]
    pub fn delta_x(&self) -> f64 {
        self.delta_x
    }
    #[napi(getter)]
    pub fn delta_y(&self) -> f64 {
        self.delta_y
    }
    #[napi(getter)]
    pub fn page_x(&self) -> f64 {
        self.inner.coords.page_x as f64
    }
    #[napi(getter)]
    pub fn page_y(&self) -> f64 {
        self.inner.coords.page_y as f64
    }
    #[napi(getter)]
    pub fn client_x(&self) -> f64 {
        self.inner.coords.client_x as f64
    }
    #[napi(getter)]
    pub fn client_y(&self) -> f64 {
        self.inner.coords.client_y as f64
    }
    #[napi(getter)]
    pub fn buttons(&self) -> u32 {
        self.inner.buttons.bits() as u32
    }
    #[napi(getter)]
    pub fn mods_bits(&self) -> u32 {
        self.inner.mods.bits()
    }
}

// ── KeyData ─────────────────────────────────────────────────────────

#[derive(Clone)]
#[napi]
pub struct KeyData {
    pub(crate) inner: Arc<BlitzKeyEvent>,
    pub(crate) state: String,
}

#[napi]
impl KeyData {
    /// e.g. "a", "ArrowLeft", "Enter"
    #[napi(getter)]
    pub fn key(&self) -> String {
        self.inner.key.to_string()
    }
    /// e.g. "KeyA", "ArrowLeft", "Enter"
    #[napi(getter)]
    pub fn code(&self) -> String {
        self.inner.code.to_string()
    }
    #[napi(getter)]
    pub fn location(&self) -> u32 {
        self.inner.location as u32
    }
    #[napi(getter)]
    pub fn mods_bits(&self) -> u32 {
        self.inner.modifiers.bits()
    }
    #[napi(getter)]
    pub fn repeat(&self) -> bool {
        self.inner.is_auto_repeating
    }
    #[napi(getter)]
    pub fn is_composing(&self) -> bool {
        self.inner.is_composing
    }
    /// "pressed" | "released"
    #[napi(getter)]
    pub fn state(&self) -> String {
        self.state.clone()
    }
    #[napi(getter)]
    pub fn text(&self) -> Option<String> {
        self.inner.text.as_ref().map(|s| s.to_string())
    }
}

// ── InputData ───────────────────────────────────────────────────────

#[derive(Clone)]
#[napi]
pub struct InputData {
    pub(crate) value: String,
}

#[napi]
impl InputData {
    #[napi(getter)]
    pub fn value(&self) -> String {
        self.value.clone()
    }
}

// ── ImeData ─────────────────────────────────────────────────────────

#[derive(Clone)]
#[napi]
pub struct ImeData {
    pub(crate) kind: String,
    pub(crate) text: Option<String>,
    pub(crate) cursor_start: Option<u32>,
    pub(crate) cursor_end: Option<u32>,
    pub(crate) before_bytes: Option<u32>,
    pub(crate) after_bytes: Option<u32>,
}

#[napi]
impl ImeData {
    /// "enabled" | "disabled" | "preedit" | "commit" | "deleteSurrounding"
    #[napi(getter)]
    pub fn kind(&self) -> String {
        self.kind.clone()
    }
    #[napi(getter)]
    pub fn text(&self) -> Option<String> {
        self.text.clone()
    }
    #[napi(getter)]
    pub fn cursor_start(&self) -> Option<u32> {
        self.cursor_start
    }
    #[napi(getter)]
    pub fn cursor_end(&self) -> Option<u32> {
        self.cursor_end
    }
    #[napi(getter)]
    pub fn before_bytes(&self) -> Option<u32> {
        self.before_bytes
    }
    #[napi(getter)]
    pub fn after_bytes(&self) -> Option<u32> {
        self.after_bytes
    }
}
