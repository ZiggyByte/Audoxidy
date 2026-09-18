//! Source-scan convention guards for the UI-agnostic boundary.
//!
//! Two guards walk the source tree and assert, with no exclusion list and no
//! self-skip:
//!
//! 1. every file under `src/core/` is free of declarations that import the GUI
//!    toolkit or the GUI module; and
//! 2. no code comment anywhere under `src/` carries a planning identifier.
//!
//! Each guard is a pure predicate over a source string, so it can be exercised
//! with synthetic violating and clean sources; the tree tests are thin wrappers
//! that feed real files to the same predicate. Matching is deliberately narrow:
//! the isolation guard inspects only import declarations, and the identifier
//! guard inspects only comment text and only the identifier forms named by the
//! convention, so standard names, numeric ranges, standards references and
//! file/line references are never touched.

use std::fs;
use std::path::{Path, PathBuf};

/// A single offending source line, located by one-based line number.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Violation {
    /// One-based line number of the offending declaration or comment.
    line: usize,
    /// Human-readable description of what matched.
    text: String,
}

/// Requirement-family prefixes whose numeric identifier form is forbidden in
/// comments.
const REQUIREMENT_FAMILIES: [&str; 22] = [
    "MANIFEST", "OBS", "DSP", "CONC", "MEM", "DATA", "PLAT", "AUDIO", "TEST", "TOOL", "STD",
    "ARCH", "CLEAN", "TICK", "UI", "WIN", "ASSETS", "FONT", "GRID", "COVER", "DB", "FUT",
];

/// Context words that turn a two-digit number pair into a plan identifier.
const PLAN_CONTEXT_WORDS: [&str; 6] = ["plan", "task", "step", "fase", "phase", "paso"];

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

fn is_boundary_before(text: &str, index: usize) -> bool {
    match text[..index].chars().next_back() {
        Some(character) => !is_word_char(character),
        None => true,
    }
}

fn bytes_at(haystack: &[u8], index: usize, needle: &[u8]) -> bool {
    haystack.len() >= index + needle.len() && &haystack[index..index + needle.len()] == needle
}

// ── Guard 1: Core isolation ───────────────────────────────────────

/// Report every import declaration in `source` that names the GUI toolkit or the
/// GUI module.
///
/// A line counts as a declaration only when its first token is `use`, `pub use`
/// or `extern crate`; comment text is stripped first, so a prose mention of a
/// toolkit, a string literal, or a call is not a violation.
fn core_forbidden_imports(source: &str) -> Vec<Violation> {
    let mut violations = Vec::new();
    for (index, raw_line) in source.lines().enumerate() {
        let code = strip_line_comment(raw_line);
        let Some(body) = import_declaration_body(code) else {
            continue;
        };
        let forbidden = if names_crate(body, "iced") {
            Some("iced")
        } else if names_module_path(body, "crate::gui") {
            Some("crate::gui")
        } else {
            None
        };
        if let Some(name) = forbidden {
            violations.push(Violation {
                line: index + 1,
                text: format!("imports `{name}`: {}", code.trim()),
            });
        }
    }
    violations
}

/// Return the imported path of an import declaration, or `None` when the line is
/// not a declaration.
fn import_declaration_body(code: &str) -> Option<&str> {
    let mut rest = code.trim_start();
    if let Some(after_pub) = rest.strip_prefix("pub") {
        if after_pub.starts_with(char::is_whitespace) || after_pub.starts_with('(') {
            rest = after_pub.trim_start();
            if let Some(after_paren) = rest.strip_prefix('(') {
                let close = after_paren.find(')')?;
                rest = after_paren[close + 1..].trim_start();
            }
        } else {
            return None;
        }
    }
    if let Some(after_use) = rest.strip_prefix("use") {
        if after_use.starts_with(char::is_whitespace) {
            return Some(after_use.trim_start());
        }
        return None;
    }
    if let Some(after_crate) = rest.strip_prefix("extern crate")
        && after_crate.starts_with(char::is_whitespace)
    {
        return Some(after_crate.trim_start());
    }
    None
}

/// True when the declaration names `name` as a crate, i.e. the name ends at a
/// path separator or any other non-identifier character.
fn names_crate(body: &str, name: &str) -> bool {
    match body.strip_prefix(name) {
        Some(rest) => match rest.chars().next() {
            Some(character) => !is_word_char(character),
            None => true,
        },
        None => false,
    }
}

/// True when the declaration contains `path` as a whole path-segment sequence.
fn names_module_path(body: &str, path: &str) -> bool {
    let mut offset = 0;
    while let Some(relative) = body[offset..].find(path) {
        let start = offset + relative;
        let end = start + path.len();
        let after_is_boundary = match body[end..].chars().next() {
            Some(character) => !is_word_char(character),
            None => true,
        };
        if is_boundary_before(body, start) && after_is_boundary {
            return true;
        }
        offset = end;
    }
    false
}

// ── Guard 2: no planning identifiers in comments ──────────────────

/// Report every comment in `source` that carries a planning identifier.
///
/// Only comment text is inspected, and only the identifier forms named by the
/// convention: single-letter decision and task identifiers, the explicit
/// requirement-family allowlist, the fix and review markers, the phase word
/// forms, and a two-digit number pair only when the same comment also names a
/// plan, task or step context word.
fn planning_identifiers_in_comments(source: &str) -> Vec<Violation> {
    let mut violations = Vec::new();
    for (index, raw_line) in source.lines().enumerate() {
        let Some(comment) = line_comment_text(raw_line) else {
            continue;
        };
        if let Some(reason) = forbidden_identifier(comment) {
            violations.push(Violation {
                line: index + 1,
                text: reason,
            });
        }
    }
    violations
}

fn line_comment_text(line: &str) -> Option<&str> {
    line.find("//").map(|position| &line[position..])
}

fn strip_line_comment(line: &str) -> &str {
    match line.find("//") {
        Some(position) => &line[..position],
        None => line,
    }
}

fn forbidden_identifier(comment: &str) -> Option<String> {
    if let Some(found) = find_decision_or_task(comment) {
        return Some(format!("decision or task identifier `{found}`"));
    }
    if let Some(found) = find_requirement_family(comment) {
        return Some(format!("requirement-family identifier `{found}`"));
    }
    if let Some(found) = find_fix_marker(comment) {
        return Some(format!("fix marker `{found}`"));
    }
    if let Some(found) = find_review_marker(comment) {
        return Some(format!("review marker `{found}`"));
    }
    if let Some(found) = find_phase_word(comment) {
        return Some(format!("phase word form `{found}`"));
    }
    if let Some(found) = find_plan_pair(comment) {
        return Some(format!("plan identifier `{found}`"));
    }
    None
}

/// A decision or task identifier: `D` or `T`, a hyphen, and at least two digits,
/// with a word boundary before the letter.
fn find_decision_or_task(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index + 4 <= bytes.len() {
        let letter = bytes[index];
        if (letter == b'D' || letter == b'T')
            && bytes[index + 1] == b'-'
            && is_boundary_before(text, index)
        {
            let mut end = index + 2;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end - (index + 2) >= 2 {
                return Some(text[index..end].to_string());
            }
        }
        index += 1;
    }
    None
}

/// A requirement-family identifier from the explicit allowlist followed by a
/// hyphen and at least two digits.
fn find_requirement_family(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    for family in REQUIREMENT_FAMILIES {
        let family_bytes = family.as_bytes();
        let mut index = 0;
        while index + family_bytes.len() < bytes.len() {
            let after_family = index + family_bytes.len();
            if bytes_at(bytes, index, family_bytes)
                && bytes[after_family] == b'-'
                && is_boundary_before(text, index)
            {
                let mut end = after_family + 1;
                while end < bytes.len() && bytes[end].is_ascii_digit() {
                    end += 1;
                }
                if end - (after_family + 1) >= 2 {
                    return Some(text[index..end].to_string());
                }
            }
            index += 1;
        }
    }
    None
}

/// The fix marker: `Fix B` followed by a digit.
fn find_fix_marker(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index + 5 < bytes.len() {
        let after_marker = index + 5;
        if bytes_at(bytes, index, b"Fix B") && bytes[after_marker].is_ascii_digit() {
            return Some(text[index..=after_marker].to_string());
        }
        index += 1;
    }
    None
}

/// The review marker: the review word pair followed by a digit, case-insensitive.
fn find_review_marker(text: &str) -> Option<String> {
    let lowered = text.to_ascii_lowercase();
    let needle = "uat round";
    let mut offset = 0;
    while let Some(relative) = lowered[offset..].find(needle) {
        let start = offset + relative;
        let mut after = start + needle.len();
        while after < lowered.len() && lowered.as_bytes()[after].is_ascii_whitespace() {
            after += 1;
        }
        if after < lowered.len() && lowered.as_bytes()[after].is_ascii_digit() {
            let mut end = after;
            while end < lowered.len() && lowered.as_bytes()[end].is_ascii_digit() {
                end += 1;
            }
            return Some(text[start..end].to_string());
        }
        offset = start + needle.len();
    }
    None
}

/// The phase word followed by whitespace and a digit, case-insensitive, with a
/// word boundary before the word. The boundary keeps the pattern out of longer
/// words such as the DSP prose that names a polyphase filter.
fn find_phase_word(text: &str) -> Option<String> {
    let lowered = text.to_ascii_lowercase();
    for word in ["phase", "fase"] {
        let mut offset = 0;
        while let Some(relative) = lowered[offset..].find(word) {
            let start = offset + relative;
            if is_boundary_before(&lowered, start) {
                let mut after = start + word.len();
                while after < lowered.len() && lowered.as_bytes()[after].is_ascii_whitespace() {
                    after += 1;
                }
                if after < lowered.len() && lowered.as_bytes()[after].is_ascii_digit() {
                    let mut end = after;
                    while end < lowered.len() && lowered.as_bytes()[end].is_ascii_digit() {
                        end += 1;
                    }
                    return Some(text[start..end].to_string());
                }
            }
            offset = start + word.len();
        }
    }
    None
}

/// A zero-padded two-digit number pair, flagged only when the same comment also
/// names a plan, task or step context word.
fn find_plan_pair(text: &str) -> Option<String> {
    let pair = find_two_digit_pair(text)?;
    if has_plan_context(text) {
        Some(pair)
    } else {
        None
    }
}

fn find_two_digit_pair(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    if bytes.len() < 5 {
        return None;
    }
    let mut index = 0;
    while index + 5 <= bytes.len() {
        let is_pair = bytes[index].is_ascii_digit()
            && bytes[index + 1].is_ascii_digit()
            && bytes[index + 2] == b'-'
            && bytes[index + 3].is_ascii_digit()
            && bytes[index + 4].is_ascii_digit();
        if is_pair {
            let before_is_boundary = index == 0 || !is_word_char(bytes[index - 1] as char);
            let after_is_boundary = match text[index + 5..].chars().next() {
                Some(character) => !is_word_char(character),
                None => true,
            };
            if before_is_boundary && after_is_boundary {
                return Some(text[index..index + 5].to_string());
            }
        }
        index += 1;
    }
    None
}

fn has_plan_context(text: &str) -> bool {
    text.to_ascii_lowercase()
        .split(|character: char| !is_word_char(character))
        .any(|word| PLAN_CONTEXT_WORDS.contains(&word))
}

/// Build a comment line without writing a literal comment marker before the
/// fixture text, so this guard file passes its own scan.
fn comment_line(text: &str) -> String {
    format!("// {text}")
}

// ── Tests ─────────────────────────────────────────────────────────

#[test]
fn test_core_guard_flags_toolkit_import() {
    let source = "use iced::Task;\n";
    let violations = core_forbidden_imports(source);
    assert_eq!(
        violations.len(),
        1,
        "an import of the GUI toolkit must be flagged"
    );
    assert_eq!(violations[0].line, 1);

    let extern_crate = "extern crate iced;\n";
    assert!(!core_forbidden_imports(extern_crate).is_empty());
}

#[test]
fn test_core_guard_flags_gui_module_import() {
    for source in [
        "use crate::gui::app::AudoxidyApp;\n",
        "pub use crate::gui::widgets;\n",
        "pub(crate) use crate::gui::Message;\n",
    ] {
        assert!(
            !core_forbidden_imports(source).is_empty(),
            "a GUI-module import must be flagged: {source}"
        );
    }
}

#[test]
fn test_core_guard_flags_grouped_toolkit_import() {
    let source = "use iced::{\n    widget::text,\n    Task,\n};\n";
    let violations = core_forbidden_imports(source);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].line, 1);
}

#[test]
fn test_core_guard_ignores_clean_and_non_declaration_text() {
    let clean = "use std::path::Path;\n\npub fn tick() -> u32 {\n    0\n}\n";
    assert!(core_forbidden_imports(clean).is_empty());

    let prose = "// the front-end owns the GUI toolkit, this layer must not\n";
    assert!(core_forbidden_imports(prose).is_empty());

    let literal = "const HINT: &str = \"use iced::Task;\";\n";
    assert!(core_forbidden_imports(literal).is_empty());

    let similar_name = "use iced_aw::widgets;\n";
    assert!(core_forbidden_imports(similar_name).is_empty());
}

#[test]
fn test_identifier_guard_flags_required_forms() {
    let offenders = [
        comment_line("decision D-01 removes the block"),
        comment_line("task T-07 follow-up"),
        comment_line("two-part T-12-34 reference"),
        comment_line("family STD-03 boundary"),
        comment_line("Fase 04 cleanup"),
        comment_line("phase 12 cleanup"),
        comment_line("Fix B2 regression"),
        comment_line("UAT round 7 note"),
        comment_line("plan 19-03 boundary"),
    ];
    for source in &offenders {
        assert!(
            !planning_identifiers_in_comments(source).is_empty(),
            "must flag: {source}"
        );
    }
}

#[test]
fn test_identifier_guard_ignores_legitimate_technical_text() {
    let legitimate = [
        comment_line("SHA-256 digest"),
        comment_line("AES-256 cipher"),
        comment_line("ISO-8859 encoding"),
        comment_line("range 15-81"),
        comment_line("range 81-96"),
        comment_line("loudness IEC 60268-17"),
        comment_line("see decoder.rs:986-1025"),
        comment_line("see widgets.rs:500-506"),
        comment_line("filtro polyphase 2x"),
    ];
    for source in &legitimate {
        assert!(
            planning_identifiers_in_comments(source).is_empty(),
            "must not flag: {source}"
        );
    }
}

#[test]
fn test_identifier_guard_requires_context_for_number_pairs() {
    let bare = comment_line("chapter 15-81 covers the range");
    assert!(planning_identifiers_in_comments(&bare).is_empty());

    let with_context = comment_line("plan 15-81 covers the range");
    assert!(!planning_identifiers_in_comments(&with_context).is_empty());
}

#[test]
fn test_identifier_guard_scans_trailing_comments() {
    let source = format!("let value = 1; {}", comment_line("task T-07 follow-up"));
    assert!(!planning_identifiers_in_comments(&source).is_empty());
}

#[test]
fn test_core_tree_has_no_forbidden_imports() {
    let files = rs_files_under(&source_root().join("core"));
    assert!(
        !files.is_empty(),
        "the core scan inspected no files, so the isolation guard would be vacuous"
    );
    let violations = collect_tree_violations(&files, core_forbidden_imports);
    assert!(
        violations.is_empty(),
        "core must not import the GUI toolkit or the GUI module:\n{}",
        violations.join("\n")
    );
}

#[test]
fn test_source_tree_has_no_planning_identifiers_in_comments() {
    let files = rs_files_under(&source_root());
    assert!(!files.is_empty(), "the source scan inspected no files");
    let violations = collect_tree_violations(&files, planning_identifiers_in_comments);
    assert!(
        violations.is_empty(),
        "code comments must not carry planning identifiers:\n{}",
        violations.join("\n")
    );
}
