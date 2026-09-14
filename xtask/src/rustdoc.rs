//! What rustdoc wrote: every public name in a crate, and the `///` and the
//! signature behind each one.
//!
//! Rustdoc has already settled what is public — re-exports, `cfg`,
//! `#[doc(hidden)]`, blanket impls — exactly as docs.rs will, so the names
//! come from its HTML. The words come from the source, which rustdoc links to.
//! Its neighbour `reference.rs` compares both with the pages.

use std::fs;
use std::path::Path;

use crate::pages::{between, declares, rendered, text_of};

/// How a page carries a name.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    /// A struct, enum, trait, function, constant, type alias, static or macro.
    Item,
    /// A method, an associated function or constant, or a trait method.
    Member,
    /// An enum variant or a public field, carried as a table row.
    Row,
    /// Defined in another crate: a row under `## Re-exports`, never compared.
    Foreign,
}

/// One public name, with the words and the signature a page repeats.
pub struct Symbol {
    pub name: String,
    pub kind: Kind,
    /// The first `///` paragraph, joined; empty when there is none.
    pub summary: String,
    /// From the first token to the body; empty for a row or a foreign item.
    pub declaration: String,
    /// `file:line`, for a message a reader can jump to.
    pub at: String,
}

/// Every public name rustdoc wrote into `doc`, a crate's output directory,
/// each with `prefix` in front. `root` holds the crate's root file.
pub fn symbols(doc: &Path, prefix: &str, root: &Path) -> Result<Vec<Symbol>, String> {
    let krate = doc.file_name().unwrap_or_default().to_string_lossy();
    let all = read(&doc.join("all.html"))?;
    let renamed = reexports(&read(&doc.join("index.html"))?);
    let mut out: Vec<Symbol> = Vec::new();
    for (section, href, text) in items(&all) {
        let page = read(&doc.join(&href))?;
        let Some((file, first, last)) = top_source(&page, &krate) else {
            // `pub use u8g2_fonts::fonts as u8g2` is 2,010 structs and one row.
            let name = format!("{prefix}{}", text.split("::").next().unwrap_or(&text));
            if !out.iter().any(|s| s.name == name) {
                out.push(bare(name, Kind::Foreign, href));
            }
            continue;
        };
        let shown = renamed
            .iter()
            .find(|(h, _)| *h == href)
            .map_or(text.as_str(), |(_, n)| n.as_str());
        let bang = if section == "macros" { "!" } else { "" };
        let owner = format!("{prefix}{shown}{bang}");
        let path = root.join(&file);
        let source = fs::read_to_string(&path).unwrap_or_default();
        let lines: Vec<&str> = source.lines().collect();
        let mut item = described(&owner, Kind::Item, &lines, first, &path);
        // A macro's item links to its invocation, a renamed one to its old name.
        if !declares(&item.declaration, &owner)
            && let Some((declaration, summary)) = rendered(&page)
        {
            (item.declaration, item.summary) = (declaration, summary);
        }
        out.push(item);

        for (tag, member, link, shown) in members(&page, &krate) {
            let name = format!("{owner}::{member}");
            if tag == "member" {
                let read = link.map(|(file, line)| {
                    let path = root.join(file);
                    let text = fs::read_to_string(&path).unwrap_or_default();
                    let lines: Vec<&str> = text.lines().collect();
                    described(&name, Kind::Member, &lines, line, &path)
                });
                out.push(match (read, shown) {
                    (Some(read), _) if declares(&read.declaration, &member) => read,
                    (_, Some(shown)) => Symbol {
                        name,
                        at: href.clone(),
                        ..shown
                    },
                    (read, None) => read.unwrap_or_else(|| bare(name, Kind::Member, href.clone())),
                });
            } else {
                let Some(line) = locate(&lines, first, last, &member, tag == "structfield") else {
                    out.push(bare(name, Kind::Row, href.clone()));
                    continue;
                };
                let mut row = described(&name, Kind::Row, &lines, line, &path);
                row.declaration.clear();
                out.push(row);
            }
        }
    }
    Ok(out)
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|_| format!("{} is missing", path.display()))
}

fn bare(name: String, kind: Kind, at: String) -> Symbol {
    Symbol {
        name,
        kind,
        summary: String::new(),
        declaration: String::new(),
        at,
    }
}

/// `(kind, href, text)` for every link under each `<h3 id="…">` of `all.html`.
fn items(all: &str) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for part in all.split("<h3 id=\"").skip(1) {
        let section = part.split('"').next().unwrap_or_default();
        for link in part.split("<li><a href=\"").skip(1) {
            let Some((href, rest)) = link.split_once("\">") else {
                continue;
            };
            let text = rest.split("</a>").next().unwrap_or_default();
            out.push((section.into(), href.into(), text.into()));
        }
    }
    out
}

/// `(href, name)` for every item the crate root re-exports.
fn reexports(index: &str) -> Vec<(String, String)> {
    index
        .split("<dt id=\"reexport.")
        .skip(1)
        .filter_map(|entry| {
            let name = entry.split('"').next()?;
            let code = entry.split("</dt>").next()?;
            let href = code.split(" href=\"").nth(1)?.split('"').next()?;
            Some((href.to_string(), name.to_string()))
        })
        .collect()
}

/// The item's own source link, when it is in this crate: file, first, last.
fn top_source(page: &str, krate: &str) -> Option<(String, usize, usize)> {
    let heading = page
        .split("class=\"sub-heading\"")
        .nth(1)?
        .split("</span>")
        .next()?;
    let href = heading
        .split("<a class=\"src\" href=\"")
        .nth(1)?
        .split('"')
        .next()?;
    link(href, krate)
}

fn link(href: &str, krate: &str) -> Option<(String, usize, usize)> {
    let (_, after) = href.split_once(&format!("src/{krate}/"))?;
    let (file, lines) = after.split_once(".html#")?;
    let (first, last) = lines.split_once('-').unwrap_or((lines, lines));
    Some((file.to_string(), first.parse().ok()?, last.parse().ok()?))
}

type Member = (
    &'static str,
    String,
    Option<(String, usize)>,
    Option<Symbol>,
);

/// The members a page documents as the item's own: `(tag, name, source, shown)`,
/// where `shown` is rustdoc's rendering of a member.
///
/// Everything from the trait implementations on belongs to somebody else —
/// `measure` on a widget is `View`'s, `on_tap` a blanket impl's — and a copy
/// of a trait's method on a type carries a second class, `trait-impl`.
fn members(page: &str, krate: &str) -> Vec<Member> {
    const THEIRS: [&str; 6] = [
        "id=\"trait-implementations",
        "id=\"synthetic-implementations",
        "id=\"blanket-implementations",
        "id=\"implementors",
        "id=\"foreign-impls",
        "id=\"deref-methods",
    ];
    let end = THEIRS
        .iter()
        .filter_map(|m| page.find(m))
        .min()
        .unwrap_or(page.len());
    let mut out: Vec<Member> = Vec::new();
    for chunk in page[..end].split(" id=\"").skip(1) {
        let Some((id, rest)) = chunk.split_once('"') else {
            continue;
        };
        let Some((tag, name)) = id.split_once('.') else {
            continue;
        };
        let tag = match tag {
            "method" | "tymethod" | "associatedconstant" | "associatedtype" => "member",
            "variant" => "variant",
            "structfield" => "structfield",
            _ => continue,
        };
        let own = [
            " class=\"method\"",
            " class=\"associatedconstant\"",
            " class=\"associatedtype\"",
        ];
        if tag == "member" && !own.iter().any(|c| rest.starts_with(c)) {
            continue;
        }
        let name = name.split('-').next().unwrap_or(name).to_string();
        // A variant's own fields belong to its row; a tuple field has no name.
        let nested = name.contains('.') || name.chars().all(|c| c.is_ascii_digit());
        if nested || out.iter().any(|(t, n, _, _)| *t == tag && *n == name) {
            continue;
        }
        let body = rest.split("</details>").next().unwrap_or(rest);
        let source = between(body, "class=\"src rightside\" href=\"", "\"")
            .and_then(|h| link(h, krate))
            .map(|(file, first, _)| (file, first));
        let shown = between(body, "<h4 class=\"code-header\">", "</h4>").map(|header| Symbol {
            name: String::new(),
            kind: Kind::Member,
            summary: between(body, "<div class=\"docblock\"><p>", "</p>")
                .map_or(String::new(), text_of),
            declaration: text_of(header),
            at: String::new(),
        });
        out.push((tag, name, source, shown));
    }
    out
}

/// The 1-based line of a variant or a public field, inside its type's lines.
fn locate(lines: &[&str], first: usize, last: usize, name: &str, field: bool) -> Option<usize> {
    (first.saturating_sub(1)..last.min(lines.len()))
        .find(|&i| {
            let line = lines[i].trim();
            if field {
                line.strip_prefix("pub ")
                    .and_then(|r| r.strip_prefix(name))
                    .is_some_and(|r| r.trim_start().starts_with(':'))
            } else {
                line.strip_prefix(name)
                    .is_some_and(|r| r.is_empty() || r.starts_with(['(', '{', ',', ' ', '=']))
            }
        })
        .map(|i| i + 1)
}

/// A symbol read from the source rustdoc says starts at `line`. That line may
/// be the item, one of its attributes, or its first `///`.
fn described(name: &str, kind: Kind, lines: &[&str], line: usize, path: &Path) -> Symbol {
    let decoration = |l: &str| l.trim().starts_with("///") || l.trim().starts_with("#[");
    let mut item = line.saturating_sub(1);
    while item < lines.len() && decoration(lines[item]) {
        item += 1;
    }
    let mut top = item;
    while top > 0 && decoration(lines[top - 1]) {
        top -= 1;
    }
    let summary: Vec<&str> = lines[top..item]
        .iter()
        .filter_map(|l| l.trim().strip_prefix("///"))
        .map(str::trim)
        .take_while(|l| !l.is_empty() && !l.starts_with("```"))
        .collect();
    Symbol {
        name: name.to_string(),
        kind,
        summary: summary.join(" "),
        declaration: declaration(&lines[item.min(lines.len())..]),
        at: format!("{}:{}", path.display(), item + 1),
    }
}

/// From the first line to the body or the `;`, outside any brackets.
fn declaration(lines: &[&str]) -> String {
    let mut out = String::new();
    let mut depth = 0;
    'lines: for line in lines {
        let code = line.split("//").next().unwrap_or(line);
        for c in code.chars() {
            match c {
                '(' | '[' => depth += 1,
                ')' | ']' => depth -= 1,
                '{' | ';' if depth == 0 => break 'lines,
                _ => {}
            }
            out.push(c);
        }
        out.push(' ');
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = concat!(
        r##"<span class="sub-heading"><a class="src" href="../../src/xpui/w/list.rs.html#3-5">Source</a> </span>"##,
        r##"<div id="implementations-list"><section id="method.push" class="method">"##,
        r##"<a class="src rightside" href="../../src/xpui/w/list.rs.html#9-11">Source</a></section>"##,
        r##"<section id="method.push-1" class="method"></section><section id="variant.Text" class="variant"></section>"##,
        r##"<section id="variant.Text.field.origin" class="variant"></section><span id="structfield.0" class="structfield">"##,
        r##"</span><section id="method.new" class="method"><a class="src rightside" href="../../src/xpui/w/list.rs.html#30">Source</a>"##,
        r##"<h4 class="code-header">pub fn <a href="#method.new">new</a>(n: <a>i32</a>) -&gt; Self</h4></section></summary>"##,
        r##"<div class="docblock"><p>A stack with <code>n</code> pixels, the host’s way.</p></div></details></div>"##,
        r##"<div id="trait-implementations-list"><section id="method.measure" class="method trait-impl"></section></div>"##,
        r##"<div id="blanket-implementations-list"><section id="method.on_tap" class="method trait-impl"></section></div>"##,
    );

    const SOURCE: &str = "/// A list.\n///\n/// More.\n#[derive(Clone)]\npub struct List<M> {\n    rows: Vec<M>,\n}\n\n/// Hints.\npub enum Hint {\n    /// The host's label.\n    Standard,\n    /// A label.\n    Text(String),\n}\n\n    /// Adds a row.\n    pub fn push(\n        mut self,\n        row: Row,\n    ) -> Self {\n";

    #[test]
    fn every_link_under_a_kind_is_an_item_and_the_root_names_it() {
        let all = r#"<h3 id="structs">S</h3><ul><li><a href="w/struct.List.html">w::List</a></li></ul><h3 id="macros">M</h3><ul><li><a href="macro.list.html">list</a></li></ul>"#;
        let found = items(all);
        assert_eq!(found.len(), 2);
        assert_eq!(
            found[0],
            (
                "structs".into(),
                "w/struct.List.html".into(),
                "w::List".into()
            )
        );
        assert_eq!(found[1].0, "macros");
        let index = r#"<dt id="reexport.List"><code>pub use w::<a class="struct" href="w/struct.List.html" title="x">List</a>;</code></dt>"#;
        assert_eq!(
            reexports(index),
            [("w/struct.List.html".to_string(), "List".to_string())]
        );
    }

    #[test]
    fn only_the_items_own_members_count() {
        let found = members(PAGE, "xpui");
        let names: Vec<&str> = found.iter().map(|(_, n, _, _)| n.as_str()).collect();
        assert_eq!(
            names,
            ["push", "Text", "new"],
            "not a trait impl's, a blanket impl's, a variant's field or a tuple field"
        );
        assert_eq!(found[0].2, Some(("w/list.rs".to_string(), 9)));
        let shown = found[2].3.as_ref().expect("rustdoc's rendering of `new`");
        assert_eq!(shown.declaration, "pub fn new(n: i32) -> Self");
        assert_eq!(shown.summary, "A stack with `n` pixels, the host's way.");
    }

    #[test]
    fn an_item_with_no_source_in_this_crate_is_foreign() {
        assert_eq!(top_source(PAGE, "xpui"), Some(("w/list.rs".into(), 3, 5)));
        assert_eq!(top_source(PAGE, "xpui_eg"), None);
    }

    #[test]
    fn a_summary_is_the_first_paragraph_and_a_declaration_stops_at_the_body() {
        let lines: Vec<&str> = SOURCE.lines().collect();
        let list = described("List", Kind::Item, &lines, 4, Path::new("x.rs"));
        assert_eq!(list.summary, "A list.");
        assert_eq!(list.declaration, "pub struct List<M>");
        assert_eq!(list.at, "x.rs:5", "the span began at the attribute");
        let push = described("List::push", Kind::Member, &lines, 18, Path::new("x.rs"));
        assert_eq!(push.summary, "Adds a row.");
        assert_eq!(
            push.declaration,
            "pub fn push( mut self, row: Row, ) -> Self"
        );
    }

    #[test]
    fn a_variant_is_found_inside_its_own_enum() {
        let lines: Vec<&str> = SOURCE.lines().collect();
        assert_eq!(locate(&lines, 10, 15, "Text", false), Some(14));
        assert_eq!(locate(&lines, 10, 15, "Standard", false), Some(12));
        assert_eq!(locate(&lines, 10, 15, "Missing", false), None);
        let text = described("Hint::Text", Kind::Row, &lines, 14, Path::new("x.rs"));
        assert_eq!(text.summary, "A label.");
    }
}
