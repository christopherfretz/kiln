//! `cargo xtask features`: check the standard support table,
//! `docs/STANDARD_SUPPORT.md`, against the corpus and kiln's predefined
//! macros. Contract: `spec/harness.md` "Feature table". A manual / M4-gate
//! check, not part of `ci`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::{test, util};

pub const USAGE: &str = "\
usage: cargo xtask features [options]

Checks docs/STANDARD_SUPPORT.md (spec/harness.md \"Feature table\"):
  (a) every `done` row names at least one test, and every named test exists;
  (b) the tests of `done` rows pass under kiln (the `cargo xtask test` runner);
  (c) for `done` rows with a macro, `kiln++ -std=<the table's standard> -dM -E`
      defines it with the table's clang 22 value (unless clang 22 does not
      define it: a feature kiln ships ahead of clang 22); and on every row the
      table's kiln value is the one kiln defines today;
  (d) the summary table at the top matches the counted sections, and each
      summary row's `-std=` gives its `__cplusplus` value under kiln.

options:
  --compiler <path>   kiln++ binary (default: cargo build -p kiln_driver)
  --table <path>      the table (default: docs/STANDARD_SUPPORT.md)
  --summary           print the per-standard counts and check the summary
                      table only (no kiln build, no test run)
  -h, --help          this text

exit status: 0 all checks pass, 1 a row failed a check, 2 usage, parse or
infrastructure error.
";

/// The default table, relative to the repository root.
const DEFAULT_TABLE: &str = "docs/STANDARD_SUPPORT.md";

/// The table's columns, in order (matched on the header cell's prefix).
const COLUMNS: [&str; 6] =
    ["Paper", "Feature", "`__cpp_*` macro", "Status", "Corpus tests", "Notes"];

/// The summary table's columns, in order (matched on the header cell's prefix).
const SUMMARY_COLUMNS: [&str; 7] =
    ["Standard", "`-std=`", "`__cplusplus`", "done", "partial", "todo", "untested"];

/// The standard a feature table's macros are read at when its header names
/// none.
const DEFAULT_STD: &str = "c++26";

/// A row's status (spec/harness.md "Feature table").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureStatus {
    Done,
    Partial,
    Todo,
    /// Only indirect evidence (the library headers compile); no corpus test.
    Untested,
}

impl FeatureStatus {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "done" => Some(Self::Done),
            "partial" => Some(Self::Partial),
            "todo" => Some(Self::Todo),
            "untested" => Some(Self::Untested),
            _ => None,
        }
    }
}

/// Per-status row counts of one section (or of the summary row naming it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub done: usize,
    pub partial: usize,
    pub todo: usize,
    pub untested: usize,
}

impl Counts {
    fn add(&mut self, s: FeatureStatus) {
        match s {
            FeatureStatus::Done => self.done += 1,
            FeatureStatus::Partial => self.partial += 1,
            FeatureStatus::Todo => self.todo += 1,
            FeatureStatus::Untested => self.untested += 1,
        }
    }

    fn total(&self) -> usize {
        self.done + self.partial + self.todo + self.untested
    }

    fn fields(&self) -> [(&'static str, usize); 4] {
        [
            ("done", self.done),
            ("partial", self.partial),
            ("todo", self.todo),
            ("untested", self.untested),
        ]
    }
}

/// One macro of a macro cell: `` `__cpp_name` KILN / CLANG22 ``, `None`
/// meaning undefined.
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
    /// The text of the `## ` heading the row's table is under.
    pub section: String,
    /// The `-std=` value the row's macros are read at (`c++26` unless the
    /// table's macro header cell names one).
    pub std: String,
    pub paper: String,
    pub feature: String,
    /// The macro cell: empty for `—`, else one or more macros (`<br>`).
    pub macros: Vec<MacroCell>,
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

fn parse_one_macro(cell: &str, part: &str) -> Result<MacroCell, String> {
    let bad = || format!("macro cell \"{cell}\": expected — or `__cpp_name` KILN / CLANG22");
    let rest = part.strip_prefix('`').ok_or_else(bad)?;
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
    Ok(MacroCell { name: name.to_owned(), kiln: value(k)?, clang22: value(c)? })
}

/// `—`, or one or more `` `__cpp_name` KILN / CLANG22 `` separated by `<br>`.
fn parse_macro_cell(cell: &str) -> Result<Vec<MacroCell>, String> {
    if is_dash(cell) {
        return Ok(Vec::new());
    }
    cell.split("<br>").map(|part| parse_one_macro(cell, part.trim())).collect()
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// A row key: a WG21 paper (`P0847R7`, or `N2242` before 2012), a core
/// issue (`CWG178`), a kiln work package (`WP-480`, for kiln extensions),
/// or a stable name (`[except]`, for the C++98 baseline).
fn is_paper(s: &str) -> bool {
    if let Some(rest) = s.strip_prefix('P') {
        return rest.split_once('R').is_some_and(|(n, r)| all_digits(n) && all_digits(r));
    }
    if let Some(rest) = s.strip_prefix("CWG") {
        return all_digits(rest);
    }
    if let Some(rest) = s.strip_prefix("WP-") {
        return all_digits(rest);
    }
    if let Some(rest) = s.strip_prefix('N') {
        return all_digits(rest);
    }
    if let Some(name) = s.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        return name.starts_with(|c: char| c.is_ascii_lowercase())
            && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.');
    }
    false
}

/// The `-std=` value named in a feature table's macro header cell
/// (`` `__cpp_*` macro (kiln / clang 22 at `-std=c++11`) ``).
fn header_std(cell: &str) -> Option<String> {
    let (_, rest) = cell.split_once("-std=")?;
    let v: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '+').collect();
    (!v.is_empty()).then_some(v)
}

/// The text of a `## ` heading line, or `None` for any other line.
fn heading(line: &str) -> Option<&str> {
    line.strip_prefix("## ").map(str::trim)
}

/// `[text](target)` -> `text`; anything else unchanged.
fn link_text(cell: &str) -> &str {
    cell.strip_prefix('[').and_then(|r| r.split_once("](")).map_or(cell, |(t, _)| t)
}

/// Parses every feature table in `text` (a table is a header row whose first
/// cell is `Paper`, a `|---` separator, then rows up to the first line not
/// starting with `|`). Errors carry 1-based line numbers.
pub fn parse_table(text: &str) -> Result<Vec<Row>, Vec<String>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut rows = Vec::new();
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();
    let mut section = String::new();
    let mut i = 0;
    let mut tables = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        if let Some(h) = heading(line) {
            h.clone_into(&mut section);
        }
        if !line.starts_with('|') || split_cells(line).first().map(String::as_str) != Some("Paper")
        {
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
        let std = header.get(2).and_then(|c| header_std(c)).unwrap_or_else(|| DEFAULT_STD.into());
        i += 1;
        if lines.get(i).is_some_and(|l| l.trim().starts_with("|-")) {
            i += 1;
        } else {
            errors.push(format!("line {}: expected the |---| separator row", i + 1));
        }
        while i < lines.len() && lines[i].trim().starts_with('|') {
            match parse_row(lines[i], i + 1) {
                Ok(mut row) => {
                    if !seen.insert(row.paper.clone()) {
                        errors.push(format!("line {}: duplicate row for {}", i + 1, row.paper));
                    }
                    row.section.clone_from(&section);
                    row.std.clone_from(&std);
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
        return Err(format!(
            "`{paper}` is not a paper number like P0847R7 or N2242 (or CWG<n>, WP-<n>, [stable.name])"
        ));
    }
    let status = FeatureStatus::parse(&cells[3]).ok_or_else(|| {
        format!("{paper}: status `{}` is not one of done, partial, todo, untested", cells[3])
    })?;
    Ok(Row {
        line: lineno,
        section: String::new(),
        std: String::new(),
        feature: cells[1].clone(),
        macros: parse_macro_cell(&cells[2]).map_err(|e| format!("{paper}: {e}"))?,
        status,
        tests: code_spans(&cells[4]),
        paper,
    })
}

/// One row of the summary table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryRow {
    pub line: usize,
    /// The section it counts: the text of a `## ` heading (a link's text).
    pub standard: String,
    /// The first `-std=` code span of the `-std=` cell, without `-std=`.
    pub std: Option<String>,
    /// The `__cplusplus` cell's value, `None` for `—`.
    pub cplusplus: Option<String>,
    pub counts: Counts,
}

/// Parses the summary table: a header row whose first cell is `Standard`,
/// the `|---` separator, then one row per section. `Ok(None)` when the text
/// has none.
pub fn parse_summary(text: &str) -> Result<Option<Vec<SummaryRow>>, Vec<String>> {
    let lines: Vec<&str> = text.lines().collect();
    let Some(start) = lines.iter().position(|l| {
        let l = l.trim();
        l.starts_with('|') && split_cells(l).first().map(String::as_str) == Some("Standard")
    }) else {
        return Ok(None);
    };
    let mut errors = Vec::new();
    let header = split_cells(lines[start]);
    if header.len() != SUMMARY_COLUMNS.len()
        || !header.iter().zip(SUMMARY_COLUMNS).all(|(h, c)| h.starts_with(c))
    {
        errors.push(format!(
            "line {}: summary header must be | {} |",
            start + 1,
            SUMMARY_COLUMNS.join(" | ")
        ));
    }
    let mut i = start + 1;
    if lines.get(i).is_some_and(|l| l.trim().starts_with("|-")) {
        i += 1;
    } else {
        errors.push(format!("line {}: expected the |---| separator row", i + 1));
    }
    let mut rows = Vec::new();
    while i < lines.len() && lines[i].trim().starts_with('|') {
        let lineno = i + 1;
        let cells = split_cells(lines[i]);
        i += 1;
        if cells.len() != SUMMARY_COLUMNS.len() {
            errors.push(format!(
                "line {lineno}: summary row: expected {} cells, found {}",
                SUMMARY_COLUMNS.len(),
                cells.len()
            ));
            continue;
        }
        let mut n = [0usize; 4];
        let mut ok = true;
        for (k, slot) in n.iter_mut().enumerate() {
            if let Ok(v) = cells[3 + k].parse() {
                *slot = v;
            } else {
                errors.push(format!(
                    "line {lineno}: summary row: `{}` count `{}` is not a number",
                    SUMMARY_COLUMNS[3 + k],
                    cells[3 + k]
                ));
                ok = false;
            }
        }
        let cplusplus = if is_dash(&cells[2]) {
            None
        } else {
            match code_spans(&cells[2]).first().or(Some(&cells[2])).map(|v| v.trim().to_owned()) {
                Some(v) if is_macro_value(&v) => Some(v),
                _ => {
                    errors.push(format!(
                        "line {lineno}: summary row: `__cplusplus` cell `{}` is not a value",
                        cells[2]
                    ));
                    ok = false;
                    None
                }
            }
        };
        if !ok {
            continue;
        }
        rows.push(SummaryRow {
            line: lineno,
            standard: link_text(&cells[0]).to_owned(),
            std: code_spans(&cells[1])
                .iter()
                .find_map(|s| s.strip_prefix("-std=").filter(|v| !v.is_empty()).map(str::to_owned)),
            cplusplus,
            counts: Counts { done: n[0], partial: n[1], todo: n[2], untested: n[3] },
        });
    }
    if errors.is_empty() { Ok(Some(rows)) } else { Err(errors) }
}

/// Counts the rows of every status-bearing table (a header row with a
/// `Status` cell; not the summary table) per `## ` section, in file order.
/// Feature tables and the per-work-package tables (C++26 reflection) both
/// count.
pub fn section_counts(text: &str) -> Result<Vec<(String, usize, Counts)>, Vec<String>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<(String, usize, Counts)> = Vec::new();
    let mut errors = Vec::new();
    let mut section = (String::new(), 0usize);
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        if let Some(h) = heading(line) {
            section = (h.to_owned(), i + 1);
        }
        let is_header =
            line.starts_with('|') && lines.get(i + 1).is_some_and(|l| l.trim().starts_with("|-"));
        if !is_header {
            i += 1;
            continue;
        }
        let header = split_cells(line);
        let col = header.iter().position(|c| c == "Status");
        i += 2;
        let Some(col) = col.filter(|_| header[0] != "Standard") else {
            while i < lines.len() && lines[i].trim().starts_with('|') {
                i += 1;
            }
            continue;
        };
        let slot = if let Some(k) = out.iter().position(|(s, _, _)| *s == section.0) {
            k
        } else {
            out.push((section.0.clone(), section.1, Counts::default()));
            out.len() - 1
        };
        while i < lines.len() && lines[i].trim().starts_with('|') {
            let cells = split_cells(lines[i]);
            match cells.get(col).and_then(|c| FeatureStatus::parse(c)) {
                Some(s) => out[slot].2.add(s),
                None => errors.push(format!(
                    "line {}: status `{}` is not one of done, partial, todo, untested",
                    i + 1,
                    cells.get(col).map_or("", String::as_str)
                )),
            }
            i += 1;
        }
    }
    if errors.is_empty() { Ok(out) } else { Err(errors) }
}

/// Check (d), static half: the summary table names each counted section
/// once, with its counts. Returns one problem per stale or missing row.
pub fn check_summary(summary: &[SummaryRow], sections: &[(String, usize, Counts)]) -> Vec<String> {
    let mut p = Vec::new();
    let mut seen = BTreeSet::new();
    for s in summary {
        match sections.iter().find(|(name, _, _)| *name == s.standard) {
            None => p.push(format!(
                "summary row `{}` (line {}): no `## {}` section with a status table",
                s.standard, s.line, s.standard
            )),
            Some((_, _, c)) => {
                for ((field, want), (_, got)) in c.fields().into_iter().zip(s.counts.fields()) {
                    if want != got {
                        p.push(format!(
                            "summary row `{}` (line {}): {field} is {got}, the section counts {want}",
                            s.standard, s.line
                        ));
                    }
                }
            }
        }
        if !seen.insert(s.standard.as_str()) {
            p.push(format!("summary row `{}` (line {}): duplicate", s.standard, s.line));
        }
    }
    for (name, line, c) in sections {
        if c.total() > 0 && !summary.iter().any(|s| s.standard == *name) {
            p.push(format!("section `{name}` (line {line}) has no summary row"));
        }
    }
    p
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
/// under `tests/` exists; `predefs` maps each row's `std` to kiln's `-dM -E`
/// output at that standard (a row whose standard is missing compares against
/// no macros). Returns the problems per row (paper -> messages), rows in
/// table order.
pub fn check_static(
    rows: &[Row],
    exists: impl Fn(&str) -> bool,
    predefs: &BTreeMap<String, BTreeMap<String, String>>,
) -> Vec<(String, Vec<String>)> {
    let none = BTreeMap::new();
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
        let defs = predefs.get(&row.std).unwrap_or(&none);
        for m in &row.macros {
            let actual = defs.get(&m.name);
            // A macro clang 22 does not define is a feature kiln ships
            // ahead of it (its row carries the written reason).
            if row.status == FeatureStatus::Done
                && m.clang22.is_some()
                && actual != m.clang22.as_ref()
            {
                p.push(format!(
                    "(c) at -std={} kiln defines `{}` as {}, clang 22 as {}",
                    row.std,
                    m.name,
                    show(actual),
                    show(m.clang22.as_ref())
                ));
            }
            if actual != m.kiln.as_ref() {
                p.push(format!(
                    "(c) stale kiln column: at -std={} kiln defines `{}` as {}, the table says {}",
                    row.std,
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

/// Check (d), kiln half: each summary row's `-std=` gives its `__cplusplus`.
pub fn check_cplusplus(
    summary: &[SummaryRow],
    predefs: &BTreeMap<String, BTreeMap<String, String>>,
) -> Vec<String> {
    let mut p = Vec::new();
    for s in summary {
        let (Some(std), Some(want)) = (&s.std, &s.cplusplus) else { continue };
        let got = predefs.get(std).and_then(|d| d.get("__cplusplus"));
        if got != Some(want) {
            p.push(format!(
                "summary row `{}` (line {}): at -std={std} kiln defines `__cplusplus` as {}, the table says {want}",
                s.standard,
                s.line,
                show(got)
            ));
        }
    }
    p
}

fn show(v: Option<&String>) -> String {
    v.map_or_else(|| "undefined".to_owned(), Clone::clone)
}

struct Args {
    compiler: Option<PathBuf>,
    table: Option<PathBuf>,
    summary: bool,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Option<Args>, String> {
    let mut a = Args { compiler: None, table: None, summary: false };
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
            "--summary" => a.summary = true,
            "-h" | "--help" => return Ok(None),
            _ => return Err(format!("unknown argument `{arg}`")),
        }
    }
    Ok(Some(a))
}

fn kiln_predefs(kiln: &Path, std: &str) -> Result<BTreeMap<String, String>, String> {
    let flag = format!("-std={std}");
    let o = Command::new(kiln)
        .args([flag.as_str(), "-dM", "-E", "-x", "c++", "/dev/null"])
        .output()
        .map_err(|e| format!("cannot run {}: {e}", kiln.display()))?;
    if !o.status.success() {
        return Err(format!(
            "`{} {flag} -dM -E` failed ({}): {}",
            kiln.display(),
            o.status,
            String::from_utf8_lossy(&o.stderr)
        ));
    }
    Ok(parse_predefs(&String::from_utf8_lossy(&o.stdout)))
}

/// Check (b): one runner invocation over every existing test of a `done`
/// row; each must PASS. Problems are added per paper; returns the number of
/// tests run.
fn check_done_tests(
    rows: &[Row],
    tests_dir: &Path,
    kiln: PathBuf,
    problems: &mut BTreeMap<String, Vec<String>>,
) -> Result<usize, String> {
    let done_tests: BTreeSet<&str> = rows
        .iter()
        .filter(|r| r.status == FeatureStatus::Done)
        .flat_map(|r| r.tests.iter().map(String::as_str))
        .filter(|t| tests_dir.join(t).is_file())
        .collect();
    let results = test::execute(&test::Options {
        compiler: Some(kiln),
        paths: done_tests.iter().map(|t| tests_dir.join(t)).collect(),
        ..Default::default()
    })?;
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
    Ok(done_tests.len())
}

fn print_counts(sections: &[(String, usize, Counts)]) {
    for (name, _, c) in sections {
        println!(
            "  {name}: {} rows: {} done, {} partial, {} todo, {} untested",
            c.total(),
            c.done,
            c.partial,
            c.todo,
            c.untested
        );
    }
}

/// `--summary`: print the per-section counts and the summary problems.
fn summary_mode(
    table: &Path,
    sections: &[(String, usize, Counts)],
    problems: &[String],
) -> ExitCode {
    println!("xtask features: {}", table.display());
    print_counts(sections);
    for p in problems {
        println!("FAIL  {p}");
    }
    if problems.is_empty() {
        println!("xtask features: summary ok");
        ExitCode::SUCCESS
    } else {
        println!("xtask features: FAILED: stale summary table");
        ExitCode::FAILURE
    }
}

/// kiln's predefines at every standard a table or a summary row names.
fn all_predefs(
    kiln: &Path,
    rows: &[Row],
    summary: &[SummaryRow],
) -> Result<BTreeMap<String, BTreeMap<String, String>>, String> {
    let stds: BTreeSet<String> = rows
        .iter()
        .map(|r| r.std.clone())
        .chain(summary.iter().filter_map(|s| s.std.clone()))
        .collect();
    stds.into_iter().map(|std| Ok((std.clone(), kiln_predefs(kiln, &std)?))).collect()
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
    let table = args.table.unwrap_or_else(|| root.join(DEFAULT_TABLE));
    let text = match std::fs::read_to_string(&table) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("xtask features: cannot read {}: {e}", table.display());
            return ExitCode::from(2);
        }
    };
    let parsed = parse_table(&text).and_then(|rows| {
        let summary = parse_summary(&text)?.unwrap_or_default();
        Ok((rows, summary, section_counts(&text)?))
    });
    let (rows, summary, sections) = match parsed {
        Ok(p) => p,
        Err(errors) => {
            for e in &errors {
                eprintln!("xtask features: {}: {e}", table.display());
            }
            return ExitCode::from(2);
        }
    };
    let mut summary_problems = check_summary(&summary, &sections);
    if args.summary {
        return summary_mode(&table, &sections, &summary_problems);
    }
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
    let predefs = match all_predefs(&kiln, &rows, &summary) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("xtask features: {e}");
            return ExitCode::from(2);
        }
    };
    summary_problems.extend(check_cplusplus(&summary, &predefs));
    let tests_dir = root.join("tests");
    let mut problems: BTreeMap<String, Vec<String>> =
        check_static(&rows, |t| tests_dir.join(t).is_file(), &predefs).into_iter().collect();

    let ran = match check_done_tests(&rows, &tests_dir, kiln, &mut problems) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("xtask features: {e}");
            return ExitCode::from(2);
        }
    };

    let mut total = Counts::default();
    for r in &rows {
        total.add(r.status);
    }
    println!(
        "xtask features: {} rows: {} done, {} partial, {} todo, {} untested; {} done-row tests run",
        rows.len(),
        total.done,
        total.partial,
        total.todo,
        total.untested,
        ran
    );
    if problems.is_empty() && summary_problems.is_empty() {
        println!("xtask features: ok");
        return ExitCode::SUCCESS;
    }
    for row in &rows {
        if let Some(p) = problems.get(&row.paper) {
            println!("FAIL  {} ({}, line {}): {}", row.paper, row.feature, row.line, p.join("; "));
        }
    }
    for p in &summary_problems {
        println!("FAIL  {p}");
    }
    println!(
        "xtask features: FAILED: {} rows, {} summary problems",
        problems.len(),
        summary_problems.len()
    );
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let p = util::repo_root().join("xtask/fixtures/features").join(name);
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    }

    fn at26(defs: &str) -> BTreeMap<String, BTreeMap<String, String>> {
        BTreeMap::from([("c++26".to_owned(), parse_predefs(defs))])
    }

    #[test]
    fn parses_good_fixture() {
        let rows = parse_table(&fixture("good.md")).expect("parses");
        assert_eq!(rows.len(), 7);
        let r = &rows[0];
        assert_eq!(r.paper, "P1938R3");
        assert_eq!(r.line, 7);
        assert_eq!((r.section.as_str(), r.std.as_str()), ("First", "c++26"));
        assert_eq!(r.status, FeatureStatus::Done);
        assert_eq!(r.tests, ["conformance/a.cpp", "conformance/b.cpp"]);
        assert_eq!(
            r.macros,
            [MacroCell {
                name: "__cpp_if_consteval".into(),
                kiln: Some("202106L".into()),
                clang22: Some("202106L".into()),
            }]
        );
        assert_eq!(rows[1].macros, []);
        assert_eq!(rows[1].status, FeatureStatus::Partial);
        let m = &rows[2].macros[0];
        assert_eq!((m.kiln.as_deref(), m.clang22.as_deref()), (None, Some("202403L")));
        // The second table; a `|` inside a code span in Notes does not split.
        assert_eq!(rows[3].paper, "P1967R14");
        assert_eq!(rows[3].status, FeatureStatus::Todo);
        assert_eq!(rows[3].tests, Vec::<String>::new());
        // The third: `-std=c++11` in the header, N-papers, two macros, untested.
        let r = &rows[4];
        assert_eq!(
            (r.paper.as_str(), r.section.as_str(), r.std.as_str()),
            ("N2442", "C++11", "c++11")
        );
        let names: Vec<&str> = r.macros.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["__cpp_raw_strings", "__cpp_unicode_literals"]);
        assert_eq!(r.macros[0].clang22.as_deref(), Some("200710"));
        assert_eq!(rows[5].status, FeatureStatus::Untested);
        assert_eq!((rows[6].paper.as_str(), rows[6].std.as_str()), ("[except]", "c++11"));
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
            "line 15: P0008R1: macro cell \"`__cpp_a` 1 / 1<br>__cpp_b 2 / 2\": expected",
            "line 16: `[Bad Name]` is not a paper number",
        ];
        for w in want {
            assert!(errors.iter().any(|e| e.starts_with(w)), "missing `{w}` in {errors:#?}");
        }
        assert_eq!(errors.len(), want.len(), "{errors:#?}");
    }

    #[test]
    fn paper_keys() {
        for ok in ["P0847R7", "N2242", "CWG178", "WP-480", "[except]", "[expr.typeid]"] {
            assert!(is_paper(ok), "{ok}");
        }
        for bad in ["P0847", "N", "NR1", "CWG", "WP-", "WP480", "[]", "[1x]", "[Except]", "X1"] {
            assert!(!is_paper(bad), "{bad}");
        }
    }

    #[test]
    fn rejects_text_without_a_table() {
        let errors = parse_table("# nothing\n\n| A | B |\n|---|---|\n").expect_err("no table");
        assert!(errors[0].starts_with("no feature table"), "{errors:?}");
    }

    #[test]
    fn static_checks() {
        let rows = parse_table(&fixture("good.md")).expect("parses");
        let predefs = at26(
            "#define __cpp_if_consteval 202106L\n#define __cpp_variadic_friend 202403L\n\
             #define F(x) x\n",
        );
        assert_eq!(predefs["c++26"].len(), 2);
        let first: Vec<Row> = rows.iter().filter(|r| r.std == "c++26").cloned().collect();
        let problems = check_static(&first, |t| t != "conformance/missing.cpp", &predefs);
        let papers: Vec<&str> = problems.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(papers, ["P0847R7", "P2893R3"]);
        // A missing test on a partial row is still a problem.
        assert!(problems[0].1[0].contains("tests/conformance/missing.cpp"));
        // The kiln column says undefined; kiln defines it.
        assert!(problems[1].1[0].contains("stale kiln column"), "{problems:?}");

        // A done row whose macro differs from clang 22, and one with no tests.
        let mut done = rows[2].clone();
        done.status = FeatureStatus::Done;
        done.macros[0].kiln = Some("202403L".into());
        let mut empty = rows[0].clone();
        empty.tests.clear();
        let ok_predefs = at26("#define __cpp_if_consteval 202106L\n");
        let p = check_static(&[done, empty], |_| true, &ok_predefs);
        assert_eq!(p.len(), 2, "{p:?}");
        assert!(p[0].1.iter().any(|m| m.contains("clang 22 as 202403L")), "{p:?}");
        assert_eq!(p[1].1, ["(a) `done` but names no test"]);

        // A done row kiln ships ahead of clang 22: only the kiln column counts.
        let mut ahead = rows[2].clone();
        ahead.status = FeatureStatus::Done;
        let cell = &mut ahead.macros[0];
        cell.clang22 = None;
        cell.kiln = Some("202411L".into());
        let name = cell.name.clone();
        let defs = at26(&format!("#define {name} 202411L\n"));
        let none: Vec<_> = Vec::new();
        assert_eq!(check_static(std::slice::from_ref(&ahead), |_| true, &defs), none);
        let stale = at26(&format!("#define {name} 202403L\n"));
        let p = check_static(&[ahead], |_| true, &stale);
        assert!(p[0].1.iter().any(|m| m.contains("stale kiln column")), "{p:?}");
    }

    #[test]
    fn macros_are_read_at_the_tables_standard() {
        let rows = parse_table(&fixture("good.md")).expect("parses");
        let n2442 = rows.iter().find(|r| r.paper == "N2442").expect("row").clone();
        // At c++11 both macros match; the c++26 set (where they are absent)
        // is not consulted.
        let mut predefs = at26("");
        predefs.insert(
            "c++11".into(),
            parse_predefs(
                "#define __cpp_raw_strings 200710\n#define __cpp_unicode_literals 200710\n",
            ),
        );
        assert_eq!(check_static(std::slice::from_ref(&n2442), |_| true, &predefs), vec![]);
        // One of two macros wrong: one problem names it and the standard.
        predefs.insert("c++11".into(), parse_predefs("#define __cpp_raw_strings 200710\n"));
        let p = check_static(&[n2442], |_| true, &predefs);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(
            p[0].1.iter().all(|m| m.contains("-std=c++11") && m.contains("__cpp_unicode_literals")),
            "{p:?}"
        );
    }

    /// A small inline table: summary, two counted sections, a per-WP table
    /// that counts, and a table without `Status` that does not.
    const SUMMARY_DOC: &str = "\
# Doc

| Standard | `-std=` | `__cplusplus` | done | partial | todo | untested |
|---|---|---|---|---|---|---|
| [C++11](#c11) | `-std=c++11` | `201103L` | 1 | 1 | 0 | 0 |
| C++26 reflection | `-std=c++26` `-freflection` | — | 2 | 0 | 0 | 0 |

## C++11

| Paper | Feature | `__cpp_*` macro (kiln / clang 22 at `-std=c++11`) | Status | Corpus tests | Notes |
|---|---|---|---|---|---|
| N2242 | Variadic templates | — | done | `conformance/a.cpp` | |
| N2672 | Initializer lists | — | partial | | |

## C++26 reflection

| WP | Scope | Status | Tests | Notes |
|---|---|---|---|---|
| WP-470 | a | done | | |
| WP-471 | b | done | | |

| Macro | kiln | clang 22 |
|---|---|---|
| `__cpp_x` | 1 | 1 |
";

    #[test]
    fn summary_matches_sections() {
        let summary = parse_summary(SUMMARY_DOC).expect("parses").expect("has a summary");
        assert_eq!(summary.len(), 2);
        assert_eq!(summary[0].standard, "C++11");
        assert_eq!(summary[0].std.as_deref(), Some("c++11"));
        assert_eq!(summary[0].cplusplus.as_deref(), Some("201103L"));
        assert_eq!(summary[1].cplusplus, None);
        let sections = section_counts(SUMMARY_DOC).expect("counts");
        let names: Vec<&str> = sections.iter().map(|(n, _, _)| n.as_str()).collect();
        assert_eq!(names, ["C++11", "C++26 reflection"]);
        assert_eq!(check_summary(&summary, &sections), Vec::<String>::new());

        // A stale count names the summary row and the field.
        let stale = SUMMARY_DOC.replace("`201103L` | 1 | 1 |", "`201103L` | 2 | 1 |");
        let s = parse_summary(&stale).expect("parses").expect("summary");
        let p = check_summary(&s, &section_counts(&stale).expect("counts"));
        assert_eq!(p, ["summary row `C++11` (line 5): done is 2, the section counts 1"]);

        // A section without a summary row, and a summary row without a section.
        let renamed = SUMMARY_DOC.replace("| C++26 reflection |", "| C++26 reflections |");
        let s = parse_summary(&renamed).expect("parses").expect("summary");
        let p = check_summary(&s, &section_counts(&renamed).expect("counts"));
        assert_eq!(p.len(), 2, "{p:?}");
        assert!(p[0].starts_with("summary row `C++26 reflections` (line 6): no `## C++26"));
        assert!(p[1].starts_with("section `C++26 reflection` (line 15) has no summary row"));

        // __cplusplus under kiln.
        let mut predefs = BTreeMap::new();
        predefs.insert("c++11".to_owned(), parse_predefs("#define __cplusplus 201103L\n"));
        assert_eq!(check_cplusplus(&summary, &predefs), Vec::<String>::new());
        predefs.insert("c++11".to_owned(), parse_predefs("#define __cplusplus 199711L\n"));
        let p = check_cplusplus(&summary, &predefs);
        assert_eq!(p.len(), 1);
        assert!(p[0].contains("as 199711L, the table says 201103L"), "{p:?}");

        // A bare `-std=` span ("every `-std=`") names no standard to query.
        let every = SUMMARY_DOC.replace("`-std=c++26` `-freflection`", "every `-std=`");
        let s = parse_summary(&every).expect("parses").expect("summary");
        assert_eq!(s[1].std, None);
    }

    #[test]
    fn summary_parse_errors() {
        let bad = SUMMARY_DOC.replace("| 1 | 1 | 0 | 0 |", "| one | 1 | 0 | 0 |");
        let e = parse_summary(&bad).expect_err("bad count");
        assert!(e[0].starts_with("line 5: summary row: `done` count `one`"), "{e:?}");
        assert_eq!(parse_summary("# no summary\n"), Ok(None));
        let bad = SUMMARY_DOC.replace("| WP-471 | b | done |", "| WP-471 | b | finished |");
        let e = section_counts(&bad).expect_err("bad status");
        assert!(e[0].starts_with("line 20: status `finished`"), "{e:?}");
    }

    #[test]
    fn checks_the_real_table_parses() {
        let text = std::fs::read_to_string(util::repo_root().join(DEFAULT_TABLE)).expect("table");
        let rows = parse_table(&text).expect("docs/STANDARD_SUPPORT.md parses");
        assert!(rows.iter().any(|r| r.paper == "P0847R7"));
        let tests = util::repo_root().join("tests");
        let missing: Vec<_> =
            rows.iter().flat_map(|r| r.tests.iter()).filter(|t| !tests.join(t).is_file()).collect();
        assert!(missing.is_empty(), "tests named in the table do not exist: {missing:?}");
        let summary = parse_summary(&text).expect("summary parses").expect("has a summary");
        let sections = section_counts(&text).expect("sections count");
        assert_eq!(check_summary(&summary, &sections), Vec::<String>::new());
    }
}
