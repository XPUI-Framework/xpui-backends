//! What FreeInkUI has no opinion about, and a firmware therefore supplies.

use xpui::{Button, Point, SwipeDir};

/// Buttons, gestures and the clock, which FreeInkUI has no opinion about.
///
/// A drawing library cannot tell you whether a button was pressed. Whatever is
/// driving the panel already knows — a firmware's input manager, a GPIO poll —
/// so it implements this and the backend forwards to it.
pub trait Platform: Sync {
    /// Milliseconds since some fixed point; only differences are read.
    fn millis(&self) -> u32;

    /// Whether `button` went down this frame.
    fn was_pressed(&self, button: Button) -> bool;
    /// Whether `button` is down, this frame included.
    fn is_pressed(&self, button: Button) -> bool;
    /// Whether `button` came up this frame.
    fn was_released(&self, button: Button) -> bool;

    /// Whether the device has a Left/Right pair to nudge a value with; see
    /// [`InputSource::has_left_right_keys`](xpui::host::InputSource::has_left_right_keys).
    ///
    /// **Required, unlike everything below it**: no answer is safe to inherit,
    /// and only the firmware knows which keys its device carries. The
    /// `xpui-boards-*` crates answer it for every board described there, as
    /// `Board::has_left_right_keys`; a firmware describing the same device
    /// differently is a disagreement nothing here can detect.
    fn has_left_right_keys(&self) -> bool;

    /// Whether *this frame* carries a touch — not whether the device has a
    /// touchscreen.
    ///
    /// See
    /// [`InputSource::has_touch`](xpui::host::InputSource::has_touch); the
    /// method above is the one that describes the hardware.
    fn has_touch(&self) -> bool {
        false
    }
    /// A completed tap, at the position the finger went down.
    fn tap(&self) -> Option<Point> {
        None
    }
    /// Where the finger is while it is down.
    fn touch_held(&self) -> Option<Point> {
        None
    }
    /// Whether a finger lifted this frame.
    fn touch_released(&self) -> bool {
        false
    }
    /// A completed swipe, or [`SwipeDir::None`].
    fn swipe(&self) -> SwipeDir {
        SwipeDir::None
    }
    /// The system back gesture.
    fn was_back_gesture(&self) -> bool {
        false
    }
    /// The system home gesture, offered to the screen before the host acts.
    fn was_home_gesture(&self) -> bool {
        false
    }
    /// Whether a swipe moves focus rather than dragging content; see
    /// [`InputSource::swipe_moves_selection`](xpui::host::InputSource::swipe_moves_selection).
    fn swipe_moves_selection(&self) -> bool {
        false
    }
}

/// A platform with no input at all, for a panel that only ever displays.
pub struct NoInput;

impl Platform for NoInput {
    fn millis(&self) -> u32 {
        0
    }
    fn was_pressed(&self, _button: Button) -> bool {
        false
    }
    fn is_pressed(&self, _button: Button) -> bool {
        false
    }
    fn was_released(&self, _button: Button) -> bool {
        false
    }
    fn has_left_right_keys(&self) -> bool {
        false
    }
}
