use crate::window::options::WindowOptions;
use napi::{Error, bindgen_prelude::Uint8Array};
use winit::{
    dpi::PhysicalSize,
    icon::{Icon, RgbaIcon},
    window::{WindowAttributes, WindowButtons},
};

pub(crate) fn parse_dimension(name: &str, value: f64) -> napi::Result<u32> {
    if !value.is_finite() {
        return Err(Error::from_reason(format!("{name} must be finite")));
    }
    if value.fract() != 0.0 {
        return Err(Error::from_reason(format!("{name} must be an integer")));
    }
    if value < 1.0 {
        return Err(Error::from_reason(format!("{name} must be >= 1")));
    }
    if value > u32::MAX as f64 {
        return Err(Error::from_reason(format!("{name} exceeds u32::MAX")));
    }
    Ok(value as u32)
}

/// Translate `WindowOptions` into a winit `WindowAttributes`. Skipped
/// fields fall back to winit's platform default.
pub(crate) fn build_window_attributes(
    options: Option<&WindowOptions>,
) -> napi::Result<WindowAttributes> {
    let mut attrs = WindowAttributes::default();
    let Some(options) = options else {
        return Ok(attrs);
    };

    if let Some(title) = options.title.as_ref() {
        attrs = attrs.with_title(title.clone());
    }
    if let Some((w, h)) = options.size {
        let w = parse_dimension("width", w)?;
        let h = parse_dimension("height", h)?;
        attrs = attrs.with_surface_size(PhysicalSize::new(w, h));
    }
    if let Some(resizable) = options.resizable {
        attrs = attrs.with_resizable(resizable);
    }
    if let Some((w, h)) = options.min_size {
        let w = parse_dimension("minWidth", w)?;
        let h = parse_dimension("minHeight", h)?;
        attrs = attrs.with_min_surface_size(PhysicalSize::new(w, h));
    }
    if let Some((w, h)) = options.max_size {
        let w = parse_dimension("maxWidth", w)?;
        let h = parse_dimension("maxHeight", h)?;
        attrs = attrs.with_max_surface_size(PhysicalSize::new(w, h));
    }
    if let Some(maximized) = options.maximized {
        attrs = attrs.with_maximized(maximized);
    }
    if let Some(visible) = options.visible {
        attrs = attrs.with_visible(visible);
    }
    if let Some(transparent) = options.transparent {
        attrs = attrs.with_transparent(transparent);
    }
    if let Some(blur) = options.blur {
        attrs = attrs.with_blur(blur);
    }
    if let Some(decorations) = options.decorations {
        attrs = attrs.with_decorations(decorations);
    }
    if let Some(fullscreen) = options.fullscreen.as_ref() {
        attrs = attrs.with_fullscreen(Some(fullscreen.clone()));
    }
    if let Some(buttons) = options.enabled_buttons.as_ref() {
        attrs = attrs.with_enabled_buttons(parse_window_buttons(buttons)?);
    }
    if let Some(icon_data) = options.window_icon.as_ref() {
        attrs = attrs.with_window_icon(Some(parse_window_icon(icon_data)?));
    }
    if let Some(parent) = options.parent_window.as_ref() {
        attrs = unsafe { attrs.with_parent_window(Some(parent.window)) };
    }
    Ok(attrs)
}

/// Parse JS string array into winit `WindowButtons` bitflags.
/// Accepted values: `"close"`, `"minimize"`, `"maximize"`.
pub(crate) fn parse_window_buttons(buttons: &[String]) -> napi::Result<WindowButtons> {
    let mut flags = WindowButtons::empty();
    for btn in buttons {
        match btn.as_str() {
            "close" => flags |= WindowButtons::CLOSE,
            "minimize" => flags |= WindowButtons::MINIMIZE,
            "maximize" => flags |= WindowButtons::MAXIMIZE,
            other => {
                return Err(Error::from_reason(format!(
                    "enabledButtons: unknown button \"{other}\", expected close/minimize/maximize"
                )));
            }
        }
    }
    Ok(flags)
}

/// Parse window icon from raw bytes. Expected layout:
/// `[width_u32_le, height_u32_le, ...rgba8_pixels]` (8 byte header + w*h*4 bytes).
pub(crate) fn parse_window_icon(data: &Uint8Array) -> napi::Result<Icon> {
    let bytes = data.as_ref();
    if bytes.len() < 8 {
        return Err(Error::from_reason(
            "windowIcon: data too short, expected 8-byte header (width, height) + RGBA pixels",
        ));
    }
    let width = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let height = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let pixels = &bytes[8..];
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| Error::from_reason("windowIcon: width*height*4 overflows usize"))?;
    if pixels.len() < expected {
        return Err(Error::from_reason(format!(
            "windowIcon: pixel data is {} bytes, expected {expected} ({}x{}x4)",
            pixels.len(),
            width,
            height
        )));
    }
    RgbaIcon::new(pixels[..expected].to_vec(), width, height)
        .map(Icon::from)
        .map_err(|e| Error::from_reason(format!("windowIcon: failed to create icon: {e}")))
}
