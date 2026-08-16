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
//! `xpui` paints in ink and background: [`Canvas::fill_rect`] takes a `bool`,
//! and `scrim` and `fill_rect_dither` only mean anything on a panel with one
//! bit per pixel. That is deliberate — it is a framework for e-ink.
//!
//! This backend is still generic over `PixelColor`. It takes a [`Palette`] of
//! two colours at construction and maps ink and background onto them, so the
//! same screens run on a colour TFT looking monochrome. What it does not do is
//! let a screen ask for a third colour, because the framework has no way to.

#![cfg_attr(target_os = "none", no_std)]

extern crate alloc;

use core::cell::{Cell, RefCell};

use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Line, PrimitiveStyle, Rectangle};
use embedded_graphics::text::{Baseline, Text as EgText};

use xpui::host::{Canvas, Clock, FontId, FontRole, FontStyle, IconRef, InputSource, TextMetrics};
use xpui::{Button, Point, Rect, Size, SwipeDir};

mod fonts;
#[cfg(feature = "framebuffer")]
pub mod framebuffer;
mod input;

pub use fonts::Fonts;
#[cfg(feature = "framebuffer")]
pub use framebuffer::Framebuffer;
pub use input::InputState;
pub use xpui_boards::Board;
pub use xpui_chrome::Tokens;

/// The two colours ink and background map onto.
#[derive(Copy, Clone, Debug)]
pub struct Palette<C> {
    pub ink: C,
    pub background: C,
}

impl<C> Palette<C> {
    pub fn new(ink: C, background: C) -> Self {
        Palette { ink, background }
    }
}

impl Palette<BinaryColor> {
    /// Ink is `On`. The common case, and what the simulator uses.
    pub const INK_IS_ON: Self = Palette {
        ink: BinaryColor::On,
        background: BinaryColor::Off,
    };

    /// Ink is `Off`. Some 1-bit panels invert: `uc8151`, the Badger 2040's
    /// controller, maps `Off` to black so bitmaps load unmirrored.
    pub const INK_IS_OFF: Self = Palette {
        ink: BinaryColor::Off,
        background: BinaryColor::On,
    };
}

/// Everything the backend mutates while a frame runs.
struct Frame<D> {
    display: D,
    /// Drawing outside this is discarded. `None` means the whole panel.
    clip: Option<Rect>,
    input: InputState,
}

/// An `xpui` host over an `embedded-graphics` display.
pub struct Backend<D: DrawTarget> {
    frame: RefCell<Frame<D>>,
    palette: Palette<D::Color>,
    fonts: Fonts,
    /// The chrome this backend paints with.
    ///
    /// Per backend rather than a global, because the whole point of the board
    /// presets is that a 296x128 panel and a 480x800 one need different
    /// numbers — and a process can drive both, as the screenshot tests do.
    tokens: Tokens,
    millis: Cell<u32>,
    /// Set whenever the framework asks for a repaint, so a caller driving its
    /// own loop can tell whether pushing pixels is worth it.
    dirty: Cell<bool>,
    /// The board this was built for, when it was built from one. A frame loop
    /// needs `refresh_ms` to decide how often polling is worth it, and without
    /// this it has to keep a second copy that can drift from the first.
    board: Option<Board>,
}

// Safety: **this backend must be driven from one thread.** Not a style note —
// the consequence of breaking it is undefined behaviour, not a clean error.
//
// `xpui` requires `Host: Sync` because a firmware may paint on a second task
// (see `xpui::screen::Screen::body`), and this claim is what satisfies that
// bound. But the state below sits behind a `RefCell`, whose borrow flag is a
// non-atomic counter: two threads can both take `borrow_mut` and end up with
// aliasing `&mut D`. The friendlier outcome is a "already borrowed" panic,
// which under this workspace's `panic = "abort"` takes the firmware down.
//
// So: a desktop simulator, or a bare-metal loop that ticks and paints in one
// place, is fine — that is every consumer today. A host that renders on its
// own task must not use this type; it should implement `Host` over whatever
// synchronisation it already has, which is what the FreeInkUI backend does.
//
// `D: Send` because sharing a `&Backend<D>` is only meaningful if `D` itself
// could have moved between threads; without it a `D` holding an `Rc` would be
// smuggled across one.
unsafe impl<D: DrawTarget + Send> Sync for Backend<D> {}

impl<D: DrawTarget> Backend<D> {
    pub fn new(display: D, palette: Palette<D::Color>) -> Self {
        Backend {
            frame: RefCell::new(Frame {
                display,
                clip: None,
                input: InputState::default(),
            }),
            palette,
            fonts: Fonts::DEFAULT,
            tokens: Tokens::DEFAULT,
            millis: Cell::new(0),
            dirty: Cell::new(true),
            board: None,
        }
    }

    /// A backend sized for a board: its chrome, and its palette.
    ///
    /// The same `Board` the simulator reads, so a screen laid out in a window
    /// and the same screen on the hardware measure against identical numbers.
    pub fn for_board(display: D, board: Board, palette: Palette<D::Color>) -> Self {
        let mut backend = Backend::new(display, palette).with_tokens(board.tokens);
        backend.board = Some(board);
        backend
    }

    /// The board this backend was built for, if it was built from one.
    ///
    /// `None` from [`Backend::new`], which is given a size and no board.
    pub fn board(&self) -> Option<Board> {
        self.board
    }

    /// Paints chrome with these tokens instead of the default.
    pub fn with_tokens(mut self, tokens: Tokens) -> Self {
        self.tokens = tokens;
        self
    }

    pub fn with_fonts(mut self, fonts: Fonts) -> Self {
        self.fonts = fonts;
        self
    }

    /// Leaks the backend so it can be installed.
    ///
    /// [`xpui::host::install`] takes a `&'static`, and a backend lives as long
    /// as the program that draws with it, so this is one allocation that was
    /// never going to be freed anyway.
    pub fn leak(display: D, palette: Palette<D::Color>) -> &'static Self
    where
        D: 'static,
    {
        alloc::boxed::Box::leak(alloc::boxed::Box::new(Backend::new(display, palette)))
    }

    /// [`leak`](Backend::leak), sized for a board.
    pub fn leak_for_board(display: D, board: Board, palette: Palette<D::Color>) -> &'static Self
    where
        D: 'static,
    {
        alloc::boxed::Box::leak(alloc::boxed::Box::new(Backend::for_board(
            display, board, palette,
        )))
    }

    /// Starts a frame: advances the clock and clears one-frame input.
    pub fn begin_frame(&self, millis: u32) {
        self.millis.set(millis);
        self.frame.borrow_mut().input.begin_frame();
    }

    /// Feeds the frame, for a caller that has its own event source.
    ///
    /// The closure runs with this backend's state borrowed, so it must not
    /// call back into the backend — no drawing, no `screen_size`. Feed input
    /// and return.
    pub fn input(&self, feed: impl FnOnce(&mut InputState)) {
        feed(&mut self.frame.borrow_mut().input);
    }

    pub fn press(&self, button: Button) {
        self.input(|state| state.press(button));
    }

    pub fn release(&self, button: Button) {
        self.input(|state| state.release(button));
    }

    pub fn tap(&self, at: Point) {
        self.input(|state| state.tap(at));
    }

    pub fn swipe(&self, direction: SwipeDir) {
        self.input(|state| state.swipe(direction));
    }

    /// Whether the framework has asked for a repaint since [`clear_dirty`].
    ///
    /// [`clear_dirty`]: Backend::clear_dirty
    pub fn is_dirty(&self) -> bool {
        self.dirty.get()
    }

    pub fn clear_dirty(&self) {
        self.dirty.set(false);
    }

    /// Borrows the display, for pushing the framebuffer to a panel.
    ///
    /// Same rule as [`input`](Backend::input): the closure holds this
    /// backend's state, so it must not draw through the backend while inside.
    /// Flush the panel, read the pixels, return.
    pub fn with_display<R>(&self, body: impl FnOnce(&mut D) -> R) -> R {
        body(&mut self.frame.borrow_mut().display)
    }

    // -- drawing -----------------------------------------------------------

    fn colour(&self, ink: bool) -> D::Color {
        if ink {
            self.palette.ink
        } else {
            self.palette.background
        }
    }

    /// Whether any of `rect` is inside the clip. Used only to skip work; the
    /// clipping itself is done by the target, not by arithmetic here.
    fn intersects_clip(&self, frame: &Frame<D>, rect: Rect) -> bool {
        let Some(clip) = frame.clip else {
            return true;
        };
        rect.x() < clip.x() + clip.width()
            && clip.x() < rect.x() + rect.width()
            && rect.y() < clip.y() + clip.height()
            && clip.y() < rect.y() + rect.height()
    }
}

/// Runs `$body` against a draw target that honours the current clip.
///
/// `DrawTargetExt::clipped` drops out-of-area pixels inside the target, which
/// is the only way to clip *text*: glyphs are rasterised by the font, and
/// intersecting rectangles beforehand cannot cut one in half. Doing it by
/// arithmetic instead is a bug this backend shipped once — a list scrolled
/// under the header painted its rows straight over it.
///
/// A macro rather than a function because the two arms have different target
/// types and `DrawTarget` is not object-safe.
macro_rules! with_clip {
    ($frame:expr, |$target:ident| $body:expr) => {
        match $frame.clip {
            Some(clip) => {
                let area = to_eg_rect(clip);
                let mut clipped = $frame.display.clipped(&area);
                let $target = &mut clipped;
                let _ = $body;
            }
            None => {
                let $target = &mut $frame.display;
                let _ = $body;
            }
        }
    };
}

impl<D: DrawTarget> Backend<D> {
    fn fill(&self, rect: Rect, colour: D::Color) {
        let mut frame = self.frame.borrow_mut();
        if !self.intersects_clip(&frame, rect) {
            return;
        }
        let area = to_eg_rect(rect);
        // Errors are swallowed on purpose: a `DrawTarget` failing mid-frame is
        // a display fault, and there is nothing a UI framework can do about it
        // that is better than drawing the rest of the screen.
        with_clip!(frame, |target| target.fill_solid(&area, colour));
    }

    /// Paints `colour` on the pixels of `rect` whose coordinates sum to
    /// `parity`.
    ///
    /// One `draw_iter` rather than a pixel loop, so a target that batches gets
    /// to. Used for both dithering and the scrim, which differ only in whether
    /// the other parity is cleared first.
    fn stipple(&self, rect: Rect, colour: D::Color, parity: u32) {
        let mut frame = self.frame.borrow_mut();
        if !self.intersects_clip(&frame, rect) {
            return;
        }
        let pixels = (rect.y()..rect.y() + rect.height()).flat_map(move |y| {
            (rect.x()..rect.x() + rect.width())
                .filter(move |x| (*x as u32).wrapping_add(y as u32) % 2 == parity)
                .map(move |x| Pixel(EgPoint::new(x, y), colour))
        });
        with_clip!(frame, |target| target.draw_iter(pixels));
    }
}

type EgPoint = embedded_graphics::geometry::Point;

fn to_eg_rect(rect: Rect) -> Rectangle {
    Rectangle::new(
        EgPoint::new(rect.x(), rect.y()),
        embedded_graphics::geometry::Size::new(
            rect.width().max(0) as u32,
            rect.height().max(0) as u32,
        ),
    )
}

impl<D: DrawTarget> Canvas for Backend<D> {
    fn screen_size(&self) -> Size {
        let bounds = self.frame.borrow().display.bounding_box();
        Size::new(bounds.size.width as i32, bounds.size.height as i32)
    }

    fn clear(&self) {
        let background = self.palette.background;
        let mut frame = self.frame.borrow_mut();
        // Straight through, ignoring the clip: clearing is a whole-screen act,
        // and the framework only calls it before anything else is drawn.
        let _ = frame.display.clear(background);
    }

    fn draw_text(&self, origin: Point, text: &str, font: FontId, style: FontStyle) {
        let Some(face) = self.fonts.face(font, style) else {
            return;
        };
        let colour = self.palette.ink;
        let mut frame = self.frame.borrow_mut();
        let text_style = MonoTextStyle::new(face, colour);
        let label = EgText::with_baseline(
            text,
            EgPoint::new(origin.x, origin.y),
            text_style,
            Baseline::Top,
        );
        with_clip!(frame, |target| label.draw(target));
    }

    fn fill_rect(&self, rect: Rect, black: bool) {
        self.fill(rect, self.colour(black));
    }

    fn stroke_rect(&self, rect: Rect) {
        let colour = self.palette.ink;
        let mut frame = self.frame.borrow_mut();
        if !self.intersects_clip(&frame, rect) {
            return;
        }
        let outline = to_eg_rect(rect).into_styled(PrimitiveStyle::with_stroke(colour, 1));
        with_clip!(frame, |target| outline.draw(target));
    }

    fn draw_line(&self, from: Point, to: Point) {
        let colour = self.palette.ink;
        let mut frame = self.frame.borrow_mut();
        let line = Line::new(EgPoint::new(from.x, from.y), EgPoint::new(to.x, to.y))
            .into_styled(PrimitiveStyle::with_stroke(colour, 1));
        with_clip!(frame, |target| line.draw(target));
    }

    fn fill_rect_dither(&self, rect: Rect, light: bool) {
        // Cleared first, then patterned: this is a *fill*, so whatever was
        // behind it goes. `scrim` is the one that preserves.
        self.fill(rect, self.palette.background);
        self.stipple(rect, self.palette.ink, u32::from(light));
    }

    fn scrim(&self, rect: Rect) {
        // Ink on one parity only, and nothing cleared, so about half of what
        // was behind survives and the region reads as grey.
        self.stipple(rect, self.palette.ink, 0);
    }

    fn set_clip(&self, rect: Option<Rect>) {
        self.frame.borrow_mut().clip = rect;
    }

    fn draw_image(&self, origin: Point, data: &[u8], size: Size) {
        if size.width <= 0 || size.height <= 0 {
            return;
        }
        let stride = (size.width as usize).div_ceil(8);
        // Refused whole rather than drawn as far as the buffer goes. Skipping
        // the missing pixels one at a time paints a fragment of the image and
        // reports nothing, which reads as a corrupt asset rather than as a
        // caller passing the wrong size.
        if data.len() < stride * size.height as usize {
            return;
        }
        let ink = self.palette.ink;

        let mut frame = self.frame.borrow_mut();
        let pixels = (0..size.height).flat_map(move |row| {
            (0..size.width).filter_map(move |column| {
                let byte = *data.get(row as usize * stride + column as usize / 8)?;
                // Bit 0 is ink, inverted from the usual convention — the
                // format the framework documents, and the one e-ink panels
                // use, where a set bit is white.
                if byte & (0x80 >> (column % 8)) != 0 {
                    return None;
                }
                Some(Pixel(EgPoint::new(origin.x + column, origin.y + row), ink))
            })
        });
        with_clip!(frame, |target| target.draw_iter(pixels));
    }

    fn draw_icon(&self, origin: Point, icon: IconRef) {
        // Drawn from lines and rectangles rather than blitted from an asset
        // set: this backend has no assets, and a stored bitmap would cost
        // flash on a board that has two megabytes of it.
        xpui_chrome::draw_icon(origin, icon);
    }

    fn icon_size(&self, icon: IconRef) -> i32 {
        // 0 for an icon this crate cannot draw, which is how the framework
        // knows to reserve no space rather than leaving a hole.
        xpui_chrome::icon_size(icon)
    }
}

impl<D: DrawTarget> TextMetrics for Backend<D> {
    fn font(&self, role: FontRole) -> FontId {
        Fonts::id(role)
    }

    fn text_width(&self, font: FontId, text: &str, style: FontStyle) -> i32 {
        self.fonts
            .face(font, style)
            .map_or(0, |face| fonts::text_width(face, text))
    }

    fn line_height(&self, font: FontId) -> i32 {
        self.fonts
            .face(font, FontStyle::Regular)
            .map_or(0, fonts::line_height)
    }
}

impl<D: DrawTarget> InputSource for Backend<D> {
    fn was_pressed(&self, button: Button) -> bool {
        self.frame.borrow().input.was_pressed(button)
    }

    fn is_pressed(&self, button: Button) -> bool {
        self.frame.borrow().input.is_pressed(button)
    }

    fn was_released(&self, button: Button) -> bool {
        self.frame.borrow().input.was_released(button)
    }

    fn has_touch(&self) -> bool {
        self.frame.borrow().input.has_touch()
    }

    fn tap(&self) -> Option<Point> {
        self.frame.borrow().input.tap_at()
    }

    fn touch_held(&self) -> Option<Point> {
        self.frame.borrow().input.touch_held()
    }

    fn touch_released(&self) -> bool {
        self.frame.borrow().input.touch_was_released()
    }

    fn swipe(&self) -> SwipeDir {
        self.frame.borrow().input.swipe_direction()
    }

    fn was_back_gesture(&self) -> bool {
        self.frame.borrow().input.was_back_gesture()
    }

    fn was_home_gesture(&self) -> bool {
        self.frame.borrow().input.was_home_gesture()
    }

    fn swipe_moves_selection(&self) -> bool {
        self.frame.borrow().input.swipe_moves_selection
    }
}

impl<D: DrawTarget> Clock for Backend<D> {
    fn millis(&self) -> u32 {
        self.millis.get()
    }
}

xpui_chrome::plain_chrome! {
    generic: [D: DrawTarget],
    for Backend<D>,
    // The backend's own tokens, not a global: two backends in one process may
    // be driving two different panels.
    tokens: |backend| &backend.tokens,
    request_update: |backend| backend.dirty.set(true),
}

/// Lets `xpui`'s UI harness feed this backend input.
///
/// The methods already exist as inherent ones; this names them through a trait
/// so the harness can drive any backend without knowing which it has.
#[cfg(feature = "testing")]
impl<D: DrawTarget> xpui::testing::Drive for Backend<D> {
    fn begin(&self, millis: u32) {
        self.begin_frame(millis);
    }

    fn inject_press(&self, button: Button) {
        self.press(button);
    }

    fn inject_release(&self, button: Button) {
        self.release(button);
    }

    fn inject_tap(&self, point: Point) {
        self.tap(point);
    }

    fn inject_swipe(&self, direction: SwipeDir) {
        self.swipe(direction);
    }
}

/// The crate's prose, compiled.
///
/// A README that does not build is worse than none: this crate's only usage
/// example passed the wrong form to its own macro for as long as nothing
/// tried it.
#[cfg(doctest)]
mod guides {
    #[doc = include_str!("../README.md")]
    pub mod readme {}
}
