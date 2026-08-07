//! `#[napi(object)]` shapes used to ferry event data between Rust and JS.
//!
//! `EventPayload` is built once per event (not per receiver) and passed
//! to the JS event factory to construct a JS `Event` object. The old
//! `target` / `receiver` / `phase` fields and the `DispatchResult`
//! round-trip have been removed: Rust now drives the dispatch chain
//! directly and reads `event.defaultPrevented` / `event.cancelBubble`
//! from the JS Event object after each `dispatchEvent` call.

/// One DomEvent serialized for JS consumption.
///
/// Built once per event and passed to the registered JS event factory.
/// Rust drives the capture/target/bubble walk separately.
#[napi(object)]
pub struct EventPayload {
    /// Event name in DOM-spec lowercased form, e.g. "click", "pointerdown".
    pub event_type: String,
    /// `event.bubbles`
    pub bubbles: bool,
    /// `event.cancelable`
    pub cancelable: bool,
    /// Pointer/mouse details, when applicable.
    pub pointer: Option<PointerData>,
    /// Wheel delta, when applicable.
    pub wheel: Option<WheelData>,
    /// Keyboard details, when applicable.
    pub key: Option<KeyData>,
    /// `<input>` value carried by `Input` events.
    pub input: Option<InputData>,
    /// IME details, when applicable.
    pub ime: Option<ImeData>,
}

#[napi(object)]
pub struct PointerData {
    /// "mouse" | "pen" | "finger"
    pub kind: String,
    /// Pointer id; for mouse / pen this is 1, for finger it's the finger id.
    pub pointer_id: f64,
    pub is_primary: bool,
    pub page_x: f64,
    pub page_y: f64,
    pub client_x: f64,
    pub client_y: f64,
    pub screen_x: f64,
    pub screen_y: f64,
    pub button: i32,
    pub buttons: u32,
    pub pressure: f64,
    pub tilt_x: i32,
    pub tilt_y: i32,
    pub twist: u32,
    pub mods_bits: u32,
}

#[napi(object)]
pub struct WheelData {
    /// "lines" | "pixels"
    pub mode: String,
    pub delta_x: f64,
    pub delta_y: f64,
    pub page_x: f64,
    pub page_y: f64,
    pub client_x: f64,
    pub client_y: f64,
    pub buttons: u32,
    pub mods_bits: u32,
}

#[napi(object)]
pub struct KeyData {
    /// e.g. "a", "ArrowLeft", "Enter"
    pub key: String,
    /// e.g. "KeyA", "ArrowLeft", "Enter"
    pub code: String,
    pub location: u32,
    pub mods_bits: u32,
    pub repeat: bool,
    pub is_composing: bool,
    /// "pressed" | "released"
    pub state: String,
    pub text: Option<String>,
}

#[napi(object)]
pub struct InputData {
    pub value: String,
}

#[napi(object)]
pub struct ImeData {
    /// "enabled" | "disabled" | "preedit" | "commit" | "deleteSurrounding"
    pub kind: String,
    pub text: Option<String>,
    pub cursor_start: Option<u32>,
    pub cursor_end: Option<u32>,
    pub before_bytes: Option<u32>,
    pub after_bytes: Option<u32>,
}
