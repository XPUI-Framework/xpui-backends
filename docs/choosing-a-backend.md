# Choosing a backend

Which of the two backends you want depends on one question: does something
already own your panel?

## No — I have a `DrawTarget`

Take [`xpui-embedded-graphics`](../embedded_graphics/). It draws every pixel
itself through any `embedded-graphics` `DrawTarget` — a driver crate for an
e-paper panel, an OLED over SPI, a colour TFT, the desktop simulator's window.
It supplies `Canvas` and `TextMetrics` from the u8g2 faces it ships, takes
`Chrome` from `xpui-chrome`, and takes `InputSource` and `Clock` from whatever
you feed it. Nothing else has to exist: a bare-metal firmware with a panel
driver and a button poll is a complete host.

It is monochrome by design and generic over `PixelColor` anyway: a `Palette`
of two colours maps ink and background onto whatever the panel takes.

## Yes — a C++ firmware draws through FreeInkUI

Take [`xpui-fui`](../fui/). A firmware that already owns the screen and paints
its own screens through FreeInkUI has a component library that knows what a
list row, a dialog and a slider look like. The backend calls that library over
a C ABI, so a Rust screen and a native one come out as the same pixels, in the
user's theme, with no second drawing path. The firmware supplies input and the
clock through one trait, `Platform`, and adds one C++ file to its build.

A firmware with its own themed renderer can implement the same ABI itself
rather than binding to FreeInkUI; that is a supported path, not a fork.

## Writing a third

`xpui`'s [`docs/writing-a-backend.md`](https://github.com/XPUI-Framework/xpui-framework/blob/main/docs/writing-a-backend.md)
is the path through the five traits, and the obligations the compiler cannot
check sit on the methods that carry them: `draw_text` takes a top-left origin,
not a baseline, and a font id of `0` means none, so a backend whose own
handles start at 0 offsets them. Both are wrong in a way that compiles. [`xpui-screenshot`](../screenshot/)
is how a new backend proves what it painted, and [`xpui-abi-check`](../abi-check/)
is how one that crosses a C boundary proves its signatures agree.
