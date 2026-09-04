//! Addon-level napi env storage.
//!
//! The env is injected once during addon init (`init_env`) and reused by
//! Rust-side paths that have no document in hand to read one from (the
//! window lifecycle dispatch). Documents carry their own env on
//! `SharedDocument`; this global exists for the document-less entry
//! points only. Accessed only from the JS thread.

use std::cell::Cell;

use napi::{Env, Error, Result, Status};

thread_local! {
    static ENV: Cell<Option<Env>> = const { Cell::new(None) };
}

fn set_env(env: Env) {
    ENV.with(|e| e.set(Some(env)));
}

pub fn env() -> Result<Env> {
    ENV.with(|e| {
        e.get()
            .ok_or_else(|| Error::new(Status::GenericFailure, "init_env not called"))
    })
}

/// One-time env injection. JS calls this during addon init so that
/// `global::env()` works in callbacks that don't receive an `Env`.
#[napi]
pub fn init_env(env: Env) -> Result<()> {
    set_env(env);
    Ok(())
}
