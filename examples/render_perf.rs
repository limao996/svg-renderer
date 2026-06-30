use std::{env, hint::black_box, time::Instant};

use svg_renderer::{RenderOptions, VulkanSvgRenderer};

// cargo run --example render_perf --release -- 100

const DEFAULT_ITERATIONS: usize = 100;
const WARMUP_ITERATIONS: usize = 10;
const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let iterations = env::args()
        .nth(1)
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(DEFAULT_ITERATIONS);

    if iterations == 0 {
        return Err("渲染次数必须大于 0".into());
    }

    println!("SVG 渲染性能");
    let svg = build_svg();
    let options = RenderOptions::new(WIDTH, HEIGHT)?;
    println!("渲染尺寸：{WIDTH}x{HEIGHT}");

    let init_start = Instant::now();
    let mut renderer = VulkanSvgRenderer::new()?;
    let init_elapsed = init_start.elapsed();
    println!(
        "渲染器初始化耗时：{:.2} ms",
        init_elapsed.as_secs_f64() * 1_000.0
    );

    for _ in 0..WARMUP_ITERATIONS {
        black_box(renderer.render_svg(&svg, &options)?);
    }
    println!("统计次数：{iterations}，预热次数：{WARMUP_ITERATIONS}");

    let mut timings = Vec::with_capacity(iterations);
    let total_start = Instant::now();

    for _ in 0..iterations {
        let frame_start = Instant::now();
        let image = renderer.render_svg(&svg, &options)?;
        black_box(image.rgba.len());
        timings.push(frame_start.elapsed());
    }

    let total_elapsed = total_start.elapsed();
    timings.sort_unstable();

    let average_ms = total_elapsed.as_secs_f64() * 1_000.0 / iterations as f64;
    let median_ms = timings[iterations / 2].as_secs_f64() * 1_000.0;
    let min_ms = timings[0].as_secs_f64() * 1_000.0;
    let max_ms = timings[iterations - 1].as_secs_f64() * 1_000.0;
    let fps = iterations as f64 / total_elapsed.as_secs_f64();

    println!(
        "总渲染耗时：{:.2} ms",
        total_elapsed.as_secs_f64() * 1_000.0
    );
    println!("平均耗时：{average_ms:.2} ms/帧");
    println!("中位耗时：{median_ms:.2} ms/帧");
    println!("最短耗时：{min_ms:.2} ms/帧");
    println!("最长耗时：{max_ms:.2} ms/帧");
    println!("吞吐量：{fps:.2} 帧/秒");

    Ok(())
}

fn build_svg() -> String {
    let mut svg = String::from(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 768">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#102a43"/>
      <stop offset="0.55" stop-color="#0f766e"/>
      <stop offset="1" stop-color="#f97316"/>
    </linearGradient>
    <filter id="shadow" x="-20%" y="-20%" width="140%" height="140%">
      <feDropShadow dx="0" dy="8" stdDeviation="6" flood-color="#020617" flood-opacity="0.32"/>
    </filter>
  </defs>
  <rect width="1024" height="768" fill="url(#bg)"/>
"##,
    );

    for row in 0..18 {
        for col in 0..24 {
            let x = 32 + col * 42;
            let y = 36 + row * 38;
            let radius = 8 + ((row + col) % 13);
            let hue = (row * 29 + col * 17) % 360;
            let opacity = 0.35 + ((row + col) % 7) as f32 * 0.07;

            svg.push_str(&format!(
                "  <circle cx=\"{x}\" cy=\"{y}\" r=\"{radius}\" fill=\"hsl({hue}, 86%, 62%)\" opacity=\"{opacity:.2}\"/>\n"
            ));
        }
    }

    for index in 0..72 {
        let x = 48 + (index % 12) * 78;
        let y = 88 + (index / 12) * 96;
        let width = 44 + (index % 5) * 11;
        let height = 28 + (index % 7) * 8;
        let rotation = (index * 13) % 360;
        let color = if index % 3 == 0 { "#f8fafc" } else { "#bae6fd" };

        svg.push_str(&format!(
            "  <rect x=\"{x}\" y=\"{y}\" width=\"{width}\" height=\"{height}\" rx=\"8\" fill=\"{color}\" opacity=\"0.58\" filter=\"url(#shadow)\" transform=\"rotate({rotation} {x} {y})\"/>\n"
        ));
    }

    svg.push_str(
        r##"  <path d="M96 620 C220 540 300 700 430 610 S670 520 830 640 S930 720 992 610" fill="none" stroke="#ffffff" stroke-width="18" stroke-linecap="round" opacity="0.86"/>
  <text x="64" y="714" font-family="Arial, sans-serif" font-size="44" font-weight="700" fill="#ffffff">svg-renderer perf sample</text>
</svg>
"##,
    );

    svg
}
