//! What FreeInkUI has no opinion about, and a firmware therefore supplies.

use xpui::{Button, Point, SwipeDir};

/// Buttons, gestures and the clock, which FreeInkUI has no opinion about.
///
/// A drawing library cannot tell you whether a button was pressed. Whatever is
/// driving the panel already knows — a firmware's input manager, a GPIO poll —
/// so it implements this and the backend forwards to it.
pub trait Platform: Sync {
    fn millis(&self) -> u32;

    fn was_pressed(&self, button: Button) -> bool;
    fn is_pressed(&self, button: Button) -> bool;
    fn was_released(&self, button: Button) -> bool;

    fn has_touch(&self) -> bool {
        false
    }
    fn tap(&self) -> Option<Point> {
        None
    }
    fn touch_held(&self) -> Option<Point> {
        None
    }
    fn touch_released(&self) -> bool {
        false
    }
    fn swipe(&self) -> SwipeDir {
        SwipeDir::None
    }
    fn was_back_gesture(&self) -> bool {
        false
    }
    fn was_home_gesture(&self) -> bool {
        false
    }
    /// See [`InputSource::swipe_moves_selection`](xpui::host::InputSource::swipe_moves_selection).
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
}
