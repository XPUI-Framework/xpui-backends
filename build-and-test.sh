#!/usr/bin/env bash

# Everything CI checks in this repository, in one command.
#
#   ./build-and-test.sh          format, lint, test, snippets, and the C++
#   ./build-and-test.sh check    the same thing; the name CI uses
#   ./build-and-test.sh fix      format Rust and C++ in place first
#
# **Half of what runs is in `bin/gate-common.sh`**, of which every repository
# in the organisation carries a byte-identical copy. This file is what this
# repository configures, what only it checks, and the order they run in.
# `xpui-dev` compares the nine copies and runs all nine gates.
#
# Two backends and two helpers share this repository, and grouping costs a
# consumer nothing: cargo resolves per crate, so a manifest naming `xpui-fui`
# compiles `xpui-fui` alone. The one real difference is provisioning — `fui`
# needs clang and the FreeInk SDK, `embedded_graphics` needs neither — and that
# is a `paths:` filter in CI rather than a second repository.

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${PROJECT_DIR}"

SOURCE_ROOTS=(abi-check embedded_graphics fui screenshot)

# `xpui-fui/testing` supplies the C symbols so that crate's tests link without
# a firmware. Miss it and the test binary fails to link rather than failing a
# test, which reads like a broken toolchain.
TEST_FEATURES="xpui-fui/testing"

# Both backends run on device, so both bare-metal architectures are linted over
# the same crate list. Neither has atomic compare-and-swap; the second is not
# the stricter run, it is the second architecture. The `?` says Cortex-M0+ may
# skip when the target is not installed rather than failing a gate somebody
# cannot fix without a download.
#
# `xpui-screenshot` is absent because it is `std` by design and host-only, and
# `xpui-abi-check` because it is a checker rather than something checked.
HOST_WORKSPACE=1
LINT_TARGETS=("riscv32imc-unknown-none-elf" "thumbv6m-none-eabi?")
LINT_TARGET_CRATES=(-p xpui-embedded-graphics -p xpui-fui)

. bin/gate-common.sh

# ---------------------------------------------------------------------------
# What only this repository checks.
# ---------------------------------------------------------------------------

lint_extra() {
  # `--workspace` unifies features, so a crate that anything opts into is
  # always compiled with that feature on. `xpui-screenshot` without `golden` is
  # what `xpui-simulator` actually uses and what nothing above ever builds: a
  # helper left ungated beside its gated caller is dead code there, and
  # `-D warnings` makes that a hard failure for a consumer and not for us.
  say "Clippy, xpui-screenshot without its optional half"
  cargo clippy -p xpui-screenshot --no-default-features --all-targets -- -D warnings
}

test_extra_golden_off() {
  # `--all-targets` above does **not** include doctests, and that gap let a
  # README mounted as a doctest reference a `golden`-only function: the crate
  # stopped compiling its own prose with the feature off, which is exactly the
  # shape `xpui-simulator` consumes. One line, because the clippy run beside it
  # has been guarding this configuration since spec 43 and could not see this.
  say "Documented snippets, xpui-screenshot without its optional half"
  cargo test -p xpui-screenshot --no-default-features --doc
}

test_extra() {
  # `embedded_graphics` has one test that only exists behind a feature, because
  # what it proves only exists behind that feature: that `Backend` is genuinely
  # `Sync` rather than claiming to be. The workspace run does not enable it, so
  # without this line `tests/sync.rs` compiles to nothing and never executes.
  say "Tests, with the backend's state guarded"
  cargo test -p xpui-embedded-graphics --features critical-section

  test_extra_golden_off
}

# Where the FreeInkUI headers are, or nothing.
#
# Neither candidate is a submodule and neither is required: a missing SDK skips
# the C++ stage with a note rather than failing a gate somebody cannot fix
# without a download. `FREEINK_SDK_INCLUDE` overrides both.
#
# Whichever is found is used at whatever revision it happens to be checked out
# at. CI clones the SHA that `xpui-cpp` pins instead, so a local pass is
# against the SDK you have and a CI pass is against the one that is pinned.
freeink_include() {
  if [[ -n "${FREEINK_SDK_INCLUDE:-}" ]]; then
    printf '%s' "${FREEINK_SDK_INCLUDE}"
    return
  fi
  for candidate in \
    "../../Freeink/freeink-sdk/libs/ui/FreeInkUI/include" \
    "../crosspoint-reader/freeink-sdk/libs/ui/FreeInkUI/include"; do
    if [[ -d "${candidate}" ]]; then
      printf '%s' "${candidate}"
      return
    fi
  done
}

# Where a documented C++ snippet's headers are, or the reason there are none.
cpp_snippet_includes() {
  local sdk
  sdk="$(freeink_include)"
  if [[ -z "${sdk}" || ! -d "${sdk}" ]]; then
    printf 'FreeInkUI headers not found. Set FREEINK_SDK_INCLUDE to run it.'
    return 1
  fi
  printf -- '-I %s -I fui/cpp' "${sdk}"
}

cpp_compiles() {
  say "The FreeInkUI shim compiles"
  local sdk
  sdk="$(freeink_include)"
  if [[ -z "${sdk}" || ! -d "${sdk}" ]]; then
    echo "    skipped: FreeInkUI headers not found."
    echo "    Set FREEINK_SDK_INCLUDE to <sdk>/libs/ui/FreeInkUI/include to run it."
    return 0
  fi
  clang++ -std=c++17 -fsyntax-only -fno-exceptions -fno-rtti -Wall -Wextra \
    -I "${sdk}" -I fui/cpp fui/cpp/xpui_fui.cpp
  echo "    clean"
}

# The `xpui_fui_*` symbols a header promises, out of the files that answer it.
ffi_symbols() {
  local pattern="$1"
  shift
  grep -hoE "${pattern}" "$@" | sort -u
}

# Two symbol lists, and what each side is called when they disagree.
same_symbols() {
  local what="$1" left_name="$2" right_name="$3" left="$4" right="$5"
  local only_left only_right
  only_left="$(comm -23 <(printf '%s\n' "${left}") <(printf '%s\n' "${right}"))"
  only_right="$(comm -13 <(printf '%s\n' "${left}") <(printf '%s\n' "${right}"))"
  if [[ -z "${only_left}" && -z "${only_right}" ]]; then
    return 0
  fi
  echo "  ${what}:" >&2
  [[ -n "${only_left}" ]] && printf '    only in %s: %s\n' "${left_name}" "$(echo ${only_left})" >&2
  [[ -n "${only_right}" ]] && printf '    only in %s: %s\n' "${right_name}" "$(echo ${only_right})" >&2
  return 1
}

# The boundary this repository owns: its own header against its own shim.
#
# `fui/tests/abi.rs` compares *signatures* — two swapped parameters link fine,
# because C has no mangling to disagree with, and the result is a corrupt call
# frame. This compares *presence*, which that cannot read: a symbol declared in
# a header with nothing defining it is a link error waiting for whoever
# includes it. The two do not overlap.
#
# The other two pairs read a C++ host and a firmware, and moved to `xpui-cpp`
# with them.
ffi_symbols_agree() {
  say "Every C header's symbols are defined by the C++ that answers them"
  local fui="xpui_fui_[a-z0-9_]+"
  if ! same_symbols "xpui_fui.h and xpui_fui.cpp" header shim \
    "$(ffi_symbols "${fui}" fui/cpp/xpui_fui.h)" \
    "$(ffi_symbols "${fui}" fui/cpp/xpui_fui.cpp)"; then
    echo "ERROR: the pair above disagrees. A symbol in a header with no" >&2
    echo "       definition is a link error waiting for whoever includes it." >&2
    return 1
  fi
  echo "    agreed"
}

# ---------------------------------------------------------------------------

gates() {
  file_sizes
  every_check_runs
  readmes_warn
  prose_is_compiled
  doc_paths
  commands_resolve
  ffi_symbols_agree
  cpp_snippets_compile
  lint
  test_suite
  doc_tests
  doc_links
  cpp_compiles
}

case "${1:-check}" in
  check)
    run_all "${FORMAT_CHECK[@]}"
    gates
    printf '\nChecks passed.\n'
    ;;
  fix)
    run_all "${FORMAT_FIX[@]}"
    gates
    printf '\nFormatted and checked.\n'
    ;;
  *)
    echo "usage: ./build-and-test.sh [check|fix]" >&2
    exit 2
    ;;
esac
