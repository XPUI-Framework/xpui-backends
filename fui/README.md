# `xpui-fui`

An [`xpui`](../../xpui/) backend that draws through **FreeInkUI**.

FreeInkUI is a header-only C++ component library for e-ink firmware. It already
knows what a list row, a dialog and a slider look like — so a screen written
against `xpui` and one written in C++ against FreeInkUI come out as the same
pixels. That is the whole reason to sit on it rather than beside it.

## Two halves

| | |
|---|---|
| `src/` | the Rust `Host` implementation, over a C ABI |
| `cpp/` | that ABI implemented against FreeInkUI |

`cpp/xpui_fui.h` is the contract. `src/raw.rs` declares exactly those symbols
and `cpp/xpui_fui.cpp` defines them. All three move together, and a mismatch is
a link error at best and a corrupt call frame at worst.

`ffi_symbols_agree()` in `build-and-test.sh` checks that every **name** exists
in every place it is written down — the header, the Rust declarations, the shim
and the host doubles. **It does not check types.** Two parameters swapped still
links, because C has no mangling to disagree with, and the symptom is a
rendering fault somewhere unrelated. That is what
[spec 07](../../../docs/specs/07-ffi-checker.md) is for.

The boundary runs both ways: `cpp/xpui_screen.h` declares six lifecycle entry
points that `src/lifecycle.rs` **defines**, so a C++ host can drive a Rust
screen through an opaque handle. `examples/cpp_host` is a worked example of
both directions.

The C++ half binds to `freeink::ui::DisplayTarget`, which is dependency-free
and takes a plain 1-bit framebuffer. It is deliberately *not* written against
any particular firmware's renderer, so any project linking the FreeInk SDK can
add these two files and be done. A firmware with its own themed renderer can
implement the same ABI itself instead — that is a supported path, not a fork.

## Wiring it up

```rust,no_run
# use xpui::Button;
# use xpui_fui::{Backend, Platform};
# struct MyPlatform;
# impl Platform for MyPlatform {
#     fn millis(&self) -> u32 { 0 }
#     fn was_pressed(&self, _button: Button) -> bool { false }
#     fn is_pressed(&self, _button: Button) -> bool { false }
#     fn was_released(&self, _button: Button) -> bool { false }
# }
# let mut framebuffer = [0u8; 480 * 800 / 8];
// Once, with the panel's framebuffer.
unsafe { xpui_fui::attach(framebuffer.as_mut_ptr(), 480, 800) };

static PLATFORM: MyPlatform = MyPlatform;
static BACKEND: Backend<MyPlatform> = Backend::new(&PLATFORM);
unsafe { xpui::host::install(&BACKEND) };
```

Add `cpp/xpui_fui.cpp` to the firmware's build with FreeInkUI's include
directory on the path. See [`cpp/README.md`](cpp/README.md).

## Input is yours

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
}
```

Those four are the whole obligation: touch and the gestures default to "nothing
happened", so a button-only device implements no more than this. `NoInput`
implements exactly those four and nothing else, for a panel that only displays.

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

## Testing

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
