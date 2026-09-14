# Backend

The Rust half of the FreeInkUI backend: the host a firmware installs, the
platform trait it supplies input and the clock through, the call that points
the C++ half at a framebuffer, and the macro that exports a screen for a C++
host to create.

[The README](../../README.md) wires it up and [firmware.md](../firmware.md)
adds the C++ half to a build. This page is what each piece does.

## Topics

| | |
|---|---|
| [`Backend`](#xpui_fuibackend) | The backend, stateless because everything it needs lives on the C++ side, bound once by `attach`. |
| [`attach`](#xpui_fuiattach) | Points the C++ side at the panel's framebuffer. |
| [`Platform`](#xpui_fuiplatform) | Buttons, gestures and the clock, which FreeInkUI has no opinion about. |
| [`NoInput`](#xpui_fuinoinput) | A platform with no input at all, for a panel that only ever displays. |
| [`register_screen!`](#xpui_fuiregister_screen) | Exports a C factory for a screen, so a C++ host can create one by name. |

## `xpui_fui::Backend`

The backend, stateless because everything it needs lives on the C++ side, bound once by [`attach`](#xpui_fuiattach).

```text
pub struct Backend<P: Platform + 'static>
```

It implements all five host traits: `Canvas`, `TextMetrics` and `Chrome` by
calling the [C ABI](raw.md), `InputSource` and `Clock` by forwarding to `P`.
Because it holds only a `&'static P`, it can be a `static` itself, built in
`const` context.

**Example — wiring it up**

```rust
use xpui_fui::{Backend, NoInput};

static PLATFORM: NoInput = NoInput;
static BACKEND: Backend<NoInput> = Backend::new(&PLATFORM);

let mut framebuffer = vec![0u8; (480 + 7) / 8 * 800];
// Safety: the buffer is the size `attach` asks for and outlives every draw.
unsafe { xpui_fui::attach(framebuffer.as_mut_ptr(), 480, 800) };
// Safety: one thread, and nothing has rendered yet.
unsafe { xpui::host::install(&BACKEND) };
```

### What draws each piece of chrome

`xpui` asks a backend for eight pieces of themed furniture. **Where FreeInkUI
has a component, the shim calls it. Where it has none, the shim draws it from
`DisplayTarget` primitives.** Calling the component is what makes a screen
written against `xpui` and one written in C++ against FreeInkUI the same
pixels.

| `Chrome` call | Asked for by | Drawn by |
|---|---|---|
| `draw_header` | `NavigationScreen`, `OverlayPanel` | `fui::header` — `controls/header.h` |
| `draw_list` | `List` | `fui::Screen::list` — `lists/list.h`, through `FreeInkApp.h` |
| `draw_option_popup`, `option_popup_row_rect` | `Modal` | `fui::optionDialog` and `fui::optionDialogHeight` — `overlays/option-dialog.h` |
| `draw_slider` | `Slider` | `fui::slider` — `controls/slider.h` |
| `draw_progress_bar` | `ProgressBar` | `fui::progressBar` — `controls/progress-bar.h` |
| `draw_scroll_indicator` | `ScrollView` | `fui::drawListScrollIndicator` — a helper in `lists/list.h`, not a component |
| `draw_sub_header` | `Section` | **the shim** — FreeInkUI has nothing for it |
| `draw_button_hints` | `NavigationScreen` | **the shim** — FreeInkUI has nothing for it |

Text, fills, strokes, lines, the dither, the scrim and bitmaps land on
`freeink::ui::DisplayTarget` directly, and so does clipping, which FreeInkUI
does not have. This build ships no icons.

### Creating a backend

#### `xpui_fui::Backend::new`

A backend forwarding input and the clock to `platform`.

```text
pub const fn new(platform: &'static P) -> Self
```

**See also:** [`attach`](#xpui_fuiattach), [`Platform`](#xpui_fuiplatform), [`lifecycle`](lifecycle.md)

## `xpui_fui::attach`

Points the C++ side at the panel's framebuffer.

```text
pub unsafe fn attach(framebuffer: *mut u8, width: i32, height: i32)
```

Call it once, before installing the backend. It forwards to
[`xpui_fui_attach`](raw.md).

| Parameter | Meaning |
|---|---|
| `framebuffer` | 1 bit per pixel, MSB first, `(width + 7) / 8` bytes per row, and a **set bit is white**. |
| `width`, `height` | The panel in pixels, in the frame the UI lays out in: rotate on the way to the glass, not on the way in. |

> [!WARNING]
> **Safety.** `framebuffer` must be writable for `(width + 7) / 8 * height`
> bytes and must outlive every subsequent draw.

## `xpui_fui::Platform`

Buttons, gestures and the clock, which FreeInkUI has no opinion about.

```text
pub trait Platform: Sync
```

A drawing library cannot tell whether a button was pressed. Whatever drives
the panel already knows, a firmware's input manager or a GPIO poll, so it
implements this and the backend forwards to it. `Sync` because the backend is
a `static`. Implemented here by [`NoInput`](#xpui_fuinoinput).

**Example — a firmware's buttons**

```rust
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use xpui::Button;
use xpui_fui::{Backend, Platform};

struct Firmware {
    millis: AtomicU32,
    confirm_down: AtomicBool,
    confirm_edge: AtomicBool,
}

impl Platform for Firmware {
    fn millis(&self) -> u32 {
        self.millis.load(Ordering::Relaxed)
    }
    fn was_pressed(&self, button: Button) -> bool {
        matches!(button, Button::Confirm) && self.confirm_edge.load(Ordering::Relaxed)
    }
    fn is_pressed(&self, button: Button) -> bool {
        matches!(button, Button::Confirm) && self.confirm_down.load(Ordering::Relaxed)
    }
    fn was_released(&self, _button: Button) -> bool {
        false
    }
    fn has_left_right_keys(&self) -> bool {
        false // a device with only Confirm, Up and Down
    }
}

static FIRMWARE: Firmware = Firmware {
    millis: AtomicU32::new(0),
    confirm_down: AtomicBool::new(false),
    confirm_edge: AtomicBool::new(false),
};
static BACKEND: Backend<Firmware> = Backend::new(&FIRMWARE);
```

### Required methods

#### `xpui_fui::Platform::millis`

Milliseconds since some fixed point; only differences are read.

```text
fn millis(&self) -> u32
```

#### `xpui_fui::Platform::was_pressed`

Whether `button` went down this frame.

```text
fn was_pressed(&self, button: Button) -> bool
```

#### `xpui_fui::Platform::is_pressed`

Whether `button` is down, this frame included.

```text
fn is_pressed(&self, button: Button) -> bool
```

#### `xpui_fui::Platform::was_released`

Whether `button` came up this frame.

```text
fn was_released(&self, button: Button) -> bool
```

#### `xpui_fui::Platform::has_left_right_keys`

Whether the device has a Left/Right pair to nudge a value with; see `InputSource::has_left_right_keys`.

```text
fn has_left_right_keys(&self) -> bool
```

Required, unlike everything below it: no answer is safe to inherit, and only
the firmware knows which keys its device carries. The `xpui-boards-*` crates
answer it for every board they describe, as `Board::has_left_right_keys`.

### Provided methods

Each answers "nothing happened" unless overridden, which is right for a device
without a touchscreen.

#### `xpui_fui::Platform::has_touch`

Whether *this frame* carries a touch — not whether the device has a touchscreen.

```text
fn has_touch(&self) -> bool
```

#### `xpui_fui::Platform::tap`

A completed tap, at the position the finger went down.

```text
fn tap(&self) -> Option<Point>
```

#### `xpui_fui::Platform::touch_held`

Where the finger is while it is down.

```text
fn touch_held(&self) -> Option<Point>
```

#### `xpui_fui::Platform::touch_released`

Whether a finger lifted this frame.

```text
fn touch_released(&self) -> bool
```

#### `xpui_fui::Platform::swipe`

A completed swipe, or `SwipeDir::None`.

```text
fn swipe(&self) -> SwipeDir
```

#### `xpui_fui::Platform::was_back_gesture`

The system back gesture.

```text
fn was_back_gesture(&self) -> bool
```

#### `xpui_fui::Platform::was_home_gesture`

The system home gesture, offered to the screen before the host acts.

```text
fn was_home_gesture(&self) -> bool
```

#### `xpui_fui::Platform::swipe_moves_selection`

Whether a swipe moves focus rather than dragging content; see `InputSource::swipe_moves_selection`.

```text
fn swipe_moves_selection(&self) -> bool
```

## `xpui_fui::NoInput`

A platform with no input at all, for a panel that only ever displays.

```text
pub struct NoInput
```

Its clock stands at `0`, no button is ever pressed, and it has no Left/Right
pair. See the example under [`Backend`](#xpui_fuibackend).

## `xpui_fui::register_screen!`

Exports a C factory for a screen, so a C++ host can create one by name.

```text
macro_rules! register_screen
```

```text
register_screen!(ScreenType, factory_name);
```

It expands to a `#[no_mangle] extern "C" fn factory_name() -> *mut c_void`
that builds the screen with `ScreenType::new()` and returns it as a
[lifecycle handle](lifecycle.md). The screen type never crosses the boundary,
so the host needs no header describing it: adding a screen is this line plus
one declaration on the C++ side.

```rust
use xpui::screen::Screen;
use xpui::{NavigationScreen, Text, View};
use xpui_fui::register_screen;

struct MainMenu;

impl MainMenu {
    fn new() -> Self {
        MainMenu // the macro calls this
    }
}

impl Screen for MainMenu {
    type Message = ();

    fn body(&self) -> impl View<()> {
        NavigationScreen::new(Text::new("Main menu")).title("Menu")
    }

    fn update(&mut self, _message: ()) {}
}

register_screen!(MainMenu, xpui_app_create_main_menu);

let handle = xpui_app_create_main_menu();
assert!(!handle.is_null());
// Safety: a live handle from the factory, destroyed exactly once.
unsafe { xpui_fui::lifecycle::xpui_screen_destroy(handle) };
```
