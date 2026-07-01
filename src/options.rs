use skia_safe::{Color, jpeg_encoder, webp_encoder};

use crate::SvgRenderError;

/// Maximum size of a rendered RGBA buffer.
pub(crate) const MAX_RENDER_BYTES: usize = 512 * 1024 * 1024;

/// Non-zero pixel dimensions for the output raster image.
///
/// Width and height must be `> 0` and `<= i32::MAX` to satisfy Skia's
/// internal surface constraints. The total RGBA buffer must also fit
/// within the crate's maximum render allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderSize {
    pub width: u32,
    pub height: u32,
}

impl RenderSize {
    /// Creates a new size after validating the dimensions.
    ///
    /// # Errors
    /// Returns [`SvgRenderError::InvalidSize`] if either dimension is zero
    /// or exceeds `i32::MAX`, or if the resulting RGBA buffer would be too large.
    pub fn new(width: u32, height: u32) -> Result<Self, SvgRenderError> {
        if width == 0 || height == 0 {
            return Err(SvgRenderError::InvalidSize { width, height });
        }

        if width > i32::MAX as u32 || height > i32::MAX as u32 {
            return Err(SvgRenderError::InvalidSize { width, height });
        }

        let byte_len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(SvgRenderError::InvalidSize { width, height })?;
        if byte_len > MAX_RENDER_BYTES {
            return Err(SvgRenderError::InvalidSize { width, height });
        }

        Ok(Self { width, height })
    }

    /// Converts to `(i32, i32)` for Skia API calls.
    pub(crate) fn as_i32_pair(self) -> (i32, i32) {
        (self.width as i32, self.height as i32)
    }
}

/// Parameters controlling a single SVG render call.
///
/// # Defaults
/// | Field          | Default         |
/// |----------------|-----------------|
/// | `clear_color`  | `TRANSPARENT`   |
/// | `sample_count` | 4 (MSAA)        |
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Output image dimensions.
    pub size: RenderSize,
    /// Background color before rendering the SVG onto the canvas.
    pub clear_color: Color,
    /// MSAA sample count (GPU only; ignored by CPU backend).
    pub sample_count: usize,
}

impl RenderOptions {
    /// Creates options for the given output size with defaults for
    /// clear color (transparent) and MSAA (4×).
    pub fn new(width: u32, height: u32) -> Result<Self, SvgRenderError> {
        Ok(Self {
            size: RenderSize::new(width, height)?,
            clear_color: Color::TRANSPARENT,
            sample_count: 4,
        })
    }
}

/// JPEG encoding parameters.
///
/// Default: quality 90, chroma subsampling in both directions, alpha ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JpegOptions {
    /// Encoding quality (0–100, clamped by the Skia converter).
    pub quality: u32,
    /// Chroma subsampling mode.
    pub downsample: JpegDownsample,
    /// How to handle the alpha channel (JPEG does not support alpha).
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

/// JPEG chroma subsampling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JpegDownsample {
    /// Subsample horizontally and vertically (4:2:0).
    BothDirections,
    /// Subsample horizontally only (4:2:2).
    Horizontal,
    /// No subsampling (4:4:4).
    No,
}

/// How to encode alpha-containing images as JPEG (which has no alpha).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JpegAlphaOption {
    /// Discard the alpha channel.
    Ignore,
    /// Blend against black before encoding.
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

/// WebP encoding parameters.
///
/// Default: lossy at quality 90.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WebpOptions {
    /// Compression mode: lossy or lossless.
    pub compression: WebpCompression,
    /// Encoding quality (0.0–100.0, clamped). Meaningless for lossless.
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

/// WebP compression mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebpCompression {
    /// Lossy compression (VP8).
    Lossy,
    /// Lossless compression (VP8L).
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
    fn rejects_render_targets_that_would_allocate_too_many_bytes() {
        assert!(matches!(
            RenderSize::new(i32::MAX as u32, i32::MAX as u32),
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
