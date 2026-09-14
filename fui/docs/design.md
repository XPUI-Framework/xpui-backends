# Why this backend has the shape it does

[The README](../README.md) says how to wire this backend up, and
[`cpp/README.md`](../cpp/README.md) how to build the C++ half into a firmware.
This is the part neither covers: why sitting *on* [FreeInkUI](https://github.com/Free-Ink/freeink-sdk/tree/main/libs/ui/FreeInkUI) is a small change
rather than a rewrite, and what that buys.

## Two libraries that arrived at the same shapes

FreeInkUI and `xpui` were written separately, against the same constraints —
1-bit panels, a few hundred KB of RAM, buttons before touch, a display that
takes a second or more to refresh. They converged.

| Concept | `xpui` | FreeInkUI |
|---|---|---|
| Geometry | `Point`, `Size`, `Rect`, `Insets` | `Point`, `Size`, `Rect`, `Insets` |
| What a control accepts | `InputMask`: `TAP`, `FOCUS`, `DRAG`, `LONG_PRESS`, `ADJUST` | `InputMask`: `InputTouch`, `InputFocus`, `InputDrag`, `InputLongPress`, plus `InputConfirm`, `InputBack`, `InputPrev`/`InputNext` and two swipes |
| Focus movement | `move_focus(delta)`, wrapping at both ends | `moveFocus(slot, delta)`, wrapping at both ends |
| Theme geometry | `ThemeMetric`, asked for one tag at a time | `ThemeTokens`, a struct of the same numbers |
| Device capabilities | the host traits | `DeviceContext { hasTouch, hasButtons }` |
| What a control reports | a typed per-screen message | `ActionId` |
| Gestures | `SwipeDir` | `SwipeDir` |

The mask sets are not identical — `ADJUST` is one bit here and
`InputPrev`/`InputNext` two bits there — but the idea is the same on both
sides: a control declares what it accepts and the runtime routes to it, rather
than each control hit-testing for itself. The same goes for the theme:
`ThemeTokens` carries `minTouchSize`, `rowHeight`, `headerHeight`,
`footerHeight` and a four-step spacing scale, which is the ground `ThemeMetric`
covers one tag at a time.

This is convergent design, not coincidence. Two libraries drawing a list row on
a slow monochrome panel with a d-pad end up in the same place.

## Each is the other's missing half

**FreeInkUI is a component library.** The application drives it: build a props
struct, call a component, read back an `ActionId`. It owns *how things look* —
`StyleSet`, `Paint`, `State`, and a style token per component — and it ships 33
of them.

**`xpui` is a declarative layer.** A screen describes a tree and receives typed
messages; the framework owns measurement, focus, input routing and repaint (see
[architecture.md](https://github.com/XPUI-Framework/xpui-framework/blob/main/docs/architecture.md)). It owns *how a screen is
written*.

FreeInkUI has no equivalent of `body()`/`update()`. `xpui` has no equivalent of
33 styled components. They are not competing, and that is the whole premise of
this crate.

## The seam was already there

`xpui` reaches a backend through
[five traits](https://github.com/XPUI-Framework/xpui-framework/blob/main/docs/host.md), and one of them — `Chrome` — says
outright that the host draws this part. Eight calls: a list, a dialog, a slider,
a progress bar, a header, a sub-header, a hint bar, a scroll indicator
([`host/chrome.rs`](https://github.com/XPUI-Framework/xpui-framework/blob/main/src/host/chrome.rs)).

On a drawing library those eight have to be *written*, which is what
[`xpui-chrome`](https://github.com/XPUI-Framework/xpui-chrome/blob/main/README.md) exists for. On a component library they
are calls into something that already exists. So adopting FreeInkUI is a change
to one backend and to nothing else — not to the framework, not to a single
screen. That is the dependency inversion paying for itself; it was built for
exactly this.

What it buys is the property the crate is for: a screen written against `xpui`
and one written in C++ against FreeInkUI come out as the same pixels, because
they are the same component with the same tokens.

## Why the shim asks FreeInkUI for its own numbers

The framework does arithmetic with the metrics it is given. A `Slider` declares
a `Trigger::Value` and the runtime turns a touch into a value in `value_at`,
using `SliderSideInset` and `SliderKnobWidth`. A number that drifts from the one
FreeInkUI paints with puts the knob where the finger is not — a failure that
looks like nothing at all until somebody drags it.

So [`xpui_fui.cpp`](../cpp/xpui_fui.cpp) never restates a constant it can read.
`xpui_fui_metric` answers out of the same `ThemeTokens` the components paint
with, and the three slider tags are read straight off a default-constructed
`fui::SliderProps`. The two pieces the shim draws itself follow the same rule in
reverse: their band heights come from the functions that draw the bands, so
`SubHeaderHeight` reports the one line a sub-header actually paints rather than
a row's worth of space. [coverage.md](coverage.md) has the piece-by-piece state.

## Theming lands on the same seam

FreeInkUI ships `ThemeDocument`, `ThemeTokens`, `StyleSet` and an
`AssetResolver` interface, which makes a theme a *data* change: the components
paint differently and `xpui_fui_metric` answers differently, with no change to
`xpui` and none to any screen. The seam that lets a backend be swapped is the
same seam that lets a theme be swapped.

Two things follow from that, and one of them is not done.

**Parsing belongs above this crate.** The shim names no file format, and a
`no_std` UI framework has no business parsing one. FreeInkUI *consumes* a
`ThemeDocument`; whatever fills it — from an SD card, from flash, from a
constant — is the application's concern, and neither half of this crate is
where it goes.

**Nothing here re-reads a theme yet.** `theme()` in `xpui_fui.cpp` derives its
`ThemeTokens` once behind `g_themeReady` and nothing ever resets that flag — not
even `xpui_fui_attach`. A theme swapped at run time would be ignored until the
next boot. The framework side is the easy half: the view tree is rebuilt every
frame, so a `request_update()` is all `xpui` needs to show the new one.

## The boundary: four places that move together

`cpp/xpui_fui.h` is the contract. `src/raw.rs` declares exactly those symbols,
`cpp/xpui_fui.cpp` defines them and `src/testing/stubs.rs` doubles them. All
four move together, and a mismatch is a link error at best and a corrupt call
frame at worst.

`tests/abi.rs` parses the header and every Rust file that declares or defines
its symbols, and compares **signatures** — not just names. Two parameters
swapped still link, because C has no mangling to disagree with, and the
symptom is a rendering fault somewhere unrelated; that is the failure it
exists for. `symbols_agree` in `xtask/` covers the half it cannot read: the
header against the C++ that defines it.

The boundary runs both ways: `cpp/xpui_screen.h` declares six lifecycle entry
points that `src/lifecycle.rs` **defines**, so a C++ host can drive a Rust
screen through an opaque handle. `xpui-cpp` is a worked example of both
directions.

The C++ half binds to `freeink::ui::DisplayTarget`, which is dependency-free
and takes a plain 1-bit framebuffer. It is deliberately *not* written against
any particular firmware's renderer, so any project linking the [FreeInk SDK](https://github.com/Free-Ink/freeink-sdk) can
add these two files and be done. A firmware with its own themed renderer can
implement the same ABI itself instead — that is a supported path, not a fork.

## Input is the firmware's

`Platform` is the one thing this crate cannot supply. A drawing library cannot
tell you whether a button was pressed; whatever drives the panel already knows.

```rust
# use xpui::Button;
# use xpui_fui::Platform;
# struct MyPlatform;
# struct Buttons;
# impl Buttons {
#     fn pressed(&self, _button: Button) -> bool { false }
#     fn held(&self, _button: Button) -> bool { false }
#     fn released(&self, _button: Button) -> bool { false }
# }
# fn my_input() -> Buttons { Buttons }
# fn my_clock() -> u32 { 0 }
impl Platform for MyPlatform {
    fn millis(&self) -> u32 { my_clock() }
    fn was_pressed(&self, button: Button) -> bool { my_input().pressed(button) }
    fn is_pressed(&self, button: Button) -> bool { my_input().held(button) }
    fn was_released(&self, button: Button) -> bool { my_input().released(button) }
    // Does this device carry Left and Right? Read it off the board rather
    // than inferring it from the shape of the device —
    // `xpui_boards_xteink::X3` does and `xpui_boards_xteink::X4_PRO` does
    // not, and both are readers.
    fn has_left_right_keys(&self) -> bool { true }
}
```

Those five are the whole obligation: touch and the gestures default to "nothing
happened", so a button-only device implements no more than this. `NoInput`
implements exactly those five and nothing else, for a panel that only displays.

`has_left_right_keys` is the only one of the five that asks about the device
rather than about this frame, and the only method below `was_released` without a
default, because there is no answer that is safe to inherit. `src/platform.rs`
says why, and points at the board crates for the answer each board gives.

## The distinctions that break things quietly

Everything crossing the boundary is a pointer, and two pairs of meanings look
identical from C:

**Null is not an empty string, for a button hint.** A null label means "host,
use your own word for this slot". An empty one means "the screen asked for this
slot to be blank". Confusing them either blanks every standard hint or labels
one the screen wanted hidden.

**Null is not an empty string, for a row cell.** Null means the row has no such
field — which is how the theme tells a one-line row from a two-line one. One
row answering `Some("")` instead of `None` makes *every* row in that list tall.

Both are pinned by tests in `tests/marshalling.rs`.

## Testing marshalling

```bash
cargo test -p xpui-fui
```

The `testing` feature swaps the C entry points for doubles that record what
crossed, so the Rust half is testable with no firmware to link against. It
tests **marshalling**, which is where the quiet bugs are. The C++ half is
checked by compiling it.

The doubles answer every `ThemeMetric` with a **distinct** value on purpose:
the tags are positional and not in declaration order (`ListRowGap` is 14,
sitting after the slider values), so two metrics sharing a number would let a
swapped tag pass unnoticed.
