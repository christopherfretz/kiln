//! Test classification by file name and the lit keywords of a libc++ test
//! (`spec/libcxx.md` "Classification" and "Directives").

use super::boolexpr::Expr;

/// What a test file asks for, from its suffix (libc++'s `format.py`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// `.pass.cpp`: compile, link and run; the program must exit 0.
    Pass,
    /// `.compile.pass.cpp`: `-fsyntax-only` must succeed.
    CompilePass,
    /// `.compile.fail.cpp`: `-fsyntax-only` must fail.
    CompileFail,
    /// `.link.pass.cpp`: compile and link must succeed.
    LinkPass,
    /// `.link.fail.cpp`: compile (`-c`) must succeed, then the link fail.
    LinkFail,
    /// `.verify.cpp`: clang-verify (`expected-error` comments).
    Verify,
    /// `.sh.<ext>`: a shell script in `// RUN:` lines.
    Sh,
    /// `.gen.<ext>`: a script that generates tests.
    Gen,
    /// `.pass.mm` and friends: Objective-C++.
    ObjCxx,
}

impl Kind {
    /// Classifies a file name, or `None` when it is not a test (a helper
    /// header, an input file). The order is `format.py`'s: `.sh.*` and
    /// `.gen.*` first, then the compound suffixes, then `.pass.cpp`.
    pub fn of(file_name: &str) -> Option<Self> {
        let ext_after = |marker: &str| {
            file_name.rfind(marker).is_some_and(|i| {
                !file_name[i + marker.len()..].is_empty()
                    && !file_name[i + marker.len()..].contains('.')
            })
        };
        if file_name.starts_with('.') {
            return None;
        }
        if ext_after(".sh.") {
            return Some(Self::Sh);
        }
        if ext_after(".gen.") {
            return Some(Self::Gen);
        }
        let k = if file_name.ends_with(".compile.pass.cpp") {
            Self::CompilePass
        } else if file_name.ends_with(".compile.fail.cpp") {
            Self::CompileFail
        } else if file_name.ends_with(".link.pass.cpp") {
            Self::LinkPass
        } else if file_name.ends_with(".link.fail.cpp") {
            Self::LinkFail
        } else if file_name.ends_with(".verify.cpp") {
            Self::Verify
        } else if file_name.ends_with(".pass.cpp") {
            Self::Pass
        } else if file_name.ends_with(".pass.mm") {
            Self::ObjCxx
        } else {
            return None;
        };
        Some(k)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::CompilePass => "compile.pass",
            Self::CompileFail => "compile.fail",
            Self::LinkPass => "link.pass",
            Self::LinkFail => "link.fail",
            Self::Verify => "verify",
            Self::Sh => "sh",
            Self::Gen => "gen",
            Self::ObjCxx => "objective-c++",
        }
    }

    /// Why the driver cannot run this kind, if it cannot.
    pub fn unsupported(self) -> Option<&'static str> {
        match self {
            Self::Sh => Some("driver: RUN lines"),
            Self::Gen => Some("driver: generated tests"),
            Self::ObjCxx => Some("driver: objective-c++"),
            _ => None,
        }
    }
}

/// The lit keywords of one test file.
#[derive(Clone, Debug, Default)]
pub struct Directives {
    pub requires: Vec<(String, Expr)>,
    pub unsupported: Vec<(String, Expr)>,
    /// `XFAIL:` expressions; `*` is stored as [`Expr::True`].
    pub xfail: Vec<(String, Expr)>,
    /// `ADDITIONAL_COMPILE_FLAGS:` (whitespace-split), in order.
    pub compile_flags: Vec<String>,
    /// `ADDITIONAL_COMPILE_FLAGS(<feature>):` with the feature.
    pub conditional_flags: Vec<(String, String)>,
    pub file_dependencies: Vec<String>,
    pub module_dependencies: Vec<String>,
    /// The file has a `RUN:` line (a custom script).
    pub has_run: bool,
}

/// The keywords recognized. lit finds a keyword anywhere on a line; here it
/// must not follow an identifier character, so `FOO_RUN:` is not `RUN:`.
const KEYWORDS: &[&str] = &[
    "REQUIRES:",
    "UNSUPPORTED:",
    "XFAIL:",
    "ADDITIONAL_COMPILE_FLAGS:",
    "ADDITIONAL_COMPILE_FLAGS(",
    "FILE_DEPENDENCIES:",
    "MODULE_DEPENDENCIES:",
    "RUN:",
    "END.",
];

impl Directives {
    /// Parses the keywords of `text`. A malformed boolean expression or an
    /// unterminated conditional keyword is an error (the test is an ERROR).
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut d = Self::default();
        'lines: for line in text.lines() {
            let mut search = 0;
            while let Some((at, kw)) = find_keyword(line, search) {
                let rest = &line[at + kw.len()..];
                search = line.len();
                match kw {
                    "END." => break 'lines,
                    "RUN:" => d.has_run = true,
                    "REQUIRES:" | "UNSUPPORTED:" | "XFAIL:" => {
                        for item in rest.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                            let e = if kw == "XFAIL:" && item == "*" {
                                Expr::True
                            } else {
                                Expr::parse(item).map_err(|e| format!("{kw} {}", e.0))?
                            };
                            let list = match kw {
                                "REQUIRES:" => &mut d.requires,
                                "UNSUPPORTED:" => &mut d.unsupported,
                                _ => &mut d.xfail,
                            };
                            list.push((item.to_owned(), e));
                        }
                    }
                    "ADDITIONAL_COMPILE_FLAGS:" => {
                        d.compile_flags.extend(rest.split_whitespace().map(str::to_owned));
                    }
                    "ADDITIONAL_COMPILE_FLAGS(" => {
                        let Some((feature, flags)) = rest.split_once("):") else {
                            return Err(format!(
                                "malformed `ADDITIONAL_COMPILE_FLAGS(` in `{line}`"
                            ));
                        };
                        for f in flags.split_whitespace() {
                            d.conditional_flags.push((feature.trim().to_owned(), f.to_owned()));
                        }
                    }
                    "FILE_DEPENDENCIES:" => d.file_dependencies.extend(
                        rest.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned),
                    ),
                    "MODULE_DEPENDENCIES:" => {
                        d.module_dependencies.extend(rest.split_whitespace().map(str::to_owned));
                    }
                    _ => unreachable!("keyword {kw} is in KEYWORDS"),
                }
            }
        }
        Ok(d)
    }

    /// The flags for this configuration: the plain ones, then the
    /// conditional ones whose feature is in `features`, in file order
    /// within each group.
    pub fn flags_for(&self, features: &[&str]) -> Vec<String> {
        let mut v = self.compile_flags.clone();
        v.extend(
            self.conditional_flags
                .iter()
                .filter(|(f, _)| features.contains(&f.as_str()))
                .map(|(_, flag)| flag.clone()),
        );
        v
    }

    /// lit's verdict before running: `Err(reason)` when the test is
    /// unsupported (a `REQUIRES:` is false, or an `UNSUPPORTED:` is true),
    /// else whether an `XFAIL:` applies (with the expression).
    pub fn verdict(&self, features: &[&str]) -> Result<Option<String>, String> {
        let missing: Vec<&str> = self
            .requires
            .iter()
            .filter(|(_, e)| !e.eval(features))
            .map(|(s, _)| s.as_str())
            .collect();
        if !missing.is_empty() {
            return Err(format!("REQUIRES: {}", missing.join(", ")));
        }
        if let Some((s, _)) = self.unsupported.iter().find(|(_, e)| e.eval(features)) {
            return Err(format!("UNSUPPORTED: {s}"));
        }
        Ok(self.xfail.iter().find(|(_, e)| e.eval(features)).map(|(s, _)| s.clone()))
    }
}

/// The first keyword at or after `from` in `line` that does not follow an
/// identifier character.
fn find_keyword(line: &str, from: usize) -> Option<(usize, &'static str)> {
    let mut best: Option<(usize, &'static str)> = None;
    for &kw in KEYWORDS {
        let mut start = from;
        while let Some(i) = line.get(start..).and_then(|s| s.find(kw)).map(|i| i + start) {
            let prev = line[..i].chars().next_back();
            if prev.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
                start = i + kw.len();
                continue;
            }
            // `ADDITIONAL_COMPILE_FLAGS:` and `(` share a prefix; the
            // earliest match wins, then the longest keyword.
            if best.is_none_or(|(b, bk)| i < b || i == b && kw.len() > bk.len()) {
                best = Some((i, kw));
            }
            break;
        }
    }
    best
}

/// A `lit.local.cfg` in the subset this driver understands: each
/// statement is `if "<feature>" [not] in config.available_features:`
/// followed by `config.unsupported = True`. Anything else makes the whole
/// directory UNSUPPORTED (it cannot be evaluated without lit).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalCfg {
    /// The directory is unsupported when any `(feature, negated)` holds.
    Conditions(Vec<(String, bool)>),
    /// The file has statements outside the subset.
    Opaque,
}

impl LocalCfg {
    pub fn parse(text: &str) -> Self {
        let mut conds = Vec::new();
        let mut pending: Option<(String, bool)> = None;
        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if let Some(cond) = pending.take() {
                if line == "config.unsupported = True" {
                    conds.push(cond);
                    continue;
                }
                return Self::Opaque;
            }
            let Some(body) = line.strip_prefix("if ").and_then(|l| l.strip_suffix(':')) else {
                return Self::Opaque;
            };
            let (feature, negated) =
                if let Some(f) = body.strip_suffix(" not in config.available_features") {
                    (f, true)
                } else if let Some(f) = body.strip_suffix(" in config.available_features") {
                    (f, false)
                } else {
                    return Self::Opaque;
                };
            let Some(feature) = feature.trim().strip_prefix('"').and_then(|f| f.strip_suffix('"'))
            else {
                return Self::Opaque;
            };
            pending = Some((feature.to_owned(), negated));
        }
        if pending.is_some() { Self::Opaque } else { Self::Conditions(conds) }
    }

    /// `Some(reason)` when the configuration makes the directory unsupported.
    pub fn unsupported(&self, features: &[&str]) -> Option<String> {
        match self {
            Self::Opaque => Some("driver: lit.local.cfg".to_owned()),
            Self::Conditions(c) => c
                .iter()
                .find(|(f, negated)| features.contains(&f.as_str()) != *negated)
                .map(|(f, negated)| {
                    format!("lit.local.cfg: {}{f}", if *negated { "missing " } else { "" })
                }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification() {
        let cases: &[(&str, Option<Kind>)] = &[
            ("is_same.pass.cpp", Some(Kind::Pass)),
            ("types.compile.pass.cpp", Some(Kind::CompilePass)),
            ("bad.compile.fail.cpp", Some(Kind::CompileFail)),
            ("x.link.pass.cpp", Some(Kind::LinkPass)),
            ("x.link.fail.cpp", Some(Kind::LinkFail)),
            ("nodiscard.verify.cpp", Some(Kind::Verify)),
            ("version.sh.cpp", Some(Kind::Sh)),
            ("script.sh.py", Some(Kind::Sh)),
            ("header_inclusions.gen.py", Some(Kind::Gen)),
            ("objc.pass.mm", Some(Kind::ObjCxx)),
            ("objc.compile.pass.mm", Some(Kind::ObjCxx)),
            ("array.version.compile.pass.cpp", Some(Kind::CompilePass)),
            ("pair.U_V.pass.cpp", Some(Kind::Pass)),
            ("helper.h", None),
            ("helper.cpp", None),
            ("pass.cpp", None),
            (".hidden.pass.cpp", None),
            ("lit.local.cfg", None),
        ];
        for &(name, want) in cases {
            assert_eq!(Kind::of(name), want, "{name}");
        }
        assert_eq!(Kind::Verify.unsupported(), None);
        assert_eq!(Kind::Pass.unsupported(), None);
    }

    const FEATURES: &[&str] = &["c++26", "linux", "kiln", "has-fconstexpr-steps"];

    #[test]
    fn keywords() {
        let src = "\
//===---===//
// UNSUPPORTED: c++03, c++11 , c++14
// REQUIRES: linux && !c++03
// XFAIL: gcc-13
// ADDITIONAL_COMPILE_FLAGS: -D_LIBCPP_DISABLE_DEPRECATION_WARNINGS -DX=1
// ADDITIONAL_COMPILE_FLAGS(has-fconstexpr-steps): -fconstexpr-steps=12345678
// ADDITIONAL_COMPILE_FLAGS(gcc-style-warnings): -Wno-array-bounds
// FILE_DEPENDENCIES: Inputs/a.txt, b.dat
// MY_RUN: not a run line
int main(int, char**) { return 0; }
";
        let d = Directives::parse(src).unwrap();
        assert_eq!(d.unsupported.len(), 3);
        assert_eq!(d.unsupported[1].0, "c++11");
        assert_eq!(d.requires.len(), 1);
        assert_eq!(d.xfail.len(), 1);
        assert!(!d.has_run);
        assert_eq!(d.file_dependencies, ["Inputs/a.txt", "b.dat"]);
        assert_eq!(
            d.flags_for(FEATURES),
            ["-D_LIBCPP_DISABLE_DEPRECATION_WARNINGS", "-DX=1", "-fconstexpr-steps=12345678"]
        );
        assert_eq!(d.verdict(FEATURES), Ok(None));
    }

    #[test]
    fn verdicts() {
        let v = |src: &str| Directives::parse(src).unwrap().verdict(FEATURES);
        assert_eq!(v("// UNSUPPORTED: c++03, c++26"), Err("UNSUPPORTED: c++26".into()));
        assert_eq!(v("// REQUIRES: c++03"), Err("REQUIRES: c++03".into()));
        assert_eq!(
            v("// REQUIRES: linux\n// REQUIRES: locale.fr_FR.UTF-8"),
            Err("REQUIRES: locale.fr_FR.UTF-8".into())
        );
        assert_eq!(v("// XFAIL: *"), Ok(Some("*".into())));
        assert_eq!(v("// XFAIL: kiln"), Ok(Some("kiln".into())));
        assert_eq!(v("// XFAIL: clang"), Ok(None));
        // UNSUPPORTED wins over XFAIL.
        assert_eq!(v("// XFAIL: *\n// UNSUPPORTED: kiln"), Err("UNSUPPORTED: kiln".into()));
        assert!(v("// RUN: %{build}").is_ok());
        assert!(Directives::parse("// RUN: %{build}").unwrap().has_run);
        // Keywords after END. are ignored.
        assert_eq!(v("// END.\n// UNSUPPORTED: kiln"), Ok(None));
        // A malformed expression is an error.
        assert!(Directives::parse("// UNSUPPORTED: c++03 &&").is_err());
        assert!(Directives::parse("// ADDITIONAL_COMPILE_FLAGS(foo -x").is_err());
    }

    #[test]
    fn local_cfg() {
        let ok = "# comment\nif \"c++experimental\" not in config.available_features:\n    config.unsupported = True\n\nif \"no-localization\" in config.available_features:\n    config.unsupported = True\n";
        let c = LocalCfg::parse(ok);
        assert_eq!(
            c,
            LocalCfg::Conditions(vec![
                ("c++experimental".into(), true),
                ("no-localization".into(), false)
            ])
        );
        assert_eq!(c.unsupported(&["c++experimental"]), None);
        assert_eq!(c.unsupported(&[]), Some("lit.local.cfg: missing c++experimental".into()));
        assert_eq!(
            c.unsupported(&["c++experimental", "no-localization"]),
            Some("lit.local.cfg: no-localization".into())
        );
        assert_eq!(LocalCfg::parse("config.substitutions.append(('%{x}', 'y'))"), LocalCfg::Opaque);
        assert_eq!(LocalCfg::parse("if \"x\" in config.available_features:\n"), LocalCfg::Opaque);
        assert_eq!(LocalCfg::Opaque.unsupported(&[]), Some("driver: lit.local.cfg".into()));
    }
}
