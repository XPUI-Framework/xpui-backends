# Screenshot tests

What this backend can prove about pixels, on a laptop, with no panel.

[`../README.md`](../README.md) is the front page.

```toml
[dev-dependencies]
xpui-screenshot = { git = "https://github.com/XPUI-Framework/xpui-backends", branch = "main", features = ["golden"] }
```

A separate crate, because it is `std`, it writes files, and this one builds for
bare metal. `Framebuffer` is a plain 1-bit `DrawTarget` with no window and no
hardware, so a screen can be rendered and asserted on in an ordinary
`cargo test`:

```rust,no_run
# use xpui_eg::{Backend, Palette};
# use xpui_screenshot::{Framebuffer, assert_screenshot};
# let backend = Backend::new(Framebuffer::new(480, 800), Palette::INK_IS_ON);
backend.with_display(|frame| {
    assert_screenshot("my_screen", frame);      // against a committed PNG
    assert!(frame.ink_in(0, 0, 480, 56) > 0);   // and what a picture cannot say
});
```

`assert_screenshot` compares the whole panel against
`tests/screenshots/my_screen.png` in the crate being tested, pixel for pixel.
The first run writes the golden and fails, so that nobody commits a picture
they never looked at; `UPDATE_SNAPSHOTS=1` rewrites it afterwards. A mismatch
prints an ASCII view marking every block that changed and writes
`target/diff/my_screen.png` — expected, actual and the differences, side by
side.

That is how this crate's own tests work — see `tests/screenshots.rs`.

`check_screenshot` is the same comparison handing back its report as
`Result<(), String>` rather than panicking with it, for a test that captures
many frames and wants to name every one that moved rather than stopping at the
first. A broken harness — a golden that will not decode, a directory that will
not take a file — still panics through either of them.
[`xpui-gallery`'s `gallery/`](https://github.com/XPUI-Framework/xpui-gallery/tree/main/gallery) renders nine screens on seven
boards that way, so one token moved by one pixel names every board it reached
rather than the first.

`write_bmp` and `thumbnail` are still there, and are for looking at a frame
rather than asserting on one. Nothing compares them.
