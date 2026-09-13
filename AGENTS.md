# `xpui-backends`

## What this is, and what it may not become

The two things that put `xpui` pixels on a panel, and the two helpers they
need. `xpui-embedded-graphics` draws every pixel itself through any
`DrawTarget`, with `xpui-chrome`'s components and u8g2 faces; `xpui-fui` calls
FreeInkUI over a C ABI, from a Rust half in `fui/src/` and a C++ half in
`fui/cpp/`. `xpui-screenshot` is the host-side framebuffer and golden
comparison a backend proves itself with; `xpui-abi-check` compares a C header
against the Rust that declares it, on types.

**A backend implements the five traits and depends on no board.** It takes
its measurements from whoever wires it and has no idea what a device is; the
`xpui-boards-*` crates are never a dependency here. The FreeInkUI shim's C
symbols live in four places that move together — the header, `raw.rs`, the
C++ and the doubles — and a symbol added to one without the others is a link
error or a corrupt call frame.

## The gate

```bash
./build-and-test.sh          # everything below
./build-and-test.sh fix      # the same, formatting Rust and C++ in place first
```

```text
format · C++ format · file sizes · crates are tested · READMEs warn · prose is compiled · documented paths resolve · rustdoc links resolve · documented commands resolve · the header's symbols are all defined · documented C++ compiles · lint · tests · doctests · the shim compiles · README sections · AGENTS.md · published crates deny missing_docs · comment blocks · comment narration
```

There is no `all` mode; this list is the whole of it, and a last stage,
`the gate is documented`, compares it to what ran. Run it before saying a
change is done, and read the real exit code.

## What only this repository checks

- **`C++ format`** — clang-format 21 or newer over `fui/cpp/`; an older
  release is refused rather than trusted.
- **`the header's symbols are all defined`** — `symbols_agree`: every symbol
  `xpui_fui.h` declares is defined by `xpui_fui.cpp`, the pair no signature
  checker can read.
- **`documented C++ compiles`** — every `cpp` fence in every page is compiled
  with the headers on the path.
- **`the shim compiles`** — `xpui_fui.cpp` against the FreeInkUI headers:
  `FREEINK_SDK_INCLUDE` when it is set, otherwise an unpinned sibling
  checkout [`docs/contributing.md`](docs/contributing.md) names; skips with a
  note only when it finds neither. CI always sets the variable, and with `CI`
  set a skip is a failure.
- **`lint` runs four times**: the host, both bare-metal targets for the two
  backends, and `xpui-screenshot` without `golden` — the shape the simulator
  consumes. `tests` runs `xpui-screenshot` without `golden` too.

## Style that bites here

- **`no_std` in the two backends.** `alloc::` explicitly; `std` is
  `xpui-screenshot`'s and `xpui-abi-check`'s alone, apart from `fui`'s
  host-only `testing` doubles.
- **Every `unsafe` block carries a `// Safety:` line** naming the invariant,
  and every `extern "C"` declaration that takes a pointer carries a
  `# Safety` section. `clippy::undocumented_unsafe_blocks` is denied.
- **Never estimate text metrics.** Every width and height comes from the
  face's own renderer.
- **A font id is the backend's own number, and `0` is none.**
  `xpui-embedded-graphics` hashes the face's bytes; the FreeInkUI shim
  offsets its slot numbers by one so that none is `0`.
- **`draw_text` takes a top-left origin, not a baseline**, and a set bit in
  the FreeInkUI framebuffer is white; `draw_image`'s bit 0 is ink. Three
  polarities, each stated where it is read.
- **Clipping goes through the target**, never through rectangle arithmetic;
  glyphs are rasterised by the font.
- **Every `pub` item is documented.** `#![deny(missing_docs)]` is on in all
  four crates, the `testing` doubles included.
- **A file under `src/` is at most 400 lines.**

## Where the documentation lives, and what proves each piece

| Document | Proven by |
|---|---|
| [`README.md`](README.md) | its paths and commands resolve; it carries no `rust` fence |
| [`docs/README.md`](docs/README.md) | its paths resolve; the README-heading check exempts it, because it is the index of `docs/`, not a front page |
| [`embedded_graphics/README.md`](embedded_graphics/README.md), [`embedded_graphics/docs/design.md`](embedded_graphics/docs/design.md), [`embedded_graphics/docs/screenshots.md`](embedded_graphics/docs/screenshots.md) | doctests, mounted by `embedded_graphics/src/lib.rs` |
| [`embedded_graphics/docs/hardware.md`](embedded_graphics/docs/hardware.md) | its paths resolve; mounted by `embedded_graphics/src/lib.rs`, but it carries no `rust` fence, so no doctest |
| [`fui/README.md`](fui/README.md), [`fui/docs/design.md`](fui/docs/design.md) | doctests, mounted by `fui/src/lib.rs` |
| [`fui/cpp/README.md`](fui/cpp/README.md), [`fui/docs/firmware.md`](fui/docs/firmware.md), [`fui/docs/coverage.md`](fui/docs/coverage.md) | paths and commands resolve; any `cpp` fence is compiled by `documented C++ compiles` |
| [`screenshot/README.md`](screenshot/README.md) | doctests, mounted by `screenshot/src/lib.rs` behind `golden` |
| [`abi-check/README.md`](abi-check/README.md) | paths resolve; its fences are `text` and `toml` on purpose |
| [`docs/choosing-a-backend.md`](docs/choosing-a-backend.md), [`docs/contributing.md`](docs/contributing.md) | paths and commands resolve; the umbrella command is `xpui-dev`'s |
| `AGENTS.md` | the stage list above is compared to what the gate runs |
| every `///` and `//!` | `rustdoc links resolve`, and the two comment checks |

## Git

Never stage, never commit, never push without being asked, each time. The
index is the reviewer's queue; leave new work unstaged. No self-attribution
in a commit message. Never rewrite a commit that exists; a correction is a new
commit. The rules that apply to all ten repositories, and the five review
steps, are in [`xpui`'s `docs/orientation.md`](https://github.com/XPUI-Framework/xpui-framework/blob/main/docs/orientation.md);
how a change is built and reviewed here is in
[`docs/contributing.md`](docs/contributing.md).
