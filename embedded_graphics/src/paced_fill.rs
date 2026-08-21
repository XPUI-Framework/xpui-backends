//! A display whose solid fills go out one pixel at a time.
//!
//! An 8080-style parallel panel latches a byte on the rising edge of its write
//! strobe, and the controller has a minimum write *cycle* — 66 ns on the
//! ST7789v — that the strobe has to respect. When every pixel of a run has the
//! same *byte in both halves*, `mipidsi` sends the word once and then loops on
//! the strobe alone:
//!
//! ```text
//! wr.set_low();
//! wr.set_high();
//! ```
//!
//! Two register stores. On a 125 MHz RP2040 in release that is a write cycle
//! of roughly 24–40 ns — comfortably inside the controller's minimum, so it
//! mislatches, and the fill arrives as noise.
//!
//! **What costs enough time on the ordinary path is the call, not the pins.**
//! `Generic8BitBus::set_value` returns early when the value is unchanged
//! (`mipidsi`'s `interface/parallel.rs`: *"quite common for multiple
//! consecutive values to be identical … so let's optimize for that case"*), so
//! the pins are skipped there too. `send_word` stays out of line at
//! `opt-level = "z"`, and that call is what stretches the cycle — incidental
//! codegen, worth knowing because inlining it would bring the fault back with
//! nothing changed in this file.
//!
//! The trap is that the shortcut triggers whenever the two bytes of the pixel
//! are **identical**, which for `Rgb565` is **256** values and not two —
//! `0x0000` and `0xFFFF` among them, but `0x1818` is a dark blue that takes the
//! same path. Ink and background are two of the 256, so on a monochrome-styled
//! panel *every* filled rectangle and every screen clear breaks while text —
//! drawn pixel by pixel — comes out perfectly. A panel showing crisp type over
//! static is this bug, and it reads like a framework fault rather than a timing
//! one.
//!
//! So fills are routed through `fill_contiguous`, which has a colour for every
//! pixel and therefore no run to shorten. A whole 320x240 screen is
//! instruction-counted at **roughly 120 ms** that way — a loop interval is
//! 10 ms, so this is not free — and **that figure has not been measured on the
//! board**. Measure it before quoting it.
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
