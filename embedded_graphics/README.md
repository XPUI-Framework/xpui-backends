[![CI](https://github.com/XPUI-Framework/xpui-backends/actions/workflows/ci.yml/badge.svg)](https://github.com/XPUI-Framework/xpui-backends/actions/workflows/ci.yml) [![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../LICENSE)

# `xpui-embedded-graphics`

An [`xpui`](https://github.com/XPUI-Framework/xpui-framework) backend that draws through any `embedded-graphics`
`DrawTarget` — which is most of the embedded Rust display ecosystem: e-paper
panels, SSD1306 and friends, colour TFTs, and the desktop simulator.

## Using it

```toml
[dependencies]
xpui-embedded-graphics = { git = "https://github.com/XPUI-Framework/xpui-backends", branch = "main" }
```

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

It supplies `Canvas` and `TextMetrics` itself — the contract for all five is
[`docs/host.md`](https://github.com/XPUI-Framework/xpui-framework/blob/main/docs/host.md) — `InputSource` and `Clock` from
whatever you feed it — bar `has_left_right_keys`, which is a fact about the
hardware and so is **told** to it: `with_left_right_keys(true)`. Left unsaid it
answers `false`, which costs a keystroke on a device that has the pair and is
the only direction that stays usable if it is wrong. It takes `Chrome` from
[`xpui-chrome`](https://github.com/XPUI-Framework/xpui-chrome/tree/main) — so a list, a dialog and a slider look like
something without you drawing one.

## Checking it

The gate is the repository's; run `./build-and-test.sh` from the root.

## Where next

| | |
|---|---|
| [`docs/reference.md`](docs/reference.md) | every public item: the backend, its input and palette, and the type it is set in |
| [`docs/design.md`](docs/design.md) | monochrome by design, the faces it ships, input, clipping through the target, and bringing your own type |
| [`docs/screenshots.md`](docs/screenshots.md) | render to memory and compare against a committed PNG, including why the first run fails |
| [`docs/hardware.md`](docs/hardware.md) | what a parallel-bus panel taught: why `PacedFill` exists |

## License

MIT — see [LICENSE](../LICENSE).
