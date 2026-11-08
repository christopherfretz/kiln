//! The predefines buffer (clang's `<built-in>`): the target/standard
//! macro set from `resource/predefines/<triple>.<std>.txt`, adjusted for
//! the dialect and the flags that change predefined macros, kiln's own
//! identity macros, then the `-D` / `-U` options in command-line order as
//! clang's `<command line>` section. See `spec/pp.md` "Predefines" and
//! `resource/README.md` for the measured flag effects.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::options::ClangCompat;

/// Position-independent code model (clang `ParsePICArgs`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PicModel {
    /// `-fno-pic` / `-fno-pie`: no `__PIC__` / `__PIE__`.
    None,
    /// `-fpic` (1) / `-fPIC` (2): `__PIC__ n`, no `__PIE__`.
    Pic(u8),
    /// `-fpie` (1) / `-fPIE` (2): `__PIC__ n` and `__PIE__ n` (Debian's
    /// default is level 2).
    Pie(u8),
}

/// Optimization level as far as macros care.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum OptLevel {
    /// `-O0` (default): `__NO_INLINE__`.
    #[default]
    None,
    /// `-O1`..`-O3`, `-Ofast`, `-Og`: `__OPTIMIZE__`.
    Speed,
    /// `-Os` / `-Oz`: `__OPTIMIZE__` and `__OPTIMIZE_SIZE__`.
    Size,
}

/// The flag-dependent part of the predefines.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug)]
pub struct PredefConfig {
    /// GNU dialect (`-std=gnu++NN`, the default): no `__STRICT_ANSI__`;
    /// `linux`, `unix` (and the `__GLIBCXX_*_INT_N_0` pair in C++).
    pub gnu: bool,
    /// C++ (else C).
    pub cplusplus: bool,
    /// The target is `AArch64` (`-ffast-math` adds `__ARM_FP_FAST`).
    pub aarch64: bool,
    /// Optimization.
    pub opt: OptLevel,
    /// `-fno-inline`: `__NO_INLINE__` even when optimizing.
    pub no_inline: bool,
    /// `-include <file>`s: `#include "<file>"` lines after the command
    /// line's macros, in the `<command line>` section (clang
    /// `AddImplicitInclude`), so they are found from the working directory.
    pub implicit_includes: Vec<String>,
    /// `-fexceptions` (C++ default on).
    pub exceptions: bool,
    /// `-frtti` (C++ default on).
    pub rtti: bool,
    /// `-fchar8_t` / `-fno-char8_t` if given (default: on from C++20).
    pub char8_t: Option<bool>,
    /// `-fsigned-char` / `-funsigned-char` if given: adds or removes
    /// `__CHAR_UNSIGNED__` (default: the target's, as in the base file).
    pub char_signed: Option<bool>,
    /// PIC / PIE.
    pub pic: PicModel,
    /// clang's cc1 `-ffast-math` (every fast-math flag but contraction):
    /// `__FAST_MATH__`.
    pub fast_math: bool,
    /// `-ffinite-math-only` (no infinities and no NaNs):
    /// `__FINITE_MATH_ONLY__` is 1.
    pub finite_math: bool,
    /// `-fno-math-errno`: `__NO_MATH_ERRNO__`.
    pub no_math_errno: bool,
    /// clang's cc1 `-funsafe-math-optimizations`: `__ARM_FP_FAST`.
    pub unsafe_math: bool,
    /// `-ffreestanding`.
    pub freestanding: bool,
    /// `-pthread`.
    pub pthread: bool,
    /// `-fthreadsafe-statics` (default on).
    pub threadsafe_statics: bool,
    /// `-fsized-deallocation` (default on).
    pub sized_deallocation: bool,
    /// kiln's version (`__KILN_MAJOR__`, `__KILN_MINOR__`).
    pub kiln_version: (u32, u32),
    /// Whose predefined macros (`-fclang-compat=`): the base files are
    /// clang 19's; [`ClangCompat::Clang22`] applies `CLANG22_COMMON`
    /// (and `CLANG22_AARCH64` on `AArch64`).
    pub clang_compat: ClangCompat,
    /// The C++ library declares `std::float16_t` ... `std::bfloat16_t`
    /// (`<stdfloat>`): libstdc++ does, libc++ 19 and 22 do not. When false
    /// the `__STDCPP_*_T__` macros stay undefined ([cpp.predefined]/2:
    /// they mean the implementation supports those aliases; libc++'s
    /// `std` module exports them when the macros are defined).
    pub std_float_aliases: bool,
    /// `-freflection`: `__cpp_impl_reflection 202603L` at C++26 (GCC
    /// 16.1's value; spec/reflection.md "Feature macros").
    pub reflection: bool,
    /// The `AArch64` target features the `__ARM_*` macros derive from.
    pub arm: ArmTarget,
    /// Off `AArch64`, how the `-march=<cpu>` macros differ from the base
    /// file's: a macro (re)defined with its value, or removed with `None`
    /// (the driver's `features::x86_cpu_macros`; WP-630).
    pub x86_macros: Vec<(String, Option<String>)>,
}

/// The `AArch64` target state of `-march=` / `-mcpu=` /
/// `-msve-vector-bits=` / `-target-feature` (spec/driver.md "Target
/// features") that the `__ARM_ARCH`, `__ARM_FP`, `__ARM_NEON*` and
/// `__ARM_FEATURE_*` macros derive from (clang `AArch64TargetInfo`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArmTarget {
    /// The architecture version `(major, minor)`: `(8, 0)` for `armv8-a`.
    pub arch: (u8, u8),
    /// The enabled LLVM feature names (`fp-armv8`, `neon`, `sve`, ...).
    pub features: Vec<String>,
    /// `-msve-vector-bits=N`: `__ARM_FEATURE_SVE_BITS N`.
    pub sve_vector_bits: Option<u32>,
    /// The `__ARM_FEATURE_{PAC,BTI,GCS}_DEFAULT` macros of
    /// `-mbranch-protection=` (name, value; WP-846).
    pub branch_protection: Vec<(String, String)>,
}

impl Default for ArmTarget {
    /// `armv8-a` with `fp-armv8` and `neon` (the base files' macros).
    fn default() -> ArmTarget {
        ArmTarget {
            arch: (8, 0),
            features: vec!["fp-armv8".to_owned(), "neon".to_owned()],
            sve_vector_bits: None,
            branch_protection: Vec::new(),
        }
    }
}

impl PredefConfig {
    /// clang's defaults for C++ (`cplusplus`) or C on Linux, GNU dialect.
    pub fn new(cplusplus: bool) -> PredefConfig {
        PredefConfig {
            gnu: true,
            cplusplus,
            aarch64: true,
            opt: OptLevel::None,
            no_inline: false,
            implicit_includes: Vec::new(),
            exceptions: cplusplus,
            rtti: cplusplus,
            char8_t: None,
            char_signed: None,
            pic: PicModel::Pie(2),
            fast_math: false,
            finite_math: false,
            no_math_errno: false,
            unsafe_math: false,
            freestanding: false,
            pthread: false,
            threadsafe_statics: true,
            sized_deallocation: true,
            kiln_version: (0, 0),
            clang_compat: ClangCompat::Clang22,
            std_float_aliases: true,
            reflection: false,
            arm: ArmTarget::default(),
            x86_macros: Vec::new(),
        }
    }
}

/// A `-D` or `-U` option.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CmdMacro {
    /// `-D name`, `-D name=value`, `-D 'f(x)=value'`.
    Define(String),
    /// `-U name`.
    Undef(String),
}

/// clang 22.1.8's predefined macros that differ from clang 19.1.7's on
/// both targets, other than the `__cpp_*` feature-test macros (spec/pp.md
/// "clang compatibility level"), each defined or replaced: clang 22
/// removes none. The difference is the same for every
/// standard, C and C++, strict and GNU, and every flag `PredefConfig`
/// models (measured with `clang-N --target=<triple> -std=<std> -dM -E`).
/// Function-like macros carry their parameter list in the name.
const CLANG22_COMMON: &[(&str, &str)] = &[
    ("__BOOL_WIDTH__", "1"),
    ("__INT16_C(c)", "c"),
    ("__INT32_C(c)", "c"),
    ("__INT64_C(c)", "c##L"),
    ("__INT8_C(c)", "c"),
    ("__INTMAX_C(c)", "c##L"),
    ("__MEMORY_SCOPE_CLUSTR", "5"),
    ("__UINT16_C(c)", "c"),
    ("__UINT32_C(c)", "c##U"),
    ("__UINT64_C(c)", "c##UL"),
    ("__UINT8_C(c)", "c"),
    ("__UINTMAX_C(c)", "c##UL"),
    ("__VERSION__", "\"Debian Clang 22.1.8 (1~deb13u4)\""),
    ("__clang_major__", "22"),
    ("__clang_patchlevel__", "8"),
    ("__clang_version__", "\"22.1.8 (1~deb13u4)\""),
];

/// The `AArch64`-only part of the clang 22 difference ([`CLANG22_COMMON`]).
const CLANG22_AARCH64: &[(&str, &str)] = &[
    ("__ARM_ACLE", "202420"),
    ("__ARM_ACLE_VERSION(year,quarter,patch)", "(100 * (year) + 10 * (quarter) + (patch))"),
    ("__ARM_NEON_SVE_BRIDGE", "1"),
    ("__ARM_PREFETCH_RANGE", "1"),
    ("__FUNCTION_MULTI_VERSIONING_SUPPORT_LEVEL", "202430"),
    ("__GCC_DESTRUCTIVE_SIZE", "256"),
];

/// WP-361: GCC 14's P1467 limit macros for `_Float32`, `_Float64`,
/// `_Float128` and `__bf16` (`g++-14 -std=c++23 -dM -E`, identical on
/// both targets), which libstdc++'s `<limits>`, `<cmath>`, `<format>`
/// and `<bits/c++config.h>` consume. Defined in every C++ mode, as GCC
/// does; the `__FLT16_*__` set stays clang's (in the base files, plus
/// GCC's `__FLT16_IS_IEC_60559__`, which clang lacks). GCC's
/// `__FLT32X_*__` (the `__FLT64_*__` values) and `__FLT64X_*__` (`long
/// double`'s) come from [`apply_p1467`] (WP-380).
const P1467_LIMITS: &[(&str, &str)] = &[
    ("__BFLT16_DECIMAL_DIG__", "4"),
    ("__BFLT16_DENORM_MIN__", "9.18354961579912115600575419704879436e-41BF16"),
    ("__BFLT16_DIG__", "2"),
    ("__BFLT16_EPSILON__", "7.81250000000000000000000000000000000e-3BF16"),
    ("__BFLT16_HAS_DENORM__", "1"),
    ("__BFLT16_HAS_INFINITY__", "1"),
    ("__BFLT16_HAS_QUIET_NAN__", "1"),
    ("__BFLT16_IS_IEC_60559__", "0"),
    ("__BFLT16_MANT_DIG__", "8"),
    ("__BFLT16_MAX_10_EXP__", "38"),
    ("__BFLT16_MAX_EXP__", "128"),
    ("__BFLT16_MAX__", "3.38953138925153547590470800371487867e+38BF16"),
    ("__BFLT16_MIN_10_EXP__", "(-37)"),
    ("__BFLT16_MIN_EXP__", "(-125)"),
    ("__BFLT16_MIN__", "1.17549435082228750796873653722224568e-38BF16"),
    ("__BFLT16_NORM_MAX__", "3.38953138925153547590470800371487867e+38BF16"),
    ("__FLT128_DECIMAL_DIG__", "36"),
    ("__FLT128_DENORM_MIN__", "6.47517511943802511092443895822764655e-4966F128"),
    ("__FLT128_DIG__", "33"),
    ("__FLT128_EPSILON__", "1.92592994438723585305597794258492732e-34F128"),
    ("__FLT128_HAS_DENORM__", "1"),
    ("__FLT128_HAS_INFINITY__", "1"),
    ("__FLT128_HAS_QUIET_NAN__", "1"),
    ("__FLT128_IS_IEC_60559__", "1"),
    ("__FLT128_MANT_DIG__", "113"),
    ("__FLT128_MAX_10_EXP__", "4932"),
    ("__FLT128_MAX_EXP__", "16384"),
    ("__FLT128_MAX__", "1.18973149535723176508575932662800702e+4932F128"),
    ("__FLT128_MIN_10_EXP__", "(-4931)"),
    ("__FLT128_MIN_EXP__", "(-16381)"),
    ("__FLT128_MIN__", "3.36210314311209350626267781732175260e-4932F128"),
    ("__FLT128_NORM_MAX__", "1.18973149535723176508575932662800702e+4932F128"),
    ("__FLT16_IS_IEC_60559__", "1"),
    ("__FLT32_DECIMAL_DIG__", "9"),
    ("__FLT32_DENORM_MIN__", "1.40129846432481707092372958328991613e-45F32"),
    ("__FLT32_DIG__", "6"),
    ("__FLT32_EPSILON__", "1.19209289550781250000000000000000000e-7F32"),
    ("__FLT32_HAS_DENORM__", "1"),
    ("__FLT32_HAS_INFINITY__", "1"),
    ("__FLT32_HAS_QUIET_NAN__", "1"),
    ("__FLT32_IS_IEC_60559__", "1"),
    ("__FLT32_MANT_DIG__", "24"),
    ("__FLT32_MAX_10_EXP__", "38"),
    ("__FLT32_MAX_EXP__", "128"),
    ("__FLT32_MAX__", "3.40282346638528859811704183484516925e+38F32"),
    ("__FLT32_MIN_10_EXP__", "(-37)"),
    ("__FLT32_MIN_EXP__", "(-125)"),
    ("__FLT32_MIN__", "1.17549435082228750796873653722224568e-38F32"),
    ("__FLT32_NORM_MAX__", "3.40282346638528859811704183484516925e+38F32"),
    ("__FLT64_DECIMAL_DIG__", "17"),
    ("__FLT64_DENORM_MIN__", "4.94065645841246544176568792868221372e-324F64"),
    ("__FLT64_DIG__", "15"),
    ("__FLT64_EPSILON__", "2.22044604925031308084726333618164062e-16F64"),
    ("__FLT64_HAS_DENORM__", "1"),
    ("__FLT64_HAS_INFINITY__", "1"),
    ("__FLT64_HAS_QUIET_NAN__", "1"),
    ("__FLT64_IS_IEC_60559__", "1"),
    ("__FLT64_MANT_DIG__", "53"),
    ("__FLT64_MAX_10_EXP__", "308"),
    ("__FLT64_MAX_EXP__", "1024"),
    ("__FLT64_MAX__", "1.79769313486231570814527423731704357e+308F64"),
    ("__FLT64_MIN_10_EXP__", "(-307)"),
    ("__FLT64_MIN_EXP__", "(-1021)"),
    ("__FLT64_MIN__", "2.22507385850720138309023271733240406e-308F64"),
    ("__FLT64_NORM_MAX__", "1.79769313486231570814527423731704357e+308F64"),
];

/// WP-380: GCC 14's `__FLT64X_*__` on x86-64, where `_Float64x` is the
/// x87 80-bit format (`x86_64-linux-gnu-g++-14 -dM -E`); on `AArch64` they
/// are the binary128 `__FLT128_*__` values.
const FLT64X_X87: &[(&str, &str)] = &[
    ("__FLT64X_DECIMAL_DIG__", "21"),
    ("__FLT64X_DENORM_MIN__", "3.64519953188247460252840593361941982e-4951F64x"),
    ("__FLT64X_DIG__", "18"),
    ("__FLT64X_EPSILON__", "1.08420217248550443400745280086994171e-19F64x"),
    ("__FLT64X_HAS_DENORM__", "1"),
    ("__FLT64X_HAS_INFINITY__", "1"),
    ("__FLT64X_HAS_QUIET_NAN__", "1"),
    ("__FLT64X_IS_IEC_60559__", "1"),
    ("__FLT64X_MANT_DIG__", "64"),
    ("__FLT64X_MAX_10_EXP__", "4932"),
    ("__FLT64X_MAX_EXP__", "16384"),
    ("__FLT64X_MAX__", "1.18973149535723176502126385303097021e+4932F64x"),
    ("__FLT64X_MIN_10_EXP__", "(-4931)"),
    ("__FLT64X_MIN_EXP__", "(-16381)"),
    ("__FLT64X_MIN__", "3.36210314311209350626267781732175260e-4932F64x"),
    ("__FLT64X_NORM_MAX__", "1.18973149535723176502126385303097021e+4932F64x"),
];

/// WP-361: the [cpp.predefined] macros of the extended floating-point
/// types kiln provides (C++23 on; GCC 14 defines them from C++23 too).
const P1467_STDCPP: &[&str] = &[
    "__STDCPP_BFLOAT16_T__",
    "__STDCPP_FLOAT128_T__",
    "__STDCPP_FLOAT16_T__",
    "__STDCPP_FLOAT32_T__",
    "__STDCPP_FLOAT64_T__",
];

/// Feature-test macros for what kiln implements beyond clang 19's
/// predefines (`docs/STANDARD_SUPPORT.md`, spec/pp.md "clang
/// compatibility level"): (minimum `__cplusplus`, name, value). Applied in
/// C++ at every compatibility level.
const KILN_FEATURES: &[(u64, &str, &str)] = &[
    // P2747 constexpr placement new (WP-357).
    (202_400, "__cpp_constexpr", "202406L"),
    // P3068 constexpr exceptions (WP-357); withdrawn with -fno-exceptions.
    (202_400, "__cpp_constexpr_exceptions", "202411L"),
    // C++20 modules (WP-363, spec/modules.md §4): clang 22's value, from
    // C++20 on (clang 19 does not define it).
    (202_002, "__cpp_modules", "1"),
    // P1306R5 expansion statements (WP-447): C++26 only, whatever the
    // -fclang-compat (clang 22 does not define it).
    (202_400, "__cpp_expansion_statements", "202506L"),
    // P2900R14 contracts (WP-706, spec/contracts.md §1): C++26 only,
    // whatever the -fclang-compat (clang 22 does not define it).
    (202_400, "__cpp_contracts", "202502L"),
    // P3074R7 trivial unions with P3726R2's value (WP-743): C++26 only,
    // whatever the -fclang-compat (neither clang 22 nor GCC 16.1 defines
    // it).
    (202_400, "__cpp_trivial_union", "202603L"),
];

/// The driver-injected `-D` that clang's driver appends after the user's
/// (it follows the unwind-table defaults on Linux).
const DRIVER_DEFINES: &[&str] = &["__GCC_HAVE_DWARF2_CFI_ASM"];

/// Builds the predefines buffer text from `base` (the contents of a
/// `resource/predefines` file: `#define NAME VALUE` lines for the strict
/// dialect with clang's default flags), `cfg` and the `-D` / `-U` options.
pub fn build_predefines(base: &str, cfg: &PredefConfig, cmdline: &[CmdMacro]) -> String {
    let mut m: BTreeMap<String, String> = BTreeMap::new();
    for line in base.lines() {
        let Some(rest) = line.strip_prefix("#define ") else { continue };
        let (name, value) = rest.split_once(' ').unwrap_or((rest, ""));
        m.insert(name.to_owned(), value.to_owned());
    }
    if cfg.clang_compat == ClangCompat::Clang22 {
        let arch: &[_] = if cfg.aarch64 { CLANG22_AARCH64 } else { &[] };
        for &(n, v) in CLANG22_COMMON.iter().chain(arch) {
            set(&mut m, n, v);
        }
    }
    let mut driver: Vec<(String, String)> = Vec::new();
    for d in DRIVER_DEFINES {
        if let Some(v) = m.remove(*d) {
            driver.push(((*d).to_owned(), v));
        }
    }
    apply_flags(&mut m, cfg);
    if cfg.aarch64 {
        apply_arm(&mut m, &cfg.arm, cfg.clang_compat);
    } else {
        for (n, v) in &cfg.x86_macros {
            match v {
                Some(v) => set(&mut m, n, v),
                None => {
                    m.remove(n);
                }
            }
        }
    }

    let mut out = String::with_capacity(base.len() + 256);
    // kiln's identity macros come before clang's first line, so the
    // line markers that name `<built-in>` lines match clang's.
    let _ = writeln!(out, "#define __KILN__ 1");
    let _ = writeln!(out, "#define __KILN_MAJOR__ {}", cfg.kiln_version.0);
    let _ = writeln!(out, "#define __KILN_MINOR__ {}", cfg.kiln_version.1);
    out.push_str("# 1 \"<built-in>\" 3\n");
    for (n, v) in &m {
        let _ = writeln!(out, "#define {n} {v}");
    }
    // clang 19 pushes these so that `-D` can redefine them without a
    // warning (`InitializePredefinedMacros`).
    for n in ["__GCC_DESTRUCTIVE_SIZE", "__GCC_CONSTRUCTIVE_SIZE"] {
        if m.contains_key(n) {
            let _ = writeln!(out, "#pragma push_macro(\"{n}\")");
        }
    }
    out.push_str("# 1 \"<command line>\" 1\n");
    for c in cmdline {
        match c {
            CmdMacro::Define(d) => {
                out.push_str("#define ");
                out.push_str(&define_line(d));
                out.push('\n');
            }
            CmdMacro::Undef(n) => {
                let _ = writeln!(out, "#undef {n}");
            }
        }
    }
    for (n, v) in &driver {
        let _ = writeln!(out, "#define {n} {v}");
    }
    for f in &cfg.implicit_includes {
        out.push_str("#include \"");
        for c in f.chars() {
            if matches!(c, '\\' | '"') {
                out.push('\\');
            }
            out.push(c);
        }
        out.push_str("\"\n");
    }
    out.push_str("# 1 \"<built-in>\" 2\n");
    out
}

fn set(m: &mut BTreeMap<String, String>, n: &str, v: &str) {
    m.insert(n.to_owned(), v.to_owned());
}

/// P1467 ([basic.extended.fp], [cpp.predefined]/2): GCC 14's limit
/// macros in every C++ mode, the `__STDCPP_*_T__` ones from C++23. GCC's
/// `_Float32x` limits are `_Float64`'s and its `_Float64x` limits `long
/// double`'s, spelled with the `F32x` / `F64x` suffix (WP-380).
fn apply_p1467(m: &mut BTreeMap<String, String>, std_float_aliases: bool, aarch64: bool) {
    for &(n, v) in P1467_LIMITS {
        set(m, n, v);
        let nx = if let Some(r) = n.strip_prefix("__FLT64_") {
            Some(("__FLT32X_", r, "F64", "F32x"))
        } else if aarch64 {
            n.strip_prefix("__FLT128_").map(|r| ("__FLT64X_", r, "F128", "F64x"))
        } else {
            None
        };
        if let Some((prefix, rest, from, to)) = nx {
            let v = v.strip_suffix(from).map_or_else(|| v.to_owned(), |b| format!("{b}{to}"));
            m.insert(format!("{prefix}{rest}"), v);
        }
    }
    if !aarch64 {
        for &(n, v) in FLT64X_X87 {
            set(m, n, v);
        }
    }
    let cplusplus: u64 =
        m.get("__cplusplus").map_or(0, |v| v.trim_end_matches('L').parse().unwrap_or(0));
    if cplusplus >= 202_302 && std_float_aliases {
        for n in P1467_STDCPP {
            set(m, n, "1");
        }
    }
}

/// Applies the dialect and flag deltas (measured against clang 19; see
/// `resource/README.md`).
/// The feature-test macros of features kiln implements beyond clang 19
/// (`docs/STANDARD_SUPPORT.md`), before the flags that remove them:
/// P1061 structured binding packs (C++17 on) and P2893 variadic friends
/// (every C++ mode) at clang 22's values in the same modes, and
/// `KILN_FEATURES` (P2747, P3068, modules) by `__cplusplus`.
fn kiln_features(m: &mut BTreeMap<String, String>, compat: ClangCompat) {
    let cplusplus =
        m.get("__cplusplus").and_then(|v| v.trim_end_matches('L').parse::<u64>().ok()).unwrap_or(0);
    for &(min, n, v) in KILN_FEATURES {
        if cplusplus >= min {
            set(m, n, v);
        }
    }
    // P2841R7 concept and variable template template parameters
    // (WP-746, WP-771): C++26 at `-fclang-compat=22`, where kiln accepts
    // them (at 19 it rejects the forms, as clang 19 does). clang 22
    // implements the paper without defining the macro.
    if compat == ClangCompat::Clang22 && cplusplus >= 202_400 {
        set(m, "__cpp_template_parameters", "202502L");
    }
    if m.contains_key("__cpp_structured_bindings") {
        set(m, "__cpp_structured_bindings", "202411L");
    }
    set(m, "__cpp_variadic_friend", "202403L");
}

/// Feature macros keyed on the `__cplusplus` value already in `m`.
fn apply_std_keyed(m: &mut BTreeMap<String, String>, reflection: bool) {
    let cplusplus =
        m.get("__cplusplus").and_then(|v| v.trim_end_matches('L').parse::<u32>().ok()).unwrap_or(0);
    // [cpp.predefined] (P0847, C++23): kiln implements explicit object
    // parameters, members and lambdas; clang 19 leaves the macro
    // undefined, clang 22 defines it from C++23.
    if cplusplus >= 202_302 {
        set(m, "__cpp_explicit_this_parameter", "202110L");
    }
    // GCC 16.1 `-freflection` (C++26 only; `g++-16 -std=c++26
    // -freflection -dM -E`): libstdc++'s `<meta>` keys on it.
    if reflection && cplusplus > 202_302 {
        set(m, "__cpp_impl_reflection", "202603L");
    }
}

fn apply_flags(m: &mut BTreeMap<String, String>, cfg: &PredefConfig) {
    if cfg.gnu {
        m.remove("__STRICT_ANSI__");
        set(m, "linux", "1");
        set(m, "unix", "1");
        if cfg.cplusplus {
            set(m, "__GLIBCXX_TYPE_INT_N_0", "__int128");
            set(m, "__GLIBCXX_BITSIZE_INT_N_0", "128");
        }
    }
    match cfg.char_signed {
        Some(true) => {
            m.remove("__CHAR_UNSIGNED__");
        }
        Some(false) => set(m, "__CHAR_UNSIGNED__", "1"),
        None => {}
    }
    match cfg.opt {
        OptLevel::None => {}
        OptLevel::Speed | OptLevel::Size => {
            if !cfg.no_inline {
                m.remove("__NO_INLINE__");
            }
            set(m, "__OPTIMIZE__", "1");
            if cfg.opt == OptLevel::Size {
                set(m, "__OPTIMIZE_SIZE__", "1");
            }
        }
    }
    if cfg.cplusplus {
        kiln_features(m, cfg.clang_compat);
        if !cfg.exceptions {
            m.remove("__EXCEPTIONS");
            m.remove("__cpp_exceptions");
            m.remove("__cpp_constexpr_exceptions");
        }
        if !cfg.rtti {
            m.remove("__GXX_RTTI");
            m.remove("__cpp_rtti");
        }
        match cfg.char8_t {
            Some(false) => {
                for n in [
                    "__cpp_char8_t",
                    "__CLANG_ATOMIC_CHAR8_T_LOCK_FREE",
                    "__GCC_ATOMIC_CHAR8_T_LOCK_FREE",
                ] {
                    m.remove(n);
                }
            }
            Some(true) if !m.contains_key("__cpp_char8_t") => {
                set(m, "__cpp_char8_t", "202207L");
                set(m, "__CLANG_ATOMIC_CHAR8_T_LOCK_FREE", "2");
                set(m, "__GCC_ATOMIC_CHAR8_T_LOCK_FREE", "2");
            }
            _ => {}
        }
        if !cfg.threadsafe_statics {
            m.remove("__cpp_threadsafe_static_init");
        }
        if !cfg.sized_deallocation {
            m.remove("__cpp_sized_deallocation");
        }
        apply_std_keyed(m, cfg.reflection);
        apply_p1467(m, cfg.std_float_aliases, cfg.aarch64);
    }
    for n in ["__PIC__", "__pic__", "__PIE__", "__pie__"] {
        m.remove(n);
    }
    match cfg.pic {
        PicModel::None => {}
        PicModel::Pic(l) => {
            set(m, "__PIC__", &l.to_string());
            set(m, "__pic__", &l.to_string());
        }
        PicModel::Pie(l) => {
            for n in ["__PIC__", "__pic__", "__PIE__", "__pie__"] {
                set(m, n, &l.to_string());
            }
        }
    }
    // clang `InitializePredefinedMacros` / `AArch64TargetInfo`.
    if cfg.fast_math {
        set(m, "__FAST_MATH__", "1");
    }
    if cfg.finite_math {
        set(m, "__FINITE_MATH_ONLY__", "1");
    }
    if cfg.no_math_errno {
        set(m, "__NO_MATH_ERRNO__", "1");
    }
    if cfg.unsafe_math && cfg.aarch64 {
        set(m, "__ARM_FP_FAST", "1");
    }
    if cfg.freestanding {
        set(m, "__STDC_HOSTED__", "0");
    }
    if cfg.pthread {
        set(m, "_REENTRANT", "1");
    }
}

/// The `__ARM_*` macros of the `AArch64` target state that the base files
/// (`armv8-a`, `fp-armv8` + `neon`) leave out or define; each is removed,
/// then defined if its condition holds.
const ARM_TARGET_MACROS: &[&str] = &[
    "__ARM_ARCH",
    "__ARM_BF16_FORMAT_ALTERNATIVE",
    "__ARM_FEATURE_AES",
    "__ARM_FEATURE_ATOMICS",
    "__ARM_FEATURE_BF16",
    "__ARM_FEATURE_BF16_SCALAR_ARITHMETIC",
    "__ARM_FEATURE_BF16_VECTOR_ARITHMETIC",
    "__ARM_FEATURE_BTI",
    "__ARM_FEATURE_COMPLEX",
    "__ARM_FEATURE_CRC32",
    "__ARM_FEATURE_CRYPTO",
    "__ARM_FEATURE_CSSC",
    "__ARM_FEATURE_DOTPROD",
    "__ARM_FEATURE_FP16_FML",
    "__ARM_FEATURE_FP16_SCALAR_ARITHMETIC",
    "__ARM_FEATURE_FP16_VECTOR_ARITHMETIC",
    "__ARM_FEATURE_FPRCVT",
    "__ARM_FEATURE_FRINT",
    "__ARM_FEATURE_GCS",
    "__ARM_FEATURE_JCVT",
    "__ARM_FEATURE_LS64",
    "__ARM_FEATURE_MATMUL_INT8",
    "__ARM_FEATURE_MEMORY_TAGGING",
    "__ARM_FEATURE_MOPS",
    "__ARM_FEATURE_PAUTH",
    "__ARM_FEATURE_QRDMX",
    "__ARM_FEATURE_RCPC",
    "__ARM_FEATURE_RNG",
    "__ARM_FEATURE_SHA2",
    "__ARM_FEATURE_SHA3",
    "__ARM_FEATURE_SHA512",
    "__ARM_FEATURE_SM3",
    "__ARM_FEATURE_SM4",
    "__ARM_FEATURE_SVE",
    "__ARM_FEATURE_SVE2",
    "__ARM_FEATURE_SVE2_AES",
    "__ARM_FEATURE_SVE2_BITPERM",
    "__ARM_FEATURE_SVE2_SHA3",
    "__ARM_FEATURE_SVE2_SM4",
    "__ARM_FEATURE_SVE2p1",
    "__ARM_FEATURE_SVE2p2",
    "__ARM_FEATURE_SVE_AES2",
    "__ARM_FEATURE_SVE_B16B16",
    "__ARM_FEATURE_SVE_BF16",
    "__ARM_FEATURE_SVE_BITS",
    "__ARM_FEATURE_SVE_F16F32MM",
    "__ARM_FEATURE_SVE_MATMUL_FP32",
    "__ARM_FEATURE_SVE_MATMUL_FP64",
    "__ARM_FEATURE_SVE_MATMUL_INT8",
    "__ARM_FEATURE_SVE_VECTOR_OPERATORS",
    "__ARM_FEATURE_SYSREG128",
    "__ARM_FEATURE_TME",
    "__ARM_FP",
    "__ARM_NEON",
    "__ARM_NEON_FP",
];

/// clang `AArch64TargetInfo::getTargetDefines` over the target state `t`
/// (measured against clang 19 and 22 for every `-march=` level and
/// extension kiln accepts). The architecture-version defines
/// (`QRDMX` from v8.1, `COMPLEX`/`JCVT` from v8.3, `FRINT` from v8.5)
/// follow the level alone; clang 19 also turns on `crc`, `lse`, `rcpc`,
/// `dotprod`, `bti`, `bf16`, `i8mm` (and, on v9, `sve2` and `fullfp16`)
/// from the level whatever the extensions say (its `setArchFeatures`),
/// where clang 22 follows the features. `__ARM_NEON_SVE_BRIDGE` is
/// clang 22's unconditionally ([`CLANG22_AARCH64`]) and clang 19's with
/// SVE and NEON.
fn apply_arm(m: &mut BTreeMap<String, String>, t: &ArmTarget, compat: ClangCompat) {
    for n in ARM_TARGET_MACROS {
        m.remove(*n);
    }
    let has = |f: &str| t.features.iter().any(|x| x == f);
    let c19 = compat == ClangCompat::Clang19;
    let (major, minor) = t.arch;
    // The level includes armv8.n (armv9.k includes armv8.(k+5)).
    let v8 = |n: u8| if major == 9 { n <= minor + 5 } else { major == 8 && n <= minor };
    let v9 = major == 9;
    let fp = has("fp-armv8");
    let neon = fp && has("neon");
    let sve = neon && has("sve");
    // `+sve` (and clang 22's `+sve-bitperm` / `+sve-aes`) set `HasFullFP16`
    // whatever `fullfp16` says (a `-target-feature +sve` alone).
    let fp16 = has("fullfp16")
        || has("sve")
        || if c19 { v9 } else { has("sve-bitperm") || has("sve-aes") };
    let bf16 = has("bf16") || c19 && v8(6);
    let i8mm = has("i8mm") || c19 && v8(6);
    let mut def = |n: &str, v: &str| set(m, n, v);
    def("__ARM_ARCH", &major.to_string());
    if fp {
        def("__ARM_FP", "0xE");
    }
    if neon {
        def("__ARM_NEON", "1");
        def("__ARM_NEON_FP", "0xE");
    }
    let flags: [(bool, &[&str]); 30] = [
        (v8(1), &["QRDMX"]),
        (v8(3), &["COMPLEX", "JCVT"]),
        (v8(5), &["FRINT"]),
        (has("crc") || c19 && v8(1), &["CRC32"]),
        (has("lse") || c19 && v8(1), &["ATOMICS"]),
        (has("dotprod") || c19 && v8(4), &["DOTPROD"]),
        (has("bti") || c19 && v8(5), &["BTI"]),
        (bf16, &["BF16", "BF16_SCALAR_ARITHMETIC", "BF16_VECTOR_ARITHMETIC"]),
        (i8mm, &["MATMUL_INT8"]),
        (has("aes"), &["AES"]),
        (has("sha2"), &["SHA2"]),
        (has("aes") && has("sha2"), &["CRYPTO"]),
        (has("sha3"), &["SHA3", "SHA512"]),
        (has("sm4"), &["SM3", "SM4"]),
        (has("pauth"), &["PAUTH"]),
        (has("mte"), &["MEMORY_TAGGING"]),
        (has("rand"), &["RNG"]),
        (has("mops"), &["MOPS"]),
        (has("ls64"), &["LS64"]),
        (has("gcs"), &["GCS"]),
        (has("d128"), &["SYSREG128"]),
        (has("tme"), &["TME"]),
        (has("cssc") && !c19, &["CSSC"]),
        (has("fprcvt"), &["FPRCVT"]),
        (fp16, &["FP16_SCALAR_ARITHMETIC"]),
        (neon && fp16, &["FP16_VECTOR_ARITHMETIC"]),
        (neon && has("fp16fml"), &["FP16_FML"]),
        (sve, &["SVE"]),
        (sve && bf16, &["SVE_BF16"]),
        (sve && i8mm, &["SVE_MATMUL_INT8"]),
    ];
    for (on, names) in flags {
        if on {
            for n in names {
                def(&format!("__ARM_FEATURE_{n}"), "1");
            }
        }
    }
    for n in sve_feature_macros(t, c19, sve) {
        def(&format!("__ARM_FEATURE_{n}"), "1");
    }
    if bf16 {
        def("__ARM_BF16_FORMAT_ALTERNATIVE", "1");
    }
    if has("rcpc3") {
        def("__ARM_FEATURE_RCPC", "3");
    } else if has("rcpc") || c19 && v8(3) {
        def("__ARM_FEATURE_RCPC", "1");
    }
    if sve {
        def("__ARM_FEATURE_SVE_VECTOR_OPERATORS", "2");
        if c19 {
            def("__ARM_NEON_SVE_BRIDGE", "1");
        }
    }
    if let Some(bits) = t.sve_vector_bits {
        def("__ARM_FEATURE_SVE_BITS", &bits.to_string());
    }
    for (n, v) in &t.branch_protection {
        def(n, v);
    }
}

/// The SVE `__ARM_FEATURE_<name>`s (value 1) beyond `SVE` itself of
/// [`apply_arm`]; `sve` is SVE mode (`sve` with NEON).
fn sve_feature_macros(t: &ArmTarget, c19: bool, sve: bool) -> Vec<&'static str> {
    let has = |f: &str| t.features.iter().any(|x| x == f);
    let v9 = t.arch.0 == 9;
    // clang 22 splits `sve2-aes` / `sve2-bitperm` into `sve2` + `sve-x`.
    let sve2 = has("sve2");
    let more: [(bool, &'static str); 12] = [
        (sve && has("f32mm"), "SVE_MATMUL_FP32"),
        (sve && has("f64mm"), "SVE_MATMUL_FP64"),
        (sve2 || c19 && v9, "SVE2"),
        (if c19 { has("sve2-aes") } else { sve2 && has("sve-aes") }, "SVE2_AES"),
        (if c19 { has("sve2-bitperm") } else { sve2 && has("sve-bitperm") }, "SVE2_BITPERM"),
        (has("sve-aes2"), "SVE_AES2"),
        (has("sve-b16b16"), "SVE_B16B16"),
        (has("sve-f16f32mm"), "SVE_F16F32MM"),
        (has("sve2-sha3"), "SVE2_SHA3"),
        (has("sve2-sm4"), "SVE2_SM4"),
        (has("sve2p1"), "SVE2p1"),
        (has("sve2p2"), "SVE2p2"),
    ];
    more.into_iter().filter_map(|(on, n)| on.then_some(n)).collect()
}

/// clang `DefineBuiltinMacro`: `X` -> `X 1`, `X=v` -> `X v` (the value
/// ends at the first newline).
fn define_line(d: &str) -> String {
    match d.split_once('=') {
        None => format!("{d} 1"),
        Some((name, value)) => {
            let value = value.split(['\n', '\r']).next().unwrap_or("");
            format!("{name} {value}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "#define __EXCEPTIONS 1\n#define __GCC_HAVE_DWARF2_CFI_ASM 1\n\
        #define __NO_INLINE__ 1\n#define __PIC__ 2\n#define __PIE__ 2\n#define __STRICT_ANSI__ 1\n\
        #define __USER_LABEL_PREFIX__ \n#define __cpp_char8_t 202207L\n#define __cpp_exceptions 199711L\n\
        #define __pic__ 2\n#define __pie__ 2\n";

    #[test]
    fn layout_and_cmdline() {
        let mut cfg = PredefConfig::new(true);
        cfg.gnu = false;
        // BASE and the expected layout are clang 19's.
        cfg.clang_compat = ClangCompat::Clang19;
        let s = build_predefines(
            BASE,
            &cfg,
            &[
                CmdMacro::Define("A".into()),
                CmdMacro::Define("B=2".into()),
                CmdMacro::Define("F(x)=x+1".into()),
                CmdMacro::Define("E=".into()),
                CmdMacro::Undef("A".into()),
            ],
        );
        assert!(
            s.starts_with(
                "#define __KILN__ 1\n#define __KILN_MAJOR__ 0\n#define __KILN_MINOR__ 0\n\
                 # 1 \"<built-in>\" 3\n#define __ARM_ARCH 8\n#define __ARM_FP 0xE\n\
                 #define __ARM_NEON 1\n#define __ARM_NEON_FP 0xE\n#define __BFLT16_DECIMAL_DIG__ 4\n"
            ),
            "{s}"
        );
        assert!(s.contains("#define __EXCEPTIONS 1\n"));
        assert!(s.contains("#define __USER_LABEL_PREFIX__ \n"));
        assert!(s.ends_with(
            "# 1 \"<command line>\" 1\n#define A 1\n#define B 2\n#define F(x) x+1\n#define E \n\
             #undef A\n#define __GCC_HAVE_DWARF2_CFI_ASM 1\n# 1 \"<built-in>\" 2\n"
        ));
        assert!(s.contains("#define __KILN__ 1\n"));
        assert!(s.contains("#define __STRICT_ANSI__ 1\n"));
    }

    /// WP-361, [cpp.predefined]/2 and [basic.extended.fp]: the P1467
    /// limit macros in every C++ mode (GCC 14's values), the
    /// `__STDCPP_*_T__` ones from C++23, neither in C.
    #[test]
    fn p1467_macros() {
        let cxx = |v: &str| format!("#define __cplusplus {v}\n{BASE}");
        let cfg = PredefConfig::new(true);
        let s17 = build_predefines(&cxx("201703L"), &cfg, &[]);
        let s23 = build_predefines(&cxx("202302L"), &cfg, &[]);
        let c = build_predefines(BASE, &PredefConfig::new(false), &[]);
        for s in [&s17, &s23] {
            assert!(s.contains("#define __FLT32_MANT_DIG__ 24\n"));
            assert!(s.contains(
                "#define __FLT128_MAX__ 1.18973149535723176508575932662800702e+4932F128\n"
            ));
            assert!(s.contains("#define __BFLT16_DIG__ 2\n"));
            // WP-380: GCC's `_Float32x` (binary64) and `_Float64x`.
            assert!(s.contains("#define __FLT32X_MANT_DIG__ 53\n"));
            assert!(s.contains(
                "#define __FLT32X_MAX__ 1.79769313486231570814527423731704357e+308F32x\n"
            ));
            assert!(s.contains("#define __FLT64X_MANT_DIG__ "));
        }
        assert!(!c.contains("__FLT64_DIG__"));
        for n in P1467_STDCPP {
            assert!(s23.contains(&format!("#define {n} 1\n")), "{n}");
            assert!(!s17.contains(n), "{n}");
        }
    }

    /// [cpp.predefined]/1 (`__cpp_*`, [tab:cpp.predefined.ft]): the
    /// macros of features kiln implements beyond clang 19 take clang
    /// 22's values in its modes (P1061 from C++17, P2893 always).
    #[test]
    fn kiln_feature_macros() {
        let cfg = PredefConfig::new(true);
        let cxx17 = format!("{BASE}#define __cpp_structured_bindings 202403L\n");
        let s = build_predefines(&cxx17, &cfg, &[]);
        assert!(s.contains("#define __cpp_structured_bindings 202411L\n"), "{s}");
        assert!(s.contains("#define __cpp_variadic_friend 202403L\n"), "{s}");
        let s = build_predefines(BASE, &cfg, &[]);
        assert!(!s.contains("__cpp_structured_bindings"), "{s}");
        assert!(s.contains("#define __cpp_variadic_friend 202403L\n"), "{s}");
        let s = build_predefines(&cxx17, &PredefConfig::new(false), &[]);
        assert!(!s.contains("__cpp_variadic_friend"), "{s}");
    }

    #[test]
    fn flag_deltas() {
        let mut cfg = PredefConfig::new(true);
        cfg.opt = OptLevel::Size;
        cfg.exceptions = false;
        cfg.char8_t = Some(false);
        cfg.pic = PicModel::Pic(1);
        let s = build_predefines(BASE, &cfg, &[]);
        for gone in ["__STRICT_ANSI__", "__NO_INLINE__", "__EXCEPTIONS", "__cpp_char8_t", "__PIE__"]
        {
            assert!(!s.contains(&format!("#define {gone} ")), "{gone}");
        }
        for here in [
            "linux 1",
            "__GLIBCXX_TYPE_INT_N_0 __int128",
            "__OPTIMIZE__ 1",
            "__OPTIMIZE_SIZE__ 1",
            "__PIC__ 1",
            "__pic__ 1",
        ] {
            assert!(s.contains(&format!("#define {here}\n")), "{here}");
        }
    }

    /// kiln's own feature-test macros (P2747, P3068, P1306) at C++26, at every
    /// compatibility level, and not before C++26 or without exceptions.
    #[test]
    fn kiln_constexpr_feature_macros() {
        let b26 = "#define __cplusplus 202400L\n#define __cpp_constexpr 202306L\n";
        let b23 = "#define __cplusplus 202302L\n#define __cpp_constexpr 202211L\n";
        let mut cfg = PredefConfig::new(true);
        for compat in [ClangCompat::Clang19, ClangCompat::Clang22] {
            cfg.clang_compat = compat;
            let s = build_predefines(b26, &cfg, &[]);
            assert!(s.contains("#define __cpp_constexpr 202406L\n"), "{s}");
            assert!(s.contains("#define __cpp_constexpr_exceptions 202411L\n"), "{s}");
            assert!(s.contains("#define __cpp_expansion_statements 202506L\n"), "{s}");
            assert!(s.contains("#define __cpp_contracts 202502L\n"), "{s}");
        }
        let s = build_predefines(b23, &cfg, &[]);
        assert!(s.contains("#define __cpp_constexpr 202211L\n"), "{s}");
        assert!(!s.contains("__cpp_constexpr_exceptions"), "{s}");
        assert!(!s.contains("__cpp_expansion_statements"), "{s}");
        assert!(!s.contains("__cpp_contracts"), "{s}");
        cfg.exceptions = false;
        let s = build_predefines(b26, &cfg, &[]);
        assert!(!s.contains("__cpp_constexpr_exceptions"), "{s}");
        assert!(s.contains("#define __cpp_constexpr 202406L\n"), "{s}");
    }

    /// P2841R7's `__cpp_template_parameters` (WP-771): C++26 at
    /// `-fclang-compat=22` only.
    #[test]
    fn template_parameters_macro() {
        let b26 = "#define __cplusplus 202400L\n";
        let b23 = "#define __cplusplus 202302L\n";
        let mut cfg = PredefConfig::new(true);
        let s = build_predefines(b26, &cfg, &[]);
        assert!(s.contains("#define __cpp_template_parameters 202502L\n"), "{s}");
        let s = build_predefines(b23, &cfg, &[]);
        assert!(!s.contains("__cpp_template_parameters"), "{s}");
        cfg.clang_compat = ClangCompat::Clang19;
        let s = build_predefines(b26, &cfg, &[]);
        assert!(!s.contains("__cpp_template_parameters"), "{s}");
    }

    /// WP-525: `-freflection` predefines GCC 16.1's
    /// `__cpp_impl_reflection 202603L`, at C++26 only.
    #[test]
    fn reflection_macro() {
        let b26 = "#define __cplusplus 202400L\n";
        let b23 = "#define __cplusplus 202302L\n";
        let mut cfg = PredefConfig::new(true);
        assert!(!build_predefines(b26, &cfg, &[]).contains("__cpp_impl_reflection"));
        cfg.reflection = true;
        assert!(
            build_predefines(b26, &cfg, &[]).contains("#define __cpp_impl_reflection 202603L\n")
        );
        assert!(!build_predefines(b23, &cfg, &[]).contains("__cpp_impl_reflection"));
    }

    /// `-fsigned-char` removes `__CHAR_UNSIGNED__` (the `AArch64` default),
    /// `-funsigned-char` adds it (x86-64), as clang 19's driver does.
    #[test]
    fn char_signedness() {
        let unsigned_base = format!("{BASE}#define __CHAR_UNSIGNED__ 1\n");
        let mut cfg = PredefConfig::new(true);
        assert!(
            build_predefines(&unsigned_base, &cfg, &[]).contains("#define __CHAR_UNSIGNED__ 1\n")
        );
        cfg.char_signed = Some(true);
        assert!(!build_predefines(&unsigned_base, &cfg, &[]).contains("__CHAR_UNSIGNED__"));
        cfg.char_signed = Some(false);
        assert!(build_predefines(BASE, &cfg, &[]).contains("#define __CHAR_UNSIGNED__ 1\n"));
    }

    /// `__cpp_explicit_this_parameter` (P0847) from C++23 on, as clang 22
    /// defines it; not in C.
    #[test]
    fn explicit_this_parameter_macro() {
        let m = "__cpp_explicit_this_parameter 202110L";
        let cfg = PredefConfig::new(true);
        for (std, on) in
            [("201703L", false), ("202002L", false), ("202302L", true), ("202400L", true)]
        {
            let base = format!("#define __cplusplus {std}\n");
            assert_eq!(build_predefines(&base, &cfg, &[]).contains(m), on, "{std}");
        }
        let c = PredefConfig::new(false);
        assert!(!build_predefines("#define __STDC_VERSION__ 202311L\n", &c, &[]).contains(m));
    }

    /// spec/pp.md "clang compatibility level": compat 19 leaves the base
    /// alone; compat 22 applies the clang 22 version, width and
    /// integer-constant macros, plus the `AArch64` ones on `AArch64` only.
    #[test]
    fn clang_compat_levels() {
        let base = "#define __BOOL_WIDTH__ 8\n#define __clang_major__ 19\n#define __ARM_ACLE 200\n";
        let mut cfg = PredefConfig::new(true);
        assert_eq!(cfg.clang_compat, ClangCompat::Clang22);
        cfg.clang_compat = ClangCompat::Clang19;
        let s19 = build_predefines(base, &cfg, &[]);
        for here in ["__BOOL_WIDTH__ 8", "__clang_major__ 19", "__ARM_ACLE 200"] {
            assert!(s19.contains(&format!("#define {here}\n")), "{here}: {s19}");
        }
        assert!(!s19.contains("__INT64_C"));
        cfg.clang_compat = ClangCompat::Clang22;
        cfg.aarch64 = false;
        let s22 = build_predefines(base, &cfg, &[]);
        for here in [
            "__BOOL_WIDTH__ 1",
            "__clang_major__ 22",
            "__clang_patchlevel__ 8",
            "__INT64_C(c) c##L",
            "__UINTMAX_C(c) c##UL",
            "__MEMORY_SCOPE_CLUSTR 5",
            "__ARM_ACLE 200",
        ] {
            assert!(s22.contains(&format!("#define {here}\n")), "{here}: {s22}");
        }
        assert!(!s22.contains("__GCC_DESTRUCTIVE_SIZE"));
        cfg.aarch64 = true;
        let a22 = build_predefines(base, &cfg, &[]);
        for here in ["__ARM_ACLE 202420", "__GCC_DESTRUCTIVE_SIZE 256", "__ARM_NEON_SVE_BRIDGE 1"] {
            assert!(a22.contains(&format!("#define {here}\n")), "{here}: {a22}");
        }
    }

    /// The `__ARM_*` target macros of `arch` + `features` under `compat`.
    fn arm(arch: (u8, u8), features: &str, bits: Option<u32>, compat: ClangCompat) -> String {
        let mut cfg = PredefConfig::new(true);
        cfg.clang_compat = compat;
        cfg.arm = ArmTarget {
            arch,
            features: features.split(',').filter(|f| !f.is_empty()).map(str::to_owned).collect(),
            sve_vector_bits: bits,
            branch_protection: Vec::new(),
        };
        let base = "#define __ARM_ARCH 8\n#define __ARM_FP 0xE\n#define __ARM_NEON 1\n\
            #define __ARM_NEON_FP 0xE\n#define __ARM_FEATURE_TME 1\n";
        let mut v: Vec<_> = build_predefines(base, &cfg, &[])
            .lines()
            .filter_map(|l| l.strip_prefix("#define __ARM_"))
            .filter(|l| !l.starts_with("ACLE") && !l.starts_with("PREFETCH"))
            .map(|l| l.replace("FEATURE_", ""))
            .collect();
        v.sort_unstable();
        v.join(",")
    }

    #[test]
    fn arm_target_macros() {
        let (c19, c22) = (ClangCompat::Clang19, ClangCompat::Clang22);
        let sorted = |s: &str| {
            let mut v: Vec<_> = s.split(',').collect();
            v.sort_unstable();
            v.join(",")
        };
        let assert_arm = |got: String, want: &str| assert_eq!(got, sorted(want));
        let base = "ARCH 8,FP 0xE,NEON 1,NEON_FP 0xE";
        assert_arm(arm((8, 0), "fp-armv8,neon", None, c19), base);
        assert_arm(arm((8, 0), "fp-armv8,neon", None, c22), &format!("{base},NEON_SVE_BRIDGE 1"));
        let fp16 = "FP16_SCALAR_ARITHMETIC 1,FP16_VECTOR_ARITHMETIC 1";
        assert_arm(arm((8, 0), "fp-armv8,fullfp16,neon", None, c19), &format!("{base},{fp16}"));
        assert_arm(
            arm((8, 0), "fp-armv8,fullfp16,neon,sve", Some(256), c19),
            &format!("{base},{fp16},SVE 1,SVE_BITS 256,SVE_VECTOR_OPERATORS 2,NEON_SVE_BRIDGE 1"),
        );
        assert_arm(
            arm((8, 0), "fp-armv8,fullfp16,neon,sve,sve2", None, c22),
            &format!("{base},{fp16},SVE 1,SVE2 1,SVE_VECTOR_OPERATORS 2,NEON_SVE_BRIDGE 1"),
        );
        // No NEON: no SVE mode, scalar FP16 only.
        assert_arm(
            arm((8, 0), "fp-armv8,fullfp16,sve", None, c19),
            "ARCH 8,FP 0xE,FP16_SCALAR_ARITHMETIC 1",
        );
        assert_arm(arm((8, 0), "", None, c19), "ARCH 8");
        // clang 19 ties `crc` / `lse` / `rdm` to v8.1, clang 22 follows
        // the features (here removed).
        assert_arm(
            arm((8, 1), "fp-armv8,neon", None, c19),
            &format!("{base},ATOMICS 1,CRC32 1,QRDMX 1"),
        );
        assert_arm(
            arm((8, 1), "fp-armv8,neon", None, c22),
            &format!("{base},QRDMX 1,NEON_SVE_BRIDGE 1"),
        );
        assert_arm(
            arm((9, 0), "fp-armv8,neon", None, c19),
            "ARCH 9,FP 0xE,NEON 1,NEON_FP 0xE,ATOMICS 1,BTI 1,COMPLEX 1,CRC32 1,DOTPROD 1,\
             FP16_SCALAR_ARITHMETIC 1,FP16_VECTOR_ARITHMETIC 1,FRINT 1,JCVT 1,QRDMX 1,RCPC 1,SVE2 1",
        );
        let crypto = "AES 1,CRYPTO 1,SHA2 1,SHA3 1,SHA512 1,SM3 1,SM4 1";
        assert_arm(
            arm((8, 0), "aes,fp-armv8,neon,sha2,sha3,sm4", None, c22),
            &format!("{base},{crypto},NEON_SVE_BRIDGE 1"),
        );
        assert_arm(arm((8, 0), "fp-armv8,neon,rcpc,rcpc3", None, c19), &format!("{base},RCPC 3"));
    }
}
