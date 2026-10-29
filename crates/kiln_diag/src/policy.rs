//! Warning policy: from options to the severity of each id (`spec/diag.md`
//! section "Policy"). A transcription of clang's `ProcessWarningOptions`
//! and `DiagnosticIDs::getDiagnosticSeverity`.

use crate::groups::{Flavor, group_members, nearest_group};
use crate::ids::{DiagClass, DiagId};
use crate::options::{DiagOptions, OptionDiag};

/// How a diagnostic is treated, from least to most severe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Not emitted.
    Ignored,
    /// Emitted as a remark.
    Remark,
    /// Emitted as a warning.
    Warning,
    /// Emitted as an error.
    Error,
    /// Emitted as a fatal error; later diagnostics are suppressed.
    Fatal,
}

/// Per-id mapping state (clang's `DiagnosticMapping`).
#[derive(Clone, Copy, Debug)]
struct Mapping {
    severity: Severity,
    /// Set by an option (as opposed to the default).
    user: bool,
    /// `-Wno-error=<group>` (or a `DefaultWarnNoWerror` id).
    no_werror: bool,
    /// `-Wno-fatal-errors=<group>`.
    no_fatal: bool,
}

/// The resolved policy. Immutable once built.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct Policy {
    map: Vec<Mapping>,
    ignore_all: bool,
    warnings_as_errors: bool,
    errors_as_fatal: bool,
    enable_all: bool,
    ext_behavior: Severity,
    /// Drop warnings located in system headers (no `-Wsystem-headers`).
    pub(crate) suppress_system: bool,
}

impl Policy {
    /// Builds the policy from `opts`. Returns it with the diagnostics the
    /// options themselves produce (unknown groups and specifiers), in
    /// option order: clang's second, reporting pass, which runs after
    /// every mapping is applied.
    pub(crate) fn new(opts: &DiagOptions) -> (Policy, Vec<OptionDiag>) {
        let map = DiagId::ALL
            .iter()
            .map(|&id| Mapping {
                severity: id.default_severity(),
                user: false,
                no_werror: id.kind() == crate::DiagKind::WarningNoWerror,
                no_fatal: false,
            })
            .collect();
        let ext_behavior = if opts.pedantic_errors {
            Severity::Error
        } else if opts.pedantic {
            Severity::Warning
        } else {
            Severity::Ignored
        };
        let mut p = Policy {
            map,
            ignore_all: opts.ignore_warnings,
            warnings_as_errors: false,
            errors_as_fatal: false,
            enable_all: false,
            ext_behavior,
            suppress_system: true,
        };
        let mut reports = Vec::new();
        for report in [false, true] {
            for opt in &opts.warnings {
                p.warning_option(opt, report, &mut reports);
            }
            for opt in &opts.remarks {
                p.remark_option(opt, report, &mut reports);
            }
        }
        (p, reports)
    }

    fn warning_option(&mut self, orig: &str, report: bool, out: &mut Vec<OptionDiag>) {
        let flavor = Flavor::WarningOrError;
        let opt = if orig == "format=0" { "no-format" } else { orig };
        let (positive, opt) = match opt.strip_prefix("no-") {
            Some(rest) => (false, rest),
            None => (true, opt),
        };
        if opt == "system-headers" {
            // clang `setSuppressSystemWarnings(!positive)`.
            if !report {
                self.suppress_system = !positive;
            }
            return;
        }
        if opt == "everything" {
            if !report {
                self.enable_all = positive;
                if !positive {
                    self.set_all(flavor, Severity::Ignored);
                }
            }
            return;
        }
        if let Some(rest) = opt.strip_prefix("error") {
            let spec = if rest.is_empty() {
                ""
            } else if let Some(s) = rest.strip_prefix('=') {
                s
            } else if rest == "-implicit-function-declaration" {
                "implicit-function-declaration"
            } else {
                if report {
                    out.push(unknown_specifier("-Werror", orig));
                }
                return;
            };
            if spec.is_empty() {
                if !report {
                    self.warnings_as_errors = positive;
                }
            } else if !report {
                self.group_as_error(spec, positive);
            } else if group_members(flavor, spec).is_none() {
                out.push(unknown_option(flavor, "-Werror=", spec));
            }
            return;
        }
        if let Some(rest) = opt.strip_prefix("fatal-errors") {
            let spec = if rest.is_empty() {
                ""
            } else if rest.len() > 1 && (rest.starts_with('=') || rest.starts_with('-')) {
                &rest[1..]
            } else {
                if report {
                    out.push(unknown_specifier("-Wfatal-errors", orig));
                }
                return;
            };
            if spec.is_empty() {
                if !report {
                    self.errors_as_fatal = positive;
                }
            } else if !report {
                self.group_as_fatal(spec, positive);
            } else if group_members(flavor, spec).is_none() {
                out.push(unknown_option(flavor, "-Wfatal-errors=", spec));
            }
            return;
        }
        if report {
            if group_members(flavor, opt).is_none() {
                out.push(unknown_option(flavor, if positive { "-W" } else { "-Wno-" }, opt));
            }
        } else {
            let sev = if positive { Severity::Warning } else { Severity::Ignored };
            self.set_group(flavor, opt, sev);
        }
    }

    fn remark_option(&mut self, orig: &str, report: bool, out: &mut Vec<OptionDiag>) {
        let flavor = Flavor::Remark;
        let (positive, opt) = match orig.strip_prefix("no-") {
            Some(rest) => (false, rest),
            None => (true, orig),
        };
        let sev = if positive { Severity::Remark } else { Severity::Ignored };
        if opt == "everything" {
            if !report {
                self.set_all(flavor, sev);
            }
            return;
        }
        if report {
            if group_members(flavor, opt).is_none() {
                out.push(unknown_option(flavor, if positive { "-R" } else { "-Rno-" }, opt));
            }
        } else {
            self.set_group(flavor, opt, sev);
        }
    }

    /// clang's `DiagnosticsEngine::setSeverity` for a command-line option.
    fn set_severity(&mut self, id: DiagId, mut sev: Severity) {
        let m = &mut self.map[id as usize];
        // A later `-Wfoo` does not undo an earlier `-Werror=foo`.
        if sev == Severity::Warning && matches!(m.severity, Severity::Error | Severity::Fatal) {
            sev = m.severity;
        }
        m.severity = sev;
        m.user = true;
    }

    fn set_group(&mut self, flavor: Flavor, group: &str, sev: Severity) {
        for id in group_members(flavor, group).unwrap_or_default() {
            self.set_severity(id, sev);
        }
    }

    fn set_all(&mut self, flavor: Flavor, sev: Severity) {
        for &id in DiagId::ALL {
            if flavor.matches(id)
                && (id.is_warning_or_extension() || id.class() == DiagClass::Remark)
            {
                self.set_severity(id, sev);
            }
        }
    }

    /// `-Werror=<group>` / `-Wno-error=<group>`.
    fn group_as_error(&mut self, group: &str, enable: bool) {
        if enable {
            self.set_group(Flavor::WarningOrError, group, Severity::Error);
            return;
        }
        for id in group_members(Flavor::WarningOrError, group).unwrap_or_default() {
            let m = &mut self.map[id as usize];
            if matches!(m.severity, Severity::Error | Severity::Fatal) {
                m.severity = Severity::Warning;
            }
            m.no_werror = true;
        }
    }

    /// `-Wfatal-errors=<group>` / `-Wno-fatal-errors=<group>`.
    fn group_as_fatal(&mut self, group: &str, enable: bool) {
        if enable {
            self.set_group(Flavor::WarningOrError, group, Severity::Fatal);
            return;
        }
        for id in group_members(Flavor::WarningOrError, group).unwrap_or_default() {
            let m = &mut self.map[id as usize];
            if m.severity == Severity::Fatal {
                m.severity = Severity::Error;
            }
            m.no_fatal = true;
        }
    }

    /// The severity `id` is emitted with (clang's `getDiagnosticSeverity`).
    /// Notes report `Ignored`; they follow their parent.
    pub(crate) fn severity(&self, id: DiagId) -> Severity {
        if id.class() == DiagClass::Note {
            return Severity::Ignored;
        }
        let m = self.map[id as usize];
        let mut r = m.severity;
        if self.enable_all && r == Severity::Ignored && !m.user && id.class() != DiagClass::Remark {
            r = Severity::Warning;
        }
        if id.class() == DiagClass::Extension && !m.user {
            r = r.max(self.ext_behavior);
        }
        if r == Severity::Ignored {
            return r;
        }
        if self.ignore_all
            && (r == Severity::Warning || (r >= Severity::Error && !id.is_default_error()))
        {
            return Severity::Ignored;
        }
        if r == Severity::Warning && self.warnings_as_errors && !m.no_werror {
            r = Severity::Error;
        }
        if r == Severity::Error && self.errors_as_fatal && !m.no_fatal {
            r = Severity::Fatal;
        }
        r
    }
}

fn unknown_option(flavor: Flavor, prefix: &str, name: &str) -> OptionDiag {
    let suggestion = nearest_group(flavor, name);
    OptionDiag {
        id: DiagId::diag_unknown_warning_option,
        args: vec![
            u8::from(flavor == Flavor::Remark).to_string(),
            format!("{prefix}{name}"),
            u8::from(suggestion.is_some()).to_string(),
            suggestion.map(|s| format!("{prefix}{s}")).unwrap_or_default(),
        ],
    }
}

fn unknown_specifier(option: &str, orig: &str) -> OptionDiag {
    OptionDiag {
        id: DiagId::diag_unknown_warning_specifier,
        args: vec![option.to_string(), format!("-W{orig}")],
    }
}
