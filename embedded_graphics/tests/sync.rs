//! What the `critical-section` feature actually buys.
//!
//! ```bash
//! cargo test -p xpui-embedded-graphics --features critical-section
//! ```
//!
//! `xpui::host::Host` requires `Sync`, and without the feature this backend
//! only claims it — the state sits behind a `RefCell`, whose borrow flag is a
//! non-atomic counter, so two threads can both take `borrow_mut` and end up
//! with aliasing `&mut D`.
//!
//! # Why this measures occupancy rather than watching it break
//!
//! The obvious test is to hammer an unguarded backend from four threads and
//! assert that it falls over. Measured, it does — but **only in a debug
//! build**: the same code passes a release run, because the race is undefined
//! behaviour and the optimiser is entitled to make it disappear. An assertion
//! whose passing depends on observing UB is flaky in both directions and has
//! no business in a gate.
//!
//! So this measures the guarantee instead, from outside, through a
//! `DrawTarget` that counts how many threads are inside it at once. Everything
//! the backend does to the display happens while the guard is held, so peak
//! occupancy is exactly what the guard controls — and the counters are atomic,
//! so nothing here is a race whatever the answer turns out to be.
//!
//! Nothing in production is instrumented for it.

#![cfg(feature = "critical-section")]

use std::sync::atomic::{AtomicUsize, Ordering};

use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use xpui::host::{Canvas, InputSource};
use xpui::{Button, Rect};
use xpui_eg::{Backend, Palette};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;

/// How many threads are inside the display right now, and the most there have
/// ever been.
static INSIDE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

/// A display that answers every draw by recording how crowded it was.
///
/// The work in the middle is not decoration: a guard is only observably a
/// guard if the window it protects is wide enough for a second thread to
/// arrive during it.
struct Counting;

impl Counting {
    fn enter() {
        let now = INSIDE.fetch_add(1, Ordering::SeqCst) + 1;
        PEAK.fetch_max(now, Ordering::SeqCst);
        for _ in 0..2_000 {
            std::hint::black_box(0u32);
        }
        INSIDE.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Dimensions for Counting {
    fn bounding_box(&self) -> Rectangle {
        Self::enter();
        Rectangle::new(Point::zero(), Size::new(WIDTH, HEIGHT))
    }
}

impl DrawTarget for Counting {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        Self::enter();
        // Drained rather than ignored, so the backend's own iterator work
        // happens inside the guard where a real one's would.
        for _pixel in pixels {}
        Ok(())
    }

    fn fill_solid(&mut self, _area: &Rectangle, _colour: Self::Color) -> Result<(), Self::Error> {
        Self::enter();
        Ok(())
    }
}

/// Four threads drawing and reading input at once, which is what `Host: Sync`
/// says is allowed.
///
/// **Break `Guarded::with` to check this test still works**: drop the
/// `critical_section::with` and leave the `RefCell`, and it goes red — either
/// on a peak above one, or on the borrow panic that beats it to it.
#[test]
fn concurrent_access_never_overlaps() {
    let backend: &'static Backend<Counting> = Backend::leak(Counting, Palette::INK_IS_ON);

    PEAK.store(0, Ordering::SeqCst);
    INSIDE.store(0, Ordering::SeqCst);

    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(move || {
                for _ in 0..200 {
                    // One of each kind of access: a fill, a pixel iterator, a
                    // read of the display's own bounds, and the input state —
                    // all of which reach the same guarded frame.
                    backend.fill_rect(Rect::new(0, 0, 16, 16), true);
                    backend.scrim(Rect::new(0, 0, 16, 16));
                    let _ = backend.screen_size();
                    let _ = backend.was_pressed(Button::Confirm);
                }
            });
        }
    });

    assert_eq!(
        PEAK.load(Ordering::SeqCst),
        1,
        "two threads were inside the display at once, so the guard is not \
         serialising: `Host: Sync` is a claim this backend does not keep"
    );
}
