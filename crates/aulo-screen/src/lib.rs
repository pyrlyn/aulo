//! Screenshots for the model (spec §6.1, T9.15).
//!
//! A capture is RGBA pixels, the bytes `xcap`'s `Monitor::capture_image` and
//! `Window::capture_image` return. [`Frame::downscale`] shrinks the long edge
//! to [`MODEL_MAX_EDGE`] and [`norm`] / [`pixel`] speak in 0–999, so a point
//! in the middle of the screen is the same number on a 1× display and a 2×
//! Retina capture. The live capturer is not linked here: xcap 0.9.8's Linux
//! backend pulls PipeWire and an unpinned git crate, and these tests only
//! need the geometry.

mod geom;

pub use geom::{NORM_MAX, Norm, Pixel, Size, bucket, norm, pixel};

/// Longest edge of the image the model sees, in pixels.
pub const MODEL_MAX_EDGE: u32 = 1280;

/// A screen or window capture, original size and the downscaled RGBA the
/// model is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Pixel size of the capture, including Retina scale.
    pub screen: Size,
    /// Pixel size of [`Self::rgba`].
    pub model: Size,
    /// Row-major RGBA, `model.width * model.height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// Why a capture could not be turned into a frame.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScreenError {
    /// Width or height was zero.
    #[error("screenshot size is empty")]
    Empty,
    /// The buffer is not `width * height * 4` bytes.
    #[error("screenshot buffer has {got} bytes, expected {expected}")]
    Buffer { got: usize, expected: usize },
    /// The capture is larger than [`MAX_EDGE`] or [`MAX_PIXELS`].
    #[error("screenshot is too large")]
    TooLarge,
}

/// Largest edge accepted from a capturer. Bigger than this is refused.
pub const MAX_EDGE: u32 = 8192;

/// Largest pixel count accepted from a capturer.
pub const MAX_PIXELS: u64 = 8_000_000;

impl Frame {
    /// Nearest-neighbour downscale of `rgba` (xcap's `RgbaImage` bytes).
    ///
    /// # Errors
    ///
    /// Empty, oversized, or a buffer whose length is not the pixel count.
    pub fn downscale(screen: Size, rgba: &[u8]) -> Result<Self, ScreenError> {
        checked_size(screen)?;
        let expected = byte_len(screen)?;
        if rgba.len() != expected {
            return Err(ScreenError::Buffer {
                got: rgba.len(),
                expected,
            });
        }
        let model = model_size(screen, MODEL_MAX_EDGE);
        let rgba = if model == screen {
            rgba.to_vec()
        } else {
            resample(rgba, screen, model)
        };
        Ok(Self {
            screen,
            model,
            rgba,
        })
    }

    /// Where `at` sits in 0–999 on this frame's screen.
    #[must_use]
    pub fn norm_of(&self, at: Pixel) -> Norm {
        norm(at, self.screen)
    }

    /// The screen pixel for a 0–999 point. Uses the original capture size,
    /// not the downscaled image, so a click lands on the display.
    #[must_use]
    pub fn pixel_of(&self, at: Norm) -> Pixel {
        pixel(at, self.screen)
    }
}

fn checked_size(size: Size) -> Result<(), ScreenError> {
    if size.width == 0 || size.height == 0 {
        return Err(ScreenError::Empty);
    }
    if size.width > MAX_EDGE || size.height > MAX_EDGE {
        return Err(ScreenError::TooLarge);
    }
    let pixels = u64::from(size.width) * u64::from(size.height);
    if pixels > MAX_PIXELS {
        return Err(ScreenError::TooLarge);
    }
    Ok(())
}

fn byte_len(size: Size) -> Result<usize, ScreenError> {
    let pixels = u64::from(size.width) * u64::from(size.height);
    let bytes = pixels.saturating_mul(4);
    usize::try_from(bytes).map_err(|_| ScreenError::TooLarge)
}

/// Size of the model image: the same as `screen` when it already fits,
/// otherwise the long edge is `max_edge` and the aspect is kept.
#[must_use]
pub fn model_size(screen: Size, max_edge: u32) -> Size {
    let width = screen.width.max(1);
    let height = screen.height.max(1);
    let max_edge = max_edge.max(1);
    let long = width.max(height);
    if long <= max_edge {
        return Size { width, height };
    }
    Size {
        width: scale_edge(width, long, max_edge),
        height: scale_edge(height, long, max_edge),
    }
}

fn scale_edge(edge: u32, long: u32, max_edge: u32) -> u32 {
    let scaled = u64::from(edge) * u64::from(max_edge) / u64::from(long);
    u32::try_from(scaled).unwrap_or(1).max(1)
}

fn resample(src: &[u8], from: Size, to: Size) -> Vec<u8> {
    let mut out =
        vec![0u8; usize::try_from(u64::from(to.width) * u64::from(to.height) * 4).unwrap_or(0)];
    if out.is_empty() {
        return out;
    }
    for y in 0..to.height {
        let sy = sample_index(y, to.height, from.height);
        for x in 0..to.width {
            let sx = sample_index(x, to.width, from.width);
            let from_i = index4(sx, sy, from.width);
            let to_i = index4(x, y, to.width);
            if let (Some(pixel), Some(dest)) =
                (src.get(from_i..from_i + 4), out.get_mut(to_i..to_i + 4))
            {
                dest.copy_from_slice(pixel);
            }
        }
    }
    out
}

fn sample_index(at: u32, to_extent: u32, from_extent: u32) -> u32 {
    if to_extent == 0 || from_extent == 0 {
        return 0;
    }
    let i = u64::from(at) * u64::from(from_extent) / u64::from(to_extent);
    u32::try_from(i).unwrap_or(0).min(from_extent - 1)
}

fn index4(x: u32, y: u32, width: u32) -> usize {
    (u64::from(y) * u64::from(width) + u64::from(x))
        .saturating_mul(4)
        .try_into()
        .unwrap_or(0)
}
