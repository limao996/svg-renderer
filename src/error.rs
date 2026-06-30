#[cfg(feature = "vulkan-backend")]
use ash::vk;

/// Errors that can occur during SVG rendering.
#[derive(Debug, thiserror::Error)]
pub enum SvgRenderError {
    /// The requested render dimensions are zero or exceed `i32::MAX`.
    #[error("invalid render size {width}x{height}")]
    InvalidSize { width: u32, height: u32 },

    /// Pipeline worker count is 0; at least 1 worker is required.
    #[error("invalid pipeline worker count {workers}; expected at least 1")]
    InvalidWorkerCount { workers: usize },

    /// Failed to load the Vulkan shared library (e.g. `vulkan-1.dll`).
    #[cfg(feature = "vulkan-backend")]
    #[error("failed to load Vulkan loader: {0}")]
    VulkanLoader(#[from] ash::LoadingError),

    /// A raw Vulkan API call returned an error code.
    #[cfg(feature = "vulkan-backend")]
    #[error("Vulkan call failed: {0:?}")]
    Vulkan(#[from] vk::Result),

    /// No physical device with a graphics queue family was found.
    #[cfg(feature = "vulkan-backend")]
    #[error("no Vulkan physical device with graphics queue was found")]
    NoVulkanDevice,

    /// Skia failed to create a Vulkan-backed direct rendering context.
    #[cfg(feature = "vulkan-backend")]
    #[error("failed to create Skia Vulkan direct context")]
    SkiaContext,

    /// The SVG document could not be parsed by Skia's SVG DOM parser.
    #[error("failed to parse SVG")]
    SvgParse,

    /// Skia failed to allocate the raster or GPU render target surface.
    #[error("failed to create render target")]
    RenderTarget,

    /// Pixel readback from the render surface failed.
    #[error("failed to read pixels from render target")]
    ReadPixels,

    /// PNG encoding via Skia failed.
    #[error("failed to encode PNG")]
    PngEncode,

    /// JPEG encoding via Skia failed.
    #[error("failed to encode JPEG")]
    JpegEncode,

    /// WebP encoding via Skia failed.
    #[error("failed to encode WebP")]
    WebpEncode,

    /// The pipeline worker thread exited before the render job completed.
    #[error("pipeline renderer worker stopped before completing the render job")]
    PipelineClosed,
}
