//! A typeface, in the sizes it was packaged in.
//!
//! A bitmap family has the sizes it has: nothing is scaled at run time, so
//! "18pt" is one of a handful of tiers the foundry cut, and asking for a
//! height between two of them gets the nearer one.
//!
//! **The id is a hash of the bytes**, not a slot number: two builds that ship
//! the same face agree on its id, and a face swapped for different bytes gets
//! a different one. A consumer keys a cache on it, so a stable id over
//! changed bytes serves the old face with nothing to notice. [`font_tier!`]
//! hashes the regular and the bold; a tier assembled by hand with an italic
//! hashes that too. [`Fonts::id`](super::Fonts::id) folds the family's name
//! in, so the same tier reached through two families is two ids.

use u8g2_fonts::{FontRenderer, fonts as u8g2};
use xpui::host::{FontId, FontStyle};

/// One size of one family: four styles, and the height of a line of it.
pub struct Tier {
    /// A hash of every byte of every style below.
    ///
    /// Stored rather than computed on demand: it is asked for on every
    /// measurement, and hashing forty kilobytes of bitmaps per label would
    /// cost more than laying the label out. `const`, so it costs nothing at
    /// run time and nothing in RAM — see [`font_id`].
    pub id: FontId,
    /// The band a line of this tier is painted into.
    ///
    /// **The tallest style, not the regular one.** A family's bold is often a
    /// pixel or two taller than its regular — Courier's 18 is 25 and 27 — and
    /// the framework asks for a line height without saying which style it is
    /// about to draw. Sizing the band to the regular puts those extra rows
    /// into whatever sits below.
    ///
    /// Declared rather than asked, because tiers are chosen in `const` context
    /// and u8g2 answers at run time. `tests/fonts.rs` asserts every one of
    /// these against the faces themselves, so a wrong number is a failing test
    /// rather than a layout that is quietly a pixel out.
    pub line_height: i32,
    /// The upright face.
    pub regular: &'static FontRenderer,
    /// The bold face.
    pub bold: &'static FontRenderer,
    /// `None` when the family was never cut in it — u8g2's Helvetica was not.
    /// Asking for italic then gets the regular face, which is the honest
    /// answer: a slanted approximation is a different typeface.
    pub italic: Option<&'static FontRenderer>,
    /// `None` when the family was never cut in it; bold is drawn instead.
    pub bold_italic: Option<&'static FontRenderer>,
}

impl Tier {
    /// The face for a style, falling back to the nearest cut that exists.
    pub fn face(&self, style: FontStyle) -> &'static FontRenderer {
        match style {
            FontStyle::Regular => self.regular,
            FontStyle::Bold => self.bold,
            FontStyle::Italic => self.italic.unwrap_or(self.regular),
            FontStyle::BoldItalic => self.bold_italic.unwrap_or(self.bold),
        }
    }
}

/// FNV-1a over every byte of every style in a tier.
///
/// A `while` loop rather than a `for`, because this runs in `const` context
/// and iterators do not. Chosen for being four lines rather than for its
/// distribution: this hashes a handful of faces per build, and the only
/// property that matters is that different bytes give different answers.
pub const fn font_id(styles: &[&[u8]]) -> FontId {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut style = 0;
    while style < styles.len() {
        let bytes = styles[style];
        let mut at = 0;
        while at < bytes.len() {
            hash ^= bytes[at] as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            at += 1;
        }
        style += 1;
    }
    // Folded to a positive `i32`, because that is what a `FontId` is and it
    // crosses an FFI boundary as one. Zero is reserved — the framework reads
    // it as "this build ships no such font" — so it becomes one.
    let folded = (((hash >> 32) ^ hash) as u32 & 0x7fff_ffff) as i32;
    FontId(if folded == 0 { 1 } else { folded })
}

/// A typeface, in ascending size order.
pub struct Family {
    /// What a picker shows. Not an identity — the id is the bytes.
    pub name: &'static str,
    /// Ascending by `line_height`. [`tier_for`](Family::tier_for) relies on it.
    pub tiers: &'static [Tier],
    /// Where a glyph this family lacks is looked for.
    ///
    /// `None` ends the chain; what is missing then draws a marker rather than
    /// nothing, because a label with a hole in it reads as a rendering bug and
    /// a label that silently loses a character reads as the wrong words.
    pub fallback: Option<&'static Family>,
}

impl Family {
    /// The tier nearest `line_height`.
    ///
    /// Nearest rather than largest-that-fits: a chrome asking for 30 on a
    /// family cut at 28 and 39 wants the 28, and one asking for 38 wants the
    /// 39. Ties go to the smaller — the one that certainly fits the row that
    /// asked — which falls out of `<` and the ascending order.
    pub fn tier_for(&'static self, line_height: i32) -> &'static Tier {
        let mut best = &self.tiers[0];
        for tier in self.tiers {
            if distance(tier.line_height, line_height) < distance(best.line_height, line_height) {
                best = tier;
            }
        }
        best
    }
}

fn distance(a: i32, b: i32) -> i32 {
    (a - b).abs()
}

// -- Helvetica -------------------------------------------------------------
//
// The family this backend ships, in the sizes u8g2 packages it in; every
// face is a `_tf` variant, u8g2's full 8-bit set, so accented Latin renders
// as itself. **These faces are not this repository's to license**: the
// bitmaps descend from the X11 distribution under Adobe's and Digital's
// notices, and anything shipping this backend ships those too —
// <https://github.com/olikraus/u8g2/blob/master/LICENSE>.

/// One tier of a family cut in a regular and a bold and nothing else.
///
/// A macro because every line of it is the same but for two type names, and
/// four `const` bindings per tier written out five times is four times as many
/// places for a face to be paired with the wrong bytes.
#[macro_export]
macro_rules! font_tier {
    ($height:expr, $regular:ty, $bold:ty) => {{
        // Every path goes through `$crate`, so a caller assembling a family
        // of its own needs this crate and nothing else — the faces and the
        // trait they implement are re-exported for exactly that.
        const R: $crate::FontRenderer =
            $crate::FontRenderer::new::<$regular>().with_ignore_unknown_chars(true);
        const B: $crate::FontRenderer =
            $crate::FontRenderer::new::<$bold>().with_ignore_unknown_chars(true);
        $crate::Tier {
            id: $crate::font_id(&[
                <$regular as $crate::Font>::DATA,
                <$bold as $crate::Font>::DATA,
            ]),
            line_height: $height,
            regular: &R,
            bold: &B,
            italic: None,
            bold_italic: None,
        }
    }};
}

const HELV_08: Tier = font_tier!(14, u8g2::u8g2_font_helvR08_tf, u8g2::u8g2_font_helvB08_tf);
const HELV_10: Tier = font_tier!(18, u8g2::u8g2_font_helvR10_tf, u8g2::u8g2_font_helvB10_tf);
const HELV_12: Tier = font_tier!(21, u8g2::u8g2_font_helvR12_tf, u8g2::u8g2_font_helvB12_tf);
const HELV_18: Tier = font_tier!(30, u8g2::u8g2_font_helvR18_tf, u8g2::u8g2_font_helvB18_tf);
const HELV_24: Tier = font_tier!(39, u8g2::u8g2_font_helvR24_tf, u8g2::u8g2_font_helvB24_tf);

/// The family every preset is built on.
pub static HELVETICA: Family = Family {
    name: "Helvetica",
    tiers: &[HELV_08, HELV_10, HELV_12, HELV_18, HELV_24],
    fallback: None,
};
