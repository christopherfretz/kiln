//! Bench entries: discovery under the bench root and the `//@` header
//! (`spec/bench.md` "Entries" and "Directives").

use std::fs;
use std::path::{Path, PathBuf};

use super::compare::{ThresholdOverrides, parse_pct};
use crate::util::header::{self, CXX_STDS, DirectiveError, OvenBudget, parse_oven, split_args};

/// What the compiler is asked to do with an entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// `-fsyntax-only` (the default): parse, sema, constant evaluation.
    Syntax,
    /// `-E -o /dev/null`: translation phases 1–4 only.
    Pp,
    /// `-S -emit-llvm -o /dev/null`: the whole frontend including codegen.
    EmitLlvm,
}

impl Mode {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "syntax" => Self::Syntax,
            "pp" => Self::Pp,
            "emit-llvm" => Self::EmitLlvm,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Syntax => "syntax",
            Self::Pp => "pp",
            Self::EmitLlvm => "emit-llvm",
        }
    }

    /// The compiler flags that select this mode. Output that would be
    /// written goes to `/dev/null`, so disk I/O does not enter the timing.
    pub fn flags(self) -> &'static [&'static str] {
        match self {
            Self::Syntax => &["-fsyntax-only"],
            Self::Pp => &["-E", "-o", "/dev/null"],
            Self::EmitLlvm => &["-S", "-emit-llvm", "-o", "/dev/null"],
        }
    }
}

/// The directive header of a bench entry.
#[derive(Clone, Debug, PartialEq)]
pub struct BenchSpec {
    pub mode: Mode,
    /// The `-std=` value, e.g. `c++26`.
    pub std: String,
    pub args: Vec<String>,
    /// Budgets checked against kiln's constexpr report (ignored for other
    /// compilers): at most one unlocated, at most one per location.
    pub oven: Vec<OvenBudget>,
    /// `threshold-*:` overrides of the default regression thresholds.
    pub thresholds: ThresholdOverrides,
}

/// Directives that belong to `cargo xtask test` and mean nothing here.
const TEST_ONLY: &[&str] = &[
    "expect",
    "diag",
    "strict-diags",
    "stdout",
    "stdout-file",
    "exit",
    "run-args",
    "run-stdin",
    "run-stdin-file",
    "env",
    "oracle",
    "skip",
    "xfail",
];

/// The keys a bench entry takes.
const KEYS: &[&str] = &[
    "std",
    "args",
    "mode",
    "oven",
    "threshold-time",
    "threshold-mem",
    "threshold-steps",
    "threshold-bytes",
];

/// Parses an entry's directive header. All errors are collected.
pub fn parse_spec(source: &str) -> Result<BenchSpec, Vec<DirectiveError>> {
    let (directives, mut errors) = header::scan(source);
    let mut mode = None;
    let mut std = None;
    let mut args = Vec::new();
    let mut oven: Vec<(usize, OvenBudget)> = Vec::new();
    let mut thresholds = ThresholdOverrides::default();
    for d in directives {
        let mut err = |m: String| errors.push(DirectiveError { line: d.line, message: m });
        let base = d.key.split('(').next().unwrap_or(d.key).trim();
        if TEST_ONLY.contains(&base) {
            err(format!(
                "`{base}` is a `cargo xtask test` directive; bench entries take {}",
                KEYS.join(", ")
            ));
            continue;
        }
        let Some(value) = d.value else {
            if KEYS.contains(&d.key) {
                err(format!("`{}` requires a value (`{}: ...`)", d.key, d.key));
            } else {
                err(format!("unknown directive `{}`", d.key));
            }
            continue;
        };
        match d.key {
            "mode" => match Mode::parse(value) {
                Some(_) if mode.is_some() => err("duplicate `mode`".into()),
                Some(m) => mode = Some(m),
                None => err(format!("unknown mode `{value}` (syntax|pp|emit-llvm)")),
            },
            "std" => {
                if std.is_some() {
                    err("duplicate `std`".into());
                } else if CXX_STDS.contains(&value) {
                    std = Some(value.to_owned());
                } else {
                    err(format!("unknown std `{value}` (allowed: {})", CXX_STDS.join("|")));
                }
            }
            "args" => match split_args(value) {
                Ok(mut a) => args.append(&mut a),
                Err(e) => err(e),
            },
            "oven" => match parse_oven(value) {
                Ok(b) if oven.iter().any(|(_, o)| o.at == b.at) => err(match b.at {
                    Some(at) => format!("duplicate `oven` for location {at}"),
                    None => "duplicate `oven` without a location".into(),
                }),
                Ok(b) => oven.push((d.line, b)),
                Err(e) => err(e),
            },
            k @ ("threshold-time" | "threshold-mem" | "threshold-steps" | "threshold-bytes") => {
                let slot = match k {
                    "threshold-time" => &mut thresholds.time_pct,
                    "threshold-mem" => &mut thresholds.mem_pct,
                    "threshold-steps" => &mut thresholds.steps_pct,
                    _ => &mut thresholds.bytes_pct,
                };
                match parse_pct(value) {
                    Some(_) if slot.is_some() => err(format!("duplicate `{k}`")),
                    Some(p) => *slot = Some(p),
                    None => err(format!("bad `{k}` value `{value}` (a non-negative percentage)")),
                }
            }
            k => err(format!("unknown directive `{k}`")),
        }
    }
    let mode = mode.unwrap_or(Mode::Syntax);
    if mode == Mode::Pp {
        for (line, _) in &oven {
            errors.push(DirectiveError {
                line: *line,
                message: "`oven` does not apply to `mode: pp` (nothing is evaluated)".into(),
            });
        }
    }
    if !errors.is_empty() {
        errors.sort_by_key(|e| e.line);
        return Err(errors);
    }
    Ok(BenchSpec {
        mode,
        std: std.unwrap_or_else(|| "c++26".to_owned()),
        args,
        oven: oven.into_iter().map(|(_, b)| b).collect(),
        thresholds,
    })
}

/// One bench file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Absolute path.
    pub path: PathBuf,
    /// `<group>/<name>`: the path relative to the bench root, without `.cpp`.
    pub id: String,
    /// The first component of `id`.
    pub group: String,
    /// The rest of `id`: the key inside the group's baseline file.
    pub name: String,
    /// The path handed to the compiler, which runs in the repo root: relative
    /// to the root when the file is under it, else absolute.
    pub arg: String,
}

/// Top-level directories of the bench root that never hold entries.
const RESERVED: &[&str] = &["results", "baselines"];

/// Expands `inputs` (files or directories under `bench_root`) into a sorted,
/// de-duplicated entry list, keeping ids that contain `filter`.
pub fn discover(
    root: &Path,
    bench_root: &Path,
    inputs: &[PathBuf],
    filter: Option<&str>,
) -> Result<Vec<Entry>, String> {
    let bench_root = canonical(bench_root)?;
    let mut files = Vec::new();
    for input in inputs {
        let input = canonical(input)?;
        if input.is_dir() {
            walk(&bench_root, &input, &mut files)
                .map_err(|e| format!("{}: {e}", input.display()))?;
        } else {
            files.push(input);
        }
    }
    let mut out = Vec::new();
    for path in files {
        let rel = path.strip_prefix(&bench_root).map_err(|_| {
            format!("{} is not under the bench root {}", path.display(), bench_root.display())
        })?;
        let rel = rel.with_extension("");
        let parts: Vec<String> =
            rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        if parts.len() < 2 {
            return Err(format!(
                "{}: bench entries live in a group directory (bench/<group>/<name>.cpp)",
                path.display()
            ));
        }
        let arg = match path.strip_prefix(canonical(root)?) {
            Ok(r) => r.to_string_lossy().into_owned(),
            Err(_) => path.to_string_lossy().into_owned(),
        };
        out.push(Entry {
            id: parts.join("/"),
            group: parts[0].clone(),
            name: parts[1..].join("/"),
            arg,
            path,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out.dedup_by(|a, b| a.path == b.path);
    if let Some(f) = filter {
        out.retain(|e| e.id.contains(f));
    }
    Ok(out)
}

fn canonical(p: &Path) -> Result<PathBuf, String> {
    fs::canonicalize(p).map_err(|e| format!("{}: {e}", p.display()))
}

fn walk(bench_root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for e in entries {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let path = e.path();
        if e.file_type()?.is_dir() {
            let reserved = dir == bench_root && RESERVED.contains(&name.as_ref());
            if !reserved && name != "Inputs" {
                walk(bench_root, &path, out)?;
            }
        } else if path.extension().is_some_and(|x| x == "cpp") {
            out.push(path);
        }
    }
    Ok(())
}

/// FNV-1a 64 of the entry's bytes, so a baseline can tell that the entry it
/// was recorded for has since changed.
pub fn source_hash(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("fnv1a64:{h:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn errs(src: &str) -> Vec<String> {
        parse_spec(src).expect_err("expected errors").into_iter().map(|e| e.to_string()).collect()
    }

    #[test]
    fn defaults() {
        let s = parse_spec("int x;\n").unwrap();
        assert_eq!(
            s,
            BenchSpec {
                mode: Mode::Syntax,
                std: "c++26".into(),
                args: vec![],
                oven: vec![],
                thresholds: ThresholdOverrides::default(),
            }
        );
    }

    #[test]
    fn mode_std_args_oven() {
        let s = parse_spec(concat!(
            "// a comment\n",
            "//@ mode: emit-llvm\n",
            "//@ std: c++20\n",
            "//@ args: -stdlib=libc++ '-DX=a b'\n",
            "//@ args: -O1\n",
            "//@ oven: steps<=1000\n",
            "//@ oven: 9:1 calls<=3\n",
            "//@ threshold-time: 25\n",
            "//@ threshold-bytes: 0.5\n",
            "int x;\n",
        ))
        .unwrap();
        assert_eq!(s.mode, Mode::EmitLlvm);
        assert_eq!(s.std, "c++20");
        assert_eq!(s.args, ["-stdlib=libc++", "-DX=a b", "-O1"]);
        assert_eq!(
            s.oven,
            [parse_oven("steps<=1000").unwrap(), parse_oven("9:1 calls<=3").unwrap()]
        );
        assert_eq!(
            s.thresholds,
            ThresholdOverrides {
                time_pct: Some(25.0),
                bytes_pct: Some(0.5),
                ..ThresholdOverrides::default()
            }
        );
        let s = parse_spec("//@ threshold-mem: 10\n//@ threshold-steps: 1\n").unwrap();
        assert_eq!((s.thresholds.mem_pct, s.thresholds.steps_pct), (Some(10.0), Some(1.0)));
        for (m, flags) in [
            ("syntax", &["-fsyntax-only"][..]),
            ("pp", &["-E", "-o", "/dev/null"]),
            ("emit-llvm", &["-S", "-emit-llvm", "-o", "/dev/null"]),
        ] {
            let s = parse_spec(&format!("//@ mode: {m}\n")).unwrap();
            assert_eq!(s.mode.as_str(), m);
            assert_eq!(s.mode.flags(), flags);
        }
    }

    #[test]
    fn rejects_bad_headers() {
        assert_eq!(errs("//@ mode: link\n"), ["line 1: unknown mode `link` (syntax|pp|emit-llvm)"]);
        assert_eq!(errs("//@ mode: pp\n//@ mode: pp\n"), ["line 2: duplicate `mode`"]);
        assert_eq!(errs("//@ mode\n"), ["line 1: `mode` requires a value (`mode: ...`)"]);
        assert_eq!(errs("//@ std: c17\n").len(), 1);
        assert!(errs("//@ expect: run\n")[0].contains("`cargo xtask test` directive"));
        assert!(errs("//@ skip(kiln): x\n")[0].contains("`cargo xtask test` directive"));
        assert!(errs("//@ strict-diags\n")[0].contains("`cargo xtask test` directive"));
        assert_eq!(errs("//@ frob: 1\n"), ["line 1: unknown directive `frob`"]);
        assert!(errs("int x;\n//@ mode: pp\n")[0].contains("after the header"));
        assert!(errs("//@ oven: time<=3\n")[0].contains("unknown budget"));
        assert!(errs("//@ oven: steps<=1\n//@ oven: bytes<=1\n")[0].contains("duplicate `oven`"));
        assert_eq!(
            errs("//@ mode: pp\n//@ oven: steps<=1\n"),
            ["line 2: `oven` does not apply to `mode: pp` (nothing is evaluated)"]
        );
        assert_eq!(
            errs("//@ threshold-time: -1\n"),
            ["line 1: bad `threshold-time` value `-1` (a non-negative percentage)"]
        );
        assert_eq!(
            errs("//@ threshold-mem: 1\n//@ threshold-mem: 2\n"),
            ["line 2: duplicate `threshold-mem`"]
        );
        assert_eq!(
            errs("//@ threshold-mem\n"),
            ["line 1: `threshold-mem` requires a value (`threshold-mem: ...`)"]
        );
        assert!(errs("//@ run-args: x\n")[0].contains("`cargo xtask test` directive"));
    }

    #[test]
    fn hashes_are_stable() {
        assert_eq!(source_hash(b""), "fnv1a64:cbf29ce484222325");
        assert_eq!(source_hash(b"a"), "fnv1a64:af63dc4c8601ec8c");
    }
}
