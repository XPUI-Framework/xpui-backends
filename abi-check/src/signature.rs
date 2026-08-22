//! What a signature is, and how a C spelling maps to a Rust one.

use std::collections::BTreeMap;

/// One function, in a spelling both languages can be reduced to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature {
    pub returns: String,
    pub params: Vec<String>,
}

impl std::fmt::Display for Signature {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "({}) -> {}", self.params.join(", "), self.returns)
    }
}

pub type Signatures = BTreeMap<String, Signature>;

/// What one caller's boundary looks like.
///
/// Every field is the caller's to state, because a checker that guessed any of
/// them would be describing one boundary while claiming to check all of them.
pub struct Boundary<'a> {
    /// Only symbols starting with this are compared. A header is free to
    /// declare things that are not part of the ABI under test.
    pub prefix: &'a str,
    /// Names the C side declares and the Rust side never mentions — a symbol
    /// the host defines, or one that takes a shape the scanner cannot read.
    pub skip: &'a [&'a str],
    /// C spellings this boundary uses that [`BASE_TYPES`] does not cover, such
    /// as a typedef of its own. Consulted after `BASE_TYPES`, so a base
    /// spelling cannot be shadowed — an entry that tried would be ignored
    /// rather than quietly changing what every other boundary means.
    pub types: &'a [(&'a str, &'a str)],
    /// Rust spellings this boundary writes for a type it already names in
    /// `types`, longest first. A path is not part of the ABI, so
    /// `crate::raw::CellFn` and `CellFn` have to reduce to one thing before
    /// two sides can be compared.
    pub rust_aliases: &'a [(&'a str, &'a str)],
}

/// C spellings, and the Rust each one has to be.
///
/// Deliberately small, explicit and closed. Anything not here and not in a
/// [`Boundary`]'s own `types` fails the run rather than being skipped — see
/// the module docs.
pub const BASE_TYPES: &[(&str, &str)] = &[
    ("void", "()"),
    ("int32_t", "i32"),
    ("uint32_t", "u32"),
    ("uint16_t", "u16"),
    ("uint8_t", "u8"),
    ("const uint8_t*", "*const u8"),
    ("uint8_t*", "*mut u8"),
    ("int32_t*", "*mut i32"),
    ("void*", "*mut c_void"),
];

pub(crate) fn rust_for_c<'a>(c_type: &str, extra: &'a [(&'a str, &'a str)]) -> Option<&'a str> {
    BASE_TYPES
        .iter()
        .chain(extra.iter())
        .find(|(c, _)| *c == c_type)
        .map(|(_, rust)| *rust)
}
