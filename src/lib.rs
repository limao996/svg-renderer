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
pub use options::{
    JpegAlphaOption, JpegDownsample, JpegOptions, RenderOptions, RenderSize, WebpCompression,
    WebpOptions,
};
#[cfg(feature = "vulkan-backend")]
pub use pipeline::VulkanSvgPipelineRenderer;
pub use pipeline::{CpuSvgPipelineRenderer, SvgPipelineRenderer};
#[cfg(feature = "vulkan-backend")]
pub use renderer::VulkanSvgRenderer;
pub use renderer::{
    CpuSvgRenderer, RenderBackend, SvgRenderer, render_svg, render_svg_to_jpeg, render_svg_to_png,
    render_svg_to_webp,
};
