//! The C++ half of the boundary: formatting it, compiling what the prose
//! shows, and checking that every symbol a header declares is defined.
//!
//! Carried only by the two repositories that hold C++.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::fences::fences;
use crate::paths::tracked;

/// Every C++ file this repository owns.
///
/// From the filesystem rather than git, because the C++ ships inside a crate
/// directory and has to be formattable before it has ever been committed.
/// Build outputs are pruned: PlatformIO writes CMake's compiler probes under
/// `.pio/`, and a fetched SDK is somebody else's code to format.
pub fn sources() -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(Path::new("."), &mut out);
    out.sort();
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    const PRUNED: [&str; 5] = [".pio", "target", ".git", "freeink-sdk", "build"];
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let name = entry.file_name();
        if PRUNED.iter().any(|p| name == *p) {
            continue;
        }
        if path.is_dir() {
            walk(&path, out);
        } else if path
            .extension()
            .is_some_and(|x| x == "cpp" || x == "h" || x == "hpp")
        {
            out.push(path.strip_prefix("./").unwrap_or(&path).to_path_buf());
        }
    }
}

/// The C++ is formatted, to the same `.clang-format` as the firmware it sits
/// beside — so a file moving between the two does not reformat on arrival.
///
/// The version is checked first. An old clang-format does not reject options
/// it does not understand; it ignores them, and hands back a file formatted
/// differently from what CI expects, with no warning at all.
pub fn format(fix: bool) -> Result<String, String> {
    let files = sources();
    if files.is_empty() {
        return Err("no C++ found — this check belongs to a repository that has some".into());
    }
    let binary = ["clang-format-21", "clang-format"]
        .into_iter()
        .find(|b| {
            Command::new(b)
                .arg("--version")
                .output()
                .is_ok_and(|o| o.status.success())
        })
        .ok_or("clang-format not found. Install clang-format 21 or newer.")?;

    let version = Command::new(binary)
        .arg("--version")
        .output()
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&version.stdout);
    let major: u32 = text
        .split(|c: char| !c.is_ascii_digit())
        .find(|s| !s.is_empty())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if major < 21 {
        return Err(format!(
            "{binary} is {}, and this repository's .clang-format needs 21 or newer.\n\
             An older binary ignores options it does not know rather than\n\
             refusing them, so it formats differently from CI in silence.",
            text.trim()
        ));
    }

    let mut command = Command::new(binary);
    command.arg("-style=file");
    if fix {
        command.arg("-i");
    } else {
        command.args(["--dry-run", "--Werror"]);
    }
    let status = command.args(&files).status().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(format!("{} file(s) with {binary}", files.len()))
    } else {
        Err(format!(
            "{binary} reported unformatted C++ (./build-and-test.sh fix)"
        ))
    }
}

/// A stage that cannot run for want of something this machine lacks.
///
/// On a laptop that is a note, because checking a Rust change should not
/// demand a download. On CI it is a failure: the job is provisioned for the
/// C++, and a skip there reports a pass for code nothing compiled.
pub fn skipped(why: &str) -> Result<String, String> {
    skip_or_fail(why, std::env::var_os("CI").is_some())
}

fn skip_or_fail(why: &str, on_ci: bool) -> Result<String, String> {
    if on_ci {
        Err(format!(
            "{why}\nCI provides this, so a skip here would pass code nothing compiled."
        ))
    } else {
        Ok(format!("skipped: {why}"))
    }
}

/// Every documented C++ snippet compiles.
///
/// `includes` is where the headers a snippet includes live, or the reason
/// there are none — a missing SDK is [`skipped`].
pub fn snippets_compile(
    includes: Result<Vec<String>, String>,
    expect_some: bool,
) -> Result<String, String> {
    let mut blocks = Vec::new();
    for page in tracked("*.md") {
        let text = fs::read_to_string(&page).unwrap_or_default();
        for fence in fences(&text)
            .iter()
            .filter(|f| f.language == "cpp" || f.language == "c++")
        {
            // The fence's own indentation is stripped so a snippet inside a
            // list item still compiles.
            let indent = fence
                .lines
                .iter()
                .filter(|(_, l)| !l.trim().is_empty())
                .map(|(_, l)| l.len() - l.trim_start().len())
                .min()
                .unwrap_or(0);
            let body: String = fence
                .lines
                .iter()
                .map(|(_, l)| {
                    if l.len() > indent {
                        &l[indent..]
                    } else {
                        l.trim()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            blocks.push((format!("{}:{}", page.display(), fence.start), body));
        }
    }
    if blocks.is_empty() {
        // In a repository that documents C++, zero blocks means the scanner
        // broke — not that the prose is clean.
        if expect_some {
            return Err("no C++ fences found, and this repository documents C++.\n\
                        Either the prose lost its examples or the fence scanner\n\
                        stopped recognising them."
                .into());
        }
        // Not `skipped:`, which always means a prerequisite is missing.
        return Ok("no C++ in this repository's prose".into());
    }
    let flags = match includes {
        Ok(flags) => flags,
        Err(why) => return skipped(&why),
    };

    let scratch = std::env::temp_dir().join(format!("xpui-snippets-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;

    let mut broken = Vec::new();
    for (index, (where_from, body)) in blocks.iter().enumerate() {
        let file = scratch.join(format!("snippet_{index}.cpp"));
        fs::write(&file, body).map_err(|e| e.to_string())?;
        let out = Command::new("clang++")
            .args(["-std=c++17", "-fsyntax-only", "-Wall", "-Wextra"])
            .args(&flags)
            .arg(&file)
            .output()
            .map_err(|e| format!("clang++: {e}"))?;
        if !out.status.success() {
            broken.push(format!(
                "  {where_from}\n{}",
                String::from_utf8_lossy(&out.stderr)
                    .lines()
                    .take(6)
                    .map(|l| format!("    {l}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
    }
    let _ = fs::remove_dir_all(&scratch);
    if broken.is_empty() {
        Ok(format!("{} snippet(s)", blocks.len()))
    } else {
        Err(broken.join("\n"))
    }
}

/// Every symbol matching `prefix` in the given files.
///
/// Presence, not signature — a separate check parses signatures. This catches
/// a symbol declared in a header with nothing defining it: a link error
/// waiting for whoever includes it.
pub fn symbols(prefix: &str, files: &[PathBuf]) -> Result<BTreeSet<String>, String> {
    let mut found = BTreeSet::new();
    for file in files {
        // Not `unwrap_or_default`: an unreadable path contributes no symbols,
        // which reads exactly like a file that defines none.
        let text = fs::read_to_string(file).map_err(|e| format!("  {}: {e}", file.display()))?;
        let mut rest = text.as_str();
        while let Some(at) = rest.find(prefix) {
            let tail = &rest[at..];
            let end = tail
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .unwrap_or(tail.len());
            found.insert(tail[..end].to_string());
            rest = &tail[end..];
        }
    }
    Ok(found)
}

/// Two symbol sets, and what each side is called when they disagree.
pub fn symbols_agree(
    what: &str,
    left_name: &str,
    left: &BTreeSet<String>,
    right_name: &str,
    right: &BTreeSet<String>,
) -> Result<(), String> {
    let only_left: Vec<_> = left.difference(right).cloned().collect();
    let only_right: Vec<_> = right.difference(left).cloned().collect();
    if only_left.is_empty() && only_right.is_empty() {
        return Ok(());
    }
    let mut out = format!("  {what}:");
    if !only_left.is_empty() {
        out.push_str(&format!(
            "\n    only in {left_name}: {}",
            only_left.join(" ")
        ));
    }
    if !only_right.is_empty() {
        out.push_str(&format!(
            "\n    only in {right_name}: {}",
            only_right.join(" ")
        ));
    }
    Err(out)
}

/// The FreeInkUI headers, or the reason there are none.
///
/// Neither candidate is a submodule and neither is required.
/// `FREEINK_SDK_INCLUDE` overrides both.
pub fn freeink_include() -> Option<PathBuf> {
    if let Ok(named) = std::env::var("FREEINK_SDK_INCLUDE") {
        let path = PathBuf::from(named);
        return path.is_dir().then_some(path);
    }
    [
        "../../Freeink/freeink-sdk/libs/ui/FreeInkUI/include",
        "../crosspoint-reader/freeink-sdk/libs/ui/FreeInkUI/include",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|p| p.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_symbol_is_read_to_its_end_and_no_further() {
        let file = std::env::temp_dir().join(format!("xpui-symbols-{}.h", std::process::id()));
        fs::write(
            &file,
            "void xpui_host_draw(int);\nint xpui_host_keys;\nother();\n",
        )
        .unwrap();
        let found =
            symbols("xpui_host_", std::slice::from_ref(&file)).expect("a file just written");
        assert_eq!(
            found.into_iter().collect::<Vec<_>>(),
            ["xpui_host_draw", "xpui_host_keys"]
        );
        let _ = fs::remove_file(file);
    }

    #[test]
    fn a_side_with_an_extra_symbol_is_named() {
        let header: BTreeSet<String> = ["a".into(), "b".into()].into_iter().collect();
        let cpp: BTreeSet<String> = ["a".into()].into_iter().collect();
        let why = symbols_agree("h and cpp", "header", &header, "cpp", &cpp).unwrap_err();
        assert!(why.contains("only in header: b"), "{why}");
        assert!(!why.contains("only in cpp"), "{why}");
    }

    #[test]
    fn a_file_that_cannot_be_read_is_an_error_and_not_an_empty_set() {
        // Silently treating an unreadable path as a file with no symbols is
        // how a comparison passes against nothing at all.
        let missing = std::env::temp_dir().join("xpui-not-here-at-all.h");
        assert!(symbols("xpui_host_", std::slice::from_ref(&missing)).is_err());
    }

    #[test]
    fn a_missing_prerequisite_is_a_note_on_a_laptop_and_a_failure_on_ci() {
        assert_eq!(
            skip_or_fail("no SDK", false),
            Ok("skipped: no SDK".to_string())
        );
        let why = skip_or_fail("no SDK", true).unwrap_err();
        assert!(why.starts_with("no SDK"), "{why}");
    }

    #[test]
    fn agreement_is_silent() {
        let both: BTreeSet<String> = ["a".into()].into_iter().collect();
        assert!(symbols_agree("x", "l", &both, "r", &both).is_ok());
    }
}
