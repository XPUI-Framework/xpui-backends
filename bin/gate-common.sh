# The half of the gate every repository runs.
#
# **This file is copied, not shared.** There is no submodule and nothing is
# published, so every repository carries a byte-identical copy — the nine under
# github.com/XPUI-Framework and, until it is gutted, the monorepo they were
# extracted from, which is ten. `gates_agree` in `xpui-dev` hashes all of them
# and fails the moment two differ. A copy nobody compares is a fork with a
# delay on it.
#
# It defines functions and reads configuration. It runs nothing, dispatches
# nothing, and is sourced by a `build-and-test.sh` that has already `cd`ed to
# its own repository root.
#
# ## What a repository sets before sourcing this
#
# | | |
# |---|---|
# | `SOURCE_ROOTS` | the directories holding crates, for the tree walks. Every one must exist |
# | `TEST_FEATURES` | the feature list the tests and doctests need, or empty |
# | `EXTRA_MANIFESTS` | manifests outside the workspace — formatted and linted by path, because `--workspace` never reaches them |
# | `HOST_WORKSPACE` | 1, or 0 for a repository that builds for no laptop — a firmware whose `.cargo/config.toml` names a board target. It silences the host clippy, test, doctest and rustdoc runs together, because none of the four can run there |
# | `LINT_TARGETS` | bare-metal triples to lint. A trailing `?` means skip when it is not installed, rather than failing a gate somebody cannot fix without a download |
# | `LINT_TARGET_CRATES` | the `-p` list those runs cover |
# | `DOC_TARGETS` | triples for the rustdoc runs a host `cargo doc` cannot reach |
# | `NOT_A_FRONT_PAGE` | tracked READMEs that are not front doors, and so need no banner |
#
# ## What a repository may add
#
# Three optional hooks, called if the repository defines them: `lint_extra`,
# `test_extra` and `doc_link_extra`. They exist because *which* crates and
# targets differ per repository while what the checks do does not.

# Both halves, for `every_check_runs`.
GATE_FILES=("build-and-test.sh" "bin/gate-common.sh")

# The formatters, named once each so `every_check_runs` can see a deletion.
# `check` and `all` run the first list, `fix` the second; a repository
# dispatches these arrays rather than spelling the names out per arm.
FORMAT_CHECK=(rust_format_check cpp_format_check)
FORMAT_FIX=(rust_format_fix cpp_format_fix)

# Defaults, so a repository declares only what it has.
#
# `${ARR[@]:-}` is the wrong idiom here and was tried first: under `set -u` it
# expands an unset array to **one empty element**, not to none, and the first
# repository to leave `NOT_A_FRONT_PAGE` alone failed with `names , which is
# not a file`. `+x` asks whether the name is set at all.
#
# **And an empty array is still not safe to expand.** macOS ships bash 3.2,
# where `"${ARR[@]}"` on an empty array is an unbound variable under `set -u` —
# so every expansion of one of these below is written `"${ARR[@]+"${ARR[@]}"}"`,
# which yields nothing when there is nothing. This repository's gate never met
# it because it never had an empty list; eight repositories with no C++, no
# bare-metal target or no extra manifest meet it immediately.
[ -n "${EXTRA_MANIFESTS+x}" ] || EXTRA_MANIFESTS=()
[ -n "${LINT_TARGETS+x}" ] || LINT_TARGETS=()
[ -n "${LINT_TARGET_CRATES+x}" ] || LINT_TARGET_CRATES=()
[ -n "${DOC_TARGETS+x}" ] || DOC_TARGETS=()
[ -n "${NOT_A_FRONT_PAGE+x}" ] || NOT_A_FRONT_PAGE=()
HOST_WORKSPACE="${HOST_WORKSPACE:-1}"
TEST_FEATURES="${TEST_FEATURES:-}"

say() { printf '\n==> %s\n' "$*"; }

# Rust and C++ are both formatted, always.
#
# `--manifest-path` per extra manifest, because a crate excluded from the
# workspace — a firmware that cannot compile for a laptop — is reached by
# `cargo fmt` no other way.
rust_format_check() {
  say "Rust formatting"
  cargo fmt --check
  local manifest
  for manifest in "${EXTRA_MANIFESTS[@]+"${EXTRA_MANIFESTS[@]}"}"; do
    [ -n "${manifest}" ] || continue
    cargo fmt --check --manifest-path "${manifest}"
  done
}

rust_format_fix() {
  say "Rust formatting (fixing)"
  cargo fmt
  local manifest
  for manifest in "${EXTRA_MANIFESTS[@]+"${EXTRA_MANIFESTS[@]}"}"; do
    [ -n "${manifest}" ] || continue
    cargo fmt --manifest-path "${manifest}"
  done
}

# `bin/clang-format-fix` is the only sanctioned entry point: it enforces
# clang-format 21+, and an older binary does not reject options it does not
# understand — it ignores them and hands back a file formatted differently from
# what CI expects, with no warning.
#
# A repository with no C++ carries no such script, and says that rather than
# walking nothing. It is a different sentence from `skipped:`, which here
# always means a prerequisite is missing and a check that should have run did
# not.
#
# **Whether there is C++ is asked of the tree, not of the script.** Keying it
# on the script's absence made losing the executable bit — a `git archive`, a
# zip, a filesystem without one — print "no C++ in this repository" in a
# repository with twenty-three C++ files, and exit 0. `cpp_snippets_compile`
# already gets this right by counting fences; this counts files.
has_cpp() {
  git ls-files '*.cpp' '*.h' '*.hpp' 2>/dev/null | grep -q .
}

# What the two entry points below should do, said once. Three answers:
# `run` when the formatter is here, `none` when there is nothing to format,
# and a failure when there is C++ and no way to check it.
#
# Its callers read it through `|| state=$?`, which is not decoration: a bare
# call returning non-zero under `set -e` takes the whole gate down before the
# caller can look at the number. That is exactly what happened the first time
# this was written, and the symptom was a gate that stopped after printing
# "C++ formatting" with no message at all.
cpp_formatter_state() {
  [ -x ./bin/clang-format-fix ] && return 0
  if has_cpp; then
    echo "ERROR: this repository has C++ and no runnable bin/clang-format-fix." >&2
    echo "       Copy it from a sibling; an unformattable file is one CI will" >&2
    echo "       reject and nothing here would have told you." >&2
    return 2
  fi
  echo "    no C++ in this repository"
  return 1
}

cpp_format_check() {
  say "C++ formatting"
  local state=0
  cpp_formatter_state || state=$?
  if [ "${state}" -eq 1 ]; then return 0; fi
  if [ "${state}" -eq 2 ]; then return 1; fi
  ./bin/clang-format-fix -c
}

cpp_format_fix() {
  say "C++ formatting (fixing)"
  local state=0
  cpp_formatter_state || state=$?
  if [ "${state}" -eq 1 ]; then return 0; fi
  if [ "${state}" -eq 2 ]; then return 1; fi
  ./bin/clang-format-fix
}

# Runs each named check in order. The dispatch arms call this with
# `FORMAT_CHECK` or `FORMAT_FIX` so no check name is written twice.
run_all() {
  local check
  for check in "$@"; do
    "${check}"
  done
}

# Whether a target is installed, so a check can skip rather than fail.
#
# Called by `lint`, and available to a repository's own `lint_extra` — which is
# the point. The RP2040 firmware's lint used to sit *nested* inside the
# Cortex-M0+ guard, so one missing target silenced two checks and only a table
# said so. A caller that wants that now writes it.
target_installed() {
  rustc --print target-list | grep -qx "$1" \
    && rustup target list --installed | grep -qx "$1"
}

# Clippy, and a warning is a failure.
#
# The host build never parses code behind `cfg(target_os = "none")` — no
# allocator, no panic handler, none of the `no_std` paths — so the bare-metal
# runs are the only gates that reach them before a firmware build does.
#
# A second bare-metal architecture is not a stricter run: neither Cortex-M0+
# nor `riscv32imc` has atomic compare-and-swap. Two exist so a crate cannot be
# checked on one architecture and not the other.
lint() {
  if [ "${HOST_WORKSPACE}" = 1 ]; then
    say "Clippy, host — warnings are failures"
    if [ -n "${TEST_FEATURES}" ]; then
      cargo clippy --workspace --all-targets --features "${TEST_FEATURES}" -- -D warnings
    else
      cargo clippy --workspace --all-targets -- -D warnings
    fi
  fi

  local entry target
  for entry in "${LINT_TARGETS[@]+"${LINT_TARGETS[@]}"}"; do
    [ -n "${entry}" ] || continue
    target="${entry%\?}"
    say "Clippy, ${target} — the code the host build cannot see"
    if [ "${entry}" != "${target}" ] && ! target_installed "${target}"; then
      echo "    skipped: rustup target add ${target}"
      continue
    fi
    cargo clippy --release "${LINT_TARGET_CRATES[@]+"${LINT_TARGET_CRATES[@]}"}" --target "${target}" -- -D warnings
  done

  if declare -F lint_extra >/dev/null; then lint_extra; fi
}

test_suite() {
  say "Tests"
  if [ "${HOST_WORKSPACE}" != 1 ]; then
    echo "    this workspace builds for a board; its tests are in test_extra"
  elif [ -n "${TEST_FEATURES}" ]; then
    cargo test --workspace --features "${TEST_FEATURES}"
  else
    cargo test --workspace
  fi
  if declare -F test_extra >/dev/null; then test_extra; fi
}

# The guides are pulled into their crates with `include_str!`, so every ```rust
# block in them is a doctest. Without this a document can drift from the API it
# documents and nothing notices until a reader tries to follow it.
doc_tests() {
  say "Documented snippets"
  if [ "${HOST_WORKSPACE}" != 1 ]; then
    # rustdoc runs a snippet, so it needs a host target. A firmware's prose is
    # compiled by a host-target crate beside it, dispatched from `test_extra`.
    echo "    this workspace builds for a board; rustdoc cannot run here"
  elif [ -n "${TEST_FEATURES}" ]; then
    cargo test --workspace --features "${TEST_FEATURES}" --doc
  else
    cargo test --workspace --doc
  fi
}

# Every cross-reference in a doc comment resolves.
#
# `cargo doc` is not `cargo test --doc`: the second compiles and runs the code
# blocks and says nothing about the links between them. A broken intra-doc link
# is a rustdoc lint, not a compiler one, so six of them shipped here — two
# pointing at items behind a `device` cfg, three at private implementation
# details, one at a private module — and rustdoc quietly rendered plain text
# where a reader was meant to be able to click.
#
# `--no-deps` because a warning inside a third-party crate is neither this
# repository's fault nor its to fix.
#
# It resolves paths, not meaning: a link that points at the wrong method
# resolves perfectly. That half is `doc_paths`, and then a person.
doc_links() {
  say "Every doc link resolves"
  if [ "${HOST_WORKSPACE}" != 1 ]; then
    echo "    this workspace builds for a board; see the target runs below"
  elif [ -n "${TEST_FEATURES}" ]; then
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --features "${TEST_FEATURES}"
  else
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
  fi

  # And the doc comments a host run never parses — the same blind spot the
  # bare-metal lints exist for.
  local entry target
  for entry in "${DOC_TARGETS[@]+"${DOC_TARGETS[@]}"}"; do
    [ -n "${entry}" ] || continue
    target="${entry%\?}"
    say "Every doc link resolves on ${target}"
    if [ "${entry}" != "${target}" ] && ! target_installed "${target}"; then
      echo "    skipped: rustup target add ${target}"
      continue
    fi
    RUSTDOCFLAGS="-D warnings" cargo doc --release --no-deps \
      "${LINT_TARGET_CRATES[@]+"${LINT_TARGET_CRATES[@]}"}" --target "${target}"
  done

  if declare -F doc_link_extra >/dev/null; then doc_link_extra; fi
}

# Every cross-reference in a doc comment resolves.
#
# `cargo doc` is not `cargo test --doc`: the second compiles and runs the code
# blocks and has always run here, and it says nothing about the links between
# them. A broken intra-doc link is a rustdoc lint, not a compiler one, so six
# of them shipped — two pointing at items behind a `device` cfg, three at
# private implementation details, one at a private module — and rustdoc quietly
# rendered plain text where a reader was meant to be able to click.
#
# `--no-deps` because a warning inside `embedded-graphics` or `u8g2-fonts` is
# neither this repository's fault nor its to fix.
#
# It resolves paths, not meaning: a link that points at the wrong method
# resolves perfectly. That half is still a person's job.
# A markdown URL is not an intra-doc link, and rustdoc never looks at one.
#
# `[text](path.md)` in a doc comment or a guide is passed through untouched:
# rustdoc resolves only ``[`Ident`]``-shaped targets, so a link naming a file
# that does not exist is invisible to every other check here. Three were —
# `crates/boards/src/lib.rs` and `src/bezel.rs` from the simulator guide, and a
# header from `lifecycle.rs` — and the first two broke the day the boards crate
# was carved by vendor, with the gate green.
#
# The rules are narrow on purpose, because this repository's documentation is
# partly *about* links and a noisy check gets an exception list, which is how a
# gate becomes decoration:
#
#   - a target is a path only if it names a location (a slash) or a file (a
#     known extension). `Ident::method` and bare words are rustdoc's job.
#   - a link inside a `code span` or a fenced block is a quotation of a link,
#     not a link. Spec 47 tabulates six deliberately wrong ones; a spec quoting
#     a path in a ```text block is doing the same thing at a different scale.
#   - regex metacharacters mean the "link" is a pattern being written about.
#   - a bare word with no slash and no known extension is left to rustdoc:
#     `[Screen](Screen)` is an identifier link, not a path, and there is no way
#     to tell one from a file called `Screen` without guessing.
#   - symlinks are skipped: `CLAUDE.md` and `AGENTS.md` are `.skills/SKILL.md`,
#     whose relative links resolve from where the real file lives.
#
# Existence is checked case included, because the filesystem this usually runs
# on is not: `[ -e docs/ROADMAP.md ]` is true on APFS and the link is broken
# everywhere else, and a check that passes only where it cannot tell is worse
# than none.
#
# There is no exception list, and no suppression anywhere: on a clean tree
# every link the rules admit resolves.
doc_paths() {
  say "Every path a document links to exists"

  local broken=0
  while IFS= read -r source; do
    local dir
    dir="$(dirname "${source}")"
    local links
    if ! links="$(doc_path_links "${source}")"; then
      printf '  %s: could not be scanned for links\n' "${source}" >&2
      broken=$((broken + 1))
      continue
    fi
    [ -n "${links}" ] || continue

    while IFS= read -r hit; do
      local line="${hit%%:*}"
      local target="${hit#*:}"
      if ! path_exists_exactly "${dir}/${target}"; then
        printf '  %s:%s: links to %s, which does not exist\n' \
          "${source}" "${line}" "${target}" >&2
        broken=$((broken + 1))
      fi
    done <<< "${links}"
  done < <(git_prose_files '*.md'; git_prose_files '*.rs')

  if [ "${broken}" -gt 0 ]; then
    echo "ERROR: ${broken} link(s) above name a file that is not there." >&2
    return 1
  fi
}

# Whether `$1` exists, **case included**.
#
# `[ -e ]` cannot answer this here: APFS is case-insensitive, so
# `[ -e docs/ROADMAP.md ]` is true on the machine this is usually run on and
# the link is broken everywhere else — a check that passes only where it cannot
# tell is worse than none. `find -maxdepth 1 -name` is case-sensitive on both
# BSD and GNU, so every component is confirmed against its own directory.
path_exists_exactly() {
  local rest="$1"
  local part
  local here=""

  # `.` and `..` are resolved textually first, on the positional stack: a `..`
  # that pops a component never needs to be looked up, and popping past the
  # root leaves the stack empty, which is the repository root.
  set --
  while [ -n "${rest}" ]; do
    part="${rest%%/*}"
    if [ "${part}" = "${rest}" ]; then rest=""; else rest="${rest#*/}"; fi
    case "${part}" in
      '' | '.') ;;
      '..') [ "$#" -gt 0 ] && set -- "${@:1:$#-1}" ;;
      *) set -- "$@" "${part}" ;;
    esac
  done

  for part in "$@"; do
    local parent="${here:-.}"
    [ -d "${parent}" ] || return 1
    find "${parent}" -maxdepth 1 -name "${part}" -print -quit 2>/dev/null | grep -q . || return 1
    here="${here:+${here}/}${part}"
  done
}

# `line:target` for every relative path a document links to.
doc_path_links() {
  local source="$1"
  local rust=no
  case "${source}" in *.rs) rust=yes ;; esac

  awk -v rust="${rust}" '
    {
      line = $0
      if (rust == "yes") {
        trimmed = line
        sub(/^[ \t]+/, "", trimmed)
        if (trimmed !~ /^\/\/[\/!]/) next
        sub(/^[ \t]*\/\/[\/!]/, "", line)
      }

      # A fenced block is quoted material: a spec showing a path that is wrong
      # on purpose is not linking to it. Tracked the same way
      # `prose_opening_fences` does, so the two agree about what a fence is.
      {
        fenced = line
        sub(/^[ \t]+/, "", fenced)
        if (fenced ~ /^(`{3,}|~{3,})/) {
          mark = substr(fenced, 1, 1)
          run = 0
          while (substr(fenced, run + 1, 1) == mark) run++
          if (in_fence) {
            if (mark == fence_mark && run >= fence_run) in_fence = 0
          } else {
            in_fence = 1
            fence_mark = mark
            fence_run = run
          }
          next
        }
      }
      if (in_fence) next

      # Blank out code spans: a link written inside one is being quoted.
      out = ""
      rest = line
      while (match(rest, /`+[^`]*`+/)) {
        out = out substr(rest, 1, RSTART - 1)
        for (i = 0; i < RLENGTH; i++) out = out " "
        rest = substr(rest, RSTART + RLENGTH)
      }
      line = out rest

      rest = line
      while (match(rest, /\]\([^)]*\)/)) {
        target = substr(rest, RSTART + 2, RLENGTH - 3)
        rest = substr(rest, RSTART + RLENGTH)

        # CommonMark allows a title after the destination, and angle brackets
        # around it — which is what it prescribes for a path with a space in
        # it, so a pattern that stops at the first space misses exactly the
        # hard case it was avoiding.
        sub(/^[ \t]+/, "", target)
        if (substr(target, 1, 1) == "<") {
          shut = index(target, ">")
          if (shut == 0) continue
          target = substr(target, 2, shut - 2)
        } else {
          sub(/[ \t].*$/, "", target)
        }

        if (target ~ /^(https?:|#|mailto:)/) continue
        if (target ~ /[\\*\[\]|?$^]/) continue
        sub(/#.*$/, "", target)
        if (target == "") continue
        if (target !~ /\//    &&
            target !~ /\.(md|rs|h|hpp|cpp|c|toml|py|sh|yml|yaml|txt|png|ini)$/) continue
        print NR ":" target
      }
    }
  ' "${source}"
}

# Every ```rust block in the prose must be compiled by something.
#
# Two files were wired into the doctest harness and twenty-one blocks in nine
# others were not, so the chrome crate's only usage example had been wrong
# since it was written: it passes `tokens: &TOKENS` where the macro takes a
# closure, and nothing ever tried it. A grep, because "we should doctest that"
# does not run.
# Every ```cpp block in the prose is compiled too.
#
# The Rust blocks have been doctests since the guides existed; the C++ ones
# were prose nobody checked, in the half of the repository a firmware author
# reads first. It found two wrong snippets on its first run, one naming two
# functions that do not exist — so a snippet that stops matching the ABI it
# documents fails the build rather than misleading whoever copies it.
#
# Each block is compiled as its own translation unit, so a snippet has to
# include what it uses — which is what a reader would have to do anyway.
#
# **The blocks are counted twice, independently, and the counts must agree.**
# A fence this cannot read is a block that silently stops being checked, and a
# file that yields nothing looks exactly like a file with nothing in it. The
# first version matched `^```cpp` exactly and missed nine forms out of ten,
# including indented fences — which is already this repository's convention
# inside numbered steps.
#
# `[+]` rather than `\+`: awk processes the backslash when it turns this string
# into a regex, so `\+` would reach its matcher as a repetition operator and
# `C++` would be read differently by the two counters. Caught by the very
# mismatch check above, which is the argument for having it.
CPP_FENCE='^[[:space:]]*(```|~~~)[[:space:]]*([cC][pP][pP]|[cC][+][+])([[:space:]]|$)'

cpp_snippets_compile() {
  say "Every documented C++ snippet compiles"

  # A pre-pass, so a repository whose prose holds no C++ says exactly that.
  # It is a different sentence from `skipped:`, which always means a
  # prerequisite is missing and a check that should have run did not — and if
  # the two print the same line, the one honest skip in the tree teaches
  # everybody to ignore the word.
  local page fences_anywhere=0
  while IFS= read -r page; do
    if grep -qE "${CPP_FENCE}" "${page}"; then
      fences_anywhere=1
      break
    fi
  done < <(tracked_markdown)
  if [[ "${fences_anywhere}" -eq 0 ]]; then
    echo "    no C++ in this repository's prose"
    return 0
  fi

  # Where the headers a snippet includes are. A repository that documents C++
  # defines this; it prints `-I` arguments, or fails with the reason on stdout
  # when a prerequisite it cannot fetch is missing.
  local includes=""
  if declare -F cpp_snippet_includes >/dev/null; then
    if ! includes="$(cpp_snippet_includes)"; then
      echo "    skipped: ${includes:-a header this needs was not found}"
      return 0
    fi
  fi

  local scratch
  scratch="$(mktemp -d)"
  # shellcheck disable=SC2064
  trap "rm -rf '${scratch}'" RETURN

  local total=0
  local expected_total=0

  # The same file set `prose_is_compiled` walks, so the two cannot disagree
  # about which pages are documentation — and so a C++ block in a document
  # nobody has committed yet is compiled too.
  while IFS= read -r page; do
    local expected
    expected="$(grep -cE "${CPP_FENCE}" "${page}" || true)"
    [[ "${expected}" -eq 0 ]] && continue
    expected_total=$((expected_total + expected))

    rm -f "${scratch}"/snippet_*
    # Extracted with awk rather than a read loop: it has to strip carriage
    # returns, accept an indented or `~~~` fence, remove the block's own
    # indentation, and remember the line the fence opened on — because naming
    # the snippet that lied to you is the whole point of the check.
    awk -v dir="${scratch}" -v fence="${CPP_FENCE}" '
      BEGIN { blocks = 0; inside = 0 }
      {
        line = $0
        sub(/\r$/, "", line)
        stripped = line
        sub(/^[ \t]*/, "", stripped)

        if (inside == 0) {
          if (line ~ fence) {
            inside = 1
            blocks++
            match(line, /^[ \t]*/)
            indent = RLENGTH
            out = dir "/snippet_" blocks ".cpp"
            print NR > (dir "/snippet_" blocks ".line")
            close(dir "/snippet_" blocks ".line")
          }
          next
        }

        if (stripped ~ /^(```|~~~)[[:space:]]*$/) {
          inside = 0
          close(out)
          next
        }

        print (indent > 0 ? substr(line, indent + 1) : line) > out
      }
      END { print blocks > (dir "/count") }
    ' "${page}"

    local found
    found="$(cat "${scratch}/count")"
    if [[ "${found}" -ne "${expected}" ]]; then
      echo "ERROR: ${page} has ${expected} C++ block(s) but ${found} could be read." >&2
      echo "       A fence this cannot parse is a block that stops being checked" >&2
      echo "       with no other symptom. Close every block with a bare fence." >&2
      return 1
    fi

    local index
    for ((index = 1; index <= found; index++)); do
      local line
      line="$(cat "${scratch}/snippet_${index}.line")"
      # shellcheck disable=SC2086
      if ! clang++ -std=c++17 -fsyntax-only -Wall -Wextra ${includes} \
        "${scratch}/snippet_${index}.cpp"; then
        echo "ERROR: the C++ block at ${page}:${line} does not compile." >&2
        return 1
      fi
      total=$((total + 1))
    done
  done < <(tracked_markdown)

  if [[ "${total}" -ne "${expected_total}" ]]; then
    echo "ERROR: ${expected_total} C++ block(s) found but ${total} compiled." >&2
    return 1
  fi
  echo "    ${total} block(s) clean"
}

# Fenced as one of these, a block is not Rust and nothing needs to compile it.
# A fence labelled with something on no list is usually a typo for one that
# would have been compiled, so an unknown language is an error rather than a
# shrug.
PROSE_NOT_RUST="text bash sh console cpp c toml yaml yml json ini diff ascii mermaid markdown md python makefile cmake"

# `ignore` in a doc comment means rustdoc parses the block and runs nothing, so
# it can name a function that does not exist and say so forever. A block that
# cannot run is `no_run`; one that is not Rust is `text`. Neither needs
# `ignore`, which is why the budget is zero — and it is a ratchet, so it stays
# there. Spec 57 lists what was found the last time these went unread.
PROSE_IGNORE_BUDGET=0

prose_is_compiled() {
  say "Every documented snippet is compiled"

  # Same reason as `file_sizes`: `grep` exits 2 on a missing directory, and the
  # `|| true` below — which is there so *no matches* is not a failure — would
  # swallow that into "this repository documents nothing".
  local root
  for root in "${SOURCE_ROOTS[@]}"; do
    if [ ! -d "${root}" ]; then
      echo "ERROR: SOURCE_ROOTS names ${root}, which is not a directory." >&2
      return 1
    fi
  done

  # Every path a **doc attribute** names, resolved against the file that names
  # it. Two mistakes are already in this repository's history and both are
  # guarded here:
  #
  #   - Matching by basename let one `tutorial.md` satisfy three includes, so
  #     deleting a crate's include left five Rust blocks uncompiled and green.
  #     Hence the resolution to an absolute path.
  # `#![doc = ..]` counts as well as `#[doc = ..]`: the inner form is how a
  # crate root documents itself, and rejecting it would call a compiled guide
  # uncompiled.
  #
  #   - Matching a bare `include_str!` counted *reading* a file as proof it was
  #     compiled. Only `#[doc = include_str!(..)]` compiles what it pulls in;
  #     `crates/backend/simulator/tests/keys.rs` reads `docs/running.md` as data
  #     to check a key table, and counting that let the guide's Rust blocks go
  #     unbuilt. Hence the attribute in the pattern, not the call.
  local included
  # `--exclude-dir=target` for the same reason `file_sizes` prunes it: five
  # repositories set `SOURCE_ROOTS=(.)`, and `cargo package` extracts crates
  # under `target/package/` carrying the same attributes as the real source.
  #
  # `{ ...; } || true` because a repository whose prose pulls in no Rust at all
  # is not an error, and `pipefail` makes an unmatched `grep` one: `xpui-cpp`'s
  # gate died here on its first run with no message, because the exit status of
  # a command substitution is the exit status of its pipeline.
  included="$(
    { grep -rnoE '#!?[[:space:]]*\[[[:space:]]*doc[[:space:]]*=[[:space:]]*include_str!\("[^"]+"\)' \
      --include='*.rs' --exclude-dir=target "${SOURCE_ROOTS[@]}" || true; } \
      | while IFS= read -r hit; do
          local_file="${hit%%:*}"
          rest="${hit#*:}"
          relative="${rest#*include_str!(\"}"
          relative="${relative%\")}"
          # Resolved with `cd`, so `../` segments collapse the way the compiler
          # resolves them. A path that does not exist resolves to nothing and
          # is reported below rather than silently matching.
          (cd "$(dirname "${local_file}")/$(dirname "${relative}")" 2>/dev/null \
            && printf '%s/%s\n' "$(pwd)" "$(basename "${relative}")") || true
        done | sort -u
  )"

  local problems=0
  local ignored_markdown=()

  while IFS= read -r page; do
    local absolute
    absolute="$(cd "$(dirname "${page}")" && printf '%s/%s' "$(pwd)" "$(basename "${page}")")"
    local compiled=no
    printf '%s\n' "${included}" | grep -qxF "${absolute}" && compiled=yes

    # Fence state, tracked rather than greped. A closing ``` is not an opening
    # one, and a fence indented inside a numbered list is still a fence — the
    # convention this repository already uses in its specs, and what
    # `cpp_snippets_compile` handles for C++.
    local fences
    if ! fences="$(prose_opening_fences "${page}")"; then
      printf '  %s: could not be scanned for fences\n' "${page}" >&2
      problems=$((problems + 1))
      continue
    fi
    [ -n "${fences}" ] || continue

    while IFS= read -r fence; do
      local line="${fence%%:*}"
      local rest="${fence#*:}"
      local language="${rest%%:*}"
      local attributes="${rest#*:}"

      case " ${PROSE_NOT_RUST} " in
        *" ${language} "*) continue ;;
      esac

      if [ -z "${language}" ]; then
        printf '  %s:%s: unlabelled fence. rustdoc compiles these as Rust — label it `text` if it is not.\n' \
          "${page}" "${line}" >&2
        problems=$((problems + 1))
      elif [ "${language}" = "rust" ] || [ "${language}" = "rs" ]; then
        case ",${attributes}," in
          *,ignore,*)
            # `rust,ignore` is a block rustdoc parses and runs no more than a
            # bare `ignore` in a doc comment, and the pages this reaches are
            # the tutorial and the reference. Counted against the same budget,
            # or the ratchet holds on `///` comments and not on the markdown
            # they include.
            ignored_markdown+=("${page}:${line}")
            ;;
          *)
            if [ "${compiled}" = no ]; then
              printf '  %s:%s: a rust block nothing compiles. Pull the file into a #[cfg(doctest)] module, or fence it `text`.\n' \
                "${page}" "${line}" >&2
              problems=$((problems + 1))
            fi
            ;;
        esac
      else
        printf '  %s:%s: unknown fence language %s. Add it to PROSE_NOT_RUST if it is not Rust.\n' \
          "${page}" "${line}" "${language}" >&2
        problems=$((problems + 1))
      fi
    done <<< "${fences}"
  done < <(tracked_markdown)

  # `ignore`, wherever it is written: a `///` comment, or a ```rust,ignore
  # fence in the markdown such a comment includes. Both reach rustdoc the same
  # way and both run nothing.
  local ignored
  ignored="$(prose_ignored_blocks)"
  local count=0
  [ -n "${ignored}" ] && count="$(printf '%s\n' "${ignored}" | wc -l | tr -d ' ')"
  if [ "${#ignored_markdown[@]}" -gt 0 ]; then
    count=$((count + ${#ignored_markdown[@]}))
    ignored="$(printf '%s\n' "${ignored}" "${ignored_markdown[@]}" | sed '/^$/d')"
  fi

  if [ "${count}" -gt "${PROSE_IGNORE_BUDGET}" ]; then
    # Which one is new cannot be known from a count, so all of them are listed
    # rather than a guess pointed at.
    printf '  %s `ignore`d doc block(s), budget %s. Nothing compiles these, which is how two came to name functions that do not exist.\n' \
      "${count}" "${PROSE_IGNORE_BUDGET}" >&2
    printf '%s\n' "${ignored}" | sed 's/^/      /' >&2
    problems=$((problems + 1))
  elif [ "${count}" -lt "${PROSE_IGNORE_BUDGET}" ]; then
    printf 'NOTE: %s `ignore`d doc block(s) cleared. Lower PROSE_IGNORE_BUDGET to %s to keep the ground you won.\n' \
      "$((PROSE_IGNORE_BUDGET - count))" "${count}" >&2
  fi

  if [ "${problems}" -gt 0 ]; then
    echo "ERROR: ${problems} problem(s) above." >&2
    return 1
  fi
}

# Every markdown file git does not ignore — tracked **and** newly written, so a
# document is gated from the moment it is saved rather than from the moment it
# is committed. `git ls-files` rather than `find`, so a gitignored scratch
# directory is skipped by construction instead of by a list of `-not -path`
# arguments that falls behind: an unlabelled fence under `.remember/tmp/` once
# became a failure about nothing.
#
# The existence test is not belt and braces. `--cached` lists what the index
# holds, and a file deleted in the working tree is still in the index until the
# deletion is staged — feeding that name to `awk` fails the walk, and a walk
# that fails silently is a gate that passes silently.
#
# Symlinks are skipped, and the file they point at is checked instead. `CLAUDE.md`
# and `AGENTS.md` are both `.skills/SKILL.md`, which is tracked in its own
# right: checking it three times says the same thing three times, and its
# relative links resolve from `.skills/`, not from the root the links appear to
# sit in.
tracked_markdown() {
  git_prose_files '*.md'
}

git_prose_files() {
  git ls-files --cached --others --exclude-standard -- "$1" | sort -u \
    | while IFS= read -r path; do
        [ -L "${path}" ] && continue
        [ -f "${path}" ] && printf '%s\n' "${path}"
      done
}

# `line:language:attributes` for each **opening** fence in a page, where
# `language` is the word before the first comma and `attributes` is everything
# after it. Both are needed: the language decides whether a block should
# compile, and `rust,ignore` is a block rustdoc runs no more than a bare
# `ignore` — which is the whole reason the budget exists.
#
# A state machine, not a grep, for three reasons a grep gets wrong: a bare ```
# is compiled by rustdoc as Rust and is the case most worth catching; a grep
# counts closing fences too, so every block is reported twice and half of them
# as unlabelled; and a fence may be opened with more backticks than three
# precisely so that a three-backtick fence can appear inside it, which is the
# only way to write about one.
prose_opening_fences() {
  awk '
    {
      line = $0
      sub(/^[ \t]+/, "", line)
      if (line !~ /^(`{3,}|~{3,})/) next

      mark = substr(line, 1, 1)
      run = 0
      while (substr(line, run + 1, 1) == mark) run++

      if (inside) {
        # A closing fence is the same character, at least as long, and carries
        # no info string. Anything shorter is content.
        if (mark == open_mark && run >= open_run && substr(line, run + 1) ~ /^[ \t]*$/)
          inside = 0
        next
      }

      inside = 1
      open_mark = mark
      open_run = run

      info = substr(line, run + 1)
      sub(/^[ \t]+/, "", info)
      sub(/[ \t]+$/, "", info)
      info = tolower(info)

      language = info
      attributes = ""
      comma = index(info, ",")
      if (comma > 0) {
        language = substr(info, 1, comma - 1)
        attributes = substr(info, comma + 1)
      }
      print NR ":" language ":" attributes
    }
  ' "$1"
}

# Every ```ignore fence inside a `///` or `//!` comment.
prose_ignored_blocks() {
  git_prose_files '*.rs' | while IFS= read -r source; do
    awk -v file="${source}" '
      {
        line = $0
        sub(/^[ \t]+/, "", line)
        if (line !~ /^\/\/[\/!]/) next
        sub(/^\/\/[\/!]/, "", line)
        sub(/^[ \t]+/, "", line)
        if (line ~ /^```/ && line ~ /ignore/) print file ":" NR
      }
    ' "${source}"
  done
}

# One file, one job. A file grows past a few hundred lines when two jobs start
# sharing a name, and nothing notices — the module still compiles, the tests
# still pass, and the seam is only visible to whoever reads it next.
#
# The limit is a ratchet, not the principle. Raising it is a decision to argue
# for in a commit message, never a way to land a file.
#
# There is deliberately no allow-list: an exempted file is how a limit becomes
# decoration. Tests are exempt as a class, because an integration test is a
# list of cases and a long list of cases is not a grab-bag — the framework's
# `tests/interactions.rs` is thousands of lines of them, legitimately. So this
# looks only under `src/`.
MAX_SRC_LINES=400

file_sizes() {
  say "No file under src/ is doing two jobs (limit: ${MAX_SRC_LINES} lines)"

  # Same reason as `framework_is_generic`: `find` on a missing root yields no
  # rows, and no rows reads exactly like no offenders.
  #
  # `target` is pruned rather than filtered because a single-crate repository
  # sets `SOURCE_ROOTS=(.)`, and `cargo package` extracts crates under
  # `target/package/` with a `src/` of their own — a third party's file over
  # the limit is not this repository's to split.
  local root
  for root in "${SOURCE_ROOTS[@]}"; do
    if [ ! -d "${root}" ]; then
      echo "ERROR: SOURCE_ROOTS names ${root}, which is not a directory." >&2
      return 1
    fi
  done

  local offenders=0
  while read -r lines path; do
    if [ "${lines}" -gt "${MAX_SRC_LINES}" ]; then
      printf '  %5d  %s\n' "${lines}" "${path}" >&2
      offenders=$((offenders + 1))
    fi
  done < <(find "${SOURCE_ROOTS[@]}" -name target -prune -o \
                -path '*/src/*' -name '*.rs' -exec wc -l {} + \
           | grep -v ' total$' | sort -rn)

  if [ "${offenders}" -gt 0 ]; then
    echo "ERROR: ${offenders} file(s) above, over the ${MAX_SRC_LINES}-line limit." >&2
    echo "       Split by job, not by type — a struct, its constructors and its" >&2
    echo "       impls belong together. If you cannot name a file's job in one" >&2
    echo "       sentence without \"and\", the \"and\" is the seam." >&2
    return 1
  fi
}

readmes_warn() {
  say "Every README says the API can break"

  local missing=0
  local page
  while IFS= read -r page; do
    local excepted=no
    local exception
    for exception in "${NOT_A_FRONT_PAGE[@]+"${NOT_A_FRONT_PAGE[@]}"}"; do
      [ "${page}" = "${exception}" ] && excepted=yes
    done
    [ "${excepted}" = yes ] && continue

    # Lines 3 and 4: the title, a blank, then the warning. Not "somewhere near
    # the top" — a page that puts its pitch above its warning has buried it.
    local head
    head="$(sed -n '3,4p' "${page}")"
    case "${head}" in
      *"Under heavy development"*) ;;
      *)
        printf '  %s does not open with the warning\n' "${page}" >&2
        missing=$((missing + 1))
        continue
        ;;
    esac
    # And it says what it warns about. Three words are not a warning: the check
    # is named for the API breaking, so that is what it checks.
    case "${head}" in
      *"API can break"*) ;;
      *)
        printf '  %s warns, but not that the API can break\n' "${page}" >&2
        missing=$((missing + 1))
        ;;
    esac
  done < <(git_prose_files '*README.md')

  # A README named as an exception that no longer exists is a stale exception,
  # and the next one added inherits the excuse.
  local exception
  for exception in "${NOT_A_FRONT_PAGE[@]+"${NOT_A_FRONT_PAGE[@]}"}"; do
    if [ ! -f "${exception}" ]; then
      echo "ERROR: NOT_A_FRONT_PAGE names ${exception}, which is not a file." >&2
      return 1
    fi
  done

  if [ "${missing}" -gt 0 ]; then
    echo "ERROR: ${missing} README(s) above. Nothing here is published and the" >&2
    echo "       API moves; a visitor should learn that without scrolling." >&2
    return 1
  fi
}

# Every command a document gives names something that is there.
#
# `prose_is_compiled` reads ```rust, `cpp_snippets_compile` reads ```cpp,
# `doc_paths` reads links. **Nothing read a ```bash block**, and it showed:
# step 2 of the framework's beginner tutorial was `cargo run -p xpui-tutorial`
# in a repository whose only package is `xpui`, and the RP2040's flash command
# named `examples/rp2040/…` in a repository with no `examples/` at all. Twenty
# of them, across five repositories, every one written for a monorepo.
#
# It checks what it can know without running anything:
#
#   - `-p NAME` / `--package NAME` — a package in this workspace
#   - `--manifest-path P`, `-S P`, `-C P`, `--out-dir P`, `open P` — P is there
#   - the **first component** of any relative path anywhere in the command,
#     which is what `examples/rp2040/target/…` was wrong at, inside a flash
#     command whose verb this check otherwise leaves alone
#
# **It runs nothing.** A gate that executed a documented command would flash a
# board. Two kinds of command are left alone and counted separately, because a
# single number cannot say which is growing: those whose verb belongs to a
# device or a package manager, and those in a block that has walked out of this
# tree with `cd`.
COMMANDS_NOT_OURS="pio platformio elf2uf2-rs probe-rs espflash esptool espup brew apt apt-get rustup curl wget gh ssh scp docker pipx"

commands_resolve() {
  say "Every documented command names something that is there"

  local packages
  packages="$(cargo metadata --no-deps --format-version 1 2>/dev/null \
    | tr ',' '\n' | sed -n 's/.*"name":"\([^"]*\)".*/\1/p' | sort -u)"
  if [ -z "${packages}" ]; then
    echo "ERROR: cargo metadata named no package, so every -p below would" >&2
    echo "       pass for the wrong reason." >&2
    return 1
  fi

  local problems=0 not_ours=0 elsewhere_blocks=0 elsewhere_lines=0 page

  while IFS= read -r page; do
    # A finished specification is a record of what was asked, and "what existed
    # already" stays in the past tense it was written in — so `done/` is
    # exempt. The live queue is not: a spec still in flight names commands
    # somebody is meant to run.
    case "${page}" in
      docs/specs/done/* | */docs/specs/done/*) continue ;;
    esac

    # `block:line:text`. The block number is what keeps a `cd` from leaking:
    # walking out of the tree ends **that block**, not the rest of the page.
    local rows
    rows="$(awk '
      /^[ \t]*(```|~~~)[ \t]*(bash|sh|shell|console)[ \t]*$/ { inside = 1; block++; next }
      /^[ \t]*(```|~~~)[ \t]*$/                              { inside = 0; next }
      inside { print block ":" NR ":" $0 }
    ' "${page}")"
    [ -n "${rows}" ] || continue

    local row block line text verb gone_block=""
    while IFS= read -r row; do
      block="${row%%:*}"
      row="${row#*:}"
      line="${row%%:*}"
      text="${row#*:}"

      if [ "${gone_block}" = "${block}" ]; then
        elsewhere_lines=$((elsewhere_lines + 1))
        continue
      fi

      text="${text#\$ }"
      text="${text%%#*}"
      [ -n "${text//[[:space:]]/}" ] || continue

      # `sudo` is a prefix, not a verb: looking only at $1 would skip the
      # command it is running, along with every path and package in it.
      verb="$(printf '%s' "${text}" | awk '{ print ($1 == "sudo") ? $2 : $1 }')"
      local where

      # A block may legitimately walk somewhere else — "clone the gallery and
      # run it there" is the honest way to show a command this repository
      # cannot run. That ends **this block**; the next one starts here again.
      #
      # Checked before the words, because `cd elsewhere && cargo run -p x`
      # puts both on one line and the `-p` belongs to the elsewhere.
      #
      # `..` and `../…` leave by definition: `path_exists_exactly` resolves a
      # leading `..` textually and pops to the repository root, so asking it
      # would answer "yes, that is here".
      if [ "${verb}" = "cd" ]; then
        where="$(printf '%s' "${text}" | awk '{ print ($1 == "cd") ? $2 : $3 }')"
        case "${where}" in
          "" | . | ./*) ;;
          .. | ../*) gone_block="${block}"; elsewhere_blocks=$((elsewhere_blocks + 1)) ;;
          *)
            if ! path_exists_exactly "${where}"; then
              gone_block="${block}"
              elsewhere_blocks=$((elsewhere_blocks + 1))
            fi
            ;;
        esac
        continue
      fi

      # One pass over the words. A greedy `sed` was tried first and read only
      # the *last* `-p` on a line, so `-p bogus -p real` passed; and it never
      # ran at all on a backslash continuation, whose first word is `-p`
      # rather than `cargo`. Both are gone with the flag-and-value shape below.
      local word previous="" head
      set -f          # a documented `*` is not a glob against this directory
      for word in ${text}; do
        case "${previous}" in
          # Flags whose value is a package.
          -p | --package)
            if ! printf '%s\n' "${packages}" | grep -qx "${word}"; then
              printf '  %s:%s: `-p %s` is not a package in this workspace\n' \
                "${page}" "${line}" "${word}" >&2
              problems=$((problems + 1))
            fi
            previous="${word}"; continue ;;
          # Flags whose value is a path.
          --manifest-path | -S | -C | --out-dir)
            if ! path_exists_exactly "${word}"; then
              printf '  %s:%s: %s %s does not exist\n' \
                "${page}" "${line}" "${previous}" "${word}" >&2
              problems=$((problems + 1))
            fi
            previous="${word}"; continue ;;
          # Flags whose value only looks like a path.
          --features | -F | --target | --bin | --example | --board)
            previous="${word}"; continue ;;
        esac
        previous="${word}"

        case "${word}" in
          -* | /* | '~'* | *'$'* | \"* | \'* | *'*'* | *'<'* | *'>'* | *=* | *'{'* | *'}'* )
            continue ;;
          */*) ;;
          *) continue ;;
        esac
        head="${word%%/*}"
        case "${head}" in
          . | .. | '' | target | http* ) continue ;;
        esac
        if ! path_exists_exactly "${head}"; then
          printf '  %s:%s: `%s` names %s/, which is not in this repository\n' \
            "${page}" "${line}" "${word}" "${head}" >&2
          problems=$((problems + 1))
        fi
      done
      set +f

      # `open PATH`, which is how a reader is told to look at a screenshot. A
      # generated directory is not in the tree until something has run.
      if [ "${verb}" = "open" ]; then
        local shown
        shown="$(printf '%s' "${text}" | awk '{print $2}')"
        case "${shown}" in
          target/* | */target/* | "" | -*) ;;
          *)
            if ! path_exists_exactly "${shown%/}"; then
              printf '  %s:%s: open %s does not exist\n' "${page}" "${line}" "${shown}" >&2
              problems=$((problems + 1))
            fi
            ;;
        esac
      fi

      case " ${COMMANDS_NOT_OURS} " in
        *" ${verb} "*) not_ours=$((not_ours + 1)) ;;
      esac
    done <<< "${rows}"
  done < <(tracked_markdown)

  if [ "${problems}" -gt 0 ]; then
    echo "ERROR: ${problems} documented command(s) above name something that is" >&2
    echo "       not here. A command that belongs to another repository should" >&2
    echo "       say so with a \`cd\`, or link to it, rather than be printed as" >&2
    echo "       if it ran here." >&2
    return 1
  fi
  printf '    clean (%s for a device or a package manager; %s line(s) in %s block(s) that walked elsewhere)\n' \
    "${not_ours}" "${elsewhere_lines}" "${elsewhere_blocks}"
}

# Every crate is tested, or says in writing why it is not.
#
# A gap is fine when it is chosen. What is not fine is a gap nobody knows
# about, which is what this repository had until somebody counted.
#
# So a crate with neither a `tests/` directory nor a `#[cfg(test)]` module is a
# failure **unless it is named below with a reason**. The list is short on
# purpose: a long one is how a limit becomes decoration.
#
# `UNTESTED_CRATES` holds `path:reason` pairs, and the reason is printed, so
# the exemption is read every time somebody runs the gate rather than buried in
# a manifest.
[ -n "${UNTESTED_CRATES+x}" ] || UNTESTED_CRATES=()

crates_are_tested() {
  say "Every crate is tested, or says why not"

  local manifest dir problems=0 exempt reason tested used=""
  while IFS= read -r manifest; do
    grep -q '^\[package\]' "${manifest}" || continue
    dir="$(dirname "${manifest}")"

    tested=""
    # A `tests/` holding only an empty file is not a tested crate. `-s` is the
    # difference between "there is a file" and "there is something in it".
    if [ -d "${dir}/tests" ]; then
      for candidate in "${dir}"/tests/*.rs; do
        [ -s "${candidate}" ] && tested="yes"
      done
    fi
    # `#[cfg(test)]` on a line that is not a comment, because `grep` does not
    # know Rust and a crate should not pass on a sentence about testing.
    if [ -z "${tested}" ] \
      && grep -rs '#\[cfg(test)\]' "${dir}/src" | grep -qv '^[^:]*: *//'; then
      tested="yes"
    fi

    if [ -n "${tested}" ]; then
      # A crate that has grown tests but is still on the list: the exemption is
      # dead, and a dead exemption is how a short list becomes a long one.
      for entry in "${UNTESTED_CRATES[@]+"${UNTESTED_CRATES[@]}"}"; do
        [ -n "${entry}" ] || continue
        if [ "${entry%%:*}" = "${dir#./}" ]; then
          # Named, so the dangling-entry sweep below does not report it twice.
          used="${used} ${entry%%:*}"
          printf '  %s is exempt and has tests. Remove it from UNTESTED_CRATES.\n' \
            "${dir#./}" >&2
          problems=$((problems + 1))
        fi
      done
      continue
    fi

    exempt=""
    for entry in "${UNTESTED_CRATES[@]+"${UNTESTED_CRATES[@]}"}"; do
      [ -n "${entry}" ] || continue
      if [ "${entry%%:*}" = "${dir#./}" ]; then
        exempt="yes"
        used="${used} ${entry%%:*}"
        reason="${entry#*:}"
        printf '    %-24s %s\n' "${dir#./}" "${reason}"
      fi
    done
    [ -n "${exempt}" ] && continue

    printf '  %s has no tests/ and no #[cfg(test)], and is not exempt\n' "${dir#./}" >&2
    problems=$((problems + 1))
  done < <(git ls-files '*Cargo.toml' | grep -v '^target/')

  # An entry naming a crate that is not here at all — a path that moved, or a
  # repository that was split out from under it.
  for entry in "${UNTESTED_CRATES[@]+"${UNTESTED_CRATES[@]}"}"; do
    [ -n "${entry}" ] || continue
    case " ${used} " in
      *" ${entry%%:*} "*) ;;
      *)
        printf '  UNTESTED_CRATES names %s, which is not a crate here.\n' "${entry%%:*}" >&2
        problems=$((problems + 1))
        ;;
    esac
  done

  if [ "${problems}" -gt 0 ]; then
    echo "ERROR: ${problems} crate(s) above are untested and unexplained. Add a" >&2
    echo "       test, or add the crate to UNTESTED_CRATES with the reason —" >&2
    echo "       a gap that was chosen is not the same as one nobody saw." >&2
    return 1
  fi
}

# Every check this repository defines is one this repository runs.
#
# It replaces `ownership_is_complete`, which asked *which repository* a check
# belonged to. That question had one answer — the header table spec 46 wrote —
# and the extraction answered it for good: a check belongs to the file it is
# in. The question that replaces it is the one copying creates. A repository
# takes this file, writes a dispatch, and leaves `doc_paths` out of it; the
# check is present, defined, documented, and never called, and nothing says so.
# That is how fifty dead links got in.
#
# So: every function either half defines is named on a non-comment line
# somewhere other than its own definition. A helper counts because its caller
# names it; a check counts because a dispatch names it; a function nothing
# names at all fails.
#
# **That only works if each name appears once.** The first version let a check
# be listed in three dispatch arms, so deleting `rust_format_check` from the
# `check)` arm — the one CI runs — left two mentions and a green tick, with the
# formatter no longer running. Hence `FORMAT_CHECK` and `FORMAT_FIX` below and
# a single `gates` per repository: every check is written down exactly once,
# and deleting it there takes its second mention with it.
every_check_runs() {
  say "Every check defined here is dispatched"

  local body defined missing=0 name mentions
  body="$(cat "${GATE_FILES[@]}" | sed 's/#.*//')"
  defined="$(printf '%s\n' "${body}" \
    | sed -n 's/^\([A-Za-z_][A-Za-z_0-9]*\)() {.*/\1/p' | sort -u)"

  while IFS= read -r name; do
    [ -n "${name}" ] || continue
    # Its own definition, and every other mention. One means nothing calls it.
    mentions="$(printf '%s\n' "${body}" | grep -c "\b${name}\b" || true)"
    if [ "${mentions}" -lt 2 ]; then
      printf '  %s is defined and never called\n' "${name}" >&2
      missing=$((missing + 1))
    fi
  done <<< "${defined}"

  if [ "${missing}" -gt 0 ]; then
    echo "ERROR: ${missing} check(s) above are dead. Dispatch them, or delete them." >&2
    return 1
  fi
}
