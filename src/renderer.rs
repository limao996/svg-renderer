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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBackend {
    Cpu,
    Vulkan,
}

pub struct CpuSvgRenderer {
    resource_provider: CachedResourceProvider,
    readback_buffer: Vec<u8>,
}

impl CpuSvgRenderer {
    pub fn new() -> Result<Self, SvgRenderError> {
        Ok(Self {
            resource_provider: CachedResourceProvider::new(FontMgr::default()),
            readback_buffer: Vec::new(),
        })
    }

    pub fn add_resource_search_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.resource_provider.add_search_dir(dir);
        self
    }

    pub fn set_resource_search_dirs<I, P>(&mut self, dirs: I) -> &mut Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.resource_provider.set_search_dirs(dirs);
        self
    }

    pub fn render_svg(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<ImageData, SvgRenderError> {
        let mut surface = self.render_surface(svg, options)?;
        read_surface_pixels(&mut surface, options, &mut self.readback_buffer)
    }

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

#[cfg(feature = "vulkan-backend")]
pub struct VulkanSvgRenderer {
    vulkan: Arc<VulkanState>,
    context: gpu::DirectContext,
    resource_provider: CachedResourceProvider,
    readback_buffer: Vec<u8>,
}

#[cfg(feature = "vulkan-backend")]
impl VulkanSvgRenderer {
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

    pub fn add_resource_search_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.resource_provider.add_search_dir(dir);
        self
    }

    pub fn set_resource_search_dirs<I, P>(&mut self, dirs: I) -> &mut Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.resource_provider.set_search_dirs(dirs);
        self
    }

    pub fn render_svg(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<ImageData, SvgRenderError> {
        let mut surface = self.render_surface(svg, options)?;
        self.context.flush_and_submit();
        read_surface_pixels(&mut surface, options, &mut self.readback_buffer)
    }

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
        self.context.abandon();
        let _ = Arc::strong_count(&self.vulkan);
    }
}

pub struct SvgRenderer {
    renderer: SvgRendererBackend,
}

enum SvgRendererBackend {
    Cpu(CpuSvgRenderer),
    #[cfg(feature = "vulkan-backend")]
    Vulkan(VulkanSvgRenderer),
}

impl SvgRenderer {
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

    pub fn backend(&self) -> RenderBackend {
        match &self.renderer {
            SvgRendererBackend::Cpu(_) => RenderBackend::Cpu,
            #[cfg(feature = "vulkan-backend")]
            SvgRendererBackend::Vulkan(_) => RenderBackend::Vulkan,
        }
    }

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

fn rgba_image_info(width: i32, height: i32) -> ImageInfo {
    ImageInfo::new(
        (width, height),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    )
}

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

pub fn render_svg(
    svg: impl AsRef<[u8]>,
    options: &RenderOptions,
) -> Result<ImageData, SvgRenderError> {
    SvgRenderer::new()?.render_svg(svg, options)
}

pub fn render_svg_to_png(
    svg: impl AsRef<[u8]>,
    options: &RenderOptions,
) -> Result<Vec<u8>, SvgRenderError> {
    SvgRenderer::new()?.render_svg_to_png(svg, options)
}

pub fn render_svg_to_jpeg(
    svg: impl AsRef<[u8]>,
    options: &RenderOptions,
    jpeg_options: JpegOptions,
) -> Result<Vec<u8>, SvgRenderError> {
    SvgRenderer::new()?.render_svg_to_jpeg(svg, options, jpeg_options)
}

pub fn render_svg_to_webp(
    svg: impl AsRef<[u8]>,
    options: &RenderOptions,
    webp_options: WebpOptions,
) -> Result<Vec<u8>, SvgRenderError> {
    SvgRenderer::new()?.render_svg_to_webp(svg, options, webp_options)
}
