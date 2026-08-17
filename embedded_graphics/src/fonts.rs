//! Which typeface each role resolves to, and what it measures.
//!
//! `embedded_graphics` ships monospaced bitmap fonts, so measurement is exact
//! arithmetic rather than an estimate — which is what the framework requires.
//! A backend that guesses widths lays out correctly and paints off the edge.

use embedded_graphics::mono_font::{MonoFont, ascii};
use xpui::host::{FontId, FontRole, FontStyle};
use xpui_chrome::Tokens;

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
    /// `embedded-graphics` ships no bold under 6x13, so the two smallest sets
    /// answer with the regular face and `Font::ui_small().bold()` renders
    /// identically. A backend that cares supplies a real one; the field exists
    /// so it can.
    pub ui_small_bold: &'static MonoFont<'static>,
    pub reader: &'static MonoFont<'static>,
}

impl Fonts {
    /// At the top two tiers the reading face and the interface face are the
    /// same font, because this backend's set stops at ten by twenty and both
    /// roles want the largest thing in it. A screen demonstrating the roles
    /// therefore shows two of them identical here — not a mistake, a ceiling,
    /// and the one a larger font set would lift.
    /// The set for the default chrome: a reader held with buttons.
    ///
    /// The interface face is the largest `embedded-graphics` ships, because on
    /// a reader even that is small: 20 pixels is 2.3mm of glass at 218 ppi and
    /// 2.0mm at 257. A 15-pixel face on the same panels is 1.7mm and 1.5mm,
    /// which is under the size at which a person stops reading a label and
    /// starts recognising its shape.
    pub const DEFAULT: Fonts = Fonts {
        ui: &ascii::FONT_10X20,
        // The set ships no bold 10x20, so bold is the largest real bold face
        // rather than the regular one wearing the name — see spec 06, and the
        // note on `ui_small_bold`. Two pixels shorter than the regular face,
        // which nothing notices: `ui().bold()` is only ever a title alone in
        // its own band, and every width is measured with the face it is drawn
        // with. `line_height` reports the regular face's 20 for both, so a
        // title band comes out two pixels generous rather than two short.
        ui_bold: &ascii::FONT_9X18_BOLD,
        ui_small: &ascii::FONT_7X13,
        ui_small_bold: &ascii::FONT_7X13_BOLD,
        reader: &ascii::FONT_10X20,
    };

    /// The set for chrome a board has scaled up for a finger.
    ///
    /// **Only the small face differs from [`DEFAULT`](Fonts::DEFAULT), and
    /// that is the backend's ceiling talking.** 10x20 is the largest face in
    /// the set, so the interface text is already as big as it can be at 1.0
    /// and a board asking for 1.2 spends its scale on space and on the
    /// secondary text instead. A panel that wants 1.2x type needs a font set
    /// that has it, not a different rule here.
    pub const LARGE: Fonts = Fonts {
        ui: &ascii::FONT_10X20,
        ui_bold: &ascii::FONT_9X18_BOLD,
        ui_small: &ascii::FONT_9X15,
        ui_small_bold: &ascii::FONT_9X15_BOLD,
        reader: &ascii::FONT_10X20,
    };

    /// The set for a small colour panel — a Tufty 2040, with 30-pixel rows.
    pub const COMPACT: Fonts = Fonts {
        ui: &ascii::FONT_9X15,
        ui_bold: &ascii::FONT_9X15_BOLD,
        ui_small: &ascii::FONT_6X10,
        ui_small_bold: &ascii::FONT_6X10,
        reader: &ascii::FONT_9X18,
    };

    /// The set for a strip — a Badger 2040, with 24-pixel rows and a 16-pixel
    /// hint band.
    ///
    /// The smallest chrome and not the smallest type: 111 ppi is a third of a
    /// reader's density, so 13 pixels here is 3.0mm — larger, on the glass,
    /// than 20 pixels on an X4.
    pub const SMALL: Fonts = Fonts {
        ui: &ascii::FONT_7X13,
        ui_bold: &ascii::FONT_7X13_BOLD,
        ui_small: &ascii::FONT_6X10,
        ui_small_bold: &ascii::FONT_6X10,
        reader: &ascii::FONT_9X15,
    };

    /// The faces that fit this chrome.
    ///
    /// Chosen by list row height, because that is the band body text is
    /// painted into and the one every preset scales together with the rest of
    /// itself. A board's [`ui_scale_percent`] reaches the type this way rather
    /// than directly: scaling the chrome up moves the row across a threshold,
    /// and the type follows it.
    ///
    /// The thresholds sit between the row heights the presets actually use —
    /// 24 on a strip, 30 on a small panel, 40 on a reader, and 48 once a touch
    /// board's scale is applied.
    ///
    /// **This ladder is short at the top, and that is the backend's ceiling
    /// rather than a choice.** `embedded-graphics` ships nothing above 10x20,
    /// so a 20-pixel line is 2.3mm on a 217-ppi reader where the firmware this
    /// framework was written beside puts 29 pixels and 3.4mm. A panel that
    /// wants more needs a font set with more, not a different rule here.
    ///
    /// [`ui_scale_percent`]: xpui_boards::Board::ui_scale_percent
    pub const fn for_tokens(tokens: &Tokens) -> Fonts {
        match tokens.list_row_height {
            44.. => Fonts::LARGE,
            34..=43 => Fonts::DEFAULT,
            26..=33 => Fonts::COMPACT,
            _ => Fonts::SMALL,
        }
    }

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
