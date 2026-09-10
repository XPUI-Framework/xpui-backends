# `xpui-fui`

> ⚠️ **Under heavy development.** Not production-ready. The API can break
> without notice. Use at your own risk.

An [`xpui`](https://github.com/XPUI-Framework/xpui-framework) backend that draws through **FreeInkUI**.

FreeInkUI is a header-only C++ component library for e-ink firmware. It already
knows what a list row, a dialog and a slider look like — so a screen written
against `xpui` and one written in C++ against FreeInkUI come out as the same
pixels. That is the whole reason to sit on it rather than beside it. Two
halves: `src/` is the Rust `Host` implementation over a C ABI, and `cpp/` is
that ABI implemented against FreeInkUI.

## Using it

```toml
[dependencies]
xpui-fui = { git = "https://github.com/XPUI-Framework/xpui-backends", branch = "main" }
```

```rust,no_run
# use xpui::Button;
# use xpui_fui::{Backend, Platform};
# struct MyPlatform;
# impl Platform for MyPlatform {
#     fn millis(&self) -> u32 { 0 }
#     fn was_pressed(&self, _button: Button) -> bool { false }
#     fn is_pressed(&self, _button: Button) -> bool { false }
#     fn was_released(&self, _button: Button) -> bool { false }
#     fn has_left_right_keys(&self) -> bool { false }
# }
# let mut framebuffer = [0u8; 480 * 800 / 8];
// Once, with the panel's framebuffer.
unsafe { xpui_fui::attach(framebuffer.as_mut_ptr(), 480, 800) };

static PLATFORM: MyPlatform = MyPlatform;
static BACKEND: Backend<MyPlatform> = Backend::new(&PLATFORM);
unsafe { xpui::host::install(&BACKEND) };
```

Add `cpp/xpui_fui.cpp` to the firmware's build with FreeInkUI's include
directory on the path — [`docs/firmware.md`](docs/firmware.md) has the two
lines for PlatformIO and for CMake. `Platform` is the one thing this crate
cannot supply: whatever drives the panel already knows whether a button was
pressed, so it implements those five methods and the backend forwards to it.

## Requirements

The C++ half needs the FreeInkUI headers, `<freeink-sdk>/libs/ui/FreeInkUI/include`,
and a C++17 compiler. The Rust half needs nothing; its tests run against
doubles, with no firmware to link.

## Checking it

The gate is the repository's; run `./build-and-test.sh` from the root. The
shim's compile stage needs `FREEINK_SDK_INCLUDE` set and skips with a note
otherwise.

## Where next

| | |
|---|---|
| [`docs/design.md`](docs/design.md) | why this backend has the shape it does, the boundary's four places, input, and the distinctions that break things quietly |
| [`docs/coverage.md`](docs/coverage.md) | which FreeInkUI components the shim uses, which it draws itself, and which it leaves alone |
| [`docs/firmware.md`](docs/firmware.md) | adding the shim to a firmware: sources, includes, wiring, what the build does not ship |
| [`cpp/`](cpp/) | the C++ half, and its own README |

## License

MIT — see [LICENSE](../LICENSE).
