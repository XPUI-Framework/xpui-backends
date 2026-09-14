//! Which typeface each role resolves to, and what it measures.
//!
//! The faces are U8g2's, read through [`u8g2_fonts`]: bit-packed bitmaps that
//! live in flash and are decoded a glyph at a time. Nothing is scaled or
//! rasterised at run time and nothing allocates, which is what lets a face
//! three times the size of a mono one cost only its own bytes.
//!
//! **A role names a size, not a face.** [`Fonts`] holds a [`Family`] and the
//! line height each role wants; the family answers with the tier it was
//! actually cut in, so swapping the family keeps the design and cannot carry
//! the old family's sizes into a chrome that has no room for them.

mod family;
mod resolve;
mod switch;

pub use family::{Family, HELVETICA, Tier, font_id};
pub use resolve::{Piece, advance, baseline_offset, pieces, text_width};
pub(crate) use switch::chosen_family;
pub use switch::{clear_chosen_family, request_family};

use xpui::host::{FontId, FontRole, FontStyle};
use xpui_chrome::Metrics;

/// The type a chrome is set in: one family, and the size each role wants.
///
/// Swappable: a backend built for a particular panel passes its own `Fonts`
/// rather than editing this, and [`Backend::set_family`] changes one while it
/// runs.
///
/// [`Backend::set_family`]: crate::Backend::set_family
#[derive(Copy, Clone)]
pub struct Fonts {
    /// The family every role resolves through.
    pub family: &'static Family,
    /// The line height interface text wants.
    pub ui: i32,
    /// The line height secondary text wants.
    pub ui_small: i32,
    /// The line height a page of prose wants.
    pub reader: i32,
}

impl Fonts {
    /// The set for the default chrome: a reader held with buttons.
    ///
    /// 30 pixels of line is 3.5mm of glass at 218 ppi and 3.0mm at 257: the
    /// size below which a person stops reading a label and starts recognising
    /// its shape. The reading height is a step above, because a page of prose
    /// is read for minutes and a label for a second.
    pub const DEFAULT: Fonts = Fonts {
        family: &HELVETICA,
        ui: 30,
        ui_small: 18,
        reader: 39,
    };

    /// The set for chrome a board has scaled up for a finger.
    ///
    /// Both heights step up. The ladder is coarse because a bitmap family has
    /// the sizes it has, and the row this sits in was scaled by the same
    /// board, so it still fits with room to spare.
    pub const LARGE: Fonts = Fonts {
        family: &HELVETICA,
        ui: 39,
        ui_small: 21,
        reader: 39,
    };

    /// The set for a small colour panel, with 30-pixel rows.
    pub const COMPACT: Fonts = Fonts {
        family: &HELVETICA,
        ui: 21,
        ui_small: 14,
        reader: 30,
    };

    /// The set for a strip, with 24-pixel rows and a 16-pixel hint band.
    ///
    /// The smallest chrome and not the smallest type: 111 ppi is half a
    /// reader's density, so 18 pixels here is 4.1mm — larger, on the glass,
    /// than 30 pixels on a 218-ppi reader.
    pub const SMALL: Fonts = Fonts {
        family: &HELVETICA,
        ui: 18,
        ui_small: 14,
        reader: 21,
    };

    /// The sizes that fit this chrome.
    ///
    /// Chosen by list row height, because that is the band body text is
    /// painted into and the one every preset scales together with the rest of
    /// itself. A device's UI scale reaches the type this way rather
    /// than directly: scaling the chrome up moves the row across a threshold,
    /// and the type follows it.
    ///
    /// The thresholds sit between the row heights the presets use — 24 on a
    /// strip, 30 on a small panel, 40 on a reader, and 48 once a touch board's
    /// scale is applied.
    pub const fn for_metrics(metrics: &Metrics) -> Fonts {
        match metrics.list_row_height {
            44.. => Fonts::LARGE,
            34..=43 => Fonts::DEFAULT,
            26..=33 => Fonts::COMPACT,
            _ => Fonts::SMALL,
        }
    }

    /// The same sizes, set in another family.
    ///
    /// The sizes are kept and the tiers re-resolved, so a family cut at
    /// different heights lands in the rows this chrome laid out rather than
    /// carrying the previous family's heights into them.
    pub const fn with_family(self, family: &'static Family) -> Fonts {
        Fonts { family, ..self }
    }

    /// The line height a role asks for.
    const fn height(&self, role: FontRole) -> i32 {
        match role {
            FontRole::Ui => self.ui,
            FontRole::UiSmall => self.ui_small,
            FontRole::Reader => self.reader,
        }
    }

    /// The tier a role resolves to.
    pub fn tier(&self, role: FontRole) -> &'static Tier {
        self.family.tier_for(self.height(role))
    }

    /// The id this backend reports for a role: the tier's bytes **and the
    /// family's name**.
    ///
    /// Both halves are needed — a face swapped for different
    /// bytes has to move the id, and two families over one tier that differ
    /// only in what they fall back to draw the same string differently and
    /// must not claim the same id. Hashed on every call: a dozen byte
    /// operations against a glyph lookup per character, not worth caching.
    pub fn id(&self, role: FontRole) -> FontId {
        blend(self.tier(role).id, self.family)
    }

    /// The tier an id refers to, or `None` for one this set never handed out.
    ///
    /// Three integer comparisons rather than a search: only the three roles
    /// resolve to anything, so only their tiers can be asked about.
    pub fn tier_by_id(&self, id: FontId) -> Option<&'static Tier> {
        [FontRole::Ui, FontRole::UiSmall, FontRole::Reader]
            .into_iter()
            .map(|role| self.tier(role))
            .find(|tier| blend(tier.id, self.family) == id)
    }

    /// The font an id and style resolve to.
    ///
    /// An id this set never handed out resolves to the **interface** tier
    /// rather than to nothing. That happens for real: every id moves when the
    /// family changes, so anything holding one from before the swap is asking
    /// about a font that no longer exists. Answering with zero width and no
    /// ink lays the screen out invisibly and reports nothing; answering with
    /// the interface face is legible, and wrong in a way somebody can see.
    pub fn face(&self, id: FontId, style: FontStyle) -> Face {
        Face {
            family: self.family,
            tier: self
                .tier_by_id(id)
                .unwrap_or_else(|| self.tier(FontRole::Ui)),
            style,
        }
    }
}

/// A font, resolved: which family it came from, which size, and which style.
///
/// The family travels with the tier because a missing glyph is looked for in
/// the family's fallback, and a tier on its own does not know which family cut
/// it.
#[derive(Copy, Clone)]
pub struct Face {
    /// The family it came from, for the fallback chain.
    pub family: &'static Family,
    /// The size it was cut in.
    pub tier: &'static Tier,
    /// Weight and slant.
    pub style: FontStyle,
}

impl Face {
    /// The renderer that actually paints this style.
    pub fn renderer(&self) -> &'static u8g2_fonts::FontRenderer {
        self.tier.face(self.style)
    }
}

/// A tier's id as seen through the family that resolved it.
///
/// Kept out of [`Tier`] because a tier does not know which families point at
/// it, and the same tier reached through two of them is two different fonts as
/// far as anything caching on the answer is concerned.
fn blend(tier: FontId, family: &'static Family) -> FontId {
    let salted = font_id(&[family.name.as_bytes()]);
    let mixed = (tier.0 ^ salted.0.rotate_left(16)) & 0x7fff_ffff;
    FontId(if mixed == 0 { 1 } else { mixed })
}

impl Default for Fonts {
    fn default() -> Self {
        Fonts::DEFAULT
    }
}
