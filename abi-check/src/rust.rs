//! Reading signatures out of Rust source — declarations, definitions, and
//! what `register_screen!` generates.

use crate::signature::{Boundary, Signature, Signatures};
use crate::without_comments;

/// One spelling per type, so a path cannot make two sides look different.
///
/// `*mut core::ffi::c_void` and `*mut c_void` are the same pointer, and that
/// one is every C ABI's business. Anything else a boundary writes for a type
/// of its own goes in [`Boundary::rust_aliases`].
fn canonical_rust_type(text: &str, aliases: &[(&str, &str)]) -> String {
    let squashed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = squashed.replace("core::ffi::c_void", "c_void");
    for (written, means) in aliases {
        out = out.replace(written, means);
    }
    out
}

/// Every function a Rust file declares or defines whose name carries the
/// boundary's prefix.
///
/// Covers `pub safe fn` in an `unsafe extern` block, `pub unsafe extern "C"
/// fn` with a body, and the bare `extern "C" fn` a set of host doubles uses.
pub fn signatures_from_rust(source: &str, boundary: &Boundary<'_>) -> Signatures {
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
        if !name.starts_with(boundary.prefix) {
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
            Some(rust) => canonical_rust_type(rust, boundary.rust_aliases),
            None => "()".to_string(),
        };

        let params = raw_params
            .split(',')
            .filter_map(|param| param.split_once(':'))
            .map(|(_name, rust)| canonical_rust_type(rust, boundary.rust_aliases))
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
pub fn signatures_from_register_screen(
    macro_source: &str,
    invocations: &str,
    boundary: &Boundary<'_>,
) -> Signatures {
    let start = macro_source
        .find("macro_rules! register_screen")
        .expect("lifecycle.rs defines register_screen!");
    let body = &macro_source[start..];

    let mut found = Signatures::new();
    for factory in factories_in(invocations) {
        let expanded = body.replace("$factory", &factory);
        if let Some(signature) = signatures_from_rust(&expanded, boundary).remove(&factory) {
            found.insert(factory, signature);
        }
    }

    found
}
