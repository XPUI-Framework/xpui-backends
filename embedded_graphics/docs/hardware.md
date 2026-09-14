# Hardware notes

What running the [`embedded-graphics`](https://crates.io/crates/embedded-graphics) backend on real panels established, kept
here so the code can state the conclusion in a sentence.

## Solid fills on a parallel bus: why `PacedFill` exists

An 8080-style parallel panel latches a byte on the rising edge of its write
strobe, and the controller has a minimum write *cycle* — 66 ns on the
ST7789v — that the strobe has to respect. When every pixel of a run has the
same byte in both halves, [`mipidsi`](https://crates.io/crates/mipidsi) sends the word once and then loops on
the strobe alone:

```text
wr.set_low();
wr.set_high();
```

Two register stores. On a 125 MHz [RP2040](https://www.raspberrypi.com/products/rp2040/) in release that is a write cycle of
roughly 24–40 ns — inside the controller's minimum, so it mislatches, and the
fill arrives as noise.

What costs enough time on the ordinary path is the call, not the pins.
`Generic8BitBus::set_value` returns early when the value is unchanged
(`mipidsi`'s `interface/parallel.rs`: *"quite common for multiple consecutive
values to be identical … so let's optimize for that case"*), so the pins are
skipped there too. `send_word` stays out of line at `opt-level = "z"`, and
that call is what stretches the cycle — incidental codegen, worth knowing
because inlining it would bring the fault back with nothing changed in the
wrapper.

The shortcut triggers whenever the two bytes of the pixel are identical,
which for `Rgb565` is 256 values and not two — `0x0000` and `0xFFFF` among
them, but `0x1818` is a dark blue that takes the same path. Ink and background
are two of the 256, so on a monochrome-styled panel every filled rectangle and
every screen clear breaks while text, drawn pixel by pixel, comes out
perfectly. A panel showing crisp type over static is this fault, and it reads
like a framework bug rather than a timing one.

`PacedFill` routes `fill_solid` and `clear` through `fill_contiguous`, which
has a colour for every pixel and therefore no run to shorten. A whole 320x240
screen is instruction-counted at roughly 120 ms that way — a loop interval is
10 ms, so this is not free — and **that figure has not been measured on the
board**. Measure it before quoting it.

The wrapper lives in this crate rather than beside the firmware that needs it
because it names no HAL, no pin and no board: it is a `DrawTarget` that wraps
a `DrawTarget`. Beside the firmware it is untestable, and the property it
exists for — that two methods are *not* forwarded — is invisible on the host
and in every snapshot, and only appears on hardware.
