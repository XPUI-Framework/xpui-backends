//! How long a comment may be, and what it may not say.
//!
//! A comment says only what the code cannot. That is judged by a reader; what
//! a gate can measure is length and the five phrases that are only ever about
//! the past. Both are ratchets: a number that names a list today and can only
//! come down.

use std::fs;
use std::path::{Path, PathBuf};

use crate::paths::tracked;

/// The three caps, and how much of the repository they cover.
#[derive(Clone, Copy)]
pub struct Caps {
    /// A `///` block, or a module's `//!`, with doctest fences excluded.
    pub doc: usize,
    /// A module `//!` — the crate root's is prose and is not measured.
    pub header: usize,
    /// A run of `//` in a body, or of `#` in a manifest.
    pub run: usize,
}

/// The phrases that are only ever about the past.
///
/// `this replaces`, `before this`, `the first time` and `no longer` each have
/// a present-tense use that is a fact, so they are the reviewer's, not here.
const NARRATION: [&str; 5] = ["used to", "for a while", "previously", "was found", "spec "];

/// One comment block: where it starts, how long it is, and which cap applies.
struct Block {
    file: PathBuf,
    line: usize,
    length: usize,
    kind: Kind,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Doc,
    Header,
    Run,
}

/// Every source file a comment check reads: Rust outside `tests/`, C++, and
/// manifests, under the prefix if one is set.
fn sources(prefix: Option<&str>) -> Vec<PathBuf> {
    let mut all = tracked("*.rs");
    all.extend(tracked("*.cpp"));
    all.extend(tracked("*.h"));
    all.extend(tracked("*Cargo.toml"));
    all.retain(|p| {
        let text = p.to_string_lossy();
        !text.starts_with("tests/")
            && !text.contains("/tests/")
            && prefix.is_none_or(|pre| text.starts_with(pre))
    });
    all
}

/// Whether a Rust file's `//!` is the crate's front page rather than a module's.
fn is_crate_root(path: &Path) -> bool {
    let text = path.to_string_lossy();
    text.ends_with("src/lib.rs") || text.ends_with("src/main.rs") || text.contains("src/bin/")
}

/// Whether a comment line is a fence marker rather than prose that mentions
/// one: the content after `///` or `//!` starts with the backticks.
fn fence_line(line: &str) -> bool {
    line.trim_start_matches(['/', '!'])
        .trim_start()
        .starts_with("```")
}

/// The comment blocks of one file, with fence lines already removed from
/// doc blocks.
fn blocks(path: &Path, text: &str) -> Vec<Block> {
    let manifest = path.ends_with("Cargo.toml");
    let cpp = path.extension().is_some_and(|x| x == "cpp" || x == "h");
    let root = !cpp && !manifest && is_crate_root(path);

    let mut out = Vec::new();
    let mut open: Option<(Kind, usize, usize, bool)> = None; // kind, start, length, in_fence
    let mut first_block = true;

    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim_start();
        let kind = if manifest {
            line.starts_with('#').then_some(Kind::Run)
        } else if line.starts_with("//!") {
            Some(if root { Kind::Doc } else { Kind::Header })
        } else if line.starts_with("///") && !line.starts_with("////") {
            Some(Kind::Doc)
        } else if line.starts_with("//") {
            // A C++ file's leading block is its header.
            Some(if cpp && first_block {
                Kind::Header
            } else {
                Kind::Run
            })
        } else {
            None
        };

        let opens_fence = fence_line(line);
        match (kind, &mut open) {
            (Some(k), Some((ok, _, length, in_fence))) if *ok == k => {
                if opens_fence {
                    *in_fence = !*in_fence;
                } else if !*in_fence {
                    *length += 1;
                }
            }
            (Some(k), _) => {
                if let Some((ok, start, length, _)) = open.take() {
                    out.push(Block {
                        file: path.to_path_buf(),
                        line: start,
                        length,
                        kind: ok,
                    });
                }
                open = Some((k, index + 1, usize::from(!opens_fence), opens_fence));
            }
            (None, Some(_)) => {
                if let Some((ok, start, length, _)) = open.take() {
                    out.push(Block {
                        file: path.to_path_buf(),
                        line: start,
                        length,
                        kind: ok,
                    });
                }
                if !line.is_empty() {
                    first_block = false;
                }
            }
            (None, None) => {
                if !line.is_empty() {
                    first_block = false;
                }
            }
        }
    }
    if let Some((ok, start, length, _)) = open {
        out.push(Block {
            file: path.to_path_buf(),
            line: start,
            length,
            kind: ok,
        });
    }
    // A crate root's `//!` is prose, not a block to measure.
    out.retain(|b| !(root && b.kind == Kind::Doc && b.line == 1));
    out
}

/// No comment block is over its cap.
pub fn comment_blocks(caps: Option<Caps>, scope: Option<&str>) -> Result<String, String> {
    let Some(caps) = caps else {
        return Ok("not adopted".into());
    };
    let (mut docs, mut headers, mut runs) = (0, 0, 0);
    let mut over = Vec::new();
    for file in sources(scope) {
        let text = fs::read_to_string(&file).unwrap_or_default();
        for block in blocks(&file, &text) {
            let cap = match block.kind {
                Kind::Doc => {
                    docs += 1;
                    caps.doc
                }
                Kind::Header => {
                    headers += 1;
                    caps.header
                }
                Kind::Run => {
                    runs += 1;
                    caps.run
                }
            };
            if block.length > cap {
                over.push(format!(
                    "  {}:{}  {} lines, cap {cap}",
                    block.file.display(),
                    block.line,
                    block.length
                ));
            }
        }
    }
    if docs + headers + runs == 0 {
        return Err("no comments measured — the filter is matching nothing".into());
    }
    if over.is_empty() {
        Ok(format!(
            "{docs} doc comments, {headers} headers, {runs} runs measured"
        ))
    } else {
        Err(format!(
            "{}\n\nA comment says only what the code cannot. Cut what restates it,\n\
             cut the history, and move an argument to docs/.",
            over.join("\n")
        ))
    }
}

/// No comment says something that is only ever about the past.
///
/// A run's lines are joined before matching, so a phrase wrapped across two
/// of them — "this one used / to carry" — is still one phrase.
pub fn comment_narration(checked: bool, scope: Option<&str>) -> Result<String, String> {
    if !checked {
        return Ok("not adopted".into());
    }
    let mut hits = Vec::new();
    let mut lines = 0;
    for file in sources(scope) {
        let manifest = file.ends_with("Cargo.toml");
        let text = fs::read_to_string(&file).unwrap_or_default();
        let mut run: Vec<&str> = Vec::new();
        let mut start = 0;
        let flush = |run: &mut Vec<&str>, start: usize, hits: &mut Vec<String>| {
            if run.is_empty() {
                return;
            }
            let joined = run.join(" ").to_lowercase();
            for phrase in NARRATION {
                if narrates(&joined, phrase) {
                    hits.push(format!("  {}:{start}  `{}`", file.display(), phrase.trim()));
                }
            }
            run.clear();
        };
        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim_start();
            let is_comment = if manifest {
                line.starts_with('#')
            } else {
                line.starts_with("//")
            };
            if is_comment {
                if run.is_empty() {
                    start = index + 1;
                }
                lines += 1;
                run.push(line.trim_start_matches(['/', '!', '#']).trim());
            } else {
                flush(&mut run, start, &mut hits);
            }
        }
        flush(&mut run, start, &mut hits);
    }
    if lines == 0 {
        return Err("no comment lines read — the filter is matching nothing".into());
    }
    if hits.is_empty() {
        Ok(format!("{lines} comment lines, none about the past"))
    } else {
        Err(format!(
            "{}\n\nThe merged state has no past. The commit message has.",
            hits.join("\n")
        ))
    }
}

/// Whether lowercased comment text contains `phrase`. `spec ` needs a digit
/// after it: "spec" alone is a word.
fn narrates(text: &str, phrase: &str) -> bool {
    if phrase == "spec " {
        text.match_indices("spec ")
            .any(|(at, _)| text[at + 5..].starts_with(|c: char| c.is_ascii_digit()))
    } else {
        text.contains(phrase)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure(name: &str, text: &str) -> Vec<(usize, usize, Kind)> {
        blocks(Path::new(name), text)
            .into_iter()
            .map(|b| (b.line, b.length, b.kind))
            .collect()
    }

    #[test]
    fn a_doctest_fence_is_not_counted() {
        let text =
            "/// One.\n/// ```\n/// let x = 1;\n/// let y = 2;\n/// ```\n/// Two.\nfn f() {}\n";
        assert_eq!(measure("src/a.rs", text), [(1, 2, Kind::Doc)]);
    }

    #[test]
    fn a_code_span_that_mentions_a_fence_is_not_a_fence() {
        // "because ` ```rustt ` is a typo" is prose, and counted.
        let text = "/// One.\n/// because ` ```rustt ` is a typo\n/// Three.\nfn f() {}\n";
        assert_eq!(measure("src/a.rs", text), [(1, 3, Kind::Doc)]);
    }

    #[test]
    fn a_crate_root_header_is_prose_and_a_module_header_is_measured() {
        let text = "//! a\n//! b\n//! c\nfn f() {}\n";
        assert!(measure("src/lib.rs", text).is_empty());
        assert_eq!(measure("src/module.rs", text), [(1, 3, Kind::Header)]);
    }

    #[test]
    fn a_cpp_files_leading_block_is_its_header_and_later_ones_are_runs() {
        let text = "// header\n// header\n\nint x; // trailing is code\n// body\n// body\n";
        assert_eq!(
            measure("a.cpp", text),
            [(1, 2, Kind::Header), (5, 2, Kind::Run)]
        );
    }

    #[test]
    fn a_manifest_comment_is_a_run() {
        let text = "# one\n# two\n[package]\n# three\n";
        assert_eq!(
            measure("Cargo.toml", text),
            [(1, 2, Kind::Run), (4, 1, Kind::Run)]
        );
    }

    #[test]
    fn two_kinds_back_to_back_are_two_blocks() {
        let text = "//! module\n/// item\nfn f() {}\n";
        assert_eq!(
            measure("src/m.rs", text),
            [(1, 1, Kind::Header), (2, 1, Kind::Doc)]
        );
    }

    #[test]
    fn spec_needs_a_number_after_it() {
        assert!(!narrates("the spec says", "spec "));
        assert!(narrates("see spec 08", "spec "));
    }

    #[test]
    fn a_phrase_wrapped_across_two_lines_is_still_one_phrase() {
        let text = "// this one used\n// to carry two\n";
        let joined = text
            .lines()
            .map(|l| l.trim_start_matches('/').trim())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(narrates(&joined, "used to"));
    }
}
