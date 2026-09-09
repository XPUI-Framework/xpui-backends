//! An [`xpui`] backend that draws through any `embedded-graphics`
//! [`DrawTarget`].
//!
//! That covers most of the embedded Rust display ecosystem: e-paper panels,
//! SSD1306 and friends, colour TFTs, and the desktop simulator.
//!
//! ```rust,no_run
//! # use embedded_graphics::pixelcolor::BinaryColor;
//! # use embedded_graphics::prelude::*;
//! # use embedded_graphics::primitives::Rectangle;
//! # use xpui::{App, Button, Screen, Text, View};
//! # use xpui_eg::{Backend, Palette};
//! # /// Whatever driver you already have: a panel, an OLED, a colour TFT.
//! # struct MyPanel;
//! # impl MyPanel { fn flush(&mut self) {} }
//! # impl Dimensions for MyPanel {
//! #     fn bounding_box(&self) -> Rectangle { Rectangle::new(Point::zero(), Size::new(480, 800)) }
//! # }
//! # impl DrawTarget for MyPanel {
//! #     type Color = BinaryColor;
//! #     type Error = core::convert::Infallible;
//! #     fn draw_iter<I>(&mut self, _pixels: I) -> Result<(), Self::Error>
//! #     where I: IntoIterator<Item = Pixel<Self::Color>> { Ok(()) }
//! # }
//! # struct MainMenu;
//! # impl MainMenu { fn new() -> Self { MainMenu } }
//! # impl Screen for MainMenu {
//! #     type Message = ();
//! #     fn body(&self) -> impl View<()> { Text::new("Main menu") }
//! #     fn update(&mut self, _message: ()) {}
//! # }
//! # fn millis_since_boot() -> u32 { 0 }
//! # let display = MyPanel;
//! let backend = Backend::leak(display, Palette::INK_IS_ON);
//! unsafe { xpui::host::install(backend) };
//!
//! let mut app = App::new(MainMenu::new());
//! while app.is_running() {
//!     backend.begin_frame(millis_since_boot());
//!     backend.press(Button::Down);          // from wherever your input comes from
//!     app.tick();
//!     app.render_if_dirty();
//!     backend.with_display(|display| display.flush());
//! }
//! ```
//!
//! # Ask your driver which colour is ink
//!
//! The one decision here with no compiler help. `Palette::INK_IS_ON` is the
//! common case, but some 1-bit panels invert: `uc8151`, the Badger 2040's
//! controller, maps `BinaryColor::Off` to black so bitmaps load unmirrored,
//! and wants [`Palette::INK_IS_OFF`].
//!
//! Both polarities compile, both are plausible, and choosing wrong produces a
//! fully inverted panel with no error anywhere — so read the driver's docs
//! rather than guessing, and prefer the named constants over spelling out a
//! pair of enum variants at the call site.
//!
//! # Monochrome by design, not by limitation
//!
//! `xpui` paints in ink and background:
//! [`Canvas::fill_rect`](xpui::host::Canvas::fill_rect) takes a `bool`, and
//! `scrim` and `fill_rect_dither` only mean anything on a panel with one bit
//! per pixel. That is deliberate — it is a framework for e-ink.
//!
//! This backend is still generic over `PixelColor`. It takes a [`Palette`] of
//! two colours at construction and maps ink and background onto them, so the
//! same screens run on a colour TFT looking monochrome. What it does not do is
//! let a screen ask for a third colour, because the framework has no way to.

#![no_std]

extern crate alloc;

mod backend;
mod canvas;
mod clip;
mod display;
mod fonts;
mod guarded;
mod input;
mod paced_fill;
mod palette;
mod traits;

pub use backend::Backend;
pub use display::DisplayLoan;
/// The trait a [`Backend`] draws through, re-exported for the same reason the
/// typefaces are: a caller that writes its own `fn wire<D: DrawTarget>` needs
/// this bound, and reaching it through a second `embedded-graphics` dependency
/// is how the two versions drift apart. They are different types when they do,
/// and the error names the same path twice.
pub use embedded_graphics::draw_target::DrawTarget;
pub use fonts::{
    Face, Family, Fonts, HELVETICA, Piece, Tier, advance, clear_chosen_family, font_id, pieces,
    request_family,
};
pub use input::InputState;
pub use paced_fill::PacedFill;
pub use palette::Palette;
/// The typefaces, re-exported so a caller can assemble a [`Family`] of its own
/// without adding a second dependency on `u8g2-fonts` and keeping the two
/// versions in step.
///
/// `Font` is among them because [`font_tier!`](crate::font_tier) names it: the
/// macro reads each face's bytes to derive that tier's id.
pub use u8g2_fonts::{Font, FontRenderer, fonts as u8g2};
pub use xpui_chrome::{Labels, Metrics};

/// The crate's prose, compiled: a README that does not build is worse than
/// none.
#[cfg(doctest)]
mod guides {
    #[doc = include_str!("../README.md")]
    pub mod readme {}
    #[doc = include_str!("../docs/screenshots.md")]
    pub mod screenshots {}
}
