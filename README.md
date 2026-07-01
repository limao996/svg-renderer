<p align="center">
  <h1 align="center">svg-renderer</h1>
  <p align="center">
    A high-performance SVG-to-raster renderer powered by <a href="https://skia.org">Skia</a>
    <br />
    CPU backend · Optional Vulkan GPU backend · Pipeline parallel renderer
  </p>
  <p align="center">
    <a href="README-zh.md"><strong>中文文档</strong></a>
  </p>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-edition%202024-orange" alt="Rust Edition 2024" />
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT License" />
  <img src="https://img.shields.io/badge/edition-2024-purple" alt="Rust Edition 2024" />
</p>

---

**svg-renderer** converts SVG documents to raster images (raw RGBA, PNG, JPEG, WebP) with automatic backend selection. It prefers Vulkan GPU acceleration when available and gracefully falls back to CPU rendering.

This crate was developed with **Vibe Coding** using **GPT-5.5**.

---

## Why svg-renderer?

| Need | Solution |
|---|---|
| Quick SVG → PNG/JPEG/WebP in a CLI tool | Free-standing functions or `SvgRenderer` |
| Batch-rendering thousands of SVGs | `SvgPipelineRenderer` with N worker threads |
| Always CPU, no GPU dependency | `CpuSvgRenderer` / `CpuSvgPipelineRenderer` |
| Explicit GPU-only path | `VulkanSvgRenderer` / `VulkanSvgPipelineRenderer` (with `vulkan-backend`) |
| SVG references external fonts / images | `set_resource_search_dirs(…)` on any renderer |
| Fine-grained output format control | `JpegOptions`, `WebpOptions` structs |

---

## Installation

```toml
[dependencies]
svg-renderer = "1.0.1"
```

Default features enable the Vulkan backend. For CPU-only:

```toml
[dependencies]
svg-renderer = { version = "1.0.1", default-features = false }
```

### Feature flags

| Feature | Default | Description |
|---|---|---|
| `vulkan-backend` | ✅ | Vulkan GPU rendering via `ash` + Skia Vulkan integration |
| *(none)* | — | CPU rendering only (always available) |

### Requirements

- A Rust toolchain compatible with Edition 2024
- Skia build / runtime dependencies (via `skia-safe`)
- Vulkan drivers + runtime — only if using the Vulkan backend

### skia-bindings binary download proxy

`skia-safe` builds through `skia-bindings`, which tries to download a prebuilt Skia binary during compilation. If that download fails because GitHub or the binary host is unreachable, configure a curl proxy and run the build again.

For a one-off build:

```powershell
$env:HTTPS_PROXY = "http://127.0.0.1:7890"
$env:HTTP_PROXY = "http://127.0.0.1:7890"
cargo build
```

For a global curl proxy, create or edit the curl configuration file:

- Windows: `%USERPROFILE%\.curlrc`
- Linux / macOS: `~/.curlrc`

```text
proxy = "http://127.0.0.1:7890"
```

Use your own proxy address and port. Remove the setting after the build if you do not want all curl requests to use that proxy.

---

## Quick start

### One-liner free functions

```rust
use svg_renderer::{render_svg_to_png, RenderOptions};

let png = render_svg_to_png(
    br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 200">
         <circle cx="100" cy="100" r="80" fill="#0f766e"/></svg>"#,
    &RenderOptions::new(200, 200)?,
)?;
std::fs::write("circle.png", png)?;
```

### Using `SvgRenderer` (auto-detect backend)

```rust
use svg_renderer::{RenderOptions, SvgRenderer};

let mut renderer = SvgRenderer::new()?;
let image = renderer.render_svg(
    br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 120">
         <rect width="300" height="120" fill="#0f766e"/>
         <text x="24" y="72" font-size="32" fill="white">svg-renderer</text></svg>"#,
    &RenderOptions::new(300, 120)?,
)?;
println!("{}x{} RGBA image", image.width, image.height);
```

Check which backend was selected:

```rust
let mut renderer = SvgRenderer::new()?;
println!("{:?}", renderer.backend()); // Cpu or Vulkan
```

---

## Output formats

### PNG

```rust
use svg_renderer::{RenderOptions, SvgRenderer};
let mut renderer = SvgRenderer::new()?;
let png = renderer.render_svg_to_png(svg_bytes, &options)?;
```

### JPEG

```rust
use svg_renderer::{
    JpegAlphaOption, JpegDownsample, JpegOptions, RenderOptions, SvgRenderer,
};

let options = RenderOptions::new(512, 320)?;
let jpeg_opts = JpegOptions {
    quality: 90,
    downsample: JpegDownsample::BothDirections,
    alpha_option: JpegAlphaOption::BlendOnBlack,
};
let mut renderer = SvgRenderer::new()?;
let jpeg = renderer.render_svg_to_jpeg(svg_bytes, &options, jpeg_opts)?;
```

| JpegDownsample | Behavior |
|---|---|
| `BothDirections` | 4:2:0 chroma subsampling (default) |
| `Horizontal` | 4:2:2 |
| `No` | 4:4:4 (best quality, largest file) |

| JpegAlphaOption | Behavior |
|---|---|
| `Ignore` | Discard alpha |
| `BlendOnBlack` | Composite over black before encoding |

### WebP

```rust
use svg_renderer::{RenderOptions, SvgRenderer, WebpCompression, WebpOptions};

let options = RenderOptions::new(512, 320)?;
let webp_opts = WebpOptions {
    compression: WebpCompression::Lossy,
    quality: 90.0,
};
let mut renderer = SvgRenderer::new()?;
let webp = renderer.render_svg_to_webp(svg_bytes, &options, webp_opts)?;
```

| WebpCompression | Description |
|---|---|
| `Lossy` | VP8 (default) |
| `Lossless` | VP8L |

---

## Backend selection

| Type | Feature | Behavior |
|---|---|---|
| `SvgRenderer` | (any) | Try Vulkan → fallback to CPU |
| `CpuSvgRenderer` | (always) | CPU raster, no fallback needed |
| `VulkanSvgRenderer` | `vulkan-backend` | GPU only, errors on Vulkan failure |

CPU renderer — no GPU dependency, identical API:

```rust
use svg_renderer::CpuSvgRenderer;
let mut renderer = CpuSvgRenderer::new()?;
let png = renderer.render_svg_to_png(svg_bytes, &options)?;
```

Vulkan renderer — explicit GPU path:

```rust
use svg_renderer::VulkanSvgRenderer; // requires "vulkan-backend"
let mut renderer = VulkanSvgRenderer::new()?;
let png = renderer.render_svg_to_png(svg_bytes, &options)?;
```

---

## Pipeline rendering (batch workloads)

`SvgPipelineRenderer` spawns a pool of dedicated worker threads and dispatches jobs via round-robin. Each worker owns its own renderer — ideal for rendering many SVGs concurrently.

```rust
use svg_renderer::{RenderOptions, SvgPipelineRenderer};

let renderer = SvgPipelineRenderer::new(4)?;  // 4 workers
let options = RenderOptions::new(512, 320)?;
let svg = b"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 512 320'/>";

// All pipeline methods are async
let image = renderer.render_svg(svg, &options).await?;
let png   = renderer.render_svg_to_png(svg, &options).await?;
let jpeg  = renderer.render_svg_to_jpeg(svg, &options, jpeg_opts).await?;
let webp  = renderer.render_svg_to_webp(svg, &options, webp_opts).await?;
```

Pipeline variants mirror the single-shot renderers:

| Pipeline type | Backend |
|---|---|
| `SvgPipelineRenderer` | Auto-select (Vulkan → CPU) |
| `CpuSvgPipelineRenderer` | CPU only |
| `VulkanSvgPipelineRenderer` | Vulkan only (requires `vulkan-backend`) |

---

## External resources

SVG files may reference external fonts, images, or other assets. Set one or more search directories on any renderer:

```rust
let mut renderer = SvgRenderer::new()?;
renderer.set_resource_search_dirs(["assets"]);
```

The renderer probes:

1. Local search directories (in order)
2. HTTP(S) fallback via `ureq` (built into Skia's resource provider)

Absolute paths in SVG references resolve directly. URL-like references skip local lookup and fall back to HTTP(S).

---

## Image data

```rust
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub row_bytes: usize,   // always width * 4
    pub rgba: Vec<u8>,      // premultiplied alpha, RGBA8888, row-major
}
```

---

## Error handling

All public APIs return `Result<_, SvgRenderError>`.

```rust
pub enum SvgRenderError {
    InvalidSize { width: u32, height: u32 },
    InvalidWorkerCount { workers: usize },
    VulkanLoader(ash::LoadingError),       // cfg(vulkan-backend)
    Vulkan(vk::Result),                    // cfg(vulkan-backend)
    NoVulkanDevice,                        // cfg(vulkan-backend)
    SkiaContext,                           // cfg(vulkan-backend)
    SvgParse,
    RenderTarget,
    ReadPixels,
    PngEncode,
    JpegEncode,
    WebpEncode,
    PipelineClosed,
}
```

---

## Examples

All examples can be found in the [`examples/`](https://github.com/limao996/svg-renderer/tree/master/examples) directory.

### `render_svg`

Basic single-shot rendering to multiple output formats.

```bash
cargo run --example render_svg
```

Output in `target/example-output/`:
- `sample.rgba` — raw RGBA data
- `sample.png`
- `sample.jpg`
- `sample.webp`

### `render_pipeline`

Async pipeline rendering with 4 worker threads using `SvgPipelineRenderer`.

```bash
cargo run --example render_pipeline
```

Writes `target/example-output/pipeline-sample.png`.

### `render_perf`

Benchmark all available backends (CPU, Vulkan, pipeline) and reports throughput. Accepts optional arguments:

```bash
# 100 iterations, 4 pipeline workers (defaults)
cargo run --example render_perf --release

# Custom parameters
cargo run --example render_perf --release -- 500 8
```

Reports: average/median/min/max latency, FPS throughput.

---

## License

Licensed under the [MIT License](LICENSE).
