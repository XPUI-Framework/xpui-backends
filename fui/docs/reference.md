# Reference

The whole public API of `xpui-fui`, crate name `xpui_fui`, by area.
[The README](../README.md) wires the backend up, [firmware.md](firmware.md)
adds the C++ half to a build, and [design.md](design.md) explains why it has
the shape it does; this is what you reach for once you know the shape and want
to know what exists.

## Topics

Each page lists every public name in its area with its declaration, examples
you can copy, and what it does. The gate checks every page against the code,
so what a page says an item is, the compiler agrees with.

| Page | Holds |
|---|---|
| [backend](reference/backend.md) | `Backend`, `Platform`, `NoInput`, `attach`, `register_screen!`, and which FreeInkUI component draws each piece of chrome |
| [lifecycle](reference/lifecycle.md) | the `lifecycle` module: the opaque handle a C++ host drives a screen through, and its six `extern "C"` entry points |
| [raw](reference/raw.md) | the `raw` module: every function of the C ABI the Rust half draws through, and `CellFn` |

Every `rust` block on these pages is compiled and run by the gate's doctests,
against the host doubles the `testing` feature supplies in place of the C++
half, so none of them needs a firmware to link.
