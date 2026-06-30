use std::{fs, path::PathBuf};

use svg_renderer::{
    JpegAlphaOption, JpegDownsample, JpegOptions, RenderOptions, VulkanSvgRenderer,
    WebpCompression, WebpOptions,
};

const SVG: &str = r##"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 320">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#0f766e"/>
      <stop offset="1" stop-color="#1d4ed8"/>
    </linearGradient>
    <filter id="shadow" x="-20%" y="-20%" width="140%" height="140%">
      <feDropShadow dx="0" dy="10" stdDeviation="8" flood-color="#0f172a" flood-opacity="0.35"/>
    </filter>
    <filter id="blur_filter">
      <feGaussianBlur in="SourceGraphic" stdDeviation="5" />
    </filter>
  </defs>
  <rect width="512" height="320" rx="24" fill="url(#bg)"/>
  <circle cx="412" cy="82" r="58" fill="#facc15" opacity="0.9" filter="url(#blur_filter)"/>
  <g filter="url(#shadow)">
    <rect x="64" y="72" width="260" height="176" rx="18" fill="#ffffff" opacity="0.94"/>
    <path d="M96 204 L148 138 L194 188 L230 146 L292 224 H96 Z" fill="#14b8a6"/>
    <circle cx="132" cy="116" r="20" fill="#f97316"/>
  </g>
  <text x="64" y="286" font-family="Arial, sans-serif" font-size="30" font-weight="700" fill="#ffffff">
    Vulkan SVG Renderer
  </text>
</svg>
"##;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = PathBuf::from("target/example-output");
    fs::create_dir_all(&output_dir)?;

    let options = RenderOptions::new(512, 320)?;
    let mut renderer = VulkanSvgRenderer::new()?;
    renderer.set_resource_search_dirs([PathBuf::from("examples/assets"), output_dir.clone()]);

    let image = renderer.render_svg(SVG, &options)?;
    fs::write(output_dir.join("sample.rgba"), &image.rgba)?;

    let png = renderer.render_svg_to_png(SVG, &options)?;
    fs::write(output_dir.join("sample.png"), png)?;

    let jpeg = renderer.render_svg_to_jpeg(
        SVG,
        &options,
        JpegOptions {
            quality: 90,
            downsample: JpegDownsample::BothDirections,
            alpha_option: JpegAlphaOption::BlendOnBlack,
        },
    )?;
    fs::write(output_dir.join("sample.jpg"), jpeg)?;

    let webp = renderer.render_svg_to_webp(
        SVG,
        &options,
        WebpOptions {
            compression: WebpCompression::Lossy,
            quality: 90.0,
        },
    )?;
    fs::write(output_dir.join("sample.webp"), webp)?;

    println!("wrote target/example-output/sample.rgba");
    println!("wrote target/example-output/sample.png");
    println!("wrote target/example-output/sample.jpg");
    println!("wrote target/example-output/sample.webp");
    println!(
        "rgba: {}x{}, row_bytes={}",
        image.width, image.height, image.row_bytes
    );

    Ok(())
}
