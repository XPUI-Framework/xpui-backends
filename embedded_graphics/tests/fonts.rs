//! Which face each role resolves to, for a given piece of chrome.
//!
//! The rule is small and the consequences are not: a face taller than a list
//! row overprints the row below, and a face two sizes too small is a label
//! nobody on a 200-ppi panel can read. `physical.rs` checks what that comes to
//! in millimetres; this checks the rule itself.

use xpui_chrome::Tokens;
use xpui_eg::Fonts;

/// A `Tokens` that differs from the default in one number, so a case says only
/// what it varies.
fn with_row_height(row: i32) -> Tokens {
    Tokens {
        list_row_height: row,
        ..Tokens::DEFAULT
    }
}

fn ui_height(tokens: &Tokens) -> i32 {
    Fonts::for_tokens(tokens).ui.character_size.height as i32
}

/// Each preset is the answer for the chrome it was written for. A preset
/// nothing selects is a preset that has quietly stopped being used.
#[test]
fn every_preset_is_what_its_own_chrome_selects() {
    for (name, tokens, expected) in [
        ("SMALL", Tokens::SMALL, Fonts::SMALL),
        ("COMPACT", Tokens::COMPACT, Fonts::COMPACT),
        ("DEFAULT", Tokens::DEFAULT, Fonts::DEFAULT),
        (
            "DEFAULT scaled for touch",
            Tokens::DEFAULT.scaled(120),
            Fonts::LARGE,
        ),
    ] {
        let chosen = Fonts::for_tokens(&tokens);
        assert_eq!(
            chosen.ui.character_size, expected.ui.character_size,
            "{name}: chose a {:?} interface face",
            chosen.ui.character_size
        );
        assert_eq!(
            chosen.ui_small.character_size, expected.ui_small.character_size,
            "{name}: chose a {:?} small face",
            chosen.ui_small.character_size
        );
    }
}

/// Scaling the chrome up is what carries a board's scale into the type — as
/// far as this font set allows.
///
/// The interface face is already the largest one at 1.0, because a reader's
/// panel needs it there; what a touch board's 1.2 can still buy is the
/// secondary text. If that stopped moving too, a scaled board would be nothing
/// but more space around the same letters.
#[test]
fn a_board_that_scales_its_chrome_gets_larger_secondary_type() {
    let plain = Fonts::for_tokens(&Tokens::DEFAULT);
    let scaled = Fonts::for_tokens(&Tokens::DEFAULT.scaled(120));

    assert!(
        scaled.ui_small.character_size.height > plain.ui_small.character_size.height,
        "a touch board's chrome selected the same {}px small face as a button board's",
        plain.ui_small.character_size.height
    );
    assert_eq!(
        scaled.ui.character_size, plain.ui.character_size,
        "the interface face is the same at both scales only because 10x20 is \
         the largest `embedded-graphics` ships. If a larger one has arrived, \
         this is where a scaled board should spend it"
    );
}

/// The ladder only ever goes up. A dip means some panel gets smaller type than
/// a smaller panel does, which is the kind of thing nobody finds by reading.
#[test]
fn a_taller_row_never_gets_shorter_type() {
    let mut previous = 0;
    for row in 8..=80 {
        let height = ui_height(&with_row_height(row));
        assert!(
            height >= previous,
            "a {row}px row chose a {height}px face after a shorter row chose {previous}px"
        );
        previous = height;
    }
}

/// Type has to fit the row it is painted into, whatever the row.
#[test]
fn every_step_of_the_ladder_fits_its_own_row() {
    for row in 20..=80 {
        let tokens = with_row_height(row);
        let fonts = Fonts::for_tokens(&tokens);
        let ui = fonts.ui.character_size.height as i32;
        assert!(
            ui < row,
            "a {row}px row chose a {ui}px face, which leaves no row to put it in"
        );
        assert!(
            (fonts.ui_small.character_size.height as i32) <= ui,
            "the small face is not smaller than the interface face at {row}px"
        );
    }
}

/// `ui_small_bold` and `ui_bold` name a weight, and the two largest sets have a
/// real face for it. The two smallest do not — `embedded-graphics` ships no
/// bold under 6x13 — and that is recorded rather than hidden, because a
/// sub-header that silently renders as body text looks like sloppiness.
#[test]
fn the_larger_sets_have_a_real_bold() {
    for (name, fonts) in [("DEFAULT", Fonts::DEFAULT), ("LARGE", Fonts::LARGE)] {
        assert!(
            !core::ptr::eq(fonts.ui_small, fonts.ui_small_bold),
            "{name}: the small bold face is the regular one wearing its name"
        );
        assert!(
            !core::ptr::eq(fonts.ui, fonts.ui_bold),
            "{name}: the bold interface face is the regular one wearing its name"
        );
    }

    for (name, fonts) in [("COMPACT", Fonts::COMPACT), ("SMALL", Fonts::SMALL)] {
        assert!(
            core::ptr::eq(fonts.ui_small, fonts.ui_small_bold),
            "{name}: there is now a bold face at this size — say so in the doc \
             comment that claims there is not"
        );
    }
}
