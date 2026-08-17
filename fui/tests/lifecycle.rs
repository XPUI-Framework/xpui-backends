//! Driving a screen the way a C++ host does: through an opaque handle.
//!
//! `marshalling.rs` checks what crosses the boundary going out. This checks the
//! boundary coming *in* — the six entry points a host calls, and the handle
//! they all take. Nothing here can be checked by the compiler: on the C side
//! every one of them is a `void*`, so a double-box unwrapped once is a wild
//! pointer rather than a type error.
//!
//! The probe reports through statics rather than by being read back out of the
//! handle, because a host only ever holds the `void*` — and reaching into it
//! from a test would mean asserting a layout the compiler does not promise.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard};

use xpui::screen::{Driver, Runtime, Screen};
use xpui::{NavigationScreen, Text, View};
use xpui_fui::lifecycle::{
    handle_for, into_handle, reclaim, xpui_screen_destroy, xpui_screen_home_gesture,
    xpui_screen_loop, xpui_screen_on_enter, xpui_screen_on_exit, xpui_screen_render,
};
use xpui_fui::testing::{self, Call};
use xpui_fui::{Backend, NoInput};

static PLATFORM: NoInput = NoInput;
static BACKEND: Backend<NoInput> = Backend::new(&PLATFORM);

static ENTERED: AtomicU32 = AtomicU32::new(0);
static EXITED: AtomicU32 = AtomicU32::new(0);
static FRAMES: AtomicU32 = AtomicU32::new(0);
static DROPPED: AtomicU32 = AtomicU32::new(0);

fn count(counter: &AtomicU32) -> u32 {
    counter.load(Ordering::Relaxed)
}

/// The host is process-wide and so are the counters, so a test that installs
/// one runs alone.
static SERIAL: Mutex<()> = Mutex::new(());

fn install() -> MutexGuard<'static, ()> {
    let guard = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // Safety: serialised by `SERIAL`, and no frame is in flight.
    unsafe { xpui::host::install(&BACKEND) };
    testing::reset();
    for counter in [&ENTERED, &EXITED, &FRAMES, &DROPPED] {
        counter.store(0, Ordering::Relaxed);
    }
    guard
}

/// A screen that records which lifecycle calls reached it.
struct Probe {
    /// What `handle_home_gesture` answers, so both branches can be driven.
    claims_home: bool,
}

impl Drop for Probe {
    fn drop(&mut self) {
        DROPPED.fetch_add(1, Ordering::Relaxed);
    }
}

impl Screen for Probe {
    type Message = ();

    fn body(&self) -> impl View<Self::Message> {
        NavigationScreen::new(Text::new("probe")).title("Probe")
    }

    fn update(&mut self, _message: Self::Message) {}

    fn tick(&mut self) {
        FRAMES.fetch_add(1, Ordering::Relaxed);
    }

    fn on_enter(&mut self) {
        ENTERED.fetch_add(1, Ordering::Relaxed);
    }

    fn on_exit(&mut self) {
        EXITED.fetch_add(1, Ordering::Relaxed);
    }

    fn handle_home_gesture(&mut self) -> bool {
        self.claims_home
    }

    fn title(&self) -> Option<&'static str> {
        Some("Probe")
    }
}

#[test]
fn every_entry_point_reaches_the_screen() {
    let _guard = install();

    let handle = into_handle(Probe { claims_home: true });

    // Safety: `handle` is live for all of these, and destroyed exactly once.
    unsafe {
        xpui_screen_on_enter(handle);
        assert_eq!(count(&ENTERED), 1, "on_enter did not reach the screen");

        xpui_screen_loop(handle);
        assert_eq!(count(&FRAMES), 1, "loop did not tick the screen");

        testing::reset();
        xpui_screen_render(handle);
        assert!(
            testing::calls().contains(&Call::Clear),
            "render drew nothing through the backend: {:?}",
            testing::calls()
        );

        assert_eq!(
            xpui_screen_home_gesture(handle),
            1,
            "a screen that claims the home gesture must answer non-zero"
        );

        xpui_screen_on_exit(handle);
        assert_eq!(count(&EXITED), 1, "on_exit did not reach the screen");

        assert_eq!(count(&DROPPED), 0, "nothing was freed before destroy");
        xpui_screen_destroy(handle);
        assert_eq!(count(&DROPPED), 1, "destroy did not free the screen");
    }
}

/// A screen that does not want the gesture must be distinguishable from one
/// that does, or a host can never tell whether to apply its own meaning.
#[test]
fn a_screen_can_decline_the_home_gesture() {
    let _guard = install();

    let handle = into_handle(Probe { claims_home: false });

    // Safety: a live handle, destroyed once.
    unsafe {
        assert_eq!(xpui_screen_home_gesture(handle), 0);
        xpui_screen_destroy(handle);
    }
}

/// Every entry point takes NULL, because a host whose factory failed has
/// nothing else to pass and must not have to special-case each call.
#[test]
fn null_is_survivable_everywhere() {
    let _guard = install();
    let null = core::ptr::null_mut();

    // Safety: null is explicitly allowed by every one of these.
    unsafe {
        xpui_screen_on_enter(null);
        xpui_screen_loop(null);
        xpui_screen_render(null);
        assert_eq!(xpui_screen_home_gesture(null), 0);
        xpui_screen_on_exit(null);
        xpui_screen_destroy(null);
        assert!(reclaim(null).is_none());
    }

    assert_eq!(count(&ENTERED) + count(&EXITED) + count(&FRAMES), 0);
}

/// The handle a host declines must come back as the screen it was, still
/// alive: that is what lets `Navigator::present` hand the screen back rather
/// than leak it or silently swallow it.
#[test]
fn a_declined_handle_reclaims_the_screen() {
    let _guard = install();

    let erased: Box<dyn Driver> = Box::new(Runtime::new(Probe { claims_home: false }));
    let handle = handle_for(erased);

    // Safety: a live handle, reclaimed exactly once.
    let mut screen = unsafe { reclaim(handle) }.expect("a non-null handle holds a screen");
    assert_eq!(
        count(&DROPPED),
        0,
        "reclaim must hand the screen back alive, not drop it"
    );

    // Still drivable, which is the point of handing it back.
    screen.on_enter();
    assert_eq!(count(&ENTERED), 1, "the reclaimed screen still runs");

    drop(screen);
    assert_eq!(
        count(&DROPPED),
        1,
        "the reclaimed screen is owned by whoever took it"
    );
}
