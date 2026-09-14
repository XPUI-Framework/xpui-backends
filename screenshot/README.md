[![CI](https://github.com/XPUI-Framework/xpui-backends/actions/workflows/ci.yml/badge.svg)](https://github.com/XPUI-Framework/xpui-backends/actions/workflows/ci.yml) [![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../LICENSE)

# `xpui-screenshot`

A framebuffer you can draw a whole screen into, and a golden-image comparison
that fails when the pixels change.

This is the fourth of the four testing layers. Behaviour tests assert what a
screen *decided*; draw-call tests assert what it *asked a backend for*. Only
this one asserts what actually landed in memory — and it is the only layer that
catches a backend that agrees with the framework about everything and paints
the wrong thing anyway.

## Using it

```toml
[dev-dependencies]
xpui-screenshot = { git = "https://github.com/XPUI-Framework/xpui-backends", branch = "main", features = ["golden"] }
```

`golden` is a feature because the comparison pulls in `png`, and a consumer who
only wants the framebuffer — the simulator does — should not pay for it.

```rust,no_run
# use xpui_screenshot::Framebuffer;
# let framebuffer: Framebuffer = unimplemented!();
// Compares against `tests/screenshots/settings.png`, and on a mismatch writes
// a side-by-side image to `target/diff/settings.png` before panicking.
xpui_screenshot::assert_screenshot("settings", &framebuffer);
```

A separate crate from the backends because it is `std`, it writes files, and
a backend builds for bare metal. `Framebuffer` is a plain 1-bit `DrawTarget`
with no window and no hardware, so a backend built over one renders a screen in
an ordinary `cargo test`, and its `with_display` hands the frame to the
assertion. A picture and a number say different things, so a test usually
carries both:

```rust,no_run
# use xpui_screenshot::{Framebuffer, assert_screenshot};
# let frame = Framebuffer::new(480, 800);
assert_screenshot("my_screen", &frame);      // against a committed PNG
assert!(frame.ink_in(0, 0, 480, 56) > 0);    // and what a picture cannot say
```

`check_screenshot` is the same comparison handing back its report as
`Result<(), String>` rather than panicking with it, for a test that captures
many frames and wants to name every one that moved rather than stopping at the
first. A broken harness — a golden that will not decode, a directory that will
not take a file — still panics through either of them.
[`xpui-gallery`'s `gallery/`](https://github.com/XPUI-Framework/xpui-gallery/tree/main/gallery)
renders its screens on every board that way, so one token moved by one pixel
names every board it reached rather than the first.

**The first run fails on purpose.** A golden that does not exist yet is
written, and then the test fails: nobody can commit a picture they have never
looked at. Accept an intended change with `UPDATE_SNAPSHOTS=1`, then open the
file and read it before staging.

```bash
UPDATE_SNAPSHOTS=1 cargo test
```

A mismatch prints an ASCII view marking every block that changed, and writes
`expected`, `actual` and `differences` side by side into `target/diff/`, so a failure on CI can be looked at rather than guessed at —
the workflows in the repositories that hold goldens upload that directory on
failure. [`xpui-embedded-graphics`](../embedded_graphics/) uses it for its
seven images, [`xpui-gallery`](https://github.com/XPUI-Framework/xpui-gallery)
for its seventy board captures and three typeface captures, and
[`xpui-simulator`](https://github.com/XPUI-Framework/xpui-simulator) for the
framebuffer alone. `embedded_graphics/tests/screenshots.rs` is a whole suite
written this way.

`write_bmp` and `thumbnail` are for looking at a frame rather than asserting on
one. Nothing compares them.

## Checking it

The gate is the repository's; run `./build-and-test.sh` from the root. It
lints and doctests this crate a second time without `golden`, the shape the
simulator consumes.

## Where next

| | |
|---|---|
| [`docs/reference.md`](docs/reference.md) | `Framebuffer`, `assert_screenshot`, `check_screenshot` and `screenshot_dir`: every public item, with examples |

## License

MIT — see [LICENSE](../LICENSE). Copyright (c) 2026 Thiago Holanda.
