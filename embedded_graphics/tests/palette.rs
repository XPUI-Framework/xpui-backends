//! Which `BinaryColor` is ink is a per-driver decision, and getting it
//! backwards inverts the whole panel with no error anywhere — both polarities
//! compile and both are plausible. The named constants exist so the call site
//! states the convention instead of the enum variant; these tests prove the
//! names describe what actually reaches the display.

use std::sync::{Mutex, MutexGuard};

use embedded_graphics::pixelcolor::BinaryColor;
use xpui::host::Canvas;
use xpui::{Rect, Renderer, Size};
use xpui_eg::{Backend, Palette};
use xpui_screenshot::Framebuffer;

/// These tests install a global host, so they cannot overlap. Without this two
/// of them race on the backend's `RefCell` and one panics "already borrowed".
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Fills the whole panel with ink and reports how many pixels came back set.
fn set_pixels_when_filled_with_ink(palette: Palette<BinaryColor>) -> usize {
    let _guard = serial();
    let backend = Backend::leak(Framebuffer::new(16, 16), palette);
    // Safety: serialised by `SERIAL`, and this backend has not rendered yet.
    unsafe { xpui::host::install(backend) };

    Renderer::clear();
    Renderer::fill_rect(Rect::new(0, 0, 16, 16), true);
    backend.with_display(|f| f.ink_count())
}

#[test]
fn ink_is_on_sets_every_pixel() {
    assert_eq!(
        set_pixels_when_filled_with_ink(Palette::INK_IS_ON),
        16 * 16,
        "INK_IS_ON must reach the display as `On`"
    );
}

#[test]
fn ink_is_off_sets_none() {
    assert_eq!(
        set_pixels_when_filled_with_ink(Palette::INK_IS_OFF),
        0,
        "INK_IS_OFF must reach the display as `Off` — this is the polarity the \
         Badger 2040's uc8151 needs, and swapping it inverts the panel silently"
    );
}

#[test]
fn the_two_constants_are_genuinely_opposite() {
    assert_eq!(Palette::INK_IS_ON.ink, Palette::INK_IS_OFF.background);
    assert_eq!(Palette::INK_IS_ON.background, Palette::INK_IS_OFF.ink);
}

/// A frame loop that holds only a `&Backend` still needs the board's numbers —
/// `refresh_ms` above all, which decides how often presenting is worth it.
/// Without this it keeps a second copy that can drift from the first.
#[test]
fn a_board_backend_remembers_which_board_it_is() {
    let backend = Backend::new(Framebuffer::new(16, 16), Palette::INK_IS_ON);
    assert_eq!(backend.board(), None, "`new` is given a size, not a board");

    let badger = xpui_eg::Board::BADGER_2040;
    let backend = Backend::for_board(Framebuffer::new(16, 16), badger, Palette::INK_IS_OFF);
    let recorded = backend.board().expect("`for_board` was given one");
    assert_eq!(recorded, badger);
    assert_eq!(
        recorded.refresh_ms, badger.refresh_ms,
        "the loop reads its cadence from here"
    );

    // The board supplies chrome, not size.
    assert_eq!(
        backend.screen_size(),
        Size::new(16, 16),
        "the size is the display's, whatever the board says"
    );
    assert_ne!(
        (badger.width, badger.height),
        (16, 16),
        "or this asserts nothing"
    );
}
