//! `AGENTS.md`: that it exists, that `CLAUDE.md` is a symlink to it, and that
//! what it says the gate runs is what the gate runs.

use std::fs;
use std::path::Path;

use crate::fences::fences;

/// `AGENTS.md` is a file and `CLAUDE.md` is a relative symlink to it.
///
/// Read with `symlink_metadata`, because `paths::tracked` filters symlinks
/// out. Relative, single-component: an absolute link works on one machine.
pub fn agents_file_exists(required: bool) -> Result<String, String> {
    if !required {
        return Ok("not adopted".into());
    }
    if !Path::new("AGENTS.md").is_file() {
        return Err("  AGENTS.md is missing".into());
    }
    let link = Path::new("CLAUDE.md");
    let meta = fs::symlink_metadata(link).map_err(|_| "  CLAUDE.md is missing".to_string())?;
    if !meta.file_type().is_symlink() {
        return Err(
            "  CLAUDE.md is a file, not a symlink to AGENTS.md — a checkout with\n\
                    core.symlinks=false turns a link into a text file; re-link it"
                .into(),
        );
    }
    let target = fs::read_link(link).map_err(|e| format!("  CLAUDE.md: {e}"))?;
    if target != Path::new("AGENTS.md") {
        return Err(format!(
            "  CLAUDE.md points at {}, not AGENTS.md — the link must be relative",
            target.display()
        ));
    }
    Ok("AGENTS.md, and CLAUDE.md → AGENTS.md".into())
}

/// The stage list under `## The gate` in `AGENTS.md` is the list the gate is
/// running now.
///
/// `stages` is snapshotted after the gate vector is complete — every
/// `insert` and `extend` — and pushed last, owning the names: a closure in
/// the vector cannot borrow the vector. An empty list in `AGENTS.md` prints
/// `not adopted`.
pub fn agents_documents_the_gate(stages: &[String]) -> Result<String, String> {
    let text = fs::read_to_string("AGENTS.md").unwrap_or_default();
    let Some(at) = text.find("## The gate") else {
        return Ok("not adopted".into());
    };
    let section = &text[at..];
    let listed: Vec<String> = fences(section)
        .iter()
        .find(|f| f.language == "text")
        .map(|f| {
            f.lines
                .iter()
                .flat_map(|(_, l)| l.split('·'))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if listed.is_empty() {
        return Ok("not adopted".into());
    }
    // The list documents `check`; an `all` line adds its own stages after a
    // `+` marker and is compared only when they are running.
    let (base, extra): (Vec<&String>, Vec<&String>) =
        listed.iter().partition(|s| !s.starts_with('+'));
    let extra: Vec<String> = extra
        .iter()
        .map(|s| s.trim_start_matches('+').trim().to_string())
        .collect();
    let running: Vec<&str> = stages.iter().map(String::as_str).collect();
    let mut expected: Vec<&str> = base.iter().map(|s| s.as_str()).collect();
    if running.len() > expected.len() {
        expected.extend(extra.iter().map(String::as_str));
    }
    if running == expected {
        return Ok(format!("{} stages, as documented", running.len()));
    }
    Err(format!(
        "  AGENTS.md lists: {}\n  the gate runs:   {}\n\nCopy what ./build-and-test.sh prints; do not type it.",
        expected.join(" · "),
        running.join(" · ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::in_directory;

    fn scratch(name: &str, agents: Option<&str>, link: Option<&str>) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("xpui-agents-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("scratch");
        if let Some(text) = agents {
            fs::write(root.join("AGENTS.md"), text).expect("scratch");
        }
        if let Some(target) = link {
            std::os::unix::fs::symlink(target, root.join("CLAUDE.md")).expect("scratch");
        }
        root
    }

    #[test]
    fn a_relative_symlink_passes_and_a_copy_or_absolute_link_fails() {
        let ok = scratch("ok", Some("# x\n"), Some("AGENTS.md"));
        assert!(in_directory(&ok, || agents_file_exists(true)).is_ok());
        let abs = scratch("abs", Some("# x\n"), Some("/tmp/AGENTS.md"));
        assert!(in_directory(&abs, || agents_file_exists(true)).is_err());
        let copy = scratch("copy", Some("# x\n"), None);
        fs::write(copy.join("CLAUDE.md"), "# x\n").unwrap();
        assert!(in_directory(&copy, || agents_file_exists(true)).is_err());
        for r in [ok, abs, copy] {
            let _ = fs::remove_dir_all(r);
        }
    }

    #[test]
    fn the_documented_list_must_equal_the_running_list_in_order() {
        let page = "# x\n\n## The gate\n\n```text\nformat · lint · tests\n```\n";
        let root = scratch("stages", Some(page), None);
        let ok = ["format", "lint", "tests"].map(String::from);
        let reordered = ["lint", "format", "tests"].map(String::from);
        assert!(in_directory(&root, || agents_documents_the_gate(&ok)).is_ok());
        assert!(in_directory(&root, || agents_documents_the_gate(&reordered)).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn an_all_line_is_compared_only_when_all_is_running() {
        let page = "## The gate\n\n```text\nformat · lint\n+ links\n```\n";
        let root = scratch("all", Some(page), None);
        let check = ["format", "lint"].map(String::from);
        let all = ["format", "lint", "links"].map(String::from);
        assert!(in_directory(&root, || agents_documents_the_gate(&check)).is_ok());
        assert!(in_directory(&root, || agents_documents_the_gate(&all)).is_ok());
        let _ = fs::remove_dir_all(root);
    }
}
