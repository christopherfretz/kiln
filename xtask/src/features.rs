//! `cargo xtask features`: check the C++23/C++26 language feature table,
//! `docs/status/cxx-features.md`, against the corpus and kiln's predefined
//! macros. Contract: `spec/harness.md` "Feature table". A manual / M4-gate
//! check, not part of `ci`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::{test, util};

pub const USAGE: &str = "\
usage: cargo xtask features [options]

Checks docs/status/cxx-features.md (spec/harness.md \"Feature table\"):
  (a) every `done` row names at least one test, and every named test exists;
  (b) the tests of `done` rows pass under kiln (the `cargo xtask test` runner);
  (c) for `done` rows with a macro, `kiln++ -std=c++26 -dM -E` defines it with
      the table's clang 22 value; and on every row the table's kiln value is
      the one kiln defines today.

options:
  --compiler <path>   kiln++ binary (default: cargo build -p kiln_driver)
  --table <path>      the table (default: docs/status/cxx-features.md)
  -h, --help          this text

exit status: 0 all checks pass, 1 a row failed a check, 2 usage, parse or
infrastructure error.
";

/// The table's columns, in order (matched on the header cell's prefix).
const COLUMNS: [&str; 6] =
    ["Paper", "Feature", "`__cpp_*` macro", "Status", "Corpus tests", "Notes"];

/// A row's status (spec/harness.md "Feature table").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureStatus {
    Done,
    Partial,
    Todo,
}

impl FeatureStatus {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "done" => Some(Self::Done),
            "partial" => Some(Self::Partial),
            "todo" => Some(Self::Todo),
            _ => None,
        }
    }
}

/// The macro cell: `` `__cpp_name` KILN / CLANG22 ``, `None` meaning undefined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacroCell {
    pub name: String,
    pub kiln: Option<String>,
    pub clang22: Option<String>,
}

/// One table row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// 1-based line in the table file.
    pub line: usize,
    pub paper: String,
    pub feature: String,
    pub macro_cell: Option<MacroCell>,
    pub status: FeatureStatus,
    /// Paths relative to `tests/`.
    pub tests: Vec<String>,
}

/// Splits a Markdown table line into trimmed cells. A `|` inside a backtick
/// span, or written `\|`, does not split.
fn split_cells(line: &str) -> Vec<String> {
    let inner = line.trim();
    let inner = inner.strip_prefix('|').unwrap_or(inner);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut in_code = false;
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '`' => {
                in_code = !in_code;
                cur.push(c);
            }
            '\\' if chars.peek() == Some(&'|') => {
                cur.push('|');
                chars.next();
            }
            '|' if !in_code => cells.push(std::mem::take(&mut cur).trim().to_owned()),
            _ => cur.push(c),
        }
    }
    cells.push(cur.trim().to_owned());
    cells
}

/// The backtick spans of `cell`, in order.
fn code_spans(cell: &str) -> Vec<String> {
    cell.split('`').skip(1).step_by(2).map(str::to_owned).collect()
}

fn is_dash(s: &str) -> bool {
    matches!(s, "—" | "-" | "")
}

/// `202110L` / `1`: digits with an optional `L` suffix.
fn is_macro_value(s: &str) -> bool {
    let digits = s.strip_suffix('L').unwrap_or(s);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

fn parse_macro_cell(cell: &str) -> Result<Option<MacroCell>, String> {
    if is_dash(cell) {
        return Ok(None);
    }
    let bad = || format!("macro cell \"{cell}\": expected — or `__cpp_name` KILN / CLANG22");
    let rest = cell.strip_prefix('`').ok_or_else(bad)?;
    let (name, values) = rest.split_once('`').ok_or_else(bad)?;
    if !name.starts_with("__cpp_") || name.len() == "__cpp_".len() {
        return Err(format!("macro cell \"{cell}\": `{name}` is not a __cpp_* macro"));
    }
    let (k, c) = values.split_once('/').ok_or_else(bad)?;
    let value = |v: &str| -> Result<Option<String>, String> {
        let v = v.trim();
        if is_dash(v) {
            Ok(None)
        } else if is_macro_value(v) {
            Ok(Some(v.to_owned()))
        } else {
            Err(format!("macro cell \"{cell}\": `{v}` is neither — nor a value like 202110L"))
        }
    };
    Ok(Some(MacroCell { name: name.to_owned(), kiln: value(k)?, clang22: value(c)? }))
}

fn is_paper(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('P') else { return false };
    let Some((num, rev)) = rest.split_once('R') else { return false };
    !num.is_empty()
        && !rev.is_empty()
        && num.bytes().all(|b| b.is_ascii_digit())
        && rev.bytes().all(|b| b.is_ascii_digit())
}

/// Parses every feature table in `text` (a table is a header row whose first
/// cell is `Paper`, a `|---` separator, then rows up to the first line not
/// starting with `|`). Errors carry 1-based line numbers.
pub fn parse_table(text: &str) -> Result<Vec<Row>, Vec<String>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut rows = Vec::new();
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();
    let mut i = 0;
    let mut tables = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        if !line.starts_with('|') || split_cells(line).first().map(String::as_str) != Some("Paper") {
            i += 1;
            continue;
        }
        tables += 1;
        let header = split_cells(line);
        if header.len() != COLUMNS.len()
            || !header.iter().zip(COLUMNS).all(|(h, c)| h.starts_with(c))
        {
            errors.push(format!(
                "line {}: table header must be | {} |",
                i + 1,
                COLUMNS.join(" | ")
            ));
        }
        i += 1;
        if !lines.get(i).is_some_and(|l| l.trim().starts_with("|-")) {
            errors.push(format!("line {}: expected the |---| separator row", i + 1));
        } else {
            i += 1;
        }
        while i < lines.len() && lines[i].trim().starts_with('|') {
            match parse_row(lines[i], i + 1) {
                Ok(row) => {
                    if !seen.insert(row.paper.clone()) {
                        errors.push(format!("line {}: duplicate row for {}", i + 1, row.paper));
                    }
                    rows.push(row);
                }
                Err(e) => errors.push(format!("line {}: {e}", i + 1)),
            }
            i += 1;
        }
    }
    if tables == 0 {
        errors.push("no feature table (a header row starting `| Paper |`)".into());
    }
    if errors.is_empty() { Ok(rows) } else { Err(errors) }
}

fn parse_row(line: &str, lineno: usize) -> Result<Row, String> {
    let cells = split_cells(line);
    if cells.len() != COLUMNS.len() {
        return Err(format!("expected {} cells, found {}", COLUMNS.len(), cells.len()));
    }
    let paper = cells[0].clone();
    if !is_paper(&paper) {
        return Err(format!("`{paper}` is not a paper number like P0847R7"));
    }
    let status = FeatureStatus::parse(&cells[3]).ok_or_else(|| {
        format!("{paper}: status `{}` is not one of done, partial, todo", cells[3])
    })?;
    Ok(Row {
        line: lineno,
        feature: cells[1].clone(),
        macro_cell: parse_macro_cell(&cells[2]).map_err(|e| format!("{paper}: {e}"))?,
        status,
        tests: code_spans(&cells[4]),
        paper,
    })
}

/// `#define NAME VALUE` lines of `-dM -E` output, as NAME -> VALUE.
pub fn parse_predefs(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|l| l.strip_prefix("#define "))
        .filter_map(|l| {
            let (name, value) = l.split_once(' ').unwrap_or((l, ""));
            (!name.contains('(')).then(|| (name.to_owned(), value.trim().to_owned()))
        })
        .collect()
}

/// Checks (a) and (c), which need no test run: `exists` says whether a path
/// under `tests/` exists; `predefs` is kiln's `-dM -E` output. Returns the
/// problems per row (paper -> messages), rows in table order.
pub fn check_static(
    rows: &[Row],
    exists: impl Fn(&str) -> bool,
    predefs: &BTreeMap<String, String>,
) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for row in rows {
        let mut p = Vec::new();
        if row.status == FeatureStatus::Done && row.tests.is_empty() {
            p.push("(a) `done` but names no test".to_owned());
        }
        for t in &row.tests {
            if !exists(t) {
                p.push(format!("(a) test `tests/{t}` does not exist"));
            }
        }
        if let Some(m) = &row.macro_cell {
            let actual = predefs.get(&m.name);
            if row.status == FeatureStatus::Done && actual != m.clang22.as_ref() {
                p.push(format!(
                    "(c) kiln defines `{}` as {}, clang 22 as {}",
                    m.name,
                    show(actual),
                    show(m.clang22.as_ref())
                ));
            }
            if actual != m.kiln.as_ref() {
                p.push(format!(
                    "(c) stale kiln column: kiln defines `{}` as {}, the table says {}",
                    m.name,
                    show(actual),
                    show(m.kiln.as_ref())
                ));
            }
        }
        if !p.is_empty() {
            out.push((row.paper.clone(), p));
        }
    }
    out
}

fn show(v: Option<&String>) -> String {
    v.map_or_else(|| "undefined".to_owned(), Clone::clone)
}

struct Args {
    compiler: Option<PathBuf>,
    table: Option<PathBuf>,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Option<Args>, String> {
    let mut a = Args { compiler: None, table: None };
    let mut it = args;
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_owned(), Some(v.to_owned())),
            _ => (arg.clone(), None),
        };
        let mut value = |name: &str| {
            inline.clone().or_else(|| it.next()).ok_or_else(|| format!("{name} needs a value"))
        };
        match flag.as_str() {
            "--compiler" => a.compiler = Some(PathBuf::from(value("--compiler")?)),
            "--table" => a.table = Some(PathBuf::from(value("--table")?)),
            "-h" | "--help" => return Ok(None),
            _ => return Err(format!("unknown argument `{arg}`")),
        }
    }
    Ok(Some(a))
}

fn kiln_predefs(kiln: &Path) -> Result<BTreeMap<String, String>, String> {
    let o = Command::new(kiln)
        .args(["-std=c++26", "-dM", "-E", "-x", "c++", "/dev/null"])
        .output()
        .map_err(|e| format!("cannot run {}: {e}", kiln.display()))?;
    if !o.status.success() {
        return Err(format!(
            "`{} -std=c++26 -dM -E` failed ({}): {}",
            kiln.display(),
            o.status,
            String::from_utf8_lossy(&o.stderr)
        ));
    }
    Ok(parse_predefs(&String::from_utf8_lossy(&o.stdout)))
}

pub fn main(args: impl Iterator<Item = String>) -> ExitCode {
    let args = match parse_args(args) {
        Ok(Some(a)) => a,
        Ok(None) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprint!("xtask features: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let root = util::repo_root();
    let table = args.table.unwrap_or_else(|| root.join("docs/status/cxx-features.md"));
    let text = match std::fs::read_to_string(&table) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("xtask features: cannot read {}: {e}", table.display());
            return ExitCode::from(2);
        }
    };
    let rows = match parse_table(&text) {
        Ok(r) => r,
        Err(errors) => {
            for e in &errors {
                eprintln!("xtask features: {}: {e}", table.display());
            }
            return ExitCode::from(2);
        }
    };
    let kiln = match args.compiler {
        Some(c) => c,
        None => match util::target_dir(&root)
            .and_then(|t| util::build_kiln(&root, &t, "features", false))
        {
            Ok(k) => k,
            Err(e) => {
                eprintln!("xtask features: {e}");
                return ExitCode::from(2);
            }
        },
    };
    let predefs = match kiln_predefs(&kiln) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("xtask features: {e}");
            return ExitCode::from(2);
        }
    };
    let tests_dir = root.join("tests");
    let mut problems: BTreeMap<String, Vec<String>> =
        check_static(&rows, |t| tests_dir.join(t).is_file(), &predefs).into_iter().collect();

    // (b): one runner invocation over every existing test of a `done` row.
    let done_tests: BTreeSet<&str> = rows
        .iter()
        .filter(|r| r.status == FeatureStatus::Done)
        .flat_map(|r| r.tests.iter().map(String::as_str))
        .filter(|t| tests_dir.join(t).is_file())
        .collect();
    let results = match test::execute(&test::Options {
        compiler: Some(kiln),
        paths: done_tests.iter().map(|t| tests_dir.join(t)).collect(),
        ..Default::default()
    }) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("xtask features: {e}");
            return ExitCode::from(2);
        }
    };
    let outcome: BTreeMap<String, &'static str> =
        results.iter().map(|r| (r.name.clone(), r.status.label())).collect();
    for row in rows.iter().filter(|r| r.status == FeatureStatus::Done) {
        for t in row.tests.iter().filter(|t| done_tests.contains(t.as_str())) {
            match outcome.get(&format!("tests/{t}")).copied() {
                Some("PASS") => {}
                got => problems
                    .entry(row.paper.clone())
                    .or_default()
                    .push(format!("(b) `tests/{t}`: {}", got.unwrap_or("not run"))),
            }
        }
    }

    let count = |s| rows.iter().filter(|r| r.status == s).count();
    println!(
        "xtask features: {} rows: {} done, {} partial, {} todo; {} done-row tests run",
        rows.len(),
        count(FeatureStatus::Done),
        count(FeatureStatus::Partial),
        count(FeatureStatus::Todo),
        done_tests.len()
    );
    if problems.is_empty() {
        println!("xtask features: ok");
        return ExitCode::SUCCESS;
    }
    for row in &rows {
        if let Some(p) = problems.get(&row.paper) {
            println!("FAIL  {} ({}, line {}): {}", row.paper, row.feature, row.line, p.join("; "));
        }
    }
    println!("xtask features: FAILED: {} rows", problems.len());
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let p = util::repo_root().join("xtask/fixtures/features").join(name);
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    }

    #[test]
    fn parses_good_fixture() {
        let rows = parse_table(&fixture("good.md")).expect("parses");
        assert_eq!(rows.len(), 4);
        let r = &rows[0];
        assert_eq!(r.paper, "P1938R3");
        assert_eq!(r.line, 7);
        assert_eq!(r.status, FeatureStatus::Done);
        assert_eq!(r.tests, ["conformance/a.cpp", "conformance/b.cpp"]);
        assert_eq!(
            r.macro_cell,
            Some(MacroCell {
                name: "__cpp_if_consteval".into(),
                kiln: Some("202106L".into()),
                clang22: Some("202106L".into()),
            })
        );
        assert_eq!(rows[1].macro_cell, None);
        assert_eq!(rows[1].status, FeatureStatus::Partial);
        let m = rows[2].macro_cell.as_ref().expect("macro");
        assert_eq!((m.kiln.as_deref(), m.clang22.as_deref()), (None, Some("202403L")));
        // The second table; a `|` inside a code span in Notes does not split.
        assert_eq!(rows[3].paper, "P1967R14");
        assert_eq!(rows[3].status, FeatureStatus::Todo);
        assert!(rows[3].tests.is_empty());
    }

    #[test]
    fn reports_bad_rows_with_lines() {
        let errors = parse_table(&fixture("bad.md")).expect_err("must fail");
        let want = [
            "line 7: `X1` is not a paper number",
            "line 8: P0002R1: status `finished`",
            "line 9: P0003R1: macro cell \"__cpp_x 1 / 1\": expected",
            "line 10: expected 6 cells, found 5",
            "line 11: P0005R1: macro cell \"`FOO` 1 / 1\": `FOO` is not a __cpp_* macro",
            "line 12: P0006R1: macro cell \"`__cpp_y` 1 / soon\": `soon` is neither",
            "line 14: duplicate row for P0007R1",
        ];
        for w in want {
            assert!(errors.iter().any(|e| e.starts_with(w)), "missing `{w}` in {errors:#?}");
        }
        assert_eq!(errors.len(), want.len(), "{errors:#?}");
    }

    #[test]
    fn rejects_text_without_a_table() {
        let errors = parse_table("# nothing\n\n| A | B |\n|---|---|\n").expect_err("no table");
        assert!(errors[0].starts_with("no feature table"), "{errors:?}");
    }

    #[test]
    fn static_checks() {
        let rows = parse_table(&fixture("good.md")).expect("parses");
        let predefs = parse_predefs(
            "#define __cpp_if_consteval 202106L\n#define __cpp_variadic_friend 202403L\n\
             #define F(x) x\n",
        );
        assert_eq!(predefs.len(), 2);
        let problems = check_static(&rows, |t| t != "conformance/missing.cpp", &predefs);
        let papers: Vec<&str> = problems.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(papers, ["P0847R7", "P2893R3"]);
        // A missing test on a partial row is still a problem.
        assert!(problems[0].1[0].contains("tests/conformance/missing.cpp"));
        // The kiln column says undefined; kiln defines it.
        assert!(problems[1].1[0].contains("stale kiln column"), "{problems:?}");

        // A done row whose macro differs from clang 22, and one with no tests.
        let mut done = rows[2].clone();
        done.status = FeatureStatus::Done;
        done.macro_cell.as_mut().expect("macro").kiln = Some("202403L".into());
        let mut empty = rows[0].clone();
        empty.tests.clear();
        let ok_predefs = parse_predefs("#define __cpp_if_consteval 202106L\n");
        let p = check_static(&[done, empty], |_| true, &ok_predefs);
        assert_eq!(p.len(), 2, "{p:?}");
        assert!(p[0].1.iter().any(|m| m.contains("clang 22 as 202403L")), "{p:?}");
        assert_eq!(p[1].1, ["(a) `done` but names no test"]);
    }

    #[test]
    fn checks_the_real_table_parses() {
        let text = std::fs::read_to_string(util::repo_root().join("docs/status/cxx-features.md"))
            .expect("table");
        let rows = parse_table(&text).expect("docs/status/cxx-features.md parses");
        assert!(rows.iter().any(|r| r.paper == "P0847R7"));
        let tests = util::repo_root().join("tests");
        let missing: Vec<_> = rows
            .iter()
            .flat_map(|r| r.tests.iter())
            .filter(|t| !tests.join(t).is_file())
            .collect();
        assert!(missing.is_empty(), "tests named in the table do not exist: {missing:?}");
    }
}
