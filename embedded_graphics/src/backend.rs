//! The backend itself: what it holds while a frame runs, and how a caller
//! builds, drives and feeds one.

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use embedded_graphics::prelude::*;
use xpui::{Button, Point, Rect, SwipeDir};
use xpui_boards::Board;
use xpui_chrome::Tokens;

use crate::fonts::{Family, Fonts};
use crate::guarded::Guarded;
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
    pub(crate) frame: Guarded<Frame<D>>,
    pub(crate) palette: Palette<D::Color>,
    /// The type this backend is set in.
    ///
    /// Guarded rather than a `Cell` because it can change while the thing is
    /// running — a font picker is a screen like any other, and by the time one
    /// is on screen the backend is behind a `&'static`. Four words is too wide
    /// to be an atomic, so a torn read is possible without the guard.
    fonts: Guarded<Fonts>,
    /// The chrome this backend paints with.
    ///
    /// Per backend rather than a global, because the whole point of the board
    /// presets is that a 296x128 panel and a 480x800 one need different
    /// numbers — and a process can drive both, as the screenshot tests do.
    pub(crate) tokens: Tokens,
    /// Atomics rather than `Cell`s, and no guard: a load and a store are all
    /// either needs, and Cortex-M0+ has both. What it does **not** have is
    /// compare-and-swap, so nothing here may become a read-modify-write.
    pub(crate) millis: AtomicU32,
    /// Set whenever the framework asks for a repaint, so a caller driving its
    /// own loop can tell whether pushing pixels is worth it.
    pub(crate) dirty: AtomicBool,
    /// The board this was built for, when it was built from one. A frame loop
    /// needs `refresh_ms` to decide how often polling is worth it, and without
    /// this it has to keep a second copy that can drift from the first.
    board: Option<Board>,
}

// With the `critical-section` feature there is **no `unsafe impl` here at
// all**: `Guarded` is a `critical_section::Mutex`, the two counters are
// atomics, and the compiler works `Sync` out for itself. Nothing is claimed,
// so nothing can be claimed wrongly.
//
// Safety: without that feature, **this backend must be driven from one
// thread.** Not a style note — the consequence of breaking it is undefined
// behaviour, not a clean error.
//
// `xpui` requires `Host: Sync` because a firmware may paint on a second task
// (see `xpui::screen::Screen::body`), and this claim is what satisfies that
// bound. But `Guarded` is then a bare `RefCell`, whose borrow flag is a
// non-atomic counter: two threads can both take `borrow_mut` and end up with
// aliasing `&mut D`. The friendlier outcome is an "already borrowed" panic,
// which under this workspace's `panic = "abort"` takes the firmware down.
//
// So: a desktop simulator, or a bare-metal loop that ticks and paints in one
// place, is fine — that is every consumer of the default. A host that renders
// on its own task must turn the feature on. `examples/rp2040` turns it on
// without being one: it is a single executor task, and the reason there is
// that an interrupt handler could plausibly reach the backend.
//
// `D: Send` because sharing a `&Backend<D>` is only meaningful if `D` itself
// could have moved between threads; without it a `D` holding an `Rc` would be
// smuggled across one.
#[cfg(not(feature = "critical-section"))]
unsafe impl<D: DrawTarget + Send> Sync for Backend<D> {}

// And with the feature on, that the compiler agrees — here, rather than two
// crates away at a use site.
//
// Without this, adding an unguarded `Cell` to `Backend` still compiles: the
// library never names `Sync` itself, so the error surfaces only where a
// backend is installed or shared. That is exactly how the bug this feature
// fixes arose — three `Cell`s crept in under one `unsafe impl` and nothing
// local objected.
#[cfg(feature = "critical-section")]
const _: () = {
    // The supertrait is the assertion: an `impl` of it is only accepted if
    // `Backend<D>` really is `Sync`, and that is checked here rather than at
    // a call, so nothing has to be invoked for it to bite.
    //
    // Never used, and that is the point — it exists to be compiled, not
    // called.
    #[expect(dead_code, reason = "a compile-time assertion has no callers")]
    trait IsSync: Sync {}
    impl<D: DrawTarget + Send> IsSync for Backend<D> where D::Color: Sync {}
};

impl<D: DrawTarget> Backend<D> {
    pub fn new(display: D, palette: Palette<D::Color>) -> Self {
        Backend {
            frame: Guarded::new(Frame {
                display,
                clip: None,
                input: InputState::default(),
            }),
            palette,
            fonts: Guarded::new(Fonts::DEFAULT),
            tokens: Tokens::DEFAULT,
            millis: AtomicU32::new(0),
            dirty: AtomicBool::new(true),
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

    pub fn with_fonts(self, fonts: Fonts) -> Self {
        self.fonts.with(|current| *current = fonts);
        self
    }

    /// The type this backend is currently set in.
    pub fn fonts(&self) -> Fonts {
        self.fonts.with(|fonts| *fonts)
    }

    /// The chrome this backend paints with.
    pub fn tokens(&self) -> &Tokens {
        &self.tokens
    }

    /// Sets the type in another family, and asks for a repaint.
    ///
    /// **The sizes are re-derived from the chrome, not carried over.** Each
    /// role keeps the line height this board's tokens called for and the new
    /// family answers with the tier it was cut in — so a family with a coarser
    /// ladder still lands inside the rows that were laid out, rather than
    /// inheriting the previous family's heights and overflowing them.
    ///
    /// **This replaces a [`Fonts`] given to [`with_fonts`](Backend::with_fonts).**
    /// The sizes come from the tokens, so a caller that hand-picked its own
    /// gets the chrome's back. That is the point — a swap must not carry sizes
    /// into a family that was never measured for them — but it means
    /// `with_fonts` is for a backend nothing will re-set.
    ///
    /// Every id every role reports changes with it, because an id is a hash of
    /// the bytes behind it. Anything keyed on one — a cached page layout, a
    /// measured column — is invalidated by that alone, with nobody having to
    /// remember to say so.
    pub fn set_family(&self, family: &'static Family) {
        let next = Fonts::for_tokens(&self.tokens).with_family(family);
        self.fonts.with(|fonts| *fonts = next);
        // Both flags, because they answer to different readers. `dirty` is for
        // a caller driving its own loop; `request_update` is what `App`
        // consults, and setting only the first leaves a panel painted in the
        // face that has just been replaced — on e-ink, until something else
        // happens to change.
        // Set before `request_update`, which reaches this same backend
        // through the installed host — and does so without a guard held,
        // because `dirty` is an atomic rather than guarded state.
        self.dirty.store(true, Ordering::Relaxed);
        xpui::host::request_update();
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
    ///
    /// A family a screen asked for lands here, which is the one moment it can
    /// safely: between frames, before anything has been measured against the
    /// type it is about to replace.
    pub fn begin_frame(&self, millis: u32) {
        // Reconciled rather than consumed: every backend catches up to the
        // application's choice as it opens a frame, so switching to one that
        // was built before the choice was made does not silently undo it.
        // Compared by address, or this would set the family — and ask for a
        // repaint — on every frame forever.
        //
        // Read, then set, as two separate guarded calls: `set_family` takes
        // the same guard, and taking it while it is held would panic.
        let current = self.fonts.with(|fonts| fonts.family);
        if let Some(family) = crate::fonts::chosen_family()
            && !core::ptr::eq(family, current)
        {
            self.set_family(family);
        }
        self.millis.store(millis, Ordering::Relaxed);
        self.frame.with(|frame| frame.input.begin_frame());
    }

    /// Feeds the frame, for a caller that has its own event source.
    ///
    /// The closure runs with this backend's state borrowed, so it must not
    /// call back into the backend — no drawing, no `screen_size`. Feed input
    /// and return.
    pub fn input(&self, feed: impl FnOnce(&mut InputState)) {
        self.frame.with(|frame| feed(&mut frame.input));
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
    /// **Not a test-and-clear, and it cannot be made one.** A caller that
    /// writes `if is_dirty() { clear_dirty(); paint(); }` loses any
    /// `request_update` landing between the two, and on e-ink a lost repaint
    /// means a stale panel until the user presses something. Closing that
    /// needs an atomic swap, which Cortex-M0+ does not have — the same
    /// constraint that made these flags plain load-and-store in the first
    /// place. Clear it *after* painting, as [`App::render_if_dirty`] does.
    ///
    /// [`clear_dirty`]: Backend::clear_dirty
    /// [`App::render_if_dirty`]: xpui::App::render_if_dirty
    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }

    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::Relaxed);
    }

    /// Borrows the display, for pushing the framebuffer to a panel.
    ///
    /// Same rule as [`input`](Backend::input): the closure holds this
    /// backend's state, so it must not draw through the backend while inside.
    /// Flush the panel, read the pixels, return.
    pub fn with_display<R>(&self, body: impl FnOnce(&mut D) -> R) -> R {
        self.frame.with(|frame| body(&mut frame.display))
    }
}
