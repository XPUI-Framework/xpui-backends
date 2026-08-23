#!/usr/bin/env bash
#
# Everything CI checks, in one command. The checks themselves are in `xtask/`,
# in Rust, where the text handling has tests; this only starts them.
#
#   ./build-and-test.sh          check everything
#   ./build-and-test.sh fix      format in place first

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
exec cargo run --quiet -p xtask -- "${1:-check}"
