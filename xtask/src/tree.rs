//! Rules about the shape of the repository: how big a file may be, what a
//! front page must say, and which crates may have no tests.
//!
//! Its neighbour `paths.rs` answers what is here; this decides what that is
//! allowed to look like.

use std::fs;
use std::path::Path;

use crate::paths::{tracked, under_src};

/// No file under a `src/` is doing two jobs.
///
/// The limit is a ratchet, not the principle; raising it is a decision to
/// argue for in a commit message. There is no allow-list. Tests are exempt as
/// a class — a list of cases is not a grab-bag — so only `src/` is read.
pub fn file_sizes(limit: usize) -> Result<String, String> {
    let mut over = Vec::new();
    let mut counted = 0;
    for file in tracked("*.rs") {
        if !under_src(&file) {
            continue;
        }
        counted += 1;
        let lines = fs::read_to_string(&file)
            .map(|s| s.lines().count())
            .unwrap_or(0);
        if lines > limit {
            over.push(format!("  {lines:5}  {}", file.display()));
        }
    }
    if counted == 0 {
        // A filter that matches nothing reads exactly like a tree with no
        // offenders.
        return Err("no source files found — the filter is matching nothing".into());
    }
    if over.is_empty() {
        Ok(format!("{counted} file(s), none over {limit} lines"))
    } else {
        Err(format!(
            "{}\n{} file(s) above the {limit}-line limit. Split by job, not by\n\
             type — if you cannot name a file's job in one sentence without\n\
             \"and\", the \"and\" is the seam.",
            over.join("\n"),
            over.len()
        ))
    }
}

/// The front page warns that the API moves.
///
/// The root README only. A crate README below it is arrived at from the front
/// page rather than found cold, and the same warning on every one of them is
/// noise a reader learns to skip past.
pub fn readmes_warn() -> Result<String, String> {
    let mut missing = Vec::new();
    let mut checked = 0;
    for file in tracked("*README.md") {
        if file.parent().is_some_and(|p| !p.as_os_str().is_empty()) {
            continue;
        }
        checked += 1;
        let path = file.to_string_lossy().to_string();
        let text = fs::read_to_string(&file).unwrap_or_default();
        // The callout itself, not the lines around it: read the header instead
        // and the box a reader sees can say anything, passing on words found
        // elsewhere. The block is joined because where it wraps is formatting.
        let warning: String = text
            .lines()
            .skip_while(|line| line.trim() != "> [!WARNING]")
            .skip(1)
            .take_while(|line| line.starts_with('>'))
            .collect::<Vec<_>>()
            .join(" ");
        if !(warning.contains("Under heavy development") && warning.contains("API can break")) {
            missing.push(format!("  {path}: no development warning in the header"));
        }
    }
    // No front page at all is the one failure this would otherwise report as
    // success. It reads what git tracks, so ignored is the same as absent.
    if checked == 0 {
        return Err("  no front page found — the filter is matching nothing".into());
    }
    if missing.is_empty() {
        Ok("the front page warns".into())
    } else {
        Err(missing.join("\n"))
    }
}

/// Every crate is tested, or says in writing why it is not.
///
/// A gap is fine when it is chosen. What is not fine is a gap nobody knows
/// about, so a crate with neither a `tests/` directory holding something nor a
/// `#[cfg(test)]` fails unless it is named with a reason — and the reason
/// prints, so it is re-read rather than accumulated.
pub fn crates_are_tested(exempt: &[(&str, &str)]) -> Result<String, String> {
    let mut problems = Vec::new();
    let mut notes = Vec::new();
    let mut used = Vec::new();

    for manifest in tracked("*Cargo.toml") {
        let text = fs::read_to_string(&manifest).unwrap_or_default();
        if !text.contains("[package]") {
            continue;
        }
        // Empty for a crate at the repository root: `Path::new(".").join("src")`
        // is `./src`, whose components never prefix `src/lib.rs`.
        let dir = manifest
            .parent()
            .unwrap_or(Path::new(""))
            .to_string_lossy()
            .to_string();
        // The name a reader would use for the root.
        let named = if dir.is_empty() {
            ".".to_string()
        } else {
            dir.clone()
        };

        let has_tests = fs::read_dir(Path::new(&dir).join("tests"))
            .map(|entries| {
                entries.filter_map(Result::ok).any(|e| {
                    e.path().extension().is_some_and(|x| x == "rs")
                        // An empty file is not a test.
                        && e.metadata().map(|m| m.len() > 0).unwrap_or(false)
                })
            })
            .unwrap_or(false);

        // `#[cfg(test)]` on a line that is not a comment — a sentence about
        // testing is not a test.
        let has_unit = tracked("*.rs").iter().any(|f| {
            f.starts_with(Path::new(&dir).join("src"))
                && fs::read_to_string(f).unwrap_or_default().lines().any(|l| {
                    let t = l.trim();
                    t.contains("#[cfg(test)]") && !t.starts_with("//")
                })
        });

        if has_tests || has_unit {
            if let Some((name, _)) = exempt.iter().find(|(n, _)| *n == named) {
                used.push(*name);
                problems.push(format!(
                    "  {named} is exempt and has tests. Remove the exemption."
                ));
            }
            continue;
        }
        match exempt.iter().find(|(n, _)| *n == named) {
            Some((name, reason)) => {
                used.push(*name);
                notes.push(format!("  {named:<24} {reason}"));
            }
            None => problems.push(format!("  {named} has no tests and is not exempt")),
        }
    }

    for (name, _) in exempt {
        if !used.contains(name) {
            problems.push(format!(
                "  the exemption names {name}, which is not a crate here"
            ));
        }
    }

    if problems.is_empty() {
        Ok(notes.join("\n"))
    } else {
        Err(problems.join("\n"))
    }
}

/// Every crate that publishes denies `missing_docs`.
///
/// Where nothing publishes, `true` prints `no publishable crates` — never
/// `ok` — because a filter that matches nothing reads like a clean tree.
pub fn published_crates_deny_missing_docs(required: bool) -> Result<String, String> {
    if !required {
        return Ok("not adopted".into());
    }
    let mut publishable = Vec::new();
    let mut without = Vec::new();
    for manifest in tracked("*Cargo.toml") {
        let text = fs::read_to_string(&manifest).unwrap_or_default();
        if !text.contains("[package]") || text.contains("publish = false") {
            continue;
        }
        let dir = manifest.parent().unwrap_or(Path::new(""));
        let root = ["src/lib.rs", "src/main.rs"]
            .iter()
            .map(|r| dir.join(r))
            .find(|p| p.is_file());
        let Some(root) = root else { continue };
        publishable.push(root.display().to_string());
        let denies = fs::read_to_string(&root)
            .unwrap_or_default()
            .lines()
            .any(|l| l.trim() == "#![deny(missing_docs)]");
        if !denies {
            without.push(format!("  {}", root.display()));
        }
    }
    if publishable.is_empty() {
        return Ok("no publishable crates".into());
    }
    if without.is_empty() {
        Ok(format!("{} crate(s) deny missing_docs", publishable.len()))
    } else {
        Err(format!(
            "{}\n\nA crate that publishes documents every public item.",
            without.join("\n")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::in_directory;
    use std::path::PathBuf;

    /// A tree shaped like a repository, and a real git checkout, because
    /// `tracked` asks git what is there.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str, files: &[(&str, &str)]) -> Self {
            let root =
                std::env::temp_dir().join(format!("xpui-tree-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("scratch");
            for (path, body) in files {
                let at = root.join(path);
                fs::create_dir_all(at.parent().expect("a parent")).expect("scratch");
                fs::write(&at, body).expect("scratch");
            }
            let done = std::process::Command::new("git")
                .args(["init", "--quiet"])
                .current_dir(&root)
                .status()
                .expect("git init");
            assert!(done.success(), "the scratch tree has to be a checkout");
            Self(root)
        }

        /// Runs `f` with the scratch tree as the working directory, because
        /// every check here reads paths relative to the repository root.
        fn run<T>(&self, f: impl FnOnce() -> T) -> T {
            in_directory(&self.0, f)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_file_over_the_limit_is_named_and_one_under_it_is_not() {
        let long = "// pad\n".repeat(12);
        let tree = Scratch::new(
            "sizes",
            &[("src/big.rs", &long), ("src/small.rs", "// one line\n")],
        );
        let over = tree.run(|| file_sizes(10));
        let why = over.expect_err("12 lines is over a limit of 10");
        assert!(why.contains("src/big.rs"), "{why}");
        assert!(!why.contains("src/small.rs"), "{why}");
        assert!(tree.run(|| file_sizes(100)).is_ok());
    }

    #[test]
    fn a_tree_with_no_sources_is_a_filter_matching_nothing() {
        // Reporting "none over the limit" for a tree it never looked at is
        // the failure this guard exists for.
        let tree = Scratch::new("no-sources", &[("README.md", "# x\n")]);
        assert!(tree.run(|| file_sizes(400)).is_err());
    }

    #[test]
    fn a_front_page_without_the_warning_is_named() {
        let good = "[![CI](x)](y)\n\n# x\n\n> [!WARNING]\n> Under heavy development. Not production-ready. The\n> API can break without notice.\n";
        // A crate README below the root is not asked for it.
        let tree = Scratch::new(
            "readmes",
            &[("README.md", good), ("core/README.md", "# a crate\n")],
        );
        assert!(tree.run(readmes_warn).is_ok());
        // The front page is, and the message names the page rather than the
        // filter — those are not the same failure.
        let tree = Scratch::new("readmes", &[("README.md", "# x\n")]);
        let why = tree.run(readmes_warn).expect_err("no warning");
        assert!(why.contains("README.md: no development warning"), "{why}");
        // Both halves of the sentence are required, and so is the callout
        // carrying them — the last has the phrases beside the box, not in it.
        for bad in [
            "> [!WARNING]\n> Under heavy development.\n",
            "> [!WARNING]\n> The API can break.\n",
            "> Under heavy development. The API can break.\n",
            "> [!WARNING]\n> Do not use.\n\nUnder heavy development. The API can break.\n",
        ] {
            let text = format!("# x\n\n{bad}");
            let tree = Scratch::new("readmes", &[("README.md", text.as_str())]);
            assert!(tree.run(readmes_warn).is_err(), "{bad}");
        }
        // Counting a nested README as the front page is how "none" passes.
        let tree = Scratch::new("nested", &[("docs/README.md", "# nested\n")]);
        let why = tree.run(readmes_warn).expect_err("there is no front page");
        assert!(why.contains("matching nothing"), "{why}");
    }

    #[test]
    fn a_crate_tested_only_by_a_cfg_test_module_counts_as_tested() {
        // A crate at the root has sources at `src/…`; the prefix must not be
        // `./src`.
        let tree = Scratch::new(
            "root-crate",
            &[
                ("Cargo.toml", "[package]\nname = \"root\"\n"),
                ("src/lib.rs", "#[cfg(test)]\nmod tests {}\n"),
            ],
        );
        assert!(tree.run(|| crates_are_tested(&[])).is_ok());
    }

    #[test]
    fn a_crate_with_no_tests_needs_a_written_reason() {
        let tree = Scratch::new(
            "untested",
            &[
                ("Cargo.toml", "[package]\nname = \"root\"\n"),
                ("src/lib.rs", "pub fn f() {}\n"),
            ],
        );
        let why = tree
            .run(|| crates_are_tested(&[]))
            .expect_err("no tests anywhere");
        assert!(why.contains("has no tests"), "{why}");
        // The reason is accepted, and printed back.
        let note = tree.run(|| crates_are_tested(&[(".", "a HAL cannot compile for a laptop")]));
        assert!(note.expect("exempt").contains("HAL"));
    }

    #[test]
    fn an_exemption_for_a_crate_that_grew_tests_is_a_failure() {
        let tree = Scratch::new(
            "stale",
            &[
                ("Cargo.toml", "[package]\nname = \"root\"\n"),
                ("src/lib.rs", "#[cfg(test)]\nmod tests {}\n"),
            ],
        );
        let why = tree
            .run(|| crates_are_tested(&[(".", "was untested once")]))
            .expect_err("the exemption is stale");
        assert!(why.contains("Remove the exemption"), "{why}");
    }

    #[test]
    fn a_sentence_about_cfg_test_is_not_a_test() {
        let tree = Scratch::new(
            "comment",
            &[
                ("Cargo.toml", "[package]\nname = \"root\"\n"),
                ("src/lib.rs", "// Add a #[cfg(test)] module here one day.\n"),
            ],
        );
        assert!(tree.run(|| crates_are_tested(&[])).is_err());
    }

    #[test]
    fn a_publishable_crate_without_the_deny_is_named_and_none_at_all_is_said() {
        let tree = Scratch::new(
            "deny",
            &[
                ("Cargo.toml", "[package]\nname = \"pub\"\n"),
                ("src/lib.rs", "pub fn f() {}\n"),
            ],
        );
        let why = tree
            .run(|| published_crates_deny_missing_docs(true))
            .expect_err("no deny");
        assert!(why.contains("src/lib.rs"), "{why}");
        let private = Scratch::new(
            "private",
            &[
                ("Cargo.toml", "[package]\nname = \"x\"\npublish = false\n"),
                ("src/lib.rs", "pub fn f() {}\n"),
            ],
        );
        assert_eq!(
            private
                .run(|| published_crates_deny_missing_docs(true))
                .expect("nothing to check"),
            "no publishable crates"
        );
    }
}
