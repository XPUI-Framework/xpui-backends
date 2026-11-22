# The dessen backend

The host an app installs, what it reads about the device, and the loop that
runs an app's screens.

[The README](../../README.md) runs a first app; [coverage.md](../coverage.md)
says which dessen call answers each host trait. This page is what each piece
does.

## Topics

| | |
|---|---|
| [`Backend`](#xpui_dessenbackend) | An `xpui` host over dessen's calls. |
| [`Device`](#xpui_dessendevice) | What the backend needs to know about the device, read from dessen. |
| [`main!`](#xpui_dessenmain) | Writes the app's entry point: installs the backend, builds the app with `$app`, a `fn() -> xpui::App`, and runs it until it finishes. |
| [`install`](#xpui_desseninstall) | Builds a backend for the running device and installs it as `xpui`'s host. |
| [`run`](#xpui_dessenrun) | Runs `app` until its last screen finishes, then returns 0 for the app to exit with. |

## `xpui_dessen::Backend`

An `xpui` host over dessen's calls.

```text
pub struct Backend
```

It implements all five host traits: `Canvas` and `TextMetrics` over dessen's
drawing calls, `Chrome` from xpui-chrome's painting with the button hints
left to the host (`dsn_button_hints`), and `InputSource`
and `Clock` from the frame [`run`](#xpui_dessenrun) feeds it. Most apps never
build one by hand: [`install`](#xpui_desseninstall) builds it for the running
device.

**Example — a backend for a device described by hand**

```rust
use xpui_dessen::{Backend, Device};

let touch_only = Device {
    width: 480,
    height: 800,
    has_touch: true,
    has_buttons: false,
    hint_height: 0,
};
let backend = Backend::new(touch_only);
assert_eq!(backend.device(), touch_only);
```

### Creating a backend

#### `xpui_dessen::Backend::new`

A backend for `device`, with chrome sized to its panel.

```text
pub fn new(device: Device) -> Backend
```

### Reading it back

#### `xpui_dessen::Backend::device`

The device this backend was built for.

```text
pub fn device(&self) -> Device
```

## `xpui_dessen::Device`

What the backend needs to know about the device, read from dessen.

```text
pub struct Device
```

### Fields

#### `xpui_dessen::Device::width`

The drawable area's width: the safe area, inside any bezel.

#### `xpui_dessen::Device::height`

The drawable area's height.

#### `xpui_dessen::Device::has_touch`

Whether taps and swipes arrive.

#### `xpui_dessen::Device::has_buttons`

Whether buttons arrive.

#### `xpui_dessen::Device::hint_height`

The height of the band along the bottom where the host draws button hints,
from `dsn_device`; 0 where it shows none (a touch board, a board without keys,
or keys not along the bottom as the screen is turned). The backend keeps that
band clear and hands the host each action's label with `dsn_button_hints`; the
host draws them in its own look, over its own keys. Where it is 0 no band is
reserved and no hints are asked for.

### Reading the device

#### `xpui_dessen::Device::read`

The running device, from `dsn_screen` and `dsn_device`.

```text
pub fn read() -> Device
```

## `xpui_dessen::main!`

Writes the app's entry point: installs the backend, builds the app with `$app`, a `fn() -> xpui::App`, and runs it until it finishes.

```text
macro_rules! main
```

```text
fn app() -> xpui::App {
    xpui::App::new(Hello)
}
xpui_dessen::main!(app);
```

`$app` runs after the backend is installed, so it may measure text or read the
theme while it builds the first screen. The app then needs no `unsafe` of its
own and can `#![forbid(unsafe_code)]`.

## `xpui_dessen::install`

Builds a backend for the running device and installs it as `xpui`'s host.

```text
pub fn install() -> &'static Backend
```

Call it once, before the first screen is built; `main!` does. The backend lives
for the rest of the app: in a `static` when it is linked (`--cfg
xpui_static_host`, as every dessen app is), leaked onto the heap when it is
installed.

## `xpui_dessen::run`

Runs `app` until its last screen finishes, then returns 0 for the app to exit with.

```text
pub fn run(backend: &Backend, app: App) -> i32
```

Paints the first frame, then for each dessen event feeds its frames and
refreshes the panel when anything repainted. Back on the root screen finishes
it, which is how a dessen app is expected to quit.

**Example — the app ends when Back leaves its only screen**

```rust
use xpui::{App, NavigationScreen, Screen, Text, View, vstack};
use xpui_dessen::testing;

struct Hello;
impl Screen for Hello {
    type Message = ();
    fn body(&self) -> impl View<()> {
        NavigationScreen::new(vstack![0; Text::new("Hello")])
    }
    fn update(&mut self, _message: ()) {}
}

let _guard = testing::lock();
let backend = xpui_dessen::install();
testing::events(&[dessen::DSN_EV_BACK as u32]);
assert_eq!(xpui_dessen::run(backend, App::new(Hello)), 0);
```

**See also:** [`input`](input.md)
