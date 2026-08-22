//! Whether a C header and the Rust beside it agree about **types**.
//!
//! Names are the easy half, and a shell `grep` can do them. A parameter
//! reordered on one side and not the other still links — C has no mangling to
//! disagree with — and the result is a corrupt call frame: the callee reads a
//! width where a height was passed, and the failure surfaces as a rendering
//! fault somewhere unrelated.
//!
//! So this parses both sides and compares signatures. It is a checker for
//! anyone who owns a C ABI boundary, not for one backend: each caller names
//! its own files, its own skips and its own extra type spellings.
//!
//! **An unrecognised C type is a failure, not a skip.** A checker that quietly
//! ignores what it does not understand is worse than no checker: it reports
//! agreement it never established. [`assert_agree`] refuses an empty parse for
//! the same reason — two empty sets agree perfectly.
//!
//! # What these parsers do not read
//!
//! Both are hand-rolled scanners, not compilers, and the boundary they cover
//! is expected to be written in a deliberately plain subset. Every shape below
//! was tried; all of them either panic or produce a mismatch, so **none of
//! them can turn into a silent pass** — but they would be confusing to hit:
//!
//! - C: an `__attribute__((...))` prefix makes a declaration unparseable, and
//!   the mismatch then reads "defined in Rust, absent from C", which is true
//!   only in the sense that the parser could not see it.
//! - Rust: an inline function-pointer parameter, a tuple parameter, a `where`
//!   clause, and a raw identifier are all mangled or missed.
//! - Rust: string literals are not stripped, so a `&str` constant containing
//!   something shaped like `fn xpui_…(…)` is read as a declaration. It fails
//!   loudly as an extra symbol rather than passing.
//!
//! # What it cannot catch, by construction
//!
//! **Two parameters that normalise to the same spelling, swapped.**
//! `(int32_t x, int32_t y)` and `(int32_t y, int32_t x)` are the same
//! signature, and a name is not part of a C ABI. The call frame is corrupt and
//! nothing here can see it — no type-based check can. Reordering parameters
//! that normalise *differently* is caught, which is the case a header edit
//! usually produces.
//!
//! Note "normalise", not "are the same C type": a [`Boundary`] whose `types`
//! map a second C spelling onto a Rust one already in [`BASE_TYPES`] widens
//! this blind spot, with no signal that it did.
//!
//! Add to the mapping or the scanner when a boundary needs a shape it does not
//! have; do not work around one.

mod c;
mod rust;
mod signature;

pub use c::signatures_from_c;
pub use rust::{signatures_from_register_screen, signatures_from_rust};
pub use signature::{BASE_TYPES, Boundary, Signature, Signatures};

/// Both kinds of comment, gone. Prose in these files names sibling symbols
/// constantly, and a declaration is not a mention.
pub(crate) fn without_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == '/' && index + 1 < bytes.len() {
            if bytes[index + 1] == '/' {
                while index < bytes.len() && bytes[index] != '\n' {
                    index += 1;
                }
                continue;
            }
            if bytes[index + 1] == '*' {
                index += 2;
                while index + 1 < bytes.len() && !(bytes[index] == '*' && bytes[index + 1] == '/') {
                    index += 1;
                }
                index = (index + 2).min(bytes.len());
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    out
}

/// Compares one pair, and says exactly what differs.
pub fn assert_agree(what: &str, c_side: &Signatures, rust_side: &Signatures) {
    // Two empty maps agree perfectly, and that is the one way this whole file
    // can report agreement it never established — a parser that stops
    // understanding the real headers would leave all five comparisons green.
    // A caller's own fixtures cannot catch that: they parse inline text, not
    // the real headers.
    assert!(
        !c_side.is_empty(),
        "{what}: nothing was parsed from the C side. A comparison over an empty \
         set passes without checking anything."
    );
    assert!(
        !rust_side.is_empty(),
        "{what}: nothing was parsed from the Rust side. A comparison over an \
         empty set passes without checking anything."
    );

    let mut problems = Vec::new();

    for (name, expected) in c_side {
        match rust_side.get(name) {
            None => problems.push(format!("  {name}: declared in C, absent from Rust")),
            Some(actual) if actual != expected => problems.push(format!(
                "  {name}\n      C:    {expected}\n      Rust: {actual}"
            )),
            Some(_) => {}
        }
    }

    for name in rust_side.keys() {
        if !c_side.contains_key(name) {
            problems.push(format!("  {name}: defined in Rust, absent from C"));
        }
    }

    assert!(
        problems.is_empty(),
        "{what}: the two sides disagree.\n{}\n\nA name that matches with a type that does not \
         still links, because C has no mangling to disagree with. The call frame is then corrupt.",
        problems.join("\n")
    );
}
