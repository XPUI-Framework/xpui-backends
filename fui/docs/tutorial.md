# Rust screens in a C++ firmware

You have a C++ firmware. You want to write its next screen in Rust, without
rewriting the firmware and without the two halves drifting apart.

This walks the whole boundary once. Every Rust block below is compiled by
`cargo test`, and every C++ block is compiled by `./build-and-test.sh` — so a
snippet that stops being true fails the build rather than quietly misleading
whoever reads it next.

The finished thing is [`examples/cpp_host`](../../../../examples/cpp_host/),
which runs in a window, and
[`examples/firmware`](../../../../examples/firmware/), which is the same code
in an ESP32 image. Read either beside this.

## The shape of it

Four sets of C symbols, two crossing each way. That is the whole design, and
everything below is one of these four boxes:

```text
   your C++                                          Rust
   ────────                                          ────
   main, screen stack, input, panel
        │
        ├── xpui_screen_*  drive a screen ────────►  xpui-fui defines these
        ├── xpui_app_*     make a screen ─────────►  your crate defines these
        │
        ◄── xpui_fui_*     draw ──────────────────── xpui-fui calls these
        ◄── xpui_host_*    input, i18n, device ───── your crate calls these
```

The two arrows pointing left are things **you** implement in C++. The two
pointing right are things Rust gives you. Nothing else crosses.

---

## 1. Write a screen

A screen is a struct with a `body()` and an `update()`. Nothing about it knows
it will be driven from C++ — this is the same code the desktop simulator runs.

```rust
use xpui::screen::Screen;
use xpui::{List, ListRow, NavigationScreen, View};

pub struct Settings {
    wifi: bool,
}

#[derive(Clone, Copy)]
pub enum Message {
    ToggleWifi,
}

impl Settings {
    pub fn new() -> Self {
        Settings { wifi: false }
    }
}

impl Screen for Settings {
    type Message = Message;

    fn body(&self) -> impl View<Message> {
        NavigationScreen::new(
            List::new().push(
                ListRow::new("Wi-Fi")
                    .value(if self.wifi { "On" } else { "Off" })
                    .on_tap(Message::ToggleWifi),
            ),
        )
        .title("Settings")
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::ToggleWifi => self.wifi = !self.wifi,
        }
    }
}
```

If you have written a screen for the simulator before, this is that. Skip to
step 2.

## 2. Export it to C++

C++ cannot name a Rust type, so it never sees one. It gets an opaque handle.

```rust
use xpui::screen::Screen;
use xpui::{NavigationScreen, Text, View};
use xpui_fui::register_screen;

# pub struct Settings;
# impl Settings { pub fn new() -> Self { Settings } }
# impl Screen for Settings {
#     type Message = ();
#     fn body(&self) -> impl View<()> {
#         NavigationScreen::new(Text::new("Settings")).title("Settings")
#     }
#     fn update(&mut self, _message: ()) {}
# }
register_screen!(Settings, xpui_app_create_settings);
```

That macro is the whole of it. It exports one C function:

```text
void* xpui_app_create_settings(void);
```

Declare it in a header your C++ includes — `examples/cpp_host/cpp/xpui_app.h`
is the worked example — and you have a screen your firmware can hold.

> Fenced `text` rather than `cpp`, because it is a declaration of a symbol the
> Rust side defines: there is nothing here for a C++ compiler to check that the
> linker will not check better.

## 3. Drive it

Six calls, declared in [`cpp/xpui_screen.h`](../cpp/xpui_screen.h) and
**defined in Rust**. Your C++ calls them; you implement none of them.

```cpp
#include <xpui_screen.h>

// The smallest thing that can run a screen. `examples/cpp_host`'s ScreenHost
// is this plus a title and the globals; a firmware's activity class is this
// plus whatever it already had.
class Screen {
 public:
  explicit Screen(void* handle) : handle_(handle) { xpui_screen_on_enter(handle_); }

  ~Screen() {
    xpui_screen_on_exit(handle_);
    xpui_screen_destroy(handle_);
  }

  // One frame. Input first, then paint — a screen asks for its repaint from
  // inside loop(), so painting first shows the frame before the one it wanted.
  void frame(bool paint) {
    xpui_screen_loop(handle_);
    if (paint) {
      xpui_screen_render(handle_);
    }
  }

 private:
  void* handle_;
};
```

The handle is a `void*` and must stay one. It is a double-boxed trait object on
the Rust side, so unwrapping it once too few is a wild pointer rather than a
type error — which is why nothing on this side ever looks inside it.

## 4. Answer what the framework asks

Two headers' worth, and this is the part that is genuinely yours.

`xpui_fui.h` is drawing: 30 functions, and
[`cpp/xpui_fui.cpp`](../cpp/xpui_fui.cpp) already implements every one of them
against FreeInkUI. **Add that file to your build and you are done with it.** A
firmware with its own themed renderer can implement the same header instead —
that is a supported path, not a fork.

`xpui_host.h` is everything else: input, translations, device identity, the
heap. Nobody can implement those for you, because they are your firmware. They
are small:

```cpp
#include <stdint.h>

#include <xpui_host.h>

// Your input manager already knows this. `button` is xpui::Button's own
// discriminant — 0 Back, 1 Confirm, 2 Left, 3 Right, 4 Up, 5 Down, and so on.
uint8_t xpui_host_was_pressed(uint8_t button) {
  // return my_input.wasPressed(my_button_for(button));
  (void)button;
  return 0;
}

// The only clock the framework has. It times key auto-repeat, and its contract
// is that it may wrap — what reads it measures short intervals.
uint32_t xpui_host_millis(void) {
  // return millis();
  return 0;
}
```

## 5. Start it

Three calls, in this order, once:

```cpp
#include <stdint.h>

#include <xpui_app.h>
#include <xpui_fui.h>

// 1 bit per pixel, MSB first, (width + 7) / 8 bytes per row, and **a set bit
// is white** — FreeInkUI's convention, the inverse of the usual one.
static uint8_t framebuffer[(480 + 7) / 8 * 800];

void start_the_ui(void) {
  // Where the panel is. The shim draws into this and nothing else.
  xpui_fui_attach(framebuffer, 480, 800);

  // How a finished frame reaches the glass. Prefer this to overriding the weak
  // `xpui_fui_present`: whether an override wins depends on the object format,
  // and the failure is silent — a panel that never updates.
  xpui_fui_set_present([]() { /* mark dirty; blit after rendering, not here */ });

  // What paints, and what owns the screen stack. Before the first screen.
  xpui_app_install();
}
```

The ordering trap is worth stating plainly, because nothing checks it:
`xpui_fui_attach` tells the **shim** where the panel is, and `xpui_app_install`
tells the **framework** where the shim is. Neither works without the other, and
missing one is a blank screen with no error.

## 6. Build it

The Rust half is a `staticlib`. Your C++ links the archive:

```text
cargo build --release -p your-crate --target <your-triple>
# -> target/<your-triple>/release/libyour_crate.a
```

Two worked examples, and they differ only in the build system:

| | |
|---|---|
| CMake, on a desktop | [`examples/cpp_host/CMakeLists.txt`](../../../../examples/cpp_host/CMakeLists.txt) |
| PlatformIO, on an ESP32 | [`examples/firmware/scripts/build_rust.py`](../../../../examples/firmware/scripts/build_rust.py) |

Both call `cargo` in plain sight rather than through a helper, because forty
lines you can read transplant into another build system and a dependency that
hides the mechanism does not.

**Pass `--target` explicitly even for the host**, or cargo writes to
`target/release/` instead of `target/<triple>/release/` and your link path is
wrong. And note the trap beside it: the `dev` profile's directory is `debug`,
not `dev`.

## What changes for a real firmware

Not much, which is the point. `examples/firmware` is the same Rust — the same
crate, not a fork — and five C++ files:

| | |
|---|---|
| `main.cpp` | your firmware already has one |
| `host_input.cpp` | wire it to your input manager |
| `host_device.cpp` | your board configuration |
| `host_heap.cpp` | your RTOS |
| `host_panic.cpp` | your log |

Two things do change, and both are improvements:

**Rust allocates from your heap.** Give it a `#[global_allocator]` that calls
your `malloc`, as
[`examples/cpp_host/src/runtime.rs`](../../../../examples/cpp_host/src/runtime.rs)
does, and `xpui_host_heap_*` then reports figures covering both languages. It
is the only visibility you get into what Rust costs at run time — a build-time
size report measures static sections, where it contributes almost nothing.

**Fragmentation becomes measurable.** On a desktop `largest_block` answers "I
cannot tell you". Your RTOS can, and on a device with no MMU the gap between
"free" and "largest block" is what actually decides whether the next allocation
fails.

## The one rule that keeps it working

**Every C symbol lives in more than one place, and they move together.** The
drawing ABI is a header, a Rust declaration, a C++ definition and a host double
for tests; the host ABI is a header, a Rust declaration and your C++.

Miss one and you get a link error, which is loud. Get one *wrong* — a parameter
reordered, an `int32_t` narrowed — and it links perfectly and corrupts the call
frame, because C has no mangling to disagree with. The symptom is a rendering
fault somewhere unrelated.

Two checks stand behind that, and they are worth copying if you fork this:

- `crates/backend/fui/tests/abi.rs` parses each header and the Rust beside it
  and compares **signatures**, across five boundaries. An unrecognised C type
  fails the run rather than being skipped.
- `ffi_symbols_agree()` in `build-and-test.sh` covers the half it cannot read:
  a header against the C++ that defines it.
