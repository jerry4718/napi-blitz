//! Diagnostics that can never kill the host: every macro writes and ignores
//! write errors, so a closed or saturated stderr cannot turn into a panic
//! that aborts across the napi FFI boundary.

/// Print a diagnostic line to stderr, ignoring write errors.
///
/// stderr may be a non-blocking pipe owned by the embedding host (ava
/// workers attach theirs through `pipe.open`, which sets `O_NONBLOCK`).
/// A plain `eprintln!` panics when the write returns EAGAIN, and a panic
/// crossing the napi FFI boundary aborts the whole process. This macro
/// drops the line instead - a lost diagnostic must not be fatal.
///
/// # Examples
///
/// ```ignore
/// native_log!("[rust] values(index={}) -> {:?}", index, v);
/// ```
#[macro_export]
macro_rules! native_log {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

/// Explicitly swallow a known, non-fatal error.
///
/// cfg-controlled: in debug builds the error panics (never silently
/// ignored, forcing a fix); in release builds it is logged via
/// [`native_log!`] and dropped.
///
/// Takes a result expression followed by a format string and optional
/// arguments (like `format!`); the error itself is appended to the message.
///
/// # Examples
///
/// ```ignore
/// discard_err!(event.set_named_property("currentTarget", ()), "failed to reset currentTarget");
/// discard_err!(factory_fn.call(args), "call event factory for node {}", node_id);
/// ```
#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! discard_err {
    ($result:expr, $($arg:tt)*) => {
        if let Err(e) = $result {
            $crate::native_log!("napi-blitz: {}: {e}", format_args!($($arg)*));
        }
    };
}
#[cfg(debug_assertions)]
#[macro_export]
macro_rules! discard_err {
    ($result:expr, $($arg:tt)*) => {
        if let Err(e) = $result {
            panic!("napi-blitz: discarded error in debug build: {}: {e}", format_args!($($arg)*));
        }
    };
}
