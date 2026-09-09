//! Everything a host owes `xpui` besides painting: metrics, chrome, input and
//! the clock.

use embedded_graphics::prelude::*;

use xpui::host::{Clock, FontId, FontRole, FontStyle, InputSource, TextMetrics};
use xpui::{Button, Point, SwipeDir};

use crate::backend::Backend;
use crate::fonts;

impl<D: DrawTarget> TextMetrics for Backend<D> {
    fn font(&self, role: FontRole) -> FontId {
        self.fonts().id(role)
    }

    fn text_width(&self, font: FontId, text: &str, style: FontStyle) -> i32 {
        fonts::text_width(self.fonts().face(font, style), text)
    }

    /// The tier's own band, which is its tallest style.
    ///
    /// Not the regular face's height: this is asked without a style, and a
    /// bold taller than its regular would then paint outside the band it was
    /// given. See [`Tier::line_height`](crate::Tier::line_height).
    fn line_height(&self, font: FontId) -> i32 {
        self.fonts().face(font, FontStyle::Regular).tier.line_height
    }
}

impl<D: DrawTarget> InputSource for Backend<D> {
    fn was_pressed(&self, button: Button) -> bool {
        self.frame.with_ref(|frame| frame.input.was_pressed(button))
    }

    fn is_pressed(&self, button: Button) -> bool {
        self.frame.with_ref(|frame| frame.input.is_pressed(button))
    }

    fn was_released(&self, button: Button) -> bool {
        self.frame
            .with_ref(|frame| frame.input.was_released(button))
    }

    fn has_touch(&self) -> bool {
        self.frame.with_ref(|frame| frame.input.has_touch())
    }

    /// What the caller said with
    /// [`with_left_right_keys`](crate::Backend::with_left_right_keys), rather
    /// than what the frame carries: this describes the device.
    fn has_left_right_keys(&self) -> bool {
        self.left_right_keys
    }

    fn tap(&self) -> Option<Point> {
        self.frame.with_ref(|frame| frame.input.tap_at())
    }

    fn touch_held(&self) -> Option<Point> {
        self.frame.with_ref(|frame| frame.input.touch_held())
    }

    fn touch_released(&self) -> bool {
        self.frame
            .with_ref(|frame| frame.input.touch_was_released())
    }

    fn swipe(&self) -> SwipeDir {
        self.frame.with_ref(|frame| frame.input.swipe_direction())
    }

    fn was_back_gesture(&self) -> bool {
        self.frame.with_ref(|frame| frame.input.was_back_gesture())
    }

    fn was_home_gesture(&self) -> bool {
        self.frame.with_ref(|frame| frame.input.was_home_gesture())
    }

    fn swipe_moves_selection(&self) -> bool {
        self.frame
            .with_ref(|frame| frame.input.swipe_moves_selection)
    }
}

impl<D: DrawTarget> Clock for Backend<D> {
    fn millis(&self) -> u32 {
        self.millis.load(core::sync::atomic::Ordering::Relaxed)
    }
}

xpui_chrome::plain_chrome! {
    generic: [D: DrawTarget],
    for Backend<D>,
    // The backend's own, not globals: two backends in one process may be
    // driving two different panels, in two different languages, with two
    // different key rows.
    metrics: |backend| &backend.metrics,
    labels: |backend| &backend.labels,
    keys: |backend| &backend.keys,
    request_update: |backend| backend.dirty.store(true, core::sync::atomic::Ordering::Relaxed),
}

/// Lets `xpui`'s UI harness feed this backend input.
///
/// The methods already exist as inherent ones; this names them through a trait
/// so the harness can drive any backend without knowing which it has.
#[cfg(feature = "testing")]
impl<D: DrawTarget> xpui::testing::Drive for Backend<D> {
    fn begin(&self, millis: u32) {
        self.begin_frame(millis);
    }

    fn inject_press(&self, button: Button) {
        self.press(button);
    }

    fn inject_release(&self, button: Button) {
        self.release(button);
    }

    fn inject_tap(&self, point: Point) {
        self.tap(point);
    }

    fn inject_swipe(&self, direction: SwipeDir) {
        self.swipe(direction);
    }
}
