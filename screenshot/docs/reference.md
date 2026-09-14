# Reference

The whole public API of `xpui-screenshot`, crate name `xpui_screenshot`: a
host-side framebuffer a backend draws into, and a comparison against a
committed PNG that fails when the pixels change. [The README](../README.md)
says how a test uses it and why the first run fails.

The `golden` feature, on by default, adds `assert_screenshot`,
`check_screenshot`, `Framebuffer::to_png` and `Framebuffer::from_png`, and the
`png` codec they need. Without it a caller gets the framebuffer and its BMP
writer, which is all the simulator wants.

## Topics

| | |
|---|---|
| [`Framebuffer`](#xpui_screenshotframebuffer) | A plain 1-bit framebuffer implementing `DrawTarget`. |
| [`screenshot_dir`](#xpui_screenshotframebufferscreenshot_dir) | Where `Framebuffer::write_bmp` puts things: `$XPUI_SCREENSHOT_DIR` if set, else `target/screenshots` relative to the working directory. |
| [`assert_screenshot`](#xpui_screenshotassert_screenshot) | Asserts that `frame` matches the golden committed as `tests/screenshots/<name>.png`. |
| [`check_screenshot`](#xpui_screenshotcheck_screenshot) | Compares `frame` with its golden as `assert_screenshot` does, returning the report rather than panicking with it. |

## `xpui_screenshot::Framebuffer`

A plain 1-bit framebuffer implementing [`DrawTarget`](https://docs.rs/embedded-graphics/0.8/embedded_graphics/draw_target/trait.DrawTarget.html).

```text
pub struct Framebuffer
```

An [`embedded-graphics`](https://crates.io/crates/embedded-graphics) `DrawTarget` like any other, with `BinaryColor` and no
window or hardware, so a backend needs no knowledge of it: point one at this
instead of a panel and the same drawing lands where a test can read it. Not
`MockDisplay`, which is 64x64 and rejects overdraw, where a dither or a scrim
paints over what is already there on purpose. Anything drawn off the frame is
dropped.

| Field | Meaning |
|---|---|
| `xpui_screenshot::Framebuffer::width` | Pixels across. |
| `xpui_screenshot::Framebuffer::height` | Pixels down. |

**Example — drawing, and asking where the ink went**

```rust
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use xpui_screenshot::Framebuffer;

let mut frame = Framebuffer::new(64, 32);
Rectangle::new(Point::new(4, 4), Size::new(16, 8))
    .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
    .draw(&mut frame)
    .unwrap();

assert_eq!(frame.ink_in(4, 4, 16, 8), 16 * 8);
assert_eq!(frame.ink_count(), 16 * 8);
assert!(!frame.get(0, 0));
println!("{}", frame.thumbnail(16));
```

### Creating a frame

#### `xpui_screenshot::Framebuffer::new`

A blank frame of `width` by `height`, no ink anywhere.

```text
pub fn new(width: i32, height: i32) -> Self
```

### Reading and writing pixels

#### `xpui_screenshot::Framebuffer::get`

Whether `x`, `y` holds ink; anything off the frame is background.

```text
pub fn get(&self, x: i32, y: i32) -> bool
```

#### `xpui_screenshot::Framebuffer::set`

Lays ink down, or takes it away.

```text
pub fn set(&mut self, x: i32, y: i32, ink: bool)
```

Out of bounds is ignored, as [`get`](#xpui_screenshotframebufferget) reads out
of bounds as blank.

#### `xpui_screenshot::Framebuffer::ink`

Every pixel, row by row, for a caller comparing two whole frames.

```text
pub fn ink(&self) -> &[bool]
```

Read-only, because the width, the height and this slice's length are one
invariant.

#### `xpui_screenshot::Framebuffer::ink_count`

How many pixels hold ink.

```text
pub fn ink_count(&self) -> usize
```

#### `xpui_screenshot::Framebuffer::ink_in`

Ink inside a region, for asserting that something landed where it should without pinning every pixel of it.

```text
pub fn ink_in(&self, x: i32, y: i32, width: i32, height: i32) -> usize
```

### Looking at a frame

Nothing compares these. They are for a person reading a frame: in a failed
test's message, a `println!`, or an image viewer.

#### `xpui_screenshot::Framebuffer::thumbnail`

A coarse ASCII view, `columns` characters wide.

```text
pub fn thumbnail(&self, columns: i32) -> String
```

Deliberately lossy: a block is `width / columns` pixels wide and twice that
tall, so a shift smaller than a block is invisible in it. Never an assertion.

#### `xpui_screenshot::Framebuffer::block_size`

The pixel size of one [`thumbnail`](#xpui_screenshotframebufferthumbnail) character cell, as `(width, height)`.

```text
pub fn block_size(&self, columns: i32) -> (i32, i32)
```

For anything that annotates a thumbnail, such as marking the cells that
changed, so it divides coordinates the same way.

#### `xpui_screenshot::Framebuffer::write_bmp`

Writes a 1-bit BMP into [`screenshot_dir`](#xpui_screenshotframebufferscreenshot_dir), so a person can actually look at the frame.

```text
pub fn write_bmp(&self, name: &str) -> PathBuf
```

Returns the path it wrote, `<name>.bmp`. A debugging helper for a frame with
no golden, such as the simulator's screenshot key. Do not pair it with a
screenshot assertion: two artifacts where one is authoritative is how the other
one gets trusted.

#### `xpui_screenshot::Framebuffer::write_bmp_in`

Writes a 1-bit BMP into `dir`, returning the file's path.

```text
pub fn write_bmp_in(&self, dir: PathBuf, name: &str) -> PathBuf
```

Cargo runs an integration test from its own crate's root, which in a workspace
is not where the shared `target/` is, so a test that wants every frame in one
place passes `env!("CARGO_TARGET_TMPDIR")` here.

### PNG

Behind the `golden` feature.

#### `xpui_screenshot::Framebuffer::to_png`

The panel as a 1-bit greyscale PNG: the format the committed screenshot goldens are written in.

```text
pub fn to_png(&self) -> Vec<u8>
```

Encoded deterministically, with the compression and the row filter pinned and
no timestamp, so re-blessing a screen that did not change rewrites the same
bytes.

#### `xpui_screenshot::Framebuffer::from_png`

Reads back what [`to_png`](#xpui_screenshotframebufferto_png) wrote.

```text
pub fn from_png(bytes: &[u8]) -> Result<Framebuffer, String>
```

Accepts any bit depth, greyscale or RGB, since a golden may have passed through
an image editor; every non-white pixel is ink. A corrupt file is an `Err`, not
a panic.

```rust
use xpui_screenshot::Framebuffer;

let mut frame = Framebuffer::new(37, 21);
frame.set(3, 5, true);
let back = Framebuffer::from_png(&frame.to_png()).expect("it decodes");
assert_eq!((back.width, back.height), (37, 21));
assert_eq!(back.ink(), frame.ink());
```

**See also:** [`assert_screenshot`](#xpui_screenshotassert_screenshot), [`screenshot_dir`](#xpui_screenshotframebufferscreenshot_dir)

## `xpui_screenshot::framebuffer::screenshot_dir`

Where [`Framebuffer::write_bmp`](#xpui_screenshotframebufferwrite_bmp) puts things: `$XPUI_SCREENSHOT_DIR` if set, else `target/screenshots` relative to the working directory.

```text
pub fn screenshot_dir() -> PathBuf
```

Public so a simulator's screenshot key puts frames in the same place.

```rust
use xpui_screenshot::framebuffer::screenshot_dir;

if std::env::var_os("XPUI_SCREENSHOT_DIR").is_none() {
    assert_eq!(screenshot_dir(), std::path::PathBuf::from("target/screenshots"));
}
```

## `xpui_screenshot::assert_screenshot`

Asserts that `frame` matches the golden committed as `tests/screenshots/<name>.png`.

```text
pub fn assert_screenshot(name: &str, frame: &Framebuffer)
```

The path is relative to the crate being tested, read from
`CARGO_MANIFEST_DIR` at run time, so each crate keeps its own goldens. Behind
the `golden` feature.

| Situation | What happens |
|---|---|
| the frame matches | nothing |
| no golden exists | it is written, **and the test fails**, so nobody commits a picture they never looked at |
| `UPDATE_SNAPSHOTS` is set | the golden is rewritten, and the test passes |
| the size differs | fails, naming both sizes |
| a pixel differs | fails with an ASCII map of the changed blocks, and writes `target/diff/<name>.png`: expected, actual and the differences side by side |

A new test therefore needs two runs, and that is the point of it.

**Example — a frame against its golden**

`no_run`, because a first run writes a golden into the crate.

```rust,no_run
use xpui_screenshot::{Framebuffer, assert_screenshot};

let mut frame = Framebuffer::new(64, 32);
frame.set(10, 10, true);
assert_screenshot("a_single_dot", &frame); // against a committed PNG
assert_eq!(frame.ink_count(), 1);          // and what a picture cannot say
```

## `xpui_screenshot::check_screenshot`

Compares `frame` with its golden as [`assert_screenshot`](#xpui_screenshotassert_screenshot) does, returning the report rather than panicking with it.

```text
pub fn check_screenshot(name: &str, frame: &Framebuffer) -> Result<(), String>
```

For a test that captures many frames and wants to name every one that moved,
not stop at the first. `Err` holds the whole report, ready to print. It still
panics when the harness itself is broken: a golden that reads but does not
decode, or one that cannot be written. Behind the `golden` feature.

```rust,no_run
use xpui_screenshot::{Framebuffer, check_screenshot};

let frames = [("home", Framebuffer::new(480, 800)), ("settings", Framebuffer::new(480, 800))];
let failures: Vec<String> = frames
    .iter()
    .filter_map(|(name, frame)| check_screenshot(name, frame).err())
    .collect();
assert!(failures.is_empty(), "{}", failures.join("\n\n"));
```
