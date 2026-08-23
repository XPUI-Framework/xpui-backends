//! What is wrong with one documented command, if anything.
//!
//! Separate from the walk over documents next door because they are two jobs:
//! one decides which commands to read, this one decides whether a command
//! could run. It is also where every fault this check has ever had is pinned
//! by a test.

use std::collections::BTreeSet;
use std::path::Path;

use crate::paths::exists_exactly;

/// Flags whose value looks like a path and is not one.
///
/// Named, rather than "anything after any flag": that blanket rule exempted
/// every argument of every command, so `ls -l invented/x` reported nothing.
const NOT_A_PATH: [&str; 8] = [
    "--features",
    "-F",
    "--target",
    "--bin",
    "--example",
    "--board",
    "-e",
    "--profile",
];

/// What is wrong with one command, if anything.
///
/// `ours` is false for a command whose verb belongs to another tool: its
/// paths are still read — `elf2uf2-rs -d examples/rp2040/…` was wrong inside
/// exactly such a command — but its flags are not, because `-p` means
/// something else to somebody else's program.
pub fn fault(parts: &[&str], packages: &BTreeSet<String>, root: &Path, ours: bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut claimed = usize::MAX;
    for (index, word) in parts.iter().enumerate() {
        let next = parts.get(index + 1);
        match *word {
            // Every `-p` on the line, not just the last: a clippy invocation
            // names four packages and only one of them used to be read.
            // Only for a command whose verb this repository owns: `-p` means
            // something else to somebody else's tool.
            "-p" | "--package" if ours => {
                if let Some(name) = next
                    && !packages.contains(*name)
                {
                    out.push(format!(
                        "`-p {name}` is not a package here (this workspace has {})",
                        packages.iter().cloned().collect::<Vec<_>>().join(", ")
                    ));
                }
            }
            "--manifest-path" | "-S" | "-C" | "--out-dir" | "open" => {
                if let Some(path) = next
                    && checkable(path)
                    && !exists_exactly(&root.join(path))
                {
                    out.push(format!("`{word} {path}` does not exist"));
                }
                // Claimed by this arm; the bare-path rule below must not
                // report the same word a second time.
                claimed = index + 1;
            }
            _ => {}
        }
        // A bare relative path argument: `./build-and-test.sh`, `docs/x.md`.
        // Only the first component is checked, because a command may create
        // what comes after it — `target/thumbv6m…/release/app` is built, not
        // committed. `--features xpui/testing` is a feature and not a path,
        // hence the flag-value guard.
        let is_flag_value = index > 0 && NOT_A_PATH.contains(&parts[index - 1]);
        let is_claimed = index == claimed;
        // At index 0 the word is the program, and only an explicit `./` makes
        // it a path in this repository — `cargo` and `make` are on PATH. That
        // distinction is the whole check for a one-word command, and
        // `./build-and-test.sh` is the most documented command in this
        // organisation.
        let is_path = if index == 0 {
            word.starts_with("./")
        } else {
            word.starts_with("./")
                || (word.contains('/') && !word.starts_with('-') && !word.starts_with('$'))
        };
        if !is_flag_value && !is_claimed && is_path && checkable(word) {
            let first = word
                .trim_start_matches("./")
                .split('/')
                .next()
                .unwrap_or("");
            if !first.is_empty() && !first.contains('*') && !exists_exactly(&root.join(first)) {
                out.push(format!("`{word}` starts at `{first}`, which is not here"));
            }
        }
    }
    out
}

/// Whether a word is a path this repository could be expected to contain.
///
/// A URL is somebody else's; a shell variable is not resolvable from here; and
/// `target/` is a build output that exists after a build and not before, so
/// requiring it would fail a gate on a clean clone.
fn checkable(word: &str) -> bool {
    !word.starts_with('-')
        && !word.starts_with('$')
        && !word.contains("://")
        && !word.starts_with("target/")
        && !word.starts_with('~')
        // A quoted word is a program's argument, not a path this repository
        // holds: `awk '/^host:/{print $2}'` has a `/` in it and is not a file.
        && !word.starts_with('\'')
        && !word.starts_with('"')
        && !word.contains('{')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A repository-shaped scratch directory, so these tests say the same
    /// thing in every repository that carries a copy of this file.
    struct Fixture(std::path::PathBuf);

    impl Fixture {
        fn new(name: &str, entries: &[&str]) -> Self {
            // The process id keeps two repositories' suites from sharing a
            // directory if they ever run at the same time.
            let root =
                std::env::temp_dir().join(format!("xpui-xtask-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            for entry in entries {
                let path = root.join(entry);
                if entry.ends_with('/') {
                    fs::create_dir_all(&path).expect("scratch directory");
                } else {
                    fs::create_dir_all(path.parent().expect("a parent")).expect("scratch");
                    fs::write(&path, "").expect("scratch file");
                }
            }
            Self(root)
        }

        fn root(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn workspace() -> BTreeSet<String> {
        ["xpui-chrome".to_string()].into_iter().collect()
    }

    #[test]
    fn a_package_that_is_not_here_is_a_fault() {
        let found = fault(
            &["cargo", "run", "-p", "xpui-gallery"],
            &workspace(),
            Path::new("."),
            true,
        );
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("xpui-gallery"));
    }

    #[test]
    fn every_p_on_the_line_is_read_and_not_only_the_last() {
        // The four-package clippy line: the first name was the broken one.
        let found = fault(
            &["cargo", "clippy", "-p", "nowhere", "-p", "xpui-chrome"],
            &workspace(),
            Path::new("."),
            true,
        );
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("nowhere"));
    }

    #[test]
    fn a_feature_is_not_a_path() {
        let found = fault(
            &["cargo", "test", "--features", "xpui/testing"],
            &workspace(),
            Path::new("."),
            true,
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_relative_path_is_checked_by_its_first_component_only() {
        let here = Fixture::new("first-component", &["src/lib.rs"]);
        // `src/` exists; what comes after may be built rather than committed.
        assert!(
            fault(
                &["ls", "src/deep/nothing.rs"],
                &workspace(),
                here.root(),
                true
            )
            .is_empty()
        );
        assert_eq!(
            fault(&["ls", "invented/x.rs"], &workspace(), here.root(), true).len(),
            1
        );
    }

    #[test]
    fn a_command_that_is_only_a_path_is_still_checked() {
        // `./build-and-test.sh` on its own line: one word, no arguments, and
        // for a while nothing looked at it at all.
        let here = Fixture::new("bare-path", &["build-and-test.sh"]);
        assert!(fault(&["./build-and-test.sh"], &workspace(), here.root(), true).is_empty());
        assert_eq!(
            fault(&["./gate.sh"], &workspace(), here.root(), true).len(),
            1
        );
        // A program on PATH is not a path in this repository.
        assert!(fault(&["cargo", "test"], &workspace(), here.root(), true).is_empty());
        assert!(fault(&["make"], &workspace(), here.root(), true).is_empty());
    }

    #[test]
    fn an_awk_program_is_not_a_path() {
        // `rustc -vV | awk '/^host:/{print $2}'` is how two repositories name
        // the host triple, and the slash in it is not a directory here.
        let here = Fixture::new("awk", &["README.md"]);
        let found = fault(
            &["rustc", "-vV", "|", "awk", "'/^host:/{print", "$2}'"],
            &workspace(),
            here.root(),
            true,
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_url_is_not_a_path_in_this_repository() {
        let here = Fixture::new("urls", &["README.md"]);
        let found = fault(
            &[
                "git",
                "clone",
                "https://github.com/XPUI-Framework/xpui-gallery",
            ],
            &workspace(),
            here.root(),
            true,
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_build_output_is_not_required_to_exist() {
        // `open target/diff/` is what a guide says to do *after* a failing
        // run. Requiring it would fail the gate on a clean clone.
        let here = Fixture::new("build-output", &["README.md"]);
        assert!(fault(&["open", "target/diff/"], &workspace(), here.root(), true).is_empty());
    }

    #[test]
    fn a_bad_path_is_reported_once_and_not_twice() {
        // `open` claims its argument; the bare-path rule must not report the
        // same word again.
        let here = Fixture::new("once", &["README.md"]);
        let found = fault(&["open", "nowhere/x/"], &workspace(), here.root(), true);
        assert_eq!(found.len(), 1, "{found:?}");
    }

    #[test]
    fn a_skipped_verb_still_has_its_paths_read() {
        // `elf2uf2-rs -d examples/rp2040/target/…` was wrong inside a flash
        // command. A verb list that silenced the whole line kept it wrong.
        let here = Fixture::new("skipped-verb", &["README.md"]);
        let found = fault(
            &["elf2uf2-rs", "-d", "examples/rp2040/app.uf2"],
            &workspace(),
            here.root(),
            false,
        );
        assert_eq!(found.len(), 1, "{found:?}");
        // ...but its `-p` means something else and is not compared.
        assert!(
            fault(
                &["pio", "run", "-p", "nowhere"],
                &workspace(),
                here.root(),
                false
            )
            .is_empty()
        );
    }

    #[test]
    fn only_the_named_flags_exempt_their_value() {
        let here = Fixture::new("flag-values", &["README.md"]);
        // `--features xpui/testing` is a feature.
        assert!(
            fault(
                &["cargo", "test", "--features", "xpui/testing"],
                &workspace(),
                here.root(),
                true
            )
            .is_empty()
        );
        // `-l` is not on the list, so its neighbour is read like any word.
        let found = fault(&["ls", "-l", "invented/x"], &workspace(), here.root(), true);
        assert_eq!(found.len(), 1, "{found:?}");
    }
}
