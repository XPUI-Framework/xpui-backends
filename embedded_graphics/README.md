# `xpui-embedded-graphics`

An [`xpui`](../../xpui/) backend that draws through any `embedded-graphics`
`DrawTarget` — which is most of the embedded Rust display ecosystem: e-paper
panels, SSD1306 and friends, colour TFTs, and the desktop simulator.

```rust
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
whatever you feed it, and takes `Chrome` from
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

## Input

`embedded-graphics` knows nothing about buttons or fingers. `InputState` is the
buffer between your event source and the framework:

```rust
backend.begin_frame(millis);        // clears one-frame edges, sets the clock
backend.press(Button::Confirm);     // an edge: true for exactly this frame
backend.tap(Point::new(x, y));
backend.input(|state| state.touch_down(at));   // everything else
```

Everything except held buttons is edge state, cleared by `begin_frame`. A
button reported as pressed on every frame re-fires whatever it is on.

## Screenshot tests, with the `framebuffer` feature

```toml
xpui-embedded-graphics = { version = "0.1", features = ["framebuffer"] }
```

`Framebuffer` is a plain 1-bit `DrawTarget` with no window and no hardware, so
a screen can be rendered and asserted on in an ordinary `cargo test`:

```rust
backend.with_display(|frame| {
    frame.write_bmp("my_screen");           // a BMP you can open
    println!("{}", frame.thumbnail(60));    // an ASCII view small enough to diff
    assert!(frame.ink_in(0, 0, 480, 56) > 0);
});
```

That is how this crate's own tests work — see `tests/screenshots.rs`.

## One thing worth knowing

**Clipping goes through the target, not through arithmetic.** Intersecting
rectangles before drawing cannot clip *text*, because glyphs are rasterised by
the font. This backend wraps every draw in `DrawTargetExt::clipped`, which drops
out-of-area pixels inside the target. Doing it the other way is a bug this crate
shipped once: a list scrolled under the header painted its rows straight over
it, and only a screenshot test noticed.
