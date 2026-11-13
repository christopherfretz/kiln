//! The constant-evaluation flags (`spec/oven.md` "Integration",
//! `spec/driver.md`):
//!
//! - `-fconstexpr-steps=N`: statements one top-level evaluation may execute
//!   (clang's flag and default, 1048576); GCC's `-fconstexpr-ops-limit=N`
//!   (which real reflection-heavy builds pass) sets the same budget;
//! - `-fconstexpr-depth=N`: nested calls (clang's, 512);
//! - `-fconstexpr-memory=N[k|m|g]`: live abstract-memory bytes (kiln's,
//!   256 MiB; the suffixes are binary);
//! - `-fconstexpr-backtrace-limit=N`: call frames shown in a failure's notes
//!   (clang's, 10; 0 shows all);
//! - `-fconstexpr-report=<path>`: the per-TU NDJSON report (`spec/oven.md`
//!   "Reporting protocol"), written at the end of each translation unit,
//!   even when it has errors;
//! - `-fconstexpr-stats`: the per-TU summary on stderr (`constexpr: ...`
//!   lines; `spec/oven.md` "Profile counters");
//! - `-fconstexpr-profile[=json|text|<file>]`: the per-function and
//!   per-evaluation-site profile, as text or NDJSON (`=json`, or a file
//!   name ending in `.json`), on stderr or to `<file>`;
//!   `-fconstexpr-profile-limit=N` rows per table (20; 0: all).

use kiln_ast::Ast;
use kiln_diag::{Diag, DiagEngine, DiagId};
use kiln_oven::Oven;
use kiln_oven::profile::{Level, RenderOptions};
use kiln_parse::ConstexprLimits;
use kiln_source::{FileId, Loc, SourceManager, Span};

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
    /// `-fconstexpr-stats`.
    pub stats: bool,
    /// `-fconstexpr-profile[=...]`.
    pub profile: Option<ProfileOut>,
    /// `-fconstexpr-profile-limit=`.
    pub profile_limit: Option<usize>,
}

/// Where and how `-fconstexpr-profile` writes the profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileOut {
    /// Text on stderr (no value, `=text`).
    Text,
    /// NDJSON on stderr (`=json`).
    Json,
    /// To a file (created or truncated per translation unit): NDJSON if
    /// its name ends in `.json`, else text.
    File(String),
}

/// Rows per profile table without `-fconstexpr-profile-limit=`.
pub const DEFAULT_PROFILE_LIMIT: usize = 20;

impl ConstexprOpts {
    /// Takes `arg` if it is one of the flags: `None` if it is not, else
    /// whether its value was valid.
    pub fn parse_arg(&mut self, arg: &str) -> Option<Result<(), ArgError>> {
        match arg {
            "-fconstexpr-stats" => self.stats = true,
            "-fconstexpr-profile" => self.profile = Some(ProfileOut::Text),
            _ => {}
        }
        if matches!(arg, "-fconstexpr-stats" | "-fconstexpr-profile") {
            return Some(Ok(()));
        }
        let (flag, v) = arg.split_once('=')?;
        let ok = match flag {
            "-fconstexpr-profile" if v.is_empty() => {
                return Some(Err(ArgError::MissingValue("-fconstexpr-profile=".to_owned())));
            }
            "-fconstexpr-profile" => {
                self.profile = Some(match v {
                    "text" => ProfileOut::Text,
                    "json" => ProfileOut::Json,
                    _ => ProfileOut::File(v.to_owned()),
                });
                true
            }
            "-fconstexpr-profile-limit" => set(&mut self.profile_limit, v.parse().ok()),
            "-fconstexpr-steps" | "-fconstexpr-ops-limit" => set(&mut self.steps, v.parse().ok()),
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
        l.profile = if self.profile.is_some() {
            Level::Calls
        } else if self.stats {
            Level::Stats
        } else {
            Level::Off
        };
        l
    }

    /// Writes the `-fconstexpr-profile` report and the
    /// `-fconstexpr-stats` summary of `oven` (the end of a translation
    /// unit whose main file is `main`), in that order, to `err` (the
    /// profile to its file if it names one; a write failure is an error
    /// diagnostic), after flushing `engine`. Nothing without the flags.
    pub fn write_profile(
        &self,
        oven: &Oven,
        ast: &Ast,
        sm: &SourceManager,
        main: Option<FileId>,
        engine: &DiagEngine<'_>,
        err: &mut dyn std::io::Write,
    ) {
        if !self.profiling() {
            return;
        }
        engine.flush();
        let main = main.map_or("", |f| sm.file_name(f));
        if let Some(out) = &self.profile
            && let Some(rows) = oven.profile_rows(ast)
        {
            let json = match out {
                ProfileOut::Text => false,
                ProfileOut::Json => true,
                ProfileOut::File(f) => std::path::Path::new(f)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("json")),
            };
            let limit = self.profile_limit.unwrap_or(DEFAULT_PROFILE_LIMIT);
            let loc = |l: Loc| {
                let p = sm.presumed_line_col(l)?;
                Some((p.file_name.to_owned(), p.line, p.col))
            };
            let name = main.rsplit('/').next().unwrap_or(main);
            let text = rows.render(name, RenderOptions { json, limit }, &loc);
            match out {
                ProfileOut::File(path) => {
                    if let Err(e) = std::fs::write(path, text) {
                        Diag::new(DiagId::drv_cannot_open_output, Span::INVALID)
                            .arg(path)
                            .arg(e.to_string())
                            .emit(engine);
                    }
                }
                _ => {
                    let _ = err.write_all(text.as_bytes());
                }
            }
        }
        if self.stats {
            let _ = err.write_all(oven.stats_summary().as_bytes());
        }
    }

    /// Whether [`ConstexprOpts::write_profile`] writes anything.
    pub fn profiling(&self) -> bool {
        self.stats || self.profile.is_some()
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
        assert_eq!(
            o.parse_arg("-fconstexpr-profile="),
            Some(Err(ArgError::MissingValue("-fconstexpr-profile=".into())))
        );
        assert_eq!(
            o.parse_arg("-fconstexpr-profile-limit=x"),
            Some(Err(ArgError::InvalidInt("-fconstexpr-profile-limit=x".into(), "x".into())))
        );
        assert_eq!(o.parse_arg("-fconstexpr-statsx"), None);
        assert_eq!(o, ConstexprOpts::default());
    }

    /// `-fconstexpr-stats`, `-fconstexpr-profile[=<format>|<file>]` and
    /// `-fconstexpr-profile-limit=<n>` (spec/driver.md "Constexpr
    /// profile"); the last `-fconstexpr-profile` wins.
    #[test]
    fn profile_flags() {
        let o = ConstexprOpts::default();
        assert!(!o.profiling());
        assert_eq!(o.limits().profile, Level::Off);
        let o = opts(&["-fconstexpr-stats"]);
        assert!(o.stats && o.profiling());
        assert_eq!(o.limits().profile, Level::Stats);
        let o = opts(&["-fconstexpr-profile"]);
        assert_eq!(o.profile, Some(ProfileOut::Text));
        assert_eq!(o.limits().profile, Level::Calls);
        assert_eq!(opts(&["-fconstexpr-profile=json"]).profile, Some(ProfileOut::Json));
        assert_eq!(opts(&["-fconstexpr-profile=text"]).profile, Some(ProfileOut::Text));
        let o = opts(&["-fconstexpr-profile=json", "-fconstexpr-profile=p.json"]);
        assert_eq!(o.profile, Some(ProfileOut::File("p.json".into())));
        let o = opts(&["-fconstexpr-profile-limit=0", "-fconstexpr-stats", "-fconstexpr-profile"]);
        assert_eq!((o.profile_limit, o.limits().profile), (Some(0), Level::Calls));
    }
}
