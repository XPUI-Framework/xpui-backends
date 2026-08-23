//! What this backend answers about the device it is driving.
//!
//! Not what it paints: what it *says the hardware has*. A control asks before
//! deciding how a value can be changed, so a wrong answer here is a control
//! that cannot be changed at all, or one that promises keys the board does not
//! carry — and neither shows up in a framebuffer.
//!
//! The backend stores what it was told and nothing more. Whether the caller
//! told it the truth about a particular device is the caller's test, and it
//! lives in the conformance suite: `xpui-gallery`'s `gallery/tests/capabilities.rs`.

use xpui::host::InputSource;
use xpui_eg::{Backend, Palette};
use xpui_screenshot::Framebuffer as TestDisplay;

fn display() -> TestDisplay {
    TestDisplay::new(200, 120)
}

/// A backend nobody told promises no keys.
///
/// `Backend::new` is handed a display and a palette — nothing describes the
/// hardware — so there is nothing to answer from. `false` is the safe
/// direction rather than the accurate one: a control told the pair exists when
/// it does not cannot be changed by any key, while one told it does not exist
/// can still be entered and left. One is unusable; the other costs a
/// keystroke.
#[test]
fn a_backend_nobody_told_promises_no_keys() {
    let backend = Backend::new(display(), Palette::INK_IS_ON);

    assert!(
        !backend.has_left_right_keys(),
        "a backend nobody described the hardware to cannot promise keys"
    );
}

/// And it is the stored answer that decides, not a constant.
///
/// Without this the test above passes against a backend hard-wired to `false`,
/// which would lose the pair on every board that has it with nothing to
/// notice.
#[test]
fn the_stored_answer_is_what_is_reported() {
    let told = Backend::new(display(), Palette::INK_IS_ON).with_left_right_keys(true);
    assert!(
        told.has_left_right_keys(),
        "what it was told is what it says"
    );

    let told_not = Backend::new(display(), Palette::INK_IS_ON).with_left_right_keys(false);
    assert!(!told_not.has_left_right_keys());
}
