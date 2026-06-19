//! Headless RGBA buffer rendering for Blitz documents.
//!
//! This module is intentionally separate from [`crate::native_window::app::BlitzApp`]. The
//! native app path owns a winit event loop and paints into OS windows. The
//! buffer path owns no window and no event loop: callers mutate a `DocHandle`,
//! then ask this renderer to resolve layout/paint into an RGBA frame that the
//! host can display however it wants.

use anyrender::{PaintScene as _, render_to_buffer};
use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz::{
    dom::util::Color,
    traits::shell::{ColorScheme, Viewport},
};
use blitz_paint::paint_scene;
use napi::{Error, Result, bindgen_prelude::Uint8Array};
use napi_derive::napi;
use peniko::{Fill, kurbo::Rect};

use crate::doc::DocHandle;

#[napi(object)]
pub struct BufferRendererOptions {
    /// Viewport width in CSS pixels.
    pub width: f64,
    /// Viewport height in CSS pixels.
    pub height: f64,
    /// Device scale factor. Defaults to 1.0.
    pub scale: Option<f64>,
}

#[napi(object)]
pub struct BufferFrame {
    /// Frame width in physical pixels.
    pub width: u32,
    /// Frame height in physical pixels.
    pub height: u32,
    /// Device scale factor used to render the frame.
    pub scale: f64,
    /// RGBA8 pixels, row-major, 4 bytes per pixel.
    pub data: Uint8Array,
}

#[napi]
pub struct BufferRenderer {
    width: u32,
    height: u32,
    scale: f64,
}

#[napi]
impl BufferRenderer {
    #[napi(factory)]
    pub fn create(options: BufferRendererOptions) -> Result<Self> {
        let (width, height, scale) = validate_options(options)?;
        Ok(Self {
            width,
            height,
            scale,
        })
    }

    /// Resize the virtual surface. Dimensions are CSS pixels; the returned
    /// frame dimensions are multiplied by `scale`.
    #[napi]
    pub fn resize(&mut self, options: BufferRendererOptions) -> Result<()> {
        let (width, height, scale) = validate_options(options)?;
        self.width = width;
        self.height = height;
        self.scale = scale;
        Ok(())
    }

    /// Resolve the document and render it into a fresh RGBA8 buffer.
    ///
    /// This is a deliberately simple first architecture pass: every call paints
    /// the whole viewport. Later we can layer dirty-region tracking and buffer
    /// reuse on top without changing the separation from the native window path.
    #[napi]
    pub fn render(&mut self, doc: &mut DocHandle) -> BufferFrame {
        let render_width = scaled_dimension(self.width, self.scale);
        let render_height = scaled_dimension(self.height, self.scale);
        let scale = self.scale as f32;

        let mut base = doc.base.doc.borrow_mut();
        base.set_viewport(Viewport::new(
            render_width,
            render_height,
            scale,
            ColorScheme::Light,
        ));
        base.resolve(0.0);

        let data = render_to_buffer::<VelloCpuImageRenderer, _>(
            |scene| {
                scene.fill(
                    Fill::NonZero,
                    Default::default(),
                    Color::WHITE,
                    Default::default(),
                    &Rect::new(0.0, 0.0, render_width as f64, render_height as f64),
                );

                paint_scene(
                    scene,
                    &mut base,
                    self.scale,
                    render_width,
                    render_height,
                    0,
                    0,
                );
            },
            render_width,
            render_height,
        );

        BufferFrame {
            width: render_width,
            height: render_height,
            scale: self.scale,
            data: data.into(),
        }
    }
}

fn validate_options(options: BufferRendererOptions) -> Result<(u32, u32, f64)> {
    let width = validate_css_dimension("width", options.width)?;
    let height = validate_css_dimension("height", options.height)?;
    let scale = options.scale.unwrap_or(1.0);
    if !scale.is_finite() || scale <= 0.0 {
        return Err(Error::from_reason(
            "scale must be a finite positive number".to_string(),
        ));
    }
    Ok((width, height, scale))
}

fn validate_css_dimension(label: &str, value: f64) -> Result<u32> {
    if !value.is_finite() || value <= 0.0 || value > u32::MAX as f64 {
        return Err(Error::from_reason(format!(
            "{label} must be a finite positive number"
        )));
    }
    Ok(value.round() as u32)
}

fn scaled_dimension(value: u32, scale: f64) -> u32 {
    ((value as f64) * scale).round().clamp(1.0, u32::MAX as f64) as u32
}
