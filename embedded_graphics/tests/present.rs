//! Presenting a frame from a driver whose flush suspends.
//!
//! `with_display` holds the guard for as long as its closure runs, so a flush
//! that `await`s cannot go inside one — a borrow held across a suspension
//! point is a second task arriving to find the state already borrowed, and
//! under this workspace's `panic = "abort"` that takes the firmware down. That
//! rules out more of the ecosystem than it sounds like: every DMA-backed
//! panel, where the whole point is to hand the transfer to hardware and do
//! something else until it completes.
//!
//! So the display **leaves** the backend for the duration, and these tests are
//! about what that costs and what it must not cost.
//!
//! # Why the future is driven by hand
//!
//! `Waker::noop` and a poll loop, rather than an executor in
//! dev-dependencies. What the tests need is not concurrency but the ability to
//! stop *between* polls and look at the backend while the flush is suspended —
//! which is exactly the moment a held borrow would be observable, and exactly
//! what an executor hides.

use core::future::Future;
use core::pin::pin;
use core::task::{Context, Poll, Waker};

use xpui::Rect;
use xpui::host::Canvas;
use xpui_eg::{Backend, Palette};
use xpui_screenshot::Framebuffer;

const WIDTH: i32 = 32;
const HEIGHT: i32 = 32;

// -- the smallest thing that can drive a future -----------------------------

/// Polls to completion, counting the polls.
///
/// The count is the assertion that matters: a fake driver that completes on
/// its first poll never suspended, so a test built on one would prove nothing
/// about holding a borrow across a suspension that did not happen.
///
/// Spins rather than sleeping, because the waker does nothing: a future that
/// never becomes ready hangs this rather than failing it.
fn block_on<F: Future>(future: F) -> (F::Output, u32) {
    let mut context = Context::from_waker(Waker::noop());
    let mut future = pin!(future);
    let mut polls = 0;
    loop {
        polls += 1;
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return (value, polls);
        }
    }
}

/// Suspends once, the way a driver waiting on DMA or a BUSY pin does.
async fn yield_once() {
    let mut yielded = false;
    core::future::poll_fn(|context| {
        if yielded {
            return Poll::Ready(());
        }
        yielded = true;
        context.waker().wake_by_ref();
        Poll::Pending
    })
    .await;
}

fn backend() -> &'static Backend<Framebuffer> {
    Backend::leak(Framebuffer::new(WIDTH, HEIGHT), Palette::INK_IS_ON)
}

// -- the tests --------------------------------------------------------------

/// The shape a firmware writes: take the display, flush it across an `await`,
/// and let the loan put it back.
#[test]
fn an_async_flush_completes_and_the_display_comes_back() {
    let backend = backend();
    backend.fill_rect(Rect::new(0, 0, WIDTH, HEIGHT), true);

    let (flushed, polls) = block_on(async {
        let loan = backend
            .loan_display()
            .expect("nothing else holds the display");
        // Whatever a real driver does here — a DMA transfer, a BUSY pin —
        // happens with no borrow of the backend outstanding.
        yield_once().await;
        loan.ink_count()
    });

    assert!(
        polls > 1,
        "the flush never suspended, so this proves nothing about awaiting: \
         {polls} poll(s)"
    );
    assert_eq!(flushed, (WIDTH * HEIGHT) as usize, "the loan saw the frame");

    // Back where it belongs, and drawable again.
    backend.fill_rect(Rect::new(0, 0, WIDTH, HEIGHT), false);
    assert_eq!(backend.with_display(|display| display.ink_count()), 0);
}

/// **The acceptance criterion.** While the flush is suspended, the backend
/// must be reachable — that is what "no borrow held across an `await`" means,
/// and it is only observable between polls.
///
/// Written against the guard on purpose: put the `await` inside
/// `with_display`'s closure instead and this cannot even be expressed, which
/// is the point. Reach into the backend from here while a borrow is held and
/// it panics, exactly as a second task would on device.
#[test]
fn the_backend_is_not_borrowed_while_a_flush_is_in_flight() {
    let backend = backend();

    let mut context = Context::from_waker(Waker::noop());
    let mut present = pin!(async {
        let _loan = backend
            .loan_display()
            .expect("nothing else holds the display");
        yield_once().await;
    });

    // One poll: the loan is taken and the flush suspends.
    assert!(present.as_mut().poll(&mut context).is_pending());

    // Mid-flush. None of this may panic, and none of it may deadlock.
    assert_eq!(
        backend.screen_size(),
        xpui::Size::new(WIDTH, HEIGHT),
        "the panel's size is still answerable while the display is out — a \
         layout measured against zero collapses without saying so"
    );
    backend.fill_rect(Rect::new(0, 0, 8, 8), true);
    assert!(backend.loan_display().is_none(), "two loans at once");

    // And it finishes.
    assert!(present.as_mut().poll(&mut context).is_ready());

    // What was painted mid-flush went nowhere, which is the documented cost.
    assert_eq!(
        backend.with_display(|display| display.ink_count()),
        0,
        "a paint arriving while the display was on loan must be discarded, \
         not applied to a panel that had already been handed away"
    );
}

/// A second loan is refused rather than handing out a second `&mut D`.
#[test]
fn two_presents_at_once_are_refused() {
    let backend = backend();
    let first = backend.loan_display().expect("the first loan");
    assert!(backend.loan_display().is_none());
    drop(first);
    assert!(
        backend.loan_display().is_some(),
        "the loan puts the display back on drop"
    );
}

/// The blocking path is unchanged for the callers that have it.
#[test]
fn the_blocking_path_still_works() {
    let backend = backend();
    backend.fill_rect(Rect::new(0, 0, WIDTH, HEIGHT), true);
    assert_eq!(
        backend.with_display(|display| display.ink_count()),
        (WIDTH * HEIGHT) as usize
    );
}
