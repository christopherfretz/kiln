//! `cargo xtask diag-split`: keeps `kiln_diag`'s diagnostic-id and
//! warning-group tables split into one file per area, each sorted by name,
//! so work packages adding rows in parallel touch different lines
//! (spec/diag.md §2.4).
//!
//! The rules, all derived from file names so nothing here needs editing
//! when an area is added or split:
//! - An id `P_rest` lives in `ids/P.rs`, or, when the prefix is split by
//!   name range, in the `ids/P_<lo>_<hi>.rs` whose `lo` is the greatest one
//!   not above the first character of `rest` (the first range also takes
//!   anything below its `lo`). A prefix with no file yet gets `ids/P.rs`.
//! - A group with subgroups, and `Pedantic`, lives in `groups/umbrellas.rs`;
//!   any other group in `groups/P.rs` for a prefix `P` of one of the ids
//!   naming it (it stays where it is while that holds, else it goes to its
//!   most common prefix), or in `groups/misc.rs` when no id names it.
//! - Rows are sorted by name; a comment directly above a row belongs to it.
//! - Rows in the legacy files' tail blocks (`diag_id_table! { ... }` at the
//!   end of `ids.rs`, `diag_group_table! { ... }` in `groups.rs`) or in a
//!   `_pending.rs` compile but are moved into their area file by this tool;
//!   `--check` fails while any are left.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::Path;
use std::process::{Command, ExitCode};

const IDS_RS: &str = "crates/kiln_diag/src/ids.rs";
const GROUPS_RS: &str = "crates/kiln_diag/src/groups.rs";
const IDS_DIR: &str = "crates/kiln_diag/src/ids";
const GROUPS_DIR: &str = "crates/kiln_diag/src/groups";
const PENDING: &str = "_pending";
const UMBRELLAS: &str = "umbrellas";
const MISC: &str = "misc";

const USAGE: &str = "\
usage: cargo xtask diag-split [--check | --replay [<base> <head>]]

Keeps the kiln_diag id and group tables split per area and sorted
(spec/diag.md §2.4).

  (no option)   move rows from the legacy tails of ids.rs/groups.rs and from
                ids/_pending.rs, groups/_pending.rs into their area files and
                normalize every area file (idempotent)
  --check       fail if any row is outside its area file, a tail or pending
                file is non-empty, or an area file is not in canonical form
                (`cargo xtask ci` runs this)
  --replay      re-apply the table edits a branch made between <base> and
                <head> onto the working tree's tables, by row name. With no
                revisions it must run during a merge: <base> is the merge
                base of HEAD and MERGE_HEAD, <head> is HEAD, and conflicted
                or pre-split ids.rs/groups.rs are first reset to MERGE_HEAD's.
                This is how a branch from before the split merges main.
";

/// Which of the two tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Table {
    Ids,
    Groups,
}

impl Table {
    const BOTH: [Table; 2] = [Table::Ids, Table::Groups];

    fn legacy(self) -> &'static str {
        match self {
            Table::Ids => IDS_RS,
            Table::Groups => GROUPS_RS,
        }
    }

    fn dir(self) -> &'static str {
        match self {
            Table::Ids => IDS_DIR,
            Table::Groups => GROUPS_DIR,
        }
    }

    /// The line opening the tail block in the legacy file.
    fn tail_opener(self) -> &'static str {
        match self {
            Table::Ids => "diag_id_table! {",
            Table::Groups => "diag_group_table! {",
        }
    }

    /// The line opening the whole table in the legacy file before the split.
    fn old_opener(self) -> &'static str {
        match self {
            Table::Ids => "diag_ids! {",
            Table::Groups => "diag_groups! {",
        }
    }
}

/// One table row with the comment lines directly above it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Row {
    name: String,
    comments: Vec<String>,
    text: String,
    /// Where the row was read from: an area file's stem, or `None` for a
    /// legacy tail, a pending file or another revision.
    stem: Option<String>,
}

impl Row {
    fn parse(text: &str, comments: Vec<String>, stem: Option<String>) -> Result<Row, String> {
        let name = text.split([' ', '=']).next().unwrap_or_default();
        let ok = !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && text.contains(" = ")
            && text.ends_with(';');
        if !ok {
            return Err(format!("not a table row: `{text}`"));
        }
        Ok(Row { name: name.to_owned(), comments, text: text.to_owned(), stem })
    }

    /// For an id row, the group in `[...]` after the kind.
    fn id_group(&self) -> Option<&str> {
        let rest = self.text.split_once(" = ")?.1;
        let mut words = rest.split_whitespace();
        words.next()?;
        words.next()?.strip_prefix('[')?.strip_suffix(']')
    }

    /// For a group row, its option name and subgroups.
    fn group_parts(&self) -> Result<(String, Vec<String>), String> {
        let bad = || format!("malformed group row: `{}`", self.text);
        let rest = self.text.split_once(" = ").ok_or_else(bad)?.1;
        let rest = rest.strip_prefix('"').ok_or_else(bad)?;
        let (option, rest) = rest.split_once('"').ok_or_else(bad)?;
        let subs =
            rest.trim().strip_prefix('[').and_then(|r| r.strip_suffix("];")).ok_or_else(bad)?;
        let subs = subs.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned);
        Ok((option.to_owned(), subs.collect()))
    }
}

fn group_text(name: &str, option: &str, subs: &[String]) -> String {
    format!("{name} = \"{option}\" [{}];", subs.join(", "))
}

/// Parses rows from `lines`. A comment block at the start followed by a
/// blank line is the file header and is dropped; other comments not
/// directly above a row are dropped with a warning.
fn parse_rows(
    lines: &[&str],
    stem: Option<&str>,
    what: &str,
    warnings: &mut Vec<String>,
) -> Result<Vec<Row>, String> {
    let mut rows = Vec::new();
    let mut comments: Vec<String> = Vec::new();
    let mut header = true;
    for line in lines {
        let t = line.trim();
        if t.is_empty() {
            if !comments.is_empty() && !header {
                warnings
                    .push(format!("{what}: dropped a comment not above a row: {}", comments[0]));
            }
            comments.clear();
            header = false;
        } else if t.starts_with("//") {
            comments.push(t.to_owned());
        } else {
            header = false;
            let row = Row::parse(t, std::mem::take(&mut comments), stem.map(str::to_owned))
                .map_err(|e| format!("{what}: {e}"))?;
            rows.push(row);
        }
    }
    if !comments.is_empty() && !header {
        warnings.push(format!("{what}: dropped a trailing comment: {}", comments[0]));
    }
    Ok(rows)
}

/// The line ranges (exclusive of the brace lines) of the row blocks in a
/// legacy file: the tail block and, before the split, the whole table.
/// `old` is set when a pre-split table block was found.
fn blocks(src: &str, table: Table) -> Result<(Vec<std::ops::Range<usize>>, bool), String> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    let mut old = false;
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i].trim_end();
        if l == table.tail_opener() || l == table.old_opener() {
            old |= l == table.old_opener();
            let end = (i + 1..lines.len())
                .find(|&j| lines[j].trim_end() == "}")
                .ok_or_else(|| format!("{}: `{l}` block is not closed", table.legacy()))?;
            out.push(i + 1..end);
            i = end;
        }
        i += 1;
    }
    Ok((out, old))
}

/// Rows of every block in a legacy file's text.
fn legacy_rows(
    src: &str,
    table: Table,
    warnings: &mut Vec<String>,
) -> Result<(Vec<Row>, bool), String> {
    let (ranges, old) = blocks(src, table)?;
    let lines: Vec<&str> = src.lines().collect();
    let mut rows = Vec::new();
    for r in ranges {
        let what = format!("{} (tail)", table.legacy());
        rows.extend(parse_rows(&lines[r], None, &what, warnings)?);
    }
    Ok((rows, old))
}

/// `src` with its row blocks removed: the hand-written code around them.
fn strip_blocks(src: &str, table: Table) -> Result<String, String> {
    let (ranges, _) = blocks(src, table)?;
    let lines: Vec<&str> = src.lines().collect();
    let keep = (0..lines.len()).filter(|i| !ranges.iter().any(|r| r.contains(i)));
    Ok(keep.map(|i| lines[i]).collect::<Vec<_>>().join("\n"))
}

/// The id prefix of a name: the text before the first `_`.
fn id_prefix(name: &str) -> Result<&str, String> {
    match name.split_once('_') {
        Some((p, rest))
            if !p.is_empty()
                && !rest.is_empty()
                && p.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) =>
        {
            Ok(p)
        }
        _ => Err(format!("id `{name}` has no `<prefix>_` (spec/diag.md §2.1)")),
    }
}

/// The id area files: prefix to its single file or its name ranges.
#[derive(Debug, Default)]
struct IdAreas {
    single: BTreeSet<String>,
    ranges: BTreeMap<String, Vec<(char, String)>>,
}

impl IdAreas {
    fn new<'a>(stems: impl IntoIterator<Item = &'a str>) -> Result<IdAreas, String> {
        let mut a = IdAreas::default();
        for stem in stems {
            let parts: Vec<&str> = stem.split('_').collect();
            let ch = |s: &str| {
                let mut c = s.chars();
                match (c.next(), c.next()) {
                    (Some(c), None) if c.is_ascii_lowercase() || c.is_ascii_digit() => Some(c),
                    _ => None,
                }
            };
            match parts[..] {
                [p] if id_prefix(&format!("{p}_x")).is_ok() => {
                    a.single.insert(p.to_owned());
                }
                [p, lo, hi] if ch(lo).is_some() && ch(hi) >= ch(lo) => {
                    a.ranges
                        .entry(p.to_owned())
                        .or_default()
                        .push((ch(lo).unwrap_or('a'), stem.to_owned()));
                }
                _ => {
                    return Err(format!(
                        "{IDS_DIR}/{stem}.rs: an id area file is `<prefix>.rs` or `<prefix>_<lo>_<hi>.rs`"
                    ));
                }
            }
        }
        for (p, r) in &mut a.ranges {
            if a.single.contains(p) {
                return Err(format!("{IDS_DIR}: both `{p}.rs` and `{p}_<lo>_<hi>.rs` files exist"));
            }
            r.sort();
            for w in r.windows(2) {
                let hi = w[0].1.rsplit('_').next().and_then(|h| h.chars().next()).unwrap_or('z');
                if char::from_u32(hi as u32 + 1) != Some(w[1].0) {
                    return Err(format!(
                        "{IDS_DIR}: ranges `{}` and `{}` are not contiguous",
                        w[0].1, w[1].1
                    ));
                }
            }
        }
        Ok(a)
    }

    /// The stem of the file id `name` belongs in.
    fn file_of(&self, name: &str) -> Result<String, String> {
        let p = id_prefix(name)?;
        let Some(ranges) = self.ranges.get(p) else { return Ok(p.to_owned()) };
        let first = name[p.len() + 1..].chars().next().unwrap_or('a');
        let pick = ranges.iter().rev().find(|(lo, _)| *lo <= first).unwrap_or(&ranges[0]);
        Ok(pick.1.clone())
    }
}

/// The stem of the file group `row` belongs in, given the prefixes of the
/// ids naming each group.
fn group_file(
    row: &Row,
    members: &HashMap<String, BTreeMap<String, usize>>,
) -> Result<String, String> {
    let (_, subs) = row.group_parts()?;
    if !subs.is_empty() || row.name == "Pedantic" {
        return Ok(UMBRELLAS.to_owned());
    }
    let Some(prefixes) = members.get(&row.name).filter(|m| !m.is_empty()) else {
        return Ok(MISC.to_owned());
    };
    if let Some(cur) = row.stem.as_deref().filter(|s| prefixes.contains_key(*s)) {
        return Ok(cur.to_owned());
    }
    // Most ids; on a tie the first prefix in name order.
    let best = prefixes.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)));
    Ok(best.map_or_else(|| MISC.to_owned(), |(p, _)| p.clone()))
}

fn header(table: Table, stem: &str) -> String {
    let what = match table {
        Table::Ids => {
            let parts: Vec<&str> = stem.split('_').collect();
            match parts[..] {
                [p, lo, hi] => format!("Diagnostic ids `{p}_{lo}*` to `{p}_{hi}*`"),
                _ => format!("Diagnostic ids `{stem}_*`"),
            }
        }
        Table::Groups if stem == UMBRELLAS => {
            "Warning groups with subgroups, and `Pedantic`".to_owned()
        }
        Table::Groups if stem == MISC => "Warning groups no id names yet".to_owned(),
        Table::Groups => format!("Warning groups named by `{stem}_*` ids"),
    };
    format!(
        "// {what} (spec/diag.md §2.4).\n\
         // One row per line, sorted by name; a comment directly above a row\n\
         // belongs to it. `cargo xtask diag-split` maintains this layout.\n\n"
    )
}

fn render(table: Table, stem: &str, rows: &[Row]) -> String {
    let mut s = header(table, stem);
    for r in rows {
        for c in &r.comments {
            let _ = writeln!(s, "{c}");
        }
        let _ = writeln!(s, "{}", r.text);
    }
    s
}

/// Sorts rows into area files: stem to rows, sorted by name. Fails on a
/// name defined twice with different text (an identical duplicate, as a
/// tail row already moved, is dropped).
fn layout(
    ids: Vec<Row>,
    groups: Vec<Row>,
    id_stems: &[String],
) -> Result<[BTreeMap<String, Vec<Row>>; 2], Vec<String>> {
    let mut errors = Vec::new();
    let areas = match IdAreas::new(id_stems.iter().map(String::as_str).filter(|s| *s != PENDING)) {
        Ok(a) => a,
        Err(e) => return Err(vec![e]),
    };
    let ids = dedup(ids, &mut errors);
    let groups = dedup(groups, &mut errors);
    let mut members: HashMap<String, BTreeMap<String, usize>> = HashMap::new();
    let mut id_files: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    for r in ids {
        match areas.file_of(&r.name) {
            Ok(stem) => {
                if let (Some(g), Ok(p)) = (r.id_group(), id_prefix(&r.name)) {
                    *members.entry(g.to_owned()).or_default().entry(p.to_owned()).or_default() += 1;
                }
                id_files.entry(stem).or_default().push(r);
            }
            Err(e) => errors.push(e),
        }
    }
    let mut group_files: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    for r in groups {
        match group_file(&r, &members) {
            Ok(stem) => group_files.entry(stem).or_default().push(r),
            Err(e) => errors.push(e),
        }
    }
    for rows in id_files.values_mut().chain(group_files.values_mut()) {
        rows.sort_by(|a, b| a.name.cmp(&b.name));
    }
    if errors.is_empty() { Ok([id_files, group_files]) } else { Err(errors) }
}

fn dedup(rows: Vec<Row>, errors: &mut Vec<String>) -> Vec<Row> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<Row> = Vec::new();
    for r in rows {
        if let Some(&i) = seen.get(&r.name) {
            if out[i].text == r.text {
                // Keep the area file's copy (and its comments).
                if out[i].stem.is_none() && r.stem.is_some() {
                    out[i] = r;
                }
            } else {
                errors.push(format!(
                    "`{}` is defined twice, differently:\n    {}\n    {}",
                    r.name, out[i].text, r.text
                ));
            }
            continue;
        }
        seen.insert(r.name.clone(), out.len());
        out.push(r);
    }
    out
}

/// The tables as found in the working tree.
#[derive(Debug, Default)]
struct Worktree {
    /// Rows per table: legacy tails first, then area and pending files.
    rows: [Vec<Row>; 2],
    /// Existing `.rs` stems per table directory (including `_pending`).
    stems: [Vec<String>; 2],
    /// Legacy file text per table.
    legacy: [String; 2],
    /// Problems `--check` reports besides misplacement.
    tail_counts: Vec<String>,
}

fn read(root: &Path, rel: &str) -> Result<String, String> {
    std::fs::read_to_string(root.join(rel)).map_err(|e| format!("cannot read {rel}: {e}"))
}

fn load(root: &Path, warnings: &mut Vec<String>) -> Result<Worktree, String> {
    let mut w = Worktree::default();
    for (k, table) in Table::BOTH.into_iter().enumerate() {
        let src = read(root, table.legacy())?;
        let (tail, old) = legacy_rows(&src, table, warnings)?;
        if old {
            return Err(format!(
                "{} still holds the pre-split table (`{}`); during a merge of main run \
                 `cargo xtask diag-split --replay` (spec/diag.md §2.4)",
                table.legacy(),
                table.old_opener()
            ));
        }
        if !tail.is_empty() {
            w.tail_counts.push(format!(
                "{} rows in the tail block of {}",
                tail.len(),
                table.legacy()
            ));
        }
        w.rows[k].extend(tail);
        w.legacy[k] = src;
        let mut stems: Vec<String> = std::fs::read_dir(root.join(table.dir()))
            .map_err(|e| format!("cannot list {}: {e}", table.dir()))?
            .filter_map(Result::ok)
            .filter_map(|e| e.file_name().to_str()?.strip_suffix(".rs").map(str::to_owned))
            .collect();
        stems.sort();
        for stem in &stems {
            let rel = format!("{}/{stem}.rs", table.dir());
            let text = read(root, &rel)?;
            let lines: Vec<&str> = text.lines().collect();
            let from = (stem != PENDING).then_some(stem.as_str());
            let rows = parse_rows(&lines, from, &rel, warnings)?;
            if stem == PENDING && !rows.is_empty() {
                w.tail_counts.push(format!("{} rows in {rel}", rows.len()));
            }
            w.rows[k].extend(rows);
        }
        w.stems[k] = stems;
    }
    Ok(w)
}

/// `src` with the content of its tail block removed.
fn empty_tail(src: &str, table: Table) -> Result<String, String> {
    let stripped = strip_blocks(src, table)?;
    Ok(if src.ends_with('\n') { stripped + "\n" } else { stripped })
}

/// Files to write (repo-relative path, text) and files to remove.
type Plan = (Vec<(String, String)>, Vec<String>);

/// Every file the canonical layout of `w`'s rows writes, by repo-relative
/// path, and the existing area files it no longer has rows for.
fn plan(w: Worktree) -> Result<Plan, Vec<String>> {
    let Worktree { rows: [ids, groups], stems, legacy, .. } = w;
    let files = layout(ids, groups, &stems[0])?;
    let mut writes = Vec::new();
    let mut removes = Vec::new();
    for (k, table) in Table::BOTH.into_iter().enumerate() {
        writes
            .push((table.legacy().to_owned(), empty_tail(&legacy[k], table).map_err(|e| vec![e])?));
        for (stem, rows) in &files[k] {
            writes.push((format!("{}/{stem}.rs", table.dir()), render(table, stem, rows)));
        }
        for stem in &stems[k] {
            if !files[k].contains_key(stem) {
                removes.push(format!("{}/{stem}.rs", table.dir()));
            }
        }
    }
    Ok((writes, removes))
}

/// Applies a plan; returns the paths changed.
fn apply(
    root: &Path,
    writes: &[(String, String)],
    removes: &[String],
) -> Result<Vec<String>, String> {
    let mut changed = Vec::new();
    for (rel, text) in writes {
        if std::fs::read_to_string(root.join(rel)).ok().as_deref() != Some(text.as_str()) {
            std::fs::write(root.join(rel), text).map_err(|e| format!("cannot write {rel}: {e}"))?;
            changed.push(rel.clone());
        }
    }
    for rel in removes {
        std::fs::remove_file(root.join(rel)).map_err(|e| format!("cannot remove {rel}: {e}"))?;
        changed.push(format!("{rel} (removed)"));
    }
    Ok(changed)
}

/// `cargo xtask diag-split --check`: the problems with the layout, if any.
pub fn check(root: &Path) -> Result<(), Vec<String>> {
    let mut warnings = Vec::new();
    let w = load(root, &mut warnings).map_err(|e| vec![e])?;
    let mut problems = w.tail_counts.clone();
    let found: Vec<(usize, String, String)> = (0..2)
        .flat_map(|k| {
            w.rows[k].iter().filter_map(move |r| Some((k, r.name.clone(), r.stem.clone()?)))
        })
        .collect();
    let (writes, removes) = plan(w)?;
    // Where each row should be, to name the misplaced ones.
    let mut target: HashMap<(usize, &str), &str> = HashMap::new();
    for (rel, text) in &writes {
        let k = usize::from(rel.starts_with(GROUPS_DIR));
        if rel == IDS_RS || rel == GROUPS_RS {
            continue;
        }
        for line in text.lines().filter(|l| !l.starts_with("//") && !l.is_empty()) {
            target.insert((k, line.split([' ', '=']).next().unwrap_or_default()), rel);
        }
    }
    for (k, name, stem) in &found {
        if let Some(rel) = target.get(&(*k, name.as_str()))
            && !rel.ends_with(&format!("/{stem}.rs"))
        {
            problems.push(format!(
                "`{name}` is in {}/{stem}.rs but belongs in {rel}",
                Table::BOTH[*k].dir()
            ));
        }
    }
    if problems.is_empty() {
        for (rel, text) in &writes {
            if std::fs::read_to_string(root.join(rel)).ok().as_deref() != Some(text.as_str()) {
                problems.push(format!(
                    "{rel} is not in canonical form (rows sorted by name, the standard header)"
                ));
            }
        }
        problems.extend(removes.iter().map(|r| format!("{r} has no rows")));
    }
    problems.extend(warnings);
    if problems.is_empty() { Ok(()) } else { Err(problems) }
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let o = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !o.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&o.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// Every row of `table` at revision `rev`, whatever the layout there.
fn rows_at(
    root: &Path,
    rev: &str,
    table: Table,
    warnings: &mut Vec<String>,
) -> Result<Vec<Row>, String> {
    let src = git(root, &["show", &format!("{rev}:{}", table.legacy())])?;
    let mut rows = legacy_rows(&src, table, warnings)?.0;
    let listing = git(root, &["ls-tree", "--name-only", rev, "--", &format!("{}/", table.dir())])?;
    for path in listing.lines().filter(|p| Path::new(p).extension().is_some_and(|x| x == "rs")) {
        let text = git(root, &["show", &format!("{rev}:{path}")])?;
        let lines: Vec<&str> = text.lines().collect();
        rows.extend(parse_rows(&lines, None, &format!("{rev}:{path}"), warnings)?);
    }
    Ok(rows)
}

/// Re-applies the edits from `base` to `head` onto `cur`, by row name.
/// Returns the conflicts it left for a human.
fn replay_rows(table: Table, base: &[Row], head: &[Row], cur: &mut Vec<Row>) -> Vec<String> {
    let by_name = |rows: &[Row]| -> HashMap<String, Row> {
        rows.iter().map(|r| (r.name.clone(), r.clone())).collect()
    };
    let (in_base, in_head) = (by_name(base), by_name(head));
    let mut conflicts = Vec::new();
    let pos = |cur: &Vec<Row>, name: &str| cur.iter().position(|r| r.name == name);
    // Head order, so added rows keep their relative order in messages.
    for hr in head {
        let i = pos(cur, &hr.name);
        match (in_base.get(&hr.name), i) {
            (None, None) => cur.push(Row { stem: None, ..hr.clone() }),
            (None, Some(i)) if cur[i].text == hr.text => {}
            (None, Some(i)) => conflicts.push(format!(
                "`{}` was added on both sides, differently:\n    yours:  {}\n    theirs: {}",
                hr.name, hr.text, cur[i].text
            )),
            (Some(br), _) if br.text == hr.text => {}
            (Some(br), None) => conflicts.push(format!(
                "`{}` was changed on your side but removed on theirs:\n    yours: {}",
                br.name, hr.text
            )),
            (Some(br), Some(i)) => {
                let c = &cur[i];
                if c.text == br.text {
                    cur[i].text.clone_from(&hr.text);
                } else if c.text != hr.text {
                    match merge_changed(table, br, hr, c) {
                        Some(t) => cur[i].text = t,
                        None => conflicts.push(format!(
                            "`{}` was changed on both sides:\n    base:   {}\n    yours:  {}\n    theirs: {}",
                            br.name, br.text, hr.text, c.text
                        )),
                    }
                }
            }
        }
    }
    for br in base.iter().filter(|r| !in_head.contains_key(&r.name)) {
        if let Some(i) = pos(cur, &br.name) {
            if cur[i].text == br.text {
                cur.remove(i);
            } else {
                conflicts.push(format!(
                    "`{}` was removed on your side but changed on theirs:\n    theirs: {}",
                    br.name, cur[i].text
                ));
            }
        }
    }
    conflicts
}

/// A group row changed on both sides: keeps their option name unless only
/// yours changed it, and applies your subgroup additions and removals to
/// theirs. `None` for an id row, or when both changed the option name.
fn merge_changed(table: Table, base: &Row, head: &Row, cur: &Row) -> Option<String> {
    if table != Table::Groups {
        return None;
    }
    let (bo, bs) = base.group_parts().ok()?;
    let (ho, hs) = head.group_parts().ok()?;
    let (co, mut cs) = cur.group_parts().ok()?;
    let option = match (ho == bo, co == bo) {
        (true, _) => co,
        (false, true) => ho,
        (false, false) if ho == co => co,
        (false, false) => return None,
    };
    cs.retain(|s| !bs.contains(s) || hs.contains(s));
    for s in hs.iter().filter(|s| !bs.contains(s)) {
        if !cs.contains(s) {
            cs.push(s.clone());
        }
    }
    Some(group_text(&head.name, &option, &cs))
}

fn replay(
    root: &Path,
    revs: Option<(String, String)>,
) -> Result<(Vec<String>, Vec<String>), String> {
    let mut warnings = Vec::new();
    let (base, head) = if let Some(r) = revs {
        r
    } else {
        if git(root, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]).is_err() {
            return Err("--replay with no revisions must run during a merge (no MERGE_HEAD)".into());
        }
        let base = git(root, &["merge-base", "HEAD", "MERGE_HEAD"])?.trim().to_owned();
        for table in Table::BOTH {
            let theirs = git(root, &["show", &format!("MERGE_HEAD:{}", table.legacy())])?;
            if blocks(&theirs, table)?.1 {
                return Err(format!(
                    "MERGE_HEAD's {} is not split yet; nothing to replay onto",
                    table.legacy()
                ));
            }
            let mine = read(root, table.legacy())?;
            let conflicted =
                mine.lines().any(|l| l.starts_with("<<<<<<<") || l.starts_with(">>>>>>>"));
            if conflicted || blocks(&mine, table).map_or(true, |b| b.1) {
                std::fs::write(root.join(table.legacy()), &theirs)
                    .map_err(|e| format!("cannot write {}: {e}", table.legacy()))?;
                warnings.push(format!("reset {} to MERGE_HEAD's version", table.legacy()));
            }
        }
        (base, "HEAD".to_owned())
    };
    // Hand-written code the branch changed in a legacy file is not replayed.
    for table in Table::BOTH {
        let at = |rev: &str| -> Result<String, String> {
            strip_blocks(&git(root, &["show", &format!("{rev}:{}", table.legacy())])?, table)
        };
        if at(&base)? != at(&head)? {
            warnings.push(format!(
                "your side changed code outside the table in {0}; re-apply it by hand \
                 (`git diff {base} {head} -- {0}`)",
                table.legacy()
            ));
        }
    }
    let mut w = load(root, &mut warnings)?;
    let mut conflicts = Vec::new();
    for (k, table) in Table::BOTH.into_iter().enumerate() {
        let b = rows_at(root, &base, table, &mut warnings)?;
        let h = rows_at(root, &head, table, &mut warnings)?;
        conflicts.extend(replay_rows(table, &b, &h, &mut w.rows[k]));
    }
    let (writes, removes) = plan(w).map_err(|e| e.join("\n"))?;
    let changed = apply(root, &writes, &removes)?;
    warnings.extend(changed.into_iter().map(|c| format!("updated {c}")));
    Ok((warnings, conflicts))
}

pub fn main(args: impl Iterator<Item = String>) -> ExitCode {
    let args: Vec<String> = args.collect();
    let root = crate::util::repo_root();
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        [] => {
            let mut warnings = Vec::new();
            let result = load(&root, &mut warnings)
                .map_err(|e| vec![e])
                .and_then(plan)
                .and_then(|(w, r)| apply(&root, &w, &r).map_err(|e| vec![e]));
            for w in &warnings {
                eprintln!("xtask diag-split: warning: {w}");
            }
            match result {
                Ok(changed) => {
                    for c in &changed {
                        eprintln!("xtask diag-split: updated {c}");
                    }
                    if changed.is_empty() {
                        eprintln!("xtask diag-split: tables already in canonical form");
                    }
                    ExitCode::SUCCESS
                }
                Err(errors) => fail(&errors),
            }
        }
        ["--check"] => match check(&root) {
            Ok(()) => {
                eprintln!("xtask diag-split: tables in canonical form");
                ExitCode::SUCCESS
            }
            Err(problems) => {
                fail(&problems);
                eprintln!(
                    "xtask diag-split: run `cargo xtask diag-split` to fix (spec/diag.md §2.4)"
                );
                ExitCode::FAILURE
            }
        },
        ["--replay"] | ["--replay", _, _] => {
            let revs = (args.len() == 3).then(|| (args[1].clone(), args[2].clone()));
            match replay(&root, revs) {
                Ok((notes, conflicts)) => {
                    for n in &notes {
                        eprintln!("xtask diag-split: {n}");
                    }
                    if conflicts.is_empty() {
                        eprintln!(
                            "xtask diag-split: replayed; `git add crates/kiln_diag` and commit"
                        );
                        ExitCode::SUCCESS
                    } else {
                        fail(&conflicts);
                        eprintln!(
                            "xtask diag-split: {} conflicts: edit the area files by hand, then run \
                             `cargo xtask diag-split --check`",
                            conflicts.len()
                        );
                        ExitCode::FAILURE
                    }
                }
                Err(e) => fail(&[e]),
            }
        }
        ["-h" | "--help"] => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        _ => {
            eprint!("xtask diag-split: bad arguments\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn fail(errors: &[String]) -> ExitCode {
    for e in errors {
        eprintln!("xtask diag-split: {e}");
    }
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(text: &str, stem: Option<&str>) -> Row {
        Row::parse(text, Vec::new(), stem.map(str::to_owned)).unwrap()
    }

    #[test]
    fn rows_and_comments() {
        let mut w = Vec::new();
        let src =
            ["// header", "", "// about b", "b_x = Error (0) \"b\";", "a_y = Note (0) \"a\";"];
        let rows = parse_rows(&src, Some("a"), "t", &mut w).unwrap();
        assert_eq!(w, Vec::<String>::new());
        assert_eq!(rows[0].comments, ["// about b"]);
        assert_eq!(rows[1].name, "a_y");
        assert!(parse_rows(&["oops"], None, "t", &mut w).is_err());
        let dangling =
            parse_rows(&["a_y = Note (0) \"a\";", "", "// stray", ""], None, "t", &mut w).unwrap();
        assert_eq!(dangling.len(), 1);
        assert_eq!(w.len(), 1);
    }

    #[test]
    fn id_rows_and_groups() {
        assert_eq!(row("a_b = Warning [Foo] (1) \"x {0}\";", None).id_group(), Some("Foo"));
        assert_eq!(row("a_b = Error (0) \"[x]\";", None).id_group(), None);
        let g = row("All = \"all\" [Most, Switch];", None);
        assert_eq!(
            g.group_parts().unwrap(),
            ("all".to_owned(), vec!["Most".to_owned(), "Switch".to_owned()])
        );
        assert_eq!(
            row("P = \"#warnings\" [];", None).group_parts().unwrap().1,
            Vec::<String>::new()
        );
    }

    #[test]
    fn id_areas() {
        let a = IdAreas::new(["lex", "sema_a_b", "sema_c_m", "sema_n_z"]).unwrap();
        assert_eq!(a.file_of("lex_foo").unwrap(), "lex");
        assert_eq!(a.file_of("sema_bar").unwrap(), "sema_a_b");
        assert_eq!(a.file_of("sema_3way").unwrap(), "sema_a_b");
        assert_eq!(a.file_of("sema_c").unwrap(), "sema_c_m");
        assert_eq!(a.file_of("sema_zz").unwrap(), "sema_n_z");
        assert_eq!(a.file_of("cg_new").unwrap(), "cg");
        assert!(a.file_of("noprefix").is_err());
        assert!(IdAreas::new(["sema_a_b", "sema_d_z"]).is_err());
        assert!(IdAreas::new(["sema", "sema_a_z"]).is_err());
        assert!(IdAreas::new(["sema_ab"]).is_err());
    }

    #[test]
    fn layout_sorts_and_places_groups() {
        let ids = vec![
            row("sema_z = Warning [Foo] (0) \"z\";", Some("sema")),
            row("sema_a = Warning [Foo] (0) \"a\";", None),
            row("pp_a = Warning [Bar] (0) \"p\";", None),
            row("sema_b = Warning [Bar] (0) \"b\";", None),
            row("sema_a = Warning [Foo] (0) \"a\";", Some("sema")),
        ];
        let groups = vec![
            row("Foo = \"foo\" [];", None),
            row("Bar = \"bar\" [];", Some("sema")),
            row("All = \"all\" [Foo];", None),
            row("Pedantic = \"pedantic\" [];", None),
            row("Lone = \"lone\" [];", None),
        ];
        let [i, g] = layout(ids, groups, &["sema".to_owned()]).unwrap();
        let names = |m: &BTreeMap<String, Vec<Row>>, k: &str| {
            m[k].iter().map(|r| r.name.clone()).collect::<Vec<_>>()
        };
        assert_eq!(names(&i, "sema"), ["sema_a", "sema_b", "sema_z"]);
        assert_eq!(names(&i, "pp"), ["pp_a"]);
        assert_eq!(names(&g, "sema"), ["Bar", "Foo"]); // Bar stays: a sema id names it
        assert_eq!(names(&g, UMBRELLAS), ["All", "Pedantic"]);
        assert_eq!(names(&g, MISC), ["Lone"]);
        let clash = vec![row("a_x = Error (0) \"1\";", None), row("a_x = Error (0) \"2\";", None)];
        assert!(layout(clash, Vec::new(), &[]).is_err());
    }

    #[test]
    fn tail_blocks() {
        let src = "code\ndiag_id_table! {\n    a_x = Error (0) \"x\";\n}\nmore\n";
        let mut w = Vec::new();
        let (rows, old) = legacy_rows(src, Table::Ids, &mut w).unwrap();
        assert_eq!((rows.len(), old), (1, false));
        assert_eq!(empty_tail(src, Table::Ids).unwrap(), "code\ndiag_id_table! {\n}\nmore\n");
        assert!(legacy_rows("diag_ids! {\n}\n", Table::Ids, &mut w).unwrap().1);
        assert!(blocks("diag_ids! {\n", Table::Ids).is_err());
    }

    #[test]
    fn replay_by_name() {
        let r = |t: &str| row(t, None);
        let base = vec![
            r("a_k = Error (0) \"k\";"),
            r("a_m = Error (0) \"m\";"),
            r("a_d = Error (0) \"d\";"),
        ];
        let head = vec![
            r("a_k = Error (0) \"k2\";"), // changed
            r("a_m = Error (0) \"m\";"),
            r("a_new = Error (0) \"n\";"), // added; a_d removed
        ];
        let mut cur = vec![
            r("a_k = Error (0) \"k\";"),
            r("a_m = Error (0) \"m3\";"),
            r("a_d = Error (0) \"d\";"),
            r("a_main = Error (0) \"x\";"),
        ];
        let conflicts = replay_rows(Table::Ids, &base, &head, &mut cur);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        let texts: Vec<&str> = cur.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "a_k = Error (0) \"k2\";",
                "a_m = Error (0) \"m3\";",
                "a_main = Error (0) \"x\";",
                "a_new = Error (0) \"n\";"
            ]
        );
        // Replaying again changes nothing.
        let again = cur.clone();
        assert_eq!(replay_rows(Table::Ids, &base, &head, &mut cur), Vec::<String>::new());
        assert_eq!(cur, again);
        // Both sides changed the same id: a conflict, theirs kept.
        let mut cur = vec![r("a_k = Error (0) \"k3\";")];
        assert_eq!(replay_rows(Table::Ids, &base[..1], &head[..1], &mut cur).len(), 1);
        assert_eq!(cur[0].text, "a_k = Error (0) \"k3\";");
    }

    #[test]
    fn replay_merges_subgroups() {
        let r = |t: &str| row(t, None);
        let base = [r("Gnu = \"gnu\" [A, B];")];
        let head = [r("Gnu = \"gnu\" [A, B, Mine];")];
        let mut cur = vec![r("Gnu = \"gnu\" [B, Theirs];")];
        assert_eq!(replay_rows(Table::Groups, &base, &head, &mut cur), Vec::<String>::new());
        assert_eq!(cur[0].text, "Gnu = \"gnu\" [B, Theirs, Mine];");
        let head = [r("Gnu = \"gnu\" [B];")];
        let mut cur = vec![r("Gnu = \"gnu\" [A, B, Theirs];")];
        assert_eq!(replay_rows(Table::Groups, &base, &head, &mut cur), Vec::<String>::new());
        assert_eq!(cur[0].text, "Gnu = \"gnu\" [B, Theirs];");
    }
}
