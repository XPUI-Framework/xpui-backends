//! The checker's own tests.
//!
//! A checker nobody checked is a checker that reports agreement it never
//! established. These break each side on purpose and confirm it is noticed.

use std::path::Path;

use xpui_abi_check::{
    Boundary, Signatures, signatures_from_c, signatures_from_register_screen, signatures_from_rust,
};

/// What these fixtures pretend a boundary looks like. `xpui_fui_cell_fn`
/// is here rather than in [`BASE_TYPES`] for the same reason a real
/// caller's typedef would be: it belongs to one boundary, not to every
/// boundary this crate might ever check.
const FIXTURE: Boundary<'_> = Boundary {
    prefix: "xpui_",
    skip: &[],
    types: &[("xpui_fui_cell_fn", "CellFn")],
    rust_aliases: &[("raw::CellFn", "CellFn")],
};

const HEADER: &str = r#"
    // A comment naming xpui_fui_ghost, which is not a declaration.
    void xpui_fui_draw(int32_t x, int32_t y, const uint8_t* text, uint8_t style);
    int32_t xpui_fui_metric(uint8_t metric);
    void xpui_fui_clear(void);
    /* a block comment naming xpui_fui_phantom */
    typedef const uint8_t* (*xpui_fui_cell_fn)(void* ctx, int32_t index);
"#;

fn header() -> Signatures {
    signatures_from_c(HEADER, Path::new("<test>"), &FIXTURE)
}

fn rust(source: &str) -> Signatures {
    signatures_from_rust(source, &FIXTURE)
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
#[should_panic(expected = "no type mapping covers")]
fn an_unknown_c_type_fails_rather_than_passing_silently() {
    signatures_from_c(
        "float xpui_fui_gamma(double correction);",
        Path::new("<test>"),
        &FIXTURE,
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
        &FIXTURE,
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
        &FIXTURE,
    );
    assert_eq!(found["xpui_app_make"].params, ["u32"]);
}

/// A parser that stops understanding the real headers must not report
/// agreement. Two empty sets agree perfectly, which is the one way this whole
/// crate could go green having checked nothing.
#[test]
#[should_panic(expected = "nothing was parsed from the C side")]
fn an_empty_c_side_is_refused() {
    xpui_abi_check::assert_agree("fixture", &Signatures::new(), &rust("pub fn xpui_a();"));
}

#[test]
#[should_panic(expected = "nothing was parsed from the Rust side")]
fn an_empty_rust_side_is_refused() {
    let c = signatures_from_c("void xpui_a(void);", Path::new("<test>"), &FIXTURE);
    xpui_abi_check::assert_agree("fixture", &c, &Signatures::new());
}

/// A boundary's prefix decides what is compared, on **both** sides.
///
/// A Rust scanner that ignored it would narrow the C side and not the Rust
/// side, and report every unmatched Rust symbol as "defined in Rust, absent
/// from C" — a red test naming a fault that does not exist.
#[test]
fn the_prefix_narrows_both_sides_alike() {
    const HOST_ONLY: Boundary<'_> = Boundary {
        prefix: "xpui_host_",
        skip: &[],
        types: &[],
        rust_aliases: &[],
    };

    let source = "void xpui_host_battery(void);\nvoid xpui_app_start(void);";
    let rust = "pub fn xpui_host_battery();\npub fn xpui_app_start();";

    let c = signatures_from_c(source, Path::new("<test>"), &HOST_ONLY);
    let r = signatures_from_rust(rust, &HOST_ONLY);

    assert_eq!(c.keys().collect::<Vec<_>>(), ["xpui_host_battery"]);
    assert_eq!(
        r.keys().collect::<Vec<_>>(),
        ["xpui_host_battery"],
        "the Rust side ignored the prefix and kept xpui_app_start"
    );
}

/// A path is not part of the ABI, so a boundary says which spellings it writes.
#[test]
fn rust_aliases_reduce_a_path_to_the_type_it_names() {
    const WITH: Boundary<'_> = Boundary {
        prefix: "xpui_",
        skip: &[],
        types: &[("xpui_fui_cell_fn", "CellFn")],
        rust_aliases: &[("crate::raw::CellFn", "CellFn")],
    };
    const WITHOUT: Boundary<'_> = Boundary {
        prefix: "xpui_",
        skip: &[],
        types: &[("xpui_fui_cell_fn", "CellFn")],
        rust_aliases: &[],
    };

    let rust = "pub fn xpui_fui_rows(cells: crate::raw::CellFn);";
    assert_eq!(
        signatures_from_rust(rust, &WITH)["xpui_fui_rows"].params,
        ["CellFn"]
    );
    assert_eq!(
        signatures_from_rust(rust, &WITHOUT)["xpui_fui_rows"].params,
        ["crate::raw::CellFn"],
        "without an alias the path stands, which is what makes the alias load-bearing"
    );
}

/// A boundary's own typedef is read, and without it the run fails.
///
/// `BASE_TYPES` is closed, so a spelling it does not carry has nowhere else to
/// come from. Set `types` to `&[]` and this panics instead of comparing —
/// which is the whole point of the field.
#[test]
fn a_boundarys_own_typedef_is_consulted() {
    let header = "void xpui_fui_rows(xpui_fui_cell_fn cells);";
    let parsed = signatures_from_c(header, Path::new("<test>"), &FIXTURE);
    assert_eq!(parsed["xpui_fui_rows"].params, ["CellFn"]);
}

#[test]
#[should_panic(expected = "no type mapping covers")]
fn without_its_typedef_the_same_header_fails() {
    const BARE: Boundary<'_> = Boundary {
        prefix: "xpui_",
        skip: &[],
        types: &[],
        rust_aliases: &[],
    };
    signatures_from_c(
        "void xpui_fui_rows(xpui_fui_cell_fn cells);",
        Path::new("<test>"),
        &BARE,
    );
}

/// A skipped symbol is one the Rust side is not expected to declare.
///
/// Without the skip it reads as "declared in C, absent from Rust" — a red test
/// naming a fault that is really the boundary's shape.
#[test]
fn a_skipped_symbol_is_left_out_of_the_comparison() {
    const SKIPS: Boundary<'_> = Boundary {
        prefix: "xpui_",
        skip: &["xpui_fui_present"],
        types: &[],
        rust_aliases: &[],
    };

    let header = "void xpui_fui_clear(void);\nvoid xpui_fui_present(void);";
    let parsed = signatures_from_c(header, Path::new("<test>"), &SKIPS);

    assert_eq!(
        parsed.keys().collect::<Vec<_>>(),
        ["xpui_fui_clear"],
        "the skip was ignored, so a host-defined symbol entered the comparison"
    );
}
