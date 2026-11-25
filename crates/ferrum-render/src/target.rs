//! The caller's pixel buffer and rectangles on it.

use crate::color::Rgb;
use crate::error::RenderError;

/// A rectangle in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rect {
    /// Left edge.
    pub x: u32,
    /// Top edge.
    pub y: u32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

impl Rect {
    /// The rectangle at (`x`, `y`) of `width` × `height`.
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }

    /// One past the right edge.
    pub const fn right(&self) -> u32 {
        self.x.saturating_add(self.width)
    }

    /// One past the bottom edge.
    pub const fn bottom(&self) -> u32 {
        self.y.saturating_add(self.height)
    }

    /// Whether it covers no pixel.
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// The pixels both rectangles cover (empty when they do not overlap).
    #[must_use]
    pub fn intersect(&self, other: &Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        Self { x, y, width: right.saturating_sub(x), height: bottom.saturating_sub(y) }
    }

    /// The smallest rectangle holding both (an empty one adds nothing).
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let width = self.right().max(other.right()) - x;
        Self { x, y, width, height: self.bottom().max(other.bottom()) - y }
    }
}

/// The caller's pixel buffer: `width × height` pixels, row after row, `0x00RRGGBB` (the
/// `softbuffer` buffer itself; `ferrum-render` keeps no copy, `docs/03` §5).
#[derive(Debug)]
pub struct Target<'a> {
    pixels: &'a mut [u32],
    width: u32,
    height: u32,
}

impl<'a> Target<'a> {
    /// Wraps `pixels`.
    ///
    /// # Errors
    ///
    /// [`RenderError::BufferSize`] when `pixels` is not exactly `width × height` long.
    pub fn new(pixels: &'a mut [u32], width: u32, height: u32) -> Result<Self, RenderError> {
        let len = usize::try_from(u64::from(width) * u64::from(height));
        if len.ok() != Some(pixels.len()) {
            return Err(RenderError::BufferSize);
        }
        Ok(Self { pixels, width, height })
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The whole buffer.
    pub fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.width, self.height)
    }

    /// Fills `rect` (clipped to the buffer) with `color`.
    pub fn fill(&mut self, rect: Rect, color: Rgb) {
        let rect = rect.intersect(&self.bounds());
        for y in rect.y..rect.bottom() {
            if let Some(span) = self.span_mut(y, rect.x, rect.width) {
                span.fill(color.pixel());
            }
        }
    }

    /// The pixels of row `y` from `x`, at most `len` of them, clipped to the buffer.
    pub(crate) fn span_mut(&mut self, y: u32, x: u32, len: u32) -> Option<&mut [u32]> {
        if y >= self.height || x >= self.width {
            return None;
        }
        let start = usize::try_from(u64::from(y) * u64::from(self.width) + u64::from(x)).ok()?;
        let len = usize::try_from(len.min(self.width - x)).ok()?;
        self.pixels.get_mut(start..start.checked_add(len)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_buffer_of_the_wrong_length_is_refused() {
        let mut pixels = vec![0; 11];
        assert_eq!(Target::new(&mut pixels, 4, 3).err(), Some(RenderError::BufferSize));
        assert!(Target::new(&mut pixels, 11, 1).is_ok());
    }

    #[test]
    fn fill_is_clipped_to_the_buffer() {
        let mut pixels = vec![0; 12];
        let mut target = Target::new(&mut pixels, 4, 3).unwrap();
        target.fill(Rect::new(2, 1, 10, 10), Rgb::new(0, 0, 1));
        assert_eq!(pixels, [0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 1]);
    }

    #[test]
    fn rectangles_intersect_and_unite() {
        let a = Rect::new(0, 0, 4, 4);
        let b = Rect::new(2, 3, 4, 4);
        assert_eq!(a.intersect(&b), Rect::new(2, 3, 2, 1));
        assert_eq!(a.union(&b), Rect::new(0, 0, 6, 7));
        assert!(a.intersect(&Rect::new(9, 9, 1, 1)).is_empty());
        assert_eq!(Rect::default().union(&b), b);
    }
}
