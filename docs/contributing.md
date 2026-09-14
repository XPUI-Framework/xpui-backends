# Contributing to `xpui-backends`

## Building it

`rust-toolchain.toml` pins the toolchain and the two bare-metal targets, so
`cargo build` on a fresh clone installs what it needs. The Rust half builds
and tests with nothing else; the C++ half has two requirements:

- **[clang-format](https://clang.llvm.org/docs/ClangFormat.html) 21 or newer, and it is not optional.** `C++ format` is the
  gate's second stage and fails outright when no binary is found; older
  releases silently ignore options in `.clang-format` and hand back a
  differently formatted file, so it refuses those rather than trusting them.
  `.clang-format` is the firmware's own and is not edited here.
- **The [FreeInkUI](https://github.com/Free-Ink/freeink-sdk/tree/main/libs/ui/FreeInkUI) headers, which are optional.** `the shim compiles` and
  `documented C++ compiles` skip with a note when there are none, and fail
  instead when `CI` is set. The gate
  takes `FREEINK_SDK_INCLUDE` if it is set; otherwise it looks in
  `../../Freeink/freeink-sdk/` and `../crosspoint-reader/freeink-sdk/`, so on
  a machine that has one of those the stages run against **that** checkout
  rather than the revision `fui/freeink-sdk.rev` pins. Set the variable when
  the revision matters; CI always does.

```bash
cargo test --workspace --features xpui-fui/testing   # the suite, on a laptop
./build-and-test.sh                                  # everything CI checks
```

## The gate

A change is not finished until `./build-and-test.sh` passes. It is the same
command CI runs, so a green run locally means what a green tick means there.
The checks are listed in [`AGENTS.md`](../AGENTS.md) and implemented in
[`xtask/`](../xtask/); `./build-and-test.sh fix` formats in place first, Rust
and C++ both.

Three things bite here more than anywhere else:

- **A C symbol lives in four places** that move together;
  [`fui/docs/design.md`](../fui/docs/design.md#the-boundary-four-places-that-move-together)
  names them and what catches a miss.
- **Every FFI call carries a `// Safety:` line** naming the invariant, and
  `clippy::undocumented_unsafe_blocks` is denied.
- **The goldens.** `embedded_graphics/tests/` compares frames against
  committed PNGs; a first run for a new one writes it and fails. Accept an
  intended change with `UPDATE_SNAPSHOTS=1 cargo test --workspace`, then open
  the file and read it before staging. `xpui-screenshot`'s comparator is
  tested against itself: ten cases in `screenshot/src/golden/`, one of which
  is a committed image whose only job is to prove the comparison still returns
  `Err` when it should. Replace its last two lines with `Ok(())` and the whole
  organisation's pixel suite passes while comparing nothing — the one failure
  this technique cannot survive, so it is checked there.

## The review

Five steps, in order, none skipped:

1. The gate passes, with the real exit code read.
2. The [code-reviewer](../.claude/agents/code-reviewer.md) agent reviews the
   change — every finding resolved, not noted.
3. The [docs-reviewer](../.claude/agents/docs-reviewer.md) agent reviews the
   prose, last: it runs every command a document gives and resolves every
   snippet against the API.
4. The author reviews the code and looks at it in the simulator or on a board.
5. They say commit.

A test that cannot fail is worse than no test. Before adding one, break the
code on purpose and confirm the test notices.

## Commits

The subject says what was done — imperative, under fifty characters, one
concern. The body says what changed and why, in under about ten lines,
carrying the fact that is not in the diff. Nothing about how the bug was
found. No self-attribution.

## Working across the repositories

The simulator, the gallery, both firmwares and `xpui-cpp` depend on these
crates through a `git` dependency on `main`, and `xpui-cpp` compiles
`fui/cpp/xpui_fui.cpp` into a host of its own. Before pushing a change, run
the umbrella:

```bash
for d in ../xpui*/; do git -C "$d" fetch --quiet --all; done
cd ../xpui-dev && ./build-and-test.sh cross
```

It builds every crate from the sibling checkouts on disk and says which one
broke. `cross` is that repository's gate, not this one's — run it from there,
not here. The fetch first, because its link check resolves every
`github.com/XPUI-Framework/…` URL against each sibling's `origin/main`, and a
stale remote is a stale answer. `xpui`'s [`docs/orientation.md`](https://github.com/XPUI-Framework/xpui-framework/blob/main/docs/orientation.md)
describes the layout it expects.
