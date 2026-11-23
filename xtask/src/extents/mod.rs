//! `cargo xtask extents`: the expression-extent differential
//! (spec/harness.md "Expression extents"). Every expression kiln dumps with
//! `-fkiln-dump-expr-extents` (spec/ast.md §21.4) is compared with the
//! source range clang's `-ast-dump=json` gives the same node: a written
//! extent kiln states must be clang's, a `None` is counted per kind, and
//! the per-unit `None` counts may not grow past the checked-in baseline.

mod clang;
mod json;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::test::Shard;
use crate::test::directives::{self, Expect, Lang};
use crate::util::CompilerKind;
use crate::util::process;

const USAGE: &str = "\
usage: cargo xtask extents [options] [paths...]

Compares kiln's written extent of every expression (-fkiln-dump-expr-extents)
with clang's source range of the same node (spec/harness.md \"Expression
extents\"). The units are the test files under [paths...] (default:
tests/conformance) both front ends can take as they are, or with --compdb
the entries of a compilation database.

options:
  --kiln <kiln++>        the kiln to test (default: build it)
  --clang <clang++>      the clang to compare with (default: clang++-19)
  --kinds <file>         the kinds table (default: tests/extents/kinds.txt)
  --baseline <file>      the None baseline (default: tests/extents/baseline.txt)
  --update-baseline      rewrite the baseline from this run (not with --shard)
  --shard K/N            only the units of shard K of N
  --compdb <file>        the units of a compilation database (a report: no
                         baseline gate)
  --filter <substr>      only the units whose path contains <substr>
  -j, --jobs <n>         units run at once (default: the CPUs)
  --nones                list every None: `none <file>:<offset> <Kind> \"<text>\"`
                         (`none - <Kind>` without a file location)
  -v, --verbose          list every unit and its verdict
";

/// The default inputs of a run (stage 1, the gate of `ci`).
pub const DEFAULT_INPUTS: [&str; 1] = ["tests/conformance"];
const KINDS: &str = "tests/extents/kinds.txt";
const BASELINE: &str = "tests/extents/baseline.txt";
/// The time either front end may take on one unit.
const UNIT_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    pub kiln: Option<PathBuf>,
    pub clang: Option<PathBuf>,
    pub kinds: Option<PathBuf>,
    pub baseline: Option<PathBuf>,
    pub update_baseline: bool,
    pub shard: Option<Shard>,
    pub compdb: Option<PathBuf>,
    pub filter: Option<String>,
    pub jobs: Option<usize>,
    pub nones: bool,
    pub verbose: bool,
    pub paths: Vec<PathBuf>,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<Options>, String> {
    let mut o = Options::default();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        let (flag, inline) = match a.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_owned(), Some(v.to_owned())),
            _ => (a.clone(), None),
        };
        let mut value = |name: &str| -> Result<String, String> {
            inline.clone().or_else(|| it.next()).ok_or_else(|| format!("{name} needs a value"))
        };
        match flag.as_str() {
            "-h" | "--help" => return Ok(None),
            "--kiln" => o.kiln = Some(value("--kiln")?.into()),
            "--clang" => o.clang = Some(value("--clang")?.into()),
            "--kinds" => o.kinds = Some(value("--kinds")?.into()),
            "--baseline" => o.baseline = Some(value("--baseline")?.into()),
            "--update-baseline" => o.update_baseline = true,
            "--shard" => o.shard = Some(Shard::parse(&value("--shard")?)?),
            "--compdb" => o.compdb = Some(value("--compdb")?.into()),
            "--filter" => o.filter = Some(value("--filter")?),
            "-j" | "--jobs" => {
                let v = value("--jobs")?;
                o.jobs = Some(
                    v.parse().ok().filter(|&n| n > 0).ok_or(format!("--jobs {v}: not a count"))?,
                );
            }
            "--nones" => o.nones = true,
            "-v" | "--verbose" => o.verbose = true,
            f if f.starts_with('-') => return Err(format!("unknown option `{f}`")),
            _ => o.paths.push(a.into()),
        }
    }
    if o.update_baseline && (o.shard.is_some() || o.compdb.is_some() || o.filter.is_some()) {
        return Err("--update-baseline needs the whole default run (no --shard, --compdb or \
                    --filter)"
            .to_owned());
    }
    Ok(Some(o))
}

pub fn main(args: impl IntoIterator<Item = String>) -> ExitCode {
    let opts = match parse_args(args) {
        Ok(Some(o)) => o,
        Ok(None) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("xtask extents: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let root = crate::util::repo_root();
    let kiln = match &opts.kiln {
        Some(k) => k.clone(),
        None => match crate::util::target_dir(&root)
            .and_then(|t| crate::util::build_kiln(&root, &t, "extents", false))
        {
            Ok(k) => k,
            Err(e) => {
                eprintln!("xtask extents: {e}");
                return ExitCode::from(2);
            }
        },
    };
    match execute(&root, &kiln, &opts) {
        Ok(r) => {
            print!("{}", r.text);
            if r.failed_units > 0 { ExitCode::FAILURE } else { ExitCode::SUCCESS }
        }
        Err(e) => {
            eprintln!("xtask extents: {e}");
            ExitCode::from(2)
        }
    }
}

/// One `ci` run (stage 1 over [`DEFAULT_INPUTS`], gated by the baseline).
pub fn ci(kiln: &Path, shard: Option<Shard>) -> crate::test::CorpusOutcome {
    let root = crate::util::repo_root();
    let opts = Options { kiln: Some(kiln.to_path_buf()), shard, ..Options::default() };
    let start = Instant::now();
    match execute(&root, kiln, &opts) {
        Ok(r) => {
            // Only the problems: the counts are in the outcome's line.
            for l in r.text.lines().filter(|l| is_problem_line(l)) {
                eprintln!("xtask ci: extents: {l}");
            }
            eprintln!("xtask ci: extents: {}", r.text.lines().next().unwrap_or_default());
            crate::test::CorpusOutcome::of_units(
                r.passed,
                r.failed_units,
                r.considered,
                start.elapsed(),
            )
        }
        Err(e) => crate::test::CorpusOutcome::Error(e),
    }
}

fn is_problem_line(l: &str) -> bool {
    ["mismatch ", "failed ", "none-grew ", "baseline "].iter().any(|p| l.starts_with(p))
}

// ---------------------------------------------------------------------------
// The kinds table.

/// `tests/extents/kinds.txt`: how kiln's kinds meet clang's.
#[derive(Clone, Debug, Default)]
pub struct Kinds {
    /// Kiln kind → the clang kinds besides its own name it matches
    /// (`map` lines).
    compat: HashMap<String, Vec<String>>,
    /// Every clang kind of a `map` line.
    mapped: std::collections::HashSet<String>,
    /// Kinds that are not compared (`skip`).
    skip: HashMap<String, String>,
    /// Kinds kiln never states an extent for (`none`).
    none: HashMap<String, String>,
    /// Clang kinds whose operands clang's dump does not print
    /// (`opaque`): kiln's nodes inside one are compared only where clang
    /// has a node at the same begin.
    opaque: HashMap<String, String>,
    /// Places where clang's range is wrong (`quirk <file>:<begin>`):
    /// kiln's nodes beginning there are not compared.
    quirk: HashMap<(String, u32), String>,
}

impl Kinds {
    /// Parses the table: `map <KilnKind> <ClangKind>[,<ClangKind>...]`,
    /// `none <KilnKind> <reason>`, `opaque <ClangKind> <reason>`,
    /// `quirk <file>:<begin> <reason>`,
    /// `skip <KilnKind> <reason>` followed by
    /// one to three lines, indented, of a source that shows the two front
    /// ends build different nodes; `#` comments and blank lines.
    pub fn parse(text: &str) -> Result<Kinds, String> {
        let mut k = Kinds::default();
        let mut lines = text.lines().enumerate().peekable();
        while let Some((i, raw)) = lines.next() {
            let line = raw.trim_end();
            let at = |what: &str| format!("{KINDS}:{}: {what}", i + 1);
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            if line.starts_with(char::is_whitespace) {
                return Err(at("an indented line not under a `skip`"));
            }
            let mut words = line.splitn(3, char::is_whitespace);
            let (verb, kind, rest) = (
                words.next().unwrap_or_default(),
                words.next().unwrap_or_default(),
                words.next().unwrap_or_default().trim(),
            );
            if kind.is_empty() {
                return Err(at("a line needs a kind"));
            }
            match verb {
                "map" => {
                    if rest.is_empty() || rest.contains(char::is_whitespace) {
                        return Err(at("`map <KilnKind> <ClangKind>[,<ClangKind>...]`"));
                    }
                    let list = k.compat.entry(kind.to_owned()).or_default();
                    for c in rest.split(',') {
                        if c.is_empty() || list.iter().any(|x| x == c) {
                            return Err(at(&format!(
                                "`map {kind}`: clang kind `{c}` twice or empty"
                            )));
                        }
                        list.push(c.to_owned());
                        k.mapped.insert(c.to_owned());
                    }
                }
                "quirk" => {
                    let place = kind
                        .rsplit_once(':')
                        .and_then(|(f, b)| Some((f.to_owned(), b.parse::<u32>().ok()?)))
                        .ok_or_else(|| at("`quirk <file>:<begin> <reason>`"))?;
                    if rest.is_empty() {
                        return Err(at(&format!("`quirk {kind}` needs a reason")));
                    }
                    if k.quirk.insert(place, rest.to_owned()).is_some() {
                        return Err(at(&format!("`quirk {kind}` twice")));
                    }
                }
                "none" | "skip" | "opaque" => {
                    if rest.is_empty() {
                        return Err(at(&format!("`{verb} {kind}` needs a reason")));
                    }
                    if verb == "skip" {
                        let mut example = 0;
                        while lines.peek().is_some_and(|(_, l)| {
                            l.starts_with(char::is_whitespace) && !l.trim().is_empty()
                        }) {
                            lines.next();
                            example += 1;
                        }
                        if !(1..=3).contains(&example) {
                            return Err(at(&format!(
                                "`skip {kind}` needs one to three indented lines of a source \
                                 showing the different nodes (has {example})"
                            )));
                        }
                    }
                    let table = match verb {
                        "none" => &mut k.none,
                        "skip" => &mut k.skip,
                        _ => &mut k.opaque,
                    };
                    if table.insert(kind.to_owned(), rest.to_owned()).is_some() {
                        return Err(at(&format!("`{verb} {kind}` twice")));
                    }
                }
                _ => return Err(at(&format!("unknown verb `{verb}`"))),
            }
        }
        Ok(k)
    }

    /// Is `kind` (clang's) an expression the comparison reads: named like
    /// one, or a kind of a `map` line.
    fn is_clang_expr(&self, kind: &str) -> bool {
        ["Expr", "Operator", "Literal"].iter().any(|s| kind.ends_with(s))
            || self.mapped.contains(kind)
            || self.compat.contains_key(kind)
    }

    /// Does kiln's kind `kiln` match clang's kind `clang`: the same name,
    /// or a `map` line of `kiln` lists `clang` or `*` (any kind).
    fn matches(&self, kiln: &str, clang: &str) -> bool {
        kiln == clang
            || self.compat.get(kiln).is_some_and(|l| l.iter().any(|c| c == clang || c == "*"))
    }
}

// ---------------------------------------------------------------------------
// The baseline.

/// Per kind, `(none, total)`.
type NoneCounts = BTreeMap<String, (usize, usize)>;

/// Clang's file-text nodes by (file, begin): kind, end, matched.
type Exact<'a> = HashMap<(&'a str, u32), Vec<(&'a str, u32, bool)>>;

/// `tests/extents/baseline.txt`: per unit, the hash of its text and its
/// kinds with a `None` share (`unit <path> <hash> <Kind>=<none>/<total>...`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Baseline {
    /// The host architecture the baseline was recorded on (its `arch`
    /// line; `None` when it has none). The system headers a unit includes
    /// expand differently per architecture (`va_list`, `NULL`), so the
    /// `None` shares are only gated on this one.
    arch: Option<String>,
    units: BTreeMap<String, (u64, NoneCounts)>,
}

impl Baseline {
    pub fn parse(text: &str) -> Result<Baseline, String> {
        let mut b = Baseline::default();
        for (i, line) in text.lines().enumerate() {
            let at = |what: &str| format!("{BASELINE}:{}: {what}", i + 1);
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(a) = line.strip_prefix("arch ") {
                b.arch = Some(a.trim().to_owned());
                continue;
            }
            let mut w = line.split(' ');
            if w.next() != Some("unit") {
                return Err(at("expected `unit <path> <hash> <Kind>=<none>/<total>...`"));
            }
            let path = w.next().ok_or_else(|| at("no path"))?;
            let hash = w
                .next()
                .and_then(|h| u64::from_str_radix(h, 16).ok())
                .ok_or_else(|| at("bad hash"))?;
            let mut kinds = BTreeMap::new();
            for e in w {
                let parsed = e.split_once('=').and_then(|(k, v)| {
                    let (n, t) = v.split_once('/')?;
                    Some((k.to_owned(), (n.parse().ok()?, t.parse().ok()?)))
                });
                let (k, v) = parsed.ok_or_else(|| at(&format!("bad entry `{e}`")))?;
                kinds.insert(k, v);
            }
            b.units.insert(path.to_owned(), (hash, kinds));
        }
        Ok(b)
    }

    fn render(&self) -> String {
        let mut out = String::from(
            "# The expression kinds kiln gives no written extent, per unit of `cargo xtask\n\
             # extents` (spec/harness.md \"Expression extents\"): unit, FNV-1a hash of its\n\
             # text, <Kind>=<none>/<total>. A unit whose text is unchanged may not get a\n\
             # larger None share of any kind. Regenerate: cargo xtask extents --update-baseline\n\
             # The arch line names the host it was recorded on; other hosts are not gated.\n",
        );
        if let Some(a) = &self.arch {
            let _ = writeln!(out, "arch {a}");
        }
        for (path, (hash, kinds)) in &self.units {
            let _ = write!(out, "unit {path} {hash:016x}");
            for (k, (n, t)) in kinds {
                let _ = write!(out, " {k}={n}/{t}");
            }
            out.push('\n');
        }
        out
    }
}

/// FNV-1a, 64 bits: the baseline's hash of a unit's text.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ---------------------------------------------------------------------------
// Units.

/// One unit: the arguments both front ends get (besides the dump options),
/// the directory they run in and the source file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unit {
    /// The name of the unit in the report (and the baseline).
    pub name: String,
    pub dir: PathBuf,
    pub file: PathBuf,
    pub args: Vec<String>,
    /// Only the files of the repository are compared (a test unit).
    pub in_repo: bool,
}

/// Why a test file is not a stage-1 unit, or `None` when it is one.
pub fn not_a_unit(source: &str, spec: &directives::TestSpec) -> Option<&'static str> {
    if matches!(spec.expect, Expect::CompileFail | Expect::Pp | Expect::Ice) {
        return Some("expects an error, a preprocessed output or a crash");
    }
    if !spec.modules.is_empty()
        || !spec.units.is_empty()
        || !spec.header_units.is_empty()
        || spec.pch.is_some()
        || spec.as_header_unit
    {
        return Some("more than one unit");
    }
    if spec.gcc {
        // GCC is the oracle: what clang accepts may differ (a builtin it
        // lacks takes another branch of `__has_builtin`).
        return Some("GCC is the oracle");
    }
    if spec.skip_reason(CompilerKind::Kiln).is_some()
        || spec.skip_reason(CompilerKind::Clang).is_some()
    {
        return Some("skipped for kiln or clang");
    }
    if spec.requires_clang.is_some()
        || spec.requires_clang_below.is_some()
        || spec.requires_target.is_some()
        || spec.requires_host.is_some()
        || spec.requires_llvm.is_some()
        || spec.requires_llvm_below.is_some()
        || spec.min_oracle.is_some()
        || spec.requires_sanitizer_runtime
    {
        return Some("needs a particular toolchain");
    }
    if spec
        .args
        .iter()
        .any(|a| a.starts_with("-fkiln") || a.starts_with("-fclang-compat") || a.contains("%{"))
    {
        return Some("kiln-only arguments or placeholders");
    }
    // A system header makes the unit slow and compares the library, not
    // the test.
    if source.lines().any(|l| {
        let t = l.trim_start();
        let t = t.strip_prefix('#').map_or("", str::trim_start);
        ["include", "import", "embed"].iter().any(|d| t.starts_with(d)) && t.contains('<')
    }) {
        return Some("includes a system header");
    }
    None
}

/// The stage-1 units of `inputs` (test files), sorted, with the count of
/// test files considered.
fn test_units(root: &Path, inputs: &[PathBuf]) -> Result<(Vec<Unit>, usize), String> {
    let cases = crate::test::discover::discover(root, inputs, None)?;
    let mut out = Vec::new();
    let mut considered = 0;
    for c in cases {
        if c.lang != Lang::Cxx || c.is_header {
            continue;
        }
        considered += 1;
        let Ok(source) = std::fs::read_to_string(&c.path) else { continue };
        let Ok(spec) = directives::parse(&source, Lang::Cxx) else { continue };
        if not_a_unit(&source, &spec).is_some() {
            continue;
        }
        let mut args = vec![format!("-std={}", spec.std)];
        args.extend(spec.args.iter().cloned());
        out.push(Unit {
            name: c.name.clone(),
            dir: root.to_path_buf(),
            file: c.name.into(),
            args,
            in_repo: true,
        });
    }
    Ok((out, considered))
}

/// The units of a compilation database: each entry's arguments without the
/// compiler, `-c`, `-o <file>` and the source file.
fn compdb_units(path: &Path) -> Result<Vec<Unit>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let json =
        crate::util::json::Json::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let crate::util::json::Json::Arr(entries) = json else {
        return Err(format!("{}: not an array", path.display()));
    };
    let mut out = Vec::new();
    for e in &entries {
        let field = |k: &str| e.get(k).and_then(crate::util::json::Json::as_str);
        let (Some(dir), Some(file)) = (field("directory"), field("file")) else { continue };
        let words = match e.get("arguments") {
            Some(crate::util::json::Json::Arr(a)) => {
                a.iter().filter_map(|w| w.as_str().map(str::to_owned)).collect()
            }
            _ => split_command(field("command").unwrap_or_default()),
        };
        out.push(Unit {
            name: file.to_owned(),
            dir: dir.into(),
            file: file.into(),
            args: compdb_args(&words, file),
            in_repo: false,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.name == b.name);
    Ok(out)
}

/// A compilation database's command without the compiler, the output and
/// the source.
pub fn compdb_args(words: &[String], file: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = words.iter().skip(1);
    while let Some(w) = it.next() {
        match w.as_str() {
            "-c" => {}
            "-o" => {
                it.next();
            }
            _ if w == file || w.starts_with("-o") && w.len() > 2 => {}
            _ => out.push(w.clone()),
        }
    }
    out
}

/// A shell command line as words (spaces separate; single and double
/// quotes and backslashes are honored).
pub fn split_command(cmd: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut have = false;
    let mut chars = cmd.chars();
    let mut quote = None;
    while let Some(c) = chars.next() {
        match (quote, c) {
            (None, ' ' | '\t' | '\n') => {
                if have {
                    out.push(std::mem::take(&mut cur));
                    have = false;
                }
            }
            (None, '\'' | '"') => {
                quote = Some(c);
                have = true;
            }
            (Some(q), _) if c == q => quote = None,
            (Some('\''), _) => cur.push(c),
            (_, '\\') => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
                have = true;
            }
            _ => {
                cur.push(c);
                have = true;
            }
        }
    }
    if have {
        out.push(cur);
    }
    out
}

// ---------------------------------------------------------------------------
// One unit's comparison.

/// One line of kiln's dump.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KilnLine {
    pub kind: String,
    /// `Some((file, begin, end))` for a written extent.
    pub extent: Option<(String, u32, u32)>,
    /// A `Some` that is no file range (`!`): always a failure.
    pub unmapped: bool,
    /// The file and offset of a `None` line's own location, when it has
    /// one.
    pub at: Option<(String, u32)>,
}

/// Parses kiln's dump (spec/ast.md §21.4).
pub fn parse_kiln(dump: &str) -> Result<Vec<KilnLine>, String> {
    let mut out = Vec::new();
    for (i, line) in dump.lines().enumerate() {
        let f: Vec<&str> = line.split('\t').collect();
        let [kind, file, begin, end] = f[..] else {
            return Err(format!("kiln's dump, line {}: not four fields: {line:?}", i + 1));
        };
        let num = |s: &str| {
            s.parse::<u32>().map_err(|_| format!("kiln's dump, line {}: bad offset {s:?}", i + 1))
        };
        let (extent, unmapped) = match (file, end) {
            ("!", _) => (None, true),
            (_, "-") => (None, false),
            _ => (Some((file.to_owned(), num(begin)?, num(end)?)), false),
        };
        let at = match (&extent, unmapped, file) {
            (None, false, f) if f != "-" => Some((f.to_owned(), num(begin)?)),
            _ => None,
        };
        out.push(KilnLine { kind: kind.to_owned(), extent, unmapped, at });
    }
    Ok(out)
}

/// One of clang's nodes, its kind kiln's: the ranges kiln's rule could
/// give it (one for file text; for a node touching a macro, each the rule
/// could give, an over-approximation).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClangNode {
    pub kind: String,
    pub file_text: bool,
    pub ranges: Vec<(String, u32, u32)>,
    /// The file and end of a node clang printed with an invalid begin
    /// (clang 19's construction of a pack-indexed type).
    pub lost_begin: Option<(String, u32)>,
    /// Both ends in one argument of an expansion, the argument itself a
    /// macro: clang's dump gives the spelling in the inner macro's
    /// definition, not the argument's text, so the node is somewhere
    /// inside its `ranges` (the invocation).
    pub nested_arg: bool,
}

/// The per-kind counts of the report.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KindCounts {
    pub some: usize,
    pub none: usize,
    pub clang_only: usize,
    pub mismatch: usize,
}

/// The comparison of one unit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[allow(clippy::struct_field_names, reason = "the count of the report line")]
pub struct Compared {
    pub kinds: BTreeMap<String, KindCounts>,
    /// `kiln` written extents compared.
    pub compared: usize,
    /// Clang nodes touching a macro the rule gives no range for.
    pub macro_skipped: usize,
    /// `mismatch` lines (without the prefix), sorted.
    pub mismatches: Vec<String>,
}

/// Compares kiln's lines with clang's nodes (`text` reads a file's text,
/// for the mismatch lines).
#[allow(clippy::too_many_lines, reason = "one matching, pass after pass")]
pub fn compare(
    kinds: &Kinds,
    kiln: &[KilnLine],
    clang: &[ClangNode],
    macro_skipped: usize,
    text: &dyn Fn(&str, u32, u32) -> String,
) -> Compared {
    let mut c = Compared { macro_skipped, ..Compared::default() };
    // Clang's file-text nodes by (file, begin): kind, end, matched.
    let mut exact: Exact<'_> = HashMap::new();
    let mut macros: Vec<(&ClangNode, bool)> = Vec::new();
    for n in clang.iter().filter(|n| !kinds.skip.contains_key(&n.kind)) {
        match (n.file_text, n.ranges.first()) {
            (true, Some((f, b, e))) => exact.entry((f, *b)).or_default().push((&n.kind, *e, false)),
            _ => macros.push((n, false)),
        }
    }
    // The ranges of clang's nodes whose operands its dump leaves out.
    let opaque: Vec<&(String, u32, u32)> = clang
        .iter()
        .filter(|n| kinds.opaque.contains_key(&n.kind))
        .filter_map(|n| n.ranges.first())
        .collect();
    let inside_opaque = |f: &str, b: u32, e: u32| {
        opaque.iter().any(|(of, ob, oe)| of == f && *ob <= b && e <= *oe && (*ob, *oe) != (b, e))
    };
    let mut leftover = Vec::new();
    // The ranges of the kiln nodes that matched a clang node.
    let mut verified: std::collections::HashSet<(&str, u32, u32)> =
        std::collections::HashSet::new();
    for l in kiln {
        let counts = c.kinds.entry(l.kind.clone()).or_default();
        if l.unmapped {
            counts.some += 1;
            counts.mismatch += 1;
            c.mismatches
                .push(format!("- {} kiln ! (a written extent that is no file range)", l.kind));
            continue;
        }
        let Some((f, b, e)) = &l.extent else {
            counts.none += 1;
            continue;
        };
        counts.some += 1;
        // Inside a node whose operands clang leaves out, a kiln node is
        // compared only where clang printed a node too.
        let unprinted = inside_opaque(f, *b, *e) && !exact.contains_key(&(f.as_str(), *b));
        let quirk = kinds.quirk.contains_key(&(f.clone(), *b));
        if kinds.skip.contains_key(&l.kind) || unprinted || quirk {
            continue;
        }
        c.compared += 1;
        if kinds.none.contains_key(&l.kind) {
            counts.mismatch += 1;
            c.mismatches.push(format!(
                "{f}:{b}-{e} {} kiln {:?} clang - \"\" (a `none` kind has an extent)",
                l.kind,
                text(f, *b, *e)
            ));
            continue;
        }
        // Ends are compared as sets: a node the two trees have a
        // different number of times at one place is a shape difference,
        // not an extent one. The node of the same kind is preferred.
        let mut hit = false;
        if let Some(bucket) = exact.get_mut(&(f.as_str(), *b)) {
            for want_same in [true, false] {
                if let Some(slot) = bucket.iter_mut().find(|(k, end, _)| {
                    end == e && (if want_same { *k == l.kind } else { kinds.matches(&l.kind, k) })
                }) {
                    slot.2 = true;
                    hit = true;
                    break;
                }
            }
        }
        if hit {
            verified.insert((f.as_str(), *b, *e));
        } else {
            leftover.push(l);
        }
    }
    let mut unmatched = Vec::new();
    for l in leftover {
        let Some((f, b, e)) = &l.extent else { continue };
        let want = (f.clone(), *b, *e);
        let mut hit = false;
        for (n, used) in &mut macros {
            if !n.nested_arg && kinds.matches(&l.kind, &n.kind) && n.ranges.contains(&want) {
                *used = true;
                hit = true;
            }
        }
        if hit {
            verified.insert((f.as_str(), *b, *e));
        } else {
            unmatched.push(l);
        }
    }
    for l in unmatched {
        let Some((f, b, e)) = &l.extent else { continue };
        // The range of a kiln node clang matched: an implicit node around
        // a written one, which has the written node's extent (spec/ast.md
        // §21.2) where clang builds none.
        if verified.contains(&(f.as_str(), *b, *e)) {
            continue;
        }
        // A node inside an invocation whose argument is a macro: clang's
        // dump does not say where in it.
        if clang.iter().any(|n| {
            n.nested_arg
                && kinds.matches(&l.kind, &n.kind)
                && n.ranges.iter().any(|(rf, rb, re)| rf == f && rb <= b && e <= re)
        }) {
            c.compared -= 1;
            c.macro_skipped += 1;
            continue;
        }
        // A node clang lost the begin of: compared by its end.
        if clang.iter().any(|n| {
            n.lost_begin.as_ref().is_some_and(|(nf, ne)| nf == f && ne == e)
                && kinds.matches(&l.kind, &n.kind)
        }) {
            continue;
        }
        // Text no clang expression touches is text clang did not compile
        // as one (the other branch of a conditional inclusion, an operand
        // of a declaration clang does not print): not compared.
        let touched = clang
            .iter()
            .flat_map(|n| n.ranges.iter())
            .any(|(rf, rb, re)| rf == f && rb < e && b < re);
        if !touched {
            c.compared -= 1;
            continue;
        }
        if let Some(k) = c.kinds.get_mut(&l.kind) {
            k.mismatch += 1;
        }
        // Clang's node of a matching kind at the same begin (else at the
        // same end), to show.
        let same_begin = exact
            .get(&(f.as_str(), *b))
            .and_then(|bucket| bucket.iter().find(|(k, ..)| kinds.matches(&l.kind, k)))
            .map(|&(_, end, _)| (*b, end))
            .or_else(|| {
                macros.iter().flat_map(|(n, _)| n.ranges.iter().map(move |r| (n, r))).find_map(
                    |(n, (rf, rb, re))| {
                        (kinds.matches(&l.kind, &n.kind) && rf == f && rb == b)
                            .then_some((*rb, *re))
                    },
                )
            })
            .or_else(|| {
                exact
                    .iter()
                    .filter(|((rf, _), _)| rf == f)
                    .flat_map(|((_, rb), bucket)| bucket.iter().map(move |n| (*rb, n)))
                    .filter(|(_, (k, re, _))| re == e && kinds.matches(&l.kind, k))
                    .map(|(rb, &(_, re, _))| (rb, re))
                    .min()
            });
        let clang = match same_begin {
            Some((cb, ce)) => format!("{cb}-{ce} {:?}", text(f, cb, ce)),
            None => "- \"\"".to_owned(),
        };
        c.mismatches.push(format!(
            "{f}:{b}-{e} {} kiln {:?} clang {clang}",
            l.kind,
            text(f, *b, *e)
        ));
    }
    for (k, _, used) in exact.values().flatten() {
        if !used {
            c.kinds.entry((*k).to_owned()).or_default().clang_only += 1;
        }
    }
    for (n, used) in &macros {
        if !used {
            c.kinds.entry(n.kind.clone()).or_default().clang_only += 1;
        }
    }
    c.mismatches.sort();
    c
}

/// The file names of one unit as the report shows them: relative to the
/// unit's directory's repository root when under it, canonical otherwise.
struct Names {
    root: PathBuf,
    dir: PathBuf,
    cache: HashMap<String, String>,
}

impl Names {
    fn name(&mut self, raw: &str) -> String {
        if let Some(n) = self.cache.get(raw) {
            return n.clone();
        }
        let p = self.dir.join(raw);
        let n = match std::fs::canonicalize(&p) {
            Ok(c) => match c.strip_prefix(&self.root) {
                Ok(rel) => rel.to_string_lossy().into_owned(),
                Err(_) => c.to_string_lossy().into_owned(),
            },
            Err(_) => raw.to_owned(),
        };
        self.cache.insert(raw.to_owned(), n.clone());
        n
    }
}

/// The file texts a unit's report quotes and the macro rule reads.
#[derive(Default)]
struct Texts {
    files: Mutex<HashMap<String, Arc<Vec<u8>>>>,
}

impl Texts {
    fn get(&self, root: &Path, name: &str) -> Arc<Vec<u8>> {
        let mut m = self.files.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        m.entry(name.to_owned())
            .or_insert_with(|| Arc::new(std::fs::read(root.join(name)).unwrap_or_default()))
            .clone()
    }
}

/// The text of `[b, e)` of `bytes`, shortened for a report line.
fn quote(bytes: &[u8], b: u32, e: u32) -> String {
    let (b, e) = (b as usize, e as usize);
    let Some(s) = bytes.get(b..e.min(bytes.len())) else { return String::new() };
    let s = String::from_utf8_lossy(s);
    let s: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() > 60 {
        let head: String = s.chars().take(28).collect();
        let tail: String = s.chars().rev().take(28).collect::<Vec<_>>().into_iter().rev().collect();
        format!("{head} ... {tail}")
    } else {
        s
    }
}

/// The end of a macro invocation whose name ends at `name_end` in `text`:
/// the name's end and, when a `(` follows it, the end of the balanced
/// parentheses after it (clang's location does not say whether the macro
/// is function-like, so both are candidates).
pub fn invocation_ends(text: &[u8], name_end: u32) -> Vec<u32> {
    let mut out = vec![name_end];
    let mut i = name_end as usize;
    while i < text.len() && text[i].is_ascii_whitespace() {
        i += 1;
    }
    if text.get(i) != Some(&b'(') {
        return out;
    }
    let mut depth = 0usize;
    while i < text.len() {
        match text[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    if let Ok(end) = u32::try_from(i + 1) {
                        out.push(end);
                    }
                    return out;
                }
            }
            q @ (b'"' | b'\'') => {
                i += 1;
                while i < text.len() && text[i] != q {
                    if text[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// The begin of the function-like macro invocation whose `)` is at
/// `close` in `text`: the name before the balanced parentheses ending
/// there.
pub fn invocation_begin(text: &[u8], close: u32) -> Option<u32> {
    let mut i = close as usize;
    if text.get(i) != Some(&b')') {
        return None;
    }
    let mut depth = 0usize;
    loop {
        match text[i] {
            b')' => depth += 1,
            b'(' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        i = i.checked_sub(1)?;
    }
    while i > 0 && text[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    let end = i;
    while i > 0 && (text[i - 1].is_ascii_alphanumeric() || text[i - 1] == b'_') {
        i -= 1;
    }
    (i < end).then(|| u32::try_from(i).ok()).flatten()
}

/// Clang's node as kiln's rule would map it: the candidate ranges, and
/// whether it is file text.
#[allow(clippy::many_single_char_names, reason = "begin and end, spelling and expansion")]
fn clang_node(
    dump_files: &[String],
    names: &mut Names,
    texts: &Texts,
    n: &clang::Node,
) -> ClangNode {
    let kind = n.kind.clone();
    let file = |names: &mut Names, i: u32| names.name(&dump_files[i as usize]);
    let mut ranges = Vec::new();
    let (Some(b), Some(e)) = (n.begin, n.end) else {
        let lost_begin = match (n.begin, n.end) {
            (None, Some(clang::CLoc::File(e))) => Some((file(names, e.file), e.offset + e.tok_len)),
            _ => None,
        };
        return ClangNode { kind, file_text: false, ranges, lost_begin, nested_arg: false };
    };
    if let (clang::CLoc::File(b), clang::CLoc::File(e)) = (b, e) {
        if b.file == e.file {
            ranges.push((file(names, b.file), b.offset, e.offset + e.tok_len));
        }
        return ClangNode { kind, file_text: true, ranges, lost_begin: None, nested_arg: false };
    }
    // Both in the same argument of one expansion: the spelling, when it
    // is the argument's text (inside the invocation).
    let mut nested_arg = false;
    if let (
        clang::CLoc::Macro { spelling: Some(sb), expansion: Some(xb), arg: true },
        clang::CLoc::Macro { spelling: Some(se), expansion: Some(xe), arg: true },
    ) = (b, e)
        && xb == xe
        && sb.file == se.file
        && sb.offset <= se.offset
    {
        {
            let name = file(names, xb.file);
            let last = invocation_ends(&texts.get(&names.root, &name), xb.offset + xb.tok_len);
            let inside = sb.file == xb.file
                && sb.offset >= xb.offset
                && last.iter().max().is_some_and(|&m| se.offset + se.tok_len <= m);
            if inside {
                ranges.push((file(names, sb.file), sb.offset, se.offset + se.tok_len));
            } else {
                nested_arg = true;
            }
        }
    }
    // The whole expansion: the invocation's begin and end. A builtin
    // macro's result (`__has_builtin(x)`) is spelled in clang's scratch
    // space and expanded at the invocation's `)`: its begin is also the
    // invocation's name.
    let mut begins = Vec::new();
    match b {
        clang::CLoc::File(b) => begins.push((b.file, b.offset)),
        clang::CLoc::Macro { spelling, expansion: Some(x), .. } => {
            begins.push((x.file, x.offset));
            if spelling.is_some_and(|s| dump_files[s.file as usize] == "<scratch space>") {
                let name = file(names, x.file);
                if let Some(o) = invocation_begin(&texts.get(&names.root, &name), x.offset) {
                    begins.push((x.file, o));
                }
            }
        }
        clang::CLoc::Macro { .. } => {}
    }
    let ends = match e {
        clang::CLoc::File(e) => Some((e.file, vec![e.offset + e.tok_len])),
        clang::CLoc::Macro { expansion: Some(x), .. } => {
            let name = file(names, x.file);
            Some((x.file, invocation_ends(&texts.get(&names.root, &name), x.offset + x.tok_len)))
        }
        clang::CLoc::Macro { .. } => None,
    };
    if let Some((ef, eos)) = ends {
        for &(bf, bo) in begins.iter().filter(|(bf, _)| *bf == ef) {
            let f = file(names, bf);
            for &eo in &eos {
                let r = (f.clone(), bo, eo);
                if bo < eo && !ranges.contains(&r) {
                    ranges.push(r);
                }
            }
        }
    }
    ClangNode { kind, file_text: false, ranges, lost_begin: None, nested_arg }
}

/// What became of one unit.
#[derive(Debug)]
enum Verdict {
    Compared {
        compared: Compared,
        kiln: Vec<KilnLine>,
    },
    /// A front end rejected the unit (exit status 1): listed, not compared.
    Rejected(String),
    /// A front end crashed, hung, or printed what cannot be read.
    Failed(String),
}

fn first_error(stderr: &str) -> String {
    stderr
        .lines()
        .find(|l| l.contains("error"))
        .or_else(|| stderr.lines().next())
        .unwrap_or_default()
        .chars()
        .take(160)
        .collect()
}

fn run_unit(
    root: &Path,
    kiln: &Path,
    clang_bin: &Path,
    kinds: &Arc<Kinds>,
    texts: &Texts,
    u: &Unit,
) -> Verdict {
    let mut kcmd = Command::new(kiln);
    kcmd.current_dir(&u.dir)
        .args(&u.args)
        .args(["-fsyntax-only", "-fkiln-dump-expr-extents"])
        .arg(&u.file);
    let ko = match process::run(&mut kcmd, UNIT_TIMEOUT) {
        Ok(o) => o,
        Err(e) => return Verdict::Failed(format!("cannot run kiln: {e}")),
    };
    let mut ccmd = Command::new(clang_bin);
    ccmd.current_dir(&u.dir)
        .args(&u.args)
        .args(["-fsyntax-only", "-Xclang", "-ast-dump=json"])
        .arg(&u.file);
    let table = Arc::clone(kinds);
    let read = move |mut pipe: std::process::ChildStdout| {
        let r = clang::read(&mut pipe, &|k| table.is_clang_expr(k));
        // Drain what is left, so clang never blocks on a full pipe.
        let _ = io::copy(&mut pipe, &mut io::sink());
        r
    };
    let (co, dump) = match process::run_streaming(&mut ccmd, UNIT_TIMEOUT, read) {
        Ok(r) => r,
        Err(e) => return Verdict::Failed(format!("cannot run clang: {e}")),
    };
    let ended = |o: &process::Output| o.code() == Some(0) || o.code() == Some(1);
    if !ended(&ko) {
        return Verdict::Failed(format!(
            "kiln: {}: {}",
            ko.describe_end(),
            first_error(&ko.stderr_str())
        ));
    }
    if !ended(&co) {
        // Clang crashing (its JSON dumper on a few units) is clang's
        // failure, not kiln's: listed, not compared. A hang is a failure.
        if co.timed_out() {
            return Verdict::Failed(format!("clang: {}", co.describe_end()));
        }
        return Verdict::Rejected(format!("clang: {}", co.describe_end()));
    }
    match (ko.success(), co.success()) {
        (true, true) => {}
        (false, true) => {
            return Verdict::Rejected(format!("kiln: {}", first_error(&ko.stderr_str())));
        }
        (true, false) => {
            return Verdict::Rejected(format!("clang: {}", first_error(&co.stderr_str())));
        }
        (false, false) => return Verdict::Rejected("both".to_owned()),
    }
    let dump = match dump {
        Some(Ok(d)) => d,
        Some(Err(e)) => return Verdict::Failed(format!("clang: {e}")),
        None => return Verdict::Failed("clang: the reader panicked".to_owned()),
    };
    let mut names = Names { root: root.to_path_buf(), dir: u.dir.clone(), cache: HashMap::new() };
    let mut kiln_lines = match parse_kiln(&ko.stdout_str()) {
        Ok(l) => l,
        Err(e) => return Verdict::Failed(e),
    };
    for l in &mut kiln_lines {
        if let Some((f, ..)) = &mut l.extent {
            *f = names.name(f);
        }
        if let Some((f, _)) = &mut l.at {
            *f = names.name(f);
        }
    }
    // A test unit compares the files of the repository (a system header
    // is the library's, not the test's): a name that stayed absolute is
    // outside it.
    let outside = |f: &str| u.in_repo && Path::new(f).is_absolute();
    kiln_lines.retain(|l| {
        let f =
            l.extent.as_ref().map(|(f, ..)| f.as_str()).or(l.at.as_ref().map(|(f, _)| f.as_str()));
        !f.is_some_and(outside)
    });
    let mut macro_skipped = 0;
    let mut nodes = Vec::with_capacity(dump.nodes.len());
    for n in &dump.nodes {
        let c = clang_node(&dump.files, &mut names, texts, n);
        if c.ranges.first().is_some_and(|(f, ..)| outside(f)) {
            continue;
        }
        if c.ranges.is_empty() && c.lost_begin.is_none() {
            if !c.file_text {
                macro_skipped += 1;
            }
            continue;
        }
        nodes.push(c);
    }
    let text = |f: &str, b: u32, e: u32| quote(&texts.get(root, f), b, e);
    let compared = compare(kinds, &kiln_lines, &nodes, macro_skipped, &text);
    Verdict::Compared { compared, kiln: kiln_lines }
}

// ---------------------------------------------------------------------------
// A run.

/// A finished run: the report, the units passed and failed, and the
/// test files considered.
#[derive(Debug)]
pub struct Report {
    pub text: String,
    /// Units compared with no mismatch, and units failed (compared or
    /// not).
    pub passed: usize,
    pub failed_units: usize,
    pub considered: usize,
}

fn read_table<T>(
    root: &Path,
    given: Option<&PathBuf>,
    default: &str,
    parse: fn(&str) -> Result<T, String>,
) -> Result<T, String> {
    let p = given.cloned().unwrap_or_else(|| root.join(default));
    let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    parse(&text)
}

/// A unit's per-kind `(none, total)` counts.
fn none_counts(kiln: &[KilnLine]) -> BTreeMap<String, (usize, usize)> {
    let mut m: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for l in kiln {
        let e = m.entry(l.kind.clone()).or_default();
        e.1 += 1;
        if l.extent.is_none() && !l.unmapped {
            e.0 += 1;
        }
    }
    m
}

#[allow(clippy::too_many_lines, reason = "one run: units, workers, report")]
pub fn execute(root: &Path, kiln: &Path, opts: &Options) -> Result<Report, String> {
    let kinds = Arc::new(read_table(root, opts.kinds.as_ref(), KINDS, Kinds::parse)?);
    let gated = opts.compdb.is_none();
    let mut other_arch = None;
    let baseline = if gated && !opts.update_baseline {
        let b = read_table(root, opts.baseline.as_ref(), BASELINE, Baseline::parse)?;
        match &b.arch {
            Some(a) if a != std::env::consts::ARCH => {
                other_arch = Some(a.clone());
                None
            }
            _ => Some(b),
        }
    } else {
        None
    };
    let (mut units, considered) = if let Some(db) = &opts.compdb {
        let u = compdb_units(db)?;
        let n = u.len();
        (u, n)
    } else {
        let inputs: Vec<PathBuf> = if opts.paths.is_empty() {
            DEFAULT_INPUTS.iter().map(|d| root.join(d)).collect()
        } else {
            opts.paths
                .iter()
                .map(|p| if p.is_absolute() { p.clone() } else { root.join(p) })
                .collect()
        };
        test_units(root, &inputs)?
    };
    if let Some(f) = &opts.filter {
        units.retain(|u| u.name.contains(f.as_str()));
    }
    if let Some(s) = opts.shard {
        units = s.select(units);
    }
    let clang_bin = opts.clang.clone().unwrap_or_else(|| PathBuf::from("clang++-19"));
    crate::util::identify(&clang_bin)?;
    let texts = Texts::default();
    let verdicts: Vec<Mutex<Option<Verdict>>> = units.iter().map(|_| Mutex::new(None)).collect();
    let next = AtomicUsize::new(0);
    let workers = opts
        .jobs
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(4, std::num::NonZero::get))
        .min(units.len().max(1));
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(u) = units.get(i) else { break };
                    let v = run_unit(root, kiln, &clang_bin, &kinds, &texts, u);
                    if opts.verbose {
                        eprintln!("xtask extents: {} done", u.name);
                    }
                    *verdicts[i].lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
                        Some(v);
                }
            });
        }
    });
    let mut total = Compared::default();
    let mut lines = Vec::new();
    let mut failed_units = 0;
    let mut compared_units = 0;
    let mut bad_compared = 0;
    let mut fresh =
        Baseline { arch: Some(std::env::consts::ARCH.to_owned()), ..Baseline::default() };
    let mut stale = 0;
    for (u, v) in units.iter().zip(verdicts) {
        let v = v.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner);
        match v {
            None => {
                failed_units += 1;
                lines.push(format!("failed {}: no verdict", u.name));
            }
            Some(Verdict::Rejected(why)) => lines.push(format!("rejected {}: {why}", u.name)),
            Some(Verdict::Failed(why)) => {
                failed_units += 1;
                lines.push(format!("failed {}: {why}", u.name));
            }
            Some(Verdict::Compared { compared, kiln }) => {
                compared_units += 1;
                let mut bad = !compared.mismatches.is_empty();
                for m in &compared.mismatches {
                    lines.push(format!("mismatch {m}"));
                }
                if opts.nones {
                    for l in kiln.iter().filter(|l| l.extent.is_none() && !l.unmapped) {
                        lines.push(match &l.at {
                            Some((f, o)) => {
                                let t = quote(&texts.get(root, f), *o, o.saturating_add(40));
                                format!("none {f}:{o} {} {t:?}", l.kind)
                            }
                            None => format!("none - {}", l.kind),
                        });
                    }
                }
                let hash = std::fs::read(u.dir.join(&u.file)).map_or(0, |b| fnv1a(&b));
                let counts: BTreeMap<String, (usize, usize)> =
                    none_counts(&kiln).into_iter().filter(|(_, (n, _))| *n > 0).collect();
                if let Some(base) = &baseline {
                    match base.units.get(&u.name) {
                        Some((h, old)) if *h == hash => {
                            let all = none_counts(&kiln);
                            for (k, &(n, t)) in &all {
                                let (bn, bt) = old.get(k).copied().unwrap_or((0, t.max(1)));
                                // n/t > bn/bt
                                if n * bt > bn * t {
                                    bad = true;
                                    lines.push(format!(
                                        "none-grew {} {k}: {n}/{t} (baseline {bn}/{bt})",
                                        u.name
                                    ));
                                }
                            }
                        }
                        _ => stale += 1,
                    }
                }
                fresh.units.insert(u.name.clone(), (hash, counts));
                for (k, kc) in compared.kinds {
                    let t = total.kinds.entry(k).or_default();
                    t.some += kc.some;
                    t.none += kc.none;
                    t.clang_only += kc.clang_only;
                    t.mismatch += kc.mismatch;
                }
                total.compared += compared.compared;
                total.macro_skipped += compared.macro_skipped;
                if bad {
                    failed_units += 1;
                    bad_compared += 1;
                }
                if opts.verbose {
                    lines.push(format!("unit {}: {}", u.name, if bad { "FAIL" } else { "ok" }));
                }
            }
        }
    }
    let mismatches: usize = total.kinds.values().map(|k| k.mismatch).sum();
    let mut text = format!(
        "extents: {compared_units} units, {} compared, {mismatches} mismatches, {} macro-skipped\n",
        total.compared, total.macro_skipped
    );
    for (k, c) in &total.kinds {
        let _ = writeln!(
            text,
            "kind {k}: some {} none {} clang-only {} mismatch {}",
            c.some, c.none, c.clang_only, c.mismatch
        );
    }
    lines.sort();
    for l in &lines {
        text.push_str(l);
        text.push('\n');
    }
    if let Some(a) = &other_arch {
        let _ = writeln!(
            text,
            "baseline: recorded on {a}, not gated on {} (mismatches still are)",
            std::env::consts::ARCH
        );
    }
    if stale > 0 {
        let _ = writeln!(
            text,
            "baseline: {stale} units new or changed since the baseline (not gated; \
             cargo xtask extents --update-baseline)"
        );
    }
    if opts.update_baseline {
        let p = opts.baseline.clone().unwrap_or_else(|| root.join(BASELINE));
        std::fs::write(&p, fresh.render()).map_err(|e| format!("{}: {e}", p.display()))?;
        let _ = writeln!(text, "baseline: wrote {} units", fresh.units.len());
    }
    Ok(Report { text, passed: compared_units - bad_compared, failed_units, considered })
}
