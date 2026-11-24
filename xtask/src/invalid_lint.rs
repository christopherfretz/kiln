//! Test-side lint for the trap ledger (WP-3281, spec/sema-templates.md
//! §5.7.3 "Silent failures"): Sema and the parser make a declaration invalid
//! only through `Sema::invalidate` / `Sema::invalidate_as` (or create it
//! invalid and call `Sema::note_born_invalid`), and open and end a SFINAE
//! context only through `Sema::push_sfinae_state` / `pop_sfinae_state`,
//! so that every invalid declaration whose errors were trapped is marked
//! (`DeclArena::subst_failed`).
//!
//! The scan is textual. In `crates/kiln_sema/src` and
//! `crates/kiln_parse/src` (outside the ledger, `subst_failed.rs`), a line
//! that names `DeclFlags::INVALID` other than to test it (`has(`,
//! `contains(`, `.bits()`) is a finding, unless it carries
//! `// ledger: born invalid` (a `DeclHead` made invalid, followed by
//! `note_born_invalid`); so is a raw `sfinae.borrow_mut().push(` or
//! `.pop()`. Code after `#[cfg(test)]` and comments are not scanned.

#![cfg(test)]

use std::path::{Path, PathBuf};

/// The marker that allows a creation-time `INVALID` on its line.
const ALLOW: &str = "// ledger: born invalid";

/// Directories scanned recursively, relative to the repo root.
const DIRS: &[&str] = &["crates/kiln_sema/src", "crates/kiln_parse/src"];

/// The ledger itself.
const LEDGER: &str = "crates/kiln_sema/src/subst_failed.rs";

/// One finding per offending line: `path:line: text`.
fn scan(path: &str, src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, line) in src.lines().enumerate() {
        if line.trim_start().starts_with("#[cfg(test)]") {
            break;
        }
        let code = line.find("//").map_or(line, |k| &line[..k]);
        if raw_invalid(code) && !line.contains(ALLOW) || raw_sfinae(code) {
            out.push(format!("{path}:{}: {}", i + 1, line.trim()));
        }
    }
    out
}

/// Whether `code` names `DeclFlags::INVALID` other than to test it.
fn raw_invalid(code: &str) -> bool {
    let mut rest = code;
    while let Some(k) = rest.find("DeclFlags::INVALID") {
        let before = rest[..k].trim_end_matches("kiln_ast::decl::");
        let after = &rest[k + "DeclFlags::INVALID".len()..];
        let tested = before.ends_with("has(")
            || before.ends_with("contains(")
            || after.starts_with(".bits()");
        if !tested {
            return true;
        }
        rest = after;
    }
    false
}

/// Whether `code` pushes or pops the SFINAE stack directly.
fn raw_sfinae(code: &str) -> bool {
    code.contains("sfinae.borrow_mut().push(") || code.contains("sfinae.borrow_mut().pop()")
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
fn invalid_declarations_go_through_the_trap_ledger() {
    let root = crate::util::repo_root();
    let mut files = Vec::new();
    for d in DIRS {
        rust_files(&root.join(d), &mut files);
    }
    let mut found = Vec::new();
    for f in &files {
        let rel = f.strip_prefix(&root).unwrap_or(f).display().to_string();
        if rel == LEDGER {
            continue;
        }
        let src = std::fs::read_to_string(f).expect("readable source");
        found.extend(scan(&rel, &src));
    }
    assert!(
        found.is_empty(),
        "an invalid declaration or a SFINAE context bypasses the trap ledger; use \
         Sema::invalidate / invalidate_as (or `{ALLOW}` with note_born_invalid), \
         push_sfinae_state / pop_sfinae_state (spec/sema-templates.md §5.7.3):\n{}",
        found.join("\n")
    );
}

#[test]
fn lint_flags_raw_invalid_and_sfinae_stack_use() {
    let src = "\
fn f() {
    self.ast.decls.set_flags(d, DeclFlags::INVALID);
    if self.ast.decls.get(d).has(DeclFlags::INVALID) {}
    h.flags = h.flags.with(DeclFlags::INVALID);
    h.flags = h.flags.with(DeclFlags::INVALID); // ledger: born invalid
    if n.has(kiln_ast::decl::DeclFlags::INVALID) || x & DeclFlags::INVALID.bits() != 0 {}
    self.inst.sfinae.borrow_mut().push(SfinaeState::default());
    let st = self.inst.sfinae.borrow_mut().pop();
    // self.ast.decls.set_flags(d, DeclFlags::INVALID);
    let f = DeclFlags::IMPLICIT.with(DeclFlags::INVALID);
}
#[cfg(test)]
fn g() { self.ast.decls.set_flags(d, DeclFlags::INVALID); }
";
    let found = scan("x.rs", src);
    let lines: Vec<&str> = found.iter().map(|f| f.split(':').nth(1).unwrap_or_default()).collect();
    assert_eq!(lines, ["2", "4", "7", "8", "10"], "{found:#?}");
}
