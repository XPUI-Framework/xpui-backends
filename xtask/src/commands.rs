//! Whether every command a document gives could run here.
//!
//! Not executed — that would flash boards and install packages. What is
//! checked is what goes stale: a `-p` naming a package that is not here, and
//! a path that is not in this repository.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::faults::fault;
use crate::fences::{Fence, commands, fences, words};
use crate::paths::{exists_exactly, tracked};

/// Info strings that mean "this block is shell".
const SHELL: [&str; 4] = ["bash", "sh", "shell", "console"];

/// Verbs whose semantics cannot be checked from here: a flasher, a package
/// manager, another repository's build system. Their paths are still read.
/// The count is reported so the list cannot quietly grow.
const NOT_OURS: &[&str] = &[
    "pio",
    "platformio",
    "elf2uf2-rs",
    "probe-rs",
    "espflash",
    "esptool",
    "espup",
    "brew",
    "apt",
    "apt-get",
    "rustup",
    "curl",
    "wget",
    "gh",
    "ssh",
    "scp",
    "docker",
    "pipx",
    "code",
    "python3",
];

/// Every documented command could run here.
pub fn resolve(packages: &BTreeSet<String>, exempt: &[&str]) -> Result<String, String> {
    // Resolved from the repository root, wherever the document sits — unlike
    // a markdown link. A `cargo` command in a nested README is one you run
    // where the workspace is.
    let root = Path::new(".");
    let mut problems = Vec::new();
    let (mut checked, mut skipped, mut left) = (0, 0, 0);
    let (mut blocks, mut left_blocks) = (0, 0);

    for doc in tracked("*.md") {
        let path = doc.to_string_lossy().to_string();
        if exempt.iter().any(|e| path.starts_with(e)) {
            continue;
        }
        let text = fs::read_to_string(&doc).unwrap_or_default();

        for fence in fences(&text)
            .iter()
            .filter(|f| SHELL.contains(&f.language.as_str()))
        {
            blocks += 1;
            let (faults, ran, ignored, away) = block_faults_counted(fence, packages, root);
            checked += ran;
            skipped += ignored;
            if away > 0 {
                left_blocks += 1;
            }
            left += away;
            problems.extend(
                faults
                    .into_iter()
                    .map(|(line, why)| format!("  {path}:{line}  {why}")),
            );
        }
    }

    if problems.is_empty() {
        // Every number, because each is a way of checking nothing: a guide
        // that `cd`s away entirely means this read nothing at all.
        Ok(format!(
            "{checked} command(s) in {blocks} block(s); {skipped} another tool's, \
             {left} in another repository across {left_blocks} block(s)"
        ))
    } else {
        Err(format!(
            "{}\n\nA command belonging to another repository is a link to that\n\
             repository, not a command printed here that fails.",
            problems.join("\n")
        ))
    }
}

/// Everything wrong in one fenced block with its line, plus how many commands
/// were read, were another tool's, and belonged to a repository this block
/// had stepped into. Testable against a scratch tree.
fn block_faults_counted(
    fence: &Fence,
    packages: &BTreeSet<String>,
    root: &Path,
) -> (Vec<(usize, String)>, usize, usize, usize) {
    let mut out = Vec::new();
    let (mut checked, mut skipped, mut left) = (0, 0, 0);
    // A `cd` out of the tree ends this block, not the page.
    let mut elsewhere = false;
    for command in commands(fence) {
        let parts = words(&command.text);
        let Some(verb) = parts.first() else { continue };
        if *verb == "cd" {
            // Anywhere that is not a directory here — absolute, above the
            // root, or a sibling checkout — is another repository's commands,
            // not ours to resolve. Latched: after `cd xpui-gallery`, a later
            // `cd docs` is *that* tree's `docs`. `..` is spelled out because
            // `exists_exactly` resolves a parent step textually and answers
            // "yes, that is here".
            elsewhere |= parts.get(1).is_none_or(|d| {
                let d = d.trim_end_matches('/');
                d == ".."
                    || d.starts_with("../")
                    || d.starts_with('/')
                    || d.starts_with('~')
                    || !exists_exactly(&root.join(d))
            });
            continue;
        }
        if elsewhere {
            left += 1;
            continue;
        }
        let ours = !NOT_OURS.contains(verb);
        if ours {
            checked += 1;
        } else {
            skipped += 1;
        }
        for why in fault(&parts, packages, root, ours) {
            out.push((command.line, why));
        }
    }
    (out, checked, skipped, left)
}

/// The faults alone. Only the tests want this shape.
#[cfg(test)]
fn block_faults(fence: &Fence, packages: &BTreeSet<String>, root: &Path) -> Vec<(usize, String)> {
    block_faults_counted(fence, packages, root).0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fences::fences;

    /// A repository-shaped scratch directory, so these tests say the same
    /// thing in every repository that carries a copy of this file.
    struct Fixture(std::path::PathBuf);

    impl Fixture {
        fn new(name: &str, entries: &[&str]) -> Self {
            let root =
                std::env::temp_dir().join(format!("xpui-walk-{name}-{}", std::process::id()));
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
    fn a_step_above_the_root_leaves_the_repository() {
        // `exists_exactly` resolves `..` textually and answers "yes, that is
        // here", so the parent step has to be spelled out.
        let here = Fixture::new("cd-up", &["docs/"]);
        let up = fences("```bash\ncd ..\ncargo run -p nowhere\n```\n");
        assert!(block_faults(&up[0], &workspace(), here.root()).is_empty());
    }
    #[test]
    fn leaving_the_repository_latches_for_the_rest_of_the_block() {
        // After `cd xpui-gallery`, a later `cd docs` is *that* tree's docs.
        let here = Fixture::new("latch", &["docs/"]);
        let block = fences("```bash\ncd xpui-gallery\ncd docs\ncargo run -p nowhere\n```\n");
        assert!(block_faults(&block[0], &workspace(), here.root()).is_empty());
    }
    #[test]
    fn a_cd_into_a_sibling_checkout_ends_the_block() {
        // `cd` into a directory this repository does not have is how the check
        // knows the commands after it are another repository's.
        let here = Fixture::new("cd-away", &["docs/"]);
        // `cd docs` stays inside, so what follows is still checked.
        let inside = fences("```bash\ncd docs\ncargo run -p nowhere\n```\n");
        assert_eq!(block_faults(&inside[0], &workspace(), here.root()).len(), 1);
        // `cd xpui-gallery` leaves, so what follows is not ours.
        let away = fences("```bash\ncd xpui-gallery\ncargo run -p xpui-gallery\n```\n");
        assert!(block_faults(&away[0], &workspace(), here.root()).is_empty());
        // ...and a package that is wrong *before* the cd is still reported.
        let mixed = fences(
            "```bash\ncargo run -p nowhere\ncd xpui-gallery\ncargo run -p also-nowhere\n```\n",
        );
        assert_eq!(block_faults(&mixed[0], &workspace(), here.root()).len(), 1);
    }
    #[test]
    fn a_console_or_shell_block_is_shell() {
        for language in ["bash", "sh", "shell", "console"] {
            assert!(
                SHELL.contains(&language),
                "a {language} block would not be read at all"
            );
        }
    }
}
