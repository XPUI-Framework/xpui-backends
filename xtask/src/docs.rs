//! Where a document's links point, on disk.
//!
//! Whether rustdoc resolves an intra-doc link is asked by running rustdoc;
//! whether a page's Rust compiles is `prose.rs`; whether its commands could
//! run is `commands.rs`.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::fences::fences;
use crate::paths::{exists_exactly, tracked};

/// Every relative link in every document resolves to a path on disk.
///
/// Rust sources too: a `///` comment writes a markdown link like any page.
/// Absolute URLs are somebody else's uptime and are not fetched. Anchors are
/// split off — `docs/x.md#section` is a claim about `docs/x.md`.
pub fn doc_paths() -> Result<String, String> {
    let mut broken = Vec::new();
    let mut checked = 0;
    let mut pages = 0;
    for doc in tracked("*.md").into_iter().chain(tracked("*.rs")) {
        let raw = fs::read_to_string(&doc).unwrap_or_default();
        // In a Rust file only the doc comments are prose; a `[a](b.md)` in a
        // string literal is data.
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
            // Nothing on disk, and it could only be an item: rustdoc's.
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
/// Not a markdown parser. It skips links inside a fence or a `code span`,
/// URLs, anchors and `mailto:`; strips a CommonMark title (`[a](x.md "Title")`)
/// and angle brackets (`[a](<a b.md>)`); and returns a bare word for the
/// filesystem to judge.
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
    // A bare word — `Screen`, `LICENSE` — could be an item or a file, and its
    // spelling does not say which. It is returned; the resolution decides.
    Some(path.to_string())
}

/// Whether a bare word names a Rust item rather than a file.
///
/// `::` says item; a dot says file. Otherwise: a file named without an
/// extension is shouted (`LICENSE`, `README`), and a Rust path never is —
/// modules are lowercase, types CamelCase. No lowercase letter means a file.
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
        // `links` returns every bare word; the resolution decides.
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
