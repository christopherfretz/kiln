//! `DejaGnu` `dg-` directives: extraction from a test file and resolution,
//! per option set, into a run plan or a skip (spec/harness.md "GCC
//! testsuites"). Suite-independent: the torture runner uses it for
//! `gcc.c-torture/{execute,compile}`, and a g++.dg runner can reuse it
//! with its own default action and default flags.

use super::target::Target;
use super::tcl::{self, Word};

/// One `{ dg-<name> <args> }` directive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Directive {
    /// 1-based source line.
    pub line: usize,
    /// `dg-do`, `dg-options`, ...
    pub name: String,
    pub args: Vec<Word>,
}

/// The directives of a test, the way `DejaGnu`'s `dg-get-options` finds them:
/// on each line, from a `{` followed by white space and `dg-<name>` to the
/// last white-space-preceded `}` of the line, read as a Tcl list.
pub fn directives(text: &str) -> Vec<Directive> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let mut from = 0;
        while let Some(off) = line[from..].find('{') {
            let open = from + off;
            let after = &line[open + 1..];
            let body = after.trim_start_matches([' ', '\t']);
            if body.len() == after.len() || !body.starts_with("dg-") {
                from = open + 1;
                continue;
            }
            let Some(close) = last_close(after) else { break };
            let inner = &after[..close];
            let mut words = tcl::list(inner);
            if words.is_empty() {
                break;
            }
            let name = words.remove(0).text().to_owned();
            if name.len() > 3
                && name[3..].bytes().all(|b| b.is_ascii_lowercase() || b == b'-' || b == b'_')
            {
                out.push(Directive { line: n + 1, name, args: words });
            }
            break;
        }
    }
    out
}

/// The byte offset in `s` of its last `}` preceded by a space or tab.
fn last_close(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    (1..b.len()).rev().find(|&i| b[i] == b'}' && (b[i - 1] == b' ' || b[i - 1] == b'\t'))
}

/// What a test does (`dg-do`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Preprocess,
    Compile,
    Assemble,
    Link,
    Run,
}

impl Action {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "preprocess" => Self::Preprocess,
            "compile" => Self::Compile,
            "assemble" => Self::Assemble,
            "link" => Self::Link,
            "run" => Self::Run,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Preprocess => "preprocess",
            Self::Compile => "compile",
            Self::Assemble => "assemble",
            Self::Link => "link",
            Self::Run => "run",
        }
    }

    /// Whether the test produces an executable.
    pub fn links(self) -> bool {
        matches!(self, Self::Link | Self::Run)
    }
}

/// The resolved plan of one test under one option set.
#[derive(Clone, Debug, PartialEq)]
pub enum Plan {
    Skip(String),
    Run(RunPlan),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunPlan {
    pub action: Action,
    /// The test's flags after the option set: `dg-options` (else the
    /// suite's default extra flags), then every applicable
    /// `dg-additional-options`.
    pub flags: Vec<String>,
    /// `dg-timeout-factor`.
    pub timeout_factor: f64,
    /// GCC's testsuite expects this to fail (`dg-do ... { xfail }`,
    /// `dg-xfail-if`, `dg-xfail-run-if`, a `.x` file's xfail): reported
    /// beside the result, never changes it.
    pub xfail: bool,
    /// The test expects an error (an applicable `dg-error`): a rejection
    /// is its pass and an acceptance is `accept-invalid`.
    pub negative: bool,
}

/// What a suite supplies: the action without a `dg-do`, and the flags
/// that `dg-options` replaces (`-w` for the torture suites).
#[derive(Clone, Debug)]
pub struct SuiteDefaults {
    pub action: Action,
    pub extra_flags: Vec<String>,
}

/// Splits a flag string into words (`dg-options "-O2 -fno-foo"`).
pub fn split_flags(s: &str) -> Vec<String> {
    tcl::list(s).iter().map(|w| w.text().to_owned()).filter(|w| !w.is_empty()).collect()
}

/// Resolves a test's directives for one option set (`option` is the
/// torture flags, e.g. `-O2`): `Skip` with the first reason that applies,
/// else the run plan.
pub fn resolve(dirs: &[Directive], d: &SuiteDefaults, target: &Target, option: &[String]) -> Plan {
    match resolve_inner(dirs, d, &target.with_option(option), option) {
        Ok(p) => p,
        Err(reason) => Plan::Skip(reason),
    }
}

fn applies(target: &Target, arg: Option<&Word>, what: &str) -> Result<Option<String>, String> {
    // `Ok(None)`: applies; `Ok(Some(kw))` with kw `xfail` applies as an
    // expected failure; `Err` means skip; a non-matching target selector
    // is `Ok(Some("no"))`.
    let Some(a) = arg else { return Ok(None) };
    match target.keyword_selector(a.text()) {
        Some((kw, Ok(true))) if kw == "xfail" => Ok(Some("xfail".into())),
        Some((kw, Ok(false))) if kw != "xfail" => Ok(Some("no".into())),
        Some((_, Err(e))) => Err(format!("{what}: {e}")),
        Some((_, Ok(_))) | None => Ok(None),
    }
}

fn resolve_inner(
    dirs: &[Directive],
    d: &SuiteDefaults,
    target: &Target,
    option: &[String],
) -> Result<Plan, String> {
    let mut action = d.action;
    let mut xfail = false;
    let mut negative = false;
    let mut options: Option<Vec<String>> = None;
    let mut additional: Vec<String> = Vec::new();
    let mut timeout_factor = 1.0;
    let mut skips: Vec<&Directive> = Vec::new();
    for dir in dirs {
        let a = &dir.args;
        let arg = |i: usize| a.get(i).map_or("", Word::text);
        match dir.name.as_str() {
            "dg-do" => {
                let Some(act) = Action::parse(arg(0)) else {
                    return Err(format!("dg-do {}", arg(0)));
                };
                match applies(target, a.get(1), "dg-do")?.as_deref() {
                    Some("no") => return Ok(Plan::Skip("dg-do target does not match".into())),
                    Some(_) => xfail = true,
                    None => {}
                }
                action = act;
            }
            "dg-options" => {
                if applies(target, a.get(1), "dg-options")?.as_deref() != Some("no") {
                    options = Some(split_flags(arg(0)));
                }
            }
            "dg-additional-options" => {
                if applies(target, a.get(1), "dg-additional-options")?.as_deref() != Some("no") {
                    additional.extend(split_flags(arg(0)));
                }
            }
            "dg-require-effective-target" => {
                if applies(target, a.get(1), "dg-require-effective-target")?.as_deref()
                    != Some("no")
                    && !target.effective(arg(0))?
                {
                    return Ok(Plan::Skip(format!("requires effective target {}", arg(0))));
                }
            }
            "dg-skip-if" => skips.push(dir),
            "dg-xfail-if" | "dg-xfail-run-if" => {
                if a.get(1).is_some_and(|s| target.selector(s.text()).unwrap_or(false)) {
                    xfail = true;
                }
            }
            "dg-timeout-factor" => {
                timeout_factor = arg(0).parse().unwrap_or(1.0);
            }
            "dg-add-options" => match arg(0) {
                "ieee" | "stack_size" => {}
                o => return Ok(Plan::Skip(format!("dg-add-options {o}"))),
            },
            "dg-require-alias"
            | "dg-require-weak"
            | "dg-require-visibility"
            | "dg-require-stack-size"
            | "dg-require-named-sections"
            | "dg-require-ifunc" => {}
            n if n.starts_with("dg-require-") => return Ok(Plan::Skip(n.to_owned())),
            "dg-error" => {
                // dg-error <regexp> [<comment> [<selector> [<line>]]]
                if applies(target, a.get(2), "dg-error")?.as_deref() != Some("no") {
                    negative = true;
                }
            }
            // Output checks the harness does not compare.
            _ => {}
        }
    }
    let mut flags = options.unwrap_or_else(|| d.extra_flags.clone());
    flags.extend(additional);
    let mut all = option.to_vec();
    all.extend(flags.iter().cloned());
    for dir in skips {
        let a = &dir.args;
        let sel = a.get(1).map_or("*-*-*", Word::text);
        if !target.selector(sel).map_err(|e| format!("dg-skip-if: {e}"))? {
            continue;
        }
        let include =
            a.get(2).map_or_else(|| vec!["*".to_owned()], |w| tcl::list_strings(w.text()));
        let exclude = a.get(3).map_or_else(Vec::new, |w| tcl::list_strings(w.text()));
        if include.iter().all(|p| flag_match(p, &all))
            && !exclude.iter().any(|p| flag_match(p, &all))
        {
            let why = a.first().map_or("", |w| w.text().trim());
            let why =
                if why.is_empty() { format!("target {}", sel.trim()) } else { why.to_owned() };
            return Ok(Plan::Skip(format!("dg-skip-if: {why}")));
        }
    }
    Ok(Plan::Run(RunPlan { action, flags, timeout_factor, xfail, negative }))
}

/// `DejaGnu`'s option test (`check_conditional_xfail`): `*` matches always,
/// `""` never, else the pattern as a glob over the space-separated flags.
pub fn flag_match(pattern: &str, flags: &[String]) -> bool {
    match pattern {
        "*" => true,
        "" => false,
        p => tcl::glob(&format!("* {p} *"), &format!(" {} ", flags.join(" "))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> SuiteDefaults {
        SuiteDefaults { action: Action::Run, extra_flags: vec!["-w".into()] }
    }

    fn o(s: &[&str]) -> Vec<String> {
        s.iter().map(|&x| x.to_owned()).collect()
    }

    #[test]
    fn extraction_follows_dejagnu() {
        let t = "/* { dg-do run } */\n\
                 /* { dg-skip-if \"big\" { avr-*-* } { \"*\" } { \"\" } } */\n\
                 int a; /* {dg-error \"x\"} */ { not-dg }\n\
                 /* { dg-additional-options \"-std=gnu17\" { target { ! ia32 } } } */\n";
        let d = directives(t);
        assert_eq!(d.len(), 3);
        assert_eq!((d[0].line, d[0].name.as_str()), (1, "dg-do"));
        assert_eq!(d[1].args.len(), 4);
        assert_eq!(d[2].args[1], Word::Braced(" target { ! ia32 } ".into()));
    }

    #[test]
    fn options_and_additional_options() {
        let t = Target::for_arch("aarch64", false);
        let d = directives(
            "/* { dg-options \"-O1 -fno-x\" } */\n/* { dg-additional-options \"-DA\" } */\n\
             /* { dg-additional-options \"-DB\" { target x86_64-*-* } } */\n",
        );
        let Plan::Run(p) = resolve(&d, &defaults(), &t, &o(&["-O2"])) else { panic!() };
        assert_eq!(p.flags, o(&["-O1", "-fno-x", "-DA"]));
        assert_eq!(p.action, Action::Run);
        let Plan::Run(p) = resolve(&[], &defaults(), &t, &o(&["-O0"])) else { panic!() };
        assert_eq!(p.flags, o(&["-w"]));
    }

    #[test]
    fn skips_and_requirements() {
        let t = Target::for_arch("aarch64", false);
        let d = directives("/* { dg-skip-if \"no O0\" { *-*-* } { \"-O0\" } { \"\" } } */\n");
        assert_eq!(
            resolve(&d, &defaults(), &t, &o(&["-O0"])),
            Plan::Skip("dg-skip-if: no O0".into())
        );
        assert!(matches!(resolve(&d, &defaults(), &t, &o(&["-O2"])), Plan::Run(_)));
        let d = directives("/* { dg-skip-if \"\" { *-*-* } { \"-O*\" } { \"-O0\" } } */\n");
        assert!(matches!(resolve(&d, &defaults(), &t, &o(&["-O0"])), Plan::Run(_)));
        assert!(matches!(resolve(&d, &defaults(), &t, &o(&["-O2"])), Plan::Skip(_)));
        let d = directives("/* { dg-skip-if \"avr\" { avr-*-* } } */\n");
        assert!(matches!(resolve(&d, &defaults(), &t, &o(&["-O2"])), Plan::Run(_)));
        let d = directives("/* { dg-require-effective-target int128 } */\n");
        assert!(matches!(resolve(&d, &defaults(), &t, &o(&["-O2"])), Plan::Run(_)));
        let d = directives("/* { dg-require-effective-target ia32 } */\n");
        assert_eq!(
            resolve(&d, &defaults(), &t, &o(&["-O2"])),
            Plan::Skip("requires effective target ia32".into())
        );
        let d = directives("/* { dg-require-effective-target frobnicate } */\n");
        assert_eq!(
            resolve(&d, &defaults(), &t, &o(&["-O2"])),
            Plan::Skip("unknown effective target frobnicate".into())
        );
        let d = directives("/* { dg-do run { target avr-*-* } } */\n");
        assert!(matches!(resolve(&d, &defaults(), &t, &o(&["-O2"])), Plan::Skip(_)));
        let d = directives("/* { dg-add-options bind_pic_locally } */\n");
        assert!(matches!(resolve(&d, &defaults(), &t, &o(&["-O2"])), Plan::Skip(_)));
    }

    #[test]
    fn xfail_negative_and_timeout() {
        let t = Target::for_arch("x86_64", false);
        let d = directives(
            "/* { dg-do compile { xfail *-*-* } } */\n/* { dg-timeout-factor 4.0 } */\n\
             int x = y; /* { dg-error \"undeclared\" } */\n",
        );
        let Plan::Run(p) = resolve(&d, &defaults(), &t, &o(&["-O0"])) else { panic!() };
        assert!(p.xfail && p.negative);
        assert_eq!(p.action, Action::Compile);
        assert!((p.timeout_factor - 4.0).abs() < f64::EPSILON);
        let d = directives("int x; /* { dg-error \"e\" \"\" { target avr-*-* } } */\n");
        let Plan::Run(p) = resolve(&d, &defaults(), &t, &o(&["-O0"])) else { panic!() };
        assert!(!p.negative);
    }

    #[test]
    fn flag_globs() {
        assert!(flag_match("-flto*", &o(&["-O2", "-flto", "-w"])));
        assert!(!flag_match("-O0", &o(&["-O2"])));
        assert!(flag_match("-O2 -w", &o(&["-O2", "-w"])));
        assert!(flag_match("*", &[]));
        assert!(!flag_match("", &o(&["-O2"])));
    }
}
