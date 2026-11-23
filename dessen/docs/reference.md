# Reference

The whole public API of `xpui-dessen`, crate name `xpui_dessen`, by area.
[The README](../README.md) runs a first app and [coverage.md](coverage.md) says
which dessen call answers each host trait; this is what you reach for once you
know the shape and want to know what exists.

## Topics

Each page lists every public name in its area with its declaration and what it
does. The gate checks every page against the code, so what a page says an item
is, the compiler agrees with.

| Group | Page | Holds |
|---|---|---|
| Host | [backend](reference/backend.md) | `Backend`, `Device`, `install` and `run` |
| Input | [input](reference/input.md) | the `input` module: how one dessen event becomes `xpui` frames |
| Doubles | [testing](reference/testing.md) | the `testing` module: dessen's calls off the VM, recorded |

Every `rust` block on these pages is compiled and run by the gate's doctests,
on the host, where dessen's calls are answered by the `testing` doubles.
