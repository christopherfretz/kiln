//! The per-WP "Landed" spec files (`spec/harness.md` "CI", step 0;
//! `spec/README.md` "Landed sections").
//!
//! A work package's "Landed" section lives in its own file,
//! `spec/landed/<spec>/WP-NNN.md`, rather than appended to the end of
//! `spec/<spec>.md`, so two open pull requests never conflict on it.
//! [`check`] is the lint `cargo xtask ci` runs: each such file names an
//! existing spec, is titled `# WP-NNN — <title>` after its own file name,
//! and its WP has a row in `docs/BACKLOG.md`.

use std::collections::BTreeSet;
use std::path::Path;

/// The directory of the per-WP files, relative to the repository root.
pub const DIR: &str = "spec/landed";

/// The backlog whose rows the files must match, relative to the root.
const BACKLOG: &str = "docs/BACKLOG.md";

/// Checks every file under [`DIR`]; `Err` lists each problem, one per line,
/// each naming the file it is about. A missing [`DIR`] is not a problem.
pub fn check(root: &Path) -> Result<(), Vec<String>> {
    let dir = root.join(DIR);
    if !dir.is_dir() {
        return Ok(());
    }
    let backlog = std::fs::read_to_string(root.join(BACKLOG))
        .map_err(|e| vec![format!("cannot read {BACKLOG}: {e}")])?;
    let rows = backlog_ids(&backlog);
    let mut files = Vec::new();
    let mut problems = Vec::new();
    collect(&dir, &mut files).map_err(|e| vec![format!("cannot list {DIR}: {e}")])?;
    files.sort();
    for path in &files {
        let rel = path.strip_prefix(root).unwrap_or(path).display().to_string();
        let sub = path.strip_prefix(&dir).unwrap_or(path);
        let parts: Vec<&str> = sub.iter().filter_map(|c| c.to_str()).collect();
        if parts == ["README.md"] {
            continue;
        }
        let [spec, file] = parts[..] else {
            problems.push(format!("{rel}: not of the form {DIR}/<spec>/WP-NNN.md"));
            continue;
        };
        if !root.join("spec").join(format!("{spec}.md")).is_file() {
            problems.push(format!("{rel}: there is no spec/{spec}.md"));
        }
        let Some(id) = file.strip_suffix(".md").filter(|id| is_wp_id(id)) else {
            problems.push(format!("{rel}: the file name is not WP-NNN.md"));
            continue;
        };
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                problems.push(format!("{rel}: cannot read: {e}"));
                continue;
            }
        };
        let first = text.lines().next().unwrap_or("");
        let title = first.strip_prefix(&format!("# {id} — ")).map(str::trim);
        if title.is_none_or(str::is_empty) {
            problems.push(format!("{rel}: the first line must be `# {id} — <title>`"));
        }
        if !rows.contains(id) {
            problems.push(format!("{rel}: {BACKLOG} has no `| {id} |` row"));
        }
    }
    if problems.is_empty() { Ok(()) } else { Err(problems) }
}

/// Every file below `dir`, recursively.
fn collect(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

/// `WP-` followed by digits and at most one lowercase letter (`WP-068a`).
fn is_wp_id(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("WP-") else { return false };
    let digits = rest.trim_end_matches(|c: char| c.is_ascii_lowercase());
    rest.len() - digits.len() <= 1
        && !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
}

/// The ids of the backlog's table rows, `| WP-NNN | … |`.
fn backlog_ids(backlog: &str) -> BTreeSet<&str> {
    backlog
        .lines()
        .filter_map(|l| l.strip_prefix('|')?.split('|').next().map(str::trim))
        .filter(|id| is_wp_id(id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("xtask-landed-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        for (rel, text) in files {
            let p = d.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        d
    }

    const BACKLOG_ROWS: &str = "| id | scope | needs | status |\n|---|---|---|---|\n\
                                | WP-100 | x | — | done |\n| WP-068a | y | — | open |\n";

    /// spec/README.md "Landed sections": well-formed files pass, and a
    /// missing directory is not an error.
    #[test]
    fn well_formed_files_pass() {
        let d = tree(
            "ok",
            &[
                ("docs/BACKLOG.md", BACKLOG_ROWS),
                ("spec/codegen.md", "# spec\n"),
                ("spec/landed/README.md", "# Landed\n"),
                ("spec/landed/codegen/WP-100.md", "# WP-100 — exceptions\n\nBody.\n"),
                ("spec/landed/codegen/WP-068a.md", "# WP-068a — part a\n"),
            ],
        );
        assert_eq!(check(&d), Ok(()));
        let empty = tree("none", &[("docs/BACKLOG.md", BACKLOG_ROWS)]);
        assert_eq!(check(&empty), Ok(()));
        std::fs::remove_dir_all(&d).unwrap();
        std::fs::remove_dir_all(&empty).unwrap();
    }

    /// Each kind of problem is reported, naming its file.
    #[test]
    fn problems_are_listed() {
        let d = tree(
            "bad",
            &[
                ("docs/BACKLOG.md", BACKLOG_ROWS),
                ("spec/codegen.md", "# spec\n"),
                ("spec/landed/codegen/WP-101.md", "# WP-101 — no row\n"),
                ("spec/landed/codegen/WP-100.md", "# WP-101 — wrong id\n"),
                ("spec/landed/nospec/WP-100.md", "# WP-100 — x\n"),
                ("spec/landed/codegen/notes.md", "# notes\n"),
                ("spec/landed/stray.md", "# stray\n"),
                ("spec/landed/codegen/WP-068a.md", "# WP-068a — \n"),
            ],
        );
        let p = check(&d).unwrap_err();
        let has = |file: &str, what: &str| {
            assert!(
                p.iter().any(|l| l.contains(file) && l.contains(what)),
                "{file}: {what}\n{p:#?}"
            );
        };
        has("codegen/WP-101.md", "has no `| WP-101 |` row");
        has("codegen/WP-100.md", "must be `# WP-100 — <title>`");
        has("nospec/WP-100.md", "there is no spec/nospec.md");
        has("codegen/notes.md", "not WP-NNN.md");
        has("landed/stray.md", "not of the form");
        has("codegen/WP-068a.md", "must be `# WP-068a — <title>`");
        assert_eq!(p.len(), 6, "{p:#?}");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn wp_ids() {
        for ok in ["WP-001", "WP-488", "WP-068a"] {
            assert!(is_wp_id(ok), "{ok}");
        }
        for bad in ["WP-", "WP-ab", "WP-1ab", "wp-001", "WP-12-3", "id"] {
            assert!(!is_wp_id(bad), "{bad}");
        }
    }
}
