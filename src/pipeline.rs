use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    task::{Context, Poll, Waker},
    thread::{self, JoinHandle},
};

#[cfg(feature = "vulkan-backend")]
use crate::VulkanSvgRenderer;
use crate::{
    CpuSvgRenderer, ImageData, JpegOptions, RenderBackend, RenderOptions, SvgRenderError,
    WebpOptions,
};

pub struct CpuSvgPipelineRenderer {
    inner: PipelineInner,
}

impl CpuSvgPipelineRenderer {
    pub fn new(workers: usize) -> Result<Self, SvgRenderError> {
        Ok(Self {
            inner: PipelineInner::new(workers, RenderBackend::Cpu)?,
        })
    }

    pub fn set_resource_search_dirs<I, P>(&mut self, dirs: I) -> &mut Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.inner.set_resource_search_dirs(dirs);
        self
    }

    pub fn add_resource_search_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.inner.add_resource_search_dir(dir);
        self
    }

    pub async fn render_svg(
        &self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<ImageData, SvgRenderError> {
        self.inner.render_svg(svg, options).await
    }

    pub async fn render_svg_to_png(
        &self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        self.inner.render_svg_to_png(svg, options).await
    }

    pub async fn render_svg_to_jpeg(
        &self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
        jpeg_options: JpegOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        self.inner
            .render_svg_to_jpeg(svg, options, jpeg_options)
            .await
    }

    pub async fn render_svg_to_webp(
        &self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
        webp_options: WebpOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        self.inner
            .render_svg_to_webp(svg, options, webp_options)
            .await
    }
}

#[cfg(feature = "vulkan-backend")]
pub struct VulkanSvgPipelineRenderer {
    inner: PipelineInner,
}

#[cfg(feature = "vulkan-backend")]
impl VulkanSvgPipelineRenderer {
    pub fn new(workers: usize) -> Result<Self, SvgRenderError> {
        Ok(Self {
            inner: PipelineInner::new(workers, RenderBackend::Vulkan)?,
        })
    }

    pub fn set_resource_search_dirs<I, P>(&mut self, dirs: I) -> &mut Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.inner.set_resource_search_dirs(dirs);
        self
    }

    pub fn add_resource_search_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.inner.add_resource_search_dir(dir);
        self
    }

    pub async fn render_svg(
        &self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<ImageData, SvgRenderError> {
        self.inner.render_svg(svg, options).await
    }

    pub async fn render_svg_to_png(
        &self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        self.inner.render_svg_to_png(svg, options).await
    }

    pub async fn render_svg_to_jpeg(
        &self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
        jpeg_options: JpegOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        self.inner
            .render_svg_to_jpeg(svg, options, jpeg_options)
            .await
    }

    pub async fn render_svg_to_webp(
        &self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
        webp_options: WebpOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        self.inner
            .render_svg_to_webp(svg, options, webp_options)
            .await
    }
}

pub struct SvgPipelineRenderer {
    inner: PipelineInner,
}

impl SvgPipelineRenderer {
    pub fn new(workers: usize) -> Result<Self, SvgRenderError> {
        #[cfg(feature = "vulkan-backend")]
        if let Ok(inner) = PipelineInner::new(workers, RenderBackend::Vulkan) {
            return Ok(Self { inner });
        }

        Ok(Self {
            inner: PipelineInner::new(workers, RenderBackend::Cpu)?,
        })
    }

    pub fn backend(&self) -> RenderBackend {
        self.inner.backend()
    }

    pub fn set_resource_search_dirs<I, P>(&mut self, dirs: I) -> &mut Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.inner.set_resource_search_dirs(dirs);
        self
    }

    pub fn add_resource_search_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.inner.add_resource_search_dir(dir);
        self
    }

    pub async fn render_svg(
        &self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<ImageData, SvgRenderError> {
        self.inner.render_svg(svg, options).await
    }

    pub async fn render_svg_to_png(
        &self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        self.inner.render_svg_to_png(svg, options).await
    }

    pub async fn render_svg_to_jpeg(
        &self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
        jpeg_options: JpegOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        self.inner
            .render_svg_to_jpeg(svg, options, jpeg_options)
            .await
    }

    pub async fn render_svg_to_webp(
        &self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
        webp_options: WebpOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        self.inner
            .render_svg_to_webp(svg, options, webp_options)
            .await
    }
}

struct PipelineInner {
    workers: Vec<Worker>,
    next_worker: AtomicUsize,
    resource_search_dirs: Vec<PathBuf>,
    backend: RenderBackend,
}

impl PipelineInner {
    fn new(workers: usize, backend: RenderBackend) -> Result<Self, SvgRenderError> {
        if workers == 0 {
            return Err(SvgRenderError::InvalidWorkerCount { workers });
        }

        let mut worker_handles = Vec::with_capacity(workers);

        for _ in 0..workers {
            let (sender, receiver) = mpsc::channel();
            let (ready_sender, ready_receiver) = mpsc::channel();
            let handle = spawn_worker(receiver, ready_sender, backend);
            ready_receiver
                .recv()
                .map_err(|_| SvgRenderError::PipelineClosed)??;
            worker_handles.push(Worker {
                sender,
                handle: Some(handle),
            });
        }

        Ok(Self {
            workers: worker_handles,
            next_worker: AtomicUsize::new(0),
            resource_search_dirs: Vec::new(),
            backend,
        })
    }

    fn backend(&self) -> RenderBackend {
        self.backend
    }

    fn set_resource_search_dirs<I, P>(&mut self, dirs: I) -> &mut Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.resource_search_dirs = dirs.into_iter().map(Into::into).collect();
        self
    }

    fn add_resource_search_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.resource_search_dirs.push(dir.into());
        self
    }

    async fn render_svg(
        &self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<ImageData, SvgRenderError> {
        self.submit(RenderJobKind::Rgba, svg.as_ref(), options)
            .await?
            .into_image()
    }

    async fn render_svg_to_png(
        &self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        self.submit(RenderJobKind::Png, svg.as_ref(), options)
            .await?
            .into_bytes()
    }

    async fn render_svg_to_jpeg(
        &self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
        jpeg_options: JpegOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        self.submit(RenderJobKind::Jpeg(jpeg_options), svg.as_ref(), options)
            .await?
            .into_bytes()
    }

    async fn render_svg_to_webp(
        &self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
        webp_options: WebpOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        self.submit(RenderJobKind::Webp(webp_options), svg.as_ref(), options)
            .await?
            .into_bytes()
    }

    fn submit(
        &self,
        kind: RenderJobKind,
        svg: &[u8],
        options: &RenderOptions,
    ) -> RenderResponseFuture {
        let (response, future) = render_response_channel();
        let job = RenderJob {
            kind,
            svg: svg.to_vec(),
            options: options.clone(),
            resource_search_dirs: self.resource_search_dirs.clone(),
            response,
        };

        let worker_index = self.next_worker.fetch_add(1, Ordering::Relaxed) % self.workers.len();
        if self.workers[worker_index]
            .sender
            .send(WorkerMessage::Render(job))
            .is_err()
        {
            future.complete(Err(SvgRenderError::PipelineClosed));
        }

        future
    }
}

impl Drop for PipelineInner {
    fn drop(&mut self) {
        for worker in &mut self.workers {
            let _ = worker.sender.send(WorkerMessage::Stop);
            if let Some(handle) = worker.handle.take() {
                let _ = handle.join();
            }
        }
    }
}

struct Worker {
    sender: mpsc::Sender<WorkerMessage>,
    handle: Option<JoinHandle<()>>,
}

fn spawn_worker(
    receiver: mpsc::Receiver<WorkerMessage>,
    ready: mpsc::Sender<Result<(), SvgRenderError>>,
    backend: RenderBackend,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut renderer = match WorkerRenderer::new(backend) {
            Ok(renderer) => {
                let _ = ready.send(Ok(()));
                renderer
            }
            Err(error) => {
                let _ = ready.send(Err(error));
                return;
            }
        };

        loop {
            match receiver.recv() {
                Ok(WorkerMessage::Render(job)) => {
                    let result = run_job(
                        &mut renderer,
                        job.kind,
                        &job.svg,
                        &job.options,
                        job.resource_search_dirs,
                    );
                    job.response.complete(result);
                }
                Ok(WorkerMessage::Stop) | Err(_) => return,
            }
        }
    })
}

enum WorkerRenderer {
    Cpu(CpuSvgRenderer),
    #[cfg(feature = "vulkan-backend")]
    Vulkan(VulkanSvgRenderer),
}

impl WorkerRenderer {
    fn new(backend: RenderBackend) -> Result<Self, SvgRenderError> {
        match backend {
            RenderBackend::Cpu => Ok(Self::Cpu(CpuSvgRenderer::new()?)),
            RenderBackend::Vulkan => new_vulkan_worker_renderer(),
        }
    }

    fn set_resource_search_dirs(&mut self, dirs: Vec<PathBuf>) {
        match self {
            Self::Cpu(renderer) => {
                renderer.set_resource_search_dirs(dirs);
            }
            #[cfg(feature = "vulkan-backend")]
            Self::Vulkan(renderer) => {
                renderer.set_resource_search_dirs(dirs);
            }
        }
    }

    fn render_svg(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<ImageData, SvgRenderError> {
        match self {
            Self::Cpu(renderer) => renderer.render_svg(svg, options),
            #[cfg(feature = "vulkan-backend")]
            Self::Vulkan(renderer) => renderer.render_svg(svg, options),
        }
    }

    fn render_svg_to_png(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        match self {
            Self::Cpu(renderer) => renderer.render_svg_to_png(svg, options),
            #[cfg(feature = "vulkan-backend")]
            Self::Vulkan(renderer) => renderer.render_svg_to_png(svg, options),
        }
    }

    fn render_svg_to_jpeg(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
        jpeg_options: JpegOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        match self {
            Self::Cpu(renderer) => renderer.render_svg_to_jpeg(svg, options, jpeg_options),
            #[cfg(feature = "vulkan-backend")]
            Self::Vulkan(renderer) => renderer.render_svg_to_jpeg(svg, options, jpeg_options),
        }
    }

    fn render_svg_to_webp(
        &mut self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
        webp_options: WebpOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        match self {
            Self::Cpu(renderer) => renderer.render_svg_to_webp(svg, options, webp_options),
            #[cfg(feature = "vulkan-backend")]
            Self::Vulkan(renderer) => renderer.render_svg_to_webp(svg, options, webp_options),
        }
    }
}

#[cfg(feature = "vulkan-backend")]
fn new_vulkan_worker_renderer() -> Result<WorkerRenderer, SvgRenderError> {
    Ok(WorkerRenderer::Vulkan(VulkanSvgRenderer::new()?))
}

#[cfg(not(feature = "vulkan-backend"))]
fn new_vulkan_worker_renderer() -> Result<WorkerRenderer, SvgRenderError> {
    Ok(WorkerRenderer::Cpu(CpuSvgRenderer::new()?))
}

fn run_job(
    renderer: &mut WorkerRenderer,
    kind: RenderJobKind,
    svg: &[u8],
    options: &RenderOptions,
    resource_search_dirs: Vec<PathBuf>,
) -> Result<RenderJobOutput, SvgRenderError> {
    renderer.set_resource_search_dirs(resource_search_dirs);

    match kind {
        RenderJobKind::Rgba => renderer
            .render_svg(svg, options)
            .map(RenderJobOutput::Image),
        RenderJobKind::Png => renderer
            .render_svg_to_png(svg, options)
            .map(RenderJobOutput::Bytes),
        RenderJobKind::Jpeg(jpeg_options) => renderer
            .render_svg_to_jpeg(svg, options, jpeg_options)
            .map(RenderJobOutput::Bytes),
        RenderJobKind::Webp(webp_options) => renderer
            .render_svg_to_webp(svg, options, webp_options)
            .map(RenderJobOutput::Bytes),
    }
}

enum WorkerMessage {
    Render(RenderJob),
    Stop,
}

struct RenderJob {
    kind: RenderJobKind,
    svg: Vec<u8>,
    options: RenderOptions,
    resource_search_dirs: Vec<PathBuf>,
    response: RenderResponse,
}

#[derive(Clone, Copy)]
enum RenderJobKind {
    Rgba,
    Png,
    Jpeg(JpegOptions),
    Webp(WebpOptions),
}

enum RenderJobOutput {
    Image(ImageData),
    Bytes(Vec<u8>),
}

impl RenderJobOutput {
    fn into_image(self) -> Result<ImageData, SvgRenderError> {
        match self {
            Self::Image(image) => Ok(image),
            Self::Bytes(_) => Err(SvgRenderError::PipelineClosed),
        }
    }

    fn into_bytes(self) -> Result<Vec<u8>, SvgRenderError> {
        match self {
            Self::Bytes(bytes) => Ok(bytes),
            Self::Image(_) => Err(SvgRenderError::PipelineClosed),
        }
    }
}

struct RenderResponse {
    shared: Arc<Mutex<RenderResponseState>>,
}

struct RenderResponseFuture {
    shared: Arc<Mutex<RenderResponseState>>,
}

struct RenderResponseState {
    result: Option<Result<RenderJobOutput, SvgRenderError>>,
    waker: Option<Waker>,
}

fn render_response_channel() -> (RenderResponse, RenderResponseFuture) {
    let shared = Arc::new(Mutex::new(RenderResponseState {
        result: None,
        waker: None,
    }));

    (
        RenderResponse {
            shared: Arc::clone(&shared),
        },
        RenderResponseFuture { shared },
    )
}

impl RenderResponse {
    fn complete(self, result: Result<RenderJobOutput, SvgRenderError>) {
        let waker = {
            let mut state = self.shared.lock().expect("render response mutex poisoned");
            state.result = Some(result);
            state.waker.take()
        };

        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

impl RenderResponseFuture {
    fn complete(&self, result: Result<RenderJobOutput, SvgRenderError>) {
        let waker = {
            let mut state = self.shared.lock().expect("render response mutex poisoned");
            state.result = Some(result);
            state.waker.take()
        };

        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

impl Future for RenderResponseFuture {
    type Output = Result<RenderJobOutput, SvgRenderError>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.shared.lock().expect("render response mutex poisoned");

        if let Some(result) = state.result.take() {
            Poll::Ready(result)
        } else {
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_pipeline_rejects_zero_workers() {
        assert!(matches!(
            CpuSvgPipelineRenderer::new(0),
            Err(SvgRenderError::InvalidWorkerCount { workers: 0 })
        ));
    }

    #[test]
    fn generic_pipeline_rejects_zero_workers() {
        assert!(matches!(
            SvgPipelineRenderer::new(0),
            Err(SvgRenderError::InvalidWorkerCount { workers: 0 })
        ));
    }

    #[cfg(feature = "vulkan-backend")]
    #[test]
    fn vulkan_pipeline_rejects_zero_workers() {
        assert!(matches!(
            VulkanSvgPipelineRenderer::new(0),
            Err(SvgRenderError::InvalidWorkerCount { workers: 0 })
        ));
    }
}
