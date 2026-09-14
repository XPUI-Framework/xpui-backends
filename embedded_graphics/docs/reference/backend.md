# The embedded-graphics backend

The host itself: a `Backend` over any [`embedded-graphics`](https://crates.io/crates/embedded-graphics) `DrawTarget`, the
input a caller feeds it each frame, the two colours ink and background land
as, and the wrapper a parallel-bus panel needs. Everything a screen draws goes
through one of these, and everything it hears about buttons and fingers comes
in through one.

[The crate's README](../../README.md) wires one into a loop from nothing, and
[`design.md`](../design.md) says why it has the shape it does. This page is
what each piece does.

## Topics

| | |
|---|---|
| [`Backend`](#xpui_egbackend) | An `xpui` host over an `embedded-graphics` display. |
| [`DisplayLoan`](#xpui_egdisplayloan) | A display that has been taken out of its backend, and goes back on drop. |
| [`InputState`](#xpui_eginputstate) | One frame of input, as the event source writes it and the framework reads it. |
| [`PacedFill`](#xpui_egpacedfill) | Wraps a display, replacing its repeated-pixel path with a per-pixel one. |
| [`Palette`](#xpui_egpalette) | The two colours ink and background map onto. |
| [Re-exports](#re-exports) | `DrawTarget`, `Labels` and `Metrics`, from the crates this one is built on. |

## `xpui_eg::Backend`

An `xpui` host over an `embedded-graphics` display.

```text
pub struct Backend<D: DrawTarget>
```

`D` is the display: a panel driver, an OLED, a colour TFT, or
`xpui_screenshot::Framebuffer` in a test. The backend supplies `Canvas` and
`TextMetrics` itself, `InputSource` and `Clock` from what the caller feeds it,
and `Chrome` from `xpui-chrome`, so a list, a dialog and a slider look like
something without anyone drawing one.

**`Canvas::clear` fills the whole panel and ignores any clip.** It writes the
background straight to the display, however small a clip is set at the time;
every other primitive draws through the clip.

| Builder | Sets | When not called |
|---|---|---|
| [`with_metrics`](#xpui_egbackendwith_metrics) | the measurements chrome is painted to | `Metrics::DEFAULT` |
| [`with_fonts`](#xpui_egbackendwith_fonts) | the family, and the size each role wants | `Fonts::DEFAULT` |
| [`with_labels`](#xpui_egbackendwith_labels) | the words in the hint bar | `Labels::ENGLISH` |
| [`with_keys`](#xpui_egbackendwith_keys) | what the keys along the bottom edge mean | `KeyRow::READER` |
| [`with_left_right_keys`](#xpui_egbackendwith_left_right_keys) | whether the device has a Left/Right pair | `false` |

**The caller drives the frame.** A loop calls
[`begin_frame`](#xpui_egbackendbegin_frame) with the clock, reports whatever
input arrived, ticks the `App`, renders if a repaint was asked for, and pushes
the pixels with [`with_display`](#xpui_egbackendwith_display) or
[`loan_display`](#xpui_egbackendloan_display).

> [!WARNING]
> Without the [`critical-section`](https://crates.io/crates/critical-section) feature the backend's state is a bare
> `RefCell`, so it must be driven from one thread. A host that paints on a
> second task turns the feature on.

**Example — a frame, driven by hand**

```rust
use xpui::{App, Button, NavigationScreen, Screen, Text, View};
use xpui_eg::{Backend, Palette};
use xpui_screenshot::Framebuffer;

struct Hello;

impl Screen for Hello {
    type Message = ();

    fn body(&self) -> impl View<()> {
        NavigationScreen::new(Text::new("Hello"))
    }

    fn update(&mut self, _message: ()) {}
}

let backend = Backend::leak(Framebuffer::new(480, 800), Palette::INK_IS_ON);
// Safety: one thread, and nothing has rendered on this backend yet.
unsafe { xpui::host::install(backend) };

let mut app = App::new(Hello);
backend.begin_frame(0);
backend.press(Button::Down); // from wherever input comes from
app.tick();
if app.render_if_dirty() {
    backend.clear_dirty();
    backend.with_display(|frame| assert!(frame.ink_count() > 0));
}
```

**Example — a small panel, configured**

```rust
use xpui::host::KeyRow;
use xpui_eg::{Backend, Fonts, Labels, Metrics, Palette};
use xpui_screenshot::Framebuffer;

let backend = Backend::new(Framebuffer::new(296, 128), Palette::INK_IS_OFF)
    .with_metrics(Metrics::SMALL)
    .with_fonts(Fonts::SMALL)
    .with_labels(Labels::ENGLISH_SHORT)
    .with_keys(KeyRow::READER)
    .with_left_right_keys(true);

assert!(backend.left_right_keys());
assert_eq!(backend.fonts().ui, Fonts::SMALL.ui);
```

### Creating a backend

#### `xpui_eg::Backend::new`

A backend over `display`, painting ink and background as `palette` says, with the default metrics, English labels and a reader's key row.

```text
pub fn new(display: D, palette: Palette<D::Color>) -> Self
```

The panel's size is read once, here, so it can still be answered while the
display is [on loan](#xpui_egbackendloan_display).

#### `xpui_eg::Backend::leak`

Leaks the backend so it can be installed.

```text
pub fn leak(display: D, palette: Palette<D::Color>) -> &'static Self where D: 'static,
```

`xpui::host::install` takes a `&'static`, and a backend lives as long as the
program that draws with it, so this is one allocation that was never going to
be freed anyway.

#### `xpui_eg::Backend::leaked`

Leaks a backend a caller has finished building.

```text
pub fn leaked(self) -> &'static Self where D: 'static,
```

[`leak`](#xpui_egbackendleak) builds and leaks in one step, which leaves
nowhere for the builders. This is the other order:
`Backend::new(display, palette).with_metrics(m).leaked()`.

### Configuring it

#### `xpui_eg::Backend::with_metrics`

Paints chrome to these measurements instead of the default.

```text
pub fn with_metrics(self, metrics: Metrics) -> Self
```

Per backend rather than global, because a 296x128 panel and a 480x800 one need
different numbers, and a process can drive both.

#### `xpui_eg::Backend::with_fonts`

Sets the type the framework's roles resolve through: a family, and the size each role wants.

```text
pub fn with_fonts(self, fonts: Fonts) -> Self
```

For a backend nothing will re-set. [`set_family`](#xpui_egbackendset_family)
replaces it while running. See [`Fonts`](fonts.md#xpui_egfonts).

#### `xpui_eg::Backend::with_labels`

The words its hint bar shows.

```text
pub fn with_labels(self, labels: Labels) -> Self
```

English until an application says so.

#### `xpui_eg::Backend::with_keys`

What the keys along the device's bottom edge mean, left to right.

```text
pub fn with_keys(self, keys: KeyRow) -> Self
```

#### `xpui_eg::Backend::with_left_right_keys`

Says the device has a Left/Right pair.

```text
pub fn with_left_right_keys(self, present: bool) -> Self
```

A fact about the hardware, so it is told rather than derived: the backend has
no idea what is around the display it was handed.

> [!NOTE]
> `false` until called, the safe direction. A control told the pair exists
> when it does not cannot be changed by any key; one told it does not exist is
> entered and left instead, which costs a keystroke.

### Reading its configuration

#### `xpui_eg::Backend::metrics`

The measurements every chrome painter lays out against.

```text
pub fn metrics(&self) -> &Metrics
```

#### `xpui_eg::Backend::fonts`

The type in use, as [`with_fonts`](#xpui_egbackendwith_fonts) or a switch left it.

```text
pub fn fonts(&self) -> Fonts
```

#### `xpui_eg::Backend::labels`

The words this backend paints hints with.

```text
pub fn labels(&self) -> &Labels
```

#### `xpui_eg::Backend::keys`

What the keys along the device's bottom edge mean.

```text
pub fn keys(&self) -> &KeyRow
```

#### `xpui_eg::Backend::left_right_keys`

Whether it was told the device has a Left/Right pair.

```text
pub fn left_right_keys(&self) -> bool
```

### Changing the type while it runs

#### `xpui_eg::Backend::set_family`

Sets the type in another family, and asks for a repaint.

```text
pub fn set_family(&self, family: &'static Family)
```

The sizes are re-derived from the chrome, not carried over: each role keeps
the line height these metrics call for, and the new family answers with the
tier it was cut in. Every font id changes with it, since an id is a hash of the
bytes. A screen that wants every backend to follow calls
[`request_family`](fonts.md#xpui_egrequest_family) instead.

```rust
use xpui_eg::{Backend, Fonts, HELVETICA, Palette};
use xpui_screenshot::Framebuffer;

# xpui::testing::install();
let backend = Backend::new(Framebuffer::new(480, 800), Palette::INK_IS_ON);
backend.clear_dirty();
backend.set_family(&HELVETICA);
assert!(backend.is_dirty());
assert_eq!(backend.fonts().ui, Fonts::for_metrics(backend.metrics()).ui);
```

### Driving a frame

#### `xpui_eg::Backend::begin_frame`

Starts a frame: advances the clock and clears one-frame input.

```text
pub fn begin_frame(&self, millis: u32)
```

A family a screen asked for with `request_family` lands here, between frames,
before anything has been measured against the type it replaces.

#### `xpui_eg::Backend::is_dirty`

Whether the framework has asked for a repaint since [`clear_dirty`](#xpui_egbackendclear_dirty).

```text
pub fn is_dirty(&self) -> bool
```

> [!WARNING]
> Not a test-and-clear, and it cannot be made one. `if is_dirty() {
> clear_dirty(); paint(); }` loses a `request_update` landing between the two,
> and on e-ink a lost repaint is a stale panel. Clear it *after* painting, as
> `App::render_if_dirty` does.

#### `xpui_eg::Backend::clear_dirty`

Marks the pending repaint as pushed; call after presenting the frame.

```text
pub fn clear_dirty(&self)
```

### Feeding input

Each of these is an edge, true for the frame it is reported in and cleared by
the next [`begin_frame`](#xpui_egbackendbegin_frame).

#### `xpui_eg::Backend::press`

Reports `button` as pressed this frame.

```text
pub fn press(&self, button: Button)
```

#### `xpui_eg::Backend::release`

Reports `button` as released this frame.

```text
pub fn release(&self, button: Button)
```

#### `xpui_eg::Backend::tap`

Reports a completed tap at `at`.

```text
pub fn tap(&self, at: Point)
```

#### `xpui_eg::Backend::swipe`

Reports a completed swipe.

```text
pub fn swipe(&self, direction: SwipeDir)
```

#### `xpui_eg::Backend::input`

Feeds the frame, for a caller that has its own event source.

```text
pub fn input(&self, feed: impl FnOnce(&mut InputState))
```

The closure runs with the backend's state borrowed, so it must not call back
into the backend: no drawing, no `screen_size`. Feed input and return. What it
can write is [`InputState`](#xpui_eginputstate).

### Reaching the panel

#### `xpui_eg::Backend::with_display`

Borrows the display, for pushing the framebuffer to a panel.

```text
pub fn with_display<R>(&self, body: impl FnOnce(&mut D) -> R) -> R
```

The closure holds the backend's state, so flush the panel, read the pixels,
and return without drawing through the backend.

> [!WARNING]
> Panics if the display is on loan. Presenting twice at once is a caller bug,
> and a present that quietly did nothing would be a stale frame with no other
> symptom.

#### `xpui_eg::Backend::loan_display`

Takes the display **out** of the backend, until the loan is dropped.

```text
pub fn loan_display(&self) -> Option<DisplayLoan<'_, D>>
```

For a flush that suspends. `with_display` holds the guard for as long as its
closure runs, and a borrow held across an `await` is a second task finding the
state already borrowed. `None` when the display is already out. While it is out,
anything that paints is discarded and `screen_size` still answers.

> [!WARNING]
> Do not `mem::forget` a loan: the display never comes back and the panel stops
> updating. Do not drop one inside a backend callback: `Drop` takes the guard
> that callback already holds.

```rust
use xpui_eg::{Backend, Palette};
use xpui_screenshot::Framebuffer;

let backend = Backend::new(Framebuffer::new(480, 800), Palette::INK_IS_ON);
let loan = backend.loan_display().expect("the display is in");
assert!(backend.loan_display().is_none(), "it is out until the loan drops");
assert_eq!(loan.width, 480); // a loan derefs to the display
drop(loan);
assert!(backend.loan_display().is_some());
```

**See also:** [`DisplayLoan`](#xpui_egdisplayloan), [`InputState`](#xpui_eginputstate), [`Palette`](#xpui_egpalette), [`Fonts`](fonts.md#xpui_egfonts)

## `xpui_eg::DisplayLoan`

A display that has been taken out of its backend, and goes back on drop.

```text
pub struct DisplayLoan<'a, D: DrawTarget>
```

Returned by [`Backend::loan_display`](#xpui_egbackendloan_display). It derefs,
mutably too, to the display, and it borrows the backend only shared, so a loan
of a `Sync` backend is `Send` and can be held across an `await`, which is what
it is for.

## `xpui_eg::InputState`

One frame of input, as the event source writes it and the framework reads it.

```text
pub struct InputState
```

Reached through [`Backend::input`](#xpui_egbackendinput). Every event here is
an edge, cleared by `begin_frame`; what survives it is the held set of buttons,
the finger's position until `touch_up`, and `swipe_moves_selection`, which is a
setting.

| Field | Meaning |
|---|---|
| `xpui_eg::InputState::swipe_moves_selection` | Whether a swipe up should move focus up rather than dragging content. |

**Example — a touch driver's frame**

```rust
use xpui::host::InputSource;
use xpui::{Button, Point, SwipeDir};
use xpui_eg::{Backend, Palette};
use xpui_screenshot::Framebuffer;

let backend = Backend::new(Framebuffer::new(480, 800), Palette::INK_IS_ON);
backend.begin_frame(16);
backend.input(|state| {
    state.press(Button::Confirm);
    state.touch_down(Point::new(40, 120));
    state.swipe(SwipeDir::Up);
});
assert!(backend.was_pressed(Button::Confirm));

backend.begin_frame(32);
assert!(!backend.was_pressed(Button::Confirm), "an edge lasts one frame");
assert!(backend.is_pressed(Button::Confirm), "the held state survives");
```

### Starting a frame

#### `xpui_eg::InputState::begin_frame`

Clears everything that lasts one frame.

```text
pub fn begin_frame(&mut self)
```

Held buttons survive; that is the difference between "is down" and "went
down". [`Backend::begin_frame`](#xpui_egbackendbegin_frame) calls it.

### Buttons

#### `xpui_eg::InputState::press`

A button went down: reports both the edge and the held state.

```text
pub fn press(&mut self, button: Button)
```

#### `xpui_eg::InputState::release`

A button came up.

```text
pub fn release(&mut self, button: Button)
```

### Touch

#### `xpui_eg::InputState::tap`

A completed tap, at the position the finger went down.

```text
pub fn tap(&mut self, at: Point)
```

#### `xpui_eg::InputState::touch_down`

A finger is down at `at`.

```text
pub fn touch_down(&mut self, at: Point)
```

Reported every frame it stays down, which is the signal a slider drag needs.

#### `xpui_eg::InputState::touch_up`

The finger came up.

```text
pub fn touch_up(&mut self)
```

#### `xpui_eg::InputState::swipe`

A completed swipe in `direction`.

```text
pub fn swipe(&mut self, direction: SwipeDir)
```

### Gestures

#### `xpui_eg::InputState::back_gesture`

The system back gesture, this frame.

```text
pub fn back_gesture(&mut self)
```

#### `xpui_eg::InputState::home_gesture`

The system home gesture, this frame.

```text
pub fn home_gesture(&mut self)
```

## `xpui_eg::PacedFill`

Wraps a display, replacing its repeated-pixel path with a per-pixel one.

```text
pub struct PacedFill<D>(D)
```

**Wrap a parallel-bus panel in `PacedFill`.** An 8080-style parallel panel
driven through [`mipidsi`](https://crates.io/crates/mipidsi) shortens a run of one colour into a loop on the write
strobe alone, and on a fast microcontroller that loop outruns the controller's
minimum write cycle, so the fill lands as noise. It triggers whenever a pixel's
two bytes are identical, and ink and background are two such colours, so every
filled rectangle and every clear breaks while text, drawn pixel by pixel, comes
out perfectly. A panel showing crisp type over static is this fault.

`PacedFill` routes `fill_solid` and `clear` through `fill_contiguous`, which has
a colour for every pixel and so no run to shorten. It is opt-in: an SPI panel
clocks its own bytes and a framebuffer has no timing, so neither needs it.
[`hardware.md`](../hardware.md) has the whole account, with the timings.

```rust
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use xpui_eg::{Backend, PacedFill, Palette};
use xpui_screenshot::Framebuffer;

let mut panel = PacedFill::new(Framebuffer::new(320, 240));
panel.clear(BinaryColor::On).unwrap();
let frame = panel.into_inner();
assert_eq!(frame.ink_count(), 320 * 240);

// In a firmware, the wrapped panel is what the backend is built over.
let backend = Backend::new(PacedFill::new(frame), Palette::INK_IS_ON);
```

#### `xpui_eg::PacedFill::new`

Wraps a display so its solid fills go out a pixel at a time.

```text
pub fn new(display: D) -> Self
```

#### `xpui_eg::PacedFill::into_inner`

The display back, for a caller that has finished pacing it.

```text
pub fn into_inner(self) -> D
```

## `xpui_eg::Palette`

The two colours ink and background map onto.

```text
pub struct Palette<C>
```

`xpui` paints in ink and background, and a palette maps the two onto any
`PixelColor`, so a colour TFT runs the same screens looking monochrome.

| Field | Meaning |
|---|---|
| `xpui_eg::Palette::ink` | What ink paints as. |
| `xpui_eg::Palette::background` | What background paints as. |

> [!WARNING]
> Ask your driver which colour is ink. Both polarities compile, both are
> plausible, and the wrong one inverts the whole panel with no error anywhere.
> Prefer the named constants to a pair of enum variants at the call site.

**Example — a colour panel**

```rust
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::RgbColor;
use xpui_eg::Palette;

let tft = Palette::new(Rgb565::BLACK, Rgb565::WHITE);
assert_eq!(tft.ink, Rgb565::BLACK);
```

#### `xpui_eg::Palette::new`

A palette painting ink as `ink` and background as `background`.

```text
pub fn new(ink: C, background: C) -> Self
```

#### `xpui_eg::Palette::INK_IS_ON`

Ink is `On`.

```text
pub const INK_IS_ON: Self = Palette
```

The common case, and what the simulator uses.

#### `xpui_eg::Palette::INK_IS_OFF`

Ink is `Off`.

```text
pub const INK_IS_OFF: Self = Palette
```

Some 1-bit panels invert: [`uc8151`](https://crates.io/crates/uc8151), the [Badger 2040](https://shop.pimoroni.com/products/badger-2040)'s controller, maps `Off` to
black so bitmaps load unmirrored.

## Re-exports

Re-exported so a caller never needs a second dependency that could drift to
another version of the same type.

| Name | What it is |
|---|---|
| `xpui_eg::DrawTarget` | `embedded-graphics`' drawing trait, the bound on a `Backend`'s display. A caller writing its own `fn wire<D: DrawTarget>` names this one. |
| `xpui_eg::Labels` | `xpui-chrome`'s hint-bar words, for [`with_labels`](#xpui_egbackendwith_labels). |
| `xpui_eg::Metrics` | `xpui-chrome`'s chrome measurements, for [`with_metrics`](#xpui_egbackendwith_metrics). |
