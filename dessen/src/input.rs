//! Input: a dessen event, turned into the frames `xpui` polls.
//!
//! Dessen hands an app one finished event at a time (a button, a tap, a long
//! press, a swipe), where `xpui` polls a snapshot every frame and times holds
//! itself. So one event becomes one frame, except a long press: `xpui` fires
//! one when a finger stays down past its threshold, so it becomes three held
//! frames a quarter second apart, then the lift. The lift completes a tap
//! where the finger was, as a real one does; `xpui` swallows it after a hold
//! that fired, and reads it as a tap where nothing answered the hold.
//!
//! The clock those frames read is the backend's own, advanced only here. A
//! wall clock would see the four frames as instantaneous, and `xpui` would
//! never cross its threshold.

use dessen::{
    DSN_EV_BACK, DSN_EV_CONFIRM, DSN_EV_LONG_PRESS, DSN_EV_NEXT, DSN_EV_PREV, DSN_EV_SIDEWAYS,
    DSN_EV_SWIPE_DOWN, DSN_EV_SWIPE_LEFT, DSN_EV_SWIPE_RIGHT, DSN_EV_SWIPE_UP, DSN_EV_TAP, ev_type,
    ev_x, ev_y,
};
use xpui::host::{Clock, InputSource};
use xpui::{Button, Point, SwipeDir};

use crate::Backend;

/// The step between the held frames of a long press, in milliseconds.
///
/// Two steps reach `xpui`'s 500 ms threshold; each stays under its 500 ms
/// guard against a loop that went blind, so the hold reads as continuous.
pub const HOLD_STEP_MS: u32 = 250;

/// How far the clock moves before each event, in milliseconds.
///
/// Further than any of `xpui`'s guards, so no hold or repeat is ever credited
/// across two separate events.
pub const EVENT_GAP_MS: u32 = 1000;

/// One frame of input, as [`InputSource`] answers it.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Frame {
    /// The button pressed and released this frame.
    pub button: Option<Button>,
    /// A completed tap.
    pub tap: Option<Point>,
    /// A finger resting here.
    pub held: Option<Point>,
    /// The finger lifted this frame.
    pub released: bool,
    /// A swipe this frame.
    pub swipe: SwipeDir,
}

/// The frames one dessen event becomes, and how many of the four are used.
///
/// An event this backend does not know becomes no frames, rather than a
/// frame of nothing that would still advance `xpui`'s state.
pub fn frames(event: u32) -> ([Frame; 4], usize) {
    let mut out = [Frame::default(); 4];
    let point = Point::new(ev_x(event), ev_y(event));
    let button = |b| Frame {
        button: Some(b),
        ..Frame::default()
    };
    let swipe = |s| Frame {
        swipe: s,
        ..Frame::default()
    };
    let lift = Frame {
        tap: Some(point),
        released: true,
        ..Frame::default()
    };
    let sideways = event & DSN_EV_SIDEWAYS != 0;
    let len = match ev_type(event) {
        DSN_EV_CONFIRM => put(&mut out, &[button(Button::Confirm)]),
        DSN_EV_BACK => put(&mut out, &[button(Button::Back)]),
        // Left and Right arrive as Prev and Next marked sideways; a firmware
        // that does not mark them sends every key as Up or Down.
        DSN_EV_PREV if sideways => put(&mut out, &[button(Button::Left)]),
        DSN_EV_NEXT if sideways => put(&mut out, &[button(Button::Right)]),
        DSN_EV_PREV => put(&mut out, &[button(Button::Up)]),
        DSN_EV_NEXT => put(&mut out, &[button(Button::Down)]),
        DSN_EV_TAP => put(&mut out, &[lift]),
        DSN_EV_LONG_PRESS => {
            let held = Frame {
                held: Some(point),
                ..Frame::default()
            };
            put(&mut out, &[held, held, held, lift])
        }
        DSN_EV_SWIPE_UP => put(&mut out, &[swipe(SwipeDir::Up)]),
        DSN_EV_SWIPE_DOWN => put(&mut out, &[swipe(SwipeDir::Down)]),
        DSN_EV_SWIPE_LEFT => put(&mut out, &[swipe(SwipeDir::Left)]),
        DSN_EV_SWIPE_RIGHT => put(&mut out, &[swipe(SwipeDir::Right)]),
        _ => 0,
    };
    (out, len)
}

fn put(out: &mut [Frame; 4], frames: &[Frame]) -> usize {
    out[..frames.len()].copy_from_slice(frames);
    frames.len()
}

impl Backend {
    /// Makes `frame` the input `xpui` sees, advancing the clock by `step` ms.
    pub(crate) fn set_frame(&self, frame: Frame, step: u32) {
        self.clock.set(self.clock.get().wrapping_add(step));
        if matches!(frame.button, Some(Button::Left | Button::Right)) {
            self.sideways.set(true);
        }
        self.frame.set(frame);
    }

    fn frame(&self) -> Frame {
        self.frame.get()
    }
}

impl InputSource for Backend {
    fn was_pressed(&self, button: Button) -> bool {
        self.frame().button == Some(button)
    }

    fn is_pressed(&self, _button: Button) -> bool {
        false // a dessen button event is a completed press: nothing stays down
    }

    fn was_released(&self, button: Button) -> bool {
        self.frame().button == Some(button)
    }

    fn has_touch(&self) -> bool {
        self.device.has_touch
    }

    fn has_left_right_keys(&self) -> bool {
        // Not knowable up front: true from the first sideways key, so a
        // firmware that never sends one keeps Confirm opening value controls.
        self.sideways.get()
    }

    fn tap(&self) -> Option<Point> {
        self.frame().tap
    }

    fn touch_held(&self) -> Option<Point> {
        self.frame().held
    }

    fn touch_released(&self) -> bool {
        self.frame().released
    }

    fn swipe(&self) -> SwipeDir {
        self.frame().swipe
    }

    fn was_back_gesture(&self) -> bool {
        false // the firmware keeps the back gesture: holding Back leaves the app
    }

    fn was_home_gesture(&self) -> bool {
        false
    }
}

impl Clock for Backend {
    fn millis(&self) -> u32 {
        self.clock.get()
    }
}
