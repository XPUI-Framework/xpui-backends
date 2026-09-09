//! A display whose solid fills go out one pixel at a time.
//!
//! Opt-in: an SPI panel clocks its own bytes and a framebuffer has no timing,
//! so neither is wrapped.

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Dimensions;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;

/// Wraps a display, replacing its repeated-pixel path with a per-pixel one.
///
/// `mipidsi` shortens a run of one colour into a strobe loop that outruns an
/// 8080-style controller's write cycle, and the fill lands as noise.
pub struct PacedFill<D>(D);

impl<D> PacedFill<D> {
    /// Wraps a display so its solid fills go out a pixel at a time.
    pub fn new(display: D) -> Self {
        PacedFill(display)
    }

    /// The display back, for a caller that has finished pacing it.
    pub fn into_inner(self) -> D {
        self.0
    }
}

impl<D: Dimensions> Dimensions for PacedFill<D> {
    fn bounding_box(&self) -> Rectangle {
        self.0.bounding_box()
    }
}

impl<D: DrawTarget> DrawTarget for PacedFill<D> {
    type Color = D::Color;
    type Error = D::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.0.draw_iter(pixels)
    }

    /// Already one colour per pixel, so it is handed straight over — this is
    /// the path the two below are redirected *onto*.
    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        self.0.fill_contiguous(area, colors)
    }

    /// The fix. `repeat` is unbounded on purpose: `fill_contiguous` takes
    /// exactly the number of pixels the area needs and stops.
    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        self.0.fill_contiguous(area, core::iter::repeat(color))
    }

    /// Redundant with the trait default today, and kept anyway.
    ///
    /// The default is `self.fill_solid(&self.bounding_box(), color)`, and
    /// `self` is this wrapper — so it already lands on the override above.
    /// Written out so that a change to the default cannot quietly route a
    /// clear around the pacing, and because the mistake that is easy to make
    /// here is forwarding to the inner display, which the tests do catch.
    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        let area = self.bounding_box();
        self.fill_solid(&area, color)
    }
}
