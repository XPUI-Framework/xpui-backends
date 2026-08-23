# Why this backend has the shape it does

[The README](../README.md) says how to wire this backend up, and
[`cpp/README.md`](../cpp/README.md) how to build the C++ half into a firmware.
This is the part neither covers: why sitting *on* FreeInkUI is a small change
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
[architecture.md](../../../xpui/docs/architecture.md)). It owns *how a screen is
written*.

FreeInkUI has no equivalent of `body()`/`update()`. `xpui` has no equivalent of
33 styled components. They are not competing, and that is the whole premise of
this crate.

## The seam was already there

`xpui` reaches a backend through
[five traits](../../../xpui/docs/host.md), and one of them — `Chrome` — says
outright that the host draws this part. Eight calls: a list, a dialog, a slider,
a progress bar, a header, a sub-header, a hint bar, a scroll indicator
([`host/chrome.rs`](../../../xpui/src/host/chrome.rs)).

On a drawing library those eight have to be *written*, which is what
[`xpui-chrome`](../../chrome/README.md) exists for. On a component library they
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
