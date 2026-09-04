//! `BlitzApp`: the JS-facing wrapper around a winit event loop.
//!
//! Four roles, one question each:
//!
//! - `NativeApp`: how JS calls in - napi signatures, promise wrapping,
//!   and the pump driving cadence.
//! - `AppHandler`: how winit events flow in - the `ApplicationHandler`
//!   adapter that routes callbacks to `Lifecycle` (lifecycle events)
//!   or to the owning `View` (window events).
//! - `Lifecycle`: how windows are born and die - the single owner of
//!   the open/close flow, the shell-event pump, and the synthetic-exit
//!   check.
//! - `AppState`: what exists right now - the live-window table and the
//!   pending-request queue. Pure data.
//!
//! JS drives the loop synchronously via `pumpAppEvents(millis)` from
//! the main thread; this keeps event callbacks re-entrant on the napi
//! env so we can call back into JS without a ThreadsafeFunction.

mod event_loop;
mod handler;
pub(crate) mod lifecycle;
mod native;
mod state;
