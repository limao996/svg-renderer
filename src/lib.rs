#![cfg_attr(not(feature = "vulkan-backend"), allow(unused))]

#[cfg(not(feature = "vulkan-backend"))]
compile_error!(
    "svg-renderer requires the default `vulkan-backend` feature; this crate does not provide a non-Vulkan backend."
);

mod error;
mod image;
mod options;
mod pipeline;
mod renderer;
mod resource;
mod vulkan;

pub(crate) use resource::CachedResourceProvider;
pub(crate) use vulkan::VulkanState;

pub use error::SvgRenderError;
pub use image::ImageData;
pub use options::{
    JpegAlphaOption, JpegDownsample, JpegOptions, RenderOptions, RenderSize, WebpCompression,
    WebpOptions,
};
pub use pipeline::VulkanSvgPipelineRenderer;
pub use renderer::{
    VulkanSvgRenderer, render_svg, render_svg_to_jpeg, render_svg_to_png, render_svg_to_webp,
};
