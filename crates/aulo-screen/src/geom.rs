//! 0–999 coordinates. One number is one thousandth of the capture, so a
//! Retina capture (twice the pixels) and a 1× capture of the same screen
//! agree.

/// Highest coordinate. The span is 1000 buckets, `0..=999`.
pub const NORM_MAX: u32 = 999;

const NORM_SPAN: u32 = 1000;

/// A pixel in a capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pixel {
    /// From the left edge.
    pub x: u32,
    /// From the top edge.
    pub y: u32,
}

/// A point the model names. Both axes are `0..=999`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Norm {
    /// From the left edge.
    pub x: u32,
    /// From the top edge.
    pub y: u32,
}

/// Width and height in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    /// Pixels across.
    pub width: u32,
    /// Pixels down.
    pub height: u32,
}

/// `at` as 0–999 inside `size`. A pixel past the far edge clamps to that edge.
#[must_use]
pub fn norm(at: Pixel, size: Size) -> Norm {
    Norm {
        x: axis_norm(at.x, size.width),
        y: axis_norm(at.y, size.height),
    }
}

/// The pixel in `size` for a 0–999 point. Values above [`NORM_MAX`] clamp.
#[must_use]
pub fn pixel(at: Norm, size: Size) -> Pixel {
    Pixel {
        x: axis_pixel(at.x, size.width),
        y: axis_pixel(at.y, size.height),
    }
}

/// How far a round trip may move, in pixels: one 0–999 bucket.
#[must_use]
pub fn bucket(extent: u32) -> u32 {
    if extent == 0 {
        return 0;
    }
    extent.div_ceil(NORM_SPAN)
}

fn axis_norm(pixel: u32, extent: u32) -> u32 {
    if extent == 0 {
        return 0;
    }
    let pixel = u64::from(pixel.min(extent - 1));
    let n = pixel * u64::from(NORM_SPAN) / u64::from(extent);
    u32::try_from(n).unwrap_or(NORM_MAX).min(NORM_MAX)
}

fn axis_pixel(norm: u32, extent: u32) -> u32 {
    if extent == 0 {
        return 0;
    }
    let norm = u64::from(norm.min(NORM_MAX));
    let px = norm * u64::from(extent) / u64::from(NORM_SPAN);
    u32::try_from(px).unwrap_or(0).min(extent - 1)
}
