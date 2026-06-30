use ash::vk;

#[derive(Debug, thiserror::Error)]
pub enum SvgRenderError {
    #[error("invalid render size {width}x{height}")]
    InvalidSize { width: u32, height: u32 },
    #[error("invalid pipeline worker count {workers}; expected at least 1")]
    InvalidWorkerCount { workers: usize },
    #[error("failed to load Vulkan loader: {0}")]
    VulkanLoader(#[from] ash::LoadingError),
    #[error("Vulkan call failed: {0:?}")]
    Vulkan(#[from] vk::Result),
    #[error("no Vulkan physical device with graphics queue was found")]
    NoVulkanDevice,
    #[error("failed to create Skia Vulkan direct context")]
    SkiaContext,
    #[error("failed to parse SVG")]
    SvgParse,
    #[error("failed to create Vulkan render target")]
    RenderTarget,
    #[error("failed to read pixels from Vulkan render target")]
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
