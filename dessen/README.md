[![CI](https://github.com/XPUI-Framework/xpui-backends/actions/workflows/ci.yml/badge.svg)](https://github.com/XPUI-Framework/xpui-backends/actions/workflows/ci.yml) [![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../LICENSE)

# `xpui-dessen`

An [`xpui`](https://github.com/XPUI-Framework/xpui-framework) backend for
**[dessen](https://github.com/Free-Ink/dessen)** apps.

Dessen runs small sandboxed RV32IM apps inside CrossPoint and FreeInk reader
firmware, launched from the SD card instead of flashed. An app reaches the
device only through dessen's calls, which the firmware forwards to FreeInkUI's
`DrawTarget`. This crate answers `xpui`'s five host traits with those calls,
taking `Chrome` from [`xpui-chrome`](https://github.com/XPUI-Framework/xpui-chrome),
so a screen written against `xpui` runs as a dessen app. One `app.bin` runs on
every board, whatever its chip: the chip only ever runs the VM.

## Using it

```toml
[dependencies]
xpui = { git = "https://github.com/XPUI-Framework/xpui-framework", branch = "main" }
xpui-dessen = { git = "https://github.com/XPUI-Framework/xpui-backends", branch = "main" }
# The app, not the backend, takes dessen's entry point and a heap for `alloc`.
dessen = { git = "https://github.com/Free-Ink/dessen", features = ["heap"] }
```

An app installs the backend once, then hands its first screen to `run`, which
owns the loop until the last screen finishes:

```rust
use xpui::{App, List, ListRow, NavigationScreen, Screen, View};

struct Menu;

impl Screen for Menu {
    type Message = ();
    fn body(&self) -> impl View<()> {
        NavigationScreen::new(List::new().push(ListRow::new("Hello from xpui")))
    }
    fn update(&mut self, _message: ()) {}
}

// In the app's `extern "C" fn main() -> i32`, which dessen's entry point calls:
let backend = xpui_dessen::install();
let exit_code = xpui_dessen::run(backend, App::new(Menu));
# assert_eq!(exit_code, 0);
```

An app writes none of that by hand: `xpui_dessen::main!(app)`, given a
`fn app() -> App`, writes the entry point dessen's `_start` calls, installs the
backend and runs the app, so the app itself needs no `unsafe` and can carry
`#![forbid(unsafe_code)]`.

A dessen app is `no_std` and `no_main`, sets aside a heap with
`dessen::heap!(SIZE)`, and builds for `riscv32im-unknown-none-elf` with
`-C link-arg=-Tdessen.ld`; dessen's `docs/writing-apps.md` has the whole recipe.
Off the VM, the calls go to the recording doubles in `testing`, which is how
this README's example runs as a doctest.

## Requirements

Apps build for `riscv32im-unknown-none-elf`, never `riscv32imc`: the dessen VM
decodes no compressed instructions. The repository's `rust-toolchain.toml`
installs the target. Tests and doctests run on the host, against the doubles.

## Checking it

The gate is the repository's; run `./build-and-test.sh` from the root. It
lints this crate for the host and for `riscv32im-unknown-none-elf`, and runs its
tests against the doubles.

## Where next

| | |
|---|---|
| [`docs/reference.md`](docs/reference.md) | every public item: the backend, its input frames, and the host doubles |
| [`docs/coverage.md`](docs/coverage.md) | which dessen call answers each host trait, and what dessen cannot yet do |

## License

MIT — see [LICENSE](../LICENSE).
