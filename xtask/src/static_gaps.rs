//! The static-gap inventory lint (WP-1719; spec/harness.md "CI", step 0;
//! docs/status/static-gaps.md).
//!
//! A *gap site* is a place in a crate's (non-test) source that reports a
//! language corner kiln knows it does not handle: a use of one of the gap
//! diagnostics ([`GAP_DIAG_IDS`]), a call of a function that forwards its
//! text to one ([`WRAPPERS`]), a string literal with one of the gap phrases
//! ([`PHRASES`]: codegen fallbacks, Oven traps, reflection), or a
//! `todo!`/`unimplemented!`. Each site carries a marker comment, on its own
//! line or in the comment block directly above it:
//!
//! - `// kiln-gap: GAP-NNN`: the site belongs to inventory entry `GAP-NNN`;
//! - `// kiln-gap: forward`: the site is inside a wrapper of [`WRAPPERS`]
//!   and only forwards its caller's text (the callers are the sites);
//! - `// kiln-gap: check`: the line only compares a diagnostic's id with a
//!   gap id.
//!
//! A `GAP-NNN` marker that no detected site uses marks the next line of
//! code as a site of its own: a gap whose text is none of the detected
//! forms (a mangling failure recorded for a later diagnostic).
//!
//! [`check`] fails when (a) a site has no marker (or a `forward` marker
//! outside a wrapper, a `check` marker on another line, or names an id the
//! inventory lacks), (b) an inventory
//! entry has no site left (stale: delete it once the gap is fixed), (c) an
//! entry's sites cell is not the `file::fn` list its markers give, or (d) an
//! entry's BACKLOG row is missing or neither `open` nor `in progress`. The
//! markers make it insensitive to line shifts.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The inventory, relative to the repository root.
pub const INVENTORY: &str = "docs/status/static-gaps.md";

/// The backlog whose rows the entries point at, relative to the root.
const BACKLOG: &str = "docs/BACKLOG.md";

/// The diagnostics whose every use is a gap site (spec/diag.md ids).
pub const GAP_DIAG_IDS: &[&str] = &[
    "cg_cannot_mangle_expression",
    "drv_not_implemented",
    "parse_unsupported",
    "sema_attribute_unsupported_by_kiln",
    "sema_conv_unsupported",
    "sema_unimplemented_template_feature",
    "sema_vla_unsupported",
];

/// Functions that forward a text to a gap diagnostic or trap, by crate:
/// every call of one in that crate is a gap site.
pub const WRAPPERS: &[(&str, &str)] = &[
    ("kiln_oven", "not_implemented"),
    ("kiln_oven", "unsupported"),
    ("kiln_sema", "not_implemented"),
    ("kiln_sema", "templ_unimplemented"),
    ("kiln_sema", "unsupported"),
];

/// Phrases (lowercase) that make a string literal a gap site.
pub const PHRASES: &[&str] = &[
    "not supported yet",
    "not yet supported",
    "not yet implemented",
    "not implemented yet",
    "cannot yet",
    "support header units yet",
];

/// Two-part phrases: a literal holding both parts is a gap site (clang's
/// own "cannot compile this X yet").
const PHRASE_PAIRS: &[(&str, &str)] = &[("cannot compile", " yet")];

/// Macros whose every (non-test) use is a gap site.
const MACROS: &[&str] = &["todo!", "unimplemented!"];

/// Source directories never scanned: the diagnostic catalogue (its texts
/// are definitions; the sites are the ids' uses).
const SKIP_DIRS: &[&str] = &["crates/kiln_diag/src/ids"];

/// The marker of a site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Marker {
    /// `// kiln-gap: GAP-NNN`.
    Gap(String),
    /// `// kiln-gap: forward`.
    Forward,
    /// `// kiln-gap: check`: the line compares a diagnostic's id with a
    /// gap id; it reports nothing.
    Check,
    /// `// kiln-gap: <anything else>`.
    Bad(String),
}

/// One gap site.
#[derive(Clone, Debug)]
pub struct Site {
    /// The file, relative to the root, with `/` separators.
    pub file: String,
    /// 1-based line (informative).
    pub line: usize,
    /// The innermost preceding `fn` (empty at file scope).
    pub func: String,
    /// What makes it a site: `DiagId::x`, `call x`, `text`, `todo!`.
    pub what: String,
    /// The string literal or source text (informative).
    pub text: String,
    /// Its marker, if any.
    pub marker: Option<Marker>,
}

impl Site {
    /// The key the inventory lists: `file::fn`.
    pub fn key(&self) -> String {
        format!("{}::{}", self.file, self.func)
    }
}

/// One source line split into its parts.
#[derive(Default, Debug)]
struct Line {
    /// The code with literals blanked (`""`, `' '`) and comments removed.
    code: String,
    /// The comment text on the line.
    comment: String,
    /// Whether the line has only a comment (or is inside a block comment).
    comment_only: bool,
}

/// A string literal: its content and the line of each content byte.
struct Lit {
    text: String,
    lines: Vec<usize>,
}

/// Splits `src` into lines of code and comments, and its string literals.
#[allow(clippy::too_many_lines, reason = "one character-level state machine")]
fn lex(src: &str) -> (Vec<Line>, Vec<Lit>) {
    let chars: Vec<char> = src.chars().collect();
    let mut lines = vec![Line::default()];
    let mut lits = Vec::new();
    let mut i = 0;
    let n = chars.len();
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut block_depth = 0usize;
    let mut in_line_comment = false;
    while i < n {
        let c = chars[i];
        let cur = lines.len() - 1;
        if c == '\n' {
            in_line_comment = false;
            lines.push(Line::default());
            i += 1;
            continue;
        }
        if in_line_comment {
            lines[cur].comment.push(c);
            i += 1;
            continue;
        }
        if block_depth > 0 {
            if c == '*' && chars.get(i + 1) == Some(&'/') {
                block_depth -= 1;
                i += 2;
            } else if c == '/' && chars.get(i + 1) == Some(&'*') {
                block_depth += 1;
                i += 2;
            } else {
                lines[cur].comment.push(c);
                i += 1;
            }
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            in_line_comment = true;
            i += 2;
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            block_depth = 1;
            i += 2;
            continue;
        }
        let prev_ident = i > 0 && is_ident(chars[i - 1]);
        // Raw strings: r"..", r#".."#, br"..".
        let raw_start = if c == 'r' && !prev_ident {
            Some(i + 1)
        } else if c == 'b' && !prev_ident && chars.get(i + 1) == Some(&'r') {
            Some(i + 2)
        } else {
            None
        };
        if let Some(mut j) = raw_start {
            let mut hashes = 0;
            while chars.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if chars.get(j) == Some(&'"') {
                let mut lit = Lit { text: String::new(), lines: Vec::new() };
                let mut line = lines.len() - 1;
                j += 1;
                while j < n {
                    if chars[j] == '"' && (0..hashes).all(|h| chars.get(j + 1 + h) == Some(&'#')) {
                        j += 1 + hashes;
                        break;
                    }
                    if chars[j] == '\n' {
                        lines.push(Line::default());
                        line += 1;
                    }
                    lit.text.push(chars[j]);
                    lit.lines.push(line);
                    j += 1;
                }
                lines[line].code.push_str("\"\"");
                lits.push(lit);
                i = j;
                continue;
            }
        }
        if c == '"' {
            let mut lit = Lit { text: String::new(), lines: Vec::new() };
            let mut line = lines.len() - 1;
            let mut j = i + 1;
            while j < n && chars[j] != '"' {
                if chars[j] == '\\' && j + 1 < n {
                    if chars[j + 1] == '\n' {
                        // A continuation: the next line's leading blanks
                        // are not part of the literal.
                        lines.push(Line::default());
                        line += 1;
                        j += 2;
                        while j < n && chars[j].is_whitespace() && chars[j] != '\n' {
                            j += 1;
                        }
                        continue;
                    }
                    lit.text.push(chars[j + 1]);
                    lit.lines.push(line);
                    j += 2;
                    continue;
                }
                if chars[j] == '\n' {
                    lines.push(Line::default());
                    line += 1;
                }
                lit.text.push(chars[j]);
                lit.lines.push(line);
                j += 1;
            }
            lines[line].code.push_str("\"\"");
            lits.push(lit);
            i = j + 1;
            continue;
        }
        if c == '\'' {
            // A char literal ('x', '\n', '\u{..}') or a lifetime ('a).
            if chars.get(i + 1) == Some(&'\\') {
                let mut j = i + 2;
                while j < n && chars[j] != '\'' && chars[j] != '\n' {
                    j += 1;
                }
                lines[cur].code.push_str("' '");
                i = j + 1;
                continue;
            }
            if chars.get(i + 2) == Some(&'\'') {
                lines[cur].code.push_str("' '");
                i += 3;
                continue;
            }
        }
        lines[cur].code.push(c);
        i += 1;
    }
    for l in &mut lines {
        l.comment_only = l.code.trim().is_empty() && !l.comment.trim().is_empty();
    }
    (lines, lits)
}

/// The lines (0-based) inside a `#[cfg(test)]` item.
fn test_lines(lines: &[Line]) -> Vec<bool> {
    let mut out = vec![false; lines.len()];
    let mut i = 0;
    while i < lines.len() {
        if !lines[i].code.contains("#[cfg(test)]") {
            i += 1;
            continue;
        }
        let start = i;
        let mut depth = 0usize;
        let mut opened = false;
        let mut end = None;
        'scan: for (k, l) in lines.iter().enumerate().skip(i) {
            let from = if k == start {
                l.code.find("#[cfg(test)]").map_or(0, |p| p + "#[cfg(test)]".len())
            } else {
                0
            };
            for ch in l.code[from..].chars() {
                match ch {
                    ';' if !opened => {
                        end = Some(k);
                        break 'scan;
                    }
                    '{' => {
                        depth += 1;
                        opened = true;
                    }
                    '}' if opened => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(k);
                            break 'scan;
                        }
                    }
                    _ => {}
                }
            }
        }
        let end = end.unwrap_or(lines.len() - 1);
        for t in &mut out[start..=end] {
            *t = true;
        }
        i = end + 1;
    }
    out
}

/// Whether `code[at..]` starts `word` as a whole identifier.
fn word_at(code: &str, at: usize, word: &str) -> bool {
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    code[at..].starts_with(word)
        && !code[..at].chars().next_back().is_some_and(is_ident)
        && !code[at + word.len()..].chars().next().is_some_and(is_ident)
}

/// The name of the last `fn NAME` on a code line.
fn fn_name(code: &str) -> Option<String> {
    let mut found = None;
    for (at, _) in code.match_indices("fn") {
        if !word_at(code, at, "fn") {
            continue;
        }
        let rest = code[at + 2..].trim_start();
        let name: String = rest.chars().take_while(|&c| c.is_alphanumeric() || c == '_').collect();
        if !name.is_empty() && rest.len() < code[at + 2..].len() {
            found = Some(name);
        }
    }
    found
}

/// The calls of `name` on a code line: `name(` or `name::<`, not its
/// definition.
fn calls(code: &str, name: &str) -> bool {
    code.match_indices(name).any(|(at, _)| {
        let after = &code[at + name.len()..];
        word_at(code, at, name)
            && (after.starts_with('(') || after.starts_with("::<"))
            && !code[..at].trim_end().ends_with("fn")
    })
}

/// The marker in a comment, if any.
fn marker_of(comment: &str) -> Option<Marker> {
    let at = comment.find("kiln-gap:")?;
    let rest = comment[at + "kiln-gap:".len()..].trim();
    let word: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
    Some(if word == "forward" {
        Marker::Forward
    } else if word == "check" {
        Marker::Check
    } else if is_gap_id(&word) {
        Marker::Gap(word)
    } else {
        Marker::Bad(word)
    })
}

/// `GAP-` and three or more digits.
fn is_gap_id(s: &str) -> bool {
    s.strip_prefix("GAP-").is_some_and(|d| d.len() >= 3 && d.bytes().all(|b| b.is_ascii_digit()))
}

/// The crate directory name of a path `crates/<name>/...`.
fn crate_of(rel: &str) -> &str {
    rel.strip_prefix("crates/").and_then(|r| r.split('/').next()).unwrap_or("")
}

/// The gap sites of one file (`rel` relative to the root).
pub fn scan_file(rel: &str, src: &str) -> Vec<Site> {
    let (lines, lits) = lex(src);
    let tests = test_lines(&lines);
    let krate = crate_of(rel);
    let wrappers: Vec<&str> =
        WRAPPERS.iter().filter(|(c, _)| *c == krate).map(|&(_, w)| w).collect();
    let mut hits: BTreeMap<usize, (String, String)> = BTreeMap::new();
    for (k, l) in lines.iter().enumerate() {
        if tests[k] {
            continue;
        }
        for id in GAP_DIAG_IDS {
            let tok = format!("DiagId::{id}");
            if l.code.match_indices(&tok).any(|(at, _)| word_at(&l.code, at + 8, id)) {
                hits.entry(k).or_insert_with(|| (tok.clone(), l.code.trim().to_owned()));
            }
        }
        for w in &wrappers {
            if calls(&l.code, w) {
                hits.entry(k).or_insert_with(|| (format!("call {w}"), l.code.trim().to_owned()));
            }
        }
        for m in MACROS {
            let name = &m[..m.len() - 1];
            if l.code.match_indices(m).any(|(at, _)| word_at(&l.code, at, name)) {
                hits.entry(k).or_insert_with(|| ((*m).to_owned(), l.code.trim().to_owned()));
            }
        }
    }
    for lit in &lits {
        let lower = lit.text.to_lowercase();
        let at = PHRASES.iter().filter_map(|p| lower.find(p)).min().or_else(|| {
            PHRASE_PAIRS
                .iter()
                .filter_map(|(a, b)| lower.find(a).filter(|_| lower.contains(b)))
                .min()
        });
        // `to_lowercase` keeps byte offsets for the ASCII these texts are.
        let Some(at) = at.filter(|&a| a < lit.lines.len()) else { continue };
        let k = lit.lines[at];
        if !tests[k] {
            hits.entry(k).or_insert_with(|| ("text".to_owned(), lit.text.clone()));
        }
    }
    let func_at =
        |k: usize| (0..=k).rev().find_map(|j| fn_name(&lines[j].code)).unwrap_or_default();
    let mut used = BTreeSet::new();
    let mut out: Vec<Site> = hits
        .into_iter()
        .map(|(k, (what, text))| {
            let found = std::iter::once(k)
                .chain((0..k).rev().take_while(|&j| lines[j].comment_only))
                .find_map(|j| marker_of(&lines[j].comment).map(|m| (j, m)));
            let marker = found.map(|(j, m)| {
                used.insert(j);
                m
            });
            Site { file: rel.to_owned(), line: k + 1, func: func_at(k), what, text, marker }
        })
        .collect();
    // A marker no detected site uses marks the next line of code itself
    // (a gap whose text is not one of the detected forms).
    for (j, l) in lines.iter().enumerate() {
        let Some(m) = marker_of(&l.comment) else { continue };
        if used.contains(&j) || tests[j] {
            continue;
        }
        let k = (j..lines.len()).find(|&k| !lines[k].code.trim().is_empty()).unwrap_or(j);
        let text = lines[k].code.trim().to_owned();
        out.push(Site {
            file: rel.to_owned(),
            line: k + 1,
            func: func_at(k),
            what: "marked".to_owned(),
            text,
            marker: Some(m),
        });
    }
    out.sort_by_key(|s| s.line);
    out
}

/// Every `.rs` file under `crates/*/src`, except test modules and
/// [`SKIP_DIRS`], relative to `root`, sorted.
fn sources(root: &Path) -> std::io::Result<Vec<String>> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let p = entry?.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == "tests") {
                    continue;
                }
                walk(&p, out)?;
            } else if p.extension().is_some_and(|e| e == "rs")
                && p.file_name().is_some_and(|n| n != "tests.rs")
            {
                out.push(p);
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    let crates = root.join("crates");
    if crates.is_dir() {
        for entry in std::fs::read_dir(&crates)? {
            let src = entry?.path().join("src");
            if src.is_dir() {
                walk(&src, &mut out)?;
            }
        }
    }
    let mut rels: Vec<String> = out
        .iter()
        .filter_map(|p| p.strip_prefix(root).ok())
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .filter(|r| !SKIP_DIRS.iter().any(|d| r.starts_with(&format!("{d}/"))))
        .collect();
    rels.sort();
    Ok(rels)
}

/// Every gap site under `root`.
///
/// # Errors
/// An I/O error listing or reading the sources.
pub fn scan(root: &Path) -> Result<Vec<Site>, String> {
    let files = sources(root).map_err(|e| format!("cannot list crates/: {e}"))?;
    let mut out = Vec::new();
    for rel in files {
        let src = std::fs::read_to_string(root.join(&rel))
            .map_err(|e| format!("cannot read {rel}: {e}"))?;
        out.extend(scan_file(&rel, &src));
    }
    Ok(out)
}

/// One inventory entry: `| GAP-NNN | family | area | text | sites | reach
/// | row |`.
#[derive(Debug)]
struct Entry {
    id: String,
    sites: String,
    row: String,
}

/// The entries of the inventory's tables.
fn entries(text: &str) -> Vec<Entry> {
    text.lines()
        .filter(|l| l.starts_with("| GAP-"))
        .map(|l| {
            let cells: Vec<&str> = l.trim().trim_matches('|').split(" | ").map(str::trim).collect();
            Entry {
                id: cells.first().copied().unwrap_or("").to_owned(),
                sites: cells.get(4).copied().unwrap_or("").to_owned(),
                row: cells.last().copied().unwrap_or("").to_owned(),
            }
        })
        .collect()
}

/// The sites cell for a list of site keys: each `` `file::fn` `` once,
/// with ` ×N` when it holds N > 1 sites, sorted, joined by `, `.
pub fn sites_cell<'a>(keys: impl IntoIterator<Item = &'a str>) -> String {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for k in keys {
        *counts.entry(k).or_default() += 1;
    }
    let mut out = String::new();
    for (k, n) in counts {
        if !out.is_empty() {
            out.push_str(", ");
        }
        let _ = write!(out, "`{k}`");
        if n > 1 {
            let _ = write!(out, " ×{n}");
        }
    }
    out
}

/// The status cell of each backlog row, by id.
fn backlog_status(backlog: &str) -> BTreeMap<&str, &str> {
    backlog
        .lines()
        .filter_map(|l| {
            let cells: Vec<&str> = l.strip_prefix('|')?.split('|').map(str::trim).collect();
            let id = *cells.first()?;
            id.starts_with("WP-").then(|| (id, cells.get(3).copied().unwrap_or("")))
        })
        .collect()
}

/// The lint: `Err` lists every problem, one per line.
pub fn check(root: &Path) -> Result<(), Vec<String>> {
    let sites = scan(root).map_err(|e| vec![e])?;
    let inventory = std::fs::read_to_string(root.join(INVENTORY))
        .map_err(|e| vec![format!("cannot read {INVENTORY}: {e}")])?;
    let backlog = std::fs::read_to_string(root.join(BACKLOG))
        .map_err(|e| vec![format!("cannot read {BACKLOG}: {e}")])?;
    check_with(&sites, &inventory, &backlog)
}

/// [`check`] over already scanned sites and the two files' texts.
pub fn check_with(sites: &[Site], inventory: &str, backlog: &str) -> Result<(), Vec<String>> {
    let mut problems = Vec::new();
    let entries = entries(inventory);
    let status = backlog_status(backlog);
    let mut by_id: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut keys: Vec<String> = Vec::new();
    for s in sites {
        keys.push(s.key());
    }
    let mut seen = BTreeSet::new();
    for e in &entries {
        if !is_gap_id(&e.id) {
            problems.push(format!("{INVENTORY}: `{}` is not a GAP-NNN id", e.id));
        } else if !seen.insert(e.id.as_str()) {
            problems.push(format!("{INVENTORY}: {} is listed twice", e.id));
        }
    }
    for (s, key) in sites.iter().zip(&keys) {
        let at = format!("{}:{} ({}, {})", s.file, s.line, s.func, s.what);
        match &s.marker {
            None => problems.push(format!(
                "{at}: a gap site without a `// kiln-gap: GAP-NNN` marker; add an entry to \
                 {INVENTORY} (and a BACKLOG row) and mark the site"
            )),
            Some(Marker::Bad(w)) => {
                problems.push(format!("{at}: `kiln-gap: {w}` is neither GAP-NNN nor forward"));
            }
            Some(Marker::Check) => {
                if !s.what.starts_with("DiagId::") || !s.text.contains("==") {
                    problems.push(format!(
                        "{at}: `kiln-gap: check` on a line that compares no gap DiagId"
                    ));
                }
            }
            Some(Marker::Forward) => {
                let krate = crate_of(&s.file);
                if !WRAPPERS.iter().any(|&(c, w)| c == krate && w == s.func) {
                    problems.push(format!(
                        "{at}: `kiln-gap: forward` outside a gap wrapper (static_gaps::WRAPPERS)"
                    ));
                }
            }
            Some(Marker::Gap(id)) => {
                if seen.contains(id.as_str()) {
                    by_id.entry(id.as_str()).or_default().push(key);
                } else {
                    problems.push(format!("{at}: {id} has no entry in {INVENTORY}"));
                }
            }
        }
    }
    for e in entries.iter().filter(|e| is_gap_id(&e.id)) {
        match by_id.get(e.id.as_str()) {
            None => problems.push(format!(
                "{INVENTORY}: {} has no site left (a fixed gap: delete its entry)",
                e.id
            )),
            Some(ks) => {
                let want = sites_cell(ks.iter().copied());
                if e.sites != want {
                    problems.push(format!("{INVENTORY}: {}: the sites cell must be: {want}", e.id));
                }
            }
        }
        if e.row.starts_with("excluded") {
            continue;
        }
        match status.get(e.row.as_str()) {
            None => problems.push(format!(
                "{INVENTORY}: {}: row `{}` is not a {BACKLOG} row (or `excluded: <reason>`)",
                e.id, e.row
            )),
            Some(&st) if st == "open" || st == "in progress" => {}
            Some(st) => problems.push(format!(
                "{INVENTORY}: {}: row {} is `{st}`, not open (a fixed gap leaves no site)",
                e.id, e.row
            )),
        }
    }
    if problems.is_empty() { Ok(()) } else { Err(problems) }
}

const HELP: &str = "\
usage: cargo xtask static-gaps [--check | --sites]

The static-gap inventory (docs/status/static-gaps.md; spec/harness.md \"CI\"
step 0). --check (the default) is the ci lint; --sites prints every gap site as
TSV: file, line, fn, marker, what, text.
";

/// `cargo xtask static-gaps`.
pub fn main(mut args: impl Iterator<Item = String>) -> std::process::ExitCode {
    let root = crate::util::repo_root();
    match args.next().as_deref() {
        None | Some("--check") => match check(&root) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(p) => {
                for l in &p {
                    eprintln!("static-gaps: {l}");
                }
                std::process::ExitCode::FAILURE
            }
        },
        Some("--sites") => match scan(&root) {
            Ok(sites) => {
                for s in sites {
                    let m = match &s.marker {
                        None => String::new(),
                        Some(Marker::Gap(g)) => g.clone(),
                        Some(Marker::Forward) => "forward".to_owned(),
                        Some(Marker::Check) => "check".to_owned(),
                        Some(Marker::Bad(b)) => format!("?{b}"),
                    };
                    let text = s.text.replace(['\t', '\n'], " ");
                    println!("{}\t{}\t{}\t{m}\t{}\t{text}", s.file, s.line, s.func, s.what);
                }
                std::process::ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("static-gaps: {e}");
                std::process::ExitCode::FAILURE
            }
        },
        _ => {
            eprint!("{HELP}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BACKLOG_ROWS: &str = "| id | scope | needs | status |\n|---|---|---|---|\n\
                                | WP-10 | x | — | open |\n| WP-11 | y | — | done |\n";

    const SRC: &str = r#"
impl P {
    fn unsupported(&self, what: &str) {
        // kiln-gap: forward
        self.diag(DiagId::parse_unsupported).arg(what);
    }
    fn a(&self) {
        // Explained.
        // kiln-gap: GAP-001
        self.unsupported("x");
        let s = "this is not supported yet"; // kiln-gap: GAP-002
        let c = '"';
        self.unsupported_declarator(); // not a call of `unsupported`
    }
}
#[cfg(test)]
mod tests {
    fn t() { todo!() }
}
"#;

    fn inv(rows: &str) -> String {
        format!(
            "| gap | family | area | text | sites | reach | row |\n|---|---|---|---|---|---|---|\n{rows}"
        )
    }

    /// Sites are found by id, wrapper call and phrase; markers on the line
    /// or in the comment block above count; test modules are skipped.
    #[test]
    fn scan_finds_marked_sites() {
        let sites = scan_file("crates/kiln_sema/src/p.rs", SRC);
        let got: Vec<(usize, &str, &str, Option<Marker>)> = sites
            .iter()
            .map(|s| (s.line, s.func.as_str(), s.what.as_str(), s.marker.clone()))
            .collect();
        assert_eq!(
            got,
            [
                (5, "unsupported", "DiagId::parse_unsupported", Some(Marker::Forward)),
                (10, "a", "call unsupported", Some(Marker::Gap("GAP-001".into()))),
                (11, "a", "text", Some(Marker::Gap("GAP-002".into()))),
            ]
        );
    }

    /// A clean tree passes.
    #[test]
    fn listed_sites_pass() {
        let sites = scan_file("crates/kiln_sema/src/p.rs", SRC);
        let rows = "| GAP-001 | f | misc | `x` | `crates/kiln_sema/src/p.rs::a` | 0 | WP-10 |\n\
                    | GAP-002 | f | misc | `y` | `crates/kiln_sema/src/p.rs::a` | 0 | excluded: clang's |\n";
        assert_eq!(check_with(&sites, &inv(rows), BACKLOG_ROWS), Ok(()));
    }

    /// An unlisted (unmarked) site fails, and so does a marker without an
    /// entry.
    #[test]
    fn unlisted_site_fails() {
        let src = "fn f() {\n    x.diag(DiagId::sema_vla_unsupported);\n    \
                   // kiln-gap: GAP-009\n    y(\"not yet implemented\");\n}\n";
        let sites = scan_file("crates/kiln_sema/src/v.rs", src);
        let p = check_with(&sites, &inv(""), BACKLOG_ROWS).unwrap_err();
        assert_eq!(p.len(), 2, "{p:#?}");
        assert!(p[0].contains("v.rs:2 (f, DiagId::sema_vla_unsupported): a gap site without"));
        assert!(p[1].contains("GAP-009 has no entry"));
    }

    /// A stale entry (no site left), a wrong sites cell, a done or missing
    /// row and a forward marker outside a wrapper fail.
    #[test]
    fn stale_entries_fail() {
        let sites = scan_file("crates/kiln_sema/src/p.rs", SRC);
        let rows = "| GAP-001 | f | misc | `x` | `crates/kiln_sema/src/p.rs::b` | 0 | WP-11 |\n\
                    | GAP-002 | f | misc | `y` | `crates/kiln_sema/src/p.rs::a` | 0 | WP-12 |\n\
                    | GAP-003 | f | misc | `z` | `crates/kiln_sema/src/p.rs::a` | 0 | WP-10 |\n";
        let p = check_with(&sites, &inv(rows), BACKLOG_ROWS).unwrap_err();
        let has = |what: &str| assert!(p.iter().any(|l| l.contains(what)), "{what}\n{p:#?}");
        has("GAP-001: the sites cell must be: `crates/kiln_sema/src/p.rs::a`");
        has("GAP-001: row WP-11 is `done`");
        has("GAP-002: row `WP-12` is not a docs/BACKLOG.md row");
        has("GAP-003 has no site left");
        assert_eq!(p.len(), 4, "{p:#?}");
        let fwd = "fn g() {\n    // kiln-gap: forward\n    todo!()\n}\n";
        let sites = scan_file("crates/kiln_sema/src/g.rs", fwd);
        let p = check_with(&sites, &inv(""), BACKLOG_ROWS).unwrap_err();
        assert!(p[0].contains("outside a gap wrapper"), "{p:#?}");
    }

    /// A marker no detected site uses marks the next line of code; a
    /// `check` marker is valid only on a comparison with a gap id.
    #[test]
    fn standalone_and_check_markers() {
        let src = "fn m() {\n    // kiln-gap: GAP-004\n    self.unmangleable.push(at);\n    \
                   // kiln-gap: check\n    if d.id() == Some(DiagId::sema_vla_unsupported) {}\n    \
                   // kiln-gap: check\n    x.diag(DiagId::sema_vla_unsupported);\n}\n";
        let sites = scan_file("crates/kiln_abi/src/m.rs", src);
        let got: Vec<(usize, &str)> = sites.iter().map(|s| (s.line, s.what.as_str())).collect();
        assert_eq!(
            got,
            [
                (3, "marked"),
                (5, "DiagId::sema_vla_unsupported"),
                (7, "DiagId::sema_vla_unsupported")
            ]
        );
        let rows = "| GAP-004 | f | mangling | `x` | `crates/kiln_abi/src/m.rs::m` | 0 | WP-10 |\n";
        let p = check_with(&sites, &inv(rows), BACKLOG_ROWS).unwrap_err();
        assert_eq!(p.len(), 1, "{p:#?}");
        assert!(p[0].contains("m.rs:7") && p[0].contains("compares no gap DiagId"), "{p:#?}");
    }

    #[test]
    fn cells_and_lexing() {
        assert_eq!(sites_cell(["b::f", "a::g", "b::f"]), "`a::g`, `b::f` ×2");
        let (lines, lits) = lex("let x = r#\"a \"q\" b\"#; // c\n/* d\n e */ 'a' '\\n'\n");
        assert_eq!(lines[0].code, "let x = \"\"; ");
        assert_eq!(lines[0].comment, " c");
        assert!(lines[1].comment_only);
        assert_eq!(lines[2].code, " ' ' ' '");
        assert_eq!(lits[0].text, "a \"q\" b");
        assert!(is_gap_id("GAP-001") && !is_gap_id("GAP-01") && !is_gap_id("WP-001"));
    }
}
