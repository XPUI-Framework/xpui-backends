//! What a reference page says: the names it carries, the abstract and
//! declaration under each, and the words both sides are compared as.
//!
//! Its neighbour `reference.rs` holds these against what rustdoc wrote.

use crate::fences::fences;

/// A name a page carries: a heading that is one code span, or a table row
/// whose first cell is one qualified code span.
pub struct Section {
    pub name: String,
    pub at: String,
    pub summary: Option<String>,
    pub declaration: Option<String>,
    pub row: bool,
}

/// Every section in a page. A fenced line is never a heading or a row.
pub fn sections_in(text: &str, page: &str) -> Vec<Section> {
    let blocks = fences(text);
    let lines: Vec<&str> = text.lines().collect();
    let mut fenced = vec![false; lines.len() + 2];
    for block in &blocks {
        let close = block.lines.last().map_or(block.start, |(n, _)| *n) + 1;
        fenced[block.start..=close.min(lines.len())].fill(true);
    }
    let heading = |n: usize| !fenced[n] && lines[n - 1].starts_with('#');
    let mut out = Vec::new();
    let mut reexports = false;
    for (n, line) in lines.iter().enumerate().map(|(i, l)| (i + 1, *l)) {
        if fenced[n] {
            continue;
        }
        let at = format!("{page}:{n}");
        if line.starts_with('#') {
            let level = line.chars().take_while(|c| *c == '#').count();
            let title = line[level..].trim();
            if level == 2 {
                reexports = title == "Re-exports";
            }
            let Some(name) = code_span(title).filter(|_| (2..=4).contains(&level)) else {
                continue;
            };
            let next = (n + 1..=lines.len())
                .find(|m| heading(*m))
                .unwrap_or(lines.len() + 1);
            let declaration = blocks
                .iter()
                .find(|b| b.language == "text" && b.start > n && b.start < next)
                .map(|b| {
                    b.lines
                        .iter()
                        .map(|(_, l)| *l)
                        .collect::<Vec<_>>()
                        .join(" ")
                });
            let summary = paragraph(&lines, &fenced, n + 1, next);
            out.push(Section {
                name: name.into(),
                at,
                summary,
                declaration,
                row: false,
            });
        } else if let Some(cells) = line.trim().strip_prefix('|') {
            let mut cells = cells.split('|').map(str::trim);
            let (Some(first), Some(second)) = (cells.next(), cells.next()) else {
                continue;
            };
            // A parameter table's `index` is not a name; `Hint::Standard` is.
            let Some(name) = code_span(first).filter(|n| reexports || n.contains("::")) else {
                continue;
            };
            let summary = Some(second.to_string());
            out.push(Section {
                name: name.into(),
                at,
                summary,
                declaration: None,
                row: true,
            });
        }
    }
    out
}

fn code_span(text: &str) -> Option<&str> {
    let inner = text.strip_prefix('`')?.strip_suffix('`')?;
    (!inner.is_empty() && !inner.contains('`')).then_some(inner)
}

/// The first paragraph in `from..to`, unless what comes first is not prose.
fn paragraph(lines: &[&str], fenced: &[bool], from: usize, to: usize) -> Option<String> {
    let start = (from..to).find(|n| !lines[n - 1].trim().is_empty())?;
    if fenced[start]
        || lines[start - 1]
            .trim_start()
            .starts_with(['|', '>', '!', '<', '#'])
    {
        return None;
    }
    let body: Vec<&str> = (start..to)
        .take_while(|n| !fenced[*n] && !lines[n - 1].trim().is_empty())
        .map(|n| lines[n - 1].trim())
        .collect();
    Some(body.join(" "))
}

/// A paragraph as it reads: links reduced to their text, whitespace collapsed.
pub fn prose(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find(']').map(|c| open + c) else {
            break;
        };
        out.push_str(&rest[..open]);
        out.push_str(&rest[open + 1..close]);
        rest = &rest[close + 1..];
        let closer = match rest.chars().next() {
            Some('(') => ')',
            Some('[') => ']',
            _ => continue,
        };
        rest = rest.find(closer).map_or("", |at| &rest[at + 1..]);
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A signature as it is compared: whitespace collapsed, and what rustdoc does
/// not show removed — `mut` on a binding, a trailing comma, a constant's value.
///
/// The item's own name is `name`, so an item re-exported under another name
/// reads the way docs.rs shows it.
pub fn signature(text: &str, name: &str) -> String {
    let flat = text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("( ", "(")
        .replace(" )", ")")
        .replace(",)", ")")
        .replace("(mut ", "(")
        .replace(", mut ", ", ");
    let flat = match flat.split_once(" = ") {
        Some((head, _))
            if [" const ", " static "]
                .iter()
                .any(|k| format!(" {head}").contains(k)) =>
        {
            head.to_string()
        }
        _ => flat,
    };
    renamed(&flat, name)
}

/// Whether `declaration` declares `name` itself, rather than calling a macro
/// that writes it or carrying the name it had before a re-export renamed it.
pub fn declares(declaration: &str, name: &str) -> bool {
    let name = name
        .rsplit("::")
        .next()
        .unwrap_or(name)
        .trim_end_matches('!');
    let keywords = [
        "fn ",
        "struct ",
        "enum ",
        "trait ",
        "type ",
        "const ",
        "static ",
        "union ",
        "macro_rules! ",
    ];
    keywords.iter().any(|keyword| {
        declaration.match_indices(keyword).any(|(at, _)| {
            let rest = &declaration[at + keyword.len()..];
            rest.strip_prefix(name)
                .is_some_and(|after| !after.starts_with(|c: char| c.is_alphanumeric() || c == '_'))
        })
    })
}

/// Rustdoc's own rendering of an item page: its declaration, without the
/// attributes and body rustdoc draws around it, and its first paragraph.
pub fn rendered(page: &str) -> Option<(String, String)> {
    let code = between(
        page,
        "<pre class=\"rust item-decl\"><code>",
        "</code></pre>",
    )?;
    let code: String = code
        .split("<div class=\"code-attribute\">")
        .enumerate()
        .map(|(i, part)| {
            if i == 0 {
                part
            } else {
                part.split_once("</div>").map_or(part, |(_, rest)| rest)
            }
        })
        .collect();
    let text = text_of(&code);
    let declaration = text
        .split(" {")
        .next()
        .unwrap_or(&text)
        .trim_end_matches(';')
        .to_string();
    let summary = between(page, "top-doc", "</details>")
        .and_then(|doc| between(doc, "<div class=\"docblock\"><p>", "</p>"))
        .map_or(String::new(), text_of);
    Some((declaration, summary))
}

/// `declaration` with the identifier after its keyword replaced by `name`.
fn renamed(declaration: &str, name: &str) -> String {
    let name = name
        .rsplit("::")
        .next()
        .unwrap_or(name)
        .trim_end_matches('!');
    for keyword in [
        "fn ",
        "struct ",
        "enum ",
        "trait ",
        "type ",
        "const ",
        "static ",
        "macro_rules! ",
    ] {
        let Some(at) = declaration
            .find(keyword)
            .filter(|at| *at == 0 || declaration[..*at].ends_with(' '))
        else {
            continue;
        };
        let start = at + keyword.len();
        let end = declaration[start..]
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .map_or(declaration.len(), |n| start + n);
        if name.is_empty() || declaration[start..end] == *name {
            return declaration.to_string();
        }
        return format!("{}{name}{}", &declaration[..start], &declaration[end..]);
    }
    declaration.to_string()
}

pub fn between<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    Some(text.split_once(open)?.1.split_once(close)?.0)
}

/// Rustdoc's HTML as the source said it: `<code>` as backticks, tags gone,
/// entities and typographic quotes undone.
pub fn text_of(html: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in html.replace("<code>", "`").replace("</code>", "`").chars() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            _ if !inside => out.push(c),
            _ => {}
        }
    }
    let out = out
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    let out = out
        .replace(['\u{2018}', '\u{2019}'], "'")
        .replace(['\u{201c}', '\u{201d}'], "\"")
        .replace("&amp;", "&");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
pub const PAGE: &str = "# Lists\n\n## Topics\n\n| [`List`](#list) | A list. |\n\n## `List`\n\nA themed list, filling [the space](#x)\nit is given.\n\n```text\npub struct List<M>\n```\n\n| Parameter | Meaning |\n|---|---|\n| `index` | zero-based |\n\n```rust\n## `NotASection`\n```\n\n### Adding rows\n\n#### `List::push`\n\nAdds a row.\n\n```text\npub fn push(self, row: ListRow<M>) -> Self\n```\n\n| `Hint::Standard` | The host's label. |\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heading_that_is_one_code_span_is_a_section_and_nothing_else_is() {
        let found = sections_in(PAGE, "lists.md");
        let names: Vec<&str> = found.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            ["List", "List::push", "Hint::Standard"],
            "not the topics link, the parameter, or the fence"
        );
        assert_eq!(found[0].at, "lists.md:7");
        assert_eq!(found[0].declaration.as_deref(), Some("pub struct List<M>"));
    }

    #[test]
    fn a_link_reads_as_its_text_and_a_binding_loses_its_mut() {
        assert_eq!(
            prose("See [`Text`](crate::Text),\n[the guide][g] and [`List`]."),
            "See `Text`, the guide and `List`."
        );
        assert_eq!(
            signature("pub fn push(\n mut self,\n row: Row,\n) -> Self", "push"),
            "pub fn push(self, row: Row) -> Self"
        );
    }

    #[test]
    fn a_constant_loses_its_value_and_a_renamed_item_takes_its_exported_name() {
        assert_eq!(
            signature("pub const NONE: Run = Run", "Run::NONE"),
            "pub const NONE: Run"
        );
        assert_eq!(
            signature("pub const UNBOUNDED: i32 = 1 << 24", "UNBOUNDED"),
            "pub const UNBOUNDED: i32"
        );
        assert_eq!(
            signature("pub fn capture(panel: &Panel) -> Frame", "capture_panel"),
            "pub fn capture_panel(panel: &Panel) -> Frame"
        );
        assert_eq!(
            signature("macro_rules! vstack", "vstack!"),
            "macro_rules! vstack"
        );
        assert_eq!(
            signature("pub fn new(spacing: i32) -> Self", "VStack::new"),
            "pub fn new(spacing: i32) -> Self"
        );
    }

    #[test]
    fn a_macro_call_or_an_old_name_does_not_declare_the_item() {
        assert!(declares(
            "pub unsafe extern \"C\" fn xpui_app_install()",
            "xpui_app_install"
        ));
        assert!(declares("macro_rules! vstack", "vstack!"));
        assert!(!declares(
            "register_screen!(screens::Menu, xpui_app_create_menu)",
            "xpui_app_create_menu"
        ));
        assert!(!declares("pub fn capture(panel: &Panel)", "capture_panel"));
        assert!(!declares("pub fn newer()", "App::new"));
    }

    #[test]
    fn rustdoc_renders_what_a_macro_wrote() {
        let page = concat!(
            r#"<pre class="rust item-decl"><code><div class="code-attribute">#[unsafe(no_mangle)]</div>"#,
            r#"pub extern &quot;C&quot; fn make() -&gt; <a>*mut </a>c_void</code></pre>"#,
            r#"<details class="toggle top-doc" open><div class="docblock"><p>The root screen’s <code>make</code>.</p>"#,
            r#"<p>More.</p></div></details>"#,
        );
        let (declaration, summary) = rendered(page).expect("an item page");
        assert_eq!(declaration, "pub extern \"C\" fn make() -> *mut c_void");
        assert_eq!(summary, "The root screen's `make`.");
    }
}
