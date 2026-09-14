# Fonts

The type a backend is set in: which family each role resolves through, the
sizes a family was cut in, and how a string is split into the faces that
actually draw it. The faces are U8g2's bitmaps, read from flash a glyph at a
time, so nothing is scaled at run time and nothing allocates.

[`design.md`](../design.md#type) says why a role names a size rather than a
face, and why a font id is a hash of the bytes. This page is what each piece
does.

## Topics

| | |
|---|---|
| [`Fonts`](#xpui_egfonts) | The type a chrome is set in: one family, and the size each role wants. |
| [`Family`](#xpui_egfamily) | A typeface, in ascending size order. |
| [`Tier`](#xpui_egtier) | One size of one family: four styles, and the height of a line of it. |
| [`Face`](#xpui_egface) | A font, resolved: which family it came from, which size, and which style. |
| [`HELVETICA`](#xpui_eghelvetica) | The family every preset is built on. |
| [`font_tier!`](#xpui_egfont_tier) | One tier of a family cut in a regular and a bold and nothing else. |
| [`font_id`](#xpui_egfont_id) | FNV-1a over every byte of every style in a tier. |
| [`request_family`](#xpui_egrequest_family) | Sets the type in `family`, from the next frame, on every backend. |
| [`clear_chosen_family`](#xpui_egclear_chosen_family) | Forgets the choice, so backends keep whatever they were built with. |
| [`Piece`](#xpui_egpiece) | One part of a string, and what draws it. |
| [`pieces`](#xpui_egpieces) | Walks `text` as the pieces it will actually be drawn as. |
| [`advance`](#xpui_egadvance) | What one piece moves the pen by. |
| [Re-exports](#re-exports) | `Font`, `FontRenderer` and the `u8g2` faces. |

## `xpui_eg::Fonts`

The type a chrome is set in: one family, and the size each role wants.

```text
pub struct Fonts
```

**A role names a size, not a face.** Each role asks for a line height, and the
family answers with the tier nearest it. Swapping the family keeps the design:
it cannot carry the old family's sizes into a chrome that has no room for them.
A backend takes one with [`Backend::with_fonts`](backend.md#xpui_egbackendwith_fonts)
and changes it while running with
[`Backend::set_family`](backend.md#xpui_egbackendset_family). `Fonts` is also
`Default`, as `Fonts::DEFAULT`.

| Field | Meaning |
|---|---|
| `xpui_eg::Fonts::family` | The family every role resolves through. |
| `xpui_eg::Fonts::ui` | The line height interface text wants. |
| `xpui_eg::Fonts::ui_small` | The line height secondary text wants. |
| `xpui_eg::Fonts::reader` | The line height a page of prose wants. |

**Example — what each role resolves to**

```rust
use xpui::host::{FontRole, FontStyle};
use xpui_eg::{Fonts, HELVETICA};

let fonts = Fonts::DEFAULT;
assert!(core::ptr::eq(fonts.family, &HELVETICA));
assert_eq!(fonts.tier(FontRole::Ui).line_height, 30);
assert_eq!(fonts.tier(FontRole::Reader).line_height, 39);

let id = fonts.id(FontRole::Ui);
assert!(fonts.tier_by_id(id).is_some());
let face = fonts.face(id, FontStyle::Bold);
assert_eq!(face.tier.line_height, 30);
```

### Presets

#### `xpui_eg::Fonts::DEFAULT`

The set for the default chrome: a reader held with buttons.

```text
pub const DEFAULT: Fonts = Fonts
```

Interface text at 30 pixels, secondary at 18, prose at 39. Thirty pixels is
3.5mm of glass at 218 ppi: the size below which a person stops reading a label
and starts recognising its shape.

#### `xpui_eg::Fonts::LARGE`

The set for chrome a board has scaled up for a finger.

```text
pub const LARGE: Fonts = Fonts
```

#### `xpui_eg::Fonts::COMPACT`

The set for a small colour panel, with 30-pixel rows.

```text
pub const COMPACT: Fonts = Fonts
```

#### `xpui_eg::Fonts::SMALL`

The set for a strip, with 24-pixel rows and a 16-pixel hint band.

```text
pub const SMALL: Fonts = Fonts
```

The smallest chrome, not the smallest type: at 111 ppi, 18 pixels is larger on
the glass than 30 pixels on a reader.

#### `xpui_eg::Fonts::for_metrics`

The sizes that fit this chrome.

```text
pub const fn for_metrics(metrics: &Metrics) -> Fonts
```

Chosen by `list_row_height`, the band body text is painted into:

| Row height | Preset |
|---|---|
| 44 and up | [`LARGE`](#xpui_egfontslarge) |
| 34 to 43 | [`DEFAULT`](#xpui_egfontsdefault) |
| 26 to 33 | [`COMPACT`](#xpui_egfontscompact) |
| below 26 | [`SMALL`](#xpui_egfontssmall) |

#### `xpui_eg::Fonts::with_family`

The same sizes, set in another family.

```text
pub const fn with_family(self, family: &'static Family) -> Fonts
```

### Resolving a role

#### `xpui_eg::Fonts::tier`

The tier a role resolves to.

```text
pub fn tier(&self, role: FontRole) -> &'static Tier
```

#### `xpui_eg::Fonts::id`

The id this backend reports for a role: the tier's bytes **and the family's name**.

```text
pub fn id(&self, role: FontRole) -> FontId
```

Both halves are needed: a face swapped for different bytes has to move the id,
and two families over one tier that differ only in what they fall back to draw
the same string differently. Every id moves when the family changes.

#### `xpui_eg::Fonts::tier_by_id`

The tier an id refers to, or `None` for one this set never handed out.

```text
pub fn tier_by_id(&self, id: FontId) -> Option<&'static Tier>
```

#### `xpui_eg::Fonts::face`

The font an id and style resolve to.

```text
pub fn face(&self, id: FontId, style: FontStyle) -> Face
```

> [!NOTE]
> An id this set never handed out resolves to the **interface** tier, not to
> nothing. Anything holding an id from before a family swap is asking about a
> font that no longer exists, and a legible wrong answer is one somebody can
> see.

**See also:** [`Family`](#xpui_egfamily), [`Face`](#xpui_egface), [`request_family`](#xpui_egrequest_family)

## `xpui_eg::Family`

A typeface, in ascending size order.

```text
pub struct Family
```

A bitmap family has the sizes it has: "18pt" is one of a handful of tiers the
foundry cut. Build one of your own with [`font_tier!`](#xpui_egfont_tier) and
the [`u8g2`](#re-exports) faces.

| Field | Meaning |
|---|---|
| `xpui_eg::Family::name` | What a picker shows. |
| `xpui_eg::Family::tiers` | The sizes it was cut in, ascending by `line_height`. |
| `xpui_eg::Family::fallback` | Where a glyph this family lacks is looked for. |

`name` is not an identity: the id is the bytes. `tiers` must be in ascending
order, which [`tier_for`](#xpui_egfamilytier_for) relies on. A `fallback` of
`None` ends the chain, and a character nothing in the chain has draws as a
marker box rather than disappearing.

**Example — a family of your own**

```rust
use xpui::host::FontRole;
use xpui_eg::{Family, Fonts, HELVETICA, Tier, font_tier, u8g2};

const COURIER_18: Tier = font_tier!(
    27,
    u8g2::u8g2_font_courR18_tf,
    u8g2::u8g2_font_courB18_tf
);

static COURIER: Family = Family {
    name: "Courier",
    tiers: &[COURIER_18],
    fallback: Some(&HELVETICA),
};

let fonts = Fonts::DEFAULT.with_family(&COURIER);
assert_eq!(fonts.tier(FontRole::Reader).line_height, 27);
assert_ne!(fonts.id(FontRole::Ui), Fonts::DEFAULT.id(FontRole::Ui));
```

#### `xpui_eg::Family::tier_for`

The tier nearest `line_height`.

```text
pub fn tier_for(&'static self, line_height: i32) -> &'static Tier
```

Nearest, not the largest that fits: a chrome asking for 30 on a family cut at
28 and 39 gets the 28. A tie goes to the smaller.

## `xpui_eg::Tier`

One size of one family: four styles, and the height of a line of it.

```text
pub struct Tier
```

| Field | Meaning |
|---|---|
| `xpui_eg::Tier::id` | A hash of every byte of every style below. |
| `xpui_eg::Tier::line_height` | The band a line of this tier is painted into. |
| `xpui_eg::Tier::regular` | The upright face. |
| `xpui_eg::Tier::bold` | The bold face. |
| `xpui_eg::Tier::italic` | `None` when the family was never cut in it — u8g2's Helvetica was not. |
| `xpui_eg::Tier::bold_italic` | `None` when the family was never cut in it; bold is drawn instead. |

`line_height` is the **tallest** style, not the regular: a bold is often a
pixel or two taller, and the framework asks for a line height without saying
which style it will draw. Asking for italic from a tier without one gets the
regular face.

#### `xpui_eg::Tier::face`

The face for a style, falling back to the nearest cut that exists.

```text
pub fn face(&self, style: FontStyle) -> &'static FontRenderer
```

## `xpui_eg::Face`

A font, resolved: which family it came from, which size, and which style.

```text
pub struct Face
```

The family travels with the tier because a missing glyph is looked for in the
family's fallback, and a tier on its own does not know which family cut it.
[`Fonts::face`](#xpui_egfontsface) makes one.

| Field | Meaning |
|---|---|
| `xpui_eg::Face::family` | The family it came from, for the fallback chain. |
| `xpui_eg::Face::tier` | The size it was cut in. |
| `xpui_eg::Face::style` | Weight and slant. |

#### `xpui_eg::Face::renderer`

The renderer that actually paints this style.

```text
pub fn renderer(&self) -> &'static u8g2_fonts::FontRenderer
```

## `xpui_eg::HELVETICA`

The family every preset is built on.

```text
pub static HELVETICA: Family = Family
```

Cut at line heights 14, 18, 21, 30 and 39, each a regular and a bold from
u8g2's full 8-bit set, so accented Latin renders as itself. It has no italic
and no fallback.

> [!NOTE]
> These faces are not this repository's to license. The bitmaps descend from
> the X11 distribution under Adobe's and Digital's notices, and anything
> shipping this backend ships those too:
> <https://github.com/olikraus/u8g2/blob/master/LICENSE>.

## `xpui_eg::font_tier!`

One tier of a family cut in a regular and a bold and nothing else.

```text
macro_rules! font_tier
```

```text
font_tier!(line_height, RegularFace, BoldFace)
```

It expands to a `Tier` whose id is [`font_id`](#xpui_egfont_id) over both
faces' bytes, with no italic. Every path in it goes through this crate, so a
caller needs no dependency on `u8g2-fonts` of its own. The line height is
declared, not measured: give the taller of the two faces. See
[the example under `Family`](#xpui_egfamily).

## `xpui_eg::font_id`

FNV-1a over every byte of every style in a tier.

```text
pub const fn font_id(styles: &[&[u8]]) -> FontId
```

`const`, so a tier's id costs nothing at run time. The answer is folded to a
positive `i32`, and never `0`, which the framework reads as "no such font".

```rust
use xpui_eg::font_id;

let one = font_id(&[b"one".as_slice()]);
let two = font_id(&[b"two".as_slice()]);
assert_ne!(one, two);
assert!(font_id(&[]).0 > 0);
```

## `xpui_eg::request_family`

Sets the type in `family`, from the next frame, on every backend.

```text
pub fn request_family(family: &'static Family)
```

A standing choice, not a one-shot: each backend picks it up in its next
[`begin_frame`](backend.md#xpui_egbackendbegin_frame), so a simulator that
holds one backend per board keeps it across a switch of board. It lands between
frames because a family applied mid-frame would leave a screen measured in one
face and painted in another. It asks for a repaint, so a host must be installed.

```rust
use xpui_eg::{Backend, HELVETICA, Palette, clear_chosen_family, request_family};
use xpui_screenshot::Framebuffer;

# xpui::testing::install();
let backend = Backend::new(Framebuffer::new(480, 800), Palette::INK_IS_ON);
request_family(&HELVETICA);
backend.begin_frame(0);
assert!(core::ptr::eq(backend.fonts().family, &HELVETICA));
clear_chosen_family();
```

## `xpui_eg::clear_chosen_family`

Forgets the choice, so backends keep whatever they were built with.

```text
pub fn clear_chosen_family()
```

For tests, which share the choice with every other test in their binary.

## `xpui_eg::Piece`

One part of a string, and what draws it.

```text
pub enum Piece<'a>
```

| Variant | Meaning |
|---|---|
| `xpui_eg::Piece::Run` | Draw `text` with `face`. |
| `xpui_eg::Piece::Marker` | A character nothing in the chain has, drawn as a box this size. |

A `Run`'s `text` is usually a slice of the string, but an ellipsis no face in
the chain has arrives as `"..."`. A `Marker` carries its `width` and `height`.

## `xpui_eg::pieces`

Walks `text` as the pieces it will actually be drawn as.

```text
pub fn pieces<'a>(font: Face, text: &'a str, emit: impl FnMut(Piece<'a>))
```

**Measuring and drawing walk this same function**, so a layout cannot measure
with one face and paint with another. Fallback is per glyph: the face the role
resolved to, then the family's fallback chain, then a marker. A string the
first face draws whole is one piece.

```rust
use xpui::host::{FontRole, FontStyle};
use xpui_eg::{Fonts, Piece, advance, pieces};

let fonts = Fonts::DEFAULT;
let face = fonts.face(fonts.id(FontRole::Ui), FontStyle::Regular);

let mut runs = 0;
let mut width = 0;
pieces(face, "Wi-Fi…", |piece| {
    if let Piece::Run { .. } = piece {
        runs += 1;
    }
    width += advance(piece);
});
assert_eq!(runs, 2, "the ellipsis is drawn as three full stops");
assert!(width > 0);
```

## `xpui_eg::advance`

What one piece moves the pen by.

```text
pub fn advance(piece: Piece<'_>) -> i32
```

Drawing advances by exactly this after each piece, so a string is as wide as
its parts wherever it is asked. A marker advances one pixel past its box.

## Re-exports

Re-exported so a caller can assemble a family without a second dependency on
`u8g2-fonts` that could drift to another version.

| Name | What it is |
|---|---|
| `xpui_eg::Font` | `u8g2-fonts`' trait for a face; [`font_tier!`](#xpui_egfont_tier) reads each face's bytes through it. |
| `xpui_eg::FontRenderer` | `u8g2-fonts`' renderer for one face, which a [`Tier`](#xpui_egtier) holds per style. |
| `xpui_eg::u8g2` | Every face `u8g2-fonts` ships, as `u8g2_fonts::fonts`: `u8g2::u8g2_font_helvR18_tf` and the rest. |
