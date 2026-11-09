//! The GCC/clang code generation flags build systems pass by default
//! (spec/driver.md "Codegen flags"). Each is in one of three classes:
//!
//! 1. honoured: it reaches the IR or the backend command line, as clang
//!    19 (`-fstack-protector*`, `-f[no-]unwind-tables`,
//!    `-f[no-]asynchronous-unwind-tables`, `-f[no-]plt`, `-f[no-]wrapv`,
//!    `-f[no-]wrapv-pointer`, `-f[no-]strict-overflow`,
//!    `-f[no-]c++-static-destructors[=…]`,
//!    `-f[no-]delete-null-pointer-checks`, `-f[no-]function-sections`,
//!    `-f[no-]data-sections`, `-mtune=`, `-mcmodel=`, and WP-813's
//!    hardening and instrumentation flags: `-fcf-protection[=…]`,
//!    `-fpatchable-function-entry=`, `-fzero-call-used-regs=`,
//!    `-mfunction-return=`, `-m[no-]speculative-load-hardening`,
//!    `-f[no-]split-stack`, `-finstrument-functions[-after-inlining]`,
//!    `-finstrument-function-entry-bare`, `-f[no-]xray-instrument`,
//!    `-fxray-instruction-threshold=`, `-f[no-]xray-ignore-loops`; WP-842's
//!    `-mbranch-protection=` and `-msign-return-address=` (`AArch64`);
//!    WP-961's `-mbranches-within-32B-boundaries` (x86-64); WP-873's
//!    `-flto[=full|thin|auto|jobserver]`, `-fno-lto`, `-f[no-]split-lto-unit`;
//!    and WP-1032's `-ftrapv`, `-fno-trapv` (a kiln spelling: clang has
//!    none, GCC's cancels `-ftrapv`) and `-ftrapv-handler=`), the target
//!    checks of which are [`CodegenFlags::target_errors`];
//! 2. accepted and ignored: no observable effect in kiln
//!    (`-f[no-]stack-clash-protection`,
//!    `-f[no-]common`, `-fstack-usage`, the defaults
//!    `-fno-short-enums` and `-m64`, and `-pipe`), or deliberately ignored
//!    although clang honours it (WP-873: `-fdiagnostics-show-template-tree`,
//!    `-f[no-]strict-vtable-pointers`, `-f[no-]vectorize`,
//!    `-f[no-]slp-vectorize`, `-f[no-]tree-[slp-]vectorize`,
//!    `-fno-fat-lto-objects`; WP-982: `-f[no-]modules-reduced-bmi`;
//!    spec/driver.md "Codegen flags");
//! 3. rejected with "'<flag>' is not supported by kiln": `-fshort-enums`
//!    and `-ffat-lto-objects`, and, only with `-fxray-instrument`, the
//!    other `-fxray-*` options ([`CodegenFlags::xray_errors`]; without it
//!    they are clang's "argument unused during compilation",
//!    [`CodegenFlags::unused_xray_args`]).
//!
//! GCC's `-finstrument-functions-exclude-*` is not a clang option and is
//! an unknown argument, as in clang.
//!
//! The floating-point flags are `crate::fp_flags`'s, the coverage and
//! profiling flags `crate::coverage_flags`'s (WP-941), and the sanitizer
//! flags, `-f[no-]sanitize-merge[=]` and `-fsanitize-ignorelist=` among
//! them, `crate::sanitize`'s.
//!
//! `-Wp,<args>` and `-Xpreprocessor <arg>` are spliced into the command
//! line before parsing ([`splice_preprocessor_args`]).

use kiln_codegen::{CodeModel, StackProtector, StaticDestructors, UnwindTables};

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
    /// The last of `-fc++-static-destructors[=all|thread-local|none]` /
    /// `-fno-c++-static-destructors` (WP-805).
    pub static_destructors: StaticDestructors,
    /// The last of `-fwrapv` / `-fno-wrapv`.
    pub wrapv: Option<bool>,
    /// The last of `-fwrapv-pointer` / `-fno-wrapv-pointer` (clang 20+).
    pub wrapv_pointer: Option<bool>,
    /// The last of `-ftrapv` (true) / `-fno-trapv` (WP-1032).
    pub trapv: bool,
    /// The last `-ftrapv-handler=<name>` (empty: none).
    pub trapv_handler: Option<String>,
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
    /// `-mbranches-within-32B-boundaries` (x86; WP-961): `llc
    /// -x86-branches-within-32B-boundaries`, as clang's
    /// `addX86AlignBranchArgs`. Off x86 an "unsupported option ... for
    /// target" error. clang has no negative form.
    pub branches_within_32b: bool,
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
    /// Every `XRay` option but `-f[no-]xray-instrument`, as written, in
    /// command-line order ([`Self::unused_xray_args`]).
    pub xray_written: Vec<String>,
    /// A `-mfunction-return=` was given (even with an invalid value).
    pub function_return_given: bool,
    /// The last `-mfunction-return=`, `-fzero-call-used-regs=` and
    /// `-fcf-protection=` when its value is invalid (as written, value).
    pub invalid_values: [Option<(String, String)>; 3],
    /// `-fxray-ignore-loops` (the last of it and `-fno-xray-ignore-loops`).
    pub xray_ignore_loops: bool,
    /// The last of `-flto[=full|thin|auto|jobserver]` / `-fno-lto`:
    /// `Some(false)` full LTO, `Some(true)` thin (WP-873).
    pub lto: Option<bool>,
    /// The last of `-fsplit-lto-unit` / `-fno-split-lto-unit`.
    pub split_lto_unit: Option<bool>,
    /// The last of `-mbranch-protection=<spec>` / `-msign-return-address=
    /// <scope>` (`AArch64`; WP-846): (option as written up to `=`, value).
    pub branch_protection: Option<(&'static str, String)>,
    /// A `-mbranch-protection=` was given (off `AArch64`, clang's
    /// "unsupported option ... for target").
    pub mbranch_protection_given: bool,
    /// The `-msign-return-address=` options as written (off `AArch64`,
    /// unused).
    pub sign_return_address_written: Vec<String>,
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
    "-fno-short-enums",
    // Every target kiln has is 64-bit (`-m32` stays unknown).
    "-m64",
    // kiln's jobs talk through pipes already.
    "-pipe",
    // WP-677: clang accepts these (CMake and configure scripts probe
    // them). The template-tree printing is not implemented; the
    // vectorizer and strict vtable pointer flags change clang's IR but are
    // deliberately ignored (spec/driver.md "Codegen flags").
    "-fdiagnostics-show-template-tree",
    "-fstrict-vtable-pointers",
    "-fno-strict-vtable-pointers",
    "-fvectorize",
    "-fno-vectorize",
    "-fslp-vectorize",
    "-fno-slp-vectorize",
    "-ftree-vectorize",
    "-fno-tree-vectorize",
    "-ftree-slp-vectorize",
    "-fno-tree-slp-vectorize",
    // The default: kiln's LTO objects are bitcode only.
    "-fno-fat-lto-objects",
    // WP-982: kiln's module file has one form, the source capsule, which
    // is already as small as a reduced BMI gets (spec/modules.md §11.9).
    "-fmodules-reduced-bmi",
    "-fno-modules-reduced-bmi",
];

/// Flags rejected (class 3), exactly as written.
const REJECTED: &[&str] = &[
    "-fshort-enums",
    // Bitcode plus object code in one `.o` (WP-873 follow-up).
    "-ffat-lto-objects",
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
            "-fc++-static-destructors" => self.static_destructors = StaticDestructors::All,
            "-fno-c++-static-destructors" => self.static_destructors = StaticDestructors::None,
            "-fwrapv" => set(&mut self.wrapv, true),
            "-fno-wrapv" => set(&mut self.wrapv, false),
            "-ftrapv" => self.trapv = true,
            "-fno-trapv" => self.trapv = false,
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
            "-mbranches-within-32B-boundaries" => self.branches_within_32b = true,
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
            "-flto" => self.lto = Some(false),
            "-fno-lto" => self.lto = None,
            "-fsplit-lto-unit" => self.split_lto_unit = Some(true),
            "-fno-split-lto-unit" => self.split_lto_unit = Some(false),
            _ if IGNORED.contains(&arg) => {}
            _ if arg.starts_with("-fxray-") || arg.starts_with("-fno-xray-") => {
                if !arg.contains('=') {
                    self.xray_other.push(arg.to_owned());
                    return Some(Ok(()));
                }
                return self.valued(arg);
            }
            _ if REJECTED.contains(&arg) => {
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
            // clang `getLTOMode`: `auto` and `jobserver` are full LTO.
            "-flto" => match v {
                "full" | "auto" | "jobserver" => self.lto = Some(false),
                "thin" => self.lto = Some(true),
                _ => return unsupported(),
            },
            "-ftrapv-handler" => self.trapv_handler = Some(v.to_owned()),
            "-mtune" => {
                if v.is_empty() {
                    return unsupported();
                }
                self.mtune = Some(v.to_owned());
            }
            // clang's values; an unknown one is its "invalid value".
            "-fc++-static-destructors" => {
                self.static_destructors = match v {
                    "all" => StaticDestructors::All,
                    "thread-local" => StaticDestructors::ThreadLocal,
                    "none" => StaticDestructors::None,
                    _ => {
                        return Some(Err(ArgError::InvalidFlagValue(arg.to_owned(), v.to_owned())));
                    }
                };
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
            // WP-846: checked against the target ([`Self::branch_protection`]).
            "-mbranch-protection" => {
                self.mbranch_protection_given = true;
                self.branch_protection = Some(("-mbranch-protection=", v.to_owned()));
            }
            "-msign-return-address" => {
                self.sign_return_address_written.push(arg.to_owned());
                self.branch_protection = Some(("-msign-return-address=", v.to_owned()));
            }
            // Checked with `-fxray-instrument` only ([`Self::xray_errors`]).
            "-fxray-instruction-threshold" => {
                self.xray_threshold = Some((arg.to_owned(), v.parse::<u32>().ok()));
            }
            _ if flag.starts_with("-fxray-") || flag.starts_with("-fno-xray-") => {
                self.xray_other.push(arg.to_owned());
            }
            _ => return None,
        }
        Some(Ok(()))
    }

    /// The module's LTO kind (clang: a thin unit is split only with
    /// `-fsplit-lto-unit`, a full one always).
    pub fn lto(&self) -> Option<kiln_codegen::Lto> {
        self.lto.map(|thin| {
            if thin {
                kiln_codegen::Lto::Thin { split: self.split_lto_unit.unwrap_or(false) }
            } else {
                kiln_codegen::Lto::Full
            }
        })
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
        // `-ftrapv` wins (clang `SOB_Trapping`, set after `SOB_Defined`).
        !self.trapv && self.wrapv.or(self.strict_overflow.map(|s| !s)).unwrap_or(false)
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
    /// `x86_64`, else `AArch64`; the driver reports the flags the target
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
            branch_protection: self.branch_protection(!x86).unwrap_or_default(),
        }
    }

    /// The branch protection of clang 19 `CollectARMPACBTIOptions`: on
    /// `AArch64` the last of `-mbranch-protection=` (an invalid spec is
    /// "unsupported argument '<item>' to option '-mbranch-protection='")
    /// and `-msign-return-address=none|non-leaf|all` (the A key, no BTI,
    /// PAuth-LR or GCS); off `AArch64` none (the driver reports
    /// `-mbranch-protection=` as unsupported for the target and
    /// `-msign-return-address=` as unused). kiln knows no `-march=`
    /// extension enabling PAuth-LR, so `standard` never has it.
    pub fn branch_protection(&self, aarch64: bool) -> Result<kiln_abi::BranchProtection, ArgError> {
        use kiln_abi::{BranchProtection, SignScope};
        let Some((opt, v)) = self.branch_protection.as_ref().filter(|_| aarch64) else {
            return Ok(BranchProtection::default());
        };
        let bad = |x: &str| ArgError::UnsupportedValue((*opt).to_owned(), x.to_owned());
        if *opt == "-msign-return-address=" {
            let scope = SignScope::from_name(v).ok_or_else(|| bad(v))?;
            return Ok(BranchProtection { scope, ..BranchProtection::default() });
        }
        BranchProtection::parse(v, false).map_err(|x| bad(&x))
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

    /// The driver's errors for the `XRay` options (clang `XRayArgs`): with
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

    /// The `XRay` options given without `-fxray-instrument`, as written
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
            "-mtune=native",
            "-ffunction-sections",
            "-fdata-sections",
        ] {
            assert_eq!(parse(&[a]).1, [Some(Ok(()))], "{a}");
        }
        let a = "-fshort-enums";
        assert_eq!(parse(&[a]).1, [Some(Err(ArgError::UnsupportedByKiln(a.into())))], "{a}");
        assert_eq!(parse(&["-fbogus"]).1, [None]);
        assert_eq!(parse(&["-mtune=x86-64"]).0.mtune.as_deref(), Some("x86-64"));
    }

    /// WP-873: `-flto[=...]`, `-f[no-]split-lto-unit`, and the flags
    /// `CMake` probes that kiln accepts and ignores (WP-677).
    #[test]
    fn lto_and_probe_flags() {
        use kiln_codegen::Lto;
        assert_eq!(CodegenFlags::default().lto(), None);
        for (a, want) in [
            ("-flto", Some(Lto::Full)),
            ("-flto=full", Some(Lto::Full)),
            ("-flto=auto", Some(Lto::Full)),
            ("-flto=jobserver", Some(Lto::Full)),
            ("-flto=thin", Some(Lto::Thin { split: false })),
        ] {
            let (f, r) = parse(&[a]);
            assert_eq!(r, [Some(Ok(()))], "{a}");
            assert_eq!(f.lto(), want, "{a}");
        }
        assert_eq!(parse(&["-flto=thin", "-fno-lto"]).0.lto(), None);
        assert_eq!(parse(&["-fno-lto", "-flto"]).0.lto(), Some(Lto::Full));
        let (f, _) = parse(&["-fsplit-lto-unit", "-flto=thin"]);
        assert_eq!(f.lto(), Some(Lto::Thin { split: true }));
        let (f, _) = parse(&["-flto=thin", "-fsplit-lto-unit", "-fno-split-lto-unit"]);
        assert_eq!(f.lto(), Some(Lto::Thin { split: false }));
        assert_eq!(
            parse(&["-flto=bogus"]).1,
            [Some(Err(ArgError::UnsupportedValue("-flto=".into(), "bogus".into())))]
        );
        assert_eq!(
            parse(&["-ffat-lto-objects"]).1,
            [Some(Err(ArgError::UnsupportedByKiln("-ffat-lto-objects".into())))]
        );
        for a in [
            "-fdiagnostics-show-template-tree",
            "-fstrict-vtable-pointers",
            "-fno-strict-vtable-pointers",
            "-fvectorize",
            "-fno-vectorize",
            "-fslp-vectorize",
            "-fno-slp-vectorize",
            "-ftree-vectorize",
            "-fno-tree-slp-vectorize",
            "-fno-fat-lto-objects",
            "-fmodules-reduced-bmi",
            "-fno-modules-reduced-bmi",
        ] {
            let (f, r) = parse(&[a]);
            assert_eq!(r, [Some(Ok(()))], "{a}");
            assert_eq!(f, CodegenFlags::default(), "{a}");
        }
    }

    /// WP-813: the hardening and instrumentation flags.
    #[test]
    fn hardening() {
        let (flags, results) = parse(&[
            "-fcf-protection",
            "-fpatchable-function-entry=3,1",
            "-fzero-call-used-regs=used-gpr",
            "-mfunction-return=thunk-extern",
            "-mspeculative-load-hardening",
            "-fsplit-stack",
            "-finstrument-functions",
            "-finstrument-function-entry-bare",
            "-fxray-instrument",
            "-fxray-instruction-threshold=7",
            "-fxray-ignore-loops",
        ]);
        assert!(results.iter().all(|r| *r == Some(Ok(()))), "{results:?}");
        let h = flags.hardening(true);
        assert!(h.cf_protection_branch && h.cf_protection_return && h.fn_return_thunk_extern);
        assert_eq!(h.patchable_function_entry, Some((3, 1)));
        assert_eq!(h.zero_call_used_regs, Some("used-gpr"));
        assert!(h.speculative_load_hardening && h.split_stack && h.instrument_functions);
        assert!(h.instrument_function_entry_bare && !h.instrument_functions_after_inlining);
        let x = h.xray.expect("xray");
        assert_eq!((x.threshold, x.ignore_loops), (7, true));
        // Off x86 the x86-only options are dropped and reported.
        let a = flags.hardening(false);
        assert!(!a.cf_protection_branch && !a.cf_protection_return && !a.fn_return_thunk_extern);
        assert_eq!(flags.target_errors(false).len(), 2);
        assert!(flags.target_errors(true).is_empty());
        let triple = "aarch64-unknown-linux-gnu";
        assert_eq!(
            flags.invocation_errors(false, triple),
            [TargetError::NotAllowedWith("-mfunction-return=", triple.into())]
        );
        assert!(flags.invocation_errors(true, triple).is_empty());
        // XRay with patchable entries is the driver's error.
        assert_eq!(flags.xray_errors().len(), 1);
        assert!(!flags.cf_protection_branch(false) && flags.cf_protection_branch(true));
    }

    #[test]
    fn hardening_errors() {
        let bad = |a: &str, v: &str| TargetError::InvalidValue(a.into(), v.into());
        let (f, r) =
            parse(&["-fcf-protection=bogus", "-mfunction-return=x", "-fzero-call-used-regs=y"]);
        assert!(r.iter().all(|r| *r == Some(Ok(()))), "{r:?}");
        // clang's order, not the command line's; x86 repeats the
        // `-mfunction-return=` one.
        let t = "x86_64-unknown-linux-gnu";
        assert_eq!(
            f.invocation_errors(true, t),
            [
                bad("-mfunction-return=x", "x"),
                bad("-fzero-call-used-regs=y", "y"),
                bad("-fcf-protection=bogus", "bogus"),
                bad("-mfunction-return=x", "x"),
            ]
        );
        // A later valid value replaces an invalid one.
        assert!(
            parse(&["-fcf-protection=bogus", "-fcf-protection=none"])
                .0
                .invocation_errors(true, t)
                .is_empty()
        );
        let inv = |v: &str| {
            ArgError::InvalidArgumentToOption(v.into(), "fpatchable-function-entry=".into())
        };
        assert_eq!(parse(&["-fpatchable-function-entry=x"]).1, [Some(Err(inv("x")))]);
        assert_eq!(parse(&["-fpatchable-function-entry=1,"]).1, [Some(Err(inv("1,")))]);
        assert_eq!(
            parse(&["-fpatchable-function-entry=1,2"]).1,
            [Some(Err(ArgError::PatchableEntryOffset))]
        );
        assert_eq!(
            parse(&["-fpatchable-function-entry=2,2"]).0.patchable_function_entry,
            Some((2, 2))
        );
        // XRay's options: unused without `-fxray-instrument`, checked with it.
        let (f, _) = parse(&["-fxray-instruction-threshold=abc", "-fxray-modes=none"]);
        assert_eq!(f.unused_xray_args(), ["-fxray-instruction-threshold=abc", "-fxray-modes=none"]);
        assert!(f.xray_errors().is_empty());
        let (f, _) =
            parse(&["-fxray-instruction-threshold=abc", "-fxray-modes=none", "-fxray-instrument"]);
        assert!(f.unused_xray_args().is_empty());
        assert_eq!(f.xray_errors().len(), 2);
        assert_eq!(parse(&["-finstrument-functions-exclude-file-list=a"]).1, [None]);
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
