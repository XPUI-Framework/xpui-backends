//! `PacedFill`'s whole job is a negative: two methods must **not** reach the
//! display it wraps.
//!
//! That is invisible everywhere it used to live. A forwarded `fill_solid` draws
//! the same pixels on a framebuffer, passes every snapshot, and only misbehaves
//! on a panel whose write strobe is driven by the driver's shortcut — which no
//! test rig here has. So the property is asserted directly instead: a target
//! that counts which method it was asked for.

use embedded_graphics::Pixel;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{Dimensions, Point, Size};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::primitives::Rectangle;
use xpui_eg::PacedFill;

/// A display that draws nothing and remembers how it was asked.
#[derive(Default)]
struct Counting {
    fill_solid: usize,
    fill_contiguous: usize,
    draw_iter: usize,
    /// Colours actually consumed, which is how a fill of the wrong size shows.
    pixels: usize,
}

impl Dimensions for Counting {
    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(Point::zero(), Size::new(8, 4))
    }
}

impl DrawTarget for Counting {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.draw_iter += 1;
        self.pixels += pixels.into_iter().count();
        Ok(())
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        self.fill_contiguous += 1;
        // Exactly the area's worth, and no more. `PacedFill` hands over an
        // unbounded `repeat`, so a `count()` here would never return — which is
        // itself the contract every `fill_contiguous` already owes.
        let wanted = (area.size.width * area.size.height) as usize;
        self.pixels += colors.into_iter().take(wanted).count();
        Ok(())
    }

    fn fill_solid(&mut self, _area: &Rectangle, _color: Self::Color) -> Result<(), Self::Error> {
        self.fill_solid += 1;
        Ok(())
    }
}

/// A solid fill is turned into a contiguous one, with a colour per pixel.
#[test]
fn a_solid_fill_never_reaches_the_display() {
    let mut paced = PacedFill::new(Counting::default());
    let area = Rectangle::new(Point::new(1, 1), Size::new(4, 3));
    paced.fill_solid(&area, BinaryColor::On).unwrap();

    let inner = paced.into_inner();
    assert_eq!(
        inner.fill_solid, 0,
        "the display's own fill_solid is the shortcut this type exists to avoid"
    );
    assert_eq!(inner.fill_contiguous, 1, "it went out as a contiguous fill");
    assert_eq!(inner.pixels, 12, "one colour per pixel of a 4x3 area");
}

/// And so is a clear, which the trait would otherwise route around the wrapper.
#[test]
fn clearing_never_reaches_the_display_either() {
    let mut paced = PacedFill::new(Counting::default());
    paced.clear(BinaryColor::Off).unwrap();

    let inner = paced.into_inner();
    assert_eq!(inner.fill_solid, 0, "clear must not take the shortcut");
    assert_eq!(inner.fill_contiguous, 1);
    assert_eq!(inner.pixels, 32, "the whole 8x4 bounding box");
}

/// The other two are forwarded untouched: pacing a path that already sends a
/// colour per pixel would cost time and buy nothing.
#[test]
fn the_per_pixel_paths_are_forwarded_unchanged() {
    let mut paced = PacedFill::new(Counting::default());
    paced
        .draw_iter([Pixel(Point::new(0, 0), BinaryColor::On)])
        .unwrap();
    paced
        .fill_contiguous(
            &Rectangle::new(Point::zero(), Size::new(2, 2)),
            [BinaryColor::On; 4],
        )
        .unwrap();

    let inner = paced.into_inner();
    assert_eq!(inner.draw_iter, 1, "forwarded once");
    assert_eq!(inner.fill_contiguous, 1, "forwarded once");
    assert_eq!(inner.fill_solid, 0);
    assert_eq!(inner.pixels, 5, "one pixel, then four");
}
