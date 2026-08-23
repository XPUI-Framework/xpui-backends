//! Every C symbol, and whether both sides agree about its **types**.
//!
//! `symbols_agree` in the gate's `xtask/` checks that the names line up.
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
//!
//! Two more pairs cross into `xpui-cpp` — the host's own header against the
//! Rust that calls it, and what the application exports against the header
//! that declares it. They live in that repository, beside the files they read,
//! because a repository checks the boundary it owns.

use std::path::{Path, PathBuf};

use xpui_abi_check::{Boundary, assert_agree, signatures_from_c, signatures_from_rust};

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
/// One pair here is this shape: the lifecycle, which is entirely inside this
/// crate. The C++ host's two were the others; they moved to the repository
/// that owns them.
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
    for relative in mine {
        let (text, path) = own(relative);
        assert!(!text.trim().is_empty(), "{} is empty", path.display());
    }
}
