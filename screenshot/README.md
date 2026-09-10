# `xpui-screenshot`

> ⚠️ **Under heavy development.** Not production-ready. The API can break
> without notice. Use at your own risk.

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

`check_screenshot` is the same comparison as a `Result`, for a test capturing
many frames that wants to report all the failures rather than the first.

**The first run fails on purpose.** A golden that does not exist yet is
written, and then the test fails: nobody can commit a picture they have never
looked at. Accept an intended change with `UPDATE_SNAPSHOTS=1`, then open the
file and read it before staging.

```bash
UPDATE_SNAPSHOTS=1 cargo test
```

A mismatch writes `expected`, `actual` and `differences` side by side into
`target/diff/`, so a failure on CI can be looked at rather than guessed at —
the workflows in the repositories that hold goldens upload that directory on
failure. [`xpui-embedded-graphics`](../embedded_graphics/) uses it for its
seven images, [`xpui-gallery`](https://github.com/XPUI-Framework/xpui-gallery)
for its seventy board captures, and
[`xpui-simulator`](https://github.com/XPUI-Framework/xpui-simulator) for the
framebuffer alone.

Its own comparator is tested against itself: ten cases in `src/golden/`, one of
which is a committed image whose only job is to prove the comparison still
returns `Err` when it should. Replace its last two lines with `Ok(())` and the
whole organisation's pixel suite passes while comparing nothing — which is the
one failure this technique cannot survive, so it is checked here.

## Checking it

The gate is the repository's; run `./build-and-test.sh` from the root. It
lints and doctests this crate a second time without `golden`, the shape the
simulator consumes.

## License

MIT — see [LICENSE](../LICENSE). Copyright (c) 2026 Thiago Holanda.
