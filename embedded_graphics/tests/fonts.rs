//! Which face each role resolves to, for a given piece of chrome.
//!
//! The rule is small and the consequences are not: a face taller than a list
//! row overprints the row below, and a face two sizes too small is a label
//! nobody on a 200-ppi panel can read. `physical.rs` checks what that comes to
//! in millimetres; this checks the rule itself.
//!
//! Almost every figure here is taken through the backend's own `TextMetrics`,
//! which is how the framework asks. What it cannot answer is asked of the faces
//! directly: the height of any one *style*, because `line_height` takes no
//! style and answers with the tier's whole band — so a test that went through
//! the metrics could not tell a bold that overflows that band from one that
//! fits.

use xpui::host::{FontRole, FontStyle, TextMetrics};
use xpui_chrome::Tokens;
use xpui_eg::{Backend, FontRenderer, Fonts, Palette};
use xpui_screenshot::Framebuffer;

/// A string that puts every kind of glyph in front of the face: caps,
/// descenders, digits, a space and an accent. Two different faces cannot
/// measure it identically.
const SAMPLE: &str = "Hamburgefonstiv 123 éî";

/// What a face measures: its line height, and the width of [`SAMPLE`].
///
/// The pair is a fingerprint. Height alone cannot tell a regular face from its
/// own bold — they are deliberately the same height — and width alone cannot
/// tell two sizes of the same face apart if one is condensed.
fn metrics(fonts: Fonts, role: FontRole, style: FontStyle) -> (i32, i32) {
    let backend = Backend::new(Framebuffer::new(8, 8), Palette::INK_IS_ON).with_fonts(fonts);
    let font = backend.font(role);
    (
        backend.line_height(font),
        backend.text_width(font, SAMPLE, style),
    )
}

fn line_height(fonts: Fonts, role: FontRole) -> i32 {
    metrics(fonts, role, FontStyle::Regular).0
}

/// A `Tokens` that differs from the default in one number, so a case says only
/// what it varies.
fn with_row_height(row: i32) -> Tokens {
    Tokens {
        list_row_height: row,
        ..Tokens::DEFAULT
    }
}

fn ui_height(tokens: &Tokens) -> i32 {
    line_height(Fonts::for_tokens(tokens), FontRole::Ui)
}

/// Every family this backend ships. A caller may register its own, and the
/// same rules apply to those — `examples/gallery` asserts them for the two it
/// adds.
const FAMILIES: [&xpui_eg::Family; 1] = [&xpui_eg::HELVETICA];

const PRESETS: [(&str, Fonts); 4] = [
    ("SMALL", Fonts::SMALL),
    ("COMPACT", Fonts::COMPACT),
    ("DEFAULT", Fonts::DEFAULT),
    ("LARGE", Fonts::LARGE),
];

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
        for role in [FontRole::Ui, FontRole::UiSmall, FontRole::Reader] {
            assert_eq!(
                metrics(chosen, role, FontStyle::Regular),
                metrics(expected, role, FontStyle::Regular),
                "{name}: {role:?} is not the face this preset was written with"
            );
        }
    }
}

/// Scaling the chrome up is what carries a board's scale into the type, and
/// both faces have to move. If only the secondary one did, a scaled board
/// would be more space around the same letters.
#[test]
fn a_board_that_scales_its_chrome_gets_larger_type() {
    let plain = Fonts::for_tokens(&Tokens::DEFAULT);
    let scaled = Fonts::for_tokens(&Tokens::DEFAULT.scaled(120));

    for role in [FontRole::Ui, FontRole::UiSmall] {
        assert!(
            line_height(scaled, role) > line_height(plain, role),
            "a touch board's chrome selected the same {}px {role:?} face as a \
             button board's",
            line_height(plain, role)
        );
    }
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
        let fonts = Fonts::for_tokens(&with_row_height(row));
        let ui = line_height(fonts, FontRole::Ui);
        assert!(
            ui < row,
            "a {row}px row chose a {ui}px face, which leaves no row to put it in"
        );
        assert!(
            line_height(fonts, FontRole::UiSmall) <= ui,
            "the small face is not smaller than the interface face at {row}px"
        );
    }
}

/// `ui_bold` and `ui_small_bold` name a weight, and every preset answers with a
/// face that has it. A sub-header drawn in the regular face looks like
/// sloppiness rather than a missing feature, which is why it is worth a test.
///
/// Measured rather than compared by identity: a real bold is wider than its
/// regular at the same height, and the regular face renamed is not.
#[test]
fn every_preset_has_a_real_bold() {
    for (name, fonts) in PRESETS {
        for role in [FontRole::Ui, FontRole::UiSmall] {
            let regular = metrics(fonts, role, FontStyle::Regular).1;
            let bold = metrics(fonts, role, FontStyle::Bold).1;
            assert!(
                bold > regular,
                "{name}: the bold {role:?} face measures {bold} against the \
                 regular's {regular} — it is the regular one wearing the name"
            );
        }
    }
}

/// `TextMetrics::line_height` takes no style, so one band has to hold every
/// style a heading might be drawn in. A bold taller than the band it is
/// measured against spills that difference into whatever sits below it.
///
/// Asked of the faces rather than through `line_height`, which answers with
/// the tier whatever style it is handed — so a version of this test that went
/// through the metrics could not fail.
#[test]
fn no_style_is_taller_than_the_band_it_is_painted_into() {
    for (name, fonts) in PRESETS {
        for role in [FontRole::Ui, FontRole::UiSmall, FontRole::Reader] {
            let tier = fonts.tier(role);
            for (style, face) in styles(tier) {
                assert!(
                    face.get_default_line_height() as i32 <= tier.line_height,
                    "{name}: the {style} {role:?} face is {}px in a {}px band",
                    face.get_default_line_height(),
                    tier.line_height
                );
            }
        }
    }
}

/// Every tier declares its own band, because tiers are picked in `const`
/// context and u8g2 only answers at run time. A declared height above the
/// tallest face wastes a row; one below it overprints the row beneath. It has
/// to be exactly the tallest.
#[test]
fn every_tier_declares_the_band_its_tallest_style_needs() {
    for family in FAMILIES {
        for tier in family.tiers {
            let tallest = styles(tier)
                .into_iter()
                .map(|(_, face)| face.get_default_line_height() as i32)
                .max()
                .unwrap_or(0);
            assert_eq!(
                tier.line_height, tallest,
                "{}: a tier says it is {}px and its tallest face is {tallest}px \
                 — every preset is chosen by that number",
                family.name, tier.line_height
            );
        }
    }
}

/// Every style a tier actually carries, named.
fn styles(tier: &'static xpui_eg::Tier) -> Vec<(&'static str, &'static FontRenderer)> {
    let mut found = vec![("regular", tier.regular), ("bold", tier.bold)];
    if let Some(face) = tier.italic {
        found.push(("italic", face));
    }
    if let Some(face) = tier.bold_italic {
        found.push(("bold italic", face));
    }
    found
}

/// A family answers with what it was cut in, so a chrome asking for a height
/// between two tiers gets the nearer one rather than the first or the last.
#[test]
fn a_family_answers_with_its_nearest_cut() {
    let family = &xpui_eg::HELVETICA;

    assert_eq!(
        family.tier_for(30).line_height,
        30,
        "an exact size is itself"
    );
    assert_eq!(
        family.tier_for(1).line_height,
        14,
        "below the whole ladder is its bottom rung"
    );
    assert_eq!(
        family.tier_for(1_000).line_height,
        39,
        "above it is its top rung"
    );
    assert_eq!(
        family.tier_for(23).line_height,
        21,
        "23 is two below 21 and seven below 30 — the nearer cut, not the next \
         one up, which would leave the row over-set"
    );
    // A genuine tie: 16 is two from 14 and two from 18. The documented answer
    // is the smaller — the cut that certainly fits the row that asked for it.
    let ladder: Vec<i32> = family.tiers.iter().map(|tier| tier.line_height).collect();
    assert!(
        ladder.contains(&14) && ladder.contains(&18),
        "this case needs 14 and 18 to be adjacent cuts: {ladder:?}"
    );
    assert_eq!(
        family.tier_for(16).line_height,
        14,
        "a height exactly between two cuts takes the smaller, which is the one \
         that certainly fits the row that asked"
    );
    assert_eq!(
        family.tier_for(26).line_height,
        30,
        "26 is five above 21 and four below 30 — nearest either way, so a size \
         just under a cut reaches up to it rather than dropping a whole step"
    );
}

/// The reading face is what a page of prose is set in, and it is asked for
/// separately from the interface face precisely so it can differ. At the top
/// tier the family runs out and the two meet — asserted here, so that a larger
/// face arriving is a test failure rather than something nobody notices.
#[test]
fn the_reading_face_is_larger_than_the_interface_face() {
    for (name, fonts) in PRESETS {
        let reader = line_height(fonts, FontRole::Reader);
        let ui = line_height(fonts, FontRole::Ui);
        if name == "LARGE" {
            assert_eq!(
                reader, ui,
                "the top of the family is no longer the ceiling for both roles \
                 — give the reading face the larger one and say so in \
                 `Fonts::LARGE`"
            );
        } else {
            assert!(
                reader > ui,
                "{name}: a page of prose is set in the same {ui}px face as a \
                 button label"
            );
        }
    }
}
