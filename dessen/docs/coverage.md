# What answers each host trait

`xpui` asks a backend for five traits. This page says which dessen call answers
each method, and where dessen has nothing to answer with yet. Dessen's calls
are generated from its `abi/freeink.api`; every coordinate is in the safe area,
and the firmware clips and range-checks every value before it reaches
FreeInkUI.

## `Canvas`

| Method | Dessen call | Notes |
|---|---|---|
| `screen_size` | `dsn_screen` | the safe area, inside any bezel |
| `clear` | `dsn_clear` | |
| `draw_text` | `dsn_text_box` | top-left origin on both sides; one line, to the right edge |
| `fill_rect` | `dsn_fill` | black or white |
| `stroke_rect` | `dsn_stroke` | one pixel, square corners |
| `draw_line` | `dsn_line` | one pixel |
| `fill_rect_dither` | `dsn_fill` | FreeInkUI's light and dark grays, its dither patterns |
| `scrim` | `dsn_fill` with `DSN_SCRIM` | light-gray ink over what is there; whether the background shows through is the firmware renderer's |
| `set_clip` | `dsn_set_clip` | the firmware clips the target, inside the safe area; `None` is a negative width |
| `draw_image` | `dsn_bitmap` | black-and-white format, every bit flipped: `xpui`'s bit 0 is ink |
| `draw_icon`, `icon_size` | `xpui-chrome` | drawn from lines and rectangles |

## `TextMetrics`

| Method | Dessen call | Notes |
|---|---|---|
| `font` | — | `UiSmall` is dessen's small font; `Ui` and `Reader` its body font. The id is dessen's number plus one |
| `text_width` | `dsn_measure_text` | the firmware's own renderer measures it |
| `line_height` | `dsn_line_height` | |

The firmware's faces are fixed for a build and an app cannot load its own, so a
constant id names the same bytes for the app's whole life.

## `Chrome`

All eight components are xpui-chrome's painting, drawn through the `Canvas`
above and sized by `Metrics::for_panel` for the safe area, except the button
hints. Those are the host's: `dsn_device` reports the height of its hint band
(0 where it shows none, as on a touch board), the backend keeps it clear, and
`dsn_button_hints` hands the host each action's label to draw in its own look,
over its own keys.

## `InputSource` and `Clock`

Dessen delivers one finished event at a time. Each becomes one frame, except a
long press, which becomes three held frames and a lift; see the `input` module.

| Dessen event | `xpui` sees |
|---|---|
| `CONFIRM`, `BACK` | `Confirm`, `Back` pressed and released |
| `PREV`, `NEXT` | `Up`, `Down` |
| `PREV`, `NEXT` with `DSN_EV_SIDEWAYS` | `Left`, `Right` |
| `TAP` | a tap and a release at its point |
| `LONG_PRESS` | a finger held 500 ms at its point, then a tap and a release |
| `SWIPE_*` | the matching `SwipeDir` |

`is_pressed` is always false, so nothing auto-repeats; `has_left_right_keys`
turns true at the first sideways key, so a firmware that never marks one keeps
Confirm opening value controls; the back and home gestures belong to the firmware. `millis` is the
backend's own clock, moved only by events, so a hold is timed in event time.

## What dessen cannot do yet

| Gap | Effect | Closed by |
|---|---|---|
| CrossPoint's renderer dithers in `Replace` mode | `scrim` hides what is behind it instead of dimming it | FreeInkUI's GfxRenderer adapter passing `PreserveInk` for a dithered fill |
| A firmware that does not mark sideways keys | Left and Right arrive as Up and Down | the firmware calling `FreeInkHost::markSideways` |
