//! What the chrome measures on the glass.
//!
//! Every other test in this crate counts pixels, and a pixel is not a size.
//! The same 40-pixel row is 4.6mm on an X4 and 3.9mm on an X3 because one
//! panel is denser than the other; the firmware's own board table says a 30px
//! row at these densities "is only ~3mm", which is where a finger stops
//! finding it.
//!
//! So these convert through each panel's ppi and assert millimetres. A change
//! that leaves the pixel numbers alone and moves the chrome onto a denser
//! panel still fails here, and so does one that drops a board's UI scale —
//! neither of which a pixel comparison can notice.

use std::sync::{Mutex, MutexGuard};

use xpui::Font;
use xpui_eg::{Backend, Board, Framebuffer, Palette};

/// Floors, in tenths of a millimetre.
///
/// Each is a claim about a hand and an eye, not about this code:
///
/// - **A list row, anywhere.** Above the ~3mm the firmware calls too small,
///   with enough margin that the densest panel here still clears it.
/// - **A list row you tap.** On a touch board the row *is* the target, so it
///   answers to the finger rather than to the eye.
/// - **A touch target.** Between the 5mm of a fingertip and the 7mm every
///   phone platform asks for. The touch boards reach it only because of their
///   UI scale: at the button-era 44px both would fall short.
/// - **A line of body text.** Above the 3mm a row is judged by, on every panel
///   here. The binding case is the X3: at 257 ppi it is the densest panel this
///   backend drives, and the 30-pixel face it shares with the X4 is 3.4mm
///   there and 2.9mm here. On the X4 itself — the panel the firmware's own
///   figure was measured on — the same face is the firmware's 3.4mm.
///
/// - **A line of body text on a reader.** What the firmware achieves from a
///   29-pixel line, asserted on the panel it was measured on rather than
///   averaged across a Badger. This is the one that fails if the type ever
///   goes back to being chosen by what a font set happens to have.
const ROW_FLOOR: i32 = 35;
const TOUCH_ROW_FLOOR: i32 = 50;
const TOUCH_TARGET_FLOOR: i32 = 55;
const LINE_FLOOR: i32 = 29;
const READER_LINE_FLOOR: i32 = 34;

/// The installed host is process-wide, so the font cases take turns.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Tenths of a millimetre, for a panel that has been measured.
fn tenths(board: Board, pixels: i32) -> i32 {
    board
        .tenths_of_a_mm(pixels)
        .unwrap_or_else(|| panic!("{}: no diagonal, so nothing here can be judged", board.name))
}

/// A millimetre figure a person can read in a failure message.
fn mm(tenths: i32) -> String {
    format!("{}.{}mm", tenths / 10, tenths % 10)
}

/// The line height this backend hands out for body text on `board`.
///
/// Measured through the installed host rather than read off a constant: the
/// point is what a screen would actually be painted with.
fn body_line_px(board: Board) -> i32 {
    let backend = Backend::leak_for_board(
        Framebuffer::new(board.width, board.height),
        board,
        Palette::INK_IS_ON,
    );
    // Safety: serialised by `SERIAL`, and nothing has rendered on this one.
    unsafe { xpui::host::install(backend) };
    Font::ui().line_height()
}

#[test]
fn a_list_row_can_be_read_on_every_panel() {
    for board in Board::ALL {
        let row = tenths(board, board.tokens.list_row_height);
        assert!(
            row >= ROW_FLOOR,
            "{}: a {}px row on a {}-ppi panel is {}, under the {} floor",
            board.name,
            board.tokens.list_row_height,
            board.ppi().unwrap_or(0),
            mm(row),
            mm(ROW_FLOOR)
        );
    }
}

#[test]
fn a_row_on_a_touch_board_is_big_enough_to_tap() {
    for board in Board::ALL.into_iter().filter(|board| board.touch) {
        let row = tenths(board, board.tokens.list_row_height);
        assert!(
            row >= TOUCH_ROW_FLOOR,
            "{}: a row is the tap target on a touch board, and {} is under \
             the {} floor",
            board.name,
            mm(row),
            mm(TOUCH_ROW_FLOOR)
        );
    }
}

#[test]
fn a_touch_target_is_the_size_of_a_finger() {
    for board in Board::ALL.into_iter().filter(|board| board.touch) {
        let target = tenths(board, board.tokens.min_touch_size);
        assert!(
            target >= TOUCH_TARGET_FLOOR,
            "{}: the smallest touch target is {} ({}px at {} ppi, scale {}%), \
             under the {} floor — a board's UI scale is what carries it there",
            board.name,
            mm(target),
            board.tokens.min_touch_size,
            board.ppi().unwrap_or(0),
            board.ui_scale_percent,
            mm(TOUCH_TARGET_FLOOR)
        );
    }
}

#[test]
fn body_text_can_be_read_on_every_panel() {
    let _guard = serial();

    for board in Board::ALL {
        let line = tenths(board, body_line_px(board));
        assert!(
            line >= LINE_FLOOR,
            "{}: a line of body text is {} on a {}-ppi panel, under the {} floor",
            board.name,
            mm(line),
            board.ppi().unwrap_or(0),
            mm(LINE_FLOOR)
        );
    }
}

/// What a line of body text measures on the panels this framework was written
/// beside.
///
/// The firmware puts 29 pixels on an X4 and calls it 3.4mm. This is the same
/// claim, made of the readers rather than of every panel: a Badger's 111 ppi
/// would carry a floor that a 218-ppi reader fails, which is how body text
/// ends up sized by what a font set happens to ship.
#[test]
fn a_reader_gets_the_body_text_the_firmware_gets() {
    let _guard = serial();

    for board in [Board::X4, Board::X4_PRO, Board::STICKY] {
        let line = tenths(board, body_line_px(board));
        assert!(
            line >= READER_LINE_FLOOR,
            "{}: a line of body text is {} on a {}-ppi reader, under the {} \
             floor the firmware reaches",
            board.name,
            mm(line),
            board.ppi().unwrap_or(0),
            mm(READER_LINE_FLOOR)
        );
    }
}

/// The type has to fit the chrome that contains it, or a row's label overprints
/// the row below and a hint runs into the panel edge.
#[test]
fn the_type_fits_the_chrome_it_is_painted_into() {
    let _guard = serial();

    for board in Board::ALL {
        let line = body_line_px(board);
        assert!(
            line < board.tokens.list_row_height,
            "{}: a {line}px line does not fit a {}px row",
            board.name,
            board.tokens.list_row_height
        );
        // A band of zero is a board that draws no hints at all — its Back and
        // Confirm come from the touchscreen, so there is no row of keys to
        // label. Nothing is painted there, so there is nothing to fit; the
        // check would otherwise read a missing band as one that is too small.
        if board.tokens.button_hints_height == 0 {
            continue;
        }
        assert!(
            Font::ui_small().line_height() <= board.tokens.button_hints_height,
            "{}: the hint band is {}px and its type is {}px",
            board.name,
            board.tokens.button_hints_height,
            Font::ui_small().line_height()
        );
    }
}

/// A touch board's chrome has to be visibly bigger than the same panel's with
/// buttons, or the scale is a number nobody can see.
///
/// The X4 and the X4 Pro are the same panel — same pixels, same inches — and
/// differ only in whether a finger drives them, which makes the pair the one
/// honest measurement of what the scale buys.
#[test]
fn the_same_panel_gets_bigger_chrome_when_a_finger_drives_it() {
    let buttons = tenths(Board::X4, Board::X4.tokens.list_row_height);
    let finger = tenths(Board::X4_PRO, Board::X4_PRO.tokens.list_row_height);

    assert_eq!(
        Board::X4.ppi(),
        Board::X4_PRO.ppi(),
        "this comparison only means something while they share a panel"
    );
    assert!(
        finger >= buttons + 5,
        "the touch board's row is {}, the button board's {} — half a \
         millimetre apart is not a difference anybody can see",
        mm(finger),
        mm(buttons)
    );
}

/// What each panel actually measures — printed, because a floor tells a
/// reviewer nothing about how much room is left above it:
///
/// ```bash
/// cargo test -p xpui-embedded-graphics --test physical -- --nocapture
/// ```
///
/// And checked, because a printed table nobody can disagree with is
/// decoration. Every figure above comes out of integer arithmetic — the
/// conversion is a `const fn` on targets with no FPU — so it is compared here
/// against the same sum in floating point, which is the one place a rounding
/// mistake in it would show.
#[test]
fn every_figure_here_survives_a_second_derivation() {
    let _guard = serial();

    println!(
        "\n{:<14} {:>5} {:>7} {:>9} {:>9} {:>9}",
        "board", "ppi", "scale", "row", "touch", "line"
    );
    for board in Board::ALL {
        let line = body_line_px(board);
        let ppi = board.ppi().expect("a measured panel");

        println!(
            "{:<14} {:>5} {:>6}% {:>9} {:>9} {:>9}",
            board.slug,
            ppi,
            board.ui_scale_percent,
            mm(tenths(board, board.tokens.list_row_height)),
            mm(tenths(board, board.tokens.min_touch_size)),
            format!("{} ({line}px)", mm(tenths(board, line))),
        );

        for pixels in [
            board.tokens.list_row_height,
            board.tokens.min_touch_size,
            line,
        ] {
            let float = pixels as f32 * 254.0 / ppi as f32;
            let integer = tenths(board, pixels) as f32;
            assert!(
                (float - integer).abs() < 1.0,
                "{}: {pixels}px at {ppi} ppi is {float:.2} tenths of a millimetre, \
                 and the integer path says {integer}",
                board.name
            );
        }
    }
}
