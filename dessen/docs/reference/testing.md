# The host doubles

The `testing` module: dessen's calls answered off the VM, so the backend, its
tests and these pages' examples run on the host.

It exists only off `riscv32`; on the VM, dessen answers the same calls.

## Topics

| | |
|---|---|
| [`Op`](#xpui_dessentestingop) | One recorded call, with its arguments in dessen's safe-area coordinates. |
| [`lock`](#xpui_dessentestinglock) | Serialises tests that use the doubles or install a host. |
| [`reset`](#xpui_dessentestingreset) | Starts over: `device` is what the next `Device::read` sees, and the recorded calls and queued events are dropped. |
| [`events`](#xpui_dessentestingevents) | Queues events for `dsn_wait_event`, in order. |
| [`take_ops`](#xpui_dessentestingtake_ops) | Takes every call recorded since the last take. |
| [`glyph_width`](#xpui_dessentestingglyph_width) | The width of one glyph in `font`, as the double measures it. |
| [`line_height`](#xpui_dessentestingline_height) | The height of a line in `font`, as the double measures it. |

## Example

**Example — what drawing a line of text sends to dessen**

```rust
use xpui::Point;
use xpui::host::{Canvas, FontRole, FontStyle, TextMetrics};
use xpui_dessen::testing::{self, Op};
use xpui_dessen::{Backend, Device};

let _guard = testing::lock();
let device = Device { width: 480, height: 800, has_touch: true, has_buttons: true, hint_height: 0 };
testing::reset(device);
let backend = Backend::new(device);
let font = backend.font(FontRole::Ui);
backend.draw_text(Point::new(10, 20), "Hi", font, FontStyle::Regular);
assert_eq!(testing::take_ops(), [Op::Text(10, 20, "Hi".into(), dessen::DSN_FONT_BODY, 0)]);
```

## `xpui_dessen::testing::Op`

One recorded call, with its arguments in dessen's safe-area coordinates.

```text
pub enum Op
```

### Variants

#### `xpui_dessen::testing::Op::Clear`

`dsn_clear`.

#### `xpui_dessen::testing::Op::Fill`

`dsn_fill`: x, y, width, height, colour.

#### `xpui_dessen::testing::Op::Stroke`

`dsn_stroke`: x, y, width, height.

#### `xpui_dessen::testing::Op::Line`

`dsn_line`: from x, from y, to x, to y.

#### `xpui_dessen::testing::Op::Text`

`dsn_text_box`: x, y, the text, font, flags.

#### `xpui_dessen::testing::Op::Bitmap`

`dsn_bitmap`: x, y, width, height, the bytes passed, format, colour.

#### `xpui_dessen::testing::Op::Refresh`

`dsn_refresh`.

#### `xpui_dessen::testing::Op::Clip`

`dsn_set_clip`: x, y, width, height; a negative width lifts the clip.

#### `xpui_dessen::testing::Op::Hints`

`dsn_button_hints`: back, confirm, previous, next.

## `xpui_dessen::testing::lock`

Serialises tests that use the doubles or install a host.

```text
pub fn lock() -> MutexGuard<'static, ()>
```

Hold it for the whole test; a failed test does not poison it for the next one.

## `xpui_dessen::testing::reset`

Starts over: `device` is what the next `Device::read` sees, and the recorded calls and queued events are dropped.

```text
pub fn reset(device: Device)
```

## `xpui_dessen::testing::events`

Queues events for `dsn_wait_event`, in order.

```text
pub fn events(events: &[u32])
```

Once they run out it answers 0, which ends [`run`](backend.md#xpui_dessenrun).

## `xpui_dessen::testing::take_ops`

Takes every call recorded since the last take.

```text
pub fn take_ops() -> Vec<Op>
```

## `xpui_dessen::testing::glyph_width`

The width of one glyph in `font`, as the double measures it.

```text
pub fn glyph_width(font: i32) -> i32
```

## `xpui_dessen::testing::line_height`

The height of a line in `font`, as the double measures it.

```text
pub fn line_height(font: i32) -> i32
```
