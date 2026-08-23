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
/// Read out of the manifests rather than `cargo metadata`, for two reasons.
/// One: metadata's JSON gives every *target* a `"name"` too, so a scan for the
/// key accepted an integration test's filename as a package and let a wrong
/// `-p` through. Two: a repository may hold a crate its root workspace
/// excludes — `docs-test/` is one — and a document naming that crate is
/// naming something that is genuinely here.
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
/// `cfg(target_os = "none")` — no allocator, no panic handler. Where the
/// target is absent the check says so out loud rather than passing quietly,
/// because a gate that skips silently is a gate that is green on a laptop and
/// wrong everywhere it matters.
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
/// This is the check `doc_paths` is *not*. A path check reads what is on
/// disk; this asks rustdoc whether every intra-doc link — `[`Screen`]`,
/// `[`Canvas::draw_text`]` — resolves to an item that exists and is public
/// enough to link to. Six broken ones shipped here before it existed: two
/// pointing at items behind a `cfg`, three at private implementation details,
/// one at a private module.
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
