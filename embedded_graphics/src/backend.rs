//! The backend itself: what it holds while a frame runs, and how a caller
//! builds, drives and feeds one.

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use embedded_graphics::prelude::*;
use xpui::host::KeyRow;
use xpui::{Button, Point, Rect, Size, SwipeDir};
use xpui_chrome::{Labels, Metrics};

use crate::fonts::{Family, Fonts};
use crate::guarded::Guarded;
use crate::input::InputState;
use crate::palette::Palette;

/// Everything the backend mutates while a frame runs.
pub(crate) struct Frame<D> {
    /// `None` while the display is on loan — see [`Backend::loan_display`].
    /// Every paint arriving in that window is discarded.
    pub(crate) display: Option<D>,
    /// The panel's size, captured once so it can still be answered while the
    /// display is out. Layout asks for it constantly and a zero would collapse
    /// every measurement silently.
    pub(crate) size: Size,
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
    /// Per backend rather than a global, because the whole point of device
    /// presets is that a 296x128 panel and a 480x800 one need different
    /// numbers — and a process can drive both, as the screenshot tests do.
    pub(crate) metrics: Metrics,
    pub(crate) labels: Labels,
    pub(crate) keys: KeyRow,
    /// Atomics rather than `Cell`s, and no guard: a load and a store are all
    /// either needs, and Cortex-M0+ has both. What it does **not** have is
    /// compare-and-swap, so nothing here may become a read-modify-write.
    pub(crate) millis: AtomicU32,
    /// Set whenever the framework asks for a repaint, so a caller driving its
    /// own loop can tell whether pushing pixels is worth it.
    pub(crate) dirty: AtomicBool,
    /// Whether the device has a Left/Right pair to nudge a value with.
    ///
    /// Stored, not derived: this backend draws on whatever `DrawTarget` it was
    /// handed and has no way to know what is around it. `false` unless a
    /// caller says otherwise, which is the safe direction rather than the
    /// accurate one — a control told the pair exists when it does not cannot
    /// be changed by any key, while one told it does not exist is entered and
    /// left instead.
    pub(crate) left_right_keys: bool,
}

// Safety: without `critical-section`, `Guarded` is a bare `RefCell`, so this
// backend must be driven from one thread — two threads in `borrow_mut` alias
// `&mut D`, which is undefined behaviour, not a clean error. A desktop
// simulator or a bare-metal loop that ticks and paints in one place is fine;
// a host that renders on its own task turns the feature on, and the compiler
// then works `Sync` out for itself. `D: Send` because sharing a `&Backend<D>`
// is only meaningful if `D` could have moved between threads.
#[cfg(not(feature = "critical-section"))]
unsafe impl<D: DrawTarget + Send> Sync for Backend<D> {}

// With the feature on, the compiler's verdict — here, rather than two crates
// away at a use site: the library never names `Sync` itself, so without this
// an unguarded `Cell` added to `Backend` would surface only where a backend
// is installed.
#[cfg(feature = "critical-section")]
const _: () = {
    // The supertrait is the assertion: the `impl` is accepted only if
    // `Backend<D>` really is `Sync`, checked here rather than at a call.
    #[expect(dead_code, reason = "a compile-time assertion has no callers")]
    trait IsSync: Sync {}
    impl<D: DrawTarget + Send> IsSync for Backend<D> where D::Color: Sync {}
};

impl<D: DrawTarget> Backend<D> {
    /// A backend over `display`, painting ink and background as `palette`
    /// says, with the default metrics, English labels and a reader's key row.
    pub fn new(display: D, palette: Palette<D::Color>) -> Self {
        let bounds = display.bounding_box();
        Backend {
            frame: Guarded::new(Frame {
                display: Some(display),
                size: Size::new(bounds.size.width as i32, bounds.size.height as i32),
                clip: None,
                input: InputState::default(),
            }),
            palette,
            fonts: Guarded::new(Fonts::DEFAULT),
            metrics: Metrics::DEFAULT,
            labels: Labels::ENGLISH,
            keys: KeyRow::READER,
            millis: AtomicU32::new(0),
            dirty: AtomicBool::new(true),
            left_right_keys: false,
        }
    }

    /// The words this backend paints hints with.
    pub fn labels(&self) -> &Labels {
        &self.labels
    }

    /// What the keys along the device's bottom edge mean.
    pub fn keys(&self) -> &KeyRow {
        &self.keys
    }

    /// Says the device has a Left/Right pair.
    ///
    /// `false` until called, the safe direction: a control told the pair
    /// exists when it does not cannot be changed by any key.
    pub fn with_left_right_keys(mut self, present: bool) -> Self {
        self.left_right_keys = present;
        self
    }

    /// Whether it was told the device has a Left/Right pair.
    pub fn left_right_keys(&self) -> bool {
        self.left_right_keys
    }

    /// The words its hint bar shows.
    ///
    /// English until an application says so.
    pub fn with_labels(mut self, labels: Labels) -> Self {
        self.labels = labels;
        self
    }

    /// What the keys along the device's bottom edge mean, left to right.
    pub fn with_keys(mut self, keys: KeyRow) -> Self {
        self.keys = keys;
        self
    }

    /// Paints chrome to these measurements instead of the default.
    pub fn with_metrics(mut self, metrics: Metrics) -> Self {
        self.metrics = metrics;
        self
    }

    /// Sets the type the framework's roles resolve through: a family, and the
    /// size each role wants.
    pub fn with_fonts(self, fonts: Fonts) -> Self {
        self.fonts.with(|current| *current = fonts);
        self
    }

    /// The type in use, as [`with_fonts`](Backend::with_fonts) or a switch
    /// left it.
    pub fn fonts(&self) -> Fonts {
        self.fonts.with(|fonts| *fonts)
    }

    /// The measurements every chrome painter lays out against.
    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    /// Sets the type in another family, and asks for a repaint.
    ///
    /// The sizes are re-derived from the chrome, not carried over: each role
    /// keeps the line height these metrics called for and the new family
    /// answers with the tier it was cut in, so a coarser ladder still lands
    /// inside the rows laid out. That replaces a [`Fonts`] given to
    /// [`with_fonts`](Backend::with_fonts), which is for a backend nothing
    /// will re-set. Every id every role reports changes with it — an id is a
    /// hash of the bytes — so anything keyed on one is invalidated unasked.
    pub fn set_family(&self, family: &'static Family) {
        let next = Fonts::for_metrics(&self.metrics).with_family(family);
        self.fonts.with(|fonts| *fonts = next);
        // Both flags: `dirty` is for a caller driving its own loop,
        // `request_update` is what `App` consults. `dirty` first, because
        // `request_update` reaches this same backend through the installed
        // host — without a guard held, since `dirty` is an atomic.
        self.dirty.store(true, Ordering::Relaxed);
        xpui::host::request_update();
    }

    /// Leaks a backend a caller has finished building.
    ///
    /// [`leak`](Backend::leak) constructs and leaks in one step, which leaves
    /// nowhere to put the builders. This is the other order: build it, then
    /// give it the `'static` life the installed host needs.
    pub fn leaked(self) -> &'static Self
    where
        D: 'static,
    {
        alloc::boxed::Box::leak(alloc::boxed::Box::new(self))
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

    /// Reports `button` as pressed this frame.
    pub fn press(&self, button: Button) {
        self.input(|state| state.press(button));
    }

    /// Reports `button` as released this frame.
    pub fn release(&self, button: Button) {
        self.input(|state| state.release(button));
    }

    /// Reports a completed tap at `at`.
    pub fn tap(&self, at: Point) {
        self.input(|state| state.tap(at));
    }

    /// Reports a completed swipe.
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

    /// Marks the pending repaint as pushed; call after presenting the frame.
    pub fn clear_dirty(&self) {
        self.dirty.store(false, Ordering::Relaxed);
    }
}
