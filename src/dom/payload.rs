//! `#[napi(object)]` shapes used to ferry events / responses between Rust and
//! JS. Opaque node ids travel as JS `bigint` values via napi-rs `BigInt`, while
//! DOM event scalar fields stay as plain JS numbers / strings.

use napi::bindgen_prelude::BigInt;
use napi_derive::napi;

/// One DomEvent serialized for JS consumption.
///
/// `receiver` is the node id currently dispatching the event in the
/// capture/target/bubble walk. Rust now drives the walk and JS only
/// dispatches a single `Event` to that receiver.
#[napi(object)]
pub struct EventPayload {
    /// Event name in DOM-spec lowercased form, e.g. "click", "pointerdown".
    pub event_type: String,
    /// node id of the original event target.
    pub target: BigInt,
    /// node id currently receiving this dispatch step.
    pub receiver: BigInt,
    /// Current dispatch phase: 1=capture, 2=target, 3=bubble.
    pub phase: u32,
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

/// JS reports back per-event how dispatch went.
#[napi(object)]
#[derive(Default)]
pub struct DispatchResult {
    /// JS called `event.preventDefault()` somewhere.
    pub default_prevented: bool,
    /// JS called `event.stopPropagation()` somewhere. We don't need this on the
    /// Rust side semantically (JS already stopped its own bubble), but we still
    /// surface it so blitz can skip its default action when appropriate.
    pub propagation_stopped: bool,
    /// JS wants a redraw; usually because a listener mutated the DOM.
    pub request_redraw: bool,
}
