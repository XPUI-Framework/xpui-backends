# `xpui-abi-check`

> ⚠️ **Under heavy development.** Not production-ready. The API can break
> without notice. Use at your own risk.

Parses a C header and the Rust that declares it, and reports where the two
disagree about a **signature**.

## Why a linker is not enough

C has no name mangling. If a header says

```text
void xpui_fui_draw_text(int32_t x, int32_t y, const uint8_t* text);
```

and the Rust beside it says

```text
fn xpui_fui_draw_text(y: i32, x: i32, text: *const u8);
```

then every symbol resolves, the firmware links, and the call frame is corrupt.
Text lands at transposed coordinates, or a pointer is read as an integer and
the panel fills with noise. Nothing in either toolchain will tell you.

This is what tells you. It compares parameter *types and order*, not just
presence — which is the half `ffi_symbols_agree` in the gate cannot see, and
the half a link error never reaches.

## Using it

```toml
[dev-dependencies]
xpui-abi-check = { git = "https://github.com/XPUI-Framework/xpui-backends", branch = "main" }
```

Then a test per boundary, naming the header and the Rust that answers it. An
**unrecognised C type fails the run** rather than being skipped: a checker that
quietly ignores what it cannot parse is a checker that passes because it
understood nothing.

## Who uses it

Five boundaries across two repositories, each checked where it is owned:

| Boundary | Checked in |
|---|---|
| `xpui_fui.h` ⇄ `fui/src/raw.rs` | [`fui/tests/abi.rs`](../fui/tests/abi.rs) |
| `xpui_screen.h` ⇄ `fui/src/lifecycle.rs` | same |
| `xpui_fui.h` ⇄ `fui/src/testing/stubs.rs` | same |
| `xpui_host.h` ⇄ `cpp_host/src/raw.rs` | [`xpui-cpp`](https://github.com/XPUI-Framework/xpui-cpp)'s `abi/tests/abi.rs` |
| `xpui_app.h` ⇄ `register_screen!`'s expansion | same |

Note what is **not** in that list: `xpui_fui.h` against `cpp/xpui_fui.cpp`,
the C++ shim itself. No signature checker reads C++ here — that pair gets
`ffi_symbols_agree`, which compares which symbols exist and not what they are.
The third row above is the same header against the *Rust* host doubles, which
is a different guarantee: it keeps the doubles honest, not the shim.

The last row is why `xpui-fui` exports its macro's source as a string: a git
dependency lands in a cargo checkout directory with no path a sibling
repository can read.

## License

MIT — see [LICENSE](../LICENSE). Copyright (c) 2026 Thiago Holanda.
