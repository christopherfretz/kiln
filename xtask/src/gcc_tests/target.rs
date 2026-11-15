//! The target a GCC testsuite run describes to the tests' metadata: the
//! triplet `istarget` and `{ target ... }` selectors match, and the answer
//! to every `check_effective_target_*` / `dg-require-effective-target`
//! name (spec/harness.md "GCC testsuites").
//!
//! The answers are those of a hosted LP64 GNU/Linux target (`aarch64` or
//! `x86_64`, glibc, PIE by default) for all three compilers alike: the point
//! is to run the same tests through kiln, GCC and clang, not to probe each
//! compiler. A name the table does not know is an error the caller turns
//! into a skip (`unknown effective target X`), so a new name shows up in
//! the skip list instead of silently running or not.

use super::tcl::{self, Word};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    /// `aarch64` or `x86_64` (the host's `std::env::consts::ARCH`).
    pub arch: String,
    /// The GNU triplet the selectors match: `aarch64-unknown-linux-gnu`.
    pub triplet: String,
    /// `run_expensive_tests` (`GCC_TEST_RUN_EXPENSIVE` set).
    pub expensive: bool,
    /// `__OPTIMIZE__`: the option set optimizes (see [`Target::with_option`]).
    pub optimize: bool,
    /// The C++ standard a C++ suite compiles the case at (98, 11, 14, 17,
    /// 20, 23 or 26; see [`cxx_std_of`]); `None` for a C suite. It answers
    /// the `c++NN`, `c++NN_only` and `c++NN_down` names, `c++` and `c`.
    pub cxx: Option<u32>,
}

impl Target {
    pub fn host() -> Self {
        Self::for_arch(std::env::consts::ARCH, std::env::var_os("GCC_TEST_RUN_EXPENSIVE").is_some())
    }

    pub fn for_arch(arch: &str, expensive: bool) -> Self {
        let triplet = match arch {
            "x86_64" => "x86_64-pc-linux-gnu".to_owned(),
            a => format!("{a}-unknown-linux-gnu"),
        };
        Self { arch: arch.to_owned(), triplet, expensive, optimize: false, cxx: None }
    }

    /// This target under one option set: `__OPTIMIZE__` holds when the
    /// last `-O` in it is not `-O0`.
    #[must_use]
    pub fn with_option(&self, option: &[String]) -> Self {
        let last = option.iter().rev().find(|f| f.starts_with("-O"));
        Self { optimize: last.is_some_and(|o| o != "-O0"), ..self.clone() }
    }

    /// This target compiling C++ at `std` (a [`cxx_std_of`] number).
    #[must_use]
    pub fn with_cxx(&self, std: u32) -> Self {
        Self { cxx: Some(std), ..self.clone() }
    }

    /// The C++ names of `target-supports.exp` (GCC 16): `c++` and `c`;
    /// `c++NN` is NN or later, `c++NN_only` exactly NN, `c++NN_down` NN or
    /// earlier (`c++98` is any C++). `None` for a name that is not one.
    fn cxx_effective(&self, name: &str) -> Option<Result<bool, String>> {
        let Some(std) = self.cxx else {
            return name.starts_with("c++").then_some(Ok(false));
        };
        if name == "c" {
            return Some(Ok(false));
        }
        if name == "c++" {
            return Some(Ok(true));
        }
        let rest = name.strip_prefix("c++")?;
        let (num, kind) = match rest.split_once('_') {
            Some((n, k)) => (n, k),
            None => (rest, ""),
        };
        let Some(want) = cxx_std_number(num) else {
            return Some(Err(format!("unknown effective target {name}")));
        };
        let rank = |s: u32| if s == 98 { 0 } else { s };
        Some(match kind {
            "" => Ok(rank(std) >= rank(want)),
            "only" => Ok(std == want),
            "down" => Ok(rank(std) <= rank(want)),
            _ => Err(format!("unknown effective target {name}")),
        })
    }

    /// `istarget <pattern>`.
    pub fn is(&self, pattern: &str) -> bool {
        tcl::glob(pattern, &self.triplet)
    }

    /// `check_effective_target_<name>`; `Err` for a name the table lacks.
    #[allow(clippy::too_many_lines, reason = "one lookup table")]
    pub fn effective(&self, name: &str) -> Result<bool, String> {
        let x86 = self.arch == "x86_64";
        let arm64 = self.arch == "aarch64";
        if let Some(v) = self.cxx_effective(name) {
            return v;
        }
        Ok(match name {
            // Hosted LP64 GNU/Linux with glibc.
            "int32plus"
            | "int32"
            | "ptr32plus"
            | "size32plus"
            | "size20plus"
            | "lp64"
            | "longlong64"
            | "double64"
            | "double64plus"
            | "int128"
            | "stdint_types"
            | "inttypes_types"
            | "le"
            | "wchar"
            | "large_double"
            | "large_long_double"
            | "c99_runtime"
            | "fileio"
            | "signal"
            | "mmap"
            | "pthread"
            | "alloca"
            | "hard_float"
            | "trampolines"
            | "nonlocal_goto"
            | "label_values"
            | "indirect_calls"
            | "indirect_jumps"
            | "return_address"
            | "untyped_assembly"
            | "non_strict_prototype"
            | "named_sections"
            | "R_flag_in_section"
            | "exceptions"
            | "unwrapped"
            | "fpic"
            | "pie"
            | "scheduling"
            | "weak_undefined"
            | "alias"
            | "ifunc"
            | "tls"
            | "tls_runtime"
            | "sync_int_long"
            | "sync_char_short"
            | "sync_long_long"
            | "sync_long_long_runtime"
            | "lto"
            | "linker_plugin"
            | "asm_goto_with_outputs"
            | "builtin_eh_return"
            | "global_constructor"
            | "init_priority"
            | "setjmp"
            | "sqrt_insn"
            | "vect_int"
            | "cxa_atexit"
            | "gas"
            | "gld"
            | "stack_size"
            | "trapping"
            | "posix_memalign"
            | "clock"
            | "lra"
            | "c"
            | "c99"
            | "struct_return"
            // The C++ suites (g++.dg, c-c++-common, g++.old-deja).
            | "exceptions_enabled"
            | "rtti"
            | "shared"
            | "size24plus"
            | "variadic_mi_thunk"
            | "fstack_protector"
            | "ident_directive"
            | "tls_link"
            | "non_bionic"
            | "inf"
            | "float16"
            | "hostedlib"
            | "musttail"
            | "struct_musttail"
            | "external_musttail"
            | "pie_enabled"
            | "4byte_wchar_t"
            | "comdat_group"
            | "speculation_barrier_defined"
            | "ucn" => true,
            // Not this target, or nothing every compiler here provides.
            "ia32"
            | "ilp32"
            | "llp64"
            | "int16"
            | "be"
            | "freestanding"
            | "newlib"
            | "newlib_nano_io"
            | "vxworks_kernel"
            | "nonpic"
            | "fopenmp"
            | "fgraphite"
            | "dfp"
            | "dfprt"
            | "coldfire_fpu"
            | "ti_c67x"
            | "nvptx_softstack"
            | "keeps_null_pointer_checks"
            | "simulator"
            | "avr_tiny"
            | "msp430_small"
            | "pcc_bitfield_type_matters"
            | "short_enums"
            | "ptr_eq_short"
            | "logical_op_short_circuit"
            | "no_trampolines"
            | "noinit"
            | "persistent"
            | "section_anchors"
            | "label_values_unsupported"
            // GCC-only machinery, or the vectorizer properties its
            // `dg-final` scans check (counted as skips, not run).
            | "strub"
            | "lto_incremental"
            | "freorder"
            | "store_merge"
            | "fopenacc"
            | "vect_simd_clones"
            | "vect_early_break"
            | "vect_float"
            | "vect_double"
            | "vect_int_mult"
            | "arm_neon_ok"
            | "naked_functions"
            | "ppc_float128_sw"
            | "powerpc_altivec"
            | "powerpc_vsx"
            | "default_packed"
            | "short_eq_int"
            | "arm_eabi"
            | "vma_equals_lma"
            | "no_fsanitize_address"
            | "vect_slp_v2qi_store_unalign"
            // `-fimplicit-constexpr`, a GCC-only flag no case passes.
            | "implicit_constexpr" => false,
            "run_expensive_tests" => self.expensive,
            "__OPTIMIZE__" => self.optimize,
            "x86" | "sse2" | "sse2_runtime" | "sse" | "sse_runtime" | "lp64_x86" | "int128_x86" => {
                x86
            }
            "aarch64_little_endian" | "aarch64_asm_sve_ok" => arm64,
            "has_q_floating_suffix" | "has_w_floating_suffix" => x86,
            // C++20 concepts (or the Concepts TS flag no case passes).
            "concepts" => self.cxx.is_some_and(|s| s != 98 && s >= 20),
            n => return Err(format!("unknown effective target {n}")),
        })
    }

    /// A selector list, as in `{ target <list> }`: its elements are
    /// alternatives (any one matching matches) unless it holds `!`, `&&`
    /// or `||`, when it is an expression whose operands are elements; a
    /// braced element is a nested list. An element with a `-` or `*` is a
    /// triplet glob, otherwise an effective-target name.
    pub fn selector(&self, list: &str) -> Result<bool, String> {
        let words = tcl::list(list);
        if words.iter().any(|w| matches!(w, Word::Bare(b) if b == "!" || b == "&&" || b == "||")) {
            let mut p = Expr { t: self, w: &words, i: 0 };
            let v = p.or()?;
            if p.i != words.len() {
                return Err(format!("bad selector expression `{}`", list.trim()));
            }
            return Ok(v);
        }
        for w in &words {
            if self.operand(w)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn operand(&self, w: &Word) -> Result<bool, String> {
        match w {
            Word::Braced(inner) => self.selector(inner),
            w => {
                let s = w.text();
                if s.contains('-') || s.contains('*') { Ok(self.is(s)) } else { self.effective(s) }
            }
        }
    }

    /// A `{ target <sel> }` / `{ xfail <sel> }` directive argument: the
    /// keyword and whether the selector matches. `None` for anything else.
    pub fn keyword_selector(&self, arg: &str) -> Option<(String, Result<bool, String>)> {
        let w = tcl::list(arg);
        let kw = w.first()?.text().to_owned();
        if !matches!(kw.as_str(), "target" | "xfail") {
            return None;
        }
        let rest: Vec<String> = w[1..]
            .iter()
            .map(|x| match x {
                Word::Braced(b) => format!("{{{b}}}"),
                Word::Quoted(q) => q.clone(),
                Word::Bare(b) => b.clone(),
            })
            .collect();
        Some((kw, self.selector(&rest.join(" "))))
    }
}

/// The standard number of a `-std=` value's year (`17`, `1z`, `2c`,
/// `0x`, `03`): 98, 11, 14, 17, 20, 23 or 26.
pub fn cxx_std_number(year: &str) -> Option<u32> {
    Some(match year {
        "98" | "03" => 98,
        "11" | "0x" => 11,
        "14" | "1y" => 14,
        "17" | "1z" => 17,
        "20" | "2a" => 20,
        "23" | "2b" => 23,
        "26" | "2c" => 26,
        _ => return None,
    })
}

/// The C++ standard the last `-std=c++NN` / `-std=gnu++NN` in `flags`
/// selects, as a [`cxx_std_number`].
pub fn cxx_std_of(flags: &[String]) -> Option<u32> {
    flags.iter().rev().find_map(|f| {
        let v = f.strip_prefix("-std=")?;
        let year = v.strip_prefix("c++").or_else(|| v.strip_prefix("gnu++"))?;
        cxx_std_number(year)
    })
}

struct Expr<'a> {
    t: &'a Target,
    w: &'a [Word],
    i: usize,
}

impl Expr<'_> {
    fn op(&self, s: &str) -> bool {
        matches!(self.w.get(self.i), Some(Word::Bare(b)) if b == s)
    }

    fn or(&mut self) -> Result<bool, String> {
        let mut v = self.and()?;
        while self.op("||") {
            self.i += 1;
            let r = self.and()?;
            v = v || r;
        }
        Ok(v)
    }

    fn and(&mut self) -> Result<bool, String> {
        let mut v = self.not()?;
        while self.op("&&") {
            self.i += 1;
            let r = self.not()?;
            v = v && r;
        }
        Ok(v)
    }

    fn not(&mut self) -> Result<bool, String> {
        if self.op("!") {
            self.i += 1;
            return Ok(!self.not()?);
        }
        let w = self.w.get(self.i).ok_or("selector expression ends early")?;
        self.i += 1;
        self.t.operand(w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectors() {
        let t = Target::for_arch("aarch64", false);
        assert!(t.selector("*-*-*").unwrap());
        assert!(!t.selector(" avr-*-* pdp11-*-* ").unwrap());
        assert!(t.selector("avr-*-* aarch64*-*-*").unwrap());
        assert!(t.selector("! ia32").unwrap());
        assert!(t.selector("{ ! int16 } && lp64").unwrap());
        assert!(!t.selector("{ ! int16 } && { ia32 || llp64 }").unwrap());
        assert!(t.selector("int128").unwrap());
        assert!(!t.selector("x86").unwrap());
        assert!(Target::for_arch("x86_64", false).selector("x86_64-*-* && lp64").unwrap());
        assert_eq!(
            t.selector("bogus_target").unwrap_err(),
            "unknown effective target bogus_target"
        );
        assert!(!t.selector("run_expensive_tests").unwrap());
        assert!(Target::for_arch("aarch64", true).effective("run_expensive_tests").unwrap());
        let o2 = t.with_option(&["-O2".to_owned()]);
        assert!(o2.selector("__OPTIMIZE__").unwrap());
        assert!(!o2.with_option(&["-O0".to_owned()]).selector("__OPTIMIZE__").unwrap());
    }

    #[test]
    fn cxx_standards() {
        let t = Target::for_arch("aarch64", false);
        let c17 = t.with_cxx(17);
        assert!(c17.selector("c++11").unwrap() && c17.selector("c++17").unwrap());
        assert!(!c17.selector("c++20").unwrap() && c17.selector("c++17_only").unwrap());
        assert!(c17.selector("c++17_down").unwrap() && !c17.selector("c++14_down").unwrap());
        assert!(c17.selector("c++98").unwrap() && !c17.selector("c++98_only").unwrap());
        assert!(c17.selector("c++").unwrap() && !c17.selector("c").unwrap());
        assert!(t.with_cxx(98).selector("c++98_only").unwrap());
        assert!(t.with_cxx(98).selector("! c++11").unwrap());
        assert!(t.with_cxx(26).selector("c++26").unwrap());
        assert!(c17.selector("c++17_bogus").is_err());
        assert!(t.selector("c").unwrap() && !t.selector("c++").unwrap());
        let f = |s: &[&str]| s.iter().map(|&x| x.to_owned()).collect::<Vec<_>>();
        assert_eq!(cxx_std_of(&f(&["-std=c++17", "-O2", "-std=gnu++2c"])), Some(26));
        assert_eq!(cxx_std_of(&f(&["-std=c++0x"])), Some(11));
        assert_eq!(cxx_std_of(&f(&["-std=gnu17"])), None);
    }

    #[test]
    fn keyword_selectors() {
        let t = Target::for_arch("x86_64", false);
        assert_eq!(t.keyword_selector(" target { ! ia32 } "), Some(("target".into(), Ok(true))));
        assert_eq!(t.keyword_selector("xfail avr-*-*"), Some(("xfail".into(), Ok(false))));
        assert_eq!(t.keyword_selector("-O2"), None);
    }
}
