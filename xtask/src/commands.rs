//! Whether every command a document gives could run here.
//!
//! Not executed — that would flash boards and install packages. What is
//! checked is the part that goes stale: a `-p` naming a package that is not
//! here, and a path that is not in this repository. Nine broken commands were
//! shipped across the organisation before anything read a ```bash block.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::faults::fault;
use crate::fences::{Fence, commands, fences, words};
use crate::paths::{exists_exactly, tracked};

/// Verbs whose *semantics* cannot be checked from here: a flasher, a package
/// manager, another repository's build system.
///
/// Only the semantics. Their **paths are still read**, because
/// `elf2uf2-rs -d examples/rp2040/target/…` was wrong inside exactly such a
/// command, and a verb list that silenced the whole line would have kept it
/// wrong. The count is reported so the list cannot quietly grow.
/// Info strings that mean "this block is shell".
const SHELL: [&str; 4] = ["bash", "sh", "shell", "console"];

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
    // Every command is resolved from the **repository root**, wherever the
    // document sits. A markdown link resolves relative to the page that holds
    // it, and this deliberately does not: a `cargo` command in a nested
    // README is one you run where the workspace is, and `open
    // gallery/tests/screenshots/` in `gallery/README.md` means exactly what it
    // says from the root and nothing sensible from beside itself.
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
        // Every number, because each is a way of checking nothing. A page
        // that `cd`s into a sibling checkout and then runs its commands is
        // documenting that repository correctly — but a whole guide doing it
        // means this check read nothing at all, and that should be visible
        // rather than reported as a pass.
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

/// Everything wrong in one fenced block, with the line each fault is on, plus
/// how many commands were read, how many were another tool's, and how many
/// belonged to a repository this block had stepped into.
///
/// Separate from the walk over documents so it can be tested against a scratch
/// tree rather than against whichever repository carries this copy.
fn block_faults_counted(
    fence: &Fence,
    packages: &BTreeSet<String>,
    root: &Path,
) -> (Vec<(usize, String)>, usize, usize, usize) {
    let mut out = Vec::new();
    let (mut checked, mut skipped, mut left) = (0, 0, 0);
    // A `cd` out of the tree ends *this block*, not the page. One block that
    // shows an SDK being cloned elsewhere must not disable the checking of
    // every block after it.
    let mut elsewhere = false;
    for command in commands(fence) {
        let parts = words(&command.text);
        let Some(verb) = parts.first() else { continue };
        if *verb == "cd" {
            // Anywhere that is not a directory of this repository — an
            // absolute path, a step above the root, or a sibling checkout. A
            // guide that says `git clone …` and then `cd` into what it cloned
            // is documenting another repository's commands, correctly, and
            // they are not ours to resolve.
            //
            // Latched, never cleared: once a block has stepped outside, a
            // later `cd docs` is *that* tree's `docs`, and resolving it here
            // would check a path against the wrong repository.
            //
            // `..` is spelled out because `exists_exactly` resolves a parent
            // step textually and would answer "yes, that is here" — the trap
            // its own doc comment names.
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
        // A guide that clones another repository and then runs its commands is
        // documenting that repository correctly. Those commands are not ours
        // to resolve, and `cd` into a directory this repository does not have
        // is how the check knows.
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
