//! The reference pages say what docs.rs says: every public name has a section,
//! and each section's abstract and declaration are rustdoc's own.
//!
//! A hand-written page drifts from the code the day after it is written. This
//! is what notices. `rustdoc.rs` reads what the pages agree with, and
//! `pages.rs` reads the pages.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::pages::{self, Section, prose, signature};
use crate::paths::tracked;
use crate::rustdoc::{self, Kind, Symbol};

/// Where a repository's reference lives, and how far it has got.
pub struct Reference {
    /// Crate names under `target/doc`, or a path to a crate's output elsewhere:
    /// `"target/thumbv6m-none-eabi/doc/xpui_rp2040"`.
    pub crates: &'static [&'static str],
    /// The pages, as a `git ls-files` pattern.
    pub pages: &'static str,
    /// While `false`, a name with no section is counted, not failed.
    pub complete: bool,
    /// `(name, reason)` for a public name no page carries.
    pub exempt: &'static [(&'static str, &'static str)],
}

/// Every public name has a section, and every section agrees with rustdoc.
///
/// Reads what `rustdoc links resolve` wrote. Output that is missing, or older
/// than the source, is a failure: a stale answer is a check passing without
/// looking.
pub fn mirrors_rustdoc(reference: Option<&Reference>) -> Result<String, String> {
    let Some(reference) = reference else {
        return Ok("not adopted".into());
    };
    let mut symbols = Vec::new();
    for entry in reference.crates {
        let doc = if entry.contains('/') {
            PathBuf::from(entry)
        } else {
            Path::new("target/doc").join(entry)
        };
        let root = fresh(&doc)?;
        let prefix = match reference.crates.len() {
            1 => String::new(),
            _ => format!(
                "{}::",
                doc.file_name().unwrap_or_default().to_string_lossy()
            ),
        };
        symbols.extend(rustdoc::symbols(&doc, &prefix, &root)?);
    }
    if !symbols.iter().any(|s| s.kind == Kind::Item) {
        return Err("rustdoc's output names no public item: its HTML has changed shape".into());
    }
    let pages = tracked(reference.pages);
    let mut sections = Vec::new();
    for page in &pages {
        let text = fs::read_to_string(page).unwrap_or_default();
        sections.extend(pages::sections_in(&text, &page.display().to_string()));
    }

    let tally = compare(&symbols, &sections, reference.exempt);
    let wanted = tally.placed + tally.missing.len();
    let mut faults = tally.faults;
    if reference.complete {
        faults.extend(tally.missing);
    }
    if !faults.is_empty() {
        return Err(format!(
            "{}\n{} fault(s). A section's abstract is its item's first `///` paragraph and\n\
             its declaration is the signature, both word for word.",
            faults.join("\n"),
            faults.len()
        ));
    }
    let across = format!("across {} page(s)", pages.len());
    Ok(if reference.complete {
        format!("{wanted} names; every section, abstract and declaration matches {across}")
    } else {
        format!(
            "partial: {} of {wanted} placed; each placed one matches {across}",
            tally.placed
        )
    })
}

/// The output exists and is newer than every source file of its crate, whose
/// source root this returns.
fn fresh(doc: &Path) -> Result<PathBuf, String> {
    let all = doc.join("all.html");
    let written = fs::metadata(&all)
        .and_then(|m| m.modified())
        .map_err(|_| format!("{} is missing: `cargo doc` has not run", all.display()))?;
    let krate = doc.file_name().unwrap_or_default().to_string_lossy();
    let root = source_root(&krate)
        .ok_or_else(|| format!("no Cargo.toml in this repository builds `{krate}`"))?;
    let newer = |p: &PathBuf| {
        fs::metadata(p)
            .and_then(|m| m.modified())
            .is_ok_and(|t| t > written)
    };
    match tracked("*.rs")
        .iter()
        .find(|p| p.starts_with(&root) && newer(p))
    {
        Some(p) => Err(format!(
            "{} is newer than {}: run `cargo doc`",
            p.display(),
            all.display()
        )),
        None => Ok(root),
    }
}

/// The directory holding a crate's root file, which rustdoc's source paths
/// are relative to.
pub fn source_root(krate: &str) -> Option<PathBuf> {
    for manifest in tracked("*Cargo.toml") {
        let text = fs::read_to_string(&manifest).unwrap_or_default();
        let (mut section, mut package, mut lib, mut path) = ("", None, None, None);
        for line in text.lines().map(str::trim) {
            if line.starts_with('[') {
                section = line;
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.split('"').nth(1).unwrap_or(value.trim()).to_string();
            match (section, key.trim()) {
                ("[package]", "name") => package = Some(value.replace('-', "_")),
                ("[lib]", "name") => lib = Some(value),
                ("[lib]", "path") => path = Some(value),
                _ => {}
            }
        }
        if lib.or(package).as_deref() == Some(krate) {
            let dir = manifest.parent().unwrap_or(Path::new(""));
            let file = dir.join(path.as_deref().unwrap_or("src/lib.rs"));
            return file.parent().map(Path::to_path_buf);
        }
    }
    None
}

struct Tally {
    faults: Vec<String>,
    missing: Vec<String>,
    placed: usize,
}

fn compare(symbols: &[Symbol], sections: &[Section], exempt: &[(&str, &str)]) -> Tally {
    let mut by_name: BTreeMap<&str, Vec<&Section>> = BTreeMap::new();
    for section in sections {
        by_name.entry(&section.name).or_default().push(section);
    }
    let mut tally = Tally {
        faults: Vec::new(),
        missing: Vec::new(),
        placed: 0,
    };
    for (name, found) in &by_name {
        if found.len() > 1 {
            let places: Vec<&str> = found.iter().map(|s| s.at.as_str()).collect();
            tally
                .faults
                .push(format!("  twice        `{name}`  {}", places.join(", ")));
        }
        if !symbols.iter().any(|s| s.name == *name) {
            tally.faults.push(format!(
                "  unknown      {}  `{name}` names no public item",
                found[0].at
            ));
        }
    }
    for symbol in symbols {
        let exempted = exempt.iter().any(|(n, _)| *n == symbol.name);
        let Some(section) = by_name.get(symbol.name.as_str()).map(|f| f[0]) else {
            if !exempted {
                tally.missing.push(format!(
                    "  missing      {}  {}  no section",
                    symbol.name, symbol.at
                ));
            }
            continue;
        };
        tally.placed += 1;
        if exempted {
            tally.faults.push(format!(
                "  stale        exemption `{}`: {} carries it",
                symbol.name, section.at
            ));
        }
        if symbol.kind == Kind::Foreign {
            continue;
        }
        let (theirs, ours) = (
            prose(&symbol.summary),
            prose(section.summary.as_deref().unwrap_or("")),
        );
        if theirs != ours {
            tally.faults.push(differs(
                "abstract   ",
                section,
                symbol,
                "rustdoc",
                &theirs,
                &ours,
            ));
        }
        if symbol.kind != Kind::Row && !section.row {
            let (theirs, ours) = (
                signature(&symbol.declaration, &symbol.name),
                signature(section.declaration.as_deref().unwrap_or(""), &symbol.name),
            );
            if theirs != ours {
                tally.faults.push(differs(
                    "declaration",
                    section,
                    symbol,
                    "source ",
                    &theirs,
                    &ours,
                ));
            }
        }
    }
    for (name, _) in exempt {
        if !symbols.iter().any(|s| s.name == *name) {
            tally.faults.push(format!(
                "  stale        exemption `{name}` names nothing public"
            ));
        }
    }
    tally
}

fn differs(
    what: &str,
    section: &Section,
    symbol: &Symbol,
    from: &str,
    theirs: &str,
    ours: &str,
) -> String {
    let shown = |t: &str| {
        if t.is_empty() {
            "(none)".to_string()
        } else {
            t.to_string()
        }
    };
    format!(
        "  {what}  {}  `{}`\n                 {from}: {}\n                 page:    {}",
        section.at,
        symbol.name,
        shown(theirs),
        shown(ours)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::{PAGE, sections_in};

    fn symbol(name: &str, kind: Kind, summary: &str, declaration: &str) -> Symbol {
        let (name, summary, declaration) = (name.into(), summary.into(), declaration.into());
        Symbol {
            name,
            kind,
            summary,
            declaration,
            at: "src/x.rs:1".into(),
        }
    }

    fn symbols(push: &str) -> Vec<Symbol> {
        vec![
            symbol(
                "List",
                Kind::Item,
                "A themed list, filling the space it is given.",
                "pub struct List<M>",
            ),
            symbol(
                "List::push",
                Kind::Member,
                push,
                "pub fn push( mut self, row: ListRow<M>, ) -> Self",
            ),
            symbol("Hint::Standard", Kind::Row, "The host's label.", ""),
        ]
    }

    #[test]
    fn a_page_that_agrees_with_rustdoc_has_no_faults() {
        let tally = compare(&symbols("Adds a row."), &sections_in(PAGE, "lists.md"), &[]);
        assert!(tally.faults.is_empty(), "{:?}", tally.faults);
        assert!(tally.missing.is_empty());
        assert_eq!(tally.placed, 3);
    }

    #[test]
    fn one_word_changed_in_a_summary_is_an_abstract_fault() {
        let tally = compare(
            &symbols("Adds a row at the end."),
            &sections_in(PAGE, "lists.md"),
            &[],
        );
        assert_eq!(tally.faults.len(), 1, "{:?}", tally.faults);
        assert!(tally.faults[0].contains("abstract") && tally.faults[0].contains("List::push"));
    }

    #[test]
    fn a_deleted_section_is_missing_and_a_changed_signature_is_a_fault() {
        let without = PAGE.replace("#### `List::push`", "#### Push");
        let tally = compare(
            &symbols("Adds a row."),
            &sections_in(&without, "lists.md"),
            &[],
        );
        assert!(
            tally.missing.iter().any(|m| m.contains("List::push")),
            "{:?}",
            tally.missing
        );
        let changed = PAGE.replace("pub struct List<M>", "pub struct List");
        let tally = compare(
            &symbols("Adds a row."),
            &sections_in(&changed, "lists.md"),
            &[],
        );
        assert!(
            tally.faults.iter().any(|f| f.contains("declaration")),
            "{:?}",
            tally.faults
        );
    }

    #[test]
    fn a_section_twice_or_for_nothing_and_a_stale_exemption_are_faults() {
        let page = format!("{PAGE}\n## `List`\n\nA themed list.\n\n## `Gone`\n\nGone.\n");
        let tally = compare(
            &symbols("Adds a row."),
            &sections_in(&page, "lists.md"),
            &[("Nothing", "why")],
        );
        let all = tally.faults.join("\n");
        assert!(all.contains("twice        `List`"), "{all}");
        assert!(all.contains("`Gone` names no public item"), "{all}");
        assert!(
            all.contains("exemption `Nothing` names nothing public"),
            "{all}"
        );
    }

    #[test]
    fn a_repository_that_has_not_adopted_it_says_so() {
        assert_eq!(mirrors_rustdoc(None), Ok("not adopted".to_string()));
    }
}
