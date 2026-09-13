//! The FreeInkUI shim's own stages: its header against its definitions, where
//! a documented snippet's headers are, and a syntax-only compile.

use std::path::PathBuf;

use crate::cpp;

/// The boundary this repository owns: its own header against its own shim.
///
/// `fui/tests/abi.rs` compares *signatures*. This compares *presence*, which
/// that cannot read, and the two do not overlap. The other two pairs read a
/// C++ host and a firmware, and moved to `xpui-cpp` with them.
pub fn symbols_agree() -> Result<String, String> {
    let header = cpp::symbols("xpui_fui_", &[PathBuf::from("fui/cpp/xpui_fui.h")])?;
    if header.is_empty() {
        return Err("no xpui_fui_ symbols in fui/cpp/xpui_fui.h — nothing was compared".into());
    }
    let shim = cpp::symbols("xpui_fui_", &[PathBuf::from("fui/cpp/xpui_fui.cpp")])?;
    cpp::symbols_agree(
        "xpui_fui.h and xpui_fui.cpp",
        "header",
        &header,
        "shim",
        &shim,
    )
    .map_err(|why| {
        format!(
            "{why}\n\nA symbol in a header with no definition is a link error waiting\n\
                 for whoever includes it."
        )
    })?;
    Ok(format!("{} symbol(s)", header.len()))
}

/// Where a documented C++ snippet's headers are, or the reason there are none.
pub fn snippet_includes() -> Result<Vec<String>, String> {
    let sdk = cpp::freeink_include()
        .ok_or("FreeInkUI headers not found. Set FREEINK_SDK_INCLUDE to run it.")?;
    Ok(vec![
        format!("-I{}", sdk.display()),
        "-Ifui/cpp".to_string(),
    ])
}

/// The shim compiles, on its own terms.
///
/// `-fno-exceptions -fno-rtti` because it is compiled into a firmware that
/// builds with both off; a shim that needs either would link there and fail
/// nowhere else.
pub fn shim_compiles() -> Result<String, String> {
    let Some(sdk) = cpp::freeink_include() else {
        return cpp::skipped("FreeInkUI headers not found. Set FREEINK_SDK_INCLUDE.");
    };
    let status = std::process::Command::new("clang++")
        .args([
            "-std=c++17",
            "-fsyntax-only",
            "-fno-exceptions",
            "-fno-rtti",
            "-Wall",
            "-Wextra",
        ])
        .arg(format!("-I{}", sdk.display()))
        .args(["-Ifui/cpp", "fui/cpp/xpui_fui.cpp"])
        .status()
        .map_err(|e| format!("clang++: {e}"))?;
    if status.success() {
        Ok("clean".into())
    } else {
        Err("fui/cpp/xpui_fui.cpp does not compile".into())
    }
}
