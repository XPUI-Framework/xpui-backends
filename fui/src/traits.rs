//! Metrics, input and the clock: what the backend forwards rather than draws.

use xpui::host::{Clock, FontId, FontRole, FontStyle, InputSource, TextMetrics};
use xpui::{Button, Point, SwipeDir};

use crate::backend::Backend;
use crate::marshal::as_c;
use crate::platform::Platform;
use crate::raw;

impl<P: Platform> TextMetrics for Backend<P> {
    fn font(&self, role: FontRole) -> FontId {
        FontId(raw::xpui_fui_font(match role {
            FontRole::Ui => 0,
            FontRole::UiSmall => 1,
            FontRole::Reader => 2,
        }))
    }

    fn text_width(&self, font: FontId, text: &str, style: FontStyle) -> i32 {
        if !font.is_available() {
            return 0;
        }
        let text = as_c(text);
        // Safety: `text` is NUL-terminated and outlives the call.
        unsafe { raw::xpui_fui_text_width(font.0, text.as_ptr().cast(), style as u8) }
    }

    fn line_height(&self, font: FontId) -> i32 {
        if !font.is_available() {
            return 0;
        }
        raw::xpui_fui_line_height(font.0)
    }
}

impl<P: Platform> InputSource for Backend<P> {
    fn was_pressed(&self, button: Button) -> bool {
        self.platform.was_pressed(button)
    }

    fn is_pressed(&self, button: Button) -> bool {
        self.platform.is_pressed(button)
    }

    fn was_released(&self, button: Button) -> bool {
        self.platform.was_released(button)
    }

    fn has_touch(&self) -> bool {
        self.platform.has_touch()
    }

    fn has_left_right_keys(&self) -> bool {
        self.platform.has_left_right_keys()
    }

    fn tap(&self) -> Option<Point> {
        self.platform.tap()
    }

    fn touch_held(&self) -> Option<Point> {
        self.platform.touch_held()
    }

    fn touch_released(&self) -> bool {
        self.platform.touch_released()
    }

    fn swipe(&self) -> SwipeDir {
        self.platform.swipe()
    }

    fn was_back_gesture(&self) -> bool {
        self.platform.was_back_gesture()
    }

    fn was_home_gesture(&self) -> bool {
        self.platform.was_home_gesture()
    }

    fn swipe_moves_selection(&self) -> bool {
        self.platform.swipe_moves_selection()
    }
}

impl<P: Platform> Clock for Backend<P> {
    fn millis(&self) -> u32 {
        self.platform.millis()
    }
}
