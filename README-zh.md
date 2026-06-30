<p align="center">
  <h1 align="center">svg-renderer</h1>
  <p align="center">
    基于 <a href="https://skia.org">Skia</a> 的高性能 SVG 栅格化渲染库
    <br />
    CPU 渲染 · 可选 Vulkan GPU 加速 · Pipeline 并行渲染
  </p>
  <p align="center">
    <a href="README.md"><strong>English Docs</strong></a>
  </p>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.96+-orange" alt="Rust 1.96+" />
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT License" />
  <img src="https://img.shields.io/badge/edition-2024-purple" alt="Rust Edition 2024" />
</p>

---

**svg-renderer** 将 SVG 文档栅格化为光栅图像（原始 RGBA、PNG、JPEG、WebP），支持自动后端选择。优先使用 Vulkan GPU 加速，不可用时无缝回退到 CPU 渲染。

本库由 **Vibe Coding** 方式开发，使用 **GPT-5.5** 辅助生成。

---

## 快速选择

| 需求 | 推荐方案 |
|---|---|
| 快速将 SVG 转为 PNG / JPEG / WebP | 自由函数或 `SvgRenderer` |
| 批量渲染数千个 SVG | `SvgPipelineRenderer` + N 个 worker |
| 纯 CPU，不含 GPU 依赖 | `CpuSvgRenderer` / `CpuSvgPipelineRenderer` |
| 强制使用 GPU | `VulkanSvgRenderer` / `VulkanSvgPipelineRenderer`（需 `vulkan-backend`） |
| SVG 引用外部字体 / 图片 | 任意渲染器调用 `set_resource_search_dirs(…)` |
| 精细控制输出格式 | `JpegOptions`、`WebpOptions` 等结构化参数 |

---

## 安装

```toml
[dependencies]
svg-renderer = "1.0.1"
```

默认启用 Vulkan 后端。若只需 CPU：

```toml
[dependencies]
svg-renderer = { version = "1.0.1", default-features = false }
```

### Feature 开关

| Feature | 默认 | 说明 |
|---|---|---|
| `vulkan-backend` | ✅ | 通过 `ash` + Skia 启用 Vulkan 渲染 |
| *(无)* | — | 仅 CPU 渲染（始终可用） |

### 环境要求

- Rust **1.96** 或更高版本
- `skia-safe` 所需的 Skia 构建 / 运行时环境
- 仅在使用 Vulkan 后端时需要 GPU 驱动

---

## 快速上手

### 一行函数

```rust
use svg_renderer::{render_svg_to_png, RenderOptions};

let png = render_svg_to_png(
    br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 200">
         <circle cx="100" cy="100" r="80" fill="#0f766e"/></svg>"#,
    &RenderOptions::new(200, 200)?,
)?;
std::fs::write("circle.png", png)?;
```

### 使用 `SvgRenderer`（自动选择后端）

```rust
use svg_renderer::{RenderOptions, SvgRenderer};

let mut renderer = SvgRenderer::new()?;
let image = renderer.render_svg(
    br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 120">
         <rect width="300" height="120" fill="#0f766e"/>
         <text x="24" y="72" font-size="32" fill="white">svg-renderer</text></svg>"#,
    &RenderOptions::new(300, 120)?,
)?;
println!("{}x{} RGBA", image.width, image.height);
```

查看当前后端：

```rust
let mut renderer = SvgRenderer::new()?;
println!("{:?}", renderer.backend()); // Cpu 或 Vulkan
```

---

## 输出格式

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

| JpegDownsample | 说明 |
|---|---|
| `BothDirections` | 4:2:0 色度采样（默认） |
| `Horizontal` | 4:2:2 |
| `No` | 4:4:4（画质最佳，文件最大） |

| JpegAlphaOption | 说明 |
|---|---|
| `Ignore` | 丢弃 Alpha |
| `BlendOnBlack` | Alpha 与黑色混合后再编码 |

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

| WebpCompression | 说明 |
|---|---|
| `Lossy` | VP8 有损（默认） |
| `Lossless` | VP8L 无损 |

---

## 后端选择

| 类型 | Feature | 行为 |
|---|---|---|
| `SvgRenderer` | (任意) | 先尝试 Vulkan，失败则回退 CPU |
| `CpuSvgRenderer` | (始终) | 纯 CPU |
| `VulkanSvgRenderer` | `vulkan-backend` | 仅 GPU，失败直接报错 |

CPU 渲染器——无任何 GPU 依赖，API 完全一致：

```rust
use svg_renderer::CpuSvgRenderer;
let mut renderer = CpuSvgRenderer::new()?;
let png = renderer.render_svg_to_png(svg_bytes, &options)?;
```

Vulkan 渲染器——显式使用 GPU：

```rust
use svg_renderer::VulkanSvgRenderer; // 需要 "vulkan-backend"
let mut renderer = VulkanSvgRenderer::new()?;
let png = renderer.render_svg_to_png(svg_bytes, &options)?;
```

---

## Pipeline 并行渲染

`SvgPipelineRenderer` 创建固定数量 worker 线程，轮询分发作业。每个 worker 拥有独立的渲染器——适合批量渲染海量 SVG。

```rust
use svg_renderer::{RenderOptions, SvgPipelineRenderer};

let renderer = SvgPipelineRenderer::new(4)?;  // 4 个 worker
let options = RenderOptions::new(512, 320)?;
let svg = b"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 512 320'/>";

// 所有 pipeline 方法都是 async
let image = renderer.render_svg(svg, &options).await?;
let png   = renderer.render_svg_to_png(svg, &options).await?;
let jpeg  = renderer.render_svg_to_jpeg(svg, &options, jpeg_opts).await?;
let webp  = renderer.render_svg_to_webp(svg, &options, webp_opts).await?;
```

Pipeline 后端变体：

| 类型 | 后端 |
|---|---|
| `SvgPipelineRenderer` | 自动选择（Vulkan → CPU） |
| `CpuSvgPipelineRenderer` | 仅 CPU |
| `VulkanSvgPipelineRenderer` | 仅 Vulkan（需 `vulkan-backend`） |

---

## 外部资源

SVG 可能引用外部字体、图片等。可在任意渲染器上配置搜索目录：

```rust
let mut renderer = SvgRenderer::new()?;
renderer.set_resource_search_dirs(["assets"]);
```

查找顺序：
1. 本地搜索目录（按顺序）
2. HTTP(S) 回退（通过 `ureq`）

SVG 中绝对路径直接解析；URL 类路径跳过本地查找。

---

## 图像数据结构

```rust
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub row_bytes: usize,   // 固定为 width * 4
    pub rgba: Vec<u8>,      // premultiplied alpha, RGBA8888, 行优先
}
```

---

## 错误类型

所有公开 API 返回 `Result<_, SvgRenderError>`。

```rust
pub enum SvgRenderError {
    InvalidSize { width: u32, height: u32 },
    InvalidWorkerCount { workers: usize },
    VulkanLoader(ash::LoadingError),       // 仅 vulkan-backend
    Vulkan(vk::Result),                    // 仅 vulkan-backend
    NoVulkanDevice,                        // 仅 vulkan-backend
    SkiaContext,                           // 仅 vulkan-backend
    SvgParse,       // SVG 解析失败
    RenderTarget,   // 渲染表面创建失败
    ReadPixels,     // 像素回读失败
    PngEncode,      // PNG 编码失败
    JpegEncode,     // JPEG 编码失败
    WebpEncode,     // WebP 编码失败
    PipelineClosed, // Worker 线程异常退出
}
```

---

## 示例

所有示例源码在 [`examples/`](https://github.com/limao996/svg-renderer/tree/master/examples) 目录。

### `render_svg`

基础的单次渲染，输出多种格式。

```bash
cargo run --example render_svg
```

输出到 `target/example-output/`：
- `sample.rgba` — 原始 RGBA 数据
- `sample.png`
- `sample.jpg`
- `sample.webp`

### `render_pipeline`

异步 Pipeline 渲染，4 个 worker 使用 `SvgPipelineRenderer`。

```bash
cargo run --example render_pipeline
```

输出 `target/example-output/pipeline-sample.png`。

### `render_perf`

对全部可用后端（CPU、Vulkan、Pipeline）进行压测，报告吞吐性能。接受可选参数：

```bash
# 100 次迭代, 4 个 pipeline worker（默认）
cargo run --example render_perf --release

# 自定义参数
cargo run --example render_perf --release -- 500 8
```

报告内容：平均 / 中位 / 最短 / 最长耗时，FPS 吞吐量。

---

## 许可证

本项目使用 [MIT License](LICENSE)。
