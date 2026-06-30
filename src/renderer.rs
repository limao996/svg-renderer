use std::path::PathBuf;

#[cfg(feature = "vulkan-backend")]
use std::sync::Arc;

#[cfg(feature = "vulkan-backend")]
use ash::vk::Handle;
#[cfg(feature = "vulkan-backend")]
use skia_safe::gpu::{self, direct_contexts, vk as skia_vk};
use skia_safe::{
    AlphaType, ColorType, FontMgr, IPoint, ImageInfo, Size, jpeg_encoder, png_encoder,
    resources::NativeResourceProvider, surfaces, svg::Dom, webp_encoder,
};

#[cfg(feature = "vulkan-backend")]
use crate::VulkanState;
use crate::{
    CachedResourceProvider, ImageData, JpegOptions, RenderOptions, SvgRenderError, WebpOptions,
};

/// Rendering backend kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBackend {
    /// Skia raster (CPU) backend.
    Cpu,
    /// Skia Vulkan (GPU) backend.
    Vulkan,
}

/// CPU-based SVG renderer using Skia's raster backend.
///
/// Suitable for most environments; no GPU required. Each instance keeps
/// an internal readback buffer that is reused across calls to reduce
/// allocation overhead.
pub struct CpuSvgRenderer {
    resource_provider: CachedResourceProvider,
    /// Reusable readback buffer to avoid per-call allocations.
    readback_buffer: Vec<u8>,
}

impl CpuSvgRenderer {
    /// Creates a new CPU renderer with a default font manager.
    pub fn new() -> Result<Self, SvgRenderError> {
        Ok(Self {
            resource_provider: CachedResourceProvider::new(FontMgr::default()),
            readback_buffer: Vec::new(),
        })
    }

    /// Appends a directory to the resource search path.
    ///
    /// Resources (fonts, images referenced by the SVG) are looked up in
    /// all registered directories and, as a fallback, via HTTP(S).
    pub fn add_resource_search_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.resource_provider.add_search_dir(dir);
        self
    }

    /// Replaces the resource search path with the given directories.
    pub fn set_resource_search_dirs<I, P>(&mut self, dirs: I) -> &mut Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.resource_provider.set_search_dirs(dirs);
        self
    }

    /// Renders an SVG into raw RGBA pixel data.
    pub fn render_svg(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<ImageData, SvgRenderError> {
        let mut surface = self.render_surface(svg, options)?;
        read_surface_pixels(&mut surface, options, &mut self.readback_buffer)
    }

    /// Renders an SVG and encodes the result as PNG.
    pub fn render_svg_to_png(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        let mut surface = self.render_surface(svg, options)?;
        let image = surface.image_snapshot();
        let data = png_encoder::encode_image(None, &image, &png_encoder::Options::default())
            .ok_or(SvgRenderError::PngEncode)?;

        Ok(data.as_bytes().to_vec())
    }

    /// Renders an SVG and encodes the result as JPEG.
    pub fn render_svg_to_jpeg(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
        jpeg_options: JpegOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        let mut surface = self.render_surface(svg, options)?;
        let image = surface.image_snapshot();
        let data = jpeg_encoder::encode_image(None, &image, &jpeg_options.into())
            .ok_or(SvgRenderError::JpegEncode)?;

        Ok(data.as_bytes().to_vec())
    }

    /// Renders an SVG and encodes the result as WebP.
    pub fn render_svg_to_webp(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
        webp_options: WebpOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        let mut surface = self.render_surface(svg, options)?;
        let image = surface.image_snapshot();
        let data = webp_encoder::encode_image(None, &image, &webp_options.into())
            .ok_or(SvgRenderError::WebpEncode)?;

        Ok(data.as_bytes().to_vec())
    }

    /// Internal: creates a raster surface, parses the SVG, renders onto it.
    fn render_surface(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<skia_safe::Surface, SvgRenderError> {
        let (width, height) = options.size.as_i32_pair();
        let info = rgba_image_info(width, height);
        let mut surface =
            surfaces::raster(&info, None, None).ok_or(SvgRenderError::RenderTarget)?;
        render_dom(surface.canvas(), svg, options, &self.resource_provider)?;
        Ok(surface)
    }
}

/// Vulkan GPU-accelerated SVG renderer using Skia's Vulkan backend.
///
/// Requires the `vulkan-backend` feature. Falls back to CPU if Vulkan
/// initialization fails (see [`SvgRenderer::new`]).
#[cfg(feature = "vulkan-backend")]
pub struct VulkanSvgRenderer {
    vulkan: Arc<VulkanState>,
    context: gpu::DirectContext,
    resource_provider: CachedResourceProvider,
    readback_buffer: Vec<u8>,
}

#[cfg(feature = "vulkan-backend")]
impl VulkanSvgRenderer {
    /// Creates a Vulkan renderer: loads the Vulkan library, enumerates
    /// devices, picks a graphics-capable queue, and wraps it in a Skia
    /// direct context.
    pub fn new() -> Result<Self, SvgRenderError> {
        let vulkan = Arc::new(VulkanState::new()?);
        let get_proc_vulkan = Arc::clone(&vulkan);
        let get_proc = move |proc| get_proc_vulkan.get_proc(proc);

        let backend_context = unsafe {
            skia_vk::BackendContext::new_builder(
                vulkan.instance.handle().as_raw() as skia_vk::Instance,
                vulkan.physical_device.as_raw() as skia_vk::PhysicalDevice,
                vulkan.device.handle().as_raw() as skia_vk::Device,
                (
                    vulkan.queue.as_raw() as skia_vk::Queue,
                    vulkan.queue_family_index as usize,
                ),
                &get_proc,
                Some(skia_vk::Version::new(1, 1, 0)),
            )
            .build()
        };

        let context = direct_contexts::make_vulkan(&backend_context, None)
            .ok_or(SvgRenderError::SkiaContext)?;
        let resource_provider = CachedResourceProvider::new(FontMgr::default());

        Ok(Self {
            vulkan,
            context,
            resource_provider,
            readback_buffer: Vec::new(),
        })
    }

    /// Appends a directory to the resource search path.
    pub fn add_resource_search_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.resource_provider.add_search_dir(dir);
        self
    }

    /// Replaces the resource search path with the given directories.
    pub fn set_resource_search_dirs<I, P>(&mut self, dirs: I) -> &mut Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.resource_provider.set_search_dirs(dirs);
        self
    }

    /// Renders an SVG into raw RGBA pixel data.
    ///
    /// Flushes the GPU command buffer before readback.
    pub fn render_svg(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<ImageData, SvgRenderError> {
        let mut surface = self.render_surface(svg, options)?;
        self.context.flush_and_submit();
        read_surface_pixels(&mut surface, options, &mut self.readback_buffer)
    }

    /// Renders an SVG and encodes the result as PNG.
    ///
    /// Flushes the GPU command buffer before encoding.
    pub fn render_svg_to_png(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        let mut surface = self.render_surface(svg, options)?;
        self.context.flush_and_submit();
        let image = surface.image_snapshot();
        let data = png_encoder::encode_image(
            Some(&mut self.context),
            &image,
            &png_encoder::Options::default(),
        )
        .ok_or(SvgRenderError::PngEncode)?;

        Ok(data.as_bytes().to_vec())
    }

    /// Renders an SVG and encodes the result as JPEG.
    pub fn render_svg_to_jpeg(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
        jpeg_options: JpegOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        let mut surface = self.render_surface(svg, options)?;
        self.context.flush_and_submit();
        let image = surface.image_snapshot();
        let data =
            jpeg_encoder::encode_image(Some(&mut self.context), &image, &jpeg_options.into())
                .ok_or(SvgRenderError::JpegEncode)?;

        Ok(data.as_bytes().to_vec())
    }

    /// Renders an SVG and encodes the result as WebP.
    pub fn render_svg_to_webp(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
        webp_options: WebpOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        let mut surface = self.render_surface(svg, options)?;
        self.context.flush_and_submit();
        let image = surface.image_snapshot();
        let data =
            webp_encoder::encode_image(Some(&mut self.context), &image, &webp_options.into())
                .ok_or(SvgRenderError::WebpEncode)?;

        Ok(data.as_bytes().to_vec())
    }

    /// Internal: creates a GPU-backed surface, parses the SVG, renders it.
    fn render_surface(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<skia_safe::Surface, SvgRenderError> {
        let (width, height) = options.size.as_i32_pair();
        let info = rgba_image_info(width, height);
        let mut surface = gpu::surfaces::render_target(
            &mut self.context,
            gpu::Budgeted::No,
            &info,
            options.sample_count,
            gpu::SurfaceOrigin::TopLeft,
            None,
            false,
            false,
        )
        .ok_or(SvgRenderError::RenderTarget)?;
        render_dom(surface.canvas(), svg, options, &self.resource_provider)?;
        Ok(surface)
    }
}

#[cfg(feature = "vulkan-backend")]
impl Drop for VulkanSvgRenderer {
    fn drop(&mut self) {
        // Abandon the Skia context so it doesn't try to destroy Vulkan
        // resources that the VulkanState drop will handle.
        self.context.abandon();
        // Keep a reference alive via strong_count read to prevent
        // compiler from optimizing away the Arc.
        let _ = Arc::strong_count(&self.vulkan);
    }
}

/// Auto-selecting SVG renderer.
///
/// Tries Vulkan first (when the `vulkan-backend` feature is enabled);
/// falls back to CPU on any error during Vulkan init. This makes it a
/// safe default for most use cases.
pub struct SvgRenderer {
    renderer: SvgRendererBackend,
}

enum SvgRendererBackend {
    Cpu(CpuSvgRenderer),
    #[cfg(feature = "vulkan-backend")]
    Vulkan(VulkanSvgRenderer),
}

impl SvgRenderer {
    /// Creates a renderer, preferring Vulkan over CPU.
    pub fn new() -> Result<Self, SvgRenderError> {
        #[cfg(feature = "vulkan-backend")]
        if let Ok(renderer) = VulkanSvgRenderer::new() {
            return Ok(Self {
                renderer: SvgRendererBackend::Vulkan(renderer),
            });
        }

        Ok(Self {
            renderer: SvgRendererBackend::Cpu(CpuSvgRenderer::new()?),
        })
    }

    /// Returns which backend is currently in use.
    pub fn backend(&self) -> RenderBackend {
        match &self.renderer {
            SvgRendererBackend::Cpu(_) => RenderBackend::Cpu,
            #[cfg(feature = "vulkan-backend")]
            SvgRendererBackend::Vulkan(_) => RenderBackend::Vulkan,
        }
    }

    /// Appends a directory to the resource search path.
    pub fn add_resource_search_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        match &mut self.renderer {
            SvgRendererBackend::Cpu(renderer) => {
                renderer.add_resource_search_dir(dir);
            }
            #[cfg(feature = "vulkan-backend")]
            SvgRendererBackend::Vulkan(renderer) => {
                renderer.add_resource_search_dir(dir);
            }
        }
        self
    }

    /// Replaces the resource search path with the given directories.
    pub fn set_resource_search_dirs<I, P>(&mut self, dirs: I) -> &mut Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        match &mut self.renderer {
            SvgRendererBackend::Cpu(renderer) => {
                renderer.set_resource_search_dirs(dirs);
            }
            #[cfg(feature = "vulkan-backend")]
            SvgRendererBackend::Vulkan(renderer) => {
                renderer.set_resource_search_dirs(dirs);
            }
        }
        self
    }

    /// Renders an SVG into raw RGBA pixel data.
    pub fn render_svg(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<ImageData, SvgRenderError> {
        match &mut self.renderer {
            SvgRendererBackend::Cpu(renderer) => renderer.render_svg(svg, options),
            #[cfg(feature = "vulkan-backend")]
            SvgRendererBackend::Vulkan(renderer) => renderer.render_svg(svg, options),
        }
    }

    /// Renders an SVG and encodes the result as PNG.
    pub fn render_svg_to_png(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        match &mut self.renderer {
            SvgRendererBackend::Cpu(renderer) => renderer.render_svg_to_png(svg, options),
            #[cfg(feature = "vulkan-backend")]
            SvgRendererBackend::Vulkan(renderer) => renderer.render_svg_to_png(svg, options),
        }
    }

    /// Renders an SVG and encodes the result as JPEG.
    pub fn render_svg_to_jpeg(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
        jpeg_options: JpegOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        match &mut self.renderer {
            SvgRendererBackend::Cpu(renderer) => {
                renderer.render_svg_to_jpeg(svg, options, jpeg_options)
            }
            #[cfg(feature = "vulkan-backend")]
            SvgRendererBackend::Vulkan(renderer) => {
                renderer.render_svg_to_jpeg(svg, options, jpeg_options)
            }
        }
    }

    /// Renders an SVG and encodes the result as WebP.
    pub fn render_svg_to_webp(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
        webp_options: WebpOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        match &mut self.renderer {
            SvgRendererBackend::Cpu(renderer) => {
                renderer.render_svg_to_webp(svg, options, webp_options)
            }
            #[cfg(feature = "vulkan-backend")]
            SvgRendererBackend::Vulkan(renderer) => {
                renderer.render_svg_to_webp(svg, options, webp_options)
            }
        }
    }
}

/// Creates an RGBA8888 premultiplied-alpha image info for the given dimensions.
fn rgba_image_info(width: i32, height: i32) -> ImageInfo {
    ImageInfo::new(
        (width, height),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    )
}

/// Parses the SVG data, sets the container size, and renders onto `canvas`.
fn render_dom(
    canvas: &skia_safe::Canvas,
    svg: impl AsRef<[u8]>,
    options: &RenderOptions,
    resource_provider: &CachedResourceProvider,
) -> Result<(), SvgRenderError> {
    let (width, height) = options.size.as_i32_pair();
    let resource_provider: NativeResourceProvider = resource_provider.clone().into();
    let mut dom =
        Dom::from_bytes(svg.as_ref(), resource_provider).map_err(|_| SvgRenderError::SvgParse)?;
    dom.set_container_size(Size::new(width as f32, height as f32));

    canvas.clear(options.clear_color);
    dom.render(canvas);
    Ok(())
}

/// Reads back pixels from the surface into an [`ImageData`].
fn read_surface_pixels(
    surface: &mut skia_safe::Surface,
    options: &RenderOptions,
    readback_buffer: &mut Vec<u8>,
) -> Result<ImageData, SvgRenderError> {
    let (width, height) = options.size.as_i32_pair();
    let info = rgba_image_info(width, height);
    let row_bytes = width as usize * 4;
    let byte_len = row_bytes * height as usize;
    readback_buffer.resize(byte_len, 0);

    if !surface.read_pixels(&info, readback_buffer, row_bytes, IPoint::new(0, 0)) {
        return Err(SvgRenderError::ReadPixels);
    }

    Ok(ImageData {
        width: options.size.width,
        height: options.size.height,
        row_bytes,
        rgba: readback_buffer.clone(),
    })
}

/// Convenience: creates an [`SvgRenderer`], renders the SVG to raw RGBA.
pub fn render_svg(
    svg: impl AsRef<[u8]>,
    options: &RenderOptions,
) -> Result<ImageData, SvgRenderError> {
    SvgRenderer::new()?.render_svg(svg, options)
}

/// Convenience: creates an [`SvgRenderer`], renders the SVG, encodes as PNG.
pub fn render_svg_to_png(
    svg: impl AsRef<[u8]>,
    options: &RenderOptions,
) -> Result<Vec<u8>, SvgRenderError> {
    SvgRenderer::new()?.render_svg_to_png(svg, options)
}

/// Convenience: creates an [`SvgRenderer`], renders the SVG, encodes as JPEG.
pub fn render_svg_to_jpeg(
    svg: impl AsRef<[u8]>,
    options: &RenderOptions,
    jpeg_options: JpegOptions,
) -> Result<Vec<u8>, SvgRenderError> {
    SvgRenderer::new()?.render_svg_to_jpeg(svg, options, jpeg_options)
}

/// Convenience: creates an [`SvgRenderer`], renders the SVG, encodes as WebP.
pub fn render_svg_to_webp(
    svg: impl AsRef<[u8]>,
    options: &RenderOptions,
    webp_options: WebpOptions,
) -> Result<Vec<u8>, SvgRenderError> {
    SvgRenderer::new()?.render_svg_to_webp(svg, options, webp_options)
}
