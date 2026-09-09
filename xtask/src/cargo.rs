//! Running cargo, and knowing which targets to run it for.

use std::collections::BTreeSet;
use std::process::Command;

/// Runs a cargo subcommand, streaming its output, and says whether it passed.
pub fn cargo(arguments: &[&str]) -> Result<String, String> {
    let status = Command::new(env!("CARGO"))
        .args(arguments)
        .status()
        .map_err(|e| format!("could not run cargo: {e}"))?;
    if status.success() {
        Ok(format!("cargo {}", arguments.join(" ")))
    } else {
        Err(format!("cargo {} failed", arguments.join(" ")))
    }
}

/// Every package in this repository, by name.
///
/// From the manifests rather than `cargo metadata`: metadata gives every
/// *target* a `"name"` too, and a repository may hold a crate its root
/// workspace excludes — `docs-test/` — that a document may still name.
pub fn packages() -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for manifest in crate::paths::tracked("*Cargo.toml") {
        let text = std::fs::read_to_string(&manifest).unwrap_or_default();
        let mut in_package = false;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_package = line == "[package]";
                continue;
            }
            if in_package && let Some(value) = line.strip_prefix("name") {
                if let Some(name) = value
                    .trim_start_matches([' ', '='])
                    .trim()
                    .strip_prefix('"')
                    && let Some(name) = name.split('"').next()
                {
                    names.insert(name.to_string());
                }
                break;
            }
        }
    }
    names
}

/// Whether a rustup target is installed.
///
/// A bare-metal lint is the only gate that reaches code behind
/// `cfg(target_os = "none")`. Where the target is absent the check says so
/// rather than passing quietly.
pub fn target_installed(triple: &str) -> bool {
    Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .any(|l| l.trim() == triple)
        })
        .unwrap_or(false)
}

/// Rustdoc over the workspace, with warnings as errors.
///
/// Not `doc_paths`, which reads disk: this asks rustdoc whether every
/// intra-doc link resolves to an item public enough to link to.
pub fn rustdoc(arguments: &[&str]) -> Result<String, String> {
    let mut all = vec!["doc", "--no-deps"];
    all.extend_from_slice(arguments);
    let status = Command::new(env!("CARGO"))
        .env("RUSTDOCFLAGS", "-D warnings")
        .args(&all)
        .status()
        .map_err(|e| format!("could not run cargo: {e}"))?;
    if status.success() {
        Ok(format!("cargo {}", all.join(" ")))
    } else {
        Err(format!(
            "cargo {} failed. An intra-doc link that does not resolve is a\n\
             link nothing else in the build reads.",
            all.join(" ")
        ))
    }
}
