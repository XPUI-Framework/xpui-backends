//! Reading signatures out of a C header.

use std::path::Path;

use crate::signature::{Boundary, Signature, Signatures, rust_for_c};
use crate::without_comments;

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
pub fn signatures_from_c(source: &str, file: &Path, boundary: &Boundary<'_>) -> Signatures {
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
        if !name.starts_with(boundary.prefix) || boundary.skip.contains(&name) {
            continue;
        }

        let returns = c_type_of(&head[..=name_start]);
        let raw_params = &statement[open + 1..close];

        // A parameter list with parentheses in it is a function pointer, which
        // this mapping does not cover. Loud rather than skipped.
        assert!(
            !raw_params.contains('('),
            "{}: {name} takes a function pointer, which this scanner cannot compare. Give \
             the boundary a `types` entry for it, or list it in `skip`.",
            file.display()
        );

        let params = raw_params
            .split(',')
            .map(c_type_of)
            .filter(|param| !param.is_empty() && param != "void")
            .map(|param| {
                rust_for_c(&param, boundary.types)
                    .unwrap_or_else(|| {
                        panic!(
                            "{}: {name} takes `{param}`, which no type mapping covers. Add \
                             it to `BASE_TYPES` or the boundary's `types` rather than \
                             leaving it unchecked.",
                            file.display()
                        )
                    })
                    .to_string()
            })
            .collect();

        let returns = rust_for_c(&returns, boundary.types)
            .unwrap_or_else(|| {
                panic!(
                    "{}: {name} returns `{returns}`, which no type mapping covers.",
                    file.display()
                )
            })
            .to_string();

        found.insert(name.to_string(), Signature { returns, params });
    }

    found
}
