//! Error clustering for the status page (`spec/libcxx.md` "Clusters").
//!
//! A failing test's diagnostics are reduced to cluster keys: the text after
//! `error: ` with the location dropped, every quoted span (`'…'` or
//! `` `…' ``) replaced by `'…'`, and every run of digits replaced by `N`.
//! The key is prefixed with the stage that failed, so a link error never
//! merges with a compile error of the same text.

use std::collections::BTreeMap;

/// The message of an `error:` (or `fatal error:`) diagnostic line, if the
/// line is one. The location prefix (`file:line:col: `) is dropped. kiln's
/// internal-compiler-error banner is not a diagnostic: [`crash_key`] owns
/// it.
pub fn error_message(line: &str) -> Option<&str> {
    let i = line.find("error: ")?;
    let msg = line[i + "error: ".len()..].trim();
    (!msg.starts_with(ICE)).then_some(msg)
}

/// The head of kiln's crash banner after `kiln: error: ` (spec/driver.md
/// "Internal compiler errors").
const ICE: &str = "internal compiler error: ";

/// Normalizes one diagnostic message into its cluster text.
pub fn normalize(msg: &str) -> String {
    let mut out = String::with_capacity(msg.len());
    let mut chars = msg.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' | '`' => {
                // A quoted span runs to the next `'`. An apostrophe with no
                // closing quote (`isn't`) is kept as is.
                let rest: String = chars.clone().collect();
                if let Some(end) = rest.find('\'') {
                    out.push_str("'…'");
                    for _ in 0..=rest[..end].chars().count() {
                        chars.next();
                    }
                } else {
                    out.push(c);
                }
            }
            '0'..='9' => {
                out.push('N');
                while chars.peek().is_some_and(char::is_ascii_digit) {
                    chars.next();
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// All distinct cluster texts in `stderr`, in order of first appearance;
/// the first one is the test's "first error".
pub fn error_keys(stderr: &str) -> Vec<(String, String)> {
    let mut seen = Vec::<(String, String)>::new();
    for line in stderr.lines() {
        if let Some(m) = error_message(line) {
            let k = normalize(m);
            if !seen.iter().any(|(s, _)| *s == k) {
                seen.push((k, m.to_owned()));
            }
        }
    }
    seen
}

/// The cluster texts of a `-verify` report (spec/libcxx.md
/// ".verify.cpp"): one key per listed entry, `verify: <level> <expected
/// but not seen|seen but not expected>: <normalized text>`, in order of
/// first appearance. Entries are the indented lines after a
/// `'<prefix>-<level>' diagnostics ...:` header; the `File X Line N` (and
/// `(directive at ...)`) prefix is dropped. Empty when `stderr` is not a
/// verify report.
pub fn verify_keys(stderr: &str) -> Vec<(String, String)> {
    let mut seen = Vec::<(String, String)>::new();
    let mut section: Option<String> = None;
    for line in stderr.lines() {
        if let Some(m) = error_message(line) {
            section = verify_section(m);
            continue;
        }
        let Some(sec) = &section else { continue };
        let Some(entry) = line.strip_prefix("  ") else {
            section = None;
            continue;
        };
        let text = verify_entry_text(entry);
        let k = format!("{sec}: {}", normalize(text));
        if !seen.iter().any(|(s, _)| *s == k) {
            seen.push((k, text.to_owned()));
        }
    }
    seen
}

/// `'expected-error' diagnostics expected but not seen:` to
/// `verify: error expected but not seen`.
fn verify_section(msg: &str) -> Option<String> {
    let rest = msg.strip_prefix('\'')?;
    let (name, rest) = rest.split_once('\'')?;
    let level = name.rsplit('-').next()?;
    let what = rest.trim().strip_prefix("diagnostics ")?.strip_suffix(':')?;
    Some(format!("verify: {level} {what}"))
}

/// The message of one report entry: after `File X Line N[ (directive at
/// f:l)]: ` or `(frontend): `.
fn verify_entry_text(entry: &str) -> &str {
    let e = entry.trim_start();
    if let Some(m) = e.strip_prefix("(frontend): ") {
        return m;
    }
    let after_line = e.find("Line ").map_or(e, |i| &e[i..]);
    let after_dir = match after_line.find(" (directive at ") {
        Some(i) if after_line[..i].bytes().skip(5).all(|b| b.is_ascii_digit()) => {
            after_line[i..].find("): ").map_or(after_line, |j| &after_line[i + j + 1..])
        }
        _ => after_line,
    };
    after_dir.split_once(": ").map_or(e, |(_, m)| m)
}

/// One row of the clusters table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cluster {
    /// Tests whose first error is this cluster.
    pub first: usize,
    /// Tests with this cluster anywhere in their diagnostics.
    pub any: usize,
    /// A test that shows it (the first, in run order, whose first error it
    /// is, else the first that has it at all).
    pub example: String,
    /// The raw message from `example`.
    pub raw: String,
    example_is_first: bool,
}

/// Cluster counts over all failing tests.
#[derive(Clone, Debug, Default)]
pub struct Clusters {
    map: BTreeMap<String, Cluster>,
}

impl Clusters {
    /// Records one failing test: `keys` are its `(cluster, raw)` pairs in
    /// order (first error first), each already stage-prefixed.
    pub fn add(&mut self, test: &str, keys: &[(String, String)]) {
        for (i, (k, raw)) in keys.iter().enumerate() {
            let c = self.map.entry(k.clone()).or_default();
            c.any += 1;
            if i == 0 {
                c.first += 1;
            }
            if c.example.is_empty() || i == 0 && !c.example_is_first {
                test.clone_into(&mut c.example);
                c.raw.clone_from(raw);
                c.example_is_first = i == 0;
            }
        }
    }

    /// Clusters ordered by first-error count, then any-count, then text.
    pub fn ranked(&self) -> Vec<(&str, &Cluster)> {
        let mut v: Vec<(&str, &Cluster)> = self.map.iter().map(|(k, c)| (k.as_str(), c)).collect();
        v.sort_by(|a, b| b.1.first.cmp(&a.1.first).then(b.1.any.cmp(&a.1.any)).then(a.0.cmp(b.0)));
        v
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }
}

/// Stage-prefixes keys: `compile: …`.
pub fn staged(stage: &str, keys: Vec<(String, String)>) -> Vec<(String, String)> {
    keys.into_iter().map(|(k, raw)| (format!("{stage}: {k}"), raw)).collect()
}

/// The cluster of a compiler crash: the panic location's file and the
/// normalized panic message. kiln's banner (spec/driver.md "Internal
/// compiler errors") gives the message on its `internal compiler error: `
/// line and the location on a later `note: in compiler source <loc>
/// (thread '…')` line; Rust's default report (`thread 'main' panicked at
/// src/x.rs:1:2:` followed by the message line) is still recognized.
pub fn crash_key(stderr: &str) -> Option<(String, String)> {
    let mut lines = stderr.lines();
    while let Some(l) = lines.next() {
        if let Some(i) = l.find(ICE) {
            let msg = l[i + ICE.len()..].trim();
            let loc = lines
                .clone()
                .find_map(|n| {
                    let j = n.find("note: in compiler source ")?;
                    let rest = &n[j + "note: in compiler source ".len()..];
                    Some(rest.split(" (thread").next().unwrap_or(rest).trim())
                })
                .unwrap_or("?");
            let file = loc.split(':').next().unwrap_or(loc);
            return Some((
                format!("crash: panicked at {file}: {}", normalize(msg)),
                format!("panicked at {loc}: {msg}"),
            ));
        }
        if let Some(i) = l.find("panicked at ") {
            let loc = l[i + "panicked at ".len()..].trim_end_matches(':');
            let file = loc.split(':').next().unwrap_or(loc);
            let msg = lines.next().unwrap_or("").trim();
            return Some((
                format!("crash: panicked at {file}: {}", normalize(msg)),
                format!("panicked at {loc}: {msg}"),
            ));
        }
        if l.contains("PLEASE submit a bug report") {
            return Some(("crash: LLVM crash".to_owned(), l.trim().to_owned()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// spec/libcxx.md ".verify.cpp": one key per report entry.
    #[test]
    fn verify_report_keys() {
        let report = "error: 'expected-error' diagnostics expected but not seen: \n  File /a/t.cpp Line 12: call to deleted function 'f'\n  File /a/t.cpp Line 9 (directive at /a/t.cpp:30): call to deleted function 'g'\nerror: 'expected-warning' diagnostics seen but not expected: \n  File /a/t.cpp Line 3: unused variable 'x'\n  (frontend): odd: thing\n3 errors generated.\n";
        let keys: Vec<String> = verify_keys(report).into_iter().map(|(k, _)| k).collect();
        assert_eq!(
            keys,
            [
                "verify: error expected but not seen: call to deleted function '…'",
                "verify: warning seen but not expected: unused variable '…'",
                "verify: warning seen but not expected: odd: thing",
            ]
        );
        assert_eq!(verify_keys("t.cpp:1:2: error: boom\n"), Vec::new());
    }

    #[test]
    fn normalization() {
        let cases: &[(&str, &str)] = &[
            (
                "use of undeclared identifier '__PRETTY_FUNCTION__'",
                "use of undeclared identifier '…'",
            ),
            ("no member named 'value' in 'std::is_same<int, long>'", "no member named '…' in '…'"),
            ("'asm' statements is not supported yet", "'…' statements is not supported yet"),
            ("expected `;' after expression", "expected '…' after expression"),
            ("too many arguments (3 for 12)", "too many arguments (N for N)"),
            ("isn't handled", "isn't handled"),
            (
                "constexpr evaluation exceeded 1048576 steps",
                "constexpr evaluation exceeded N steps",
            ),
        ];
        for &(msg, want) in cases {
            assert_eq!(normalize(msg), want, "{msg}");
        }
    }

    #[test]
    fn messages() {
        assert_eq!(
            error_message("/x/test_macros.h:294:5: error: 'asm' statements is not supported yet"),
            Some("'asm' statements is not supported yet")
        );
        assert_eq!(
            error_message("a.cpp:1:1: fatal error: 'x.h' file not found"),
            Some("'x.h' file not found")
        );
        assert_eq!(error_message("a.cpp:1:1: warning: unused"), None);
        assert_eq!(
            error_message("/usr/bin/ld: error: undefined symbol: foo"),
            Some("undefined symbol: foo")
        );
        let stderr = "a.cpp:1:1: error: no type named 'x'\nnote: here\nb.h:2:2: error: no type named 'y'\nc.h:3:3: error: other 7\n";
        let keys = error_keys(stderr);
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].0, "no type named '…'");
        assert_eq!(keys[0].1, "no type named 'x'");
        assert_eq!(keys[1].0, "other N");
    }

    #[test]
    fn counting() {
        let mut c = Clusters::default();
        let k = |v: &[&str]| {
            staged("compile", v.iter().map(|s| (normalize(s), (*s).to_owned())).collect())
        };
        c.add("t1", &k(&["a 'x'", "b"]));
        c.add("t2", &k(&["b", "a 'y'"]));
        c.add("t3", &k(&["b"]));
        c.add("t4", &staged("link", vec![("b".into(), "b".into())]));
        let r = c.ranked();
        assert_eq!(c.len(), 3);
        assert_eq!(r[0].0, "compile: b");
        assert_eq!((r[0].1.first, r[0].1.any), (2, 3));
        // The example prefers a test whose first error is the cluster.
        assert_eq!(r[0].1.example, "t2");
        assert_eq!(r[1].0, "compile: a '…'");
        assert_eq!((r[1].1.first, r[1].1.any, r[1].1.example.as_str()), (1, 2, "t1"));
        assert_eq!(r[2].0, "link: b");
    }

    #[test]
    fn crashes() {
        let s = "thread 'main' panicked at crates/kiln_sema/src/expr.rs:812:9:\nindex out of bounds: the len is 3 but the index is 7\nnote: run with RUST_BACKTRACE=1";
        assert_eq!(
            crash_key(s).unwrap().0,
            "crash: panicked at crates/kiln_sema/src/expr.rs: index out of bounds: the len is N but the index is N"
        );
        assert_eq!(
            crash_key(s).unwrap().1,
            "panicked at crates/kiln_sema/src/expr.rs:812:9: index out of bounds: the len is 3 but the index is 7"
        );
        assert_eq!(crash_key("error: x").map(|k| k.0), None);
        // kiln's own banner (spec/driver.md "Internal compiler errors").
        let ice = "a.cpp:1:1: error: x\nkiln++: error: internal compiler error: index 7 out of 3\n\
                   kiln++: note: while compiling 'a.cpp' (phase: sema)\n\
                   kiln++: note: in compiler source crates/kiln_sema/src/expr.rs:812:9 (thread 'kiln')\n";
        assert_eq!(
            crash_key(ice).unwrap(),
            (
                "crash: panicked at crates/kiln_sema/src/expr.rs: index N out of N".to_owned(),
                "panicked at crates/kiln_sema/src/expr.rs:812:9: index 7 out of 3".to_owned()
            )
        );
        let keys: Vec<String> = error_keys(ice).into_iter().map(|k| k.0).collect();
        assert_eq!(keys, ["x"]);
        assert_eq!(crash_key("PLEASE submit a bug report to ...").unwrap().0, "crash: LLVM crash");
    }
}
