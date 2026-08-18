//! Every C symbol, and whether both sides agree about its **types**.
//!
//! `ffi_symbols_agree()` in `build-and-test.sh` checks that the names line up.
//! Names are the easy half. A parameter reordered on one side and not the
//! other still links — C has no mangling to disagree with — and the result is
//! a corrupt call frame: the shim reads a width where a height was passed, and
//! the failure surfaces as a rendering fault somewhere unrelated.
//!
//! So this parses both sides and compares signatures. Five boundaries, in two
//! languages, across two crates:
//!
//! | C | Rust | who defines |
//! |---|---|---|
//! | `cpp/xpui_fui.h` | `src/raw.rs` | C++ |
//! | `cpp/xpui_fui.h` | `src/testing/stubs.rs` | Rust, for tests |
//! | `cpp/xpui_screen.h` | `src/lifecycle.rs` | Rust |
//! | `examples/cpp_host/cpp/xpui_host.h` | `examples/cpp_host/src/raw.rs` | C++ |
//! | `examples/cpp_host/cpp/xpui_app.h` | `examples/cpp_host/src/lib.rs` | Rust |
//!
//! It lives in this crate because this is the crate that owns the C boundary
//! and the only one whose test harness links — `examples/cpp_host` sets
//! `test = false`, since every `xpui_host_*` symbol it calls is defined by the
//! C++ half.
//!
//! **An unrecognised C type is a failure, not a skip.** A checker that quietly
//! ignores what it does not understand is worse than no checker: it reports
//! agreement it never established. `assert_agree` refuses an empty parse for
//! the same reason — two empty sets agree perfectly.
//!
//! # What these parsers do not read
//!
//! Both are hand-rolled scanners, not compilers, and the boundary they cover
//! is written in a deliberately plain subset. Every shape below was tried; all
//! of them either panic or produce a mismatch, so **none of them can turn into
//! a silent pass** — but they would be confusing to hit:
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
//! Add to the mapping or the scanner when the boundary needs a shape it does
//! not have; do not work around one.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// -- what a signature is ----------------------------------------------------

/// One function, in a spelling both languages can be reduced to.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Signature {
    returns: String,
    params: Vec<String>,
}

impl std::fmt::Display for Signature {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "({}) -> {}", self.params.join(", "), self.returns)
    }
}

type Signatures = BTreeMap<String, Signature>;

/// The two symbols the Rust side never calls.
///
/// A host defines `xpui_fui_present` and installs `xpui_fui_set_present`, so
/// both belong to the header and the shim alone. `set_present` also takes a
/// function pointer, which is why the exclusion has to reach the parser.
const HOST_FACING: &[&str] = &["xpui_fui_present", "xpui_fui_set_present"];

/// C spellings, and the Rust each one has to be.
///
/// Deliberately small, explicit and closed. Anything not here fails the run
/// rather than being skipped — see the module docs.
const C_TO_RUST: &[(&str, &str)] = &[
    ("void", "()"),
    ("int32_t", "i32"),
    ("uint32_t", "u32"),
    ("uint16_t", "u16"),
    ("uint8_t", "u8"),
    ("const uint8_t*", "*const u8"),
    ("uint8_t*", "*mut u8"),
    ("int32_t*", "*mut i32"),
    ("void*", "*mut c_void"),
    // The row callback's typedef. Rust spells the same type `raw::CellFn`.
    ("xpui_fui_cell_fn", "CellFn"),
];

fn rust_for_c(c_type: &str) -> Option<&'static str> {
    C_TO_RUST
        .iter()
        .find(|(c, _)| *c == c_type)
        .map(|(_, rust)| *rust)
}

// -- reading C --------------------------------------------------------------

/// Both kinds of comment, gone. Prose in these files names sibling symbols
/// constantly, and a declaration is not a mention.
fn without_comments(text: &str) -> String {
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

/// Normalises `const uint8_t *text` and friends to `const uint8_t*`.
fn canonical_c_type(text: &str) -> String {
    let squashed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    squashed.replace(" *", "*").replace("* ", "*")
}

/// Splits a C declaration's `type name` into the type alone.
///
/// The name is the trailing identifier, so everything before it is the type —
/// which is what makes `const uint8_t* title` and `int32_t x` the same shape.
fn c_type_of(declaration: &str) -> String {
    let canonical = canonical_c_type(declaration);
    if canonical == "void" || canonical.is_empty() {
        return canonical;
    }
    match canonical.rsplit_once([' ', '*']) {
        // Keep the separator with the type: `uint8_t* text` is a pointer.
        Some((head, tail)) if tail.chars().all(|c| c.is_alphanumeric() || c == '_') => {
            let cut = head.len()
                + if canonical.as_bytes()[head.len()] == b'*' {
                    1
                } else {
                    0
                };
            canonical_c_type(&canonical[..cut])
        }
        _ => canonical,
    }
}

/// A header, reduced to the statements a declaration can be one of.
///
/// Preprocessor lines are dropped, and braces read as terminators so that
/// `extern "C" {` ends rather than running on into the first declaration —
/// which would otherwise arrive as part of its return type.
fn c_statements(source: &str) -> Vec<String> {
    let text = without_comments(source);
    let code: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .replace(['{', '}'], ";");

    code.split(';')
        .map(|part| part.trim().to_string())
        .collect()
}

/// Every `xpui_*` function a C header declares, less the ones named in
/// `skip`.
///
/// Skipping happens **here**, before the types are read, because a symbol
/// excluded from a comparison must not be able to fail that comparison's
/// parsing either — `xpui_fui_set_present` takes a function pointer, which
/// this deliberately narrow mapping does not cover.
fn signatures_from_c(source: &str, file: &Path, skip: &[&str]) -> Signatures {
    let mut found = Signatures::new();

    for statement in c_statements(source) {
        let statement = statement.as_str();
        // A typedef declares a type, not a symbol; nothing links it.
        if statement.is_empty() || statement.starts_with("typedef") {
            continue;
        }
        let Some(open) = statement.find('(') else {
            continue;
        };
        let Some(close) = statement.rfind(')') else {
            continue;
        };

        let head = &statement[..open];
        let Some(name_start) = head.rfind(|c: char| !(c.is_alphanumeric() || c == '_')) else {
            continue;
        };
        let name = head[name_start + 1..].trim();
        if !name.starts_with("xpui_") || skip.contains(&name) {
            continue;
        }

        let returns = c_type_of(&head[..=name_start]);
        let raw_params = &statement[open + 1..close];

        // A parameter list with parentheses in it is a function pointer, which
        // this mapping does not cover. Loud rather than skipped.
        assert!(
            !raw_params.contains('('),
            "{}: {name} takes a function pointer, which tests/abi.rs cannot compare. \
             Either extend C_TO_RUST or list it as deliberately unchecked.",
            file.display()
        );

        let params = raw_params
            .split(',')
            .map(c_type_of)
            .filter(|param| !param.is_empty() && param != "void")
            .map(|param| {
                rust_for_c(&param)
                    .unwrap_or_else(|| {
                        panic!(
                            "{}: {name} takes `{param}`, which tests/abi.rs does not know. \
                             Add it to C_TO_RUST rather than leaving it unchecked.",
                            file.display()
                        )
                    })
                    .to_string()
            })
            .collect();

        let returns = rust_for_c(&returns)
            .unwrap_or_else(|| {
                panic!(
                    "{}: {name} returns `{returns}`, which tests/abi.rs does not know.",
                    file.display()
                )
            })
            .to_string();

        found.insert(name.to_string(), Signature { returns, params });
    }

    found
}

// -- reading Rust -----------------------------------------------------------

/// `*mut core::ffi::c_void` and `*mut c_void` are the same pointer.
fn canonical_rust_type(text: &str) -> String {
    let squashed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    squashed
        .replace("core::ffi::c_void", "c_void")
        .replace("crate::raw::CellFn", "CellFn")
        .replace("raw::CellFn", "CellFn")
}

/// Every `xpui_*` function a Rust file declares or defines.
///
/// Covers `pub safe fn` in an `unsafe extern` block, `pub unsafe extern "C"
/// fn` with a body, and the bare `extern "C" fn` the host doubles use.
fn signatures_from_rust(source: &str) -> Signatures {
    let text = without_comments(source);
    let chars: Vec<char> = text.chars().collect();
    let mut found = Signatures::new();
    let mut index = 0;

    while let Some(offset) = text[index..].find("fn ") {
        let mut cursor = index + offset + 3;
        index = cursor;

        while cursor < chars.len() && chars[cursor].is_whitespace() {
            cursor += 1;
        }
        let name_start = cursor;
        while cursor < chars.len() && (chars[cursor].is_alphanumeric() || chars[cursor] == '_') {
            cursor += 1;
        }
        let name: String = chars[name_start..cursor].iter().collect();
        if !name.starts_with("xpui_") {
            continue;
        }

        while cursor < chars.len() && chars[cursor] != '(' {
            cursor += 1;
        }
        let params_start = cursor + 1;
        let mut depth = 0;
        while cursor < chars.len() {
            match chars[cursor] {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            cursor += 1;
        }
        let raw_params: String = chars[params_start..cursor].iter().collect();
        cursor += 1;

        // The return type runs to the body or the semicolon, whichever comes
        // first — a declaration in an `extern` block has no body.
        let tail: String = chars[cursor..].iter().collect();
        let end = tail
            .find(['{', ';'])
            .unwrap_or(tail.len().min(tail.find('\n').unwrap_or(tail.len())));
        let returns = match tail[..end].trim().strip_prefix("->") {
            Some(rust) => canonical_rust_type(rust),
            None => "()".to_string(),
        };

        let params = raw_params
            .split(',')
            .filter_map(|param| param.split_once(':'))
            .map(|(_name, rust)| canonical_rust_type(rust))
            .collect();

        found.insert(name, Signature { returns, params });
    }

    found
}

/// The factories `register_screen!` was asked to export.
fn factories_in(source: &str) -> Vec<String> {
    let text = without_comments(source);
    let mut names = Vec::new();
    let mut rest = text.as_str();

    while let Some(at) = rest.find("register_screen!(") {
        rest = &rest[at + "register_screen!(".len()..];
        let Some(close) = rest.find(')') else { break };
        if let Some((_screen, factory)) = rest[..close].split_once(',') {
            names.push(factory.trim().to_string());
        }
    }

    names
}

/// What `register_screen!` exports, **read from the macro itself**.
///
/// No Rust source spells these out — the factory is generated — so the
/// signature has to come from the macro's own body, with `$factory` standing
/// in for each name it was invoked with. Written down here instead, it would
/// be a second copy of the contract, and a copy this file could not see drift
/// in: an ignored parameter added to the macro is a real ABI change that a
/// hard-coded `() -> *mut c_void` would keep calling correct.
fn signatures_from_register_screen(macro_source: &str, invocations: &str) -> Signatures {
    let start = macro_source
        .find("macro_rules! register_screen")
        .expect("lifecycle.rs defines register_screen!");
    let body = &macro_source[start..];

    let mut found = Signatures::new();
    for factory in factories_in(invocations) {
        let expanded = body.replace("$factory", &factory);
        if let Some(signature) = signatures_from_rust(&expanded).remove(&factory) {
            found.insert(factory, signature);
        }
    }

    found
}

// -- the boundaries ---------------------------------------------------------

fn workspace_root() -> PathBuf {
    // `crates/backend/fui` -> the root. Resolved from the manifest rather than
    // the working directory, which `cargo test` does not promise.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("this crate sits three levels below the workspace root")
        .to_path_buf()
}

fn read(relative: &str) -> (String, PathBuf) {
    let path = workspace_root().join(relative);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    (text, path)
}

/// Compares one pair, and says exactly what differs.
fn assert_agree(what: &str, c_side: &Signatures, rust_side: &Signatures) {
    // Two empty maps agree perfectly, and that is the one way this whole file
    // can report agreement it never established — a parser that stops
    // understanding the real headers would leave all five comparisons green.
    // The fixtures in `mod parsing` cannot catch that: they parse their own
    // inline text, not these files.
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

#[test]
fn the_drawing_abi_agrees_with_its_rust_declarations() {
    let (header, header_path) = read("crates/backend/fui/cpp/xpui_fui.h");
    let (rust, _) = read("crates/backend/fui/src/raw.rs");

    assert_agree(
        "xpui_fui.h and src/raw.rs",
        &signatures_from_c(&header, &header_path, HOST_FACING),
        &signatures_from_rust(&rust),
    );
}

#[test]
fn the_drawing_abi_agrees_with_its_host_doubles() {
    let (header, header_path) = read("crates/backend/fui/cpp/xpui_fui.h");
    let (rust, _) = read("crates/backend/fui/src/testing/stubs.rs");

    // The doubles are the file that rots quietly, because nothing calls them
    // until a test finally does.
    assert_agree(
        "xpui_fui.h and src/testing/stubs.rs",
        &signatures_from_c(&header, &header_path, HOST_FACING),
        &signatures_from_rust(&rust),
    );
}

#[test]
fn the_lifecycle_agrees_with_the_rust_that_defines_it() {
    let (header, header_path) = read("crates/backend/fui/cpp/xpui_screen.h");
    let (rust, _) = read("crates/backend/fui/src/lifecycle.rs");

    assert_agree(
        "xpui_screen.h and src/lifecycle.rs",
        &signatures_from_c(&header, &header_path, &[]),
        &signatures_from_rust(&rust),
    );
}

#[test]
fn what_the_host_answers_agrees_with_what_rust_asks_for() {
    let (header, header_path) = read("examples/cpp_host/cpp/xpui_host.h");
    let (rust, _) = read("examples/cpp_host/src/raw.rs");

    assert_agree(
        "xpui_host.h and examples/cpp_host/src/raw.rs",
        &signatures_from_c(&header, &header_path, &[]),
        &signatures_from_rust(&rust),
    );
}

#[test]
fn what_the_application_exports_agrees_with_its_header() {
    let (header, header_path) = read("examples/cpp_host/cpp/xpui_app.h");
    let (rust, _) = read("examples/cpp_host/src/lib.rs");
    let (lifecycle, _) = read("crates/backend/fui/src/lifecycle.rs");

    let mut declared = signatures_from_rust(&rust);
    declared.extend(signatures_from_register_screen(&lifecycle, &rust));

    assert_agree(
        "xpui_app.h and examples/cpp_host/src/lib.rs",
        &signatures_from_c(&header, &header_path, &[]),
        &declared,
    );
}

// -- the parser's own tests -------------------------------------------------
//
// A checker nobody checked is a checker that reports agreement it never
// established. These break each side on purpose and confirm it is noticed.

#[cfg(test)]
mod parsing {
    use super::*;

    const HEADER: &str = r#"
        // A comment naming xpui_fui_ghost, which is not a declaration.
        void xpui_fui_draw(int32_t x, int32_t y, const uint8_t* text, uint8_t style);
        int32_t xpui_fui_metric(uint8_t metric);
        void xpui_fui_clear(void);
        /* a block comment naming xpui_fui_phantom */
        typedef const uint8_t* (*xpui_fui_cell_fn)(void* ctx, int32_t index);
    "#;

    fn header() -> Signatures {
        signatures_from_c(HEADER, Path::new("<test>"), &[])
    }

    fn rust(source: &str) -> Signatures {
        signatures_from_rust(source)
    }

    #[test]
    fn a_typedef_is_not_a_symbol() {
        assert!(!header().contains_key("xpui_fui_cell_fn"));
    }

    #[test]
    fn a_symbol_named_only_in_a_comment_is_not_a_declaration() {
        let found = header();
        assert!(!found.contains_key("xpui_fui_ghost"));
        assert!(!found.contains_key("xpui_fui_phantom"));
        assert_eq!(found.len(), 3, "{found:?}");
    }

    #[test]
    fn c_pointers_and_returns_map_across() {
        let draw = &header()["xpui_fui_draw"];
        assert_eq!(draw.params, ["i32", "i32", "*const u8", "u8"]);
        assert_eq!(draw.returns, "()");
        assert_eq!(header()["xpui_fui_metric"].returns, "i32");
        assert!(header()["xpui_fui_clear"].params.is_empty());
    }

    #[test]
    fn rust_declarations_and_definitions_read_the_same() {
        let declared = rust("pub safe fn xpui_fui_metric(metric: u8) -> i32;");
        let defined =
            rust("#[unsafe(no_mangle)] extern \"C\" fn xpui_fui_metric(metric: u8) -> i32 { 0 }");
        assert_eq!(declared["xpui_fui_metric"], defined["xpui_fui_metric"]);
    }

    #[test]
    fn a_multi_line_parameter_list_is_one_signature() {
        let found = rust(
            "pub fn xpui_fui_draw(\n    x: i32,\n    y: i32,\n    text: *const u8,\n    style: u8,\n);",
        );
        assert_eq!(
            found["xpui_fui_draw"].params,
            ["i32", "i32", "*const u8", "u8"]
        );
    }

    #[test]
    fn a_reordered_parameter_is_caught() {
        // The failure this whole file exists for: same name, same arity, same
        // types — in the wrong order. It links, and the shim reads a style
        // where a pointer was passed.
        let swapped = rust("pub fn xpui_fui_draw(x: i32, y: i32, style: u8, text: *const u8);");
        assert_ne!(header()["xpui_fui_draw"], swapped["xpui_fui_draw"]);
    }

    #[test]
    fn a_narrowed_parameter_is_caught() {
        let narrowed = rust("pub safe fn xpui_fui_metric(metric: u8) -> u16;");
        assert_ne!(header()["xpui_fui_metric"], narrowed["xpui_fui_metric"]);
    }

    #[test]
    #[should_panic(expected = "does not know")]
    fn an_unknown_c_type_fails_rather_than_passing_silently() {
        signatures_from_c(
            "float xpui_fui_gamma(double correction);",
            Path::new("<test>"),
            &[],
        );
    }

    #[test]
    fn c_void_spellings_are_the_same_type() {
        let long = rust("pub fn xpui_screen_loop(handle: *mut core::ffi::c_void);");
        let short = rust("pub fn xpui_screen_loop(handle: *mut c_void);");
        assert_eq!(long["xpui_screen_loop"], short["xpui_screen_loop"]);
    }

    /// The mirror of `c_pointers_and_returns_map_across`, for the other
    /// parser. Without it, the Rust side has nothing asserting a concrete
    /// value: every remaining test compares one parser output against another
    /// or asserts they differ, and both of those still hold when parameters or
    /// return types come back empty.
    #[test]
    fn a_rust_signature_pins_both_its_parameters_and_its_return() {
        let declared = rust("pub safe fn xpui_fui_metric(metric: u8) -> i32;");
        assert_eq!(declared["xpui_fui_metric"].params, ["u8"]);
        assert_eq!(declared["xpui_fui_metric"].returns, "i32");

        let defined = rust(
            "#[unsafe(no_mangle)] extern \"C\" fn xpui_fui_draw(text: *const u8, x: i32) { let _ = x; }",
        );
        assert_eq!(defined["xpui_fui_draw"].params, ["*const u8", "i32"]);
        assert_eq!(defined["xpui_fui_draw"].returns, "()");
    }

    const MACRO: &str = r#"
        macro_rules! register_screen {
            ($screen:ty, $factory:ident) => {
                #[unsafe(no_mangle)]
                pub extern "C" fn $factory() -> *mut core::ffi::c_void {
                    $crate::lifecycle::into_handle(<$screen>::new())
                }
            };
        }
    "#;

    #[test]
    fn the_screen_factory_signature_is_read_from_the_macro() {
        let found = signatures_from_register_screen(
            MACRO,
            "register_screen!(screens::Menu, xpui_app_make);",
        );
        assert_eq!(found["xpui_app_make"].returns, "*mut c_void");
        assert!(found["xpui_app_make"].params.is_empty());
    }

    /// The reason the signature is parsed rather than written down.
    ///
    /// An ignored parameter added to the macro is a real ABI change against
    /// `void* xpui_app_create_menu(void)`, and it compiles. Held as a constant
    /// here, this file would keep calling it correct.
    #[test]
    fn a_changed_macro_body_changes_what_the_factory_declares() {
        let changed = MACRO.replace("$factory()", "$factory(_unused: u32)");
        let found = signatures_from_register_screen(
            &changed,
            "register_screen!(screens::Menu, xpui_app_make);",
        );
        assert_eq!(found["xpui_app_make"].params, ["u32"]);
    }
}
