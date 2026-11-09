//! Links in Markdown documents (WP-1027): `cargo xtask dist` rewrites the
//! relative links of the documents it installs and then checks the
//! installed tree with [`check_tree`] (spec/harness.md "Dist").
//!
//! A link is the target of an inline link or image (`[text](target)`,
//! `![alt](target "title")`) or of an HTML `src="..."` / `href="..."`
//! attribute, outside fenced code blocks and inline code spans. Reference
//! definitions (`[x]: target`) are not read: the documents use them only
//! for absolute URLs.

use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;

/// The byte ranges of the link targets in `text`, in order.
pub fn links(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut fence: Option<&str> = None;
    let mut off = 0;
    for line in text.split_inclusive('\n') {
        let start = off;
        off += line.len();
        let t = line.trim_start();
        if let Some(f) = fence {
            if t.starts_with(f) {
                fence = None;
            }
            continue;
        }
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = Some(&t[..3]);
            continue;
        }
        // Outside inline code: the even-numbered backtick-separated pieces.
        let mut pos = start;
        for (i, piece) in line.split('`').enumerate() {
            if i % 2 == 0 {
                scan_piece(piece, pos, &mut out);
            }
            pos += piece.len() + 1;
        }
    }
    out
}

fn scan_piece(piece: &str, base: usize, out: &mut Vec<Range<usize>>) {
    let mut found: Vec<Range<usize>> = Vec::new();
    for (i, _) in piece.match_indices("](") {
        let s = i + 2;
        let len = piece[s..].find(|c: char| c == ')' || c.is_whitespace()).unwrap_or(0);
        if len > 0 {
            found.push(base + s..base + s + len);
        }
    }
    for attr in ["src=\"", "href=\""] {
        for (i, _) in piece.match_indices(attr) {
            let s = i + attr.len();
            if let Some(len) = piece[s..].find('"').filter(|&l| l > 0) {
                found.push(base + s..base + s + len);
            }
        }
    }
    found.sort_by_key(|r| r.start);
    out.extend(found);
}

/// Whether `target` leaves the document tree: a URL or a `mailto:`.
pub fn is_external(target: &str) -> bool {
    target.contains("://") || target.starts_with("mailto:")
}

/// `target` split into its path and its anchor (`#x`, without the `#`).
pub fn split_anchor(target: &str) -> (&str, Option<&str>) {
    match target.split_once('#') {
        Some((p, a)) => (p, Some(a)),
        None => (target, None),
    }
}

/// The relative path `rel`, resolved against the directory of `from`
/// (both `/`-separated and relative to one root), without a trailing
/// `/`; `None` if it climbs above the root.
pub fn resolve(from: &str, rel: &str) -> Option<String> {
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for c in rel.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            c => parts.push(c),
        }
    }
    Some(parts.join("/"))
}

/// GitHub's anchor for a heading: lower case, punctuation other than `-`
/// and `_` dropped, spaces as `-`.
pub fn slug(heading: &str) -> String {
    heading
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' '))
        .flat_map(char::to_lowercase)
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// The anchors `text` defines: each ATX heading's [`slug`] (a repeated
/// one as GitHub numbers it, `x-1`, `x-2`, ...), and each HTML `id="..."`
/// or `name="..."` attribute.
pub fn anchors(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut fence: Option<&str> = None;
    for line in text.lines() {
        let t = line.trim_start();
        if let Some(f) = fence {
            if t.starts_with(f) {
                fence = None;
            }
            continue;
        }
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = Some(&t[..3]);
            continue;
        }
        let hashes = line.len() - line.trim_start_matches('#').len();
        if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
            let s = slug(line[hashes..].trim_end_matches('#'));
            let n = seen.entry(s.clone()).or_insert(0);
            out.push(if *n == 0 { s } else { format!("{s}-{n}") });
            *n += 1;
        }
        for attr in ["id=\"", "name=\""] {
            for (i, _) in line.match_indices(attr) {
                let s = i + attr.len();
                if let Some(len) = line[s..].find('"') {
                    out.push(line[s..s + len].to_owned());
                }
            }
        }
    }
    out
}

/// The broken local links of the Markdown files under `dir`
/// (recursively), each as `<file>: <target>` with `<file>` relative to
/// `dir`: a target that is neither a file nor a directory, one outside
/// `dir`, and an anchor that the target Markdown file (or the file
/// itself, for `#x`) does not define.
pub fn check_tree(dir: &Path) -> Vec<String> {
    let mut files = Vec::new();
    collect_md(dir, "", &mut files);
    let mut bad = Vec::new();
    for rel in &files {
        let Ok(text) = std::fs::read_to_string(dir.join(rel)) else {
            bad.push(format!("{rel}: unreadable"));
            continue;
        };
        for r in links(&text) {
            let target = &text[r];
            if is_external(target) {
                continue;
            }
            let (path, anchor) = split_anchor(target);
            let Some(resolved) = (if path.is_empty() {
                Some(rel.clone())
            } else {
                resolve(rel, path)
            }) else {
                bad.push(format!("{rel}: {target} (outside the tree)"));
                continue;
            };
            let p = dir.join(&resolved);
            if !p.exists() {
                bad.push(format!("{rel}: {target}"));
            } else if let Some(a) = anchor
                && p.extension().is_some_and(|e| e == "md")
                && !std::fs::read_to_string(&p).is_ok_and(|t| anchors(&t).iter().any(|x| x == a))
            {
                bad.push(format!("{rel}: {target} (no such anchor)"));
            }
        }
    }
    bad
}

fn collect_md(dir: &Path, prefix: &str, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.filter_map(Result::ok).collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for e in entries {
        let name = e.file_name().to_string_lossy().into_owned();
        let rel = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
        let p = e.path();
        if p.is_dir() {
            collect_md(&p, &rel, out);
        } else if name.ends_with(".md") {
            out.push(rel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets(text: &str) -> Vec<&str> {
        links(text).into_iter().map(|r| &text[r]).collect()
    }

    #[test]
    fn finds_links_outside_code() {
        let t = "See [a](docs/A.md#x) and ![i](i.svg \"t\"), `[no](code)`.\n\
                 ```\n[no](fenced)\n```\n<img src=\"logo.svg\"> [b](https://x.org/y)\n";
        assert_eq!(targets(t), ["docs/A.md#x", "i.svg", "logo.svg", "https://x.org/y"]);
    }

    #[test]
    fn resolves_relative_paths() {
        assert_eq!(resolve("README.md", "docs/A.md").as_deref(), Some("docs/A.md"));
        assert_eq!(resolve("docs/U.md", "../SECURITY.md").as_deref(), Some("SECURITY.md"));
        assert_eq!(resolve("docs/U.md", "status/").as_deref(), Some("docs/status"));
        assert_eq!(resolve("README.md", "../x"), None);
    }

    #[test]
    fn github_anchors() {
        assert_eq!(slug("Run-time dependencies"), "run-time-dependencies");
        assert_eq!(slug("C++20 modules"), "c20-modules");
        assert_eq!(slug("crates.io: not published"), "cratesio-not-published");
        assert_eq!(slug("[1.0.0] - unreleased"), "100---unreleased");
        let a = anchors("# T\n## Notes\n```\n# not\n```\n## Notes\n<a id=\"x\"></a>\n");
        assert_eq!(a, ["t", "notes", "notes-1", "x"]);
    }

    #[test]
    fn checks_a_tree() {
        let d = std::env::temp_dir().join(format!("xtask-doclinks-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("A.md"), "# A\n## Part one\n[b](B.md) [s](#part-one)\n").unwrap();
        std::fs::write(d.join("B.md"), "[a](A.md#part-one) [x](A.md#nope) [m](M.md) [u](../U.md)\n")
            .unwrap();
        assert_eq!(
            check_tree(&d),
            ["B.md: A.md#nope (no such anchor)", "B.md: M.md", "B.md: ../U.md (outside the tree)"]
        );
        std::fs::remove_dir_all(&d).unwrap();
    }
}
