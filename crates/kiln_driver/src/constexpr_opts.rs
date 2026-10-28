//! The constant-evaluation flags (`spec/oven.md` "Integration",
//! `spec/driver.md`):
//!
//! - `-fconstexpr-steps=N`: statements one top-level evaluation may execute
//!   (clang's flag and default, 1048576);
//! - `-fconstexpr-depth=N`: nested calls (clang's, 512);
//! - `-fconstexpr-memory=N[k|m|g]`: live abstract-memory bytes (kiln's,
//!   256 MiB; the suffixes are binary);
//! - `-fconstexpr-backtrace-limit=N`: call frames shown in a failure's notes
//!   (clang's, 10; 0 shows all);
//! - `-fconstexpr-report=<path>`: the per-TU NDJSON report (`spec/oven.md`
//!   "Reporting protocol"), written at the end of each translation unit,
//!   even when it has errors.

use kiln_diag::{Diag, DiagEngine, DiagId};
use kiln_oven::Oven;
use kiln_parse::ConstexprLimits;
use kiln_source::{Loc, SourceManager, Span};

use crate::args::ArgError;

/// The `-fconstexpr-*` options of a command line (the last of each wins).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConstexprOpts {
    /// `-fconstexpr-steps=`.
    pub steps: Option<u64>,
    /// `-fconstexpr-depth=`.
    pub depth: Option<u32>,
    /// `-fconstexpr-memory=`, in bytes.
    pub memory: Option<u64>,
    /// `-fconstexpr-backtrace-limit=`.
    pub backtrace_limit: Option<usize>,
    /// `-fconstexpr-report=`.
    pub report: Option<String>,
}

impl ConstexprOpts {
    /// Takes `arg` if it is one of the flags: `None` if it is not, else
    /// whether its value was valid.
    pub fn parse_arg(&mut self, arg: &str) -> Option<Result<(), ArgError>> {
        let (flag, v) = arg.split_once('=')?;
        let ok = match flag {
            "-fconstexpr-steps" => set(&mut self.steps, v.parse().ok()),
            "-fconstexpr-depth" => set(&mut self.depth, v.parse().ok()),
            "-fconstexpr-memory" => set(&mut self.memory, parse_bytes(v)),
            "-fconstexpr-backtrace-limit" => set(&mut self.backtrace_limit, v.parse().ok()),
            "-fconstexpr-report" if v.is_empty() => {
                return Some(Err(ArgError::MissingValue("-fconstexpr-report=".to_owned())));
            }
            "-fconstexpr-report" => set(&mut self.report, Some(v.to_owned())),
            _ => return None,
        };
        Some(if ok { Ok(()) } else { Err(ArgError::InvalidInt(arg.to_owned(), v.to_owned())) })
    }

    /// The limits of each translation unit.
    pub fn limits(&self) -> ConstexprLimits {
        let mut l = ConstexprLimits::default();
        if let Some(n) = self.steps {
            l.budget.steps = n;
        }
        if let Some(n) = self.depth {
            l.budget.depth = n;
        }
        if let Some(n) = self.memory {
            l.budget.bytes = n;
        }
        if let Some(n) = self.backtrace_limit {
            l.backtrace_limit = n;
        }
        l
    }

    /// Writes the report of `oven` (the end of a translation unit) if one
    /// was asked for; a write failure is an error diagnostic.
    pub fn write_report(&self, oven: &Oven, sm: &SourceManager, engine: &DiagEngine<'_>) {
        let Some(path) = &self.report else { return };
        let loc = |l: Loc| {
            let p = sm.presumed_line_col(l)?;
            Some(format!("{}:{}:{}", p.file_name, p.line, p.col))
        };
        if let Err(e) = kiln_oven::report::write(oven.stats(), &loc, std::path::Path::new(path)) {
            Diag::new(DiagId::drv_cannot_open_output, Span::INVALID)
                .arg(path)
                .arg(e.to_string())
                .emit(engine);
        }
    }
}

/// Stores `v` in `slot` if it is `Some`; returns whether it was.
fn set<T>(slot: &mut Option<T>, v: Option<T>) -> bool {
    let ok = v.is_some();
    if ok {
        *slot = v;
    }
    ok
}

/// `N`, `Nk`, `Nm` or `Ng` (binary multiples, either case).
fn parse_bytes(v: &str) -> Option<u64> {
    let (digits, shift) = match v.as_bytes().last()? {
        b'k' | b'K' => (&v[..v.len() - 1], 10),
        b'm' | b'M' => (&v[..v.len() - 1], 20),
        b'g' | b'G' => (&v[..v.len() - 1], 30),
        _ => (v, 0),
    };
    digits.parse::<u64>().ok()?.checked_mul(1 << shift)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(args: &[&str]) -> ConstexprOpts {
        let mut o = ConstexprOpts::default();
        for a in args {
            assert_eq!(o.parse_arg(a), Some(Ok(())), "{a}");
        }
        o
    }

    #[test]
    fn defaults_are_clangs() {
        let l = ConstexprOpts::default().limits();
        assert_eq!((l.budget.steps, l.budget.depth, l.backtrace_limit), (1_048_576, 512, 10));
        assert_eq!(l.budget.bytes, 256 << 20);
    }

    #[test]
    fn flags() {
        let o = opts(&[
            "-fconstexpr-steps=5",
            "-fconstexpr-depth=7",
            "-fconstexpr-memory=2m",
            "-fconstexpr-backtrace-limit=0",
            "-fconstexpr-report=r.jsonl",
            "-fconstexpr-steps=6",
        ]);
        let l = o.limits();
        assert_eq!((l.budget.steps, l.budget.depth, l.budget.bytes), (6, 7, 2 << 20));
        assert_eq!(l.backtrace_limit, 0);
        assert_eq!(o.report.as_deref(), Some("r.jsonl"));
        assert_eq!(opts(&["-fconstexpr-memory=4096"]).memory, Some(4096));
        assert_eq!(opts(&["-fconstexpr-memory=1G"]).memory, Some(1 << 30));
    }

    #[test]
    fn errors_and_others() {
        let mut o = ConstexprOpts::default();
        assert_eq!(o.parse_arg("-fconstexpr-stepsx=1"), None);
        assert_eq!(o.parse_arg("-fsyntax-only"), None);
        assert_eq!(
            o.parse_arg("-fconstexpr-steps=x"),
            Some(Err(ArgError::InvalidInt("-fconstexpr-steps=x".into(), "x".into())))
        );
        assert_eq!(
            o.parse_arg("-fconstexpr-depth=-1"),
            Some(Err(ArgError::InvalidInt("-fconstexpr-depth=-1".into(), "-1".into())))
        );
        assert!(matches!(o.parse_arg("-fconstexpr-memory=1t"), Some(Err(_))));
        assert!(matches!(o.parse_arg("-fconstexpr-report="), Some(Err(_))));
        assert_eq!(o, ConstexprOpts::default());
    }
}
