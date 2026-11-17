//! What the driver decides for the `.mason.meta` unit's TU summary
//! (`-fmason-metadata[=level]`, WP-1557; spec/codegen.md "Link-time
//! metadata (`.mason.meta`)"): the producer, the standard, the
//! `-fshort-enums` switch and the C++ standard library's rows, the last
//! two read from the preprocessor's macro table at the end of the TU.

use kiln_codegen::{MasonMetaLevel, MasonMetaOptions, StdlibSummary};
use kiln_pp::Preprocessor;

use crate::args::Args;
use crate::toolchain::{LangStd, StdLib};

/// The driver's part of the unit for the TU `pp` has just finished, or
/// `None` without `-fmason-metadata`.
pub(crate) fn options(
    a: &Args,
    std: &LangStd,
    pp: &Preprocessor<'_, '_>,
) -> Option<MasonMetaOptions> {
    let level: MasonMetaLevel = a.codegen.mason_meta?;
    let macros = Macros(pp);
    Some(MasonMetaOptions {
        level,
        producer: format!("kiln {}", crate::version::VERSION),
        std: std.base.to_owned(),
        short_enums: a.predef.short_enums,
        stdlib: std.cplusplus.then(|| stdlib_summary(a.stdlib.unwrap_or_default(), &macros)),
    })
}

/// libc++'s vendor ABI switches that change the layout (or the
/// representation) of a library type, in byte order of the names: each
/// one defined at the end of the TU is appended to `abi.stdlib-abi` as
/// `+<name>` (WP-1564). The classification of every `_LIBCPP_ABI_*` macro
/// of libc++ 19 and 22, with its reason, is the table in spec/codegen.md
/// "Link-time metadata (`.mason.meta`)".
pub(crate) const LIBCXX_LAYOUT_SWITCHES: [&str; 15] = [
    "_LIBCPP_ABI_ALTERNATE_STRING_LAYOUT",
    "_LIBCPP_ABI_BOUNDED_ITERATORS",
    "_LIBCPP_ABI_BOUNDED_ITERATORS_IN_OPTIONAL",
    "_LIBCPP_ABI_BOUNDED_ITERATORS_IN_STD_ARRAY",
    "_LIBCPP_ABI_BOUNDED_ITERATORS_IN_STRING",
    "_LIBCPP_ABI_BOUNDED_ITERATORS_IN_VECTOR",
    "_LIBCPP_ABI_BOUNDED_UNIQUE_PTR",
    "_LIBCPP_ABI_INCOMPLETE_TYPES_IN_DEQUE",
    "_LIBCPP_ABI_IOS_ALLOW_ARBITRARY_FILL_VALUE",
    "_LIBCPP_ABI_NO_COMPRESSED_PAIR_PADDING",
    "_LIBCPP_ABI_NO_ITERATOR_BASES",
    "_LIBCPP_ABI_NO_RANDOM_DEVICE_COMPATIBILITY_LAYOUT",
    "_LIBCPP_ABI_NO_REVERSE_ITERATOR_SECOND_MEMBER",
    "_LIBCPP_ABI_OPTIMIZED_FUNCTION",
    "_LIBCPP_ABI_VARIANT_INDEX_TYPE_OPTIMIZATION",
];

/// The `abi.stdlib*` rows of a C++ unit built against `lib` (mortar
/// `spec/meta.md` MT-26 as amended for WP-1564).
///
/// - libstdc++ (when `__GLIBCXX__` is defined): `abi.stdlib-abi` is the
///   value of `_GLIBCXX_USE_CXX11_ABI`; `abi.stdlib-debug` is
///   `_GLIBCXX_DEBUG` when defined, else `off` (`_GLIBCXX_ASSERTIONS`
///   counts as `off` there); `abi.stdlib-hardening` is
///   `_GLIBCXX_ASSERTIONS` when defined, else no row.
/// - libc++ (when `_LIBCPP_VERSION` is defined): `abi.stdlib-abi` is the
///   value of `_LIBCPP_ABI_VERSION` followed by `+<name>` for each
///   defined switch of [`LIBCXX_LAYOUT_SWITCHES`]; `abi.stdlib-debug` is
///   `off` (libc++ 17+ has no debug ABI); `abi.stdlib-hardening` is the
///   hardening mode, `none`, `fast`, `extensive` or `debug` (no row when
///   `_LIBCPP_HARDENING_MODE` names none of them).
///
/// A TU that includes no library header has none of the three rows.
fn stdlib_summary(lib: StdLib, m: &Macros<'_, '_, '_>) -> StdlibSummary {
    match lib {
        StdLib::LibStdCxx => {
            let present = m.defined("__GLIBCXX__");
            let flag = |name: &str| m.defined(name).then(|| name.to_owned());
            StdlibSummary {
                name: "libstdc++",
                abi: if present { m.value("_GLIBCXX_USE_CXX11_ABI") } else { None },
                debug: present.then(|| flag("_GLIBCXX_DEBUG").unwrap_or_else(|| "off".to_owned())),
                hardening: if present { flag("_GLIBCXX_ASSERTIONS") } else { None },
            }
        }
        StdLib::LibCxx => {
            let present = m.defined("_LIBCPP_VERSION");
            let mode = || {
                let v = m.eval("_LIBCPP_HARDENING_MODE")?;
                let named = ["NONE", "FAST", "EXTENSIVE", "DEBUG"]
                    .into_iter()
                    .find(|k| m.eval(&format!("_LIBCPP_HARDENING_MODE_{k}")) == Some(v))?;
                Some(named.to_ascii_lowercase())
            };
            let abi = || {
                let mut v = m.value("_LIBCPP_ABI_VERSION")?;
                for s in LIBCXX_LAYOUT_SWITCHES.iter().filter(|s| m.defined(s)) {
                    v.push('+');
                    v.push_str(s);
                }
                Some(v)
            };
            StdlibSummary {
                name: "libc++",
                abi: if present { abi() } else { None },
                debug: present.then(|| "off".to_owned()),
                hardening: if present { mode() } else { None },
            }
        }
    }
}

/// The macro table at the end of the TU.
struct Macros<'p, 'a, 'b>(&'p Preprocessor<'a, 'b>);

impl Macros<'_, '_, '_> {
    /// The object-like macro `name`'s replacement list, spelled token by
    /// token; `None` if it is not defined as an object-like macro.
    fn body(&self, name: &str) -> Option<Vec<String>> {
        let pp = self.0;
        let sym = pp.interner().get(name)?;
        let def = pp.macros().get(sym)?;
        if def.is_function_like() || def.is_builtin() {
            return None;
        }
        let (sm, lex) = (pp.source_manager(), pp.lex_options());
        Some(def.body().map(|t| kiln_lex::spelling(sm, &t, lex).into_owned()).collect())
    }

    fn defined(&self, name: &str) -> bool {
        self.0.interner().get(name).is_some_and(|s| self.0.macros().is_defined(s))
    }

    /// `name`'s value: its integer value when it has one ([`Self::eval`]),
    /// else its replacement list as written (tokens joined by a space).
    fn value(&self, name: &str) -> Option<String> {
        if let Some(v) = self.eval(name) {
            return Some(v.to_string());
        }
        self.body(name).map(|b| b.join(" "))
    }

    /// The integer value of macro `name` when its replacement list (after
    /// expanding object-like macros, at most 16 deep) is an integer
    /// literal, or parentheses, `<<` and `|` over such (libc++'s
    /// `(1 << 2)`); `None` otherwise.
    fn eval(&self, name: &str) -> Option<i64> {
        let mut toks = Vec::new();
        self.expand(name, 16, &mut toks)?;
        let mut p = Eval { toks: &toks, at: 0 };
        let v = p.or()?;
        (p.at == toks.len()).then_some(v)
    }

    fn expand(&self, name: &str, depth: u32, out: &mut Vec<String>) -> Option<()> {
        for t in self.body(name)? {
            let ident = t.starts_with(|c: char| c == '_' || c.is_ascii_alphabetic());
            if ident {
                self.expand(&t, depth.checked_sub(1)?, out)?;
            } else {
                out.push(t);
            }
        }
        Some(())
    }
}

/// A recursive descent over `|`, `<<`, parentheses and integer literals.
struct Eval<'t> {
    toks: &'t [String],
    at: usize,
}

impl<'t> Eval<'t> {
    fn peek(&self) -> Option<&'t str> {
        self.toks.get(self.at).map(String::as_str)
    }

    fn or(&mut self) -> Option<i64> {
        let mut v = self.shift()?;
        while self.peek() == Some("|") {
            self.at += 1;
            v |= self.shift()?;
        }
        Some(v)
    }

    fn shift(&mut self) -> Option<i64> {
        let mut v = self.primary()?;
        while self.peek() == Some("<<") {
            self.at += 1;
            let n = u32::try_from(self.primary()?).ok().filter(|n| *n < 63)?;
            v = v.checked_shl(n)?;
        }
        Some(v)
    }

    fn primary(&mut self) -> Option<i64> {
        let t = self.peek()?;
        self.at += 1;
        if t == "(" {
            let v = self.or()?;
            (self.peek() == Some(")")).then_some(())?;
            self.at += 1;
            return Some(v);
        }
        let digits = t.trim_end_matches(['u', 'U', 'l', 'L']);
        if let Some(h) = digits.strip_prefix("0x").or_else(|| digits.strip_prefix("0X")) {
            i64::from_str_radix(h, 16).ok()
        } else {
            digits.parse().ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Eval;

    fn eval(s: &str) -> Option<i64> {
        let toks: Vec<String> = s.split(' ').map(str::to_owned).collect();
        let mut p = Eval { toks: &toks, at: 0 };
        let v = p.or()?;
        (p.at == toks.len()).then_some(v)
    }

    #[test]
    fn layout_switches_are_in_byte_order() {
        let l = super::LIBCXX_LAYOUT_SWITCHES;
        assert!(l.windows(2).all(|w| w[0].as_bytes() < w[1].as_bytes()));
    }

    #[test]
    fn evaluates_the_library_spellings() {
        assert_eq!(eval("1"), Some(1));
        assert_eq!(eval("( 1 << 4 )"), Some(16));
        assert_eq!(eval("0x10UL"), Some(16));
        assert_eq!(eval("( 1 << 1 ) | 4"), Some(6));
        assert_eq!(eval("( 1 << 1"), None);
        assert_eq!(eval("__1"), None);
        assert_eq!(eval("1 2"), None);
    }
}
