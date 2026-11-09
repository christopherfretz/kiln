//! The `RUN:` lines of a `.sh.cpp` test (spec/libcxx.md "RUN lines"):
//! libc++'s `format.py` substitutions expanded the way lit expands them,
//! each command then run by `bash` with lit's `pipefail` and a `not`
//! helper. The `MODULE_DEPENDENCIES` build (`--precompile`) is the
//! runner's (`run::module_build`). `%{python}` and the install-layout
//! substitutions (`%{lib-dir}`, `%{include-dir}`, `%{gdb}`) are out of
//! scope: [`check`] names which one a test needs.

use std::path::Path;

use super::run::VERIFY_FLAGS;

/// The `%{name}` substitutions the driver defines (`format.py` and
/// `params.py` of libc++ 19). `%{exec}` is empty: tests run locally with
/// the scratch directory as the working directory, as lit's `run.py`
/// executor does with `--execdir %T`.
const BUILTIN: &[&str] = &[
    "%{build}",
    "%{compile_flags}",
    "%{cxx}",
    "%{exec}",
    "%{flags}",
    "%{link_flags}",
    "%{run}",
    "%{verify}",
];

/// lit's `recursiveExpansionLimit` in libc++'s configuration.
const EXPANSION_LIMIT: usize = 10;

/// Why the driver cannot run `run` (a test's `RUN:` commands), if it
/// cannot: the first `%{name}` that is neither built in nor a resolved
/// `lit.local.cfg` substitution. `cfg` holds the `lit.local.cfg`
/// substitutions, `None` for a value the driver cannot evaluate.
pub fn check(run: &[String], cfg: &[(String, Option<String>)]) -> Result<(), String> {
    for line in run {
        for name in names(line) {
            if BUILTIN.contains(&name) {
                continue;
            }
            match cfg.iter().rev().find(|(n, _)| n == name) {
                Some((_, Some(_))) => {}
                Some((_, None)) => {
                    return Err(format!("lit.local.cfg: unresolved substitution {name}"));
                }
                None => return Err(format!("driver: substitution {name}")),
            }
        }
    }
    Ok(())
}

/// The `%{name}` references in `line`.
fn names(line: &str) -> impl Iterator<Item = &str> {
    line.match_indices("%{").filter_map(|(at, _)| {
        let end = line[at..].find('}')?;
        Some(&line[at..=at + end])
    })
}

/// The values a test's commands are expanded with.
pub struct Substitutions {
    named: Vec<(String, String)>,
    source: String,
    source_dir: String,
    temp_dir: String,
}

impl Substitutions {
    /// `cxx` is the compiler, `compile_flags` the base flags followed by
    /// the test's `ADDITIONAL_COMPILE_FLAGS`, `cfg` the resolved
    /// `lit.local.cfg` substitutions. `%T` is `scratch`, `%t` is
    /// `scratch/t`.
    pub fn new(
        cxx: &Path,
        compile_flags: &[String],
        link_flags: &[String],
        cfg: &[(String, String)],
        src: &Path,
        scratch: &Path,
    ) -> Self {
        let mut verify = String::from("%{cxx} %s %{flags} %{compile_flags}");
        for f in VERIFY_FLAGS {
            verify.push(' ');
            verify.push_str(f);
        }
        let mut named: Vec<(String, String)> = cfg.to_vec();
        named.extend(
            [
                ("%{cxx}", quote(&cxx.display().to_string())),
                ("%{flags}", String::new()),
                ("%{compile_flags}", join(compile_flags)),
                ("%{link_flags}", join(link_flags)),
                ("%{exec}", String::new()),
                ("%{build}", "%{cxx} %s %{flags} %{compile_flags} %{link_flags} -o %t.exe".into()),
                ("%{run}", "%{exec} %t.exe".into()),
                ("%{verify}", verify),
            ]
            .map(|(n, v)| (n.to_owned(), v)),
        );
        Self {
            named,
            source: quote(&src.display().to_string()),
            source_dir: quote(&src.parent().unwrap_or(Path::new(".")).display().to_string()),
            temp_dir: scratch.display().to_string(),
        }
    }

    /// `line` with every substitution applied: `%{name}` until nothing
    /// changes (at most [`EXPANSION_LIMIT`] rounds), then `%s`, `%S`, `%p`,
    /// `%t`, `%T` and `%%` in one pass, so a `%%` never forms a new
    /// reference. `Err` when the expansion does not converge.
    pub fn expand(&self, line: &str) -> Result<String, String> {
        let mut cur = line.to_owned();
        let mut converged = false;
        for _ in 0..EXPANSION_LIMIT {
            let mut next = String::with_capacity(cur.len());
            let mut rest = cur.as_str();
            while let Some(at) = rest.find("%{") {
                next.push_str(&rest[..at]);
                let tail = &rest[at..];
                let hit = tail.find('}').and_then(|end| {
                    let name = &tail[..=end];
                    self.named.iter().rev().find(|(n, _)| n == name).map(|(_, v)| (end, v))
                });
                if let Some((end, v)) = hit {
                    next.push_str(v);
                    rest = &tail[end + 1..];
                } else {
                    next.push_str("%{");
                    rest = &tail[2..];
                }
            }
            next.push_str(rest);
            if next == cur {
                converged = true;
                break;
            }
            cur = next;
        }
        if !converged {
            return Err(format!("RUN line `{line}` does not expand in {EXPANSION_LIMIT} rounds"));
        }
        let mut out = String::with_capacity(cur.len());
        let mut chars = cur.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '%' {
                out.push(c);
                continue;
            }
            match chars.peek() {
                Some('s') => out.push_str(&self.source),
                Some('S' | 'p') => out.push_str(&self.source_dir),
                Some('t') => {
                    out.push_str(&quote(&self.temp_dir));
                    out.push_str("/t");
                }
                Some('T') => out.push_str(&quote(&self.temp_dir)),
                Some('%') => out.push('%'),
                _ => {
                    out.push('%');
                    continue;
                }
            }
            chars.next();
        }
        Ok(out)
    }
}

/// The stage a `RUN:` command stands for, from its unexpanded text: a
/// compiler invocation with a source (`%s`, `-c`, `-fsyntax-only`) is a
/// compile, one with only objects a link, anything else runs the test.
/// `verify` marks a `%{verify}` compile (spec/libcxx.md ".verify.cpp").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Compile { verify: bool },
    Link,
    Run,
}

pub fn line_kind(line: &str) -> LineKind {
    let words: Vec<&str> = line.split_whitespace().collect();
    if line.contains("%{verify}") {
        return LineKind::Compile { verify: true };
    }
    if line.contains("%{build}") {
        return LineKind::Compile { verify: false };
    }
    if words.first() == Some(&"%{cxx}") {
        return if words.iter().any(|w| matches!(*w, "%s" | "-c" | "-fsyntax-only")) {
            LineKind::Compile { verify: false }
        } else {
            LineKind::Link
        };
    }
    LineKind::Run
}

/// The `bash` program for one expanded command: lit's internal shell
/// runs pipelines with `pipefail` and provides `not`. Output that the
/// command does not redirect goes to stderr, where the harness reads a
/// failing step's detail.
pub fn bash_program(expanded: &str) -> String {
    format!("exec 1>&2\nset -o pipefail\nnot() {{ ! \"$@\"; }}\n{expanded}\n")
}

fn join(words: &[String]) -> String {
    words.iter().map(|w| quote(w)).collect::<Vec<_>>().join(" ")
}

/// `w` as one shell word: unchanged when it has no shell metacharacter.
pub fn quote(w: &str) -> String {
    let plain = !w.is_empty()
        && w.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_=+./:,@%^".contains(&b));
    if plain { w.to_owned() } else { format!("'{}'", w.replace('\'', r"'\''")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subst() -> Substitutions {
        Substitutions::new(
            Path::new("/k/kiln++"),
            &["-std=c++26".into(), "-I".into(), "%t.1".into()],
            &["-lc++abi".into()],
            &[("%{x}".into(), "%{cxx} -v".into())],
            Path::new("/src/a.sh.cpp"),
            Path::new("/s/t3"),
        )
    }

    /// libc++ `format.py`'s `%{build}`, `%{run}` and `%{verify}`; lit's
    /// `%t`/`%T`/`%s`/`%S`/`%%`.
    #[test]
    fn expansion() {
        let s = subst();
        assert_eq!(
            s.expand("%{build}").unwrap(),
            "/k/kiln++ /src/a.sh.cpp  -std=c++26 -I /s/t3/t.1 -lc++abi -o /s/t3/t.exe"
        );
        assert_eq!(s.expand("%{run} 1").unwrap(), " /s/t3/t.exe 1");
        assert_eq!(s.expand("echo %T %S 100%% %d").unwrap(), "echo /s/t3 /src 100% %d");
        assert_eq!(s.expand("%{x}").unwrap(), "/k/kiln++ -v");
        assert!(s.expand("%{verify}").unwrap().ends_with("-Xclang -verify-ignore-unexpected=note"));
        assert_eq!(s.expand("%%{cxx}").unwrap(), "%/k/kiln++");
        let looping = Substitutions::new(
            Path::new("c"),
            &[],
            &[],
            &[("%{a}".into(), "%{a}x".into())],
            Path::new("s"),
            Path::new("t"),
        );
        assert!(looping.expand("%{a}").is_err());
    }

    #[test]
    fn checks() {
        let run = |l: &str| vec![l.to_owned()];
        assert_eq!(check(&run("%{build}"), &[]), Ok(()));
        assert_eq!(
            check(&run("%{python} x"), &[("%{python}".into(), None)]),
            Err("lit.local.cfg: unresolved substitution %{python}".into())
        );
        assert_eq!(check(&run("%{x}"), &[("%{x}".into(), Some("y".into()))]), Ok(()));
        assert_eq!(
            check(&run("stat \"%{lib-dir}/libc++.a\""), &[]),
            Err("driver: substitution %{lib-dir}".into())
        );
    }

    #[test]
    fn line_kinds() {
        assert_eq!(line_kind("%{build} -O2"), LineKind::Compile { verify: false });
        assert_eq!(line_kind("%{verify} -DX"), LineKind::Compile { verify: true });
        assert_eq!(
            line_kind("%{cxx} %{flags} %{compile_flags} -c %s -o %t.o"),
            LineKind::Compile { verify: false }
        );
        assert_eq!(line_kind("%{cxx} %t.a.o %t.b.o %{link_flags} -o %t.exe"), LineKind::Link);
        assert_eq!(line_kind("%{exec} %t.exe > %t.actual"), LineKind::Run);
        assert_eq!(line_kind("echo \"%{compile_flags}\" | grep -e x"), LineKind::Run);
    }

    #[test]
    fn quoting() {
        assert_eq!(quote("/a/b-c.o"), "/a/b-c.o");
        assert_eq!(quote("a b"), "'a b'");
        assert_eq!(quote("it's"), r"'it'\''s'");
        assert_eq!(quote(""), "''");
    }
}
