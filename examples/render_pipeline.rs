use std::{fs, path::PathBuf};

use svg_renderer::{RenderOptions, VulkanSvgPipelineRenderer};

// cargo run --example render_pipeline --release

const SVG: &str = r##"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 320">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#0f766e"/>
      <stop offset="1" stop-color="#1d4ed8"/>
    </linearGradient>
  </defs>
  <rect width="512" height="320" rx="24" fill="url(#bg)"/>
  <circle cx="402" cy="92" r="58" fill="#facc15" opacity="0.9"/>
  <rect x="64" y="76" width="270" height="168" rx="18" fill="#ffffff" opacity="0.94"/>
  <path d="M96 204 L148 138 L194 188 L230 146 L292 224 H96 Z" fill="#14b8a6"/>
  <circle cx="132" cy="116" r="20" fill="#f97316"/>
  <text x="64" y="286" font-family="Arial, sans-serif" font-size="30" font-weight="700" fill="#ffffff">
    Vulkan SVG Pipeline
  </text>
</svg>
"##;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    pollster::block_on(render())
}

async fn render() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = PathBuf::from("target/example-output");
    fs::create_dir_all(&output_dir)?;

    let options = RenderOptions::new(512, 320)?;
    let renderer = VulkanSvgPipelineRenderer::new(4)?;
    let png = renderer.render_svg_to_png(SVG, &options).await?;
    fs::write(output_dir.join("pipeline-sample.png"), png)?;

    println!("wrote target/example-output/pipeline-sample.png");
    Ok(())
}
