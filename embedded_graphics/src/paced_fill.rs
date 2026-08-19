//! A display whose solid fills go out one pixel at a time.
//!
//! An 8080-style parallel panel latches a byte on the rising edge of its write
//! strobe, and the controller has a minimum write *cycle* — 66 ns on the
//! ST7789v — that the strobe has to respect. A driver that sends a run of one
//! colour can notice the data lines already hold the right value and skip
//! setting them, leaving a loop that does nothing but pulse the strobe:
//!
//! ```text
//! wr.set_low();
//! wr.set_high();
//! ```
//!
//! Two register stores. On a 125 MHz RP2040 in release that is a write cycle
//! of roughly 24–40 ns — comfortably inside the controller's minimum, so it
//! mislatches, and the fill arrives as noise. Writing the data pins each time
//! is what keeps the cycle long enough; the shortcut is what breaks it.
//!
//! The trap is that the shortcut only triggers when every byte of the pixel is
//! **identical**, and for `Rgb565` over an 8-bit bus that is true of exactly
//! two colours: black (`0x0000`) and white (`0xFFFF`). Those are ink and
//! background on a monochrome-styled panel, so *every* filled rectangle and
//! every screen clear takes the broken path while text — which is drawn pixel
//! by pixel — comes out perfectly. A panel showing crisp type over static is
//! this bug, and it reads like a framework fault rather than a timing one.
//!
//! So fills are routed through `fill_contiguous`, which has a colour for every
//! pixel and therefore no run to shorten. A whole 320x240 screen costs about
//! 12 ms that way, against a panel with no refresh delay of its own to hide
//! behind — cheap for a fill that is correct.
//!
//! Wanted only by a parallel bus. An SPI panel clocks its own bytes out and a
//! framebuffer has no timing at all, so neither is wrapped — this is opt-in,
//! and a target that does not need it should not pay for it.
//!
//! It lives here rather than beside the firmware that needs it because it
//! names no HAL, no pin and no board: it is a `DrawTarget` that wraps a
//! `DrawTarget`. Beside the firmware it was untestable, and the property it
//! exists for — that two methods are *not* forwarded — is invisible on the
//! host, invisible in every snapshot, and only appears on hardware.

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Dimensions;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;

/// Wraps a display, replacing its repeated-pixel path with a per-pixel one.
pub struct PacedFill<D>(D);

impl<D> PacedFill<D> {
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

    /// Not inherited, because the default body is `fill_solid` on the *inner*
    /// display's bounding box — which would route straight back around this.
    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        let area = self.bounding_box();
        self.fill_solid(&area, color)
    }
}
