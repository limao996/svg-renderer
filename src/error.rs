#[cfg(feature = "vulkan-backend")]
use ash::vk;

#[derive(Debug, thiserror::Error)]
pub enum SvgRenderError {
    #[error("invalid render size {width}x{height}")]
    InvalidSize { width: u32, height: u32 },
    #[error("invalid pipeline worker count {workers}; expected at least 1")]
    InvalidWorkerCount { workers: usize },
    #[cfg(feature = "vulkan-backend")]
    #[error("failed to load Vulkan loader: {0}")]
    VulkanLoader(#[from] ash::LoadingError),
    #[cfg(feature = "vulkan-backend")]
    #[error("Vulkan call failed: {0:?}")]
    Vulkan(#[from] vk::Result),
    #[cfg(feature = "vulkan-backend")]
    #[error("no Vulkan physical device with graphics queue was found")]
    NoVulkanDevice,
    #[cfg(feature = "vulkan-backend")]
    #[error("failed to create Skia Vulkan direct context")]
    SkiaContext,
    #[error("failed to parse SVG")]
    SvgParse,
    #[error("failed to create render target")]
    RenderTarget,
    #[error("failed to read pixels from render target")]
    ReadPixels,
    #[error("failed to encode PNG")]
    PngEncode,
    #[error("failed to encode JPEG")]
    JpegEncode,
    #[error("failed to encode WebP")]
    WebpEncode,
    #[error("pipeline renderer worker stopped before completing the render job")]
    PipelineClosed,
}
