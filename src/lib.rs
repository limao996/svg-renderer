#![cfg_attr(not(feature = "vulkan-backend"), allow(unused))]

#[cfg(not(feature = "vulkan-backend"))]
compile_error!(
    "svg-renderer requires the default `vulkan-backend` feature; this crate does not provide a non-Vulkan backend."
);

use std::{
    collections::HashMap,
    ffi::CString,
    fs,
    os::raw::c_void,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use ash::{Entry, vk, vk::Handle};
use skia_safe::{
    AlphaType, Color, ColorType, Data, FontMgr, IPoint, ImageInfo, Size, Typeface,
    gpu::{self, direct_contexts, vk as skia_vk},
    jpeg_encoder, png_encoder,
    resources::{NativeResourceProvider, ResourceProvider, UReqResourceProvider},
    svg::Dom,
    webp_encoder,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderSize {
    pub width: u32,
    pub height: u32,
}

impl RenderSize {
    pub fn new(width: u32, height: u32) -> Result<Self, SvgRenderError> {
        if width == 0 || height == 0 {
            return Err(SvgRenderError::InvalidSize { width, height });
        }

        if width > i32::MAX as u32 || height > i32::MAX as u32 {
            return Err(SvgRenderError::InvalidSize { width, height });
        }

        Ok(Self { width, height })
    }

    fn as_i32_pair(self) -> (i32, i32) {
        (self.width as i32, self.height as i32)
    }
}

#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub size: RenderSize,
    pub clear_color: Color,
    pub sample_count: usize,
}

impl RenderOptions {
    pub fn new(width: u32, height: u32) -> Result<Self, SvgRenderError> {
        Ok(Self {
            size: RenderSize::new(width, height)?,
            clear_color: Color::TRANSPARENT,
            sample_count: 4,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub row_bytes: usize,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JpegOptions {
    pub quality: u32,
    pub downsample: JpegDownsample,
    pub alpha_option: JpegAlphaOption,
}

impl Default for JpegOptions {
    fn default() -> Self {
        Self {
            quality: 90,
            downsample: JpegDownsample::BothDirections,
            alpha_option: JpegAlphaOption::Ignore,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JpegDownsample {
    BothDirections,
    Horizontal,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JpegAlphaOption {
    Ignore,
    BlendOnBlack,
}

impl From<JpegOptions> for jpeg_encoder::Options {
    fn from(value: JpegOptions) -> Self {
        let downsample = match value.downsample {
            JpegDownsample::BothDirections => jpeg_encoder::Downsample::BothDirections,
            JpegDownsample::Horizontal => jpeg_encoder::Downsample::Horizontal,
            JpegDownsample::No => jpeg_encoder::Downsample::No,
        };
        let alpha_option = match value.alpha_option {
            JpegAlphaOption::Ignore => jpeg_encoder::AlphaOption::Ignore,
            JpegAlphaOption::BlendOnBlack => jpeg_encoder::AlphaOption::BlendOnBlack,
        };

        Self {
            quality: value.quality.clamp(0, 100),
            downsample,
            alpha_option,
            ..jpeg_encoder::Options::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WebpOptions {
    pub compression: WebpCompression,
    pub quality: f32,
}

impl Default for WebpOptions {
    fn default() -> Self {
        Self {
            compression: WebpCompression::Lossy,
            quality: 90.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebpCompression {
    Lossy,
    Lossless,
}

impl From<WebpOptions> for webp_encoder::Options {
    fn from(value: WebpOptions) -> Self {
        let compression = match value.compression {
            WebpCompression::Lossy => webp_encoder::Compression::Lossy,
            WebpCompression::Lossless => webp_encoder::Compression::Lossless,
        };

        Self {
            compression,
            quality: value.quality.clamp(0.0, 100.0),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SvgRenderError {
    #[error("invalid render size {width}x{height}")]
    InvalidSize { width: u32, height: u32 },
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
}

struct VulkanState {
    entry: Entry,
    instance: ash::Instance,
    physical_device: vk::PhysicalDevice,
    device: ash::Device,
    queue: vk::Queue,
    queue_family_index: u32,
}

fn load_vulkan_entry() -> Result<Entry, SvgRenderError> {
    match unsafe { Entry::load() } {
        Ok(entry) => Ok(entry),
        Err(default_error) => {
            load_vulkan_entry_from_platform_names().map_err(|_| default_error.into())
        }
    }
}

fn load_vulkan_entry_from_platform_names() -> Result<Entry, ash::LoadingError> {
    for library_name in VULKAN_LIBRARY_NAMES {
        if let Ok(entry) = unsafe { Entry::load_from(library_name) } {
            return Ok(entry);
        }
    }

    unsafe { Entry::load() }
}

#[cfg(target_os = "windows")]
const VULKAN_LIBRARY_NAMES: &[&str] = &["vulkan-1.dll"];

#[cfg(target_os = "linux")]
const VULKAN_LIBRARY_NAMES: &[&str] = &["libvulkan.so.1", "libvulkan.so"];

#[cfg(target_os = "android")]
const VULKAN_LIBRARY_NAMES: &[&str] = &["libvulkan.so"];

#[cfg(any(target_os = "macos", target_os = "ios"))]
const VULKAN_LIBRARY_NAMES: &[&str] = &[
    "libvulkan.1.dylib",
    "libvulkan.dylib",
    "libMoltenVK.dylib",
    "MoltenVK.framework/MoltenVK",
];

#[cfg(all(
    not(target_os = "windows"),
    not(target_os = "linux"),
    not(target_os = "android"),
    not(target_os = "macos"),
    not(target_os = "ios")
))]
const VULKAN_LIBRARY_NAMES: &[&str] = &[];

#[derive(Debug, Clone)]
struct CachedResourceProvider {
    inner: Arc<UReqResourceProvider>,
    cache: Arc<Mutex<HashMap<String, Data>>>,
    search_dirs: Arc<Mutex<Vec<PathBuf>>>,
}

impl CachedResourceProvider {
    fn new(font_mgr: impl Into<FontMgr>) -> Self {
        Self {
            inner: Arc::new(UReqResourceProvider::new(font_mgr)),
            cache: Arc::new(Mutex::new(HashMap::new())),
            search_dirs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn add_search_dir(&self, dir: impl Into<PathBuf>) {
        self.search_dirs
            .lock()
            .expect("resource search dir mutex poisoned")
            .push(dir.into());
    }

    fn set_search_dirs<I, P>(&self, dirs: I)
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        let mut search_dirs = self
            .search_dirs
            .lock()
            .expect("resource search dir mutex poisoned");
        search_dirs.clear();
        search_dirs.extend(dirs.into_iter().map(Into::into));
    }

    fn cache_key(resource_path: &str, resource_name: &str) -> String {
        if resource_path.is_empty() {
            resource_name.to_owned()
        } else {
            format!("{resource_path}\0{resource_name}")
        }
    }

    fn load_local(&self, resource_path: &str, resource_name: &str) -> Option<Data> {
        for path in self.local_candidates(resource_path, resource_name) {
            if let Ok(bytes) = fs::read(path) {
                return Some(Data::new_copy(&bytes));
            }
        }

        None
    }

    fn local_candidates(&self, resource_path: &str, resource_name: &str) -> Vec<PathBuf> {
        let resource_name = resource_name.trim();
        if resource_name.is_empty()
            || is_url_like(resource_name)
            || resource_name.starts_with("data:")
        {
            return Vec::new();
        }

        let resource_name_path = Path::new(resource_name);
        if resource_name_path.is_absolute() {
            return vec![resource_name_path.to_path_buf()];
        }

        let mut candidates = Vec::new();
        if !resource_path.is_empty() && !is_url_like(resource_path) {
            candidates.push(Path::new(resource_path).join(resource_name_path));
        }

        if let Ok(search_dirs) = self.search_dirs.lock() {
            candidates.extend(search_dirs.iter().map(|dir| dir.join(resource_name_path)));
        }

        candidates
    }
}

fn is_url_like(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://") || value.starts_with("file://")
}

impl ResourceProvider for CachedResourceProvider {
    fn load(&self, resource_path: &str, resource_name: &str) -> Option<Data> {
        let key = Self::cache_key(resource_path, resource_name);

        if let Some(data) = self.cache.lock().ok()?.get(&key).cloned() {
            return Some(data);
        }

        let data = self
            .load_local(resource_path, resource_name)
            .or_else(|| self.inner.load(resource_path, resource_name))?;
        self.cache.lock().ok()?.insert(key, data.clone());
        Some(data)
    }

    fn load_typeface(&self, name: &str, url: &str) -> Option<Typeface> {
        self.inner.load_typeface(name, url)
    }

    fn font_mgr(&self) -> FontMgr {
        self.inner.font_mgr()
    }
}

impl VulkanState {
    fn new() -> Result<Self, SvgRenderError> {
        let entry = load_vulkan_entry()?;
        let app_name = CString::new("svg-renderer").expect("static app name has no nul byte");
        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(1)
            .engine_name(&app_name)
            .engine_version(1)
            .api_version(vk::make_api_version(0, 1, 1, 0));
        let instance_info = vk::InstanceCreateInfo::default().application_info(&app_info);
        let instance = unsafe { entry.create_instance(&instance_info, None)? };

        let (physical_device, queue_family_index) = unsafe {
            instance
                .enumerate_physical_devices()?
                .into_iter()
                .find_map(|physical_device| {
                    instance
                        .get_physical_device_queue_family_properties(physical_device)
                        .into_iter()
                        .enumerate()
                        .find(|(_, properties)| {
                            properties.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                        })
                        .map(|(index, _)| (physical_device, index as u32))
                })
                .ok_or(SvgRenderError::NoVulkanDevice)?
        };

        let queue_priorities = [1.0f32];
        let queue_info = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family_index)
            .queue_priorities(&queue_priorities)];
        let device_info = vk::DeviceCreateInfo::default().queue_create_infos(&queue_info);
        let device = unsafe { instance.create_device(physical_device, &device_info, None)? };
        let queue = unsafe { device.get_device_queue(queue_family_index, 0) };

        Ok(Self {
            entry,
            instance,
            physical_device,
            device,
            queue,
            queue_family_index,
        })
    }

    fn get_proc(&self, proc: skia_vk::GetProcOf) -> skia_vk::GetProcResult {
        match proc {
            skia_vk::GetProcOf::Instance(instance, name) => unsafe {
                let instance = vk::Instance::from_raw(instance as u64);
                self.entry
                    .get_instance_proc_addr(instance, name)
                    .map_or(std::ptr::null(), |proc| proc as *const c_void)
            },
            skia_vk::GetProcOf::Device(device, name) => unsafe {
                let device = vk::Device::from_raw(device as u64);
                self.instance
                    .get_device_proc_addr(device, name)
                    .map_or(std::ptr::null(), |proc| proc as *const c_void)
            },
        }
    }
}

impl Drop for VulkanState {
    fn drop(&mut self) {
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

pub struct VulkanSvgRenderer {
    vulkan: Arc<VulkanState>,
    context: gpu::DirectContext,
    resource_provider: CachedResourceProvider,
}

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

        let (width, height) = options.size.as_i32_pair();
        let info = ImageInfo::new(
            (width, height),
            ColorType::RGBA8888,
            AlphaType::Premul,
            None,
        );
        let row_bytes = width as usize * 4;
        let mut rgba = vec![0u8; row_bytes * height as usize];

        if !surface.read_pixels(&info, &mut rgba, row_bytes, IPoint::new(0, 0)) {
            return Err(SvgRenderError::ReadPixels);
        }

        Ok(ImageData {
            width: options.size.width,
            height: options.size.height,
            row_bytes,
            rgba,
        })
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
        let info = ImageInfo::new(
            (width, height),
            ColorType::RGBA8888,
            AlphaType::Premul,
            None,
        );
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

        let resource_provider: NativeResourceProvider = self.resource_provider.clone().into();
        let mut dom = Dom::from_bytes(svg.as_ref(), resource_provider)
            .map_err(|_| SvgRenderError::SvgParse)?;
        dom.set_container_size(Size::new(width as f32, height as f32));

        let canvas = surface.canvas();
        canvas.clear(options.clear_color);
        dom.render(canvas);

        Ok(surface)
    }
}

impl Drop for VulkanSvgRenderer {
    fn drop(&mut self) {
        self.context.abandon();
        let _ = Arc::strong_count(&self.vulkan);
    }
}

pub fn render_svg(
    svg: impl AsRef<[u8]>,
    options: &RenderOptions,
) -> Result<ImageData, SvgRenderError> {
    VulkanSvgRenderer::new()?.render_svg(svg, options)
}

pub fn render_svg_to_png(
    svg: impl AsRef<[u8]>,
    options: &RenderOptions,
) -> Result<Vec<u8>, SvgRenderError> {
    VulkanSvgRenderer::new()?.render_svg_to_png(svg, options)
}

pub fn render_svg_to_jpeg(
    svg: impl AsRef<[u8]>,
    options: &RenderOptions,
    jpeg_options: JpegOptions,
) -> Result<Vec<u8>, SvgRenderError> {
    VulkanSvgRenderer::new()?.render_svg_to_jpeg(svg, options, jpeg_options)
}

pub fn render_svg_to_webp(
    svg: impl AsRef<[u8]>,
    options: &RenderOptions,
    webp_options: WebpOptions,
) -> Result<Vec<u8>, SvgRenderError> {
    VulkanSvgRenderer::new()?.render_svg_to_webp(svg, options, webp_options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_zero_sized_render_targets() {
        assert!(matches!(
            RenderSize::new(0, 32),
            Err(SvgRenderError::InvalidSize { .. })
        ));
        assert!(matches!(
            RenderSize::new(32, 0),
            Err(SvgRenderError::InvalidSize { .. })
        ));
    }

    #[test]
    fn default_render_options_are_valid() {
        let options = RenderOptions::new(64, 48).unwrap();

        assert_eq!(options.size.width, 64);
        assert_eq!(options.size.height, 48);
        assert_eq!(options.sample_count, 4);
    }

    #[test]
    fn cloned_resource_provider_shares_cache() {
        let provider = CachedResourceProvider::new(FontMgr::default());
        let cloned = provider.clone();

        assert!(Arc::ptr_eq(&provider.cache, &cloned.cache));
    }

    #[test]
    fn cloned_resource_provider_shares_search_dirs() {
        let provider = CachedResourceProvider::new(FontMgr::default());
        let cloned = provider.clone();

        provider.add_search_dir("assets");

        assert_eq!(
            cloned.local_candidates("", "image.png"),
            vec![PathBuf::from("assets").join("image.png")]
        );
    }
}
