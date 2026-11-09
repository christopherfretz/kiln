//! Classifying one kiln run of a clang `-verify` test (spec/clang-neg.md
//! "Classification").
//!
//! kiln's own `-verify` (spec/diag.md §11) finds the directives: it scans
//! comments as its preprocessor lexes them, so a directive in an inactive
//! `#if` branch, or of another `-verify=` prefix, never counts, and
//! `@+N`/`@-N`/`@line`/`@file:line`/`@*` and `{{N}}`-style counts resolve
//! exactly as in clang. Before the run the test is copied with every
//! `expected-error` text made unmatchable ([`neutralize`]), so the
//! verifier reports every active `expected-error` occurrence as "expected
//! but not seen" and every error kiln emits as "seen but not expected",
//! each with its file and line. Wording is never compared: only lines.

use crate::libcxx::cluster::error_message;

/// Prepended to every `*-error` directive's text. No diagnostic contains
/// it, so no `expected-error` directive can match.
pub const SENTINEL: &str = "KILN-NEG ";

/// `text` with [`SENTINEL`] inserted after the opening braces of every
/// `<prefix>-error` / `<prefix>-error-re` directive. Only bytes are
/// inserted inside a line, so every line keeps its number. A directive
/// whose `{{` is not on its own line is left alone (it then matches as
/// usual and only counts when kiln reports it with clang's wording).
pub fn neutralize(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    for line in text.split_inclusive('\n') {
        let mut rest = line;
        while let Some(i) = find_directive(rest) {
            let (head, tail) = rest.split_at(i);
            out.push_str(head);
            let Some(open) = tail.find("{{") else {
                rest = tail;
                break;
            };
            let braces = tail[open..].bytes().take_while(|&b| b == b'{').count();
            out.push_str(&tail[..open + braces]);
            out.push_str(SENTINEL);
            rest = &tail[open + braces..];
        }
        out.push_str(rest);
    }
    out
}

/// The end of the first `<prefix>-error[-re]` directive name in `s`: a
/// `-error` preceded by an identifier character and followed by `-re`,
/// `@`, whitespace, a digit, `+` or `{`.
fn find_directive(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut from = 0;
    while let Some(rel) = s[from..].find("-error") {
        let at = from + rel;
        let mut end = at + "-error".len();
        let prefixed = at > 0 && (b[at - 1].is_ascii_alphanumeric() || b[at - 1] == b'_');
        if s[end..].starts_with("-re") {
            end += 3;
        }
        let next = b.get(end).copied();
        let ok = matches!(next, Some(b'@' | b'+' | b'{' | b' ' | b'\t' | b'0'..=b'9'));
        if prefixed && ok {
            return Some(end);
        }
        from = at + 1;
    }
    None
}

/// One entry of a verify report: where (a `None` file is "any file" or a
/// macro location; a `None` line is `Line *`) and the text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub file: Option<String>,
    pub line: Option<u32>,
    pub text: String,
}

/// The parts of kiln's `-verify` report the classification needs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Active `expected-error` occurrences (all unmatched after
    /// [`neutralize`]), with the expected text.
    pub expected_errors: Vec<Entry>,
    /// Errors kiln reported, with kiln's text.
    pub seen_errors: Vec<Entry>,
    /// "no expected directives found".
    pub no_directives: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    ExpectedErrors,
    SeenErrors,
    Other,
}

/// Parses kiln's `-verify` report from its stderr.
pub fn parse_report(stderr: &str) -> Report {
    let mut r = Report::default();
    let mut section: Option<Section> = None;
    for line in stderr.lines() {
        if let Some(entry) = line.strip_prefix("  ")
            && let Some(sec) = section
        {
            if sec == Section::Other {
                continue;
            }
            let e = parse_entry(entry);
            if sec == Section::ExpectedErrors {
                r.expected_errors.push(Entry { text: e.text.replacen(SENTINEL, "", 1), ..e });
            } else {
                r.seen_errors.push(e);
            }
            continue;
        }
        section = None;
        let Some(msg) = error_message(line) else { continue };
        if msg.starts_with("no expected directives found") {
            r.no_directives = true;
        } else if let Some(rest) = msg.strip_prefix('\'')
            && let Some((name, what)) = rest.split_once('\'')
            && what.trim_start().starts_with("diagnostics ")
        {
            let expected = what.contains("expected but not seen");
            section = Some(match (name.ends_with("-error"), expected) {
                (true, true) => Section::ExpectedErrors,
                (true, false) => Section::SeenErrors,
                _ => Section::Other,
            });
        }
    }
    r
}

/// `File X Line N[ (directive at f:l)]: text`, `Line N: text` or
/// `(frontend): text`.
fn parse_entry(entry: &str) -> Entry {
    let e = entry.trim_start();
    if let Some(t) = e.strip_prefix("(frontend): ") {
        return Entry { file: None, line: None, text: t.to_owned() };
    }
    let (file, after) = match e.strip_prefix("File ") {
        Some(rest) => match rest.find(" Line ") {
            Some(i) => {
                let f = &rest[..i];
                (if f == "*" { None } else { Some(f.to_owned()) }, &rest[i + 1..])
            }
            None => (None, rest),
        },
        None => (None, e),
    };
    let after = after.strip_prefix("Line ").unwrap_or(after);
    let num_end = after.find(|c: char| !c.is_ascii_digit() && c != '*').unwrap_or(after.len());
    let line = after[..num_end].parse::<u32>().ok();
    let mut rest = &after[num_end..];
    if let Some(r) = rest.strip_prefix(" (directive at ")
        && let Some(j) = r.find("): ")
    {
        rest = &r[j + 1..];
    }
    let text = rest.strip_prefix(": ").unwrap_or(rest).to_owned();
    Entry { file, line, text }
}

/// Errors the verifier itself emits about malformed directives: not
/// kiln's verdict on the program.
pub fn is_verify_error(text: &str) -> bool {
    const STARTS: &[&str] = &[
        "invalid expected ",
        "cannot find end (",
        "cannot find start (",
        "cannot find start of regex",
        "missing or invalid line number following '@'",
        "invalid range following '-' in expected",
        "use of undefined marker",
        "reference to marker",
        "expected directive cannot follow",
    ];
    STARTS.iter().any(|s| text.starts_with(s))
        || text.starts_with("file '") && text.contains("could not be located in expected")
        || text.contains("-no-diagnostics' directive cannot follow")
}

/// The verdict on one case.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Classified {
    /// Active `expected-error` occurrences.
    pub expected: Vec<Entry>,
    /// Those with no kiln error on their line (MISSED-ERROR).
    pub missed: Vec<Entry>,
    /// kiln's errors, verifier complaints excluded.
    pub kiln_errors: Vec<Entry>,
    /// Verifier complaints about the directives.
    pub verify_errors: u32,
    /// "no expected directives found".
    pub no_directives: bool,
}

impl Classified {
    /// clang expects an error and kiln reported none (ACCEPTS-INVALID).
    pub fn accepts_invalid(&self) -> bool {
        !self.expected.is_empty() && self.kiln_errors.is_empty()
    }
}

/// Classifies a parsed report: an expected occurrence is reported when
/// kiln has an error on the same line of the same file (`Line *`: any
/// line of that file; `File *`: any error; a kiln error at a macro
/// location, printed without a file, matches its line in any file).
pub fn classify(r: &Report) -> Classified {
    let (verify, kiln): (Vec<&Entry>, Vec<&Entry>) =
        r.seen_errors.iter().partition(|e| is_verify_error(&e.text));
    let hit = |x: &Entry| {
        kiln.iter().any(|k| {
            let same_file = x.file.is_none() || k.file.is_none() || x.file == k.file;
            same_file && (x.line.is_none() || x.line == k.line)
        })
    };
    Classified {
        expected: r.expected_errors.clone(),
        missed: r.expected_errors.iter().filter(|x| !hit(x)).cloned().collect(),
        kiln_errors: kiln.into_iter().cloned().collect(),
        verify_errors: u32::try_from(verify.len()).unwrap_or(u32::MAX),
        no_directives: r.no_directives,
    }
}

/// Whether the file declares `<prefix>-no-diagnostics` for one of
/// `prefixes` (textually; the verifier's own status is not printed).
pub fn declares_no_diagnostics(text: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|p| text.contains(&format!("{p}-no-diagnostics")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// spec/clang-neg.md "Classification": only `-error` directive texts
    /// change, and every line keeps its number.
    #[test]
    fn neutralize_inserts_the_sentinel() {
        let src = "int x; // expected-error {{a}} expected-warning {{b}}\n\
                   // cxx11-error@+1 2 {{c}}\n\
                   // expected-error-re {{{d{{.*}}}}}\n\
                   // -Wno-error=foo -Werror expected-note {{e}}\n";
        let out = neutralize(src);
        assert_eq!(
            out,
            "int x; // expected-error {{KILN-NEG a}} expected-warning {{b}}\n\
             // cxx11-error@+1 2 {{KILN-NEG c}}\n\
             // expected-error-re {{{KILN-NEG d{{.*}}}}}\n\
             // -Wno-error=foo -Werror expected-note {{e}}\n"
        );
        assert_eq!(out.lines().count(), src.lines().count());
    }

    const REPORT: &str = "error: 'expected-error' diagnostics expected but not seen: \n  File /s/t.cpp Line 2: KILN-NEG use of undeclared identifier 'x'\n  File /s/t.cpp Line 9 (directive at /s/t.cpp:8): KILN-NEG two\n  File * Line *: KILN-NEG any\nerror: 'expected-error' diagnostics seen but not expected: \n  File /s/t.cpp Line 2: use of undeclared identifier 'x'\n  Line 7: use of undeclared identifier 'zz'\n  File /s/t.cpp Line 4: invalid expected error: oops\nerror: 'expected-warning' diagnostics expected but not seen: \n  File /s/t.cpp Line 10: KILN-NEG blah\n9 errors generated.\n";

    /// spec/clang-neg.md "Classification": report entries and their lines.
    #[test]
    fn report_is_parsed() {
        let r = parse_report(REPORT);
        assert_eq!(r.expected_errors.len(), 3);
        assert_eq!(
            r.expected_errors[1],
            Entry { file: Some("/s/t.cpp".into()), line: Some(9), text: "two".into() }
        );
        assert_eq!(r.expected_errors[2], Entry { file: None, line: None, text: "any".into() });
        assert_eq!(r.seen_errors.len(), 3);
        assert_eq!(
            r.seen_errors[1],
            Entry { file: None, line: Some(7), text: "use of undeclared identifier 'zz'".into() }
        );
        assert!(!r.no_directives);
        assert!(
            parse_report(
                "error: no expected directives found: consider use of 'expected-no-diagnostics'\n"
            )
            .no_directives
        );
    }

    /// spec/clang-neg.md "Classification": line-level hits, verifier
    /// complaints, accepts-invalid.
    #[test]
    fn cases_are_classified() {
        let c = classify(&parse_report(REPORT));
        assert_eq!(c.expected.len(), 3);
        assert_eq!(c.missed.len(), 1);
        assert_eq!(c.missed[0].line, Some(9));
        assert_eq!(c.kiln_errors.len(), 2);
        assert_eq!(c.verify_errors, 1);
        assert!(!c.accepts_invalid());
        let accepted = "error: 'expected-error' diagnostics expected but not seen: \n  File /s/t.cpp Line 2: KILN-NEG e\n1 error generated.\n";
        let c = classify(&parse_report(accepted));
        assert!(c.accepts_invalid());
        assert_eq!(c.missed.len(), 1);
    }
}
