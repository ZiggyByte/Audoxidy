//! Source-scan convention guards for the test layout.
//!
//! Three guards walk the source tree and assert, with no exclusion list and no
//! self-skip:
//!
//! 1. no production file keeps an inline test module: a test module lives in its
//!    own `*_tests.rs` area file and is reached through a declaration, so the
//!    body and its declaration never share a file;
//! 2. every `*_tests.rs` file is named by exactly one declaration that resolves
//!    to it, so a forgotten declaration fails the suite instead of silently
//!    dropping the tests it used to run; and
//! 3. no non-zero numeric layout literal sits at a `padding`, `spacing`,
//!    `width`, `height`, `min_*` or `max_*` method call under `src/gui/`, so a
//!    measurement reaches the widgets from `src/gui/dimensions.rs` by name.
//!
//! The layout guard inspects method-call literals and nothing else: a
//! `Padding { … }` struct literal and a `Length::Fixed(<literal>)` argument are
//! outside it and need a rule of their own. Both shapes are common — 68
//! `Padding { … }` construction sites across 17 files, and 55 non-zero
//! `Length::Fixed(<literal>)` arguments across 7 files out of 194
//! `Length::Fixed` occurrences — so this guard's green result reads "no method
//! call restates a measurement", never "no literal remains".
//!
//! Each guard is a pure predicate over source text and paths, so it is exercised
//! with synthetic violating and clean sources; the tree tests are thin wrappers
//! that feed real files to the same predicates. Matching is deliberately narrow:
//! only a `mod <ident> {` line can be a body, so a declaration ending in `;` —
//! the form every migrated module uses — is structurally impossible to flag, and
//! a non-test module body is left alone. The declaration index resolves paths
//! with the compiler's own rules instead of matching names, because a migrated
//! module can be named `tests` while its file carries the area name.

use std::fs;
use std::path::{Component, Path, PathBuf};

/// The area test-file count recorded when this guard landed: the tree carried 66
/// before it and the guard file itself is the 67th. The floor may rise; a drop
/// is a coverage loss, so the tree test fails rather than shrinking silently.
const AREA_TEST_FILE_FLOOR: usize = 67;

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

// ── Guard 2: every area test file is declared exactly once ────────

/// A module declaration found in a source file.
#[derive(Debug, Clone)]
struct Declaration {
    /// The file that carries the declaration.
    declaring: PathBuf,
    /// The module name on the declaration line.
    module: String,
    /// The `#[path = "…"]` value from the attribute block, when present.
    path_attribute: Option<String>,
}

/// Report every `*_tests.rs` file that is not named by exactly one declaration
/// resolving to it from an accepted owner.
///
/// The index is built from the declaration lines themselves and resolved with
/// the compiler's rules: a module root resolves a natural declaration to a
/// sibling file or a directory's `mod.rs`, a leaf file resolves it to its
/// `<stem>/` subdirectory, and `#[path]` resolves against the declaring file's
/// own directory. Reporting is by resolved path and never by name, so a module
/// named `tests` whose file carries the area name is resolved correctly.
///
/// An accepted owner is the directory the compiler would read the declaration
/// from — the same directory, or the sibling `<stem>.rs` of a natural leaf
/// declaration — or a crate root carrying an explicit `#[path]`.
fn undeclared_area_test_files(sources: &[(PathBuf, String)]) -> Vec<String> {
    let declarations: Vec<Declaration> = sources
        .iter()
        .flat_map(|(path, source)| declarations_in(path, source))
        .collect();

    let mut reports = Vec::new();
    for (path, _) in sources {
        if !is_area_test_file(path) {
            continue;
        }
        let owners: Vec<&Declaration> = declarations
            .iter()
            .filter(|declaration| resolves_to(declaration, path))
            .collect();
        match owners.len() {
            0 => reports.push(format!(
                "{}: no module declaration resolves to this file, so its tests never run",
                path.display()
            )),
            1 => {
                if !accepted_owner(owners[0], path) {
                    reports.push(format!(
                        "{}: declared from {}, which does not own it",
                        path.display(),
                        owners[0].declaring.display()
                    ));
                }
            }
            count => {
                let places = owners
                    .iter()
                    .map(|declaration| declaration.declaring.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                reports.push(format!(
                    "{}: declared {count} times by {places}",
                    path.display()
                ));
            }
        }
    }
    reports
}

/// Collect the declarations of one source file.
fn declarations_in(path: &Path, source: &str) -> Vec<Declaration> {
    let lines: Vec<&str> = source.lines().collect();
    let mut declarations = Vec::new();
    for (index, raw_line) in lines.iter().enumerate() {
        let code = strip_line_comment(raw_line);
        let Some(module) = declaration_name(code) else {
            continue;
        };
        declarations.push(Declaration {
            declaring: path.to_path_buf(),
            module: module.to_string(),
            path_attribute: path_attribute_above(&lines, index),
        });
    }
    declarations
}

/// Return the module name of a `mod <ident>;` line, or `None` when the line is
/// not a declaration.
fn declaration_name(code: &str) -> Option<&str> {
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
    if rest[name_len..].trim_start().starts_with(';') {
        Some(&rest[..name_len])
    } else {
        None
    }
}

/// The `#[path = "…"]` value in the attribute block above line `index`.
fn path_attribute_above(lines: &[&str], index: usize) -> Option<String> {
    let mut cursor = index;
    while cursor > 0 {
        cursor -= 1;
        let line = lines[cursor].trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line.starts_with("#[") {
            if let Some(value) = path_attribute_value(line) {
                return Some(value);
            }
            continue;
        }
        return None;
    }
    None
}

/// The string of a `#[path = "…"]` attribute, or `None` for any other attribute.
fn path_attribute_value(attribute: &str) -> Option<String> {
    let body = attribute.strip_prefix("#[path")?.trim_start();
    let body = body.strip_prefix('=')?.trim_start();
    let body = body.strip_prefix('"')?;
    let close = body.find('"')?;
    Some(body[..close].to_string())
}

/// True when the file is a module root, so its declarations resolve next to it.
fn is_module_root(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("lib.rs" | "main.rs" | "mod.rs")
    )
}

/// The files a declaration may resolve to, following the compiler's rules.
fn declaration_targets(declaration: &Declaration) -> Vec<PathBuf> {
    let Some(base) = declaration.declaring.parent() else {
        return Vec::new();
    };
    if let Some(attribute) = &declaration.path_attribute {
        let relative = base.join(attribute);
        if Path::new(attribute).extension().is_some() {
            return vec![normalize_path(&relative)];
        }
        return vec![normalize_path(&relative.join("mod.rs"))];
    }
    if is_module_root(&declaration.declaring) {
        return vec![
            normalize_path(&base.join(format!("{}.rs", declaration.module))),
            normalize_path(&base.join(&declaration.module).join("mod.rs")),
        ];
    }
    let Some(stem) = declaration
        .declaring
        .file_stem()
        .and_then(|stem| stem.to_str())
    else {
        return Vec::new();
    };
    let directory = base.join(stem);
    vec![
        normalize_path(&directory.join(format!("{}.rs", declaration.module))),
        normalize_path(&directory.join(&declaration.module).join("mod.rs")),
    ]
}

/// True when the declaration resolves to `file`.
fn resolves_to(declaration: &Declaration, file: &Path) -> bool {
    let target = normalize_path(file);
    declaration_targets(declaration)
        .iter()
        .any(|candidate| *candidate == target)
}

/// True when the declaring file is an accepted owner of the declared file.
fn accepted_owner(declaration: &Declaration, file: &Path) -> bool {
    let declared = normalize_path(file);
    let declaring = normalize_path(&declaration.declaring);
    if declaring.parent() == declared.parent() {
        return true;
    }
    let sibling_directory = declaring
        .file_stem()
        .zip(declared.parent().and_then(|parent| parent.file_name()))
        .is_some_and(|(stem, directory)| stem == directory)
        && declaring
            .parent()
            .zip(declared.parent().and_then(|parent| parent.parent()))
            .is_some_and(|(parent, grandparent)| parent == grandparent);
    if sibling_directory {
        return true;
    }
    declaration.path_attribute.is_some()
        && matches!(
            declaring.file_name().and_then(|name| name.to_str()),
            Some("lib.rs" | "main.rs")
        )
}

/// Resolve `.` and `..` components without touching the filesystem, so two
/// spellings of the same location compare equal.
fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

// ── Guard 3: layout measurements come from the token module ───────

/// Method names whose numeric argument is a layout measurement, so the number
/// belongs in `src/gui/dimensions.rs`.
const LAYOUT_METHODS: [&str; 8] = [
    "padding",
    "spacing",
    "width",
    "height",
    "min_width",
    "min_height",
    "max_width",
    "max_height",
];

/// Report every method call in `source` that hands a non-zero numeric literal to
/// a guarded layout method.
///
/// The first argument is read up to its matching parenthesis. A bare numeric
/// literal other than zero, and an all-numeric array with a non-zero element, are
/// violations; a zero value, an all-zero array, a named constant, a `Length::*`
/// variant, an expression and a struct literal are all clean. `.size()` is
/// deliberately absent: it is typography, and several calls through that name are
/// widget sizes rather than font sizes.
fn layout_literals(source: &str) -> Vec<Violation> {
    let mut violations = Vec::new();
    for (index, raw_line) in source.lines().enumerate() {
        let code = strip_line_comment(raw_line);
        for method in LAYOUT_METHODS {
            for open in method_call_offsets(code, method) {
                let Some(argument) = first_argument(&code[open..]) else {
                    continue;
                };
                if is_non_zero_measurement(argument) {
                    violations.push(Violation {
                        line: index + 1,
                        text: format!("`.{method}({argument})` restates a layout measurement"),
                    });
                }
            }
        }
    }
    violations
}

/// Offsets of the opening parenthesis of every `.method(` call in `code`.
fn method_call_offsets(code: &str, method: &str) -> Vec<usize> {
    let needle = format!(".{method}");
    let mut offsets = Vec::new();
    let mut offset = 0;
    while let Some(relative) = code[offset..].find(&needle) {
        let start = offset + relative;
        let after = start + needle.len();
        let rest = &code[after..];
        let trimmed = rest.trim_start();
        if trimmed.starts_with('(') {
            offsets.push(after + (rest.len() - trimmed.len()));
        }
        offset = after;
    }
    offsets
}

/// The text of the first argument of the call whose parenthesis starts `code`, up
/// to the matching closing parenthesis.
fn first_argument(code: &str) -> Option<&str> {
    let open = code.find('(')?;
    let bytes = code.as_bytes();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut index = open;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => in_string = !in_string,
            b'(' if !in_string => depth += 1,
            b')' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(code[open + 1..index].trim());
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

/// True when the argument is a non-zero numeric literal, or an all-numeric array
/// literal with a non-zero element.
fn is_non_zero_measurement(argument: &str) -> bool {
    if let Some(value) = numeric_literal_value(argument) {
        return value != 0.0;
    }
    let Some(inner) = argument
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    else {
        return false;
    };
    let parts: Vec<&str> = inner
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    !parts.is_empty()
        && parts
            .iter()
            .all(|part| numeric_literal_value(part).is_some())
        && parts
            .iter()
            .any(|part| numeric_literal_value(part) != Some(0.0))
}

/// The value of a bare Rust numeric literal, or `None` when the text is anything
/// else — a path, a constant, an expression or a struct literal.
fn numeric_literal_value(text: &str) -> Option<f64> {
    let text = text.trim();
    let body = text
        .strip_suffix("f32")
        .or_else(|| text.strip_suffix("f64"))
        .unwrap_or(text);
    if body.is_empty() {
        return None;
    }
    let mut seen_digit = false;
    let mut seen_dot = false;
    let mut seen_exponent = false;
    for (index, character) in body.char_indices() {
        match character {
            '0'..='9' => seen_digit = true,
            '_' => {}
            '.' if !seen_dot && !seen_exponent => seen_dot = true,
            'e' | 'E' if seen_digit && !seen_exponent => seen_exponent = true,
            '+' | '-'
                if seen_exponent
                    && matches!(body[..index].chars().next_back(), Some('e' | 'E')) => {}
            _ => return None,
        }
    }
    if !seen_digit {
        return None;
    }
    body.replace('_', "").parse::<f64>().ok()
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

/// Build a synthetic source set from `(path, source)` pairs.
fn area_sources(entries: &[(&str, &str)]) -> Vec<(PathBuf, String)> {
    entries
        .iter()
        .map(|(path, source)| (PathBuf::from(path), (*source).to_string()))
        .collect()
}

/// Read every file into `(path, source)` pairs, failing on an unreadable file
/// instead of silently shrinking the inspected tree.
fn read_sources(files: &[PathBuf]) -> Vec<(PathBuf, String)> {
    files
        .iter()
        .map(|path| {
            let source = fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));
            (path.clone(), source)
        })
        .collect()
}

#[test]
fn test_area_file_guard_flags_duplicate_missing_and_foreign_declarations() {
    let duplicate = area_sources(&[
        (
            "src/a/mod.rs",
            "#[cfg(test)]\n#[path = \"foo_tests.rs\"]\nmod first;\n",
        ),
        (
            "src/a/host.rs",
            "#[cfg(test)]\n#[path = \"foo_tests.rs\"]\nmod second;\n",
        ),
        ("src/a/foo_tests.rs", ""),
    ]);
    let reports = undeclared_area_test_files(&duplicate);
    assert_eq!(
        reports.len(),
        1,
        "two declarations of one file must be reported: {reports:?}"
    );
    assert!(
        reports[0].contains("declared 2 times"),
        "the report must name the duplicate declaration: {}",
        reports[0]
    );

    let orphan = area_sources(&[("src/a/orphan_tests.rs", "")]);
    let reports = undeclared_area_test_files(&orphan);
    assert_eq!(
        reports.len(),
        1,
        "a file nothing declares must be reported: {reports:?}"
    );
    assert!(
        reports[0].contains("no module declaration resolves"),
        "the report must say the file is unreachable: {}",
        reports[0]
    );

    let elsewhere = area_sources(&[
        (
            "src/a/mod.rs",
            "#[cfg(test)]\n#[path = \"elsewhere/foo_tests.rs\"]\nmod tests;\n",
        ),
        ("src/a/foo_tests.rs", ""),
    ]);
    let reports = undeclared_area_test_files(&elsewhere);
    assert_eq!(
        reports.len(),
        1,
        "a path attribute pointing at another directory must leave the file undeclared: {reports:?}"
    );

    let foreign = area_sources(&[
        (
            "src/a/mod.rs",
            "#[cfg(test)]\n#[path = \"../b/foo_tests.rs\"]\nmod tests;\n",
        ),
        ("src/b/foo_tests.rs", ""),
    ]);
    let reports = undeclared_area_test_files(&foreign);
    assert_eq!(
        reports.len(),
        1,
        "a declaration from outside the file's directory must be reported: {reports:?}"
    );
    assert!(
        reports[0].contains("does not own it"),
        "the report must name the foreign declaration: {}",
        reports[0]
    );
}

#[test]
fn test_area_file_guard_accepts_every_owner_shape() {
    let shapes = [
        (
            "a module root declaring a sibling area file",
            area_sources(&[
                ("src/a/mod.rs", "#[cfg(test)]\nmod foo_tests;\n"),
                ("src/a/foo_tests.rs", ""),
            ]),
        ),
        (
            "a leaf file declaring an area file whose module name differs",
            area_sources(&[
                (
                    "src/a/host.rs",
                    "#[cfg(test)]\n#[path = \"host_tests.rs\"]\nmod area_tests;\n",
                ),
                ("src/a/host_tests.rs", ""),
            ]),
        ),
        (
            "the crate root declaring a file outside the library directory",
            area_sources(&[
                (
                    "src/lib.rs",
                    "#[cfg(test)]\n#[path = \"gui/widgets_tests.rs\"]\nmod widgets_tests;\n",
                ),
                ("src/gui/widgets_tests.rs", ""),
            ]),
        ),
        (
            "a leaf file resolving its declaration into its own directory",
            area_sources(&[
                ("src/a/host.rs", "#[cfg(test)]\nmod host_tests;\n"),
                ("src/a/host/host_tests.rs", ""),
            ]),
        ),
    ];
    for (label, sources) in shapes {
        let reports = undeclared_area_test_files(&sources);
        assert!(reports.is_empty(), "{label} must be accepted: {reports:?}");
    }
}

#[test]
fn test_every_area_test_file_is_declared_once() {
    let files = rs_files_under(&source_root());
    assert!(!files.is_empty(), "the source scan inspected no files");
    let sources = read_sources(&files);
    let area_files = sources
        .iter()
        .filter(|(path, _)| is_area_test_file(path))
        .count();
    assert!(
        area_files >= AREA_TEST_FILE_FLOOR,
        "the scan inspected {area_files} area test files, below the recorded floor of \
         {AREA_TEST_FILE_FLOOR}: a test file that disappeared is a coverage loss"
    );
    let reports = undeclared_area_test_files(&sources);
    assert!(
        reports.is_empty(),
        "every `*_tests.rs` file must be declared exactly once by an accepted owner:\n{}",
        reports.join("\n")
    );
}

#[test]
fn test_layout_guard_flags_non_zero_literals_at_guarded_calls() {
    let offenders = [
        ("padding(8)", "container().padding(8)\n"),
        ("spacing(2)", "column![].spacing(2)\n"),
        ("width(44)", "svg(icon).width(44)\n"),
        ("height(10.0)", "Space::new().height(10.0)\n"),
        ("min_width(12.0)", "Space::new().min_width(12.0)\n"),
        ("min_height(6)", "Space::new().min_height(6)\n"),
        ("max_width(320)", "Space::new().max_width(320)\n"),
        ("max_height(24.0)", "Space::new().max_height(24.0)\n"),
    ];
    for (label, source) in offenders {
        let violations = layout_literals(source);
        assert_eq!(
            violations.len(),
            1,
            "a non-zero literal at `{label}` must be flagged once: {violations:?}"
        );
        assert_eq!(violations[0].line, 1);
    }

    let zero_then_literal = "container().padding(0).width(24)\n";
    let violations = layout_literals(zero_then_literal);
    assert_eq!(
        violations.len(),
        1,
        "the zero argument must be skipped and the literal after it flagged: {violations:?}"
    );

    let suffixed = "container().padding(8.0f32)\n";
    assert!(
        !layout_literals(suffixed).is_empty(),
        "a suffixed literal is still a literal"
    );

    let array = "container().padding([8.0, 8.0])\n";
    assert!(
        !layout_literals(array).is_empty(),
        "an all-numeric array with a non-zero element must be flagged"
    );
}

#[test]
fn test_layout_guard_ignores_zero_tokens_and_expressions() {
    let clean = [
        "container().padding(0)\n",
        "container().padding(0.0)\n",
        "container().padding([0.0, 0.0])\n",
        "container().padding([0, 0])\n",
        "container().padding(SPACE_MD)\n",
        "container().spacing(theme::GAP)\n",
        "container().width(Length::Fill)\n",
        "container().width(Length::Fixed(8.0))\n",
        "text(label).size(20)\n",
        "container().padding(if wide { 12.0 } else { 8.0 })\n",
        "container().padding(iced::Padding { top: 8.0, right: 22.0 })\n",
        "container().width(icon_button_box(IconSize::Xl))\n",
        "// container().width(44)\n",
    ];
    for source in clean {
        assert!(
            layout_literals(source).is_empty(),
            "must not flag: {source}"
        );
    }
}

#[test]
fn test_gui_has_no_layout_literals() {
    let gui_root = source_root().join("gui");
    let all_files = rs_files_under(&gui_root);
    assert!(
        !all_files.is_empty(),
        "the layout scan inspected no files under `src/gui/`"
    );
    let exempt = gui_root.join("dimensions.rs");
    assert!(
        all_files.contains(&exempt),
        "the exempt token module {} was not in the inspected set, so the exemption proves nothing",
        exempt.display()
    );
    let files: Vec<PathBuf> = all_files
        .into_iter()
        .filter(|path| *path != exempt)
        .collect();
    assert!(
        !files.is_empty(),
        "the layout scan inspected no files besides the exempt token module"
    );
    let violations = collect_tree_violations(&files, layout_literals);
    assert!(
        violations.is_empty(),
        "layout measurements belong in `src/gui/dimensions.rs`:\n{}",
        violations.join("\n")
    );
}
