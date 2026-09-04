//! `RafQueue`: per-window `requestAnimationFrame` bookkeeping.
//!
//! The queue is the only holder of callbacks between registration and the
//! next redraw frame, so each callback is kept alive as a strong napi
//! reference that releases itself on drop. Rust owns the timing: a frame
//! runs on the window's redraw, driven by winit's `RedrawRequested` event;
//! the pump cadence is unrelated to frame scheduling.

use std::{cell::Cell, time::Instant};

use napi::{
    Env, JsValue, Result,
    bindgen_prelude::{FnArgs, FromNapiValue, Function},
};
use napi_helpers::{anything::Anything, native_log};

/// One pending frame callback plus its cancel handle.
type FrameCallback = (u32, Anything);

/// Monotonic frame clock in milliseconds, same unit as
/// `performance.now()`; starts at the first use.
fn now_ms() -> f64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// Per-window rAF state. `paused` mirrors winit's `Occluded` flag: while
/// the window is not visible, pending callbacks stay queued and frames are
/// skipped (browser behavior for hidden documents); lifting occlusion
/// resumes them.
pub(crate) struct RafQueue {
    next_handle: Cell<u32>,
    pending: std::cell::RefCell<Vec<FrameCallback>>,
    paused: Cell<bool>,
}

impl RafQueue {
    pub(crate) fn new() -> Self {
        Self {
            next_handle: Cell::new(1),
            pending: std::cell::RefCell::new(Vec::new()),
            paused: Cell::new(false),
        }
    }

    /// Register `callback` to run on the next frame; returns the cancel
    /// handle. The callback is strongly referenced until it runs (or is
    /// cancelled), so captured closures stay alive across pumps.
    pub(crate) fn push(
        &self,
        env: &Env,
        callback: Function<FnArgs<(f64,)>, Anything>,
    ) -> Result<u32> {
        let value = unsafe { Anything::from_napi_value(env.raw(), callback.raw())? };
        let handle = self.next_handle.get();
        self.next_handle.set(handle.wrapping_add(1));
        self.pending.borrow_mut().push((handle, value));
        Ok(handle)
    }

    /// Remove a previously registered callback; its strong reference is
    /// released right here.
    pub(crate) fn cancel(&self, handle: u32) {
        self.pending.borrow_mut().retain(|(h, _)| *h != handle);
    }

    /// Remove and return all pending callbacks. The borrow ends when the
    /// call returns, so the caller can run the callbacks without holding
    /// the queue borrowed (running re-enters JS, which may register or
    /// cancel again). While occluded the callbacks stay queued and
    /// nothing is returned.
    pub(crate) fn take_pending(&self) -> Vec<FrameCallback> {
        if self.paused.get() {
            return Vec::new();
        }
        std::mem::take(&mut *self.pending.borrow_mut())
    }

    /// Run callbacks produced by [`RafQueue::take_pending`]. Touches
    /// nothing but the JS env, so a callback re-entering
    /// `requestAnimationFrame` / `cancelAnimationFrame` on this queue is
    /// safe. The timestamp is the same for all callbacks of one frame.
    pub(crate) fn run(&self, env: &Env, pending: Vec<FrameCallback>) {
        let timestamp = now_ms();
        for (_, value) in pending {
            let Anything::Function(reference) = value else {
                continue;
            };
            let raw = match unsafe { reference.raw_value(env) } {
                Ok(raw) => raw,
                Err(err) => {
                    native_log!("raf: dropping callback: {err}");
                    continue;
                }
            };
            let f = match unsafe {
                Function::<FnArgs<(f64,)>, Anything>::from_napi_value(env.raw(), raw)
            } {
                Ok(f) => f,
                Err(err) => {
                    native_log!("raf: bad callback value: {err}");
                    continue;
                }
            };
            // A throwing callback must not stop the remaining ones.
            if let Err(err) = f.call(FnArgs::from((timestamp,))) {
                native_log!("raf: callback threw: {err}");
            }
        }
    }

    /// Whether any callback is waiting for a frame, used to request the
    /// first redraw after a registration (or after occlusion lifts).
    pub(crate) fn has_pending(&self) -> bool {
        !self.pending.borrow().is_empty()
    }

    /// Pause or resume frames; pausing keeps the pending callbacks.
    pub(crate) fn set_paused(&self, paused: bool) {
        self.paused.set(paused);
    }

    /// Drop everything without running it (window close). The dropped
    /// references release their JS callbacks.
    pub(crate) fn clear(&self) {
        self.pending.borrow_mut().clear();
    }
}
