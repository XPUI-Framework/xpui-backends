//! Reading fenced blocks out of markdown, once, for every check that reads
//! them.

/// One fenced block: its language, its lines, and where it started.
pub struct Fence<'a> {
    /// Lowercased, and without the attributes after it.
    pub language: String,
    pub lines: Vec<(usize, &'a str)>,
    /// 1-based line of the opening fence, for an error a reader can jump to.
    pub start: usize,
}

/// Every fenced block in a document, in order.
///
/// Both fence characters. Indented up to three spaces still counts; four is a
/// code block whose contents are not the language they look like. A fence
/// closes only on a run at least as long as the one that opened it — that is
/// how a ````markdown block quotes an inner ```bash without ending.
pub fn fences(text: &str) -> Vec<Fence<'_>> {
    let mut out = Vec::new();
    let mut open: Option<(Fence, char, usize)> = None;
    for (index, raw) in text.lines().enumerate() {
        let indent = raw.len() - raw.trim_start().len();
        let line = raw.trim_start();
        let marker = line.chars().next().filter(|c| *c == '`' || *c == '~');
        let run = marker.map_or(0, |c| line.chars().take_while(|x| *x == c).count());
        let is_fence = indent < 4 && run >= 3;

        match &mut open {
            Some((block, opener, opened_with)) => {
                // A closing fence is the same character, at least as long, and
                // carries no language of its own.
                if is_fence
                    && marker == Some(*opener)
                    && run >= *opened_with
                    && line[run..].trim().is_empty()
                {
                    out.push(open.take().expect("just matched Some").0);
                } else {
                    block.lines.push((index + 1, raw));
                }
            }
            None if is_fence => {
                let marker = marker.expect("a run implies a marker");
                open = Some((
                    Fence {
                        // ```rust,ignore, ```rust no_run and ``` rust are all
                        // rust; ```CPP is cpp. Rustdoc reads the info string
                        // this way and so must anything checking it.
                        language: language(&line[run..]),
                        lines: Vec::new(),
                        start: index + 1,
                    },
                    marker,
                    run,
                ));
            }
            None => {}
        }
    }
    // An unclosed fence at end of file swallows the rest of the document. It
    // is a document bug, but treating its contents as commands would report
    // nonsense, so it is dropped rather than guessed at.
    out
}

/// The language an info string names, lowercased, or `""` for none.
///
/// Everything after the first comma or space is an attribute — `ignore`,
/// `no_run`, `should_panic` — and is read separately by whoever cares.
fn language(info: &str) -> String {
    info.trim()
        .split([',', ' '])
        .next()
        .unwrap_or("")
        .to_lowercase()
}

/// A shell command line, joined across `\` continuations and stripped of the
/// noise that is not part of what runs.
pub struct ShellCommand {
    pub line: usize,
    pub text: String,
}

/// The runnable lines of a ```bash block, with `\` continuations joined: a
/// scanner reading lines sees only the half with the verb.
pub fn commands(fence: &Fence) -> Vec<ShellCommand> {
    let mut out: Vec<ShellCommand> = Vec::new();
    let mut pending: Option<ShellCommand> = None;
    for (number, raw) in &fence.lines {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            // A comment ends a continuation as surely as it ends anything.
            if let Some(done) = pending.take() {
                out.push(done);
            }
            continue;
        }
        // A trailing `# note` is prose, not an argument. Only outside quotes:
        // `awk '{print $2}  # x'` has no comment in it, and neither does a
        // `-DNAME="a # b"`.
        let line = strip_comment(line);
        let line = line.trim_end();
        if line.is_empty() {
            if let Some(done) = pending.take() {
                out.push(done);
            }
            continue;
        }
        let (body, continues) = match line.strip_suffix('\\') {
            Some(head) => (head.trim_end(), true),
            None => (line, false),
        };
        match &mut pending {
            Some(open) => {
                open.text.push(' ');
                open.text.push_str(body);
            }
            None => {
                pending = Some(ShellCommand {
                    line: *number,
                    text: body.to_string(),
                })
            }
        }
        if !continues && let Some(done) = pending.take() {
            out.push(done);
        }
    }
    out.extend(pending);
    out
}

/// A command line with any trailing `#` comment removed.
///
/// Quote-aware, because a `#` inside `'…'` or `"…"` is a character rather than
/// the start of a comment.
fn strip_comment(line: &str) -> &str {
    let (mut single, mut double) = (false, false);
    for (at, c) in line.char_indices() {
        match c {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            '#' if !single && !double => return &line[..at],
            _ => {}
        }
    }
    line
}

/// The words of a command with the shell's own decoration removed: `sudo`,
/// `env`, `export` and `VAR=value` prefixes are not the verb, and
/// `FREEINK_SDK_DIR=/opt/sdk` is not a path this repository holds.
pub fn words(command: &str) -> Vec<&str> {
    let mut parts = command.split_whitespace().peekable();
    while let Some(head) = parts.peek() {
        let is_assignment = head.split_once('=').is_some_and(|(name, _)| {
            !name.is_empty() && name.chars().all(|c| c.is_ascii_uppercase() || c == '_')
        });
        if *head == "sudo" || *head == "env" || *head == "export" || is_assignment {
            parts.next();
            continue;
        }
        break;
    }
    parts.collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fence_knows_its_language_and_where_it_began() {
        let blocks =
            fences("intro\n\n```bash\ncargo test\n```\n\n```rust,ignore\nlet x = 1;\n```\n");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].language, "bash");
        assert_eq!(blocks[0].start, 3);
        assert_eq!(blocks[1].language, "rust");
    }

    #[test]
    fn a_tilde_fence_is_a_fence() {
        // A page is free to use either character, and a scanner that knows one
        // reads a document with `~~~` in it as having no code at all.
        let blocks = fences("~~~bash\ncargo test\n~~~\n");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language, "bash");
    }

    #[test]
    fn a_language_is_read_case_insensitively_and_past_a_space() {
        assert_eq!(fences("```CPP\nint x;\n```\n")[0].language, "cpp");
        assert_eq!(fences("``` rust\nlet x = 1;\n```\n")[0].language, "rust");
        assert_eq!(
            fences("```rust no_run\nlet x = 1;\n```\n")[0].language,
            "rust"
        );
    }

    #[test]
    fn a_longer_fence_may_quote_a_shorter_one() {
        // ````markdown quoting an inner ```bash is one block, not three.
        let blocks = fences("````markdown\n```bash\ncargo test\n```\n````\n");
        assert_eq!(
            blocks.len(),
            1,
            "{:?}",
            blocks.iter().map(|b| &b.language).collect::<Vec<_>>()
        );
        assert_eq!(blocks[0].language, "markdown");
        assert_eq!(blocks[0].lines.len(), 3);
    }

    #[test]
    fn a_fence_of_one_character_does_not_close_a_fence_of_another() {
        let blocks = fences("```bash\n~~~\ncargo test\n```\n");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].lines.len(), 2);
    }

    #[test]
    fn a_fence_indented_four_spaces_is_not_a_fence() {
        // It is a code block whose *contents* happen to look like a fence.
        assert!(fences("    ```bash\n    rm -rf /\n    ```\n").is_empty());
        assert_eq!(fences("  ```bash\n  cargo test\n  ```\n").len(), 1);
    }

    #[test]
    fn an_unlabelled_fence_has_an_empty_language() {
        // Not the same as no fence: rustdoc compiles an unlabelled block as
        // Rust, so something has to be able to see it.
        let blocks = fences("```\nlet x = 1;\n```\n");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].language, "");
    }

    #[test]
    fn an_unclosed_fence_does_not_swallow_the_document() {
        assert!(fences("```bash\ncargo test\n").is_empty());
    }

    #[test]
    fn a_backslash_joins_two_lines_into_one_command() {
        let blocks = fences(
            "```bash\ncargo clippy \\\n  -p xpui -p xpui-chrome \\\n  -- -D warnings\n```\n",
        );
        let joined = commands(&blocks[0]);
        assert_eq!(joined.len(), 1);
        assert_eq!(
            joined[0].text,
            "cargo clippy -p xpui -p xpui-chrome -- -D warnings"
        );
        assert_eq!(
            joined[0].line, 2,
            "an error points at where the command starts"
        );
    }

    #[test]
    fn a_comment_ends_a_command_rather_than_joining_it() {
        let blocks = fences("```bash\ncargo test\n# and then\ncargo build\n```\n");
        assert_eq!(commands(&blocks[0]).len(), 2);
    }

    #[test]
    fn a_trailing_comment_is_not_part_of_the_command() {
        let blocks = fences("```bash\ncargo test   # then read invented/report.txt\n```\n");
        let found = commands(&blocks[0]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].text, "cargo test");
    }

    #[test]
    fn a_hash_inside_quotes_is_not_a_comment() {
        let blocks = fences("```bash\nawk '{print $2}  # kept'\n```\n");
        assert!(commands(&blocks[0])[0].text.contains("kept"));
    }

    #[test]
    fn sudo_and_environment_prefixes_do_not_hide_the_verb() {
        assert_eq!(words("sudo apt-get install sdl2")[0], "apt-get");
        assert_eq!(words("SDL_VIDEODRIVER=dummy cargo test")[0], "cargo");
        assert_eq!(
            words("FREEINK_SDK_DIR=/opt/sdk ./build-and-test.sh")[0],
            "./build-and-test.sh"
        );
    }

    #[test]
    fn export_does_not_hide_an_assignment() {
        // `export FREEINK_SDK_DIR=/path/to/sdk` is an instruction to the
        // reader, not a path this repository is expected to contain.
        assert!(words("export FREEINK_SDK_DIR=/path/to/freeink-sdk").is_empty());
        assert_eq!(words("export PATH=/x cargo test")[0], "cargo");
    }

    #[test]
    fn only_an_uppercase_name_before_the_equals_is_an_assignment() {
        // A head word with no `=` never reaches the rule; these do.
        assert_eq!(words("foo=bar cargo test")[0], "foo=bar");
        assert_eq!(words("Mixed_Case=1 cargo test")[0], "Mixed_Case=1");
        assert_eq!(words("=novalue cargo test")[0], "=novalue");
        assert_eq!(words("REAL_NAME=1 cargo test")[0], "cargo");
    }

    #[test]
    fn a_lowercase_equals_is_an_argument_and_not_an_assignment() {
        assert_eq!(words("cmake -DCMAKE_BUILD_TYPE=Release ..")[0], "cmake");
        assert_eq!(words("pio run -e badger2040")[0], "pio");
    }
}
