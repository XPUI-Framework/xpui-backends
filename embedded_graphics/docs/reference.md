# Reference

The whole public API of `xpui-embedded-graphics`, crate name `xpui_eg`, by
area. [The README](../README.md) wires a backend into a loop from nothing and
[design.md](design.md) explains its choices; this is what you reach for once
you know the shape and want to know what exists.

## Topics

Each page lists every public name in its area with its declaration, examples
you can copy, and what it does. The gate checks every page against the code,
so what a page says an item is, the compiler agrees with.

| Group | Page | Holds |
|---|---|---|
| Host | [backend](reference/backend.md) | `Backend`, `DisplayLoan`, `InputState`, `PacedFill`, `Palette`, and the re-exported `DrawTarget`, `Labels` and `Metrics` |
| Type | [fonts](reference/fonts.md) | `Fonts`, `Family`, `Tier`, `Face`, `HELVETICA`, `font_tier!`, `font_id`, `request_family`, `clear_chosen_family`, `Piece`, `pieces`, `advance`, and the re-exported `Font`, `FontRenderer` and `u8g2` |

Every `rust` block on these pages is compiled and run by the gate's doctests.
The golden-image harness a backend is tested with is
[`xpui-screenshot`'s reference](../../screenshot/docs/reference.md).
