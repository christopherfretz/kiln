//! The predefines buffer (clang's `<built-in>`): the target/standard
//! macro set from `resource/predefines/<triple>.<std>.txt`, adjusted for
//! the dialect and the flags that change predefined macros, kiln's own
//! identity macros, then the `-D` / `-U` options in command-line order as
//! clang's `<command line>` section. See `spec/pp.md` "Predefines" and
//! `resource/README.md` for the measured flag effects.

use std::collections::BTreeMap;
use std::fmt::Write as _;

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
    /// `-fexceptions` (C++ default on).
    pub exceptions: bool,
    /// `-frtti` (C++ default on).
    pub rtti: bool,
    /// `-fchar8_t` / `-fno-char8_t` if given (default: on from C++20).
    pub char8_t: Option<bool>,
    /// PIC / PIE.
    pub pic: PicModel,
    /// `-ffast-math`.
    pub fast_math: bool,
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
}

impl PredefConfig {
    /// clang's defaults for C++ (`cplusplus`) or C on Linux, GNU dialect.
    pub fn new(cplusplus: bool) -> PredefConfig {
        PredefConfig {
            gnu: true,
            cplusplus,
            aarch64: true,
            opt: OptLevel::None,
            exceptions: cplusplus,
            rtti: cplusplus,
            char8_t: None,
            pic: PicModel::Pie(2),
            fast_math: false,
            freestanding: false,
            pthread: false,
            threadsafe_statics: true,
            sized_deallocation: true,
            kiln_version: (0, 0),
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
    let mut driver: Vec<(String, String)> = Vec::new();
    for d in DRIVER_DEFINES {
        if let Some(v) = m.remove(*d) {
            driver.push(((*d).to_owned(), v));
        }
    }
    apply_flags(&mut m, cfg);

    let mut out = String::with_capacity(base.len() + 256);
    out.push_str("# 1 \"<built-in>\" 3\n");
    for (n, v) in &m {
        let _ = writeln!(out, "#define {n} {v}");
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
    out.push_str("# 1 \"<built-in>\" 2\n");
    out
}

fn set(m: &mut BTreeMap<String, String>, n: &str, v: &str) {
    m.insert(n.to_owned(), v.to_owned());
}

/// Applies the dialect and flag deltas (measured against clang 19; see
/// `resource/README.md`) and kiln's identity macros.
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
    match cfg.opt {
        OptLevel::None => {}
        OptLevel::Speed | OptLevel::Size => {
            m.remove("__NO_INLINE__");
            set(m, "__OPTIMIZE__", "1");
            if cfg.opt == OptLevel::Size {
                set(m, "__OPTIMIZE_SIZE__", "1");
            }
        }
    }
    if cfg.cplusplus {
        if !cfg.exceptions {
            m.remove("__EXCEPTIONS");
            m.remove("__cpp_exceptions");
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
    if cfg.fast_math {
        set(m, "__FAST_MATH__", "1");
        set(m, "__FINITE_MATH_ONLY__", "1");
        set(m, "__NO_MATH_ERRNO__", "1");
        if cfg.aarch64 {
            set(m, "__ARM_FP_FAST", "1");
        }
    }
    if cfg.freestanding {
        set(m, "__STDC_HOSTED__", "0");
    }
    if cfg.pthread {
        set(m, "_REENTRANT", "1");
    }
    set(m, "__KILN__", "1");
    set(m, "__KILN_MAJOR__", &cfg.kiln_version.0.to_string());
    set(m, "__KILN_MINOR__", &cfg.kiln_version.1.to_string());
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
        assert!(s.starts_with("# 1 \"<built-in>\" 3\n#define __EXCEPTIONS 1\n"), "{s}");
        assert!(s.contains("#define __USER_LABEL_PREFIX__ \n"));
        assert!(s.ends_with(
            "# 1 \"<command line>\" 1\n#define A 1\n#define B 2\n#define F(x) x+1\n#define E \n\
             #undef A\n#define __GCC_HAVE_DWARF2_CFI_ASM 1\n# 1 \"<built-in>\" 2\n"
        ));
        assert!(s.contains("#define __KILN__ 1\n"));
        assert!(s.contains("#define __STRICT_ANSI__ 1\n"));
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
}
