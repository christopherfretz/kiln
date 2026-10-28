//! Parsing compiler diagnostics from stderr and matching them against `diag:`
//! directives (`spec/harness.md` "Diagnostic matching").
//!
//! Input is clang's text format, which kiln emits too (`spec/diag.md`):
//! `file:line:col: severity: message`, followed by snippet lines that start
//! with whitespace. Lines that are not diagnostics (snippets, "In file included
//! from", "N errors generated.") are ignored.

use std::fmt::Write as _;

use super::directives::{DiagDirective, Severity};

/// Where a diagnostic points.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loc {
    pub file: String,
    pub line: u32,
    pub col: Option<u32>,
}

/// One parsed diagnostic line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// `None` for location-less diagnostics such as `clang++: error: ...`.
    pub loc: Option<Loc>,
    pub severity: Severity,
    /// The message text, including any trailing ` [-Wgroup]`.
    pub message: String,
    /// The original line.
    pub raw: String,
}

/// Parses every diagnostic line in `stderr`, in emission order.
pub fn parse(stderr: &str) -> Vec<Diagnostic> {
    stderr.lines().filter_map(parse_line).collect()
}

const MARKERS: &[(&str, Severity)] = &[
    ("fatal error: ", Severity::Fatal),
    ("error: ", Severity::Error),
    ("warning: ", Severity::Warning),
    ("note: ", Severity::Note),
    ("remark: ", Severity::Remark),
];

fn parse_line(line: &str) -> Option<Diagnostic> {
    // Snippet and caret lines are indented; diagnostics never are.
    if line.is_empty() || line.starts_with(char::is_whitespace) {
        return None;
    }
    // Location-less: `error: ...` at the start of the line.
    for &(m, severity) in MARKERS {
        if let Some(msg) = line.strip_prefix(m) {
            return Some(Diagnostic {
                loc: None,
                severity,
                message: msg.to_owned(),
                raw: line.to_owned(),
            });
        }
    }
    // Otherwise `<prefix>: <severity>: msg`, taking the earliest marker.
    let (pos, mlen, severity) = MARKERS
        .iter()
        .filter_map(|&(m, sev)| {
            let needle = format!(": {m}");
            line.find(&needle).map(|p| (p, needle.len(), sev))
        })
        .min_by_key(|&(p, len, _)| (p, std::cmp::Reverse(len)))?;
    let prefix = &line[..pos];
    let message = line[pos + mlen..].to_owned();
    Some(Diagnostic { loc: parse_loc(prefix), severity, message, raw: line.to_owned() })
}

/// `file:line:col` or `file:line`; anything else (`clang++`) is no location.
fn parse_loc(prefix: &str) -> Option<Loc> {
    let num = |s: &str| -> Option<u32> {
        (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().ok()).flatten()
    };
    let mut parts = prefix.rsplitn(3, ':');
    let last = parts.next()?;
    let mid = parts.next()?;
    if let (Some(col), Some(line), Some(file)) = (num(last), num(mid), parts.next())
        && !file.is_empty()
    {
        return Some(Loc { file: file.to_owned(), line, col: Some(col) });
    }
    let (file, line) = prefix.rsplit_once(':')?;
    let line = num(line)?;
    (!file.is_empty()).then(|| Loc { file: file.to_owned(), line, col: None })
}

/// A primary diagnostic (error/warning/remark/fatal) and the notes that follow
/// it. A note with no preceding primary forms a group on its own.
#[derive(Clone, Debug)]
pub struct Group<'a> {
    pub primary: &'a Diagnostic,
    pub notes: Vec<&'a Diagnostic>,
}

pub fn group(diags: &[Diagnostic]) -> Vec<Group<'_>> {
    let mut out: Vec<Group<'_>> = Vec::new();
    for d in diags {
        match out.last_mut() {
            Some(g) if d.severity == Severity::Note => g.notes.push(d),
            _ => out.push(Group { primary: d, notes: Vec::new() }),
        }
    }
    out
}

/// The outcome of matching directives against diagnostics.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MatchReport {
    /// Indices (into the directive slice) of directive groups' primaries that
    /// found no diagnostic group.
    pub unmatched: Vec<usize>,
    /// Indices (into the diagnostic group list) of groups that strict mode
    /// considers unexpected.
    pub unexpected: Vec<usize>,
}

impl MatchReport {
    pub fn is_ok(&self) -> bool {
        self.unmatched.is_empty() && self.unexpected.is_empty()
    }
}

/// Matches `directives` against `diags`.
///
/// Semantics (normative in `spec/harness.md`):
/// - Directives are grouped like diagnostics: each non-note directive plus the
///   note directives after it.
/// - A directive matches a diagnostic located in the test file (`in_test_file`)
///   with the same line, the same column (if given), an accepting severity,
///   and a message containing the directive's substring.
/// - A directive group matches a diagnostic group when the primaries match and
///   the note directives match the group's test-file notes as an in-order
///   subsequence (exactly, pairwise, under `strict`).
/// - Directive groups are assigned to distinct diagnostic groups by maximum
///   bipartite matching; their relative order is not significant (compilers
///   legitimately differ in emission order across declarations).
/// - Under `strict`, every group whose primary is in the test file must be
///   matched.
pub fn match_diags(
    directives: &[DiagDirective],
    diags: &[Diagnostic],
    in_test_file: &dyn Fn(&str) -> bool,
    strict: bool,
) -> MatchReport {
    let groups = group(diags);
    let dgroups = directive_groups(directives);
    let in_file = |d: &Diagnostic| d.loc.as_ref().is_some_and(|l| in_test_file(&l.file));

    // adjacency[i] = diagnostic groups that directive group i can match.
    let adjacency: Vec<Vec<usize>> = dgroups
        .iter()
        .map(|dg| {
            groups
                .iter()
                .enumerate()
                .filter(|(_, g)| {
                    in_file(g.primary)
                        && directive_matches(&directives[dg[0]], g.primary)
                        && notes_match(directives, &dg[1..], g, &in_file, strict)
                })
                .map(|(j, _)| j)
                .collect()
        })
        .collect();

    // Kuhn's augmenting-path bipartite matching; sizes are tiny.
    let mut owner: Vec<Option<usize>> = vec![None; groups.len()];
    for i in 0..dgroups.len() {
        let mut seen = vec![false; groups.len()];
        augment(i, &adjacency, &mut owner, &mut seen);
    }
    let matched_dirs: Vec<bool> = {
        let mut m = vec![false; dgroups.len()];
        for o in owner.iter().flatten() {
            m[*o] = true;
        }
        m
    };
    let mut report = MatchReport::default();
    for (i, dg) in dgroups.iter().enumerate() {
        if !matched_dirs[i] {
            report.unmatched.push(dg[0]);
        }
    }
    if strict {
        for (j, g) in groups.iter().enumerate() {
            if owner[j].is_none() && in_file(g.primary) {
                report.unexpected.push(j);
            }
        }
    }
    report
}

fn augment(
    i: usize,
    adjacency: &[Vec<usize>],
    owner: &mut [Option<usize>],
    seen: &mut [bool],
) -> bool {
    for &j in &adjacency[i] {
        if seen[j] {
            continue;
        }
        seen[j] = true;
        if owner[j].is_none_or(|k| augment(k, adjacency, owner, seen)) {
            owner[j] = Some(i);
            return true;
        }
    }
    false
}

/// Groups directive indices: `[primary, note, note, ...]`.
fn directive_groups(directives: &[DiagDirective]) -> Vec<Vec<usize>> {
    let mut out: Vec<Vec<usize>> = Vec::new();
    for (i, d) in directives.iter().enumerate() {
        match out.last_mut() {
            Some(g) if d.severity == Severity::Note => g.push(i),
            _ => out.push(vec![i]),
        }
    }
    out
}

fn directive_matches(d: &DiagDirective, diag: &Diagnostic) -> bool {
    let Some(loc) = &diag.loc else {
        return false;
    };
    d.severity.accepts(diag.severity)
        && d.line == loc.line
        && d.col.is_none_or(|c| loc.col == Some(c))
        && diag.message.contains(&d.message)
}

fn notes_match(
    directives: &[DiagDirective],
    note_dirs: &[usize],
    g: &Group<'_>,
    in_file: &dyn Fn(&Diagnostic) -> bool,
    strict: bool,
) -> bool {
    let notes: Vec<&Diagnostic> = g.notes.iter().copied().filter(|n| in_file(n)).collect();
    if strict {
        return notes.len() == note_dirs.len()
            && note_dirs.iter().zip(&notes).all(|(&d, n)| directive_matches(&directives[d], n));
    }
    let mut it = notes.iter();
    note_dirs.iter().all(|&d| it.any(|n| directive_matches(&directives[d], n)))
}

/// Human-readable explanation of a failed match.
pub fn explain(report: &MatchReport, directives: &[DiagDirective], diags: &[Diagnostic]) -> String {
    let mut s = String::new();
    let groups = group(diags);
    if !report.unmatched.is_empty() {
        s.push_str("expected diagnostics not found (with their notes):\n");
        for &i in &report.unmatched {
            let _ = writeln!(
                s,
                "  {}  (directive on line {})",
                directives[i], directives[i].directive_line
            );
            for d in directives[i + 1..].iter().take_while(|d| d.severity == Severity::Note) {
                let _ = writeln!(s, "    {d}");
            }
        }
    }
    if !report.unexpected.is_empty() {
        s.push_str("unexpected diagnostics (strict-diags):\n");
        for &j in &report.unexpected {
            let _ = writeln!(s, "  {}", groups[j].primary.raw);
            for n in &groups[j].notes {
                let _ = writeln!(s, "    {}", n.raw);
            }
        }
    }
    if diags.is_empty() {
        s.push_str("actual diagnostics: (none)\n");
    } else {
        s.push_str("actual diagnostics:\n");
        for d in diags {
            let _ = writeln!(s, "  {}", d.raw);
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLANG: &str = "\
t.cpp:3:2: warning: hi [-W#warnings]
    3 | #warning hi
      |  ^
t.cpp:2:15: error: static assertion failed due to requirement 'f(2) == 5'
    2 | static_assert(f(2)==5);
      |               ^~~~~~~
t.cpp:2:19: note: expression evaluates to '4 == 5'
    2 | static_assert(f(2)==5);
      |               ~~~~^~~
In file included from t.cpp:1:
inc/h.h:4:5: error: cannot initialize a variable
t.cpp:9: remark: no column
clang++: error: linker command failed with exit code 1
fatal error: too many errors emitted
dir:with:colons/x.cpp:7:1: fatal error: 'nope.h' file not found
1 warning and 2 errors generated.
";

    fn d(line: u32, col: Option<u32>, severity: Severity, message: &str) -> DiagDirective {
        DiagDirective { line, col, severity, message: message.into(), directive_line: 1 }
    }

    fn is_t(f: &str) -> bool {
        f == "t.cpp"
    }

    #[test]
    fn parses_clang_output() {
        let ds = parse(CLANG);
        let summary: Vec<_> = ds
            .iter()
            .map(|d| {
                let loc = d.loc.as_ref().map(|l| format!("{}:{}:{:?}", l.file, l.line, l.col));
                (loc, d.severity, d.message.as_str())
            })
            .collect();
        assert_eq!(
            summary,
            [
                (Some("t.cpp:3:Some(2)".into()), Severity::Warning, "hi [-W#warnings]"),
                (
                    Some("t.cpp:2:Some(15)".into()),
                    Severity::Error,
                    "static assertion failed due to requirement 'f(2) == 5'"
                ),
                (
                    Some("t.cpp:2:Some(19)".into()),
                    Severity::Note,
                    "expression evaluates to '4 == 5'"
                ),
                (Some("inc/h.h:4:Some(5)".into()), Severity::Error, "cannot initialize a variable"),
                (Some("t.cpp:9:None".into()), Severity::Remark, "no column"),
                (None, Severity::Error, "linker command failed with exit code 1"),
                (None, Severity::Fatal, "too many errors emitted"),
                (
                    Some("dir:with:colons/x.cpp:7:Some(1)".into()),
                    Severity::Fatal,
                    "'nope.h' file not found"
                ),
            ]
        );
    }

    #[test]
    fn earliest_marker_wins() {
        let ds = parse("a.cpp:1:1: error: unexpected 'note: x'\n");
        assert_eq!(ds[0].severity, Severity::Error);
        assert_eq!(ds[0].message, "unexpected 'note: x'");
    }

    #[test]
    fn grouping() {
        let ds = parse(CLANG);
        let g = group(&ds);
        assert_eq!(g.len(), 7);
        assert_eq!(g[1].notes.len(), 1);
        let orphan = parse("t.cpp:1:1: note: lonely\n");
        assert_eq!(group(&orphan).len(), 1);
    }

    #[test]
    fn matching_is_order_insensitive_across_groups() {
        let ds = parse(CLANG);
        let dirs = [
            d(2, Some(15), Severity::Error, "static assertion failed"),
            d(2, Some(19), Severity::Note, "evaluates to"),
            d(3, Some(2), Severity::Warning, "hi"),
        ];
        assert!(match_diags(&dirs, &ds, &is_t, false).is_ok());
        // Strict: the remark on line 9 is unexpected; the header error is not
        // in the test file and so is ignored.
        let r = match_diags(&dirs, &ds, &is_t, true);
        assert_eq!(r.unmatched, Vec::<usize>::new());
        assert_eq!(r.unexpected, [3]);
        let mut all = dirs.to_vec();
        all.push(d(9, None, Severity::Remark, "no column"));
        assert!(match_diags(&all, &ds, &is_t, true).is_ok());
    }

    #[test]
    fn mismatches() {
        let ds = parse(CLANG);
        for bad in [
            d(2, Some(14), Severity::Error, "static"),
            d(2, Some(15), Severity::Warning, "static"),
            d(2, None, Severity::Error, "dynamic"),
            d(4, Some(5), Severity::Error, "cannot"), // in another file
        ] {
            let r = match_diags(std::slice::from_ref(&bad), &ds, &is_t, false);
            assert_eq!(r.unmatched, [0], "{bad} should not match");
        }
        // Note attached to the wrong primary.
        let dirs = [d(3, Some(2), Severity::Warning, "hi"), d(2, Some(19), Severity::Note, "")];
        assert_eq!(match_diags(&dirs, &ds, &is_t, false).unmatched, [0]);
        // Strict notes are exhaustive.
        let dirs = [d(2, Some(15), Severity::Error, "")];
        let r = match_diags(&dirs, &ds, &is_t, true);
        assert_eq!(r.unmatched, [0]);
    }

    #[test]
    fn error_accepts_fatal_and_same_diag_not_reused() {
        let ds = parse("t.cpp:1:1: fatal error: boom\n");
        assert!(match_diags(&[d(1, Some(1), Severity::Error, "boom")], &ds, &is_t, true).is_ok());
        assert!(match_diags(&[d(1, Some(1), Severity::Fatal, "boom")], &ds, &is_t, true).is_ok());
        let dirs = [d(1, Some(1), Severity::Error, "boom"), d(1, Some(1), Severity::Error, "boom")];
        assert_eq!(match_diags(&dirs, &ds, &is_t, false).unmatched, [1]);
    }

    #[test]
    fn bipartite_assignment_beats_greedy() {
        // Directive A matches both errors on line 1; B only the first.
        // Greedy would give A the first and leave B unmatched.
        let ds = parse("t.cpp:1:1: error: foo bar\nt.cpp:1:1: error: foo\n");
        let dirs = [d(1, Some(1), Severity::Error, "foo"), d(1, Some(1), Severity::Error, "bar")];
        assert!(match_diags(&dirs, &ds, &is_t, true).is_ok());
    }

    #[test]
    fn explain_lists_everything() {
        let ds = parse("t.cpp:1:1: error: real\n");
        let dirs = [d(2, None, Severity::Error, "wanted"), d(2, None, Severity::Note, "n")];
        let r = match_diags(&dirs, &ds, &is_t, true);
        let text = explain(&r, &dirs, &ds);
        assert!(text.contains("2 error \"wanted\""), "{text}");
        assert!(text.contains("    2 note \"n\""), "{text}");
        assert!(text.contains("unexpected diagnostics"), "{text}");
        assert!(text.contains("t.cpp:1:1: error: real"), "{text}");
    }
}
