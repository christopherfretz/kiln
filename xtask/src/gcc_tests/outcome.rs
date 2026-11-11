//! The result classes of a GCC-testsuite case for one compiler, and the
//! failure keys clusters are ranked by (spec/harness.md "GCC
//! testsuites"). Suite-independent: compile-only suites use the compile
//! half, execute suites both halves.

use std::process::ExitStatus;

use crate::libcxx::cluster::crash_key;
use crate::util::process;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Class {
    Pass,
    /// A valid test the compiler (or its link step) rejected.
    CompileReject,
    /// An internal compiler error, a panic or the compiler dying on a
    /// signal or the memory cap.
    Crash,
    /// The executable exited non-zero or died on a signal (abort).
    Wrong,
    /// The compiler or the executable outran its timeout.
    Timeout,
    /// A test that expects an error compiled clean.
    AcceptInvalid,
    /// The compiler could not be run at all (missing, a tool error).
    NotRun,
}

impl Class {
    pub const ALL: [Self; 7] = [
        Self::Pass,
        Self::CompileReject,
        Self::Crash,
        Self::Wrong,
        Self::Timeout,
        Self::AcceptInvalid,
        Self::NotRun,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::CompileReject => "compile-reject",
            Self::Crash => "crash",
            Self::Wrong => "wrong",
            Self::Timeout => "timeout",
            Self::AcceptInvalid => "accept-invalid",
            Self::NotRun => "not-run",
        }
    }

    pub fn is_failure(self) -> bool {
        !matches!(self, Self::Pass | Self::NotRun)
    }
}

/// One compiler's result on one case: the class and a one-line detail
/// (the first error, the crash key, the exit status).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub class: Class,
    pub detail: String,
}

impl Verdict {
    pub fn new(class: Class, detail: impl Into<String>) -> Self {
        Self { class, detail: detail.into() }
    }

    pub fn pass() -> Self {
        Self::new(Class::Pass, "")
    }
}

/// Judges a finished compile (or link) step: `None` when it succeeded.
/// `step` prefixes the detail of a link failure (`link: ...`).
pub fn compile_step(status: Option<ExitStatus>, stderr: &str, step: &str) -> Option<Verdict> {
    let Some(st) = status else {
        return Some(Verdict::new(Class::Timeout, format!("{step} timeout")));
    };
    if process::out_of_memory(Some(st), stderr) {
        return Some(Verdict::new(Class::Crash, "out of memory"));
    }
    if let Some(l) = stderr.lines().find(|l| l.contains("internal compiler error")) {
        return Some(Verdict::new(Class::Crash, strip_location(l)));
    }
    if let Some((key, _)) = crash_key(stderr) {
        return Some(Verdict::new(Class::Crash, key));
    }
    if st.success() {
        return None;
    }
    if st.code().is_none_or(|c| c >= 128) {
        return Some(Verdict::new(Class::Crash, format!("{step}: {st}")));
    }
    let first = first_error(stderr);
    let detail = if step == "compile" { first } else { format!("{step}: {first}") };
    Some(Verdict::new(Class::CompileReject, detail))
}

/// Judges a finished run of the test executable.
pub fn run_step(status: Option<ExitStatus>) -> Verdict {
    match status {
        None => Verdict::new(Class::Timeout, "run timeout"),
        Some(s) if s.success() => Verdict::pass(),
        Some(s) => match s.code() {
            Some(c) => Verdict::new(Class::Wrong, format!("exit {c}")),
            None => Verdict::new(Class::Wrong, format!("{s}")),
        },
    }
}

/// The verdict of a negative test (one that expects an error) from its
/// compile step's verdict.
pub fn negative(compile: Option<Verdict>) -> Verdict {
    match compile {
        None => Verdict::new(Class::AcceptInvalid, "no error"),
        Some(v) if v.class == Class::CompileReject => Verdict::pass(),
        Some(v) => v,
    }
}

/// The first error line of a compiler's stderr, without its location
/// (else the first non-empty line).
pub fn first_error(stderr: &str) -> String {
    let line = stderr
        .lines()
        .find(|l| l.contains("error:") || l.contains("undefined reference"))
        .or_else(|| stderr.lines().find(|l| !l.trim().is_empty()))
        .unwrap_or("(no output)");
    let mut s = strip_location(line);
    if s.len() > 200 {
        let mut cut = 200;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
        s.push('…');
    }
    s
}

/// `path:12:3: error: x` becomes `error: x`; a linker's `path.o:(.text+0x1c):`
/// or `/usr/bin/ld:` prefix goes too.
pub fn strip_location(line: &str) -> String {
    let l = line.trim();
    for key in ["internal compiler error:", "fatal error:", "error:", "undefined reference"] {
        if let Some(i) = l.find(key) {
            return l[i..].to_owned();
        }
    }
    l.to_owned()
}

/// The cluster a failure belongs to: its class and its detail with quoted
/// names and numbers folded, so `use of undeclared identifier 'x'` and
/// `... 'y'` share a key.
pub fn cluster_key(v: &Verdict) -> String {
    let mut out = String::new();
    let mut quote: Option<char> = None;
    let mut last_digit = false;
    for ch in v.detail.chars() {
        if let Some(q) = quote {
            if ch == q || (q == '‘' && ch == '’') {
                quote = None;
                out.push(ch);
            }
            continue;
        }
        if ch == '\'' || ch == '‘' || ch == '`' {
            quote = Some(ch);
            out.push(ch);
            out.push('…');
            last_digit = false;
            continue;
        }
        if ch.is_ascii_digit() {
            if !last_digit {
                out.push('N');
            }
            last_digit = true;
            continue;
        }
        last_digit = false;
        out.push(ch);
    }
    format!("{}: {}", v.class.label(), out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[allow(clippy::unnecessary_wraps, reason = "the shape compile_step and run_step take")]
    fn code(c: i32) -> Option<ExitStatus> {
        use std::os::unix::process::ExitStatusExt;
        Some(ExitStatus::from_raw(c << 8))
    }

    #[cfg(unix)]
    #[allow(clippy::unnecessary_wraps, reason = "the shape compile_step and run_step take")]
    fn signal(s: i32) -> Option<ExitStatus> {
        use std::os::unix::process::ExitStatusExt;
        Some(ExitStatus::from_raw(s))
    }

    #[cfg(unix)]
    #[test]
    fn compile_steps() {
        assert_eq!(compile_step(code(0), "warning: x\n", "compile"), None);
        assert_eq!(compile_step(None, "", "compile").unwrap().class, Class::Timeout);
        let v =
            compile_step(code(1), "t.c:3:5: error: use of undeclared identifier 'q'\n", "compile")
                .unwrap();
        assert_eq!(
            v,
            Verdict::new(Class::CompileReject, "error: use of undeclared identifier 'q'")
        );
        let v = compile_step(
            code(1),
            "/usr/bin/ld: t.o: in function `main':\nt.c:(.text+0x8): undefined reference to `f'\n",
            "link",
        )
        .unwrap();
        assert_eq!(v.detail, "link: undefined reference to `f'");
        let v = compile_step(
            code(1),
            "t.c:1:1: internal compiler error: Segmentation fault\n",
            "compile",
        )
        .unwrap();
        assert_eq!(v, Verdict::new(Class::Crash, "internal compiler error: Segmentation fault"));
        assert_eq!(compile_step(signal(11), "", "compile").unwrap().class, Class::Crash);
    }

    #[cfg(unix)]
    #[test]
    fn run_steps_and_negatives() {
        assert_eq!(run_step(code(0)), Verdict::pass());
        assert_eq!(run_step(code(1)), Verdict::new(Class::Wrong, "exit 1"));
        assert_eq!(run_step(signal(6)).class, Class::Wrong);
        assert_eq!(run_step(None).class, Class::Timeout);
        assert_eq!(negative(None).class, Class::AcceptInvalid);
        assert_eq!(negative(Some(Verdict::new(Class::CompileReject, "e"))), Verdict::pass());
        assert_eq!(negative(Some(Verdict::new(Class::Crash, "c"))).class, Class::Crash);
    }

    #[test]
    fn cluster_keys_fold_names_and_numbers() {
        let a = Verdict::new(Class::CompileReject, "error: use of undeclared identifier 'x1'");
        let b = Verdict::new(Class::CompileReject, "error: use of undeclared identifier 'yy'");
        assert_eq!(cluster_key(&a), cluster_key(&b));
        assert_eq!(cluster_key(&a), "compile-reject: error: use of undeclared identifier '…'");
        assert_eq!(cluster_key(&Verdict::new(Class::Wrong, "exit 134")), "wrong: exit N");
        assert!(Class::Crash.is_failure() && !Class::Pass.is_failure());
    }
}
