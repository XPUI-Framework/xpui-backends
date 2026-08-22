//! Every C symbol, and whether both sides agree about its **types**.
//!
//! `ffi_symbols_agree()` in `build-and-test.sh` checks that the names line up.
//! Names are the easy half. A parameter reordered on one side and not the
//! other still links — C has no mangling to disagree with — and the result is
//! a corrupt call frame: the shim reads a width where a height was passed, and
//! the failure surfaces as a rendering fault somewhere unrelated.
//!
//! The parsing and comparing is [`xpui_abi_check`], which is its own crate
//! because the C++ host example owns two boundaries of its own and a second
//! copy could not see its own drift. This file is the list of pairs, and
//! nothing else. Five of them, in two languages, across two crates:
//!
//! | C | Rust | who defines |
//! |---|---|---|
//! | `cpp/xpui_fui.h` | `src/raw.rs` | C++ |
//! | `cpp/xpui_fui.h` | `src/testing/stubs.rs` | Rust, for tests |
//! | `cpp/xpui_screen.h` | `src/lifecycle.rs` | Rust |
//! | `examples/cpp_host/cpp/xpui_host.h` | `examples/cpp_host/src/raw.rs` | C++ |
//! | `examples/cpp_host/cpp/xpui_app.h` | `examples/cpp_host/src/lib.rs`, plus `src/lifecycle.rs` for what `register_screen!` generates | Rust |
//!
//! The last two are read across a crate boundary, which is what [`sibling`]
//! is for and what its doc comment argues about.

use std::path::{Path, PathBuf};

use xpui_abi_check::{
    Boundary, Signatures, assert_agree, signatures_from_c, signatures_from_register_screen,
    signatures_from_rust,
};

// -- this boundary ----------------------------------------------------------

/// The drawing ABI, as this crate spells it.
///
/// `xpui_fui_present` and `xpui_fui_set_present` are skipped because the Rust
/// side never calls them: a host defines the first and installs the second, so
/// both belong to the header and the shim alone. `set_present` also takes a
/// function pointer, which is why the exclusion has to reach the parser.
const FUI: Boundary<'_> = Boundary {
    prefix: "xpui_",
    skip: &["xpui_fui_present", "xpui_fui_set_present"],
    // The row callback's typedef.
    types: &[("xpui_fui_cell_fn", "CellFn")],
    // ...which Rust writes at three different depths depending on the file.
    // Longest first, so a prefix does not eat the path it is part of.
    rust_aliases: &[("crate::raw::CellFn", "CellFn"), ("raw::CellFn", "CellFn")],
};

/// A boundary with no typedef of its own and nothing the Rust side leaves out.
///
/// Three pairs are this shape: the lifecycle, which is entirely inside this
/// crate, and the C++ host's two. They share a constant because they share a
/// description, not because they belong to the same thing — the host's two
/// move out under spec 52 and this one does not.
const PLAIN: Boundary<'_> = Boundary {
    prefix: "xpui_",
    skip: &[],
    types: &[],
    rust_aliases: &[],
};

// -- finding the files ------------------------------------------------------

/// One of this crate's own files.
///
/// Resolved from the manifest, so it does not care where the crate sits. The
/// previous version of this file counted three directory levels up to a
/// workspace root; moving the crate made that resolve to some other directory
/// and read nothing, which is the failure a checker must not have.
fn own(relative: &str) -> (String, PathBuf) {
    read(Path::new(env!("CARGO_MANIFEST_DIR")).join(relative))
}

/// A file belonging to another crate in the same workspace.
///
/// Walks up for the manifest that declares a `[workspace]`, rather than
/// counting levels: a depth that is wrong resolves somewhere instead of
/// failing, and a checker that reads nothing reports agreement it never
/// established.
///
/// **This is the seam.** The two pairs below belong to `examples/cpp_host` and
/// are only here because that crate sets `test = false` — every `xpui_host_*`
/// symbol it calls is defined by the C++ half, so a harness there could link
/// only against doubles, a fourth place for the ABI to rot. When it becomes
/// its own repository they move to a package beside it that depends on
/// [`xpui_abi_check`] and not on the crate itself. See `docs/specs/done/42-an-abi-check-that-survives-a-move.md`.
fn sibling(relative: &str) -> (String, PathBuf) {
    let mut root = Path::new(env!("CARGO_MANIFEST_DIR"));
    loop {
        let manifest = root.join("Cargo.toml");
        if std::fs::read_to_string(&manifest).is_ok_and(|text| text.contains("[workspace]")) {
            return read(root.join(relative));
        }
        root = root.parent().unwrap_or_else(|| {
            panic!(
                "no [workspace] manifest above {}, so {relative} cannot be found",
                env!("CARGO_MANIFEST_DIR")
            )
        });
    }
}

fn read(path: PathBuf) -> (String, PathBuf) {
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    (text, path)
}

// -- the boundaries ---------------------------------------------------------

#[test]
fn the_drawing_abi_agrees_with_its_rust_declarations() {
    let (header, header_path) = own("cpp/xpui_fui.h");
    let (rust, _) = own("src/raw.rs");

    assert_agree(
        "xpui_fui.h and src/raw.rs",
        &signatures_from_c(&header, &header_path, &FUI),
        &signatures_from_rust(&rust, &FUI),
    );
}

#[test]
fn the_drawing_abi_agrees_with_its_host_doubles() {
    let (header, header_path) = own("cpp/xpui_fui.h");
    let (rust, _) = own("src/testing/stubs.rs");

    // The doubles are the file that rots quietly, because nothing calls them
    // until a test finally does.
    assert_agree(
        "xpui_fui.h and src/testing/stubs.rs",
        &signatures_from_c(&header, &header_path, &FUI),
        &signatures_from_rust(&rust, &FUI),
    );
}

#[test]
fn the_lifecycle_agrees_with_the_rust_that_defines_it() {
    let (header, header_path) = own("cpp/xpui_screen.h");
    let (rust, _) = own("src/lifecycle.rs");

    assert_agree(
        "xpui_screen.h and src/lifecycle.rs",
        &signatures_from_c(&header, &header_path, &PLAIN),
        &signatures_from_rust(&rust, &PLAIN),
    );
}

#[test]
fn what_the_host_answers_agrees_with_what_rust_asks_for() {
    let (header, header_path) = sibling("examples/cpp_host/cpp/xpui_host.h");
    let (rust, _) = sibling("examples/cpp_host/src/raw.rs");

    assert_agree(
        "xpui_host.h and examples/cpp_host/src/raw.rs",
        &signatures_from_c(&header, &header_path, &PLAIN),
        &signatures_from_rust(&rust, &PLAIN),
    );
}

#[test]
fn what_the_application_exports_agrees_with_its_header() {
    let (header, header_path) = sibling("examples/cpp_host/cpp/xpui_app.h");
    let (rust, _) = sibling("examples/cpp_host/src/lib.rs");
    let (lifecycle, _) = own("src/lifecycle.rs");

    // The factories are generated, so no Rust source spells them out. Their
    // signature comes from the macro's own body, with `$factory` standing in
    // for each name it was invoked with.
    let mut declared: Signatures = signatures_from_rust(&rust, &PLAIN);
    declared.extend(signatures_from_register_screen(&lifecycle, &rust, &PLAIN));

    assert_agree(
        "xpui_app.h and examples/cpp_host/src/lib.rs",
        &signatures_from_c(&header, &header_path, &PLAIN),
        &declared,
    );
}

/// Every file the five pairs above name, and whether it is there.
///
/// `assert_agree` refuses an empty parse, so a boundary that read nothing
/// fails rather than passing — but it fails inside one test with a message
/// about parsing. This says plainly which file is missing, which is what a
/// moved crate or a split repository actually produces.
#[test]
fn every_boundary_names_a_file_that_is_there() {
    let mine = [
        "cpp/xpui_fui.h",
        "cpp/xpui_screen.h",
        "src/raw.rs",
        "src/testing/stubs.rs",
        "src/lifecycle.rs",
    ];
    let theirs = [
        "examples/cpp_host/cpp/xpui_host.h",
        "examples/cpp_host/cpp/xpui_app.h",
        "examples/cpp_host/src/raw.rs",
        "examples/cpp_host/src/lib.rs",
    ];

    for relative in mine {
        let (text, path) = own(relative);
        assert!(!text.trim().is_empty(), "{} is empty", path.display());
    }
    for relative in theirs {
        let (text, path) = sibling(relative);
        assert!(!text.trim().is_empty(), "{} is empty", path.display());
    }
}
