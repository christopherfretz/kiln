//! The GCC/clang code generation flags build systems pass by default
//! (spec/driver.md "Codegen flags"). Each is in one of three classes:
//!
//! 1. honoured: it reaches the IR or the backend command line, as clang
//!    19 (`-fstack-protector*`, `-f[no-]unwind-tables`,
//!    `-f[no-]asynchronous-unwind-tables`, `-f[no-]plt`, `-f[no-]wrapv`,
//!    `-f[no-]wrapv-pointer`, `-f[no-]strict-overflow`,
//!    `-f[no-]delete-null-pointer-checks`, `-f[no-]function-sections`,
//!    `-f[no-]data-sections`, `-mtune=`, `-mcmodel=`, and WP-813's
//!    hardening and instrumentation flags: `-fcf-protection[=…]`,
//!    `-fpatchable-function-entry=`, `-fzero-call-used-regs=`,
//!    `-mfunction-return=`, `-m[no-]speculative-load-hardening`,
//!    `-f[no-]split-stack`, `-finstrument-functions[-after-inlining]`,
//!    `-finstrument-function-entry-bare`, `-f[no-]xray-instrument`,
//!    `-fxray-instruction-threshold=`, `-fxray-ignore-loops`);
//! 2. accepted and ignored: no observable effect in kiln
//!    (`-f[no-]stack-clash-protection`,
//!    `-f[no-]common`, `-fstack-usage`, `-gz[=…]`, the defaults `-fno-trapv`,
//!    `-fno-short-enums` and `-m64`, and `-pipe`);
//! 3. rejected with "'<flag>' is not supported by kiln" (`-ftrapv`,
//!    `-fshort-enums`, `-fsanitize*`, `-fno-sanitize*`, coverage and
//!    profiling, the other `-fxray-*` options and
//!    `-finstrument-functions-exclude-*`). The floating-point flags are
//!    `crate::fp_flags`'s.
//!
//! `-Wp,<args>` and `-Xpreprocessor <arg>` are spliced into the command
//! line before parsing ([`splice_preprocessor_args`]).

use kiln_codegen::{CodeModel, StackProtector, UnwindTables};

use crate::args::ArgError;

/// The code generation flags of a command line (the last of each pair
/// wins).
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodegenFlags {
    /// The last `-fstack-protector[-strong|-all]` / `-fno-stack-protector`.
    pub stack_protector: Option<StackProtector>,
    /// The last of `-funwind-tables` / `-fno-unwind-tables`.
    pub unwind_tables: Option<bool>,
    /// The last of `-fasynchronous-unwind-tables` /
    /// `-fno-asynchronous-unwind-tables`.
    pub async_unwind_tables: Option<bool>,
    /// The last of `-fno-plt` (true) / `-fplt`.
    pub no_plt: bool,
    /// The last of `-fwrapv` / `-fno-wrapv`.
    pub wrapv: Option<bool>,
    /// The last of `-fwrapv-pointer` / `-fno-wrapv-pointer` (clang 20+).
    pub wrapv_pointer: Option<bool>,
    /// The last of `-fstrict-overflow` / `-fno-strict-overflow`.
    pub strict_overflow: Option<bool>,
    /// The last of `-fdelete-null-pointer-checks` /
    /// `-fno-delete-null-pointer-checks`.
    pub delete_null_pointer_checks: Option<bool>,
    /// The last of `-ffunction-sections` / `-fno-function-sections`.
    pub function_sections: bool,
    /// The last of `-fdata-sections` / `-fno-data-sections`.
    pub data_sections: bool,
    /// The last `-mtune=<cpu>`.
    pub mtune: Option<String>,
    /// The last `-mcmodel=<model>`.
    pub code_model: Option<CodeModel>,
    /// The last `-fcf-protection[=<kind>]` (`-fcf-protection` is `full`).
    pub cf_protection: Option<&'static str>,
    /// The last `-fpatchable-function-entry=<count>[,<offset>]`.
    pub patchable_function_entry: Option<(u32, u32)>,
    /// The last `-fzero-call-used-regs=<kind>`.
    pub zero_call_used_regs: Option<&'static str>,
    /// The last `-mfunction-return=<keep|thunk-extern>`.
    pub function_return: Option<&'static str>,
    /// The last of `-mspeculative-load-hardening` /
    /// `-mno-speculative-load-hardening`.
    pub speculative_load_hardening: bool,
    /// The last of `-fsplit-stack` / `-fno-split-stack`.
    pub split_stack: bool,
    /// `-finstrument-functions`.
    pub instrument_functions: bool,
    /// `-finstrument-functions-after-inlining`.
    pub instrument_functions_after_inlining: bool,
    /// `-finstrument-function-entry-bare`.
    pub instrument_function_entry_bare: bool,
    /// The last of `-fxray-instrument` / `-fno-xray-instrument`.
    pub xray_instrument: bool,
    /// The last `-fxray-instruction-threshold=<n>` (as written, value).
    pub xray_threshold: Option<(String, Option<u32>)>,
    /// The other `-f[no-]xray-*` options, as written: with
    /// `-fxray-instrument`, not supported by kiln.
    pub xray_other: Vec<String>,
    /// Every XRay option but `-f[no-]xray-instrument`, as written, in
    /// command-line order ([`Self::unused_xray_args`]).
    pub xray_written: Vec<String>,
    /// A `-mfunction-return=` was given (even with an invalid value).
    pub function_return_given: bool,
    /// The last `-mfunction-return=`, `-fzero-call-used-regs=` and
    /// `-fcf-protection=` when its value is invalid (as written, value).
    pub invalid_values: [Option<(String, String)>; 3],
    /// `-fxray-ignore-loops` (the last of it and `-fno-xray-ignore-loops`).
    pub xray_ignore_loops: bool,
}

/// clang's `-fzero-call-used-regs=` values.
const ZERO_CALL_KINDS: &[&str] = &[
    "skip",
    "used-gpr-arg",
    "used-gpr",
    "used-arg",
    "used",
    "all-gpr-arg",
    "all-gpr",
    "all-arg",
    "all",
];

/// Flags accepted and ignored (class 2), exactly as written.
const IGNORED: &[&str] = &[
    "-fstack-clash-protection",
    "-fno-stack-clash-protection",
    "-fcommon",
    "-fno-common",
    "-fstack-usage",
    "-gz",
    "-fno-trapv",
    "-fno-short-enums",
    // Every target kiln has is 64-bit (`-m32` stays unknown).
    "-m64",
    // kiln's jobs talk through pipes already.
    "-pipe",
];

/// Flags rejected (class 3), exactly as written.
const REJECTED: &[&str] = &[
    "-ftrapv",
    "-fshort-enums",
    "-fcoverage-mapping",
    "-fprofile-arcs",
    "-ftest-coverage",
    "--coverage",
    "-fprofile-instr-generate",
    "-fprofile-generate",
    "-fprofile-instr-use",
    "-fprofile-use",
];

/// Prefixes of flags rejected (class 3): every value of the sanitizer
/// and profiling options.
const REJECTED_PREFIXES: &[&str] = &[
    "-fsanitize",
    "-fno-sanitize",
    "-fprofile-instr-generate=",
    "-fprofile-generate=",
    "-fprofile-instr-use=",
    "-fprofile-use=",
];

impl CodegenFlags {
    /// Takes `arg` if it is one of the flags: `None` if it is not, else
    /// whether it is accepted.
    pub fn parse_arg(&mut self, arg: &str) -> Option<Result<(), ArgError>> {
        let set = |slot: &mut Option<bool>, v: bool| *slot = Some(v);
        if (arg.starts_with("-fxray-") || arg.starts_with("-fno-xray-"))
            && !matches!(arg, "-fxray-instrument" | "-fno-xray-instrument")
        {
            self.xray_written.push(arg.to_owned());
        }
        match arg {
            "-fstack-protector" => self.stack_protector = Some(StackProtector::On),
            "-fstack-protector-strong" => self.stack_protector = Some(StackProtector::Strong),
            "-fstack-protector-all" => self.stack_protector = Some(StackProtector::All),
            "-fno-stack-protector" => self.stack_protector = Some(StackProtector::Off),
            "-funwind-tables" => set(&mut self.unwind_tables, true),
            "-fno-unwind-tables" => set(&mut self.unwind_tables, false),
            "-fasynchronous-unwind-tables" => set(&mut self.async_unwind_tables, true),
            "-fno-asynchronous-unwind-tables" => set(&mut self.async_unwind_tables, false),
            "-fno-plt" => self.no_plt = true,
            "-fplt" => self.no_plt = false,
            "-fwrapv" => set(&mut self.wrapv, true),
            "-fno-wrapv" => set(&mut self.wrapv, false),
            "-fwrapv-pointer" => set(&mut self.wrapv_pointer, true),
            "-fno-wrapv-pointer" => set(&mut self.wrapv_pointer, false),
            "-fstrict-overflow" => set(&mut self.strict_overflow, true),
            "-fno-strict-overflow" => set(&mut self.strict_overflow, false),
            "-fdelete-null-pointer-checks" => set(&mut self.delete_null_pointer_checks, true),
            "-fno-delete-null-pointer-checks" => {
                set(&mut self.delete_null_pointer_checks, false);
            }
            "-ffunction-sections" => self.function_sections = true,
            "-fno-function-sections" => self.function_sections = false,
            "-fdata-sections" => self.data_sections = true,
            "-fno-data-sections" => self.data_sections = false,
            "-fcf-protection" => {
                self.cf_protection = Some("full");
                self.invalid_values[2] = None;
            }
            "-mspeculative-load-hardening" => self.speculative_load_hardening = true,
            "-mno-speculative-load-hardening" => self.speculative_load_hardening = false,
            "-fsplit-stack" => self.split_stack = true,
            "-fno-split-stack" => self.split_stack = false,
            "-finstrument-functions" => self.instrument_functions = true,
            "-finstrument-functions-after-inlining" => {
                self.instrument_functions_after_inlining = true;
            }
            "-finstrument-function-entry-bare" => self.instrument_function_entry_bare = true,
            "-fxray-instrument" => self.xray_instrument = true,
            "-fno-xray-instrument" => self.xray_instrument = false,
            "-fxray-ignore-loops" => self.xray_ignore_loops = true,
            "-fno-xray-ignore-loops" => self.xray_ignore_loops = false,
            _ if IGNORED.contains(&arg) => {}
            _ if arg.starts_with("-fxray-") || arg.starts_with("-fno-xray-") => {
                if !arg.contains('=') {
                    self.xray_other.push(arg.to_owned());
                    return Some(Ok(()));
                }
                return self.valued(arg);
            }
            _ if REJECTED.contains(&arg)
                || REJECTED_PREFIXES.iter().any(|p| arg.starts_with(p)) =>
            {
                return Some(Err(ArgError::UnsupportedByKiln(arg.to_owned())));
            }
            _ => return self.valued(arg),
        }
        Some(Ok(()))
    }

    /// The `=`-valued flags.
    fn valued(&mut self, arg: &str) -> Option<Result<(), ArgError>> {
        let (flag, v) = arg.split_once('=')?;
        let unsupported = || Some(Err(ArgError::UnsupportedValue(format!("{flag}="), v.into())));
        // `-cc1`'s "invalid value" for the last one ([`Self::invocation_errors`]).
        let invalid = |me: &mut Self, slot: usize| {
            me.invalid_values[slot] = Some((arg.to_owned(), v.to_owned()));
            Some(Ok(()))
        };
        match flag {
            "-mtune" => {
                if v.is_empty() {
                    return unsupported();
                }
                self.mtune = Some(v.to_owned());
            }
            "-mcmodel" => match CodeModel::from_name(v) {
                Some(m) => self.code_model = Some(m),
                None => return unsupported(),
            },
            // clang's values; an unknown one is its "invalid value".
            "-mfunction-return" => {
                self.function_return_given = true;
                self.invalid_values[0] = None;
                match ["keep", "thunk-extern"].into_iter().find(|k| *k == v) {
                    Some(k) => self.function_return = Some(k),
                    None => return invalid(self, 0),
                }
            }
            "-fzero-call-used-regs" => {
                self.invalid_values[1] = None;
                match ZERO_CALL_KINDS.iter().find(|k| **k == v) {
                    Some(k) => self.zero_call_used_regs = Some(k),
                    None => return invalid(self, 1),
                }
            }
            "-fcf-protection" => {
                self.invalid_values[2] = None;
                match ["full", "branch", "return", "none"].into_iter().find(|k| *k == v) {
                    Some(k) => self.cf_protection = Some(k),
                    None => return invalid(self, 2),
                }
            }
            // clang `RenderFunctionPatchableEntry`... (`Clang.cpp`):
            // `<count>[,<offset>]`, the offset at most the count.
            "-fpatchable-function-entry" => {
                let mut it = v.splitn(2, ',');
                let n = it.next().and_then(|n| n.parse::<u32>().ok());
                let o = it.next().map_or(Some(0), |o| o.parse::<u32>().ok());
                match (n, o) {
                    (Some(n), Some(o)) if o <= n => self.patchable_function_entry = Some((n, o)),
                    (Some(_), Some(_)) => return Some(Err(ArgError::PatchableEntryOffset)),
                    _ => {
                        return Some(Err(ArgError::InvalidArgumentToOption(
                            v.to_owned(),
                            "fpatchable-function-entry=".to_owned(),
                        )));
                    }
                }
            }
            // Checked with `-fxray-instrument` only ([`Self::xray_errors`]).
            "-fxray-instruction-threshold" => {
                self.xray_threshold = Some((arg.to_owned(), v.parse::<u32>().ok()));
            }
            _ if flag.starts_with("-fxray-") || flag.starts_with("-fno-xray-") => {
                self.xray_other.push(arg.to_owned());
            }
            "-gz" => {
                if !matches!(v, "none" | "zlib" | "zstd") {
                    return unsupported();
                }
            }
            _ => return None,
        }
        Some(Ok(()))
    }

    /// The stack protector level (clang's default: off).
    pub fn stack_protector(&self) -> StackProtector {
        self.stack_protector.unwrap_or_default()
    }

    /// clang 19 `RenderUnwindTableOptions` / `IsUnwindTablesDefault` on
    /// Linux: asynchronous tables unless `-fno-asynchronous-unwind-tables`
    /// or freestanding, else synchronous ones with `-funwind-tables`,
    /// else none.
    pub fn unwind_tables(&self, freestanding: bool) -> Option<UnwindTables> {
        if self.async_unwind_tables.unwrap_or(!freestanding) {
            Some(UnwindTables::Async)
        } else if self.unwind_tables.unwrap_or(false) {
            Some(UnwindTables::Sync)
        } else {
            None
        }
    }

    /// Signed overflow is defined: the last of `-f[no-]wrapv`, else
    /// `-fno-strict-overflow` (clang `RenderSCPOptions`… `-fwrapv`).
    pub fn wrapv(&self) -> bool {
        self.wrapv.or(self.strict_overflow.map(|s| !s)).unwrap_or(false)
    }

    /// Pointer overflow is defined: at `-fclang-compat=19` the same as
    /// [`Self::wrapv`] (clang 19 has one switch); at 22 the last of
    /// `-f[no-]wrapv-pointer`, else `-fno-strict-overflow`.
    pub fn wrapv_pointer(&self, compat22: bool) -> bool {
        if compat22 {
            self.wrapv_pointer.or(self.strict_overflow.map(|s| !s)).unwrap_or(false)
        } else {
            self.wrapv()
        }
    }

    /// `-fno-delete-null-pointer-checks` is in effect.
    pub fn null_pointer_is_valid(&self) -> bool {
        self.delete_null_pointer_checks == Some(false)
    }

    /// The hardening and instrumentation options for a target (`x86`:
    /// x86_64, else AArch64; the driver reports the flags the target
    /// rejects, [`Self::target_errors`]).
    pub fn hardening(&self, x86: bool) -> kiln_codegen::Hardening {
        let cf = self.cf_protection.unwrap_or("none");
        kiln_codegen::Hardening {
            split_stack: self.split_stack,
            speculative_load_hardening: self.speculative_load_hardening,
            zero_call_used_regs: self.zero_call_used_regs,
            fn_return_thunk_extern: x86 && self.function_return == Some("thunk-extern"),
            patchable_function_entry: self.patchable_function_entry,
            xray: self.xray_instrument.then(|| kiln_codegen::Xray {
                threshold: self.xray_threshold.as_ref().and_then(|t| t.1).unwrap_or(200),
                ignore_loops: self.xray_ignore_loops,
            }),
            instrument_functions: self.instrument_functions,
            instrument_functions_after_inlining: self.instrument_functions_after_inlining,
            instrument_function_entry_bare: self.instrument_function_entry_bare,
            cf_protection_branch: x86 && matches!(cf, "full" | "branch"),
            cf_protection_return: x86 && matches!(cf, "full" | "return"),
        }
    }

    /// `nocf_check` is kept: `-fcf-protection=branch` or `full` on x86
    /// (clang `CodeGenOpts.CFProtectionBranch`).
    pub fn cf_protection_branch(&self, x86: bool) -> bool {
        x86 && matches!(self.cf_protection, Some("full" | "branch"))
    }

    /// The errors of the `-cc1` command line, in clang 19's
    /// `CompilerInvocation` order: the invalid values of the marshalled
    /// `-mfunction-return=` and `-fzero-call-used-regs=`, then of
    /// `-fcf-protection=`, then `ParseCodeGenArgs`'s own
    /// `-mfunction-return=` check (off x86 "not allowed with", on x86
    /// the invalid value again). The job then stops with no error count.
    pub fn invocation_errors(&self, x86: bool, triple: &str) -> Vec<TargetError> {
        let invalid = |(a, x): &(String, String)| TargetError::InvalidValue(a.clone(), x.clone());
        let mut v: Vec<TargetError> = self.invalid_values.iter().flatten().map(invalid).collect();
        if self.function_return_given && !x86 {
            v.push(TargetError::NotAllowedWith("-mfunction-return=", triple.to_owned()));
        } else if let Some(e) = &self.invalid_values[0] {
            v.push(invalid(e));
        }
        v
    }

    /// The code generator's errors for the flags the target rejects:
    /// `-fcf-protection` off x86 (`TargetInfo::
    /// checkCFProtection{Return,Branch}Supported`), in clang's order.
    pub fn target_errors(&self, x86: bool) -> Vec<TargetError> {
        let mut v = Vec::new();
        let cf = self.cf_protection.unwrap_or("none");
        if !x86 && matches!(cf, "full" | "return") {
            v.push(TargetError::NotValidOnTarget("cf-protection=return"));
        }
        if !x86 && matches!(cf, "full" | "branch") {
            v.push(TargetError::NotValidOnTarget("cf-protection=branch"));
        }
        v
    }

    /// The driver's errors for the XRay options (clang `XRayArgs`): with
    /// `-fxray-instrument`, an invalid instruction threshold, the
    /// options kiln does not implement and `-fpatchable-function-entry=`.
    pub fn xray_errors(&self) -> Vec<kiln_diag::Diag> {
        use kiln_diag::{Diag, DiagId};
        use kiln_source::Span;
        let mut v = Vec::new();
        if !self.xray_instrument {
            return v;
        }
        if let Some((a, None)) = &self.xray_threshold {
            let x = a.split_once('=').map_or("", |p| p.1);
            v.push(Diag::new(DiagId::drv_invalid_value, Span::INVALID).arg(a).arg(x));
        }
        for a in &self.xray_other {
            v.push(Diag::new(DiagId::drv_unsupported_flag, Span::INVALID).arg(a));
        }
        if self.patchable_function_entry.is_some() {
            v.push(
                Diag::new(DiagId::drv_argument_not_allowed_with, Span::INVALID)
                    .arg("-fxray-instrument")
                    .arg("-fpatchable-function-entry="),
            );
        }
        v
    }

    /// The XRay options given without `-fxray-instrument`, as written
    /// (clang: "argument unused during compilation").
    pub fn unused_xray_args(&self) -> Vec<String> {
        if self.xray_instrument {
            return Vec::new();
        }
        self.xray_written.clone()
    }
}

/// A `-cc1` error for a flag the target rejects
/// ([`CodegenFlags::target_errors`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetError {
    /// "invalid value '{1}' in '{0}'".
    InvalidValue(String, String),
    /// "invalid argument '{0}' not allowed with '{1}'".
    NotAllowedWith(&'static str, String),
    /// "option '{0}' cannot be specified on this target".
    NotValidOnTarget(&'static str),
}

impl TargetError {
    /// Reports the error.
    pub fn emit(&self, engine: &kiln_diag::DiagEngine<'_>) {
        use kiln_diag::{Diag, DiagId};
        use kiln_source::Span;
        let d = match self {
            TargetError::InvalidValue(a, v) => {
                Diag::new(DiagId::drv_invalid_value, Span::INVALID).arg(a).arg(v)
            }
            TargetError::NotAllowedWith(a, t) => {
                Diag::new(DiagId::drv_argument_not_allowed_with, Span::INVALID).arg(*a).arg(t)
            }
            TargetError::NotValidOnTarget(o) => {
                Diag::new(DiagId::drv_opt_not_valid_on_target, Span::INVALID).arg(*o)
            }
        };
        d.emit(engine);
    }
}

/// The command line with each `-Wp,<a>,<b>…` replaced by its
/// comma-separated values and each `-Xpreprocessor <arg>` by `<arg>`
/// (GCC: options for the preprocessor, which in kiln is the compiler).
/// As clang, `-Wp,-MD,<file>` / `-Wp,-MMD,<file>` become `-MD`/`-MMD`
/// plus `-MF <file>`. A trailing `-Xpreprocessor` stays (and is reported
/// as missing its value). Values of other options are not inspected.
pub fn splice_preprocessor_args(argv: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(argv.len());
    let mut it = argv.iter().peekable();
    while let Some(a) = it.next() {
        if let Some(list) = a.strip_prefix("-Wp,") {
            let vals: Vec<&str> = list.split(',').collect();
            match vals.as_slice() {
                [md @ ("-MD" | "-MMD"), file] => {
                    out.extend([(*md).to_owned(), "-MF".to_owned(), (*file).to_owned()]);
                }
                _ => out.extend(vals.iter().filter(|v| !v.is_empty()).map(|v| (*v).to_owned())),
            }
        } else if a == "-Xpreprocessor" && it.peek().is_some() {
            out.extend(it.next().cloned());
        } else {
            out.push(a.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> (CodegenFlags, Vec<Option<Result<(), ArgError>>>) {
        let mut f = CodegenFlags::default();
        let r = args.iter().map(|a| f.parse_arg(a)).collect();
        (f, r)
    }

    #[test]
    fn stack_protector_last_wins() {
        let (f, _) = parse(&["-fstack-protector-all", "-fstack-protector-strong"]);
        assert_eq!(f.stack_protector(), StackProtector::Strong);
        let (f, _) = parse(&["-fstack-protector", "-fno-stack-protector"]);
        assert_eq!(f.stack_protector(), StackProtector::Off);
        assert_eq!(CodegenFlags::default().stack_protector(), StackProtector::Off);
    }

    #[test]
    fn unwind_tables_rule() {
        let d = CodegenFlags::default();
        assert_eq!(d.unwind_tables(false), Some(UnwindTables::Async));
        assert_eq!(d.unwind_tables(true), None);
        let (f, _) = parse(&["-funwind-tables"]);
        assert_eq!(f.unwind_tables(false), Some(UnwindTables::Async));
        assert_eq!(f.unwind_tables(true), Some(UnwindTables::Sync));
        let (f, _) = parse(&["-fno-asynchronous-unwind-tables"]);
        assert_eq!(f.unwind_tables(false), None);
        let (f, _) = parse(&["-fno-asynchronous-unwind-tables", "-funwind-tables"]);
        assert_eq!(f.unwind_tables(false), Some(UnwindTables::Sync));
        let (f, _) = parse(&["-ffreestanding", "-fasynchronous-unwind-tables"]);
        assert_eq!(f.unwind_tables(true), Some(UnwindTables::Async));
    }

    #[test]
    fn wrapv_rule() {
        let (f, _) = parse(&["-fno-strict-overflow"]);
        assert!(f.wrapv() && f.wrapv_pointer(false) && f.wrapv_pointer(true));
        let (f, _) = parse(&["-fwrapv"]);
        assert!(f.wrapv() && f.wrapv_pointer(false) && !f.wrapv_pointer(true));
        let (f, _) = parse(&["-fno-wrapv", "-fno-strict-overflow"]);
        assert!(!f.wrapv() && f.wrapv_pointer(true));
        let (f, _) = parse(&["-fwrapv-pointer"]);
        assert!(!f.wrapv() && f.wrapv_pointer(true) && !f.wrapv_pointer(false));
    }

    #[test]
    fn classes() {
        for a in [
            "-fstack-clash-protection",
            "-fcf-protection",
            "-fcf-protection=full",
            "-fno-common",
            "-fstack-usage",
            "-gz",
            "-gz=zlib",
            "-mtune=native",
            "-ffunction-sections",
            "-fdata-sections",
        ] {
            assert_eq!(parse(&[a]).1, [Some(Ok(()))], "{a}");
        }
        for a in [
            "-ftrapv",
            "-fshort-enums",
            "-fsanitize=address",
            "-fno-sanitize-recover=all",
            "-fsanitize-trap=undefined",
            "-fcoverage-mapping",
            "-fprofile-arcs",
            "-ftest-coverage",
            "--coverage",
            "-fprofile-instr-generate",
            "-fprofile-instr-generate=x.profraw",
            "-fprofile-use=x.prof",
        ] {
            assert_eq!(parse(&[a]).1, [Some(Err(ArgError::UnsupportedByKiln(a.into())))], "{a}");
        }
        assert_eq!(
            parse(&["-gz=lzma"]).1,
            [Some(Err(ArgError::UnsupportedValue("-gz=".into(), "lzma".into())))]
        );
        assert_eq!(
            parse(&["-fcf-protection=bogus"]).1,
            [Some(Err(ArgError::InvalidFlagValue("-fcf-protection=bogus".into(), "bogus".into())))]
        );
        assert_eq!(parse(&["-fbogus"]).1, [None]);
        assert_eq!(parse(&["-mtune=x86-64"]).0.mtune.as_deref(), Some("x86-64"));
    }

    #[test]
    fn splice() {
        let v = |s: &[&str]| s.iter().map(|&x| x.to_owned()).collect::<Vec<_>>();
        assert_eq!(
            splice_preprocessor_args(&v(&["-Wp,-DFOO=1,-UBAR", "a.cpp"])),
            v(&["-DFOO=1", "-UBAR", "a.cpp"])
        );
        assert_eq!(splice_preprocessor_args(&v(&["-Wp,-MD,a.d"])), v(&["-MD", "-MF", "a.d"]));
        assert_eq!(splice_preprocessor_args(&v(&["-Wp,-MMD,a.d"])), v(&["-MMD", "-MF", "a.d"]));
        assert_eq!(
            splice_preprocessor_args(&v(&["-Xpreprocessor", "-DX", "-Xpreprocessor"])),
            v(&["-DX", "-Xpreprocessor"])
        );
    }
}
