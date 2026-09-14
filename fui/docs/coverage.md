# FreeInkUI coverage

`xpui` asks a backend for eight pieces of themed furniture. This is what answers
each of them, what [FreeInkUI](https://github.com/Free-Ink/freeink-sdk/tree/main/libs/ui/FreeInkUI) has no component for, and what it ships that
nothing here calls yet.

One rule decides every row below:

> **Where FreeInkUI has a component, the shim calls it. Where it has none, the
> shim draws it from `DisplayTarget` primitives.**

Calling the component is the entire point of this backend — it is what makes a
screen written against `xpui` and one written in C++ against FreeInkUI the same
pixels, and what will let a theme reach both at once.
[design.md](design.md) has the reasoning; this file has the state.

## The eight

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

The list goes through `Screen::list` rather than `ui::list` on purpose:
`Screen::list` substitutes the theme's row gap, padding, row styles and
selection style into any prop left at its inherit sentinel, so that recipe stays
in the SDK — which is also where `xpui_fui_metric` reads it from.

## The two the shim draws itself

**A sub-header.** There is no heading component. A group heading is a bold body
line with an optional value opposite it, drawn straight onto the target. The
band's height is the caller's business and the line is drawn at its top, which
is why `SubHeaderHeight` reports exactly one line of the body font: reserving a
list row's worth would leave a hole under every heading.

**A button-hint bar.** FreeInkUI's bars are `tab-bar`, `status-bar`,
`gesture-bar`, `reader-chrome` and `tap-zones`; none of them is a four-slot hint
bar. The shim fills the band, rules its top edge and centres four labels in
quarters, with the last slot taking the division's remainder. `ButtonHintsHeight`
answers `ThemeTokens::footerHeight` — the band it actually fills.

Both are candidates to contribute upstream. Until then they are the reason
`SubHeaderHeight` and `ButtonHintsHeight` are computed by the same two functions
that draw the bands rather than stated separately.

## Where the shim does more than the component

Calling a component is not always the whole answer, and each of these gaps has a
reason worth knowing before changing it.

| | |
|---|---|
| **The list's scroll indicator** | The component only ever sees the rows it is about to draw, so it cannot know the list overflows. The shim reserves the track's width out of the row band and draws the indicator itself, against the real totals. |
| **The list does not scroll** | `xpui` owns scrolling: a `ScrollView` translates the rect, and `List::interactions` declares row 0 at the top of the rect it was given. A shim that scrolled independently would paint row N where the framework registered row 0. |
| **The dialog's border** | `defaultPopupStyles` is a white panel with no outline, which on a white page is no panel at all. The shim adds a one-pixel border; the dim behind the dialog is `xpui`'s own scrim. |
| **The progress bar's track** | The component's default track is transparent, so an empty bar would be invisible. The shim paints a light dither under it. |
| **Nothing registers a hit** | The list, the dialog and the slider are all handed `NO_ACTION`. `xpui` declared the touch rects before rendering and routes taps itself, so the components must draw and nothing more. |
| **The slider's three states** | `SliderProps` carries paints and geometry and has no notion of focus. The shim sets `knob` and `border` from the `ControlState` it is handed — paper, then a dither once the keys are on the control — and strokes one rect around an open one. It matches what `xpui-chrome` paints, so a screen does not change appearance when it moves between backends. |

## What does not go through a component at all

The `Canvas` half of the backend — text, fills, strokes, lines, the dither, the
scrim and 1-bit bitmaps — lands on `freeink::ui::DisplayTarget` directly. These
are irreducible under any framework: draw a string, draw a line, blit a bitmap.

Two consequences:

- **Clipping is the shim's own.** FreeInkUI has no clipping, so the clip is
  enforced by windowing the `DisplayTarget` at the framebuffer rather than by
  intercepting draws. That clips glyph pixels exactly, which rectangle
  arithmetic cannot.
- **This build ships no icons.** `xpui_fui_icon_size` returns 0 for every role,
  which the ABI defines as "reserve no space", and `xpui_fui_draw_icon` draws
  nothing. The Icons library is a separate opt-in (`FreeInkUIIcon.h`).

## What FreeInkUI ships that nothing here uses

Five of the SDK's 33 components are called above. The other 28 are not — not
because they were rejected, but because no screen has wanted one. They are what
this backend would gain by adopting more of FreeInkUI rather than sitting
alongside it.

| Group | Components |
|---|---|
| Bars | `battery-indicator`, `status-bar`, `tab-bar`, `gesture-bar`, `tap-zones`, `reader-chrome` |
| Controls | `button`, `checkbox`, `toggle` |
| Lists | `setting-row`, `toggle-row`, `stepper-row`, `radio-group`, `dropdown`, `table` |
| Overlays | `context-menu`, `popup`, `message-panel`, `toast` |
| Text | `text-field`, `text-area` |
| Keyboard | `keyboard`, `qwerty-keyboard`, `key-grid` |
| Media | `book-card`, `cover-carousel`, `cover-grid`, `metric-card` |

Four worth singling out:

- **`keyboard` / `qwerty-keyboard`.** `xpui` has no text-entry widget at all, so
  a screen that needs one has a component waiting rather than a widget to write.
- **`toggle` / `toggle-row`.** `xpui`'s `Toggle` composes a list row showing a
  word, not a switch. Adopting these is a design change, not a port.
- **`setting-row`, `stepper-row`, `dropdown`, `radio-group`.** Rows `xpui`
  composes today out of `List` plus a value string.
- **`battery-indicator`.** `HeaderProps` reserves room for header extras with
  `leftReserve`/`rightReserve`; `xpui_fui_draw_header` sets neither, so the band
  holds a title and a subtitle and nothing else.

---

*Every count here was read from the SDK this shim compiles against — 33 headers
under `<sdk>/libs/ui/FreeInkUI/include/components/` — and every call site from
[`cpp/xpui_fui.cpp`](../cpp/xpui_fui.cpp). `./build-and-test.sh check` takes
`FREEINK_SDK_INCLUDE`, or else finds the SDK beside this checkout.*
