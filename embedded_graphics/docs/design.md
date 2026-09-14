# Design decisions

The arguments behind choices the code states in one sentence, and the facts a
consumer needs that the README's first minute does not.

## Monochrome by design, not by limitation

`xpui` paints in ink and background: `fill_rect` takes a `bool`, and `scrim`
and `fill_rect_dither` only mean anything on one bit per pixel. That is
deliberate — it is a framework for e-ink.

This backend is still generic over `PixelColor`. It takes a `Palette` of two
colours and maps ink and background onto them, so the same screens run on a
colour TFT looking monochrome, and swapping the palette inverts the panel with
no change to any screen. What it will not do is let a screen ask for a third
colour, because the framework has no way to.

## The faces it ships

The full 8-bit charset, so `Ambiência` renders as itself rather than as a row
of replacement glyphs. Which family, and how to bring your own, is
[below](#type).

`Fonts::for_metrics` picks a set from the chrome's own list row height, so a
board that scales its chrome up gets type to match: 30 pixels of interface text
on a reader, 39 once a touch board's scale is applied, 18 on a 296x128 strip.
On a 218-ppi panel that 30 is **3.5mm** of glass. The mono faces
[`embedded-graphics`](https://crates.io/crates/embedded-graphics) ships stop at `FONT_10X20`, which is 2.3mm on the same
panel.

The bitmaps live in flash — 27 KB of `.rodata` in a [Badger 2040](https://shop.pimoroni.com/products/badger-2040) build, and not
one byte of RAM. They are also not this repository's to license: see [U8g2](https://github.com/olikraus/u8g2)'s
[LICENSE](https://github.com/olikraus/u8g2/blob/master/LICENSE) for the
foundries' notices.

**No `…`.** These faces stop at U+00FF and the ellipsis is U+2026, so the
ellipsis `xpui-chrome` truncates with is drawn as three full stops rather
than one glyph.
Everything Latin-1 is there; nothing above it is.

## Input is a buffer you fill

`embedded-graphics` knows nothing about buttons or fingers. `InputState` is the
buffer between your event source and the framework:

```rust
# use xpui::{Button, Point};
# use xpui_eg::{Backend, Palette};
# use xpui_screenshot::Framebuffer;
# let backend = Backend::new(Framebuffer::new(480, 800), Palette::INK_IS_ON);
# let (millis, x, y) = (0, 40, 120);
# let at = Point::new(x, y);
backend.begin_frame(millis);        // clears one-frame edges, sets the clock
backend.press(Button::Confirm);     // an edge: true for exactly this frame
backend.tap(Point::new(x, y));
backend.input(|state| state.touch_down(at));   // everything else
```

Every event here is edge state, cleared by `begin_frame`. What survives it is
the held set, the finger's position until `touch_up`, and
`swipe_moves_selection`, which is a setting rather than an event. A
button reported as pressed on every frame re-fires whatever it is on.

## Clipping goes through the target, not through arithmetic

Intersecting rectangles before drawing cannot clip *text*, because glyphs are
rasterised by the font. This backend wraps every draw in
`DrawTargetExt::clipped`, which drops out-of-area pixels inside the target. A
list scrolled under the header would otherwise paint its rows straight over it,
and only a screenshot test notices.

## Type

This crate ships one family — u8g2's Helvetica, in five sizes — and takes
whatever else you bring. A family is a `const`, so adding one costs its bitmaps
in `.rodata` and nothing at run time:

```rust
use xpui_eg::{Family, HELVETICA, Tier, font_tier, u8g2};

const COUR_18: Tier = font_tier!(27, u8g2::u8g2_font_courR18_tf, u8g2::u8g2_font_courB18_tf);

static COURIER: Family = Family {
    name: "Courier",
    tiers: &[COUR_18],
    // Where a glyph Courier lacks is looked for. `None` ends the chain and
    // what is still missing draws a marker box.
    fallback: Some(&HELVETICA),
};
```

The number in `font_tier!` is the band a line occupies: **the tallest style in
the tier, not the regular one.** Several u8g2 families cut their bold a pixel
or two above their regular, and the framework asks for a line height without
saying which style it is about to draw — so a band sized to the regular puts
the difference into the row below. `tests/fonts.rs` asserts every declared band
against its faces.

### A role names a size, not a face

`Fonts` holds a family and the line height each role wants; the family answers
with the tier it was actually cut in. So `backend.set_family(&COURIER)` keeps
the design — a heading stays a heading's height — and cannot carry one family's
sizes into a chrome with no room for them.

### A font id is a hash of the bytes

Not a slot number. Two builds shipping the same face agree on its id, and
changing the face changes it — which is what lets anything caching against a
`FontId` invalidate on its own. The firmware this framework was written beside
writes the id into every cached page layout and compares it on load; hand out a
stable id over changed bytes and that mechanism silently stops working.

### Changing it while it runs

A screen has a `&dyn Host` and no way to name a typeface — deliberately, since
the framework must not know what one is. `request_family` goes past the
framework instead: the backend picks it up at the top of the next
`begin_frame`, which is the one moment nothing has been measured against the
face it replaces.

See `xpui-gallery`'s `gallery/src/typeface.rs` for a picker built on it, and
its `docs/design.md` for what two extra families cost in flash.
