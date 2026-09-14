# Documentation

[`../README.md`](../README.md) is the front page. Every document in this
repository, and what each is for:

| | |
|---|---|
| [choosing-a-backend.md](choosing-a-backend.md) | when `embedded_graphics`, when FreeInkUI, and what a third would need |
| [contributing.md](contributing.md) | building it, the gate, the five review steps, and how a commit is written |
| [embedded_graphics/docs/reference.md](../embedded_graphics/docs/reference.md) | the `xpui-embedded-graphics` reference: its index |
| [embedded_graphics/docs/reference/backend.md](../embedded_graphics/docs/reference/backend.md) | `Backend`, `DisplayLoan`, `InputState`, `PacedFill`, `Palette` |
| [embedded_graphics/docs/reference/fonts.md](../embedded_graphics/docs/reference/fonts.md) | `Fonts`, `Family`, `Tier`, `Face`, `Piece`, `font_tier!`, and switching the family |
| [embedded_graphics/docs/design.md](../embedded_graphics/docs/design.md) | monochrome by design, the faces it ships, input, clipping, and type |
| [embedded_graphics/docs/screenshots.md](../embedded_graphics/docs/screenshots.md) | pixel tests against a committed PNG, and why the first run fails |
| [embedded_graphics/docs/hardware.md](../embedded_graphics/docs/hardware.md) | what a parallel-bus panel taught: why `PacedFill` exists |
| [fui/docs/reference.md](../fui/docs/reference.md) | the `xpui-fui` reference: its index |
| [fui/docs/reference/backend.md](../fui/docs/reference/backend.md) | `Backend`, `Platform`, `NoInput`, `attach`, `register_screen!`, and what draws each piece of chrome |
| [fui/docs/reference/lifecycle.md](../fui/docs/reference/lifecycle.md) | the handle a C++ host drives a screen through, and its six entry points |
| [fui/docs/reference/raw.md](../fui/docs/reference/raw.md) | every function of the C ABI, and `CellFn` |
| [fui/docs/design.md](../fui/docs/design.md) | why the FreeInkUI backend has the shape it does, the boundary, input, and the distinctions that break quietly |
| [fui/docs/coverage.md](../fui/docs/coverage.md) | which FreeInkUI components the shim uses, and which it does not |
| [fui/docs/firmware.md](../fui/docs/firmware.md) | adding the shim to a firmware: sources, includes, wiring, what the build does not ship |
| [screenshot/docs/reference.md](../screenshot/docs/reference.md) | the `xpui-screenshot` reference: `Framebuffer` and golden comparison |
