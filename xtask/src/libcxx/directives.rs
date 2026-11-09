//! Test classification by file name and the lit keywords of a libc++ test
//! (`spec/libcxx.md` "Classification" and "Directives").

use super::boolexpr::{Expr, Piece};
use super::features::DIALECT_FLAG_FEATURES;

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
    /// The `RUN:` commands in order, a line ending in `\\` joined with the
    /// next `RUN:` line (lit's `ParserKind.COMMAND`).
    pub run: Vec<String>,
    /// `ALLOW_RETRIES: N`: a failing run is retried up to `N` more times.
    pub allow_retries: u32,
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
    "ALLOW_RETRIES:",
    "RUN:",
    "END.",
];

impl Directives {
    /// Parses the keywords of `text`. A malformed boolean expression or an
    /// unterminated conditional keyword is an error (the test is an ERROR).
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut d = Self::default();
        let mut retries_seen = false;
        // A `RUN:` command continued with `\\` on the next `RUN:` line.
        let mut run_cont: Option<String> = None;
        'lines: for line in text.lines() {
            let mut search = 0;
            while let Some((at, kw)) = find_keyword(line, search) {
                let rest = &line[at + kw.len()..];
                search = line.len();
                match kw {
                    "END." => break 'lines,
                    "RUN:" => {
                        d.has_run = true;
                        let piece = rest.trim();
                        let mut cmd = run_cont.take().unwrap_or_default();
                        if let Some(head) = piece.strip_suffix('\\') {
                            cmd.push_str(head);
                            run_cont = Some(cmd);
                        } else {
                            cmd.push_str(piece);
                            d.run.push(cmd);
                        }
                    }
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
                    // lit's `ParserKind.INTEGER`, at most once per test.
                    "ALLOW_RETRIES:" => {
                        if retries_seen {
                            return Err("more than one `ALLOW_RETRIES:` line".to_owned());
                        }
                        retries_seen = true;
                        d.allow_retries = rest.trim().parse().map_err(|_| {
                            format!("`ALLOW_RETRIES:` wants a number, got `{}`", rest.trim())
                        })?;
                    }
                    _ => unreachable!("keyword {kw} is in KEYWORDS"),
                }
            }
        }
        if let Some(cmd) = run_cont {
            return Err(format!("`RUN:` line ends in a continuation: `{cmd}\\`"));
        }
        Ok(d)
    }

    /// The flags for this configuration: the plain ones, then the
    /// conditional ones whose feature is in `features` or is a dialect
    /// identity kiln implements ([`DIALECT_FLAG_FEATURES`]), in file order
    /// within each group.
    pub fn flags_for(&self, features: &[&str]) -> Vec<String> {
        let mut v = self.compile_flags.clone();
        v.extend(
            self.conditional_flags
                .iter()
                .filter(|(f, _)| {
                    features.contains(&f.as_str()) || DIALECT_FLAG_FEATURES.contains(&f.as_str())
                })
                .map(|(_, flag)| flag.clone()),
        );
        v
    }

    /// Whether a `REQUIRES:` expression names the feature `name` (as a
    /// plain identifier anywhere in it): `REQUIRES: long_tests` marks a
    /// long test.
    pub fn requires_feature(&self, name: &str) -> bool {
        fn names(e: &Expr, name: &str) -> bool {
            match e {
                Expr::True => false,
                Expr::Ident(p) => matches!(p.as_slice(), [Piece::Lit(l)] if l == name),
                Expr::Not(a) => names(a, name),
                Expr::And(a, b) | Expr::Or(a, b) => names(a, name) || names(b, name),
            }
        }
        self.requires.iter().any(|(_, e)| names(e, name))
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

/// Whether `text` has a clang-verify directive (`expected-error`,
/// `-warning`, `-note` or `-remark`, optionally `-re`, then anything up to
/// `{{` on the same line), the default `-verify` prefix only. Such a
/// `.compile.fail.cpp` test is run with `-verify` (spec/libcxx.md
/// ".compile.fail.cpp").
pub fn has_verify_directives(text: &str) -> bool {
    text.lines().any(|line| {
        let mut start = 0;
        while let Some(i) = line[start..].find("expected-").map(|i| i + start) {
            start = i + "expected-".len();
            if line[..i].chars().next_back().is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                continue;
            }
            let rest = &line[start..];
            let level =
                ["error", "warning", "note", "remark"].iter().find(|l| rest.starts_with(*l));
            if level.is_some_and(|l| rest[l.len()..].contains("{{")) {
                return true;
            }
        }
        false
    })
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

/// A `lit.local.cfg` in the subset this driver understands
/// (spec/libcxx.md "lit.local.cfg"). The statements are:
/// - `import <modules>` (ignored);
/// - `if "<feature>" [not] in config.available_features:` followed by
///   `config.unsupported = True`;
/// - `config.available_features.add("<feature>")`;
/// - `config.substitutions.append(("%{name}", <value>))`, where a value
///   that is not a string literal (`shlex.quote(sys.executable)`) is
///   recorded as unresolved: only a test whose `RUN:` lines use it is
///   UNSUPPORTED;
/// - `<identifier> = <expression>` (a Python local; it can only matter
///   through one of the statements above);
/// - `config.load_from_path(localConfig, lit_config)`, the `libcxx/` idiom
///   that loads the `lit.local.cfg` of the same directory under `std/`
///   ([`LocalCfg::mirror_std`], resolved by the caller).
///
/// Anything else makes the whole directory UNSUPPORTED (it cannot be
/// evaluated without lit).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalCfg {
    /// The directory is unsupported when any `(feature, negated)` holds.
    pub conditions: Vec<(String, bool)>,
    /// Features the file adds for the tests below it.
    pub add_features: Vec<String>,
    /// `(name, value)` substitutions; `None` when the value is an
    /// expression the driver cannot evaluate.
    pub substitutions: Vec<(String, Option<String>)>,
    /// The file loads its `std/` counterpart's configuration.
    pub mirror_std: bool,
    /// The file has a statement outside the subset.
    pub opaque: bool,
}

impl LocalCfg {
    /// A configuration the driver cannot evaluate.
    pub fn opaque() -> Self {
        Self { opaque: true, ..Self::default() }
    }

    pub fn parse(text: &str) -> Self {
        let mut c = Self::default();
        let mut pending: Option<(String, bool)> = None;
        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if let Some(cond) = pending.take() {
                if line == "config.unsupported = True" {
                    c.conditions.push(cond);
                    continue;
                }
                return Self::opaque();
            }
            if line.starts_with("import ") {
                continue;
            }
            if line == "config.load_from_path(localConfig, lit_config)" {
                c.mirror_std = true;
                continue;
            }
            if let Some(arg) = call_arg(line, "config.available_features.add") {
                let Some(f) = string_literal(arg) else { return Self::opaque() };
                c.add_features.push(f.to_owned());
                continue;
            }
            if let Some(arg) = call_arg(line, "config.substitutions.append") {
                let Some((name, value)) =
                    arg.strip_prefix('(').and_then(|a| a.strip_suffix(')')).and_then(|a| {
                        let (n, v) = a.split_once(',')?;
                        Some((string_literal(n.trim())?, v.trim()))
                    })
                else {
                    return Self::opaque();
                };
                c.substitutions.push((name.to_owned(), string_literal(value).map(str::to_owned)));
                continue;
            }
            if let Some(body) = line.strip_prefix("if ").and_then(|l| l.strip_suffix(':')) {
                let (feature, negated) =
                    if let Some(f) = body.strip_suffix(" not in config.available_features") {
                        (f, true)
                    } else if let Some(f) = body.strip_suffix(" in config.available_features") {
                        (f, false)
                    } else {
                        return Self::opaque();
                    };
                let Some(feature) = string_literal(feature.trim()) else {
                    return Self::opaque();
                };
                pending = Some((feature.to_owned(), negated));
                continue;
            }
            // A local binding: `name = expression`.
            if let Some((lhs, _)) = line.split_once('=')
                && !lhs.ends_with(['=', '!', '<', '>'])
                && !lhs.trim().is_empty()
                && lhs.trim().bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            {
                continue;
            }
            return Self::opaque();
        }
        if pending.is_some() { Self::opaque() } else { c }
    }

    /// Adds the `std/` counterpart's configuration (`mirror`, parsed) for a
    /// [`LocalCfg::mirror_std`] file: its conditions and features apply
    /// here too. A missing counterpart (`None`) is opaque.
    pub fn resolve_mirror(&mut self, mirror: Option<Self>) {
        if !self.mirror_std {
            return;
        }
        self.mirror_std = false;
        match mirror {
            Some(m) if !m.opaque && !m.mirror_std => {
                self.conditions.extend(m.conditions);
                self.add_features.extend(m.add_features);
                self.substitutions.extend(m.substitutions);
            }
            _ => self.opaque = true,
        }
    }

    /// `Some(reason)` when the configuration makes the directory unsupported.
    pub fn unsupported(&self, features: &[&str]) -> Option<String> {
        if self.opaque || self.mirror_std {
            return Some("driver: lit.local.cfg".to_owned());
        }
        self.conditions.iter().find(|(f, negated)| features.contains(&f.as_str()) != *negated).map(
            |(f, negated)| format!("lit.local.cfg: {}{f}", if *negated { "missing " } else { "" }),
        )
    }
}

/// The argument text of `callee(<arg>)` when `line` is exactly that call.
fn call_arg<'a>(line: &'a str, callee: &str) -> Option<&'a str> {
    line.strip_prefix(callee)?.strip_prefix('(')?.strip_suffix(')').map(str::trim)
}

/// The contents of a Python string literal without escapes (`"x"` or
/// `'x'`).
fn string_literal(s: &str) -> Option<&str> {
    let q = s.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let inner = s.strip_prefix(q)?.strip_suffix(q)?;
    (!inner.contains(q) && !inner.contains('\\')).then_some(inner)
}

#[cfg(test)]
mod tests {
    #[test]
    fn verify_directives() {
        use super::has_verify_directives as has;
        assert!(has("int x = y; // expected-error {{undeclared identifier 'y'}}"));
        assert!(has("// expected-error-re@+1 2 {{.*}}"));
        assert!(has("f(); // expected-warning@-1{{unused}}"));
        assert!(!has("// expected-no-diagnostics"));
        assert!(!has("// unexpected-error {{x}}"));
        assert!(!has("// expected-error without braces"));
        assert!(!has("// This test checks that X fails to compile."));
    }

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
    fn clang_dialect_flags() {
        // libc++ 22 `simd_unary.pass.cpp`: kiln speaks clang's dialect, so
        // the `(clang)` flags apply; other identities' do not, and the
        // identity stays out of XFAIL/UNSUPPORTED (WP-980).
        let src = "\
// XFAIL: clang
// UNSUPPORTED: clang-19
// ADDITIONAL_COMPILE_FLAGS(clang): -flax-vector-conversions=integer
// ADDITIONAL_COMPILE_FLAGS(apple-clang): -Wapple
// ADDITIONAL_COMPILE_FLAGS(clang-19): -Wnineteen
// ADDITIONAL_COMPILE_FLAGS(clang-modules-build): -fno-modules
// ADDITIONAL_COMPILE_FLAGS(gcc): -Wno-bool-compare
";
        let d = Directives::parse(src).unwrap();
        assert_eq!(d.flags_for(FEATURES), ["-flax-vector-conversions=integer"]);
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
    fn allow_retries_and_long_tests() {
        let d = Directives::parse("// REQUIRES: long_tests\n// ALLOW_RETRIES: 2\n").unwrap();
        assert_eq!(d.allow_retries, 2);
        assert!(d.requires_feature("long_tests"));
        assert!(!d.requires_feature("long"));
        let d = Directives::parse("// REQUIRES: linux && (long_tests || c++26)").unwrap();
        assert!(d.requires_feature("long_tests"));
        assert_eq!(d.allow_retries, 0);
        // `UNSUPPORTED: long_tests` and a regex do not mark a test long.
        let d = Directives::parse("// UNSUPPORTED: long_tests\n// REQUIRES: {{long_.*}}").unwrap();
        assert!(!d.requires_feature("long_tests"));
        // lit: an integer, at most once.
        assert!(Directives::parse("// ALLOW_RETRIES: two").is_err());
        assert!(Directives::parse("// ALLOW_RETRIES: 2\n// ALLOW_RETRIES: 3").is_err());
        assert!(Directives::parse("// END.\n// ALLOW_RETRIES: x").is_ok());
    }

    #[test]
    fn local_cfg() {
        let ok = "# comment\nif \"c++experimental\" not in config.available_features:\n    config.unsupported = True\n\nif \"no-localization\" in config.available_features:\n    config.unsupported = True\n";
        let c = LocalCfg::parse(ok);
        assert_eq!(
            c.conditions,
            vec![("c++experimental".into(), true), ("no-localization".into(), false)]
        );
        assert!(!c.opaque);
        assert_eq!(c.unsupported(&["c++experimental"]), None);
        assert_eq!(c.unsupported(&[]), Some("lit.local.cfg: missing c++experimental".into()));
        assert_eq!(
            c.unsupported(&["c++experimental", "no-localization"]),
            Some("lit.local.cfg: no-localization".into())
        );
        assert!(LocalCfg::parse("if \"x\" in config.available_features:\n").opaque);
        assert!(LocalCfg::parse("config.name = 'x'").opaque);
        assert!(LocalCfg::parse("if x == y:\n  pass").opaque);
        assert_eq!(LocalCfg::opaque().unsupported(&[]), Some("driver: lit.local.cfg".into()));
    }

    /// libc++ 19's `libcxx/lit.local.cfg` and selftest configurations
    /// (spec/libcxx.md "lit.local.cfg").
    #[test]
    fn local_cfg_substitutions_and_features() {
        let c = LocalCfg::parse(
            "import shlex\nimport sys\n\
             config.substitutions.append((\"%{python}\", shlex.quote(sys.executable)))\n",
        );
        assert!(!c.opaque);
        assert_eq!(c.substitutions, vec![("%{python}".into(), None)]);
        assert_eq!(c.unsupported(&[]), None);
        let c = LocalCfg::parse("config.substitutions.append(('%{x}', 'y z'))");
        assert_eq!(c.substitutions, vec![("%{x}".into(), Some("y z".into()))]);
        let c = LocalCfg::parse("config.available_features.add(\"some-defined-feature\")");
        assert_eq!(c.add_features, vec!["some-defined-feature".to_owned()]);
        let c = LocalCfg::parse(
            "import base64, lit.util, pickle\n\
             base64Encode = lambda s: lit.util.to_string(base64.b64encode(lit.util.to_bytes(s)))\n\
             escapedSubstitutions = base64Encode(pickle.dumps(config.substitutions))\n\
             config.substitutions.append((\"%{substitutions}\", escapedSubstitutions))\n",
        );
        assert!(!c.opaque);
        assert_eq!(c.substitutions, vec![("%{substitutions}".into(), None)]);
        assert!(LocalCfg::parse("config.substitutions.append(x)").opaque);
        assert!(LocalCfg::parse("config.available_features.add(name)").opaque);
    }

    /// The `libcxx/` directories that load their `std/` counterpart.
    #[test]
    fn local_cfg_mirror() {
        let text = "import os\n\
            inLibcxx = os.path.join(\"libcxx\", \"test\", \"libcxx\")\n\
            inStd = os.path.join(\"libcxx\", \"test\", \"std\")\n\
            localConfig = os.path.normpath(os.path.realpath(__file__)).replace(inLibcxx, inStd)\n\
            config.load_from_path(localConfig, lit_config)\n";
        let mut c = LocalCfg::parse(text);
        assert!(c.mirror_std && !c.opaque);
        assert_eq!(c.unsupported(&[]), Some("driver: lit.local.cfg".into()));
        let std = LocalCfg::parse(
            "if \"no-localization\" in config.available_features:\n  config.unsupported = True",
        );
        c.resolve_mirror(Some(std));
        assert_eq!(c.unsupported(&[]), None);
        assert_eq!(
            c.unsupported(&["no-localization"]),
            Some("lit.local.cfg: no-localization".into())
        );
        let mut c = LocalCfg::parse(text);
        c.resolve_mirror(None);
        assert!(c.opaque);
    }

    #[test]
    fn run_lines() {
        let d = Directives::parse(
            "// RUN: %{build}\n// RUN: %{cxx} a \\\n// RUN:   b\n//RUN: %{run}  \n",
        )
        .unwrap();
        assert_eq!(d.run, ["%{build}", "%{cxx} a b", "%{run}"]);
        assert!(Directives::parse("// RUN: a \\\n").is_err());
    }
}
