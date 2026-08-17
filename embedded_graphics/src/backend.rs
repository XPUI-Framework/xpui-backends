//! The backend itself: what it holds while a frame runs, and how a caller
//! builds, drives and feeds one.

use core::cell::{Cell, RefCell};

use embedded_graphics::prelude::*;
use xpui::{Button, Point, Rect, SwipeDir};
use xpui_boards::Board;
use xpui_chrome::Tokens;

use crate::fonts::Fonts;
use crate::input::InputState;
use crate::palette::Palette;

/// Everything the backend mutates while a frame runs.
pub(crate) struct Frame<D> {
    pub(crate) display: D,
    /// Drawing outside this is discarded. `None` means the whole panel.
    pub(crate) clip: Option<Rect>,
    pub(crate) input: InputState,
}

/// An `xpui` host over an `embedded-graphics` display.
pub struct Backend<D: DrawTarget> {
    pub(crate) frame: RefCell<Frame<D>>,
    pub(crate) palette: Palette<D::Color>,
    pub(crate) fonts: Fonts,
    /// The chrome this backend paints with.
    ///
    /// Per backend rather than a global, because the whole point of the board
    /// presets is that a 296x128 panel and a 480x800 one need different
    /// numbers — and a process can drive both, as the screenshot tests do.
    pub(crate) tokens: Tokens,
    pub(crate) millis: Cell<u32>,
    /// Set whenever the framework asks for a repaint, so a caller driving its
    /// own loop can tell whether pushing pixels is worth it.
    pub(crate) dirty: Cell<bool>,
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

    /// A backend sized for a board: its chrome, its type, and its palette.
    ///
    /// The same `Board` the simulator reads, so a screen laid out in a window
    /// and the same screen on the hardware measure against identical numbers.
    ///
    /// The board's tokens carry its UI scale, and the faces are chosen to fit
    /// those tokens — so a board that asks for finger-sized chrome gets type to
    /// match it, and a 296x128 strip does not get a face taller than its own
    /// hint band.
    pub fn for_board(display: D, board: Board, palette: Palette<D::Color>) -> Self {
        let mut backend = Backend::new(display, palette)
            .with_tokens(board.tokens)
            .with_fonts(Fonts::for_tokens(&board.tokens));
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
}
