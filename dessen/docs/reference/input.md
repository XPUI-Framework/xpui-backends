# Input frames

The `input` module: how one finished dessen event becomes the frames `xpui`
polls, and the clock those frames read.

[`run`](backend.md#xpui_dessenrun) feeds these frames; an app never needs to.
They are public so the mapping can be read and tested on its own.

## Topics

| | |
|---|---|
| [`Frame`](#xpui_desseninputframe) | One frame of input, as `InputSource` answers it. |
| [`frames`](#xpui_desseninputframes) | The frames one dessen event becomes, and how many of the four are used. |
| [`HOLD_STEP_MS`](#xpui_desseninputhold_step_ms) | The step between the held frames of a long press, in milliseconds. |
| [`EVENT_GAP_MS`](#xpui_desseninputevent_gap_ms) | How far the clock moves before each event, in milliseconds. |

## `xpui_dessen::input::Frame`

One frame of input, as `InputSource` answers it.

```text
pub struct Frame
```

### Fields

#### `xpui_dessen::input::Frame::button`

The button pressed and released this frame.

#### `xpui_dessen::input::Frame::tap`

A completed tap.

#### `xpui_dessen::input::Frame::held`

A finger resting here.

#### `xpui_dessen::input::Frame::released`

The finger lifted this frame.

#### `xpui_dessen::input::Frame::swipe`

A swipe this frame.

## `xpui_dessen::input::frames`

The frames one dessen event becomes, and how many of the four are used.

```text
pub fn frames(event: u32) -> ([Frame; 4], usize)
```

An event this backend does not know becomes no frames.

**Example — a long press is three held frames and a lift**

```rust
use xpui::Point;
use xpui_dessen::input::frames;

let event = dessen::DSN_EV_LONG_PRESS as u32 | 30 << 8 | 40 << 20;
let (frames, len) = frames(event);
assert_eq!(len, 4);
assert!(frames[..3].iter().all(|f| f.held == Some(Point::new(30, 40))));
assert!(frames[3].released && frames[3].tap == Some(Point::new(30, 40)));
```

## `xpui_dessen::input::HOLD_STEP_MS`

The step between the held frames of a long press, in milliseconds.

```text
pub const HOLD_STEP_MS: u32 = 250;
```

## `xpui_dessen::input::EVENT_GAP_MS`

How far the clock moves before each event, in milliseconds.

```text
pub const EVENT_GAP_MS: u32 = 1000;
```
