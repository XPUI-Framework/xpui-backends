//! Deciding which face draws which part of a string.
//!
//! **Measuring and drawing walk this same function**: a layout that measured
//! with one face and painted with another would wrap correctly and overflow
//! anyway. Both call [`pieces`]; neither decides anything on its own.
//!
//! The chain is the face the role resolved to, then the family's fallback,
//! then a marker box — a label that silently loses a character reads as the
//! wrong words. Fallback is **per glyph**, not per string: sending a whole
//! label to another family over one ellipsis would change the type of a row.
//! `…` is the character that turns up missing — these faces stop at U+00FF —
//! and where no face in the chain has it, it becomes three full stops.

use u8g2_fonts::FontRenderer;
use u8g2_fonts::types::VerticalPosition;

use super::Face;
use super::family::Tier;
use crate::clip::EgPoint;

/// How far a chain of fallbacks is followed before it is treated as a chain
/// that ends.
const MAX_FALLBACK_DEPTH: u32 = 4;

/// One part of a string, and what draws it.
#[derive(Clone, Copy)]
pub enum Piece<'a> {
    /// Draw `text` with `face`.
    ///
    /// `text` is usually a slice of the string being measured, but not always:
    /// an ellipsis nothing can draw arrives here as `"..."`.
    Run {
        face: &'static FontRenderer,
        text: &'a str,
    },
    /// Nothing in the chain has this character. Draw a box this size.
    Marker { width: i32, height: i32 },
}

/// A missing character's box: narrow, and short enough to sit on the baseline.
///
/// Derived from the line height rather than fixed, so it is in proportion on a
/// strip and on a reader alike.
fn marker(tier: &Tier) -> Piece<'static> {
    Piece::Marker {
        width: (tier.line_height / 3).max(3),
        height: (tier.line_height / 2).max(4),
    }
}

/// Walks `text` as the pieces it will actually be drawn as.
///
/// The common case is one call and one piece: every character is in the face
/// the role resolved to, so the whole string is handed over as it stands. Only
/// a string that face cannot fully draw is walked character by character, and
/// only those characters cost anything.
pub fn pieces<'a>(font: Face, text: &'a str, mut emit: impl FnMut(Piece<'a>)) {
    let face = font.renderer();
    if draws_all(face, text) {
        if !text.is_empty() {
            emit(Piece::Run { face, text });
        }
        return;
    }

    // Something is missing. Walk it, gathering the characters that share a
    // face into runs so a label with one odd glyph is still three draw calls
    // and not thirty.
    let mut run_start = 0;
    let mut run_face: Option<&'static FontRenderer> = None;

    for (at, character) in text.char_indices() {
        let resolved = resolve(font, character);

        // A run continues only while the face does not change. Compared by
        // address: a `FontRenderer` is not comparable and does not need to be
        // — two runs share a face when they share the *same* face.
        if !same_face(run_face, resolved.face())
            && let Some(open) = run_face
        {
            emit(Piece::Run {
                face: open,
                text: &text[run_start..at],
            });
            run_face = None;
        }

        match resolved {
            Resolved::Face(face) => {
                if run_face.is_none() {
                    run_start = at;
                    run_face = Some(face);
                }
            }
            Resolved::Ellipsis(face) => emit(Piece::Run { face, text: "..." }),
            Resolved::Missing => emit(marker(font.tier)),
        }
    }

    if let Some(open) = run_face {
        emit(Piece::Run {
            face: open,
            text: &text[run_start..],
        });
    }
}

/// Whether two resolutions landed on the very same face.
fn same_face(a: Option<&'static FontRenderer>, b: Option<&'static FontRenderer>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => core::ptr::eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// What one character turned out to need.
#[derive(Clone, Copy)]
enum Resolved {
    /// A face in the chain has it.
    Face(&'static FontRenderer),
    /// Nothing has `…`, so three full stops in this face stand in for it.
    Ellipsis(&'static FontRenderer),
    /// Nothing has it and there is no substitute.
    Missing,
}

impl Resolved {
    /// The face a *run* may continue with. A substitution or a marker breaks
    /// the run whatever face it ends up using, because it is not a slice of
    /// the original string.
    fn face(self) -> Option<&'static FontRenderer> {
        match self {
            Resolved::Face(face) => Some(face),
            Resolved::Ellipsis(_) | Resolved::Missing => None,
        }
    }
}

/// Walks the fallback chain for one character.
fn resolve(font: Face, character: char) -> Resolved {
    let mut buffer = [0u8; 4];
    let as_str: &str = character.encode_utf8(&mut buffer);

    let primary = font.renderer();
    if draws_all(primary, as_str) {
        return Resolved::Face(primary);
    }

    let mut family = font.family.fallback;
    let mut depth = 0;
    while let Some(next) = family {
        // Capped, because `Family` has public fields and two of them naming
        // each other compiles: `A.fallback = &B, B.fallback = &A` is a chain
        // with no end, and walking it inside a text measurement is a hang in
        // the middle of a render with no panic to point at it. Four is past
        // any real chain — primary, a wider repertoire, a symbol face — and
        // stopping early draws a marker, which is what a chain that ran out
        // does anyway.
        depth += 1;
        if depth > MAX_FALLBACK_DEPTH {
            break;
        }

        // The tier nearest this one's height, so a fallback glyph is the size
        // of the text around it rather than the size the fallback happens to
        // start at.
        let face = next.tier_for(font.tier.line_height).face(font.style);
        if draws_all(face, as_str) {
            return Resolved::Face(face);
        }
        family = next.fallback;
    }

    if character == '…' {
        Resolved::Ellipsis(primary)
    } else {
        Resolved::Missing
    }
}

/// Whether `face` has a glyph for every character of `text`.
///
/// The faces handed out skip an unknown character rather than failing the
/// string it is in, which is right for drawing and useless for asking. A
/// strict copy is made to ask with: `FontRenderer` is a font pointer and two
/// flags, so the copy is a register move rather than anything to avoid.
fn draws_all(face: &FontRenderer, text: &str) -> bool {
    face.clone()
        .with_ignore_unknown_chars(false)
        .get_rendered_dimensions(text, EgPoint::zero(), VerticalPosition::Baseline)
        .is_ok()
}

/// How far below the top of a line box the baseline sits.
///
/// The framework hands a top-left origin and u8g2 draws from a baseline, so
/// something has to convert. Not the font's ascent: a capital with an accent
/// on it rises above the ascent, and anchoring there paints those few rows
/// into the line above. The font's bounding box is the box every glyph it can
/// draw fits inside, so putting the baseline at its top puts all of them
/// inside `[y, y + line_height)`.
pub fn baseline_offset(face: &FontRenderer) -> i32 {
    -face
        .get_font_bounding_box(VerticalPosition::Baseline)
        .top_left
        .y
}

/// Width of `text` in `tier`: the sum of what each piece advances the pen.
///
/// The advance, not the ink. It is where the next glyph would start, which is
/// what a layout needs, and it is exact — every glyph is looked up, none is
/// estimated. A few faces carry a glyph whose ink overhangs its own advance by
/// a pixel (`©`, `î`), which is ordinary side bearing rather than a
/// measurement error.
pub fn text_width(font: Face, text: &str) -> i32 {
    let mut width = 0;
    pieces(font, text, |piece| {
        width += advance(piece);
    });
    width
}

/// What one piece moves the pen by.
///
/// Shared with drawing, which advances by exactly this after painting each
/// piece — so a string is as wide as its parts wherever it is asked.
pub fn advance(piece: Piece<'_>) -> i32 {
    match piece {
        Piece::Run { face, text } => face
            .get_rendered_dimensions(text, EgPoint::zero(), VerticalPosition::Baseline)
            .map_or(0, |rendered| rendered.advance.x),
        // One pixel of air after the box, or two markers side by side read as
        // one wide one.
        Piece::Marker { width, .. } => width + 1,
    }
}
