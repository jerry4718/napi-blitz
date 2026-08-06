//! Renderer backend selection.
//!
//! Pick exactly one renderer feature at build time:
//!   `vello`          – GPU-only Vello (wgpu)
//!   `vello-hybrid`   – GPU + CPU hybrid Vello (default)
//!   `vello-cpu-*`    – CPU-only Vello
//!   `skia` / `skia-pixels` / `skia-softbuffer` – Skia

#[cfg(feature = "vello")]
pub use anyrender_vello::VelloWindowRenderer as CurrentRenderer;

#[cfg(feature = "vello-hybrid")]
pub use anyrender_vello_hybrid::VelloHybridWindowRenderer as CurrentRenderer;

#[cfg(feature = "vello-cpu-base")]
pub use anyrender_vello_cpu::VelloCpuWindowRenderer as CurrentRenderer;

#[cfg(feature = "skia")]
pub use anyrender_skia::SkiaWindowRenderer as CurrentRenderer;

#[cfg(any(feature = "skia-pixels", feature = "skia-softbuffer"))]
pub use anyrender_skia::raster::SkiaRasterWindowRenderer as CurrentRenderer;
