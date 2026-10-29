//! The template instantiation flags (spec/sema-templates.md §10, §11.3):
//!
//! - `-ftemplate-depth=N`: nested instantiations (clang's flag and
//!   default, 1024);
//! - `-ftemplate-backtrace-limit=N`: instantiation contexts shown in a
//!   diagnostic's notes (clang's, 10; 0 shows all);
//! - `-fkiln-instantiation-report[=all|json]`: the per-template report on
//!   stderr at the end of each translation unit (the top 50 rows as text,
//!   every row, or NDJSON);
//! - `-fkiln-stack-size=N[K|M|G]`: the compiling thread's stack (default
//!   1 GiB, at least 4 MiB); the stack guard (§1 P4, §10.3) stops with a
//!   fatal diagnostic before it runs out.

use kiln_sema::stack_guard::{DEFAULT_STACK_SIZE, parse_stack_size};
use kiln_sema::templ::report::ReportFormat;
use kiln_sema::templ::stack::InstLimits;

use crate::args::ArgError;

/// The template options of a command line (the last of each wins).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TemplOpts {
    /// `-ftemplate-depth=`.
    pub depth: Option<u32>,
    /// `-ftemplate-backtrace-limit=`.
    pub backtrace_limit: Option<u32>,
    /// `-fkiln-instantiation-report[=...]`.
    pub report: ReportFormat,
    /// `-fkiln-stack-size=`, in bytes.
    pub stack_size: Option<usize>,
}

impl TemplOpts {
    /// Takes `arg` if it is one of the flags: `None` if it is not, else
    /// whether its value was valid.
    pub fn parse_arg(&mut self, arg: &str) -> Option<Result<(), ArgError>> {
        if arg == "-fkiln-instantiation-report" {
            self.report = ReportFormat::Text;
            return Some(Ok(()));
        }
        let (flag, v) = arg.split_once('=')?;
        let ok = match flag {
            "-ftemplate-depth" => set(&mut self.depth, v.parse().ok()),
            "-ftemplate-backtrace-limit" => set(&mut self.backtrace_limit, v.parse().ok()),
            "-fkiln-stack-size" => {
                if !set(&mut self.stack_size, parse_stack_size(v)) {
                    return Some(Err(ArgError::InvalidFlagValue(arg.to_owned(), v.to_owned())));
                }
                true
            }
            "-fkiln-instantiation-report" => {
                let f = match v {
                    "all" => Some(ReportFormat::All),
                    "json" => Some(ReportFormat::Json),
                    "text" => Some(ReportFormat::Text),
                    _ => None,
                };
                let Some(f) = f else {
                    return Some(Err(ArgError::InvalidFlagValue(arg.to_owned(), v.to_owned())));
                };
                self.report = f;
                true
            }
            _ => return None,
        };
        Some(if ok { Ok(()) } else { Err(ArgError::InvalidInt(arg.to_owned(), v.to_owned())) })
    }

    /// The limits of each translation unit.
    pub fn limits(&self) -> InstLimits {
        let mut l = InstLimits::default();
        if let Some(n) = self.depth {
            l.depth = n;
        }
        if let Some(n) = self.backtrace_limit {
            l.backtrace = n;
        }
        l
    }
}

/// The stack size the compiling thread is spawned with: the last valid
/// `-fkiln-stack-size=` of `args`, else [`DEFAULT_STACK_SIZE`]. The
/// executable reads it before the command line is parsed (invalid values
/// are diagnosed by the parse, on that thread).
pub fn stack_size_of(args: &[String]) -> usize {
    args.iter()
        .rev()
        .filter_map(|a| a.strip_prefix("-fkiln-stack-size="))
        .find_map(parse_stack_size)
        .unwrap_or(DEFAULT_STACK_SIZE)
}

/// Stores `v` in `slot` if it is `Some`; returns whether it was.
fn set<T>(slot: &mut Option<T>, v: Option<T>) -> bool {
    let ok = v.is_some();
    if ok {
        *slot = v;
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_clangs() {
        let l = TemplOpts::default().limits();
        assert_eq!((l.depth, l.backtrace), (1024, 10));
        assert_eq!(TemplOpts::default().report, ReportFormat::Off);
    }

    #[test]
    fn flags() {
        let mut o = TemplOpts::default();
        for a in [
            "-ftemplate-depth=64",
            "-ftemplate-backtrace-limit=0",
            "-fkiln-instantiation-report",
            "-ftemplate-depth=128",
        ] {
            assert_eq!(o.parse_arg(a), Some(Ok(())), "{a}");
        }
        assert_eq!((o.limits().depth, o.limits().backtrace), (128, 0));
        assert_eq!(o.report, ReportFormat::Text);
        assert_eq!(o.parse_arg("-fkiln-instantiation-report=json"), Some(Ok(())));
        assert_eq!(o.report, ReportFormat::Json);
        assert_eq!(o.parse_arg("-fkiln-instantiation-report=all"), Some(Ok(())));
        assert_eq!(o.report, ReportFormat::All);
    }

    #[test]
    fn stack_size() {
        let mut o = TemplOpts::default();
        assert_eq!(o.parse_arg("-fkiln-stack-size=64M"), Some(Ok(())));
        assert_eq!(o.stack_size, Some(64 << 20));
        assert!(matches!(o.parse_arg("-fkiln-stack-size=lots"), Some(Err(_))));
        assert!(matches!(o.parse_arg("-fkiln-stack-size=0"), Some(Err(_))));
        assert_eq!(o.stack_size, Some(64 << 20));
        let args = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(stack_size_of(&args(&["a.cpp"])), DEFAULT_STACK_SIZE);
        assert_eq!(stack_size_of(&args(&["-fkiln-stack-size=8M", "-fkiln-stack-size=x"])), 8 << 20);
        assert_eq!(stack_size_of(&args(&["-fkiln-stack-size=1K"])), 4 << 20);
    }

    #[test]
    fn bad_values() {
        let mut o = TemplOpts::default();
        assert!(matches!(o.parse_arg("-ftemplate-depth=x"), Some(Err(_))));
        assert!(matches!(o.parse_arg("-fkiln-instantiation-report=yaml"), Some(Err(_))));
        assert_eq!(o.parse_arg("-ftemplate-foo=1"), None);
        assert_eq!(o, TemplOpts::default());
    }
}
