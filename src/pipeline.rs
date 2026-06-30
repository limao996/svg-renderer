use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    task::{Context, Poll, Waker},
    thread::{self, JoinHandle},
};

use crate::{
    ImageData, JpegOptions, RenderOptions, SvgRenderError, VulkanSvgRenderer, WebpOptions,
};

pub struct VulkanSvgPipelineRenderer {
    sender: mpsc::Sender<WorkerMessage>,
    workers: Vec<JoinHandle<()>>,
    resource_search_dirs: Vec<PathBuf>,
}

impl VulkanSvgPipelineRenderer {
    pub fn new(workers: usize) -> Result<Self, SvgRenderError> {
        if workers == 0 {
            return Err(SvgRenderError::InvalidWorkerCount { workers });
        }

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut handles = Vec::with_capacity(workers);

        for _ in 0..workers {
            let (ready_sender, ready_receiver) = mpsc::channel();
            handles.push(spawn_worker(Arc::clone(&receiver), ready_sender));
            ready_receiver
                .recv()
                .map_err(|_| SvgRenderError::PipelineClosed)??;
        }

        Ok(Self {
            sender,
            workers: handles,
            resource_search_dirs: Vec::new(),
        })
    }

    pub fn set_resource_search_dirs<I, P>(&mut self, dirs: I) -> &mut Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.resource_search_dirs = dirs.into_iter().map(Into::into).collect();
        self
    }

    pub fn add_resource_search_dir(&mut self, dir: impl Into<PathBuf>) -> &mut Self {
        self.resource_search_dirs.push(dir.into());
        self
    }

    pub async fn render_svg(
        &self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<ImageData, SvgRenderError> {
        self.submit(RenderJobKind::Rgba, svg.as_ref(), options)
            .await?
            .into_image()
    }

    pub async fn render_svg_to_png(
        &self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        self.submit(RenderJobKind::Png, svg.as_ref(), options)
            .await?
            .into_bytes()
    }

    pub async fn render_svg_to_jpeg(
        &self,
        svg: impl AsRef<[u8]>,
        options: &RenderOptions,
        jpeg_options: JpegOptions,
    ) -> Result<Vec<u8>, SvgRenderError> {
        self.submit(RenderJobKind::Jpeg(jpeg_options), svg.as_ref(), options)
            .await?
            .into_bytes()
    }

    pub async fn render_svg_to_webp(
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

        if self.sender.send(WorkerMessage::Render(job)).is_err() {
            future.complete(Err(SvgRenderError::PipelineClosed));
        }

        future
    }
}

impl Drop for VulkanSvgPipelineRenderer {
    fn drop(&mut self) {
        for _ in &self.workers {
            let _ = self.sender.send(WorkerMessage::Stop);
        }

        while let Some(worker) = self.workers.pop() {
            let _ = worker.join();
        }
    }
}

fn spawn_worker(
    receiver: Arc<Mutex<mpsc::Receiver<WorkerMessage>>>,
    ready: mpsc::Sender<Result<(), SvgRenderError>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut renderer = match VulkanSvgRenderer::new() {
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
            let message = match receiver.lock() {
                Ok(receiver) => receiver.recv(),
                Err(_) => return,
            };

            match message {
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

fn run_job(
    renderer: &mut VulkanSvgRenderer,
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
    fn rejects_zero_workers() {
        assert!(matches!(
            VulkanSvgPipelineRenderer::new(0),
            Err(SvgRenderError::InvalidWorkerCount { workers: 0 })
        ));
    }
}
