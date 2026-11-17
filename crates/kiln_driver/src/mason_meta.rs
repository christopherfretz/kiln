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

/// The `abi.stdlib*` rows of a C++ unit built against `lib`.
///
/// - libstdc++ (when `__GLIBCXX__` is defined): `abi.stdlib-abi` is the
///   value of `_GLIBCXX_USE_CXX11_ABI`; `abi.stdlib-debug` is
///   `_GLIBCXX_DEBUG` (layout-changing), else `_GLIBCXX_ASSERTIONS`, else
///   `off`.
/// - libc++ (when `_LIBCPP_VERSION` is defined): `abi.stdlib-abi` is the
///   value of `_LIBCPP_ABI_VERSION`; `abi.stdlib-debug` is the hardening
///   mode's macro name (`_LIBCPP_HARDENING_MODE_FAST`, `..._EXTENSIVE`,
///   `..._DEBUG`), `off` for `_LIBCPP_HARDENING_MODE_NONE`; no hardening
///   mode changes a layout.
///
/// A TU that includes no library header has neither row.
fn stdlib_summary(lib: StdLib, m: &Macros<'_, '_, '_>) -> StdlibSummary {
    match lib {
        StdLib::LibStdCxx => {
            let present = m.defined("__GLIBCXX__");
            StdlibSummary {
                name: "libstdc++",
                abi: if present { m.value("_GLIBCXX_USE_CXX11_ABI") } else { None },
                debug: present.then(|| {
                    if m.defined("_GLIBCXX_DEBUG") {
                        ("_GLIBCXX_DEBUG".to_owned(), true)
                    } else if m.defined("_GLIBCXX_ASSERTIONS") {
                        ("_GLIBCXX_ASSERTIONS".to_owned(), false)
                    } else {
                        ("off".to_owned(), false)
                    }
                }),
            }
        }
        StdLib::LibCxx => {
            let present = m.defined("_LIBCPP_VERSION");
            let mode = || {
                let v = m.eval("_LIBCPP_HARDENING_MODE")?;
                let named = ["NONE", "FAST", "EXTENSIVE", "DEBUG"]
                    .into_iter()
                    .find(|k| m.eval(&format!("_LIBCPP_HARDENING_MODE_{k}")) == Some(v))?;
                Some(if named == "NONE" {
                    "off".to_owned()
                } else {
                    format!("_LIBCPP_HARDENING_MODE_{named}")
                })
            };
            StdlibSummary {
                name: "libc++",
                abi: if present { m.value("_LIBCPP_ABI_VERSION") } else { None },
                debug: if present { mode().map(|v| (v, false)) } else { None },
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
