//! `svg-renderer` — a high-performance SVG-to-raster renderer powered by [Skia](https://skia.org).
//!
//! # Backends
//!
//! | Backend  | Feature          | Description                                  |
//! |----------|------------------|----------------------------------------------|
//! | CPU      | (always)         | Renders via Skia's raster backend.           |
//! | Vulkan   | `vulkan-backend` | GPU-accelerated via Skia's Vulkan backend.   |
//!
//! # Pipeline mode
//!
//! [`SvgPipelineRenderer`] spawns dedicated worker threads for parallel
//! rendering, suitable for throughput-oriented workloads.
//!
//! # Quick start
//!
//! ```no_run
//! use svg_renderer::{SvgRenderer, RenderOptions};
//!
//! let svg_data = b"<svg xmlns='http://www.w3.org/2000/svg'></svg>";
//! let opts = RenderOptions::new(800, 600).unwrap();
//! let mut renderer = SvgRenderer::new().unwrap();
//! let image = renderer.render_svg(svg_data, &opts).unwrap();
//! println!("{}x{} RGBA image", image.width, image.height);
//! ```

mod error;
mod image;
mod options;
mod pipeline;
mod renderer;
mod resource;
#[cfg(feature = "vulkan-backend")]
mod vulkan;

pub(crate) use resource::CachedResourceProvider;
#[cfg(feature = "vulkan-backend")]
pub(crate) use vulkan::VulkanState;

pub use error::SvgRenderError;
pub use image::ImageData;

/// JPEG alpha handling behavior.
pub use options::JpegAlphaOption;
/// JPEG chroma subsampling mode.
pub use options::JpegDownsample;
/// JPEG encoding options.
pub use options::JpegOptions;
/// Render options: size, clear color, MSAA sample count.
pub use options::RenderOptions;
/// Render size configuration.
pub use options::RenderSize;
/// WebP compression mode (lossy / lossless).
pub use options::WebpCompression;
/// WebP encoding options.
pub use options::WebpOptions;

#[cfg(feature = "vulkan-backend")]
pub use pipeline::VulkanSvgPipelineRenderer;
pub use pipeline::{CpuSvgPipelineRenderer, SvgPipelineRenderer};
#[cfg(feature = "vulkan-backend")]
pub use renderer::VulkanSvgRenderer;
pub use renderer::{
    CpuSvgRenderer, RenderBackend, SvgRenderer, render_svg, render_svg_to_jpeg, render_svg_to_png,
    render_svg_to_webp,
};
