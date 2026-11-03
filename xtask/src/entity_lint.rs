//! Test-side lint for entity identity (WP-471, spec/sema.md §4 "Entity
//! identity"): reflection code compares declarations with
//! `Sema::same_entity`, never as raw `DeclId`s, because one entity has
//! many declarations (`struct S; struct S { ... };`, an out-of-line
//! definition, a friend declaration) and a reflection names the first.
//!
//! The scan is textual and deliberately approximate (xtask has no Rust
//! type checker). In the files it covers, an operand of `==`/`!=`, or an
//! argument of `assert_eq!`/`assert_ne!`, counts as a declaration when it
//! (after `&`, `*` and a `Some(..)` wrapper) is:
//! - a name the file binds as a `DeclId` (`d: DeclId`, `d: Option<DeclId>`,
//!   `Reflectee::Decl(d)`, `let d = <declaration-valued call>`), or a
//!   field path ending in such a name (`c.record` when `record: DeclId`
//!   appears in the file);
//! - a call of a declaration-valued accessor (`first(x)`, `latest(x)`,
//!   `definition(x)`, `as_record(x)`, `resolve_alias(x)`, `ctx()`) or a
//!   `DeclId::` constructor.
//!
//! A line that compares on purpose carries `// same_entity: raw ok` (on
//! the line or the one above). Code after `#[cfg(test)]` is not scanned.

#![cfg(test)]

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The marker that allows a raw comparison on its line (or the next).
const ALLOW: &str = "// same_entity: raw ok";

/// Directories scanned recursively and single files, relative to the repo
/// root; missing ones are skipped (they may not exist yet).
const DIRS: &[&str] = &["crates/kiln_sema/src/meta", "crates/kiln_oven/src/meta"];
const FILES: &[&str] = &["crates/kiln_sema/src/reflect.rs"];

/// Calls whose value is a declaration (with at least one argument, so
/// that `slice.first()` is not one), and those taking none.
const PRODUCERS: &[&str] = &["first", "latest", "definition", "as_record", "resolve_alias"];
const PRODUCERS_NULLARY: &[&str] = &["ctx"];

/// One finding: `path:line: text`.
fn scan(path: &str, src: &str) -> Vec<String> {
    let lines: Vec<&str> = src.lines().collect();
    let end = lines.iter().position(|l| l.trim_start().starts_with("#[cfg(test)]"));
    let lines = &lines[..end.unwrap_or(lines.len())];
    let code: Vec<&str> = lines.iter().map(|l| strip_comment(l)).collect();
    let names = decl_names(&code);
    let mut out = Vec::new();
    for (i, line) in code.iter().enumerate() {
        let allowed =
            lines[i].contains(ALLOW) || (i > 0 && lines[i - 1].trim_start().starts_with(ALLOW));
        if allowed {
            continue;
        }
        let raw =
            comparisons(line).into_iter().any(|(a, b)| is_decl(&a, &names) || is_decl(&b, &names));
        if raw {
            out.push(format!("{path}:{}: {}", i + 1, lines[i].trim()));
        }
    }
    out
}

/// `line` without a trailing `//` comment (string literals holding `//`
/// are rare enough in this code to ignore).
fn strip_comment(line: &str) -> &str {
    line.find("//").map_or(line, |i| &line[..i])
}

fn is_ident_char(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}

/// The identifier ending at byte `end` of `s`, if any.
fn ident_before(s: &str, end: usize) -> &str {
    let start = s[..end].rfind(|c: char| !is_ident_char(c)).map_or(0, |i| i + 1);
    &s[start..end]
}

/// The identifier starting at byte `start` of `s`.
fn ident_after(s: &str, start: usize) -> &str {
    let rest = &s[start..];
    let end = rest.find(|c: char| !is_ident_char(c)).unwrap_or(rest.len());
    &rest[..end]
}

/// The names the file binds to declarations.
fn decl_names(code: &[&str]) -> HashSet<String> {
    let mut names = HashSet::new();
    for line in code {
        for pat in [": DeclId", ": Option<DeclId>", ": &DeclId"] {
            let mut from = 0;
            while let Some(i) = line[from..].find(pat) {
                let at = from + i;
                let n = ident_before(line, at);
                if !n.is_empty() {
                    names.insert(n.to_owned());
                }
                from = at + pat.len();
            }
        }
        let mut from = 0;
        while let Some(i) = line[from..].find("Reflectee::Decl(") {
            let at = from + i + "Reflectee::Decl(".len();
            let n = ident_after(line, at);
            if !n.is_empty() && line[at + n.len()..].starts_with(')') {
                names.insert(n.to_owned());
            }
            from = at;
        }
        // `let d = <call>;` / `let Some(d) = <call> else`.
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("let ") {
            let rest = rest.strip_prefix("mut ").unwrap_or(rest);
            let rest = rest.strip_prefix("Some(").unwrap_or(rest);
            let n = ident_after(rest, 0);
            if let Some(eq) = rest.find(" = ")
                && !n.is_empty()
                && !rest.contains("==")
                && !rest.contains("!=")
                && is_decl(rest[eq + 3..].trim_end_matches(';').trim(), &HashSet::new())
            {
                names.insert(n.to_owned());
            }
        }
    }
    names
}

/// Whether operand `o` designates a declaration.
fn is_decl(o: &str, names: &HashSet<String>) -> bool {
    let mut o = o.trim();
    loop {
        let t = o.trim_start_matches(['&', '*']).trim();
        let t = match t.strip_prefix("Some(").and_then(|r| r.strip_suffix(')')) {
            Some(inner) => inner.trim(),
            None => t,
        };
        let t = match t.strip_prefix('(').and_then(|r| r.strip_suffix(')')) {
            Some(inner) => inner.trim(),
            None => t,
        };
        if t == o {
            break;
        }
        o = t;
    }
    if o.is_empty() {
        return false;
    }
    if o.starts_with("DeclId::") {
        return true;
    }
    if o.chars().all(|c| is_ident_char(c) || c == '.') {
        let last = o.rsplit('.').next().unwrap_or(o);
        return names.contains(last);
    }
    // A trailing call `...name(args)` (or `name(args)?`).
    let o = o.trim_end_matches('?');
    if let Some(body) = o.strip_suffix(')') {
        let mut depth = 0i32;
        for (i, c) in body.char_indices().rev() {
            match c {
                ')' => depth += 1,
                '(' if depth == 0 => {
                    let name = ident_before(body, i);
                    let args = body[i + 1..].trim();
                    return (PRODUCERS.contains(&name) && !args.is_empty())
                        || (PRODUCERS_NULLARY.contains(&name) && args.is_empty());
                }
                '(' => depth -= 1,
                _ => {}
            }
        }
    }
    false
}

/// The operand pairs of every `==`/`!=` and `assert_eq!`/`assert_ne!` on
/// `line`.
fn comparisons(line: &str) -> Vec<(String, String)> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < b.len() {
        let op = (b[i] == b'=' || b[i] == b'!') && b[i + 1] == b'=';
        let around_ok = (i == 0 || !matches!(b[i - 1], b'=' | b'<' | b'>' | b'!'))
            && b.get(i + 2) != Some(&b'=');
        if op && around_ok {
            out.push((left_operand(line, i), right_operand(line, i + 2)));
            i += 2;
            continue;
        }
        i += 1;
    }
    for mac in ["assert_eq!(", "assert_ne!("] {
        let mut from = 0;
        while let Some(k) = line[from..].find(mac) {
            let start = from + k + mac.len();
            let args = split_args(&line[start..]);
            if args.len() >= 2 {
                out.push((args[0].clone(), args[1].clone()));
            }
            from = start;
        }
    }
    out
}

/// The operand ending before byte `op` (scanning back to a boundary at
/// bracket depth 0).
fn left_operand(line: &str, op: usize) -> String {
    let b = line.as_bytes();
    let mut depth = 0i32;
    let mut i = op;
    while i > 0 {
        let c = b[i - 1];
        match c {
            b')' | b']' => depth += 1,
            b'(' | b'[' if depth == 0 => break,
            b'(' | b'[' => depth -= 1,
            b',' | b';' | b'{' | b'}' | b'|' | b'=' if depth == 0 => break,
            b'&' if depth == 0 && i >= 2 && b[i - 2] == b'&' => break,
            _ => {}
        }
        i -= 1;
    }
    let mut s = line[i..op].trim();
    // A match guard or a keyword-led condition: the operand follows it.
    for k in [" if ", " return "] {
        if let Some(j) = s.rfind(k) {
            s = &s[j + k.len()..];
        }
    }
    let s = ["return ", "if ", "while ", "!"].iter().fold(s, |s, k| s.strip_prefix(k).unwrap_or(s));
    s.trim().to_owned()
}

/// The operand starting at byte `from`.
fn right_operand(line: &str, from: usize) -> String {
    let b = line.as_bytes();
    let mut depth = 0i32;
    let mut i = from;
    while i < b.len() {
        let c = b[i];
        match c {
            b'(' | b'[' => depth += 1,
            b')' | b']' if depth == 0 => break,
            b')' | b']' => depth -= 1,
            b',' | b';' | b'{' | b'}' if depth == 0 => break,
            b'|' | b'&' if depth == 0 && b.get(i + 1) == Some(&c) => break,
            b'=' if depth == 0 && b.get(i + 1) == Some(&b'>') => break,
            _ => {}
        }
        i += 1;
    }
    line[from..i].trim().to_owned()
}

/// The depth-0 comma-separated arguments of a macro call whose `(` was
/// just consumed (up to its `)`, or the end of the line).
fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' if depth == 0 => {
                out.push(s[start..i].trim().to_owned());
                return out;
            }
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                out.push(s[start..i].trim().to_owned());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(s[start..].trim().to_owned());
    out
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn reflection_code_compares_entities_not_decl_ids() {
    let root = crate::util::repo_root();
    let mut files = Vec::new();
    for d in DIRS {
        rust_files(&root.join(d), &mut files);
    }
    files.extend(FILES.iter().map(|f| root.join(f)).filter(|p| p.is_file()));
    let mut found = Vec::new();
    for f in &files {
        let src = std::fs::read_to_string(f).expect("readable source");
        let rel = f.strip_prefix(&root).unwrap_or(f).display().to_string();
        found.extend(scan(&rel, &src));
    }
    assert!(
        found.is_empty(),
        "raw DeclId comparison in reflection code; use Sema::same_entity \
         (spec/sema.md §4 \"Entity identity\") or mark the line \
         `{ALLOW}`:\n{}",
        found.join("\n")
    );
}

#[test]
fn lint_flags_raw_decl_comparisons() {
    let src = "\
fn f(d: DeclId, e: Option<DeclId>) {
    if d == e.unwrap() {}
    let ok = self.ast.first(b) == self.ast.first(r);
    let t = self.ast.types.as_record(q) == Some(r);
    assert_eq!(d, x);
    let p = self.ast.first(x);
    if p != y {}
    if c.d == other {}
    match r { Reflectee::Decl(m) if m == z => {} _ => {} }
    if DeclId::new(3) == k {}
    if self.ast.decls.get(m).ctx() == Some(w) {}
}
";
    let found = scan("x.rs", src);
    let lines: Vec<&str> = found.iter().map(|f| f.split(':').nth(1).unwrap_or_default()).collect();
    assert_eq!(lines, ["2", "3", "4", "5", "7", "8", "9", "10", "11"], "{found:#?}");
}

#[test]
fn lint_ignores_other_comparisons_and_allowed_lines() {
    let src = "\
fn f(d: DeclId, name: &str) {
    if self.kind_of(d) == DeclKind::Namespace && name == \"std\" {}
    if t != QualType::ERROR || v.first() == Some(&1) {}
    if a <= b || a >= b || x => y {}
    if self.ast.first(a) == self.ast.first(b) {} // same_entity: raw ok
    // same_entity: raw ok
    if d == e {}
    let n = self.decl_name(d) == \"x\";
}
#[cfg(test)]
mod tests { fn g(d: DeclId) { assert_eq!(d, d); } }
";
    assert_eq!(scan("x.rs", src), Vec::<String>::new());
}
