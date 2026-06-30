use skia_safe::{Color, jpeg_encoder, webp_encoder};

use crate::SvgRenderError;

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

    pub(crate) fn as_i32_pair(self) -> (i32, i32) {
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
}
