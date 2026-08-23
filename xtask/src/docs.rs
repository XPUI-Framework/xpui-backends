//! Where a document's links point.
//!
//! About **paths on disk**. Whether rustdoc can resolve an intra-doc link is a
//! different question with a different answer, and it is asked by running
//! rustdoc — see each `main.rs`. Whether a page's Rust is compiled is a third,
//! and it is next door in `prose.rs`.
//!
//! Its *commands* are next door in `commands.rs`: reading a fence is the same
//! job for both, but "does this path exist" and "would this command run" are
//! two, and they were one 500-line file until the size ratchet said so.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::fences::fences;
use crate::paths::{exists_exactly, tracked};

/// Every relative link in every document resolves to a path on disk.
///
/// Rust sources too, not only markdown: a `///` comment writes a
/// markdown-style link like any page, and at least one in this organisation
/// points at a C header two directories away. Reading only `*.md` left those
/// unchecked.
///
/// Absolute URLs are somebody else's uptime and are not fetched here. Anchors
/// are split off: `docs/x.md#section` is a claim about `docs/x.md`, and
/// checking the heading too would need a markdown parser for little gain.
pub fn doc_paths() -> Result<String, String> {
    let mut broken = Vec::new();
    let mut checked = 0;
    let mut pages = 0;
    for doc in tracked("*.md").into_iter().chain(tracked("*.rs")) {
        let raw = fs::read_to_string(&doc).unwrap_or_default();
        // In a Rust file only the doc comments are prose. A `[a](b.md)` inside
        // a string literal is data — this module's own tests are full of
        // them — and reading it as a link reports a file nobody claimed
        // exists.
        let text = if doc.extension().is_some_and(|x| x == "rs") {
            doc_comments(&raw)
        } else {
            raw
        };
        let here = doc.parent().unwrap_or(Path::new(".")).to_path_buf();
        pages += 1;
        for (number, target) in links(&text) {
            checked += 1;
            if exists_exactly(&here.join(&target)) {
                continue;
            }
            // Nothing on disk. If it could only ever have been an item, it is
            // rustdoc's to resolve and rustdoc does — see the check beside
            // this one.
            if !target.contains('/') && names_a_rust_item(&target) {
                continue;
            }
            broken.push(format!("  {}:{number}  {target}", doc.display()));
        }
    }
    if pages == 0 {
        return Err("no documents or sources found — the filter is matching nothing".into());
    }
    if broken.is_empty() {
        Ok(format!("{checked} relative link(s) across {pages} file(s)"))
    } else {
        Err(broken.join("\n"))
    }
}

/// A Rust file's `///` and `//!` lines, with every other line blanked so the
/// line numbers still point at the source.
fn doc_comments(text: &str) -> String {
    text.lines()
        .map(|line| {
            let trimmed = line.trim_start();
            trimmed
                .strip_prefix("///")
                .or_else(|| trimmed.strip_prefix("//!"))
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The relative file paths `[text](target)` names, with their line numbers.
///
/// Deliberately not a markdown parser, but not naive either. It drops what a
/// path check has no business reading, because each of these produced a false
/// failure the shell had already learned to avoid:
///
/// - links inside a fence, and inside an inline `code span`;
/// - URLs, anchors and `mailto:`;
/// - a CommonMark title — `[a](x.md "Title")` — and an angle-bracket
///   destination, `[a](<a b.md>)`;
/// - a bare word with no slash and no extension, like `[Screen](Screen)`,
///   which is rustdoc's to resolve rather than the filesystem's.
fn links(text: &str) -> Vec<(usize, String)> {
    let inside: BTreeSet<usize> = fences(text)
        .iter()
        .flat_map(|f| f.lines.iter().map(|(n, _)| *n))
        .collect();
    let mut out = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if inside.contains(&(index + 1)) {
            continue;
        }
        let line = blank_code_spans(line);
        let mut rest = line.as_str();
        let mut consumed = 0;
        while let Some(open) = rest.find("](") {
            let after = &rest[open + 2..];
            consumed += open + 2;
            let Some(close) = after.find(')') else { break };
            if let Some(target) = destination(&after[..close]) {
                out.push((index + 1, target));
            }
            rest = &after[close..];
            consumed += close;
            let _ = consumed;
        }
    }
    out
}

/// A link destination as a repository path, or nothing if it is not one.
fn destination(raw: &str) -> Option<String> {
    let raw = raw.trim();
    // `<a b.md>` — angle brackets exist so a destination may hold a space, so
    // the title split below must not run for one.
    let raw = match raw.strip_prefix('<').and_then(|r| r.strip_suffix('>')) {
        Some(bracketed) => bracketed,
        // `x.md "Title"` — everything from the space is the title.
        None => raw.split([' ', '\t']).next().unwrap_or(raw),
    };
    if raw.is_empty()
        || raw.starts_with("http")
        || raw.starts_with('#')
        || raw.starts_with("mailto:")
        || raw.contains("://")
    {
        return None;
    }
    let path = raw.split(['#', '?']).next().unwrap_or("");
    if path.is_empty() {
        return None;
    }
    // A bare word — `Screen`, `LICENSE`, `Makefile` — could be either an
    // intra-doc link or a file, and its spelling does not always say which.
    // It is returned, and the resolution below decides: a word that names a
    // file is a file, and only one that names nothing is handed back to
    // rustdoc. The shell used a list of extensions instead, and the cost was
    // that `[MIT](LICENSE)` — a real link, in every one of these
    // repositories — was silently never checked at all.
    Some(path.to_string())
}

/// Whether a bare word is the name of a Rust item rather than a file.
///
/// A path segment `::` says so outright, and a dot says the opposite. What is
/// left is a single word, where the useful observation is that a file named
/// without an extension is *shouted* — `LICENSE`, `README`, `CHANGELOG` —
/// while a Rust path never is: a module is lowercase (`metrics`, and
/// rustdoc's own `crate`, `self`, `super`) and a type is CamelCase
/// (`Screen`). So the one shape that is not an item is the one with no
/// lowercase letter in it.
fn names_a_rust_item(word: &str) -> bool {
    if word.contains("::") {
        return true;
    }
    if word.contains('.') {
        return false;
    }
    word.chars().any(char::is_lowercase)
}

/// A line with the contents of every inline `code span` replaced by spaces.
///
/// A span may hold anything, including `](`, and reading one as a link is how
/// a path check reports a fault in a sentence about paths.
fn blank_code_spans(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut inside = false;
    for c in line.chars() {
        if c == '`' {
            inside = !inside;
            out.push(c);
        } else if inside {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_inside_a_fence_is_not_a_link() {
        assert!(links("```rust\nlet x = [1](2);\n```\n").is_empty());
        assert_eq!(links("see [the guide](docs/guide.md)\n").len(), 1);
    }

    #[test]
    fn a_link_inside_a_code_span_is_not_a_link() {
        // A span may hold anything, including `](`, and reading one as a link
        // reports a fault in a sentence *about* links.
        assert!(links("write `[text](target)` like this\n").is_empty());
    }

    #[test]
    fn a_title_and_angle_brackets_are_not_part_of_the_path() {
        assert_eq!(links("[a](docs/x.md \"Title\")\n")[0].1, "docs/x.md");
        assert_eq!(links("[a](<docs/a b.md>)\n")[0].1, "docs/a b.md");
    }

    #[test]
    fn a_bare_word_is_handed_back_for_the_filesystem_to_judge() {
        // Whether `Screen`, `LICENSE` or `Makefile` is a file is a question
        // for the filesystem, not for its spelling. `links` returns all three;
        // the resolution decides, and only a word that names nothing *and*
        // could only be an item is handed to rustdoc.
        assert_eq!(links("[Screen](Screen)\n")[0].1, "Screen");
        assert_eq!(links("[MIT](LICENSE)\n")[0].1, "LICENSE");
        assert_eq!(links("[x](Makefile)\n")[0].1, "Makefile");
        assert_eq!(links("[x](README.md)\n").len(), 1);
    }

    #[test]
    fn only_a_word_that_names_nothing_is_handed_to_rustdoc() {
        // A module, a type, and rustdoc's own path keywords.
        assert!(names_a_rust_item("Screen"));
        assert!(names_a_rust_item("Vec::push"));
        assert!(names_a_rust_item("crate"));
        assert!(names_a_rust_item("metrics"));
        // A file named without an extension is shouted; a Rust path is not.
        assert!(!names_a_rust_item("LICENSE"));
        assert!(!names_a_rust_item("CHANGELOG"));
        assert!(!names_a_rust_item("README.md"));
    }

    #[test]
    fn a_url_and_an_anchor_are_not_paths() {
        assert!(links("[a](https://example.com/x)\n").is_empty());
        assert!(links("[a](#section)\n").is_empty());
        assert!(links("[a](mailto:x@y.z)\n").is_empty());
        assert_eq!(links("[a](docs/x.md#section)\n")[0].1, "docs/x.md");
    }

    #[test]
    fn only_a_doc_comment_is_prose_in_a_rust_file() {
        // A link in a string literal is data. This module's own tests are full
        // of them.
        let source = "/// See [the guide](docs/guide.md).\nlet s = \"[x](nowhere.md)\";\n";
        let prose = doc_comments(source);
        assert_eq!(links(&prose).len(), 1);
        assert_eq!(
            links(&prose)[0].0,
            1,
            "the line number still points at the source"
        );
    }
}
