//! What git says is here, and whether a path really is.
//!
//! The primitives every other check is built on. Its neighbour `tree.rs` holds
//! the rules about the tree that these make it possible to state.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Everything git tracks, plus everything it would track, as repository-root
/// relative paths.
///
/// `--others --exclude-standard` so a document written but not yet committed
/// is checked. That is when a link is wrong and when somebody can still fix it
/// cheaply.
pub fn tracked(pattern: &str) -> Vec<PathBuf> {
    let out = Command::new("git")
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            pattern,
        ])
        .output()
        .expect("git ls-files");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(PathBuf::from)
        // A symlink is the same file under another name, and its relative
        // links resolve from where the real one lives.
        .filter(|p| {
            p.symlink_metadata()
                .is_ok_and(|m| !m.file_type().is_symlink())
        })
        .filter(|p| p.exists())
        .collect()
}

/// Whether `path` exists, **case included**.
///
/// `Path::exists` cannot answer this on the machine this usually runs on:
/// APFS is case-insensitive, so `docs/ROADMAP.md` is "there" locally and
/// broken everywhere else. A check that passes only where it cannot tell is
/// worse than none, so every component is matched against its own directory's
/// real entries.
pub fn exists_exactly(path: &Path) -> bool {
    let mut here = PathBuf::new();
    for part in path.components() {
        use std::path::Component;
        match part {
            Component::CurDir => continue,
            Component::ParentDir => {
                // Resolved textually, like the compiler does: popping past the
                // root leaves the repository root, which is where we started.
                here.pop();
                continue;
            }
            Component::Normal(name) => {
                let parent = if here.as_os_str().is_empty() {
                    PathBuf::from(".")
                } else {
                    here.clone()
                };
                let Ok(entries) = fs::read_dir(&parent) else {
                    return false;
                };
                if !entries
                    .filter_map(Result::ok)
                    .any(|e| e.file_name() == name)
                {
                    return false;
                }
                here.push(name);
            }
            // A root or a Windows prefix is not a name to match; keep it and
            // carry on, so an absolute path is checked component by component
            // like any other.
            root @ (Component::RootDir | Component::Prefix(_)) => here.push(root),
        }
    }
    true
}

/// Whether a path lies under some crate's `src/`, including a crate that sits
/// at the repository root and so has no directory in front of it.
pub fn under_src(path: &Path) -> bool {
    let text = path.to_string_lossy();
    (text.starts_with("src/") || text.contains("/src/")) && !text.starts_with("target/")
}

/// Runs `f` with `root` as the working directory.
///
/// The working directory is process-wide and tests run in parallel, so every
/// test that changes it takes this one lock — one per module is no lock. The
/// directory to return to is captured once, before any test has moved: a
/// path saved later can be a scratch tree another test deletes.
#[cfg(test)]
pub fn in_directory<T>(root: &Path, f: impl FnOnce() -> T) -> T {
    use std::sync::{Mutex, OnceLock};
    static HELD: Mutex<()> = Mutex::new(());
    static HOME: OnceLock<PathBuf> = OnceLock::new();

    let home = HOME.get_or_init(|| std::env::current_dir().expect("a working directory"));
    let _lock = HELD.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_current_dir(root).expect("the scratch tree");
    let out = f();
    std::env::set_current_dir(home).expect("back where we started");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crate_at_the_repository_root_is_under_src_too() {
        // `/src/` alone matches neither of the first two.
        assert!(under_src(Path::new("src/lib.rs")));
        assert!(under_src(Path::new("src/paint/controls.rs")));
        assert!(under_src(Path::new("xtask/src/main.rs")));
        assert!(!under_src(Path::new("tests/paint.rs")));
        assert!(!under_src(Path::new("target/debug/build/x/src/y.rs")));
    }

    #[test]
    fn a_source_root_prefix_matches_a_crate_at_the_repository_root() {
        // `Path::new(".").join("src")` is `./src`, whose components are
        // [CurDir, "src"], and those never prefix `src/lib.rs`.
        assert!(Path::new("src/lib.rs").starts_with(Path::new("").join("src")));
        assert!(Path::new("core/src/lib.rs").starts_with(Path::new("core").join("src")));
        assert!(!Path::new("src/lib.rs").starts_with(Path::new("core").join("src")));
    }

    #[test]
    fn a_path_is_matched_case_exactly() {
        // APFS is case-insensitive, so `Path::exists` cannot answer this and a
        // link that works here breaks everywhere else.
        let root = std::env::temp_dir().join(format!("xpui-xtask-case-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).expect("scratch");
        fs::write(root.join("README.md"), "").expect("scratch");
        assert!(exists_exactly(&root.join("README.md")));
        assert!(!exists_exactly(&root.join("readme.md")));
        assert!(exists_exactly(&root.join("src/../README.md")));
        let _ = fs::remove_dir_all(&root);
    }
}
