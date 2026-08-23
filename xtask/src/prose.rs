//! Whether every Rust block in the documentation is actually compiled.
//!
//! Four rules rather than one, because three of them are ways for a fence to
//! look compiled and not be. Its neighbour `docs.rs` answers a different
//! question — whether the paths a document links to exist.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::fences::fences;
use crate::paths::tracked;

/// Every Rust block in the documentation is compiled, and none of it is
/// exempted from being.
///
/// A guide's snippets rot in silence otherwise: they are prose to every tool
/// in the build. The mechanism is `#[doc = include_str!(...)]` on a
/// `#[cfg(doctest)]` item, which makes rustdoc compile the fences as doctests.
///
/// Four rules, not one, because each of the other three is a way for a fence
/// to look compiled and not be:
///
/// - a page with a Rust fence that nothing mounts;
/// - an **unlabelled** fence, which rustdoc compiles as Rust — so leaving it
///   unlabelled inside a mounted page is a snippet nobody chose to compile,
///   and outside one it hides from this check entirely;
/// - an **unknown language**, because ` ```rustt ` is a typo that silently
///   compiles nothing, and a shrug is what let it in;
/// - an **`ignore` attribute**, whose budget is zero. `ignore` is how a
///   snippet stops being checked while still looking like code.
///
/// This looks for the **attribute**, not a call — a document merely
/// *mentioned* in a comment would satisfy a laxer search while compiling
/// nothing.
pub fn is_compiled(exempt: &[&str], known: &[&str]) -> Result<String, String> {
    let mut included = BTreeSet::new();
    for source in tracked("*.rs") {
        let text = fs::read_to_string(&source).unwrap_or_default();
        for line in text.lines() {
            let line = line.trim();
            if !line.starts_with("#[doc") || !line.contains("include_str!") {
                continue;
            }
            let Some(open) = line.find("include_str!(\"") else {
                continue;
            };
            let after = &line[open + 14..];
            let Some(close) = after.find('"') else {
                continue;
            };
            let from = source
                .parent()
                .unwrap_or(Path::new("."))
                .join(&after[..close]);
            included.insert(normalise(&from));
        }
    }

    let mut problems = Vec::new();
    let mut compiled = 0;
    let mut used = Vec::new();
    for doc in tracked("*.md") {
        let path = doc.to_string_lossy().to_string();
        if let Some(prefix) = exempt.iter().find(|e| path == **e) {
            used.push(*prefix);
            continue;
        }
        let text = fs::read_to_string(&doc).unwrap_or_default();
        let blocks = fences(&text);
        let mounted = included.contains(&normalise(&doc));

        for fence in &blocks {
            let attributes = attributes_of(&text, fence);
            match fence.language.as_str() {
                "rust" if attributes.iter().any(|a| a == "ignore") => problems.push(format!(
                    "  {path}:{}  ```rust,ignore — the budget for these is zero",
                    fence.start
                )),
                "rust" if !mounted => problems.push(format!(
                    "  {path}:{}  ```rust that nothing compiles",
                    fence.start
                )),
                "rust" => compiled += 1,
                "" => problems.push(format!(
                    "  {path}:{}  an unlabelled fence — rustdoc compiles these as Rust",
                    fence.start
                )),
                "ignore" => problems.push(format!(
                    "  {path}:{}  ```ignore — the budget for these is zero",
                    fence.start
                )),
                other if !known.contains(&other) => problems.push(format!(
                    "  {path}:{}  ```{other} is not a language this repository writes",
                    fence.start
                )),
                _ => {}
            }
        }
    }

    // A `///` comment can carry an `ignore` too, and rustdoc treats it the
    // same way: as a block it parses and does not run.
    for source in tracked("*.rs") {
        let text = fs::read_to_string(&source).unwrap_or_default();
        for (number, line) in text.lines().enumerate() {
            let line = line.trim();
            let Some(rest) = line
                .strip_prefix("///")
                .or_else(|| line.strip_prefix("//!"))
            else {
                continue;
            };
            let rest = rest.trim();
            if !rest.starts_with("```") {
                continue;
            }
            let info = rest.trim_start_matches('`').to_lowercase();
            if info.split([',', ' ']).any(|a| a == "ignore") {
                problems.push(format!(
                    "  {}:{}  ```{info} in a doc comment — the budget for these is zero",
                    source.display(),
                    number + 1
                ));
            }
        }
    }

    for name in exempt {
        if !used.contains(name) {
            problems.push(format!(
                "  the exemption names {name}, which is not a document here"
            ));
        }
    }

    if problems.is_empty() {
        Ok(format!("{compiled} Rust block(s), all compiled"))
    } else {
        Err(format!(
            "{}\n\nMount a page whose Rust should compile:\n\
             \n    #[cfg(doctest)]\n    #[doc = include_str!(\"../../docs/name.md\")]\n\
             \x20   struct DocsName;\n\
             \nAn `ignore` is a snippet that stopped being checked. Make it compile,\n\
             or make it `text`.",
            problems.join("\n")
        ))
    }
}

/// The attributes on a fence's info string — everything after the language.
fn attributes_of(text: &str, fence: &crate::fences::Fence) -> Vec<String> {
    let Some(line) = text.lines().nth(fence.start - 1) else {
        return Vec::new();
    };
    let info = line
        .trim_start()
        .trim_start_matches(['`', '~'])
        .to_lowercase();
    info.split([',', ' '])
        .skip(1)
        .filter(|a| !a.is_empty())
        .map(str::to_string)
        .collect()
}

/// `a/b/../c` as `a/c`, so two spellings of one file compare equal.
fn normalise(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch checkout holding one page and one source that may mount it.
    fn against(name: &str, page: &str, mount: bool) -> Result<String, String> {
        let root = std::env::temp_dir().join(format!("xpui-prose-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).expect("scratch");
        fs::write(root.join("docs.md"), page).expect("scratch");
        let source = if mount {
            "#[cfg(doctest)]\n#[doc = include_str!(\"../docs.md\")]\nstruct Docs;\n"
        } else {
            "pub fn f() {}\n"
        };
        fs::write(root.join("src/lib.rs"), source).expect("scratch");
        let done = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&root)
            .status()
            .expect("git init");
        assert!(done.success());

        let out = crate::paths::in_directory(&root, || is_compiled(&[], &["text", "bash", "cpp"]));
        let _ = fs::remove_dir_all(&root);
        out
    }

    #[test]
    fn a_rust_block_nothing_mounts_is_a_fault() {
        let why = against("unmounted", "```rust\nlet x = 1;\n```\n", false)
            .expect_err("nothing compiles it");
        assert!(why.contains("nothing compiles"), "{why}");
        assert!(against("mounted", "```rust\nlet x = 1;\n```\n", true).is_ok());
    }

    #[test]
    fn an_unlabelled_fence_is_a_fault_even_on_a_mounted_page() {
        // Rustdoc compiles an unlabelled block as Rust, so leaving it
        // unlabelled is a snippet nobody chose to compile.
        let why =
            against("unlabelled", "```\nlet x = 1;\n```\n", true).expect_err("an unlabelled fence");
        assert!(why.contains("unlabelled"), "{why}");
    }

    #[test]
    fn a_typo_in_the_language_is_a_fault_rather_than_a_shrug() {
        // ```rustt compiles nothing and looks like code.
        let why =
            against("typo", "```rustt\nlet x = 1;\n```\n", true).expect_err("an unknown language");
        assert!(why.contains("rustt"), "{why}");
        assert!(against("known", "```bash\ncargo test\n```\n", true).is_ok());
    }

    #[test]
    fn an_ignore_is_a_fault_on_a_mounted_page_where_it_hides_best() {
        // `rust,ignore` on a mounted page: rustdoc parses it and runs nothing,
        // and the old check counted the page as compiled and said nothing.
        let why = against("ignore", "```rust,ignore\nlet x = 1;\n```\n", true)
            .expect_err("the budget for these is zero");
        assert!(why.contains("budget"), "{why}");
        let why = against("bare-ignore", "```ignore\nlet x = 1;\n```\n", true)
            .expect_err("the budget for these is zero");
        assert!(why.contains("budget"), "{why}");
    }

    #[test]
    fn a_parent_step_is_resolved_before_comparing() {
        // Two spellings of one path must compare equal, or a page mounted as
        // `../../docs/x.md` looks unmounted.
        assert_eq!(
            normalise(Path::new("src/../docs/x.md")),
            PathBuf::from("docs/x.md")
        );
    }

    #[test]
    fn an_attribute_is_read_off_the_fence_that_carries_it() {
        let text = "```rust,ignore\nlet x = 1;\n```\n";
        let fence = &fences(text)[0];
        assert_eq!(attributes_of(text, fence), ["ignore"]);
        let plain = "```rust\nlet x = 1;\n```\n";
        assert!(attributes_of(plain, &fences(plain)[0]).is_empty());
    }
}
