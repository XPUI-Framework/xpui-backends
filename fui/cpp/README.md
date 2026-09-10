[![CI](https://github.com/XPUI-Framework/xpui-backends/actions/workflows/ci.yml/badge.svg)](https://github.com/XPUI-Framework/xpui-backends/actions/workflows/ci.yml) [![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

# xpui on FreeInkUI

`xpui_fui.cpp` implements the C ABI in `xpui_fui.h` against the FreeInk SDK's
UI library. It binds to `freeink::ui::DisplayTarget`, which needs nothing but a
raw 1-bit framebuffer and ships its own Noto Sans bitmap font, so this file is
firmware-agnostic: any board that can hand over a framebuffer can host xpui.

## Using it

The header is the contract. It, `../src/raw.rs`, this file and the Rust
doubles move together. A firmware compiles two sources — `xpui_fui.cpp` and
FreeInkUI's own `src/FreeInkUI.cpp` — with `<freeink-sdk>/libs/ui/FreeInkUI/include`
and this directory on the include path, C++17 or later. Two calls at startup
wire it up: `xpui_fui_attach` with the panel's framebuffer, and
`xpui_fui_set_present` with the function that pushes a frame.
[`../docs/firmware.md`](../docs/firmware.md) has the build lines, the wiring,
and what this build does not ship.

## Checking it

The gate is the repository's; run `./build-and-test.sh` from the root. Its
`the shim compiles` stage compiles this file against the SDK named by
`FREEINK_SDK_INCLUDE`, and skips with a note when that is unset.

## Where next

| | |
|---|---|
| [`../docs/firmware.md`](../docs/firmware.md) | adding it to a firmware, wiring it up, what the build does not ship, checking it compiles by hand |
| [`../docs/coverage.md`](../docs/coverage.md) | which FreeInkUI components this file uses, and which it does not |

## License

MIT — see [LICENSE](../../LICENSE).
