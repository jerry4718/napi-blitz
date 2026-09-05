//! Global runtime configuration, written once at startup.

use napi::{Error, Result};
use napi_helpers::inherits::{
    PROXY_COMPAT, ProxyCompatMode as InheritMode, set_proxy_compat as set_inherit_mode,
};

/// How layer accessors resolve their own-data registry from the (possibly
/// proxied) receiver. Written once: `setProxyCompat` rejects later calls.
#[napi(string_enum = "kebab-case")]
pub enum ProxyCompatMode {
    /// Unwrap the receiver directly; fails on a proxied receiver.
    Off,
    /// Always resolve through the instance's self-reference key first.
    On,
    /// Unwrap the receiver directly, falling back to the key when that
    /// fails (the receiver is not the raw instance).
    Auto,
}

/// Set the global proxy-compat mode for layer accessors. Succeeds once;
/// later calls error with the mode already in effect.
#[napi]
pub fn set_proxy_compat(
    #[napi(ts_arg_type = "ProxyCompatMode | 'on' | 'off' | 'auto'")] mode: ProxyCompatMode,
) -> Result<()> {
    let mode = match mode {
        ProxyCompatMode::Off => InheritMode::Off,
        ProxyCompatMode::On => InheritMode::On,
        ProxyCompatMode::Auto => InheritMode::Auto,
    };
    set_inherit_mode(mode).map_err(|_| {
        let current = PROXY_COMPAT
            .get()
            .map(|m| format!("{m:?}"))
            .unwrap_or_else(|| "unset".into());
        Error::from_reason(format!("setProxyCompat: mode already set to {current}"))
    })
}
