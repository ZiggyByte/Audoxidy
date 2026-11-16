//! Source-scan convention guards for the test layout.
//!
//! The guard here walks the source tree and asserts, with no exclusion list and
//! no self-skip, that no production file keeps an inline test module: a test
//! module lives in its own `*_tests.rs` area file and is reached through a
//! declaration, so the body and its declaration never share a file.
//!
//! The guard is a pure predicate over a source string, so it is exercised with
//! synthetic violating and clean sources; the tree test is a thin wrapper that
//! feeds real files to the same predicate. Matching is deliberately narrow: only
//! a `mod <ident> {` line can be a body, so a declaration ending in `;` — the
//! form every migrated module uses — is structurally impossible to flag, and a
//! non-test module body is left alone.

use std::fs;
use std::path::{Path, PathBuf};

/// A single offending source line, located by one-based line number.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Violation {
    /// One-based line number of the offending declaration.
    line: usize,
    /// Human-readable description of what matched.
    text: String,
}

// ── Source navigation ─────────────────────────────────────────────

/// The `src/` directory rooted at the crate manifest, so the scan is independent
/// of the process working directory.
fn source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Recursively collect `.rs` files under `dir`, sorted for deterministic output.
fn rs_files_under(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rs_files(dir, &mut files);
    files.sort();
    files
}

fn collect_rs_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, files);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

/// Run a predicate over every file and collect `path:line: detail` reports.
fn collect_tree_violations(
    files: &[PathBuf],
    predicate: fn(&str) -> Vec<Violation>,
) -> Vec<String> {
    let mut reports = Vec::new();
    for file in files {
        let source = match fs::read_to_string(file) {
            Ok(source) => source,
            Err(error) => {
                reports.push(format!("{}: unreadable: {error}", file.display()));
                continue;
            }
        };
        for violation in predicate(&source) {
            reports.push(format!(
                "{}:{}: {}",
                file.display(),
                violation.line,
                violation.text
            ));
        }
    }
    reports
}

fn is_word_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn strip_line_comment(line: &str) -> &str {
    match line.find("//") {
        Some(position) => &line[..position],
        None => line,
    }
}

/// True when the file name carries the area test-file suffix.
fn is_area_test_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with("_tests.rs"))
}

// ── Guard 1: no inline test body survives in a production file ────

/// Report every `mod <ident> { … }` body that belongs to a `#[cfg(test)]`
/// module.
///
/// A module is a body only when a brace follows its name; a declaration ends
/// with `;` and owns no body, so it can never be reported. The attribute search
/// walks backwards over the attribute and comment lines above the `mod` line, so
/// a doc comment or an `allow` attribute placed between the gate and the body
/// still counts. Line comments are stripped first, so a commented-out module is
/// not a violation.
fn inline_test_module_bodies(source: &str) -> Vec<Violation> {
    let lines: Vec<&str> = source.lines().collect();
    let mut violations = Vec::new();
    for (index, raw_line) in lines.iter().enumerate() {
        let code = strip_line_comment(raw_line);
        let Some(name) = module_body_name(code) else {
            continue;
        };
        if attributes_gate_on_test(&lines, index) {
            violations.push(Violation {
                line: index + 1,
                text: format!("inline `mod {name}` body gated on the test configuration"),
            });
        }
    }
    violations
}

/// Return the module name of a `mod <ident> {` line, or `None` when the line
/// declares a module without a body or is not a module item.
fn module_body_name(code: &str) -> Option<&str> {
    let rest = strip_visibility(code.trim_start());
    let after_mod = rest.strip_prefix("mod")?;
    if !after_mod.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = after_mod.trim_start();
    let name_len = rest.find(|character: char| !is_word_char(character))?;
    if name_len == 0 {
        return None;
    }
    if rest[name_len..].trim_start().starts_with('{') {
        Some(&rest[..name_len])
    } else {
        None
    }
}

/// Drop a leading `pub`, `pub(crate)`, `pub(super)` or `pub(in path)` prefix.
fn strip_visibility(code: &str) -> &str {
    let Some(after_pub) = code.strip_prefix("pub") else {
        return code;
    };
    if after_pub.starts_with(char::is_whitespace) {
        return after_pub.trim_start();
    }
    let Some(after_paren) = after_pub.strip_prefix('(') else {
        return code;
    };
    match after_paren.find(')') {
        Some(close) => after_paren[close + 1..].trim_start(),
        None => code,
    }
}

/// True when the attribute block above line `index` gates on the test
/// configuration.
///
/// The walk stops at the first line that is neither blank, an attribute nor a
/// comment, so an unrelated item above the block cannot make a module look
/// gated.
fn attributes_gate_on_test(lines: &[&str], index: usize) -> bool {
    let mut cursor = index;
    while cursor > 0 {
        cursor -= 1;
        let line = lines[cursor].trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line.starts_with("#[") {
            if gates_on_test(line) {
                return true;
            }
            continue;
        }
        return false;
    }
    false
}

/// True when a configuration attribute names the test configuration as a whole
/// word, so both `cfg(test)` and `cfg(all(test, …))` count.
fn gates_on_test(attribute: &str) -> bool {
    let Some(body) = attribute.strip_prefix("#[cfg(") else {
        return false;
    };
    let name = "test";
    let mut offset = 0;
    while let Some(relative) = body[offset..].find(name) {
        let start = offset + relative;
        let end = start + name.len();
        let before_is_boundary = match body[..start].chars().next_back() {
            Some(character) => !is_word_char(character),
            None => true,
        };
        let after_is_boundary = match body[end..].chars().next() {
            Some(character) => !is_word_char(character),
            None => true,
        };
        if before_is_boundary && after_is_boundary {
            return true;
        }
        offset = end;
    }
    false
}

// ── Tests ─────────────────────────────────────────────────────────

#[test]
fn test_inline_test_guard_flags_gated_module_bodies() {
    let immediate = "#[cfg(test)]\nmod tests {\n    use super::*;\n}\n";
    let violations = inline_test_module_bodies(immediate);
    assert_eq!(
        violations.len(),
        1,
        "a module body under the test gate must be flagged"
    );
    assert_eq!(violations[0].line, 2);

    let behind_doc_comment = concat!(
        "#[cfg(test)]\n",
        "/// Tests for the module above.\n",
        "#[allow(clippy::module_inception)]\n",
        "pub(crate) mod tests {\n",
        "    use super::*;\n",
        "}\n",
    );
    let violations = inline_test_module_bodies(behind_doc_comment);
    assert_eq!(
        violations.len(),
        1,
        "an intervening doc comment or allow attribute must not hide the gate"
    );
    assert_eq!(violations[0].line, 4);
}

#[test]
fn test_inline_test_guard_ignores_declarations_and_production_bodies() {
    let naturals = ["#[cfg(test)]\nmod cover_tests;\n"];
    let routed = ["#[cfg(test)]\n#[path = \"cover_tests.rs\"]\nmod tests;\n"];
    for source in naturals.iter().chain(routed.iter()) {
        assert!(
            inline_test_module_bodies(source).is_empty(),
            "a declaration owns no body and must not be flagged: {source}"
        );
    }

    let production_body = "pub mod inner {\n    pub const LIMIT: usize = 4;\n}\n";
    assert!(
        inline_test_module_bodies(production_body).is_empty(),
        "a module body that is not gated on the test configuration must not be flagged"
    );

    let commented_out = concat!("#[cfg(test)]\n", "// mod tests {\n");
    assert!(
        inline_test_module_bodies(commented_out).is_empty(),
        "a commented-out body must not be flagged"
    );

    let feature_gated_body = "#[cfg(feature = \"deadlock-detection\")]\nmod detector {\n}\n";
    assert!(
        inline_test_module_bodies(feature_gated_body).is_empty(),
        "a feature-gated body is not a test module"
    );
}

#[test]
fn test_no_inline_test_bodies_outside_area_test_files() {
    let files: Vec<PathBuf> = rs_files_under(&source_root())
        .into_iter()
        .filter(|path| !is_area_test_file(path))
        .collect();
    assert!(
        !files.is_empty(),
        "the scan inspected no production files, so the inline-body guard would be vacuous"
    );
    let violations = collect_tree_violations(&files, inline_test_module_bodies);
    assert!(
        violations.is_empty(),
        "test modules belong in `*_tests.rs` area files:\n{}",
        violations.join("\n")
    );
}
