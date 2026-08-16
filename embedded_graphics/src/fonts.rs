//! Which typeface each role resolves to, and what it measures.
//!
//! `embedded_graphics` ships monospaced bitmap fonts, so measurement is exact
//! arithmetic rather than an estimate — which is what the framework requires.
//! A backend that guesses widths lays out correctly and paints off the edge.

use embedded_graphics::mono_font::{MonoFont, ascii};
use xpui::host::{FontId, FontRole, FontStyle};

/// The faces this backend hands out, one per role.
///
/// Swappable: a backend built for a particular panel passes its own
/// [`Fonts`] rather than editing this. The `iso_8859_1` variants cost more
/// flash and cover accented Latin; the `ascii` ones are the default because
/// they are what every build can afford.
#[derive(Copy, Clone)]
pub struct Fonts {
    pub ui: &'static MonoFont<'static>,
    pub ui_bold: &'static MonoFont<'static>,
    pub ui_small: &'static MonoFont<'static>,
    /// `embedded-graphics` ships no bold 6x10, so the default is the regular
    /// face and `Font::ui_small().bold()` renders identically. A backend that
    /// cares supplies a real one; the field exists so it can.
    pub ui_small_bold: &'static MonoFont<'static>,
    pub reader: &'static MonoFont<'static>,
}

impl Fonts {
    pub const DEFAULT: Fonts = Fonts {
        ui: &ascii::FONT_9X15,
        ui_bold: &ascii::FONT_9X15_BOLD,
        ui_small: &ascii::FONT_6X10,
        ui_small_bold: &ascii::FONT_6X10,
        reader: &ascii::FONT_10X20,
    };

    /// The id this backend reports for a role.
    ///
    /// Ids are opaque to the framework and only have to round-trip, so they
    /// are small integers rather than anything meaningful. `0` is reserved:
    /// the framework reads it as "this build ships no such font".
    pub(crate) fn id(role: FontRole) -> FontId {
        FontId(match role {
            FontRole::Ui => 1,
            FontRole::UiSmall => 2,
            FontRole::Reader => 3,
        })
    }

    /// The face for an id and style, or `None` for an id this backend never
    /// handed out.
    pub(crate) fn face(&self, id: FontId, style: FontStyle) -> Option<&'static MonoFont<'static>> {
        let bold = matches!(style, FontStyle::Bold | FontStyle::BoldItalic);
        Some(match (id.0, bold) {
            (1, false) => self.ui,
            (1, true) => self.ui_bold,
            (2, false) => self.ui_small,
            (2, true) => self.ui_small_bold,
            (3, _) => self.reader,
            _ => return None,
        })
    }
}

impl Default for Fonts {
    fn default() -> Self {
        Fonts::DEFAULT
    }
}

/// Horizontal advance of one character, including the gap after it.
pub(crate) fn advance(font: &MonoFont<'_>) -> i32 {
    font.character_size.width as i32 + font.character_spacing as i32
}

/// Width of `text`, counted in characters.
///
/// Chars, never bytes: a multi-byte character is one glyph, and measuring by
/// `len()` makes any accented word wider than it is drawn.
pub(crate) fn text_width(font: &MonoFont<'_>, text: &str) -> i32 {
    let count = text.chars().count() as i32;
    if count == 0 {
        return 0;
    }
    // The trailing character carries no spacing after it.
    count * advance(font) - font.character_spacing as i32
}

pub(crate) fn line_height(font: &MonoFont<'_>) -> i32 {
    font.character_size.height as i32
}
