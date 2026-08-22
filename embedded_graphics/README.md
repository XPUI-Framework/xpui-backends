# `xpui-embedded-graphics`

An [`xpui`](../../xpui/) backend that draws through any `embedded-graphics`
`DrawTarget` — which is most of the embedded Rust display ecosystem: e-paper
panels, SSD1306 and friends, colour TFTs, and the desktop simulator.

```rust,no_run
# use embedded_graphics::pixelcolor::BinaryColor;
# use embedded_graphics::prelude::*;
# use embedded_graphics::primitives::Rectangle;
# use xpui::{App, Button, Screen, Text, View};
# use xpui_eg::{Backend, Palette};
# /// Whatever driver you already have: a panel, an OLED, a colour TFT.
# struct MyPanel;
# impl MyPanel { fn flush(&mut self) {} }
# impl Dimensions for MyPanel {
#     fn bounding_box(&self) -> Rectangle { Rectangle::new(Point::zero(), Size::new(480, 800)) }
# }
# impl DrawTarget for MyPanel {
#     type Color = BinaryColor;
#     type Error = core::convert::Infallible;
#     fn draw_iter<I>(&mut self, _pixels: I) -> Result<(), Self::Error>
#     where I: IntoIterator<Item = Pixel<Self::Color>> { Ok(()) }
# }
# struct MainMenu;
# impl MainMenu { fn new() -> Self { MainMenu } }
# impl Screen for MainMenu {
#     type Message = ();
#     fn body(&self) -> impl View<()> { Text::new("Main menu") }
#     fn update(&mut self, _message: ()) {}
# }
# fn millis_since_boot() -> u32 { 0 }
# let display = MyPanel;
let backend = Backend::leak(display, Palette::new(BinaryColor::On, BinaryColor::Off));
unsafe { xpui::host::install(backend) };

let mut app = App::new(MainMenu::new());
while app.is_running() {
    backend.begin_frame(millis_since_boot());
    backend.press(Button::Down);        // from wherever your input comes from
    app.tick();
    if app.render_if_dirty() {
        backend.clear_dirty();
        backend.with_display(|display| display.flush());
    }
}
```

It supplies `Canvas` and `TextMetrics` itself, `InputSource` and `Clock` from
whatever you feed it — bar `has_left_right_keys`, which it reads off the board
it was built for — and takes `Chrome` from
[`xpui-chrome`](../chrome/) — so a list, a dialog and a slider look like
something without you drawing one.

## Monochrome by design, not by limitation

`xpui` paints in ink and background: `fill_rect` takes a `bool`, and `scrim`
and `fill_rect_dither` only mean anything on one bit per pixel. That is
deliberate — it is a framework for e-ink.

This backend is still generic over `PixelColor`. It takes a `Palette` of two
colours and maps ink and background onto them, so the same screens run on a
colour TFT looking monochrome, and swapping the palette inverts the panel with
no change to any screen. What it will not do is let a screen ask for a third
colour, because the framework has no way to.

## Type

The faces are U8g2's Helvetica, through
[`u8g2-fonts`](https://crates.io/crates/u8g2-fonts): one family, a real bold at
every size, and the full 8-bit charset — so `Ambiência` renders as itself
rather than as a row of replacement glyphs.

`Fonts::for_metrics` picks a set from the chrome's own list row height, so a
board that scales its chrome up gets type to match: 30 pixels of interface text
on a reader, 39 once a touch board's scale is applied, 18 on a 296x128 strip.
On a 218-ppi panel that 30 is **3.4mm** of glass. The mono faces
`embedded-graphics` ships stop at `FONT_10X20`, which is 2.3mm on the same
panel.

The bitmaps live in flash — 27 KB of `.rodata` in a Badger 2040 build, and not
one byte of RAM. They are also not this repository's to license: see U8g2's
[LICENSE](https://github.com/olikraus/u8g2/blob/master/LICENSE) for the
foundries' notices.

**No `…`.** These faces stop at U+00FF and the ellipsis is U+2026, so a label
that `xpui-chrome` truncates ends where it was cut rather than in a marker.
Everything Latin-1 is there; nothing above it is.

## Input

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

Everything except held buttons is edge state, cleared by `begin_frame`. A
button reported as pressed on every frame re-fires whatever it is on.

## Screenshot tests

```toml
[dev-dependencies]
xpui-screenshot = "0.1"
```

A separate crate, because it is `std`, it writes files, and this one builds for
bare metal. `Framebuffer` is a plain 1-bit `DrawTarget` with no window and no
hardware, so a screen can be rendered and asserted on in an ordinary
`cargo test`:

```rust,no_run
# use xpui_eg::{Backend, Palette};
# use xpui_screenshot::{Framebuffer, assert_screenshot};
# let backend = Backend::new(Framebuffer::new(480, 800), Palette::INK_IS_ON);
backend.with_display(|frame| {
    assert_screenshot("my_screen", frame);      // against a committed PNG
    assert!(frame.ink_in(0, 0, 480, 56) > 0);   // and what a picture cannot say
});
```

`assert_screenshot` compares the whole panel against
`tests/screenshots/my_screen.png` in the crate being tested, pixel for pixel.
The first run writes the golden and fails, so that nobody commits a picture
they never looked at; `UPDATE_SNAPSHOTS=1` rewrites it afterwards. A mismatch
prints an ASCII view marking every block that changed and writes
`target/diff/my_screen.png` — expected, actual and the differences, side by
side.

That is how this crate's own tests work — see `tests/screenshots.rs`.

`check_screenshot` is the same comparison handing back its report as
`Result<(), String>` rather than panicking with it, for a test that captures
many frames and wants to name every one that moved rather than stopping at the
first. A broken harness — a golden that will not decode, a directory that will
not take a file — still panics through either of them.
[`examples/gallery`](../../../examples/gallery/) renders nine screens on seven
boards that way, so one token moved by one pixel names every board it reached
rather than the first.

`write_bmp` and `thumbnail` are still there, and are for looking at a frame
rather than asserting on one. Nothing compares them.

## One thing worth knowing

**Clipping goes through the target, not through arithmetic.** Intersecting
rectangles before drawing cannot clip *text*, because glyphs are rasterised by
the font. This backend wraps every draw in `DrawTargetExt::clipped`, which drops
out-of-area pixels inside the target. Doing it the other way is a bug this crate
shipped once: a list scrolled under the header painted its rows straight over
it, and only a screenshot test noticed.

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

See `examples/gallery/src/typeface.rs` for a picker built on it, and
`examples/gallery/src/fonts.rs` for what two extra families cost in flash.
