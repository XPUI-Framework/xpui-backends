//! One frame of input, as the application feeds it in.
//!
//! `embedded_graphics` knows nothing about buttons or fingers — it is a
//! drawing library. So the events come from whatever is driving the backend: a
//! desktop window, a GPIO poll, an interrupt handler. This is the buffer
//! between them and [`InputSource`](xpui::host::InputSource).
//!
//! Most of what is here is *edge* state: true for exactly one frame, cleared
//! by [`InputState::begin_frame`]. That matches what the framework expects —
//! a button reported as pressed on every frame re-fires whatever it is on.
//! What `begin_frame` leaves alone is the held set, the finger's position
//! until it lifts, and `swipe_moves_selection`, which is a setting.

use xpui::{Button, Point, SwipeDir};

/// How many buttons [`Button`] has, so the held-set is a fixed array rather
/// than an allocation.
///
/// A hand-maintained duplicate of the enum's variant count, which is exactly
/// the kind of thing that rots. [`index`] clamps rather than trusting it, so a
/// variant added upstream is a button that quietly does nothing instead of an
/// out-of-bounds panic on the first press.
const BUTTONS: usize = 16;

fn index(button: Button) -> usize {
    debug_assert!(
        (button as usize) < BUTTONS,
        "xpui gained a Button variant; raise BUTTONS to match"
    );
    (button as usize).min(BUTTONS - 1)
}

/// One frame of input, as the event source writes it and the framework reads
/// it.
///
/// Every event here is an edge, cleared by `begin_frame`; what survives it is
/// the held set, `touch_at` until `touch_up`, and `swipe_moves_selection`.
#[derive(Default)]
pub struct InputState {
    pressed: [bool; BUTTONS],
    released: [bool; BUTTONS],
    held: [bool; BUTTONS],
    tap: Option<Point>,
    touch_at: Option<Point>,
    touch_released: bool,
    swipe: SwipeDir,
    back_gesture: bool,
    home_gesture: bool,
    /// Whether a swipe up should move focus up rather than dragging content.
    pub swipe_moves_selection: bool,
}

impl InputState {
    /// Clears everything that lasts one frame.
    ///
    /// Held buttons survive; that is the difference between "is down" and "went down".
    pub fn begin_frame(&mut self) {
        self.pressed = [false; BUTTONS];
        self.released = [false; BUTTONS];
        self.tap = None;
        self.touch_released = false;
        self.swipe = SwipeDir::None;
        self.back_gesture = false;
        self.home_gesture = false;
    }

    /// A button went down: reports both the edge and the held state.
    pub fn press(&mut self, button: Button) {
        self.pressed[index(button)] = true;
        self.held[index(button)] = true;
    }

    /// A button came up.
    pub fn release(&mut self, button: Button) {
        self.released[index(button)] = true;
        self.held[index(button)] = false;
    }

    /// A completed tap, at the position the finger went down.
    pub fn tap(&mut self, at: Point) {
        self.tap = Some(at);
    }

    /// A finger is down at `at`.
    ///
    /// Reported every frame it stays down, which is the signal a slider drag needs.
    pub fn touch_down(&mut self, at: Point) {
        self.touch_at = Some(at);
    }

    /// The finger came up.
    pub fn touch_up(&mut self) {
        self.touch_at = None;
        self.touch_released = true;
    }

    /// A completed swipe in `direction`.
    pub fn swipe(&mut self, direction: SwipeDir) {
        self.swipe = direction;
    }

    /// The system back gesture, this frame.
    pub fn back_gesture(&mut self) {
        self.back_gesture = true;
    }

    /// The system home gesture, this frame.
    pub fn home_gesture(&mut self) {
        self.home_gesture = true;
    }

    // -- what the framework reads ------------------------------------------

    pub(crate) fn was_pressed(&self, button: Button) -> bool {
        self.pressed[index(button)]
    }

    pub(crate) fn is_pressed(&self, button: Button) -> bool {
        self.held[index(button)]
    }

    pub(crate) fn was_released(&self, button: Button) -> bool {
        self.released[index(button)]
    }

    pub(crate) fn has_touch(&self) -> bool {
        self.tap.is_some() || self.touch_at.is_some() || self.touch_released
    }

    pub(crate) fn tap_at(&self) -> Option<Point> {
        self.tap
    }

    pub(crate) fn touch_held(&self) -> Option<Point> {
        self.touch_at
    }

    pub(crate) fn touch_was_released(&self) -> bool {
        self.touch_released
    }

    pub(crate) fn swipe_direction(&self) -> SwipeDir {
        self.swipe
    }

    pub(crate) fn was_back_gesture(&self) -> bool {
        self.back_gesture
    }

    pub(crate) fn was_home_gesture(&self) -> bool {
        self.home_gesture
    }
}
