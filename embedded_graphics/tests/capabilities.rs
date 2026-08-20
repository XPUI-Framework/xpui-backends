//! What this backend answers about the device it is driving.
//!
//! Not what it paints: what it *says the hardware has*. A control asks before
//! deciding how a value can be changed, so a wrong answer here is a control
//! that cannot be changed at all, or one that promises keys the board does not
//! carry — and neither shows up in a framebuffer.

use xpui::host::InputSource;
use xpui_boards::Board;
use xpui_eg::Framebuffer as TestDisplay;
use xpui_eg::{Backend, Palette};

fn display() -> TestDisplay {
    TestDisplay::new(200, 120)
}

/// The backend reports what the board carries, board by board.
///
/// Walking `Board::ALL` rather than a chosen one: this is the per-board
/// regression that is easiest to ship and hardest to see, because the suite is
/// green and the board you looked at is right.
///
/// Deliberately compared against the board's own answer rather than a second
/// hand-written table. What is being checked here is the *wiring* — that a
/// backend built for a board asks it — and `crates/boards` is where the answers
/// themselves are pinned per board and argued for.
#[test]
fn a_backend_built_for_a_board_answers_for_that_board() {
    let mut with_pair = 0;
    let mut without = 0;

    for board in Board::ALL {
        let backend = Backend::for_board(display(), board, Palette::INK_IS_ON);
        assert_eq!(
            backend.has_left_right_keys(),
            board.has_left_right_keys(),
            "{} answers something its board does not",
            board.name
        );
        if board.has_left_right_keys() {
            with_pair += 1;
        } else {
            without += 1;
        }
    }

    // A backend hard-wired to either constant would satisfy the loop above on
    // whichever boards happened to agree with it. Both answers have to occur,
    // or this test is only checking one of them.
    //
    // How many boards give each answer is `crates/boards`' to assert, and it
    // does. Repeating the count here would put the census in two crates, which
    // is the drift this method was added to end.
    assert!(
        with_pair > 0 && without > 0,
        "every board now answers the same way, so this no longer proves the \
         backend asks rather than guesses"
    );
}

/// A backend with no board says the device has no pair.
///
/// `Backend::new` is handed a display and a palette — no board — so there is
/// nothing to derive the answer from. `false` is the safe direction rather than
/// the accurate one: a control told the pair exists when it does not cannot be
/// changed by any key, while one told it does not exist can still be entered
/// and left.
#[test]
fn a_backend_with_no_board_promises_no_keys() {
    let backend = Backend::new(display(), Palette::INK_IS_ON);

    assert!(
        !backend.has_left_right_keys(),
        "a backend with no board cannot promise keys nobody described"
    );
    assert!(
        backend.board().is_none(),
        "and it is the missing board that makes it so, not a stored flag"
    );
}
