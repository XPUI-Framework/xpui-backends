//! Whether a README carries the headings the standard names, in order, and no
//! others.

use std::fs;

use crate::fences::fences;
use crate::paths::tracked;

/// Every README carries exactly the headings in `order` — the root's list at
/// the root, `nested` below it — with the optional ones present or absent
/// but never out of place.
///
/// "No others" is what keeps a front page lean: a section that is not on the
/// list has a home in `docs/`. An empty `order` prints `not adopted`.
pub fn readme_sections(
    order: &[&str],
    optional: &[&str],
    nested: &[&str],
    nested_optional: &[&str],
    not_a_front_page: &[&str],
) -> Result<String, String> {
    if order.is_empty() {
        return Ok("not adopted".into());
    }
    let mut problems = Vec::new();
    let mut checked = 0;
    let mut used = Vec::new();
    for file in tracked("*README.md") {
        let path = file.to_string_lossy().to_string();
        if let Some(name) = not_a_front_page.iter().find(|e| **e == path) {
            used.push(*name);
            continue;
        }
        checked += 1;
        let at_root = file.parent().is_some_and(|p| p.as_os_str().is_empty());
        let (want, may) = if at_root {
            (order, optional)
        } else {
            (nested, nested_optional)
        };
        let text = fs::read_to_string(&file).unwrap_or_default();
        for why in faults(&headings(&text), want, may) {
            problems.push(format!("  {path}: {why}"));
        }
    }
    for name in not_a_front_page {
        if !used.contains(name) {
            problems.push(format!(
                "  the exemption names {name}, which is not a README here"
            ));
        }
    }
    if checked == 0 {
        return Err("no README found — the filter is matching nothing".into());
    }
    if problems.is_empty() {
        Ok(format!("{checked} README(s) in order"))
    } else {
        Err(format!(
            "{}\n\nA README is for arrival. What is not on the list moves to docs/.",
            problems.join("\n")
        ))
    }
}

/// The `##` headings of a page, outside fences, in order.
fn headings(text: &str) -> Vec<String> {
    let inside: std::collections::BTreeSet<usize> = fences(text)
        .iter()
        .flat_map(|f| f.lines.iter().map(|(n, _)| *n))
        .collect();
    text.lines()
        .enumerate()
        .filter(|(i, _)| !inside.contains(&(i + 1)))
        .filter_map(|(_, l)| l.strip_prefix("## ").map(|h| h.trim().to_string()))
        .collect()
}

/// What is wrong with one page's headings against `want`.
fn faults(found: &[String], want: &[&str], optional: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for h in found {
        if !want.contains(&h.as_str()) {
            out.push(format!("`## {h}` is not on the list"));
        }
    }
    for (i, h) in found.iter().enumerate() {
        if found[..i].contains(h) {
            out.push(format!("`## {h}` appears twice"));
        }
    }
    for w in want {
        if !optional.contains(w) && !found.iter().any(|h| h == w) {
            out.push(format!("`## {w}` is missing"));
        }
    }
    // Order: the found headings that are on the list must be a subsequence
    // of `want`.
    let mut cursor = 0;
    for h in found.iter().filter(|h| want.contains(&h.as_str())) {
        match want[cursor..].iter().position(|w| w == h) {
            Some(at) => cursor += at + 1,
            None => out.push(format!("`## {h}` is out of order")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDER: [&str; 4] = ["Using it", "Requirements", "Checking it", "License"];
    const OPTIONAL: [&str; 1] = ["Requirements"];

    fn check(page: &str) -> Vec<String> {
        faults(&headings(page), &ORDER, &OPTIONAL)
    }

    #[test]
    fn the_canonical_order_passes_with_or_without_the_optional() {
        assert!(check("## Using it\n## Requirements\n## Checking it\n## License\n").is_empty());
        assert!(check("## Using it\n## Checking it\n## License\n").is_empty());
    }

    #[test]
    fn a_heading_not_on_the_list_is_named() {
        let why = check("## Using it\n## Why a macro\n## Checking it\n## License\n");
        assert_eq!(why, ["`## Why a macro` is not on the list"]);
    }

    #[test]
    fn a_duplicate_is_named_once_and_a_missing_one_too() {
        let why = check("## Using it\n## Using it\n## License\n");
        assert!(why.contains(&"`## Using it` appears twice".to_string()));
        assert!(why.contains(&"`## Checking it` is missing".to_string()));
    }

    #[test]
    fn out_of_order_is_named() {
        let why = check("## Checking it\n## Using it\n## License\n");
        assert_eq!(why, ["`## Using it` is out of order"]);
    }

    #[test]
    fn a_heading_inside_a_fence_is_not_a_heading() {
        let page = "## Using it\n```text\n## not a heading\n```\n## Checking it\n## License\n";
        assert!(check(page).is_empty());
    }
}
