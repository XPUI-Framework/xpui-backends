//! What the backend answers about the device, and where the answer comes from.
//!
//! FreeInkUI draws; it has no idea which keys a device carries. The firmware
//! does, and says so through [`Platform`] — so the only thing this backend can
//! get wrong is failing to pass the answer on. A backend that returned a
//! constant instead would look right on every device that happened to agree
//! with it, which is why both directions are pinned rather than one.

use xpui::host::InputSource;
use xpui::{Button, Point, SwipeDir};
use xpui_fui::{Backend, Platform};

/// A firmware that answers whatever it was built with, and nothing else.
struct Device {
    left_right: bool,
}

impl Platform for Device {
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
        self.left_right
    }
}

static WITH_PAIR: Device = Device { left_right: true };
static WITHOUT_PAIR: Device = Device { left_right: false };

/// The backend reports what the firmware said, both ways round.
///
/// The four boards CrossPoint drives disagree — the X3's and the X4's footers
/// send Left and Right, while the X4 Pro takes them from its touchscreen and
/// the Sticky spends its three keys on confirm and a page pair — so a backend
/// answering a constant would be wrong on two of them whichever constant it
/// picked, and no test that drove only one answer would notice.
#[test]
fn the_backend_reports_what_the_firmware_says() {
    let with_pair: Backend<Device> = Backend::new(&WITH_PAIR);
    let without_pair: Backend<Device> = Backend::new(&WITHOUT_PAIR);

    assert!(
        with_pair.has_left_right_keys(),
        "a firmware that says its device has the pair must be believed"
    );
    assert!(
        !without_pair.has_left_right_keys(),
        "and so must one that says it does not"
    );
}

/// A panel that only displays reports no keys.
///
/// `NoInput` is for a device with nothing wired to it at all, so the answer is
/// not a default it inherited — there is no default — but a statement that
/// there is nothing there.
#[test]
fn a_panel_with_no_input_has_no_keys() {
    static NONE: xpui_fui::NoInput = xpui_fui::NoInput;
    let backend: Backend<xpui_fui::NoInput> = Backend::new(&NONE);

    assert!(!backend.has_left_right_keys());
    assert!(!backend.has_touch());
    assert_eq!(backend.tap(), None::<Point>);
    assert_eq!(backend.swipe(), SwipeDir::None);
}
