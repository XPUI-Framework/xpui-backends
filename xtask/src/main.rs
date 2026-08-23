//! The gate for `xpui-backends`.
//!
//! Everything CI checks, in one command, and **only what this repository has
//! to check**. It holds the FreeInkUI shim, so it carries the
//! C++ stages that seven of the ten do not: the shim's own formatting, its
//! header against its definitions, the documented snippets, and a syntax-only
//! compile.
//!
//! ```text
//! ./build-and-test.sh          check everything
//! ./build-and-test.sh fix      format in place first
//! ```
//!
//! Each repository in the organisation has its own copy of this shape, holding
//! its own list. **This file is the part that is meant to differ**; the modules
//! under it are byte-identical, and `shared_files_agree` in `xpui-dev` hashes
//! all seven across the nine, so a fix to the fence scanner cannot land in one
//! repository and not the rest.
//!
//! A check written and never listed below is a dead function, which clippy
//! fails the build over. That is what a hand-written "is every check
//! dispatched?" check used to do, and it does it better.

mod cargo;
mod commands;
mod cpp;
mod docs;
mod faults;
mod fences;
mod paths;
mod prose;
mod tree;

use std::path::PathBuf;

use std::process::ExitCode;

/// Files under a `src/` may not exceed this. A ratchet, not a law of nature:
/// raising it is a decision to argue for in a commit message, never a way to
/// land a file.
const LINE_LIMIT: usize = 400;

/// Crates with no tests, and why. The reason prints on every run so it is
/// re-read rather than accumulated — and an exemption for a crate that has
/// since grown tests fails, rather than sitting there as a comment nobody
/// removes.
const UNTESTED: [(&str, &str); 0] = [];

/// Fence languages this repository's prose is written in.
///
/// The list exists so that ` ```rustt ` is an error rather than a shrug: an
/// unknown language silently compiles nothing, and a typo is the likeliest
/// way for a Rust block to stop being checked.
const KNOWN_LANGUAGES: [&str; 19] = [
    "text", "bash", "sh", "shell", "console", "cpp", "c", "toml", "yaml", "yml", "json", "ini",
    "diff", "ascii", "mermaid", "markdown", "md", "python", "cmake",
];

/// Documents whose ```rust is illustrative rather than compilable.
const NOT_COMPILED: [&str; 0] = [];

/// Pages that are not a repository's front door and carry no banner.
const NOT_A_FRONT_PAGE: [&str; 0] = [];

/// Bare-metal targets the two backends are linted for.
const BARE_METAL: [(&str, bool); 2] = [
    ("riscv32imc-unknown-none-elf", true),
    ("thumbv6m-none-eabi", false),
];

/// What the bare-metal runs compile. `xpui-screenshot` is host-only and
/// `xpui-abi-check` is a build-time tool, so neither belongs here.
const LINT_CRATES: [&str; 4] = ["-p", "xpui-embedded-graphics", "-p", "xpui-fui"];

/// The feature the host tests need: the FreeInkUI backend's doubles, so a host
/// test binary links without a C++ toolchain.
const TEST_FEATURES: &str = "--features=xpui-fui/testing";

fn main() -> ExitCode {
    // Every path in every check is relative to the repository root, so the
    // gate answers the same from anywhere it is invoked.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask/..");
    std::env::set_current_dir(root).expect("the repository root");

    // A typo is not a check. The shell this replaced rejected an unknown
    // argument, and a gate that silently treats `fx` as `check` is a gate that
    // reports a pass for a run nobody asked for.
    let fix = match std::env::args().nth(1).as_deref() {
        None | Some("check") => false,
        Some("fix") => true,
        Some(other) => {
            eprintln!("unknown argument `{other}`\nusage: ./build-and-test.sh [check|fix]");
            return ExitCode::from(2);
        }
    };
    let mut failed = 0;

    let mut gate: Vec<(&str, Box<dyn Fn() -> Result<String, String>>)> = vec![
        (
            "format",
            Box::new(move || {
                if fix {
                    cargo::cargo(&["fmt", "--all"])
                } else {
                    cargo::cargo(&["fmt", "--all", "--check"])
                }
            }),
        ),
        ("file sizes", Box::new(|| tree::file_sizes(LINE_LIMIT))),
        (
            "crates are tested",
            Box::new(|| tree::crates_are_tested(&UNTESTED)),
        ),
        (
            "READMEs warn",
            Box::new(|| tree::readmes_warn(&NOT_A_FRONT_PAGE)),
        ),
        (
            "prose is compiled",
            Box::new(|| prose::is_compiled(&NOT_COMPILED, &KNOWN_LANGUAGES)),
        ),
        ("documented paths resolve", Box::new(docs::doc_paths)),
        (
            "rustdoc links resolve",
            Box::new(|| cargo::rustdoc(&["--workspace", TEST_FEATURES])),
        ),
        (
            "documented commands resolve",
            Box::new(|| commands::resolve(&cargo::packages(), &[])),
        ),
        (
            "the header's symbols are all defined",
            Box::new(symbols_agree),
        ),
        (
            "documented C++ compiles",
            Box::new(|| cpp::snippets_compile(snippet_includes(), true)),
        ),
        ("lint", Box::new(lint)),
        ("tests", Box::new(tests)),
        (
            "doctests",
            Box::new(|| cargo::cargo(&["test", "--workspace", TEST_FEATURES, "--doc"])),
        ),
        ("the shim compiles", Box::new(shim_compiles)),
    ];

    // C++ formatting sits beside the Rust formatting, and is the same
    // decision: this shim is the half a firmware author reads, so it is held
    // to the standard of the firmware it plugs into.
    gate.insert(1, ("C++ format", Box::new(move || cpp::format(fix))));

    for (name, check) in gate.drain(..) {
        println!("\n==> {name}");
        match check() {
            Ok(note) if note.is_empty() => println!("    ok"),
            Ok(note) => println!("    {}", note.replace('\n', "\n    ")),
            Err(why) => {
                println!("{why}");
                eprintln!("FAILED: {name}");
                failed += 1;
            }
        }
    }

    if failed == 0 {
        println!("\nChecks passed.");
        ExitCode::SUCCESS
    } else {
        eprintln!("\n{failed} check(s) failed.");
        ExitCode::FAILURE
    }
}

/// Clippy: the host, both bare-metal targets, and `xpui-screenshot` without
/// its optional half.
///
/// That last run is not redundant. `--workspace` unifies features, so a crate
/// anything opts into is always compiled with that feature on — and
/// `xpui-screenshot` without `golden` is exactly what `xpui-simulator`
/// consumes and what nothing above ever builds. A helper left ungated beside
/// its gated caller is dead code there, and `-D warnings` makes that a hard
/// failure for the consumer and not for us.
fn lint() -> Result<String, String> {
    cargo::cargo(&[
        "clippy",
        "--workspace",
        "--all-targets",
        TEST_FEATURES,
        "--",
        "-D",
        "warnings",
    ])?;
    let mut notes = vec!["host".to_string()];
    bare_metal(&mut notes)?;
    cargo::cargo(&[
        "clippy",
        "-p",
        "xpui-screenshot",
        "--no-default-features",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ])?;
    notes.push("xpui-screenshot without golden".into());
    Ok(notes.join(", "))
}

/// The workspace, plus the two configurations `--workspace` cannot reach.
fn tests() -> Result<String, String> {
    cargo::cargo(&["test", "--workspace", TEST_FEATURES])?;
    // `embedded_graphics` has one test that exists only behind a feature,
    // because what it proves exists only behind that feature: that `Backend`
    // is genuinely `Sync` rather than claiming to be. Without this line
    // `tests/sync.rs` compiles to nothing and never executes.
    cargo::cargo(&[
        "test",
        "-p",
        "xpui-embedded-graphics",
        "--features",
        "critical-section",
    ])?;
    // `--all-targets` does not include doctests, and that gap let a README
    // mounted as a doctest reference a `golden`-only function: the crate
    // stopped compiling its own prose with the feature off, which is the
    // shape `xpui-simulator` consumes.
    cargo::cargo(&[
        "test",
        "-p",
        "xpui-screenshot",
        "--no-default-features",
        "--doc",
    ])?;
    Ok("workspace, critical-section, screenshot without golden".into())
}

/// The boundary this repository owns: its own header against its own shim.
///
/// `fui/tests/abi.rs` compares *signatures*. This compares *presence*, which
/// that cannot read, and the two do not overlap. The other two pairs read a
/// C++ host and a firmware, and moved to `xpui-cpp` with them.
fn symbols_agree() -> Result<String, String> {
    let header = cpp::symbols("xpui_fui_", &[PathBuf::from("fui/cpp/xpui_fui.h")])?;
    if header.is_empty() {
        return Err("no xpui_fui_ symbols in fui/cpp/xpui_fui.h — nothing was compared".into());
    }
    let shim = cpp::symbols("xpui_fui_", &[PathBuf::from("fui/cpp/xpui_fui.cpp")])?;
    cpp::symbols_agree(
        "xpui_fui.h and xpui_fui.cpp",
        "header",
        &header,
        "shim",
        &shim,
    )
    .map_err(|why| {
        format!(
            "{why}\n\nA symbol in a header with no definition is a link error waiting\n\
                 for whoever includes it."
        )
    })?;
    Ok(format!("{} symbol(s)", header.len()))
}

/// Where a documented C++ snippet's headers are, or the reason there are none.
fn snippet_includes() -> Result<Vec<String>, String> {
    let sdk = cpp::freeink_include()
        .ok_or("FreeInkUI headers not found. Set FREEINK_SDK_INCLUDE to run it.")?;
    Ok(vec![
        format!("-I{}", sdk.display()),
        "-Ifui/cpp".to_string(),
    ])
}

/// The shim compiles, on its own terms.
///
/// `-fno-exceptions -fno-rtti` because it is compiled into a firmware that
/// builds with both off; a shim that needs either would link there and fail
/// nowhere else.
fn shim_compiles() -> Result<String, String> {
    let Some(sdk) = cpp::freeink_include() else {
        return Ok("skipped: FreeInkUI headers not found. Set FREEINK_SDK_INCLUDE.".into());
    };
    let status = std::process::Command::new("clang++")
        .args([
            "-std=c++17",
            "-fsyntax-only",
            "-fno-exceptions",
            "-fno-rtti",
            "-Wall",
            "-Wextra",
        ])
        .arg(format!("-I{}", sdk.display()))
        .args(["-Ifui/cpp", "fui/cpp/xpui_fui.cpp"])
        .status()
        .map_err(|e| format!("clang++: {e}"))?;
    if status.success() {
        Ok("clean".into())
    } else {
        Err("fui/cpp/xpui_fui.cpp does not compile".into())
    }
}

/// Clippy on each bare-metal target, with warnings as errors.
///
/// The host build never parses code behind `cfg(target_os = "none")` — no
/// allocator, no panic handler — so these are the only gates that reach it
/// before a firmware build does. Neither target has atomic compare-and-swap:
/// load and store only, never `swap`, `fetch_or` or `compare_exchange`. The
/// second is a second architecture rather than a stricter one.
fn bare_metal(notes: &mut Vec<String>) -> Result<(), String> {
    {
        for (triple, required) in BARE_METAL {
            {
                if !cargo::target_installed(triple) {
                    {
                        if required {
                            {
                                return Err(format!(
                                    "{triple} is not installed, and it is the only gate that reaches\n\
                     this repository's no_std paths. `rustup target add {triple}`"
                                ));
                            }
                        }
                        notes.push(format!("{triple} SKIPPED — rustup target add {triple}"));
                        continue;
                    }
                }
                let mut arguments = vec!["clippy", "--release"];
                arguments.extend_from_slice(&LINT_CRATES);
                arguments.extend_from_slice(&["--target", triple, "--", "-D", "warnings"]);
                cargo::cargo(&arguments)?;
                notes.push(triple.to_string());
            }
        }
        Ok(())
    }
}
