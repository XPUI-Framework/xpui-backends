# Adding the shim to a firmware

`cpp/xpui_fui.cpp` implements the C ABI in `cpp/xpui_fui.h` against the
FreeInk SDK's UI library. Why it binds to `freeink::ui::DisplayTarget`, and
why that makes it firmware-agnostic, is
[`../cpp/README.md`](../cpp/README.md); this page is the whole of what a
firmware does with it.

## Adding it to a firmware

You compile two sources and add one include path:

| | |
| --- | --- |
| sources | `xpui_fui.cpp`, plus FreeInkUI's own `src/FreeInkUI.cpp` |
| includes | `<freeink-sdk>/libs/ui/FreeInkUI/include`, and `cpp/`, where the header is |
| standard | C++17 or later; `-fno-exceptions` and `-fno-rtti` are fine |

FreeInkUI is otherwise header-only, but `src/FreeInkUI.cpp` holds the default
styles, the theme tokens and the list/layout helpers this file calls, so it has
to be in the link.

PlatformIO, with the SDK already in `lib_deps`:

```ini
build_flags =
  -I xpui-backends/fui/cpp
  -I freeink-sdk/libs/ui/FreeInkUI/include
build_src_filter =
  +<*>
  +<../xpui-backends/fui/cpp/xpui_fui.cpp>
```

CMake:

```cmake
target_sources(firmware PRIVATE
  xpui-backends/fui/cpp/xpui_fui.cpp
  freeink-sdk/libs/ui/FreeInkUI/src/FreeInkUI.cpp)
target_include_directories(firmware PRIVATE
  xpui-backends/fui/cpp
  freeink-sdk/libs/ui/FreeInkUI/include)
```

## Wiring it up

Two calls at startup, one symbol to define.

```cpp
#include <stdint.h>

#include "xpui_fui.h"

// 1 bit per pixel, MSB first, (width + 7) / 8 bytes per row, a SET bit is
// WHITE. Must stay valid for as long as anything draws — a static, or
// whatever your panel driver already owns.
static uint8_t framebuffer[(480 + 7) / 8 * 800];

void wire_up_the_panel(void) { xpui_fui_attach(framebuffer, 480, 800); }
```

Coordinates are logical: the framebuffer you hand over is already in the frame
the UI lays out in, so rotate on the way to the glass, not on the way in.

`xpui_fui_request_update` pushes the frame through whichever of two mechanisms
you chose. **Prefer the hook**: whether a strong definition beats a weak one
depends on the object format, and the failure is silent — a panel that never
updates.

```cpp
#include "xpui_fui.h"

namespace {

// Raised here, acted on after the frame. **Do not blit in this function**:
// `xpui_fui_request_update` is called from inside the input phase, before
// anything has been painted, so a present here pushes the previous frame.
bool g_updateRequested = false;

void present() { g_updateRequested = true; }

}  // namespace

void install_the_present_hook(void) { xpui_fui_set_present(&present); }
```

Then, once the frame has been rendered, push it:

```text
freeink::ui::present(display, freeink::ui::RefreshHint::Fast);
```

Fenced `text` rather than `cpp` because it cannot be compiled here: the SDK
guards its panel helpers behind `__has_include(<EInkDisplay.h>)`, which is
absent on a host, and `present` takes an `EInkDisplay&` rather than the
`DisplayTarget` this shim draws through. It compiles in a firmware and nowhere
else.

Overriding the weak `xpui_fui_present` still works, and `xpui-cpp`
proves it does on this linker — but the hook behaves the same everywhere.

## What this build does not ship

- **Icons.** `xpui_fui_icon_size` returns 0 for every role, which the ABI
  defines as "reserve no space". The Icons library is a separate opt-in
  (`FreeInkUIIcon.h`); wiring it up means resolving a role to a `BitmapRef` in
  `xpui_fui_draw_icon`.
- **A second face.** Every `DisplayTarget` slot defaults to the bundled font, so
  bold and italic render as regular and the reading face is the UI face. Call
  `setFont(slot, yourFont)` on the target to change that — slots 0, 1 and 3 are
  small interface, interface, and reading.
- **Button reordering.** `xpui_fui_draw_button_hints` draws its four slots in
  the order the ABI names them. A firmware that lets the user remap its front
  buttons reorders the four arguments before calling.

## Checking it compiles

From `cpp/`, where this repository's half sits — FreeInkUI's own source
comes from the SDK:

```sh
FUI=<freeink-sdk>/libs/ui/FreeInkUI

clang++ -std=c++17 -fsyntax-only -fno-exceptions -fno-rtti -Wall -Wextra \
  -I "$FUI/include" -I . xpui_fui.cpp
```

Clean, no warnings, on Apple clang 17. To also link and run something, add a
`main` that attaches a `malloc`'d buffer and build all three sources:

```sh
clang++ -std=c++17 -fno-exceptions -fno-rtti -I "$FUI/include" -I . \
  your_main.cpp xpui_fui.cpp "$FUI/src/FreeInkUI.cpp" -o smoke
```
