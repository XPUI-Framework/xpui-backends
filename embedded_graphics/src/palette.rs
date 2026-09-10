//! The two colours ink and background map onto.

use embedded_graphics::pixelcolor::BinaryColor;

/// The two colours ink and background map onto. The wrong pair inverts a
/// panel silently; the crate root says which is which.
#[derive(Copy, Clone, Debug)]
pub struct Palette<C> {
    /// What ink paints as.
    pub ink: C,
    /// What background paints as.
    pub background: C,
}

impl<C> Palette<C> {
    /// A palette painting ink as `ink` and background as `background`.
    pub fn new(ink: C, background: C) -> Self {
        Palette { ink, background }
    }
}

impl Palette<BinaryColor> {
    /// Ink is `On`. The common case, and what the simulator uses.
    pub const INK_IS_ON: Self = Palette {
        ink: BinaryColor::On,
        background: BinaryColor::Off,
    };

    /// Ink is `Off`. Some 1-bit panels invert: `uc8151`, the Badger 2040's
    /// controller, maps `Off` to black so bitmaps load unmirrored.
    pub const INK_IS_OFF: Self = Palette {
        ink: BinaryColor::Off,
        background: BinaryColor::On,
    };
}
