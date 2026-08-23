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
/// A file grows past a few hundred lines when two jobs start sharing a name,
/// and nothing notices — it still compiles, the tests still pass, and the seam
/// is visible only to whoever reads it next. The limit is a ratchet, not the
/// principle; raising it is a decision to argue for in a commit message.
///
/// There is deliberately no allow-list. Tests are exempt as a class, because a
/// list of cases is not a grab-bag, so this looks only under `src/`.
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
        // offenders. It happened: `/src/` never matched `src/lib.rs` in a
        // crate that sits at the repository root, so nothing was measured at
        // all and the check reported success every time.
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

/// Every README warns that the API moves.
///
/// **Every** one, so that adding a README is a decision about whether it needs
/// the warning rather than a decision nobody makes. Naming the pages that
/// carry it would protect nothing: the edit that drops a page from that list
/// is the edit that drops its banner. So this names the exceptions instead.
pub fn readmes_warn(not_a_front_page: &[&str]) -> Result<String, String> {
    let mut missing = Vec::new();
    let mut checked = 0;
    let mut used = Vec::new();
    for file in tracked("*README.md") {
        let path = file.to_string_lossy().to_string();
        if let Some(name) = not_a_front_page.iter().find(|e| **e == path) {
            used.push(*name);
            continue;
        }
        checked += 1;
        let text = fs::read_to_string(&file).unwrap_or_default();
        // Lines 3 and 4, read as one: the sentence wraps, and where it wraps
        // is a formatting decision rather than something to assert.
        let banner: String = text.lines().skip(2).take(2).collect::<Vec<_>>().join(" ");
        if !(banner.contains("Under heavy development") && banner.contains("API can break")) {
            missing.push(format!("  {path}: no development banner on lines 3-4"));
        }
    }
    // An exemption naming a file that has moved is an exemption nobody will
    // notice has stopped applying.
    for name in not_a_front_page {
        if !used.contains(name) {
            missing.push(format!(
                "  the exemption names {name}, which is not a README here"
            ));
        }
    }
    if missing.is_empty() {
        Ok(format!("{checked} README(s)"))
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
        // Empty for a crate at the repository root, which is what makes the
        // prefix below work: `Path::new(".").join("src")` is `./src`, whose
        // components are [CurDir, "src"], and those never prefix
        // `src/lib.rs`. That made the unit-test half of this check dead in
        // every repository whose crate sits at the root, masked only by those
        // crates happening to have a `tests/` directory too.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::in_directory;
    use std::path::PathBuf;

    /// A tree shaped like a repository, so these say the same thing wherever
    /// this file is carried.
    ///
    /// It is a real git checkout, because `tracked` asks git what is there —
    /// which is the point: a check that reads the filesystem directly would
    /// walk `target/` and somebody else's vendored sources.
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
    fn a_readme_without_the_banner_is_named() {
        let good = "# x\n\n> **Under heavy development.** Not production-ready. The\n> API can break without notice.\n";
        let tree = Scratch::new(
            "readmes",
            &[("README.md", good), ("docs/README.md", "# no banner\n")],
        );
        let why = tree
            .run(|| readmes_warn(&[]))
            .expect_err("one has no banner");
        assert!(why.contains("docs/README.md"), "{why}");
        assert!(!why.contains("\n  README.md"), "{why}");
        // ...and naming it as an exception is accepted.
        assert!(tree.run(|| readmes_warn(&["docs/README.md"])).is_ok());
        // ...but an exception for a file that is not there is a failure.
        let why = tree
            .run(|| readmes_warn(&["docs/README.md", "gone/README.md"]))
            .expect_err("a stale exemption");
        assert!(why.contains("gone/README.md"), "{why}");
    }

    #[test]
    fn a_crate_tested_only_by_a_cfg_test_module_counts_as_tested() {
        // This is the one that was dead: a crate at the repository root has
        // sources at `src/…`, and the prefix built for it was `./src`.
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
}
