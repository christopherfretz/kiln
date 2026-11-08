//! The GCC/clang code generation flags build systems pass by default
//! (spec/driver.md "Codegen flags"). Each is in one of three classes:
//!
//! 1. honoured: it reaches the IR or the backend command line, as clang
//!    19 (`-fstack-protector*`, `-f[no-]unwind-tables`,
//!    `-f[no-]asynchronous-unwind-tables`, `-f[no-]plt`, `-f[no-]wrapv`,
//!    `-f[no-]wrapv-pointer`, `-f[no-]strict-overflow`,
//!    `-f[no-]c++-static-destructors[=…]`,
//!    `-f[no-]delete-null-pointer-checks`, `-f[no-]function-sections`,
//!    `-f[no-]data-sections`, `-mtune=`, `-mcmodel=`);
//! 2. accepted and ignored: no observable effect in kiln
//!    (`-f[no-]stack-clash-protection`, `-fcf-protection[=…]`,
//!    `-f[no-]common`, `-fstack-usage`, the defaults `-fno-trapv`,
//!    `-fno-short-enums` and `-m64`, and `-pipe`);
//! 3. rejected with "'<flag>' is not supported by kiln" (`-ftrapv`,
//!    `-fshort-enums`, `-fsanitize*`, `-fno-sanitize*`, coverage and
//!    profiling). The floating-point flags are `crate::fp_flags`'s.
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
}

/// Flags accepted and ignored (class 2), exactly as written.
const IGNORED: &[&str] = &[
    "-fstack-clash-protection",
    "-fno-stack-clash-protection",
    "-fcf-protection",
    "-fcommon",
    "-fno-common",
    "-fstack-usage",
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
            _ if IGNORED.contains(&arg) => {}
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
        match flag {
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
            "-fcf-protection" => {
                if !matches!(v, "full" | "branch" | "return" | "none") {
                    return Some(Err(ArgError::InvalidFlagValue(arg.to_owned(), v.to_owned())));
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
