/// Explicitly swallow a known, non-fatal error.
///
/// cfg-controlled: in debug builds the error panics (never silently
/// ignored, forcing a fix); in release builds it is logged and dropped.
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
macro_rules! discard_err {
    ($result:expr, $($arg:tt)*) => {
        if let Err(e) = $result {
            eprintln!("napi-blitz: {}: {e}", format_args!($($arg)*));
        }
    };
}
#[cfg(debug_assertions)]
macro_rules! discard_err {
    ($result:expr, $($arg:tt)*) => {
        if let Err(e) = $result {
            panic!("napi-blitz: discarded error in debug build: {}: {e}", format_args!($($arg)*));
        }
    };
}

pub(crate) use discard_err;
