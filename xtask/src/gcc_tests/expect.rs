//! The diagnostics a g++.dg test expects (`dg-error`, `dg-warning`,
//! `dg-message`, `dg-bogus`, `dg-regexp`, `dg-excess-errors`,
//! `dg-prune-output`) and the judgement of a compiler's diagnostics against
//! them (spec/harness.md "GCC testsuites", "Diagnostic matching"). Only the
//! line and the severity are matched: a test's regexps are GCC's wording and
//! kiln's wording is clang's, so the message text is never compared; a
//! regexp is only read for the severity prefix it names (GCC's
//! `lib/gcc-dg.exp` gives `dg-message` none, so its regexp may match an
//! error, a warning or a note).

use super::dg::Directive;
use super::outcome::{Class, Verdict};
use super::target::Target;
use super::tcl::Word;

/// The severity a directive names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sev {
    Error,
    Warning,
    /// `dg-message`: any diagnostic, notes included; never required.
    Message,
    /// `dg-bogus`: a diagnostic that must not appear.
    Bogus,
}

/// One applicable expectation of a test.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expect {
    /// The line the diagnostic is expected on; 0 for "no line" (anywhere).
    pub line: usize,
    pub sev: Sev,
    /// GCC's testsuite marks this expectation xfail (GCC does not meet
    /// it): it is neither required nor held against an error on its line.
    pub xfail: bool,
    /// The directive's regexp (GCC's wording; only shown, never matched).
    pub pattern: String,
}

/// What a test expects of one case.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Expected {
    pub expects: Vec<Expect>,
    /// `dg-excess-errors`, or a `dg-prune-output` that may match an error
    /// line, applies: errors on lines nothing expects are not held against
    /// the compiler (a prune pattern is GCC's wording, which cannot be
    /// applied to kiln's).
    pub excess_ok: bool,
    /// The lines (0: anywhere) of the non-xfail `dg-message`s whose regexp
    /// names no severity, each with the directive as shown: GCC's output
    /// holds a diagnostic of a kind the file does not say there.
    pub any_kind: Vec<(usize, String)>,
    /// The first directive that lets GCC's output hold errors at lines the
    /// file does not give: `dg-excess-errors`, a `dg-prune-output` that
    /// may match an error line, or a `dg-regexp` naming no severity.
    pub unlocated: Option<String>,
    /// The case links (`dg-do link` or `run`): GCC must compile it, so an
    /// error nothing requires is never one the file may expect.
    pub links: bool,
    /// The first non-xfail `dg-error`, or `dg-message` naming an error or
    /// no severity, with line 0, as shown: in a case that links, a clean
    /// compile leaves it to the link's output (an expected link error).
    pub lineless: Option<String>,
}

impl Expected {
    /// Whether an error is required somewhere (the test is negative).
    pub fn negative(&self) -> bool {
        self.expects.iter().any(|e| e.sev == Sev::Error && !e.xfail)
    }

    /// The link error the case expects (the directive, as shown): it
    /// links, the compile reported no error and no diagnostic without a
    /// line (which could meet the directive), and a [`Self::lineless`]
    /// directive is left for the link's output to meet.
    pub fn link_error(&self, diags: &[Diag]) -> Option<&str> {
        if !self.links || diags.iter().any(|d| d.sev == Sev::Error || d.line == 0) {
            return None;
        }
        self.lineless.as_deref()
    }

    /// The regexp of the first required error on `line` (any line for
    /// `None`), shortened, for a failure's detail.
    fn pattern_at(&self, line: Option<usize>) -> String {
        let p = self
            .expects
            .iter()
            .find(|e| e.sev == Sev::Error && !e.xfail && line.is_none_or(|l| e.line == l))
            .map_or("", |e| e.pattern.as_str());
        let mut s: String = p.chars().take(80).collect();
        if s.len() < p.len() {
            s.push('…');
        }
        s
    }

    /// The lines an error must appear on (line 0: anywhere), sorted.
    fn required(&self) -> Vec<usize> {
        let mut v: Vec<usize> = self
            .expects
            .iter()
            .filter(|e| e.sev == Sev::Error && !e.xfail)
            .map(|e| e.line)
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Why an error the test does not require may still be one GCC's
    /// output holds, so that the file does not decide the case: `errors`
    /// (none: the compile failed without an error line) all on lines of
    /// `any_kind` messages, or anywhere under an `unlocated` directive.
    /// `None` when the file decides: a link or run case, or an error
    /// elsewhere.
    fn undecided(&self, errors: &[&Diag]) -> Option<String> {
        if self.links {
            return None;
        }
        if let Some(u) = &self.unlocated {
            return Some(u.clone());
        }
        if let Some((_, d)) = self.any_kind.iter().find(|(l, _)| *l == 0) {
            return Some(d.clone());
        }
        let first = errors.first()?;
        let on = |d: &Diag| self.any_kind.iter().find(|(l, _)| *l == d.line);
        if errors.iter().all(|d| on(d).is_some()) {
            return on(first).map(|(l, d)| format!("{d} at L{l}"));
        }
        None
    }

    /// Whether an error on `line` is accounted for: an error (xfail or
    /// not), a warning (a severity difference GCC's `-pedantic-errors` /
    /// `-Werror=` flags can make) or a message is expected there, or an
    /// error or message is expected with no line (`DejaGnu` then matches the
    /// regexp on every line of the output, and kiln's wording cannot say
    /// which of its errors that is).
    fn allows(&self, line: usize) -> bool {
        self.expects.iter().any(|e| {
            (e.line == line && (e.sev != Sev::Bogus || e.xfail))
                || (e.line == 0 && matches!(e.sev, Sev::Error | Sev::Message))
        })
    }
}

/// The line argument of a diagnostic directive on line `at`: `.`, `.-N`,
/// `.+N`, or a number (0: no line). `None` for anything else (a
/// `dg-line` name, which [`expectations`] resolves).
pub fn line_ref(arg: &str, at: usize) -> Option<usize> {
    let a = arg.trim();
    if a.is_empty() || a == "." {
        return Some(at);
    }
    if let Some(off) = a.strip_prefix(".-") {
        return at.checked_sub(off.trim().parse().ok()?);
    }
    if let Some(off) = a.strip_prefix(".+") {
        return at.checked_add(off.trim().parse().ok()?);
    }
    a.parse().ok()
}

/// The applicable expectations of `dirs` at `target` (the case's C++
/// standard and option set): `dg-<kind> <regexp> [<comment> [{ target|xfail
/// <sel> } [<line>]]]`. A non-matching `target` drops the expectation; a
/// matching `xfail` marks it. `Err` for a selector or line it cannot read
/// (the caller skips the case with that reason).
pub fn expectations(dirs: &[Directive], target: &Target) -> Result<Expected, String> {
    let mut out = Expected::default();
    // `{ dg-line <name> }` names its line for later line arguments.
    let named: Vec<(&str, usize)> = dirs
        .iter()
        .filter(|d| d.name == "dg-line")
        .filter_map(|d| d.args.first().map(|w| (w.text().trim(), d.line)))
        .collect();
    for d in dirs {
        let sev = match d.name.as_str() {
            "dg-error" => Sev::Error,
            "dg-warning" => Sev::Warning,
            "dg-message" => Sev::Message,
            "dg-bogus" => Sev::Bogus,
            "dg-excess-errors" => {
                if selector_applies(target, d.args.get(1), &d.name)?.is_some() {
                    out.excess_ok = true;
                    let pattern = d.args.first().map_or("", Word::text);
                    out.unlocated.get_or_insert_with(|| shown(&d.name, pattern));
                }
                continue;
            }
            // `lib/gcc-dg.exp` removes every output line holding a match
            // before the excess-errors check: only a pattern that may
            // match an error line can hide one (WP-1617).
            "dg-prune-output" => {
                if selector_applies(target, d.args.get(1), &d.name)?.is_some()
                    && let Some(w) = d.args.first()
                    && prune_may_match_error(w)
                {
                    out.excess_ok = true;
                    out.unlocated.get_or_insert_with(|| shown(&d.name, w.text()));
                }
                continue;
            }
            // A regexp over the whole output, no line (`lib/gcc-defs.exp`):
            // naming an error, it requires one somewhere.
            "dg-regexp" => {
                let pattern = d.args.first().map_or("", Word::text);
                match severity_named(pattern) {
                    Some(Named::Error) => out.expects.push(Expect {
                        line: 0,
                        sev: Sev::Error,
                        xfail: false,
                        pattern: pattern.trim().to_owned(),
                    }),
                    Some(Named::Other) => {}
                    None => {
                        out.unlocated.get_or_insert_with(|| shown(&d.name, pattern));
                    }
                }
                continue;
            }
            _ => continue,
        };
        let Some(state) = selector_applies(target, d.args.get(2), &d.name)? else { continue };
        let line = match d.args.get(3) {
            None => d.line,
            Some(w) => line_ref(w.text(), d.line)
                .or_else(|| named.iter().find(|(n, _)| *n == w.text().trim()).map(|&(_, l)| l))
                .ok_or_else(|| format!("{}: bad line `{}`", d.name, w.text()))?,
        };
        let pattern = d.args.first().map_or("", Word::text).trim().to_owned();
        // `dg-message` takes the severity its regexp names, if any.
        let named = severity_named(&pattern);
        let sev = match (sev, named) {
            (Sev::Message, Some(Named::Error)) => Sev::Error,
            (Sev::Message, None) if !state => {
                out.any_kind.push((line, shown(&d.name, &pattern)));
                Sev::Message
            }
            (s, _) => s,
        };
        if line == 0 && !state && (sev == Sev::Error || (sev == Sev::Message && named.is_none())) {
            out.lineless.get_or_insert_with(|| shown(&d.name, &pattern));
        }
        out.expects.push(Expect { line, sev, xfail: state, pattern });
    }
    Ok(out)
}

/// The severity a directive's regexp names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Named {
    /// `error:`, `fatal error:` (GCC's `gcc_error_prefix`, `(fatal
    /// )?error:`) or `sorry, unimplemented`.
    Error,
    /// `warning:` or `note:` and no error.
    Other,
}

/// The severity a `dg-message` or `dg-regexp` pattern names, `None` for
/// none (or both kinds).
fn severity_named(pattern: &str) -> Option<Named> {
    let p = pattern.replace('\\', "");
    let error = p.contains("error:") || p.contains("sorry, unimplemented");
    let other = p.contains("warning:") || p.contains("note:");
    match (error, other) {
        (true, false) => Some(Named::Error),
        (false, true) => Some(Named::Other),
        _ => None,
    }
}

/// Whether a `dg-prune-output` pattern may match an error line of GCC's
/// output (`<file>:<line>:<col>: error: <text>`; `lib/gcc-dg.exp`
/// removes each line holding a match of `[^\n]*<p>[^\n]*`), decided from
/// the pattern alone (spec/harness.md "Severities the file names"). It
/// cannot only when the pattern names no error (`error`, `sorry`) and
/// every match must hold `warning:` or `note:`: the pattern has no `|`,
/// and outside groups and bracket expressions it has `warning:` or
/// `note:` as a run of literal characters none of which a `*`, `?` or
/// `{m,n}` makes optional (GCC's error lines never hold either word with
/// its colon after the severity). Anything else, an empty pattern, a
/// quoted word with a Tcl substitution (`[` or `$`), or an escape the
/// reader does not know, may match one.
fn prune_may_match_error(w: &Word) -> bool {
    let Some(re) = tcl_substituted(w) else { return true };
    if re.is_empty() || re.contains('|') || re.contains("error") || re.contains("sorry") {
        return true;
    }
    !literal_runs(&re)
        .is_some_and(|runs| runs.iter().any(|r| r.contains("warning:") || r.contains("note:")))
}

/// The text Tcl hands the directive: a braced word as written; a quoted
/// or bare one with its backslash substitutions (`\n`, `\t`, `\\`, `\X`
/// for punctuation X). `None` for a command or variable substitution or
/// an unknown escape.
fn tcl_substituted(w: &Word) -> Option<String> {
    let raw = match w {
        Word::Braced(s) => return Some(s.clone()),
        Word::Quoted(s) | Word::Bare(s) => s,
    };
    let mut out = String::new();
    let mut it = raw.chars();
    while let Some(c) = it.next() {
        match c {
            '[' | '$' => return None,
            '\\' => match it.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                c if c.is_ascii_alphanumeric() => return None,
                c => out.push(c),
            },
            c => out.push(c),
        }
    }
    Some(out)
}

/// The runs of literal characters every match of the regexp `re` (Tcl's
/// ARE, without `|`) holds: characters outside groups and bracket
/// expressions, split at anything else, without a character a `*`, `?`
/// or bound makes optional. `None` for a form the reader does not know
/// (a director `***`, an embedded option `(?`, unbalanced brackets).
fn literal_runs(re: &str) -> Option<Vec<String>> {
    if re.starts_with("***") || re.contains("(?") {
        return None;
    }
    let c: Vec<char> = re.chars().collect();
    let mut runs = Vec::new();
    let mut cur = String::new();
    let mut depth = 0usize;
    let mut i = 0;
    let end = |cur: &mut String, runs: &mut Vec<String>| {
        if !cur.is_empty() {
            runs.push(std::mem::take(cur));
        }
    };
    while i < c.len() {
        let ch = c[i];
        i += 1;
        // The atom just read, if it is a literal character (pushed to
        // `cur` below only at depth 0).
        let lit = match ch {
            '(' => {
                depth += 1;
                None
            }
            ')' => {
                depth = depth.checked_sub(1)?;
                None
            }
            '[' => {
                // A bracket expression: `]` first (after `^`) is literal.
                if c.get(i) == Some(&'^') {
                    i += 1;
                }
                if c.get(i) == Some(&']') {
                    i += 1;
                }
                while c.get(i)? != &']' {
                    if c[i] == '[' && matches!(c.get(i + 1), Some(':' | '.' | '=')) {
                        let close = c[i + 1];
                        i += 2;
                        while !(c.get(i)? == &close && c.get(i + 1) == Some(&']')) {
                            i += 1;
                        }
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                i += 1;
                None
            }
            '\\' => {
                let e = *c.get(i)?;
                i += 1;
                (!e.is_ascii_alphanumeric()).then_some(e)
            }
            '.' | '^' | '$' | '*' | '+' | '?' | '{' | '}' => None,
            ch => Some(ch),
        };
        // A quantifier after the atom: `*`, `?` and a bound make it
        // optional (`{1}`, `{1,}` would not, but the reader does not
        // tell them apart); `+` keeps it but ends the run.
        let quant = c.get(i).copied();
        match quant {
            Some('*' | '?' | '{') => {
                if quant == Some('{') {
                    while c.get(i)? != &'}' {
                        i += 1;
                    }
                }
                i += 1;
                if c.get(i) == Some(&'?') {
                    i += 1;
                }
                end(&mut cur, &mut runs);
            }
            Some('+') => {
                if let (Some(l), 0) = (lit, depth) {
                    cur.push(l);
                }
                i += 1;
                if c.get(i) == Some(&'?') {
                    i += 1;
                }
                end(&mut cur, &mut runs);
            }
            _ => match (lit, depth) {
                (Some(l), 0) => cur.push(l),
                _ => end(&mut cur, &mut runs),
            },
        }
    }
    if depth != 0 {
        return None;
    }
    end(&mut cur, &mut runs);
    Some(runs)
}

/// A directive as a failure detail shows it: `dg-message ""`.
fn shown(name: &str, pattern: &str) -> String {
    let mut p: String = pattern.trim().chars().take(40).collect();
    if p.len() < pattern.trim().len() {
        p.push('…');
    }
    format!("{name} \"{p}\"")
}

/// `None`: a `{ target <sel> }` that does not match; `Some(xfail)`
/// otherwise (`true` for a matching `{ xfail <sel> }`).
fn selector_applies(
    target: &Target,
    arg: Option<&Word>,
    what: &str,
) -> Result<Option<bool>, String> {
    let Some(a) = arg else { return Ok(Some(false)) };
    match target.keyword_selector(a.text()) {
        Some((kw, Ok(m))) => Ok(match (kw.as_str(), m) {
            ("xfail", m) => Some(m),
            (_, true) => Some(false),
            (_, false) => None,
        }),
        Some((_, Err(e))) => Err(format!("{what}: {e}")),
        None => Ok(Some(false)),
    }
}

/// One diagnostic a compiler printed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diag {
    /// 0 when it has no location.
    pub line: usize,
    pub sev: Sev,
    /// The text from the severity on (`error: ...`).
    pub text: String,
}

/// The errors and warnings of a compiler's stderr, in GCC's and clang's
/// shared format `<file>:<line>[:<col>]: <severity>: <text>`. The file is
/// not looked at, as `DejaGnu` does not: a test expects an error in a
/// header it includes by that header's line (line 0: no location).
/// `sorry, unimplemented` is an error.
pub fn diagnostics(stderr: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    for l in stderr.lines() {
        let Some((sev, at)) = [
            (Sev::Error, ": fatal error: "),
            (Sev::Error, ": error: "),
            (Sev::Error, ": sorry, unimplemented: "),
            (Sev::Warning, ": warning: "),
        ]
        .into_iter()
        .filter_map(|(s, k)| l.find(k).map(|i| (s, i)))
        .min_by_key(|&(_, i)| i) else {
            if let Some(t) = l.strip_prefix("error: ").or_else(|| l.strip_prefix("fatal error: ")) {
                out.push(Diag { line: 0, sev: Sev::Error, text: format!("error: {t}") });
            }
            continue;
        };
        let text = l[at + 2..].to_owned();
        out.push(Diag { line: location_line(&l[..at]), sev, text });
    }
    out
}

/// The line of `<file>:<line>[:<col>]` (0 when there is none).
fn location_line(loc: &str) -> usize {
    let Some((rest, last)) = loc.rsplit_once(':') else { return 0 };
    let Ok(n) = last.parse::<usize>() else { return 0 };
    match rest.rsplit_once(':').map(|(_, l)| l.parse::<usize>()) {
        Some(Ok(l)) => l,
        _ => n,
    }
}

/// Judges a compile's diagnostics (`stderr` of every compile step) against
/// `exp`. `compiled` says whether every compile step succeeded. `None`:
/// the expectations hold (the case goes on to link and run when it is not
/// negative); `Some` with the failing class otherwise:
/// - `compile-reject` (rejects-valid): nothing requires an error and the
///   compiler reported one;
/// - `accept-invalid`: an error is required and the compiler reported none;
/// - `wrong-line`: the compiler reported errors, but not on a line that
///   requires one (`missed error at L<n>`), or on a line nothing expects
///   (`extra error at L<n>: <text>`, unless `excess_ok`).
pub fn judge(exp: &Expected, diags: &[Diag], compiled: bool) -> Option<Verdict> {
    let errors: Vec<&Diag> = diags.iter().filter(|d| d.sev == Sev::Error).collect();
    let required = exp.required();
    if required.is_empty() {
        if errors.is_empty() && compiled {
            return None;
        }
        let detail = errors
            .first()
            .map_or_else(|| "rejected without an error".to_owned(), |d| d.text.clone());
        if let Some(why) = exp.undecided(&errors) {
            return Some(Verdict::new(Class::ExpectUnknown, format!("{why}: {detail}")));
        }
        return Some(Verdict::new(Class::CompileReject, detail));
    }
    if errors.is_empty() && compiled {
        if exp.link_error(diags).is_some() && required.iter().all(|&l| l == 0) {
            // The link's output is left to meet it (`run_case`).
            return None;
        }
        return Some(Verdict::new(
            Class::AcceptInvalid,
            format!("no error (expected: {})", exp.pattern_at(None)),
        ));
    }
    if errors.is_empty() {
        // Failed without an error line it could read: a reject all the same.
        return None;
    }
    for &l in &required {
        if l == 0 {
            continue;
        }
        if !errors.iter().any(|d| d.line == l) {
            return Some(Verdict::new(
                Class::WrongLine,
                format!("missed error at L{l} (expected: {})", exp.pattern_at(Some(l))),
            ));
        }
    }
    if !exp.excess_ok
        && let Some(d) = errors.iter().find(|d| d.line != 0 && !exp.allows(d.line))
    {
        return Some(Verdict::new(
            Class::WrongLine,
            format!("extra error at L{}: {}", d.line, d.text),
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcc_tests::dg::directives;

    fn exp(text: &str, t: &Target) -> Expected {
        expectations(&directives(text), t).unwrap()
    }

    #[test]
    fn line_references() {
        assert_eq!(line_ref(".", 7), Some(7));
        assert_eq!(line_ref(".-1", 7), Some(6));
        assert_eq!(line_ref(".+2", 7), Some(9));
        assert_eq!(line_ref("12", 7), Some(12));
        assert_eq!(line_ref("0", 7), Some(0));
        assert_eq!(line_ref("", 7), Some(7));
        assert_eq!(line_ref("x", 7), None);
        assert_eq!(line_ref(".-9", 7), None);
    }

    #[test]
    fn expectations_follow_selectors() {
        let t = Target::for_arch("x86_64", false).with_cxx(17);
        let e = exp(
            "int a; // { dg-error \"x\" }\n\
             // { dg-error \"y\" \"\" { target c++20 } .-1 }\n\
             // { dg-error \"z\" \"\" { target c++17 } 1 }\n\
             int b; // { dg-warning \"w\" \"\" { xfail *-*-* } }\n\
             // { dg-message \"note\" \"\" { target *-*-* } 0 }\n\
             // { dg-bogus \"b\" }\n",
            &t,
        );
        assert_eq!(
            e.expects,
            vec![
                Expect { line: 1, sev: Sev::Error, xfail: false, pattern: "x".into() },
                Expect { line: 1, sev: Sev::Error, xfail: false, pattern: "z".into() },
                Expect { line: 4, sev: Sev::Warning, xfail: true, pattern: "w".into() },
                Expect { line: 0, sev: Sev::Message, xfail: false, pattern: "note".into() },
                Expect { line: 6, sev: Sev::Bogus, xfail: false, pattern: "b".into() },
            ]
        );
        assert!(e.negative() && !e.excess_ok);
        let e = exp(
            "// { dg-excess-errors \"gcc\" }\n// { dg-error \"q\" \"\" { xfail *-*-* } }\n",
            &t,
        );
        assert!(e.excess_ok && !e.negative());
        let e = exp("// { dg-prune-output \"note\" }\n", &t);
        assert!(e.excess_ok);
        let e = exp("// { dg-excess-errors \"gcc\" { target c++20 } }\n", &t);
        assert!(!e.excess_ok);
        assert!(
            expectations(&directives("// { dg-error \"x\" \"\" { target *-*-* } q }\n"), &t)
                .is_err()
        );
        let e = exp(
            "int f(); // { dg-line decl }\n// { dg-message \"here\" \"\" { target *-*-* } decl }\n",
            &t,
        );
        assert_eq!(e.expects[0].line, 1);
    }

    #[test]
    fn diagnostics_are_read_by_line_and_severity() {
        let s = "/t/a.C: In function 'int f()':\n\
                 /t/a.C:3:5: error: 'x' was not declared\n\
                 /t/a.C:4:1: warning: unused\n\
                 /t/a.C:4:1: note: here\n\
                 /usr/include/h.h:9:2: error: in header\n\
                 /t/a.C:7: sorry, unimplemented: thing\n\
                 error: unknown argument '-fgcc'\n";
        let d = diagnostics(s);
        let got: Vec<(usize, Sev)> = d.iter().map(|d| (d.line, d.sev)).collect();
        assert_eq!(
            got,
            [(3, Sev::Error), (4, Sev::Warning), (9, Sev::Error), (7, Sev::Error), (0, Sev::Error)]
        );
        assert_eq!(d[0].text, "error: 'x' was not declared");
    }

    #[test]
    fn judgement() {
        let t = Target::for_arch("aarch64", false).with_cxx(26);
        let e = |s: &str| exp(s, &t);
        let err = |line: usize| Diag { line, sev: Sev::Error, text: "error: e".into() };
        let warn = |line: usize| Diag { line, sev: Sev::Warning, text: "warning: w".into() };
        let valid = e("int a;\n");
        assert_eq!(judge(&valid, &[warn(1)], true), None);
        assert_eq!(judge(&valid, &[err(1)], false).unwrap().class, Class::CompileReject);
        let neg = e("int a; // { dg-error \"x\" }\nint b; // { dg-warning \"w\" }\n");
        assert_eq!(
            judge(&neg, &[], true).unwrap(),
            Verdict::new(Class::AcceptInvalid, "no error (expected: x)")
        );
        assert_eq!(judge(&neg, &[err(1)], false), None);
        assert_eq!(
            judge(&neg, &[err(1), err(2)], false),
            None,
            "an error where a warning is expected"
        );
        let v = judge(&neg, &[err(3)], false).unwrap();
        assert_eq!(
            (v.class, v.detail.as_str()),
            (Class::WrongLine, "missed error at L1 (expected: x)")
        );
        let v = judge(&neg, &[err(1), err(5)], false).unwrap();
        assert_eq!((v.class, v.detail.as_str()), (Class::WrongLine, "extra error at L5: error: e"));
        assert_eq!(judge(&neg, &[err(1), err(0)], false), None, "an error without a line");
        let lineless = e("// { dg-error \"x\" \"\" { target *-*-* } 0 }\n");
        assert_eq!(judge(&lineless, &[err(0)], false), None);
        assert_eq!(judge(&lineless, &[err(9)], false), None, "no line: any line (WP-1554)");
        let lined =
            e("int a; // { dg-error \"x\" }\n// { dg-warning \"w\" \"\" { target *-*-* } 0 }\n");
        assert_eq!(judge(&lined, &[err(1), err(9)], false).unwrap().class, Class::WrongLine);
        let excess = e("int a; // { dg-error \"x\" }\n// { dg-excess-errors \"\" }\n");
        assert_eq!(judge(&excess, &[err(1), err(5)], false), None);
        let bogus = e("int a; // { dg-error \"x\" }\nint b; // { dg-bogus \"y\" }\n");
        assert_eq!(judge(&bogus, &[err(1), err(2)], false).unwrap().class, Class::WrongLine);
        let bogus =
            e("int a; // { dg-error \"x\" }\nint b; // { dg-bogus \"\" \"\" { xfail *-*-* } }\n");
        assert_eq!(judge(&bogus, &[err(1), err(2)], false), None, "GCC errs there (xfail)");
    }

    /// `dg-message` has no severity prefix (`lib/gcc-dg.exp`): a regexp
    /// that names one takes it, one that names none (`""` among them)
    /// leaves the kind open on its line (WP-1554).
    #[test]
    fn dg_message_takes_the_severity_its_regexp_names() {
        let t = Target::for_arch("aarch64", false).with_cxx(26);
        let e = exp(
            "int a; // { dg-message \"error: no\" }\n\
             int b; // { dg-message \"sorry, unimplemented: x\" }\n\
             int c; // { dg-message \"note: here\" }\n\
             int d; // { dg-message \"\" }\n\
             int f; // { dg-message \"declared\" \"\" { xfail *-*-* } }\n\
             // { dg-message \"\\[^\\n\\]*warning: w\" \"\" { target *-*-* } 0 }\n",
            &t,
        );
        let sevs: Vec<(usize, Sev)> = e.expects.iter().map(|x| (x.line, x.sev)).collect();
        assert_eq!(
            sevs,
            [
                (1, Sev::Error),
                (2, Sev::Error),
                (3, Sev::Message),
                (4, Sev::Message),
                (5, Sev::Message),
                (0, Sev::Message)
            ]
        );
        assert!(e.negative());
        assert_eq!(
            e.any_kind,
            [(4, "dg-message \"\"".to_owned())],
            "xfail and named kinds are not open"
        );
        assert_eq!(e.unlocated, None);
    }

    /// `dg-regexp` matches the whole output with no line
    /// (`lib/gcc-defs.exp`): naming an error it requires one anywhere,
    /// naming none it may hold errors anywhere.
    #[test]
    fn dg_regexp_is_a_region_without_a_line() {
        let t = Target::for_arch("aarch64", false).with_cxx(26);
        let e = exp("int a;\n// { dg-regexp {[^\\n]*a.h:3:1: error: bad\\n} }\n", &t);
        assert_eq!(e.expects.len(), 1);
        assert_eq!((e.expects[0].line, e.expects[0].sev), (0, Sev::Error));
        assert!(e.negative() && e.unlocated.is_none());
        let e = exp("int a;\n// { dg-regexp {[^\\n]*: note: here\\n} }\n", &t);
        assert!(e.expects.is_empty() && e.unlocated.is_none() && !e.negative());
        let e = exp("int a;\n// { dg-regexp {In file included from [^\\n]*\\n} }\n", &t);
        assert_eq!(e.unlocated.as_deref(), Some("dg-regexp \"In file included from [^\\n]*\\n\""));
    }

    /// `dg-prune-output` and `dg-excess-errors` change what is unexpected:
    /// errors may hide in the output unless the prune pattern can only
    /// match a warning or a note line, which then changes nothing (WP-1617).
    #[test]
    fn prunes_and_excess_errors_may_hide_errors() {
        let t = Target::for_arch("aarch64", false).with_cxx(26);
        let e =
            exp("// { dg-prune-output \"note: x\" }\n// { dg-prune-output \"warning: y\" }\n", &t);
        assert!(!e.excess_ok && e.unlocated.is_none());
        let e = exp("// { dg-prune-output \"expected\" }\n// { dg-excess-errors \"\" }\n", &t);
        assert_eq!(e.unlocated.as_deref(), Some("dg-prune-output \"expected\""));
        let e = exp("// { dg-excess-errors \"crash\" }\n", &t);
        assert_eq!(e.unlocated.as_deref(), Some("dg-excess-errors \"crash\""));
        let e = exp("// { dg-excess-errors \"crash\" { target c++11_only } }\n", &t);
        assert!(e.unlocated.is_none() && !e.excess_ok);
    }

    /// Which prune patterns can match an error line (WP-1617): only one
    /// whose every match holds `warning:` or `note:` cannot.
    #[test]
    fn prune_patterns_that_cannot_match_an_error() {
        let q = |s: &str| prune_may_match_error(&Word::Quoted(s.into()));
        let b = |s: &str| prune_may_match_error(&Word::Braced(s.into()));
        // Certainly a warning or a note line.
        assert!(!q("note:"));
        assert!(!q("warning: unused"));
        assert!(!q("\\[^\\n\\r\\]*note: candidate"));
        assert!(!b("note: here 'x\\.[0-9]+' is"));
        assert!(!q("x*note: y"), "the starred atom is before the run");
        // May match an error line.
        assert!(q(""), "empty");
        assert!(q("note"), "the word alone may sit in an error's text");
        assert!(q("warning"));
        assert!(q("In function"));
        assert!(q("note:|ld: symbol"), "an alternative");
        assert!(q("(note: )?x"), "inside a group");
        assert!(q("[n]ote:"), "a bracket expression");
        assert!(q("note:?"), "the colon is optional");
        assert!(q("warn+ing: x"), "a repeat ends the run (`warnning:`)");
        assert!(q("note{0,1}:"));
        assert!(q("error: x note: y"), "names an error");
        assert!(q("sorry, unimplemented: note: x"));
        assert!(q("[^\\n]*note: x"), "a Tcl command substitution");
        assert!(q("$x note: y"), "a Tcl variable");
        assert!(q("\\d note: x") && !q("\\. note: x"), "an escape the reader does not know");
        assert!(q("(?i)note: x"), "an embedded option");
        assert!(q("note: (x"), "unbalanced");
        // A bracket holding `]` first, and a class.
        assert!(!b("[]x] note: [[:alpha:]]+"));
    }

    /// A prune that cannot match an error line neither hides an extra
    /// error nor leaves a rejection open (WP-1617).
    #[test]
    fn warning_and_note_prunes_decide() {
        let t = Target::for_arch("aarch64", false).with_cxx(26);
        let e = |s: &str| exp(s, &t);
        let err = |line: usize| Diag { line, sev: Sev::Error, text: "error: e".into() };
        let note = e("int a;\n// { dg-prune-output \"note: x\" }\n");
        assert_eq!(judge(&note, &[err(1)], false).unwrap().class, Class::CompileReject);
        let neg = e("int a; // { dg-error \"x\" }\n// { dg-prune-output \"warning: w\" }\n");
        assert_eq!(judge(&neg, &[err(1)], false), None);
        assert_eq!(judge(&neg, &[err(1), err(2)], false).unwrap().class, Class::WrongLine);
        let open = e("int a; // { dg-error \"x\" }\n// { dg-prune-output \"note\" }\n");
        assert_eq!(judge(&open, &[err(1), err(2)], false), None, "\"note\" may hide it");
        let open = e("int a;\n// { dg-prune-output \"note\" }\n");
        let v = judge(&open, &[err(1)], false).unwrap();
        assert_eq!(
            (v.class, v.detail.as_str()),
            (Class::ExpectUnknown, "dg-prune-output \"note\": error: e")
        );
    }

    /// An expected link error (WP-1664): a line-0 `dg-error`, or a line-0
    /// `dg-message` naming an error or no severity, in a case that links
    /// is left to the link when the compile is clean.
    #[test]
    fn line_zero_expectations_in_a_linking_case_are_link_errors() {
        let t = Target::for_arch("aarch64", false).with_cxx(26);
        let linking = |s: &str| {
            let mut e = exp(s, &t);
            e.links = true;
            e
        };
        let warn0 = Diag { line: 0, sev: Sev::Warning, text: "warning: w".into() };
        let warn3 = Diag { line: 3, sev: Sev::Warning, text: "warning: w".into() };
        let err3 = Diag { line: 3, sev: Sev::Error, text: "error: e".into() };

        let msg = linking("// { dg-message \"undefined\" \"\" { target *-*-* } 0 }\n");
        assert_eq!(msg.lineless.as_deref(), Some("dg-message \"undefined\""));
        assert_eq!(msg.link_error(&[]), Some("dg-message \"undefined\""));
        assert_eq!(msg.link_error(std::slice::from_ref(&warn3)), Some("dg-message \"undefined\""));
        assert_eq!(msg.link_error(std::slice::from_ref(&warn0)), None, "the compile may meet it");
        assert_eq!(msg.link_error(std::slice::from_ref(&err3)), None);
        assert_eq!(judge(&msg, &[], true), None);

        let err = linking("// { dg-error \"undefined\" \"\" { target *-*-* } 0 }\n");
        assert!(err.negative());
        assert_eq!(judge(&err, &[], true), None, "left to the link");
        assert_eq!(judge(&err, &[warn0], true).unwrap().class, Class::AcceptInvalid);
        assert_eq!(judge(&err, &[err3], false), None, "a compile error meets it");
        let mut compiled_only = err.clone();
        compiled_only.links = false;
        assert_eq!(compiled_only.link_error(&[]), None);
        assert_eq!(judge(&compiled_only, &[], true).unwrap().class, Class::AcceptInvalid);

        // Not link errors: a line, a named warning or note, an xfail, a
        // required error on a line as well.
        for s in [
            "int a; // { dg-message \"undefined\" }\n",
            "// { dg-message \"note: x\" \"\" { target *-*-* } 0 }\n",
            "// { dg-warning \"x\" \"\" { target *-*-* } 0 }\n",
            "// { dg-error \"x\" \"\" { xfail *-*-* } 0 }\n",
        ] {
            assert_eq!(linking(s).link_error(&[]), None, "{s}");
        }
        let both = linking(
            "int a; // { dg-error \"x\" }\n// { dg-error \"y\" \"\" { target *-*-* } 0 }\n",
        );
        assert_eq!(judge(&both, &[], true).unwrap().class, Class::AcceptInvalid);
    }

    /// The judgement of the forms: a decided case stays pass or
    /// rejects-valid; an error the file leaves open is `expect-unknown`;
    /// a case that links is always decided (GCC must compile it).
    #[test]
    fn undecided_errors_are_expect_unknown() {
        let t = Target::for_arch("aarch64", false).with_cxx(26);
        let e = |s: &str| exp(s, &t);
        let err = |line: usize| Diag { line, sev: Sev::Error, text: "error: e".into() };
        let class = |x: &Expected, d: &[Diag]| judge(x, d, d.is_empty()).map(|v| v.class);

        let open = e("int a;\nint b; // { dg-message \"\" }\n");
        assert_eq!(class(&open, &[]), None, "no error: pass");
        assert_eq!(class(&open, &[err(2)]), Some(Class::ExpectUnknown));
        let v = judge(&open, &[err(2)], false).unwrap();
        assert_eq!(v.detail, "dg-message \"\" at L2: error: e");
        assert_eq!(class(&open, &[err(1)]), Some(Class::CompileReject), "error elsewhere");
        assert_eq!(class(&open, &[err(2), err(1)]), Some(Class::CompileReject));
        let mut linked = open.clone();
        linked.links = true;
        assert_eq!(class(&linked, &[err(2)]), Some(Class::CompileReject), "a run test compiles");

        let named = e("int a; // { dg-message \"error: no\" }\n");
        assert_eq!(class(&named, &[err(1)]), None, "an expected error");
        assert_eq!(class(&named, &[]), Some(Class::AcceptInvalid));
        assert_eq!(class(&named, &[err(3)]), Some(Class::WrongLine));
        let note = e("int a; // { dg-message \"note: here\" }\n");
        assert_eq!(class(&note, &[err(1)]), Some(Class::CompileReject), "a note is no error");

        let anywhere = e("int a;\n// { dg-message \"\" \"\" { target *-*-* } 0 }\n");
        assert_eq!(class(&anywhere, &[err(7)]), Some(Class::ExpectUnknown));
        let excess = e("int a;\n// { dg-excess-errors \"\" }\n");
        assert_eq!(class(&excess, &[err(1)]), Some(Class::ExpectUnknown));
        assert_eq!(
            judge(&excess, &[], false).unwrap().detail,
            "dg-excess-errors \"\": rejected without an error"
        );
        let prune = e("int a;\n// { dg-prune-output \"warning: w\" }\n");
        assert_eq!(class(&prune, &[err(1)]), Some(Class::CompileReject));
        let regexp = e("int a;\n// { dg-regexp {[^\\n]*: error: x\\n} }\n");
        assert_eq!(class(&regexp, &[err(1)]), None, "the regexp's error, anywhere");
        assert_eq!(class(&regexp, &[]), Some(Class::AcceptInvalid));
    }
}
