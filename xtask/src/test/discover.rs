//! Test discovery (`spec/harness.md` "Discovery").

use std::fs;
use std::path::{Path, PathBuf};

use super::directives::Lang;

/// One test file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestCase {
    /// Absolute path.
    pub path: PathBuf,
    /// Display name and the path handed to compilers (run from the repo
    /// root): relative to the root when the file is under it, else absolute.
    pub name: String,
    pub lang: Lang,
    /// `.h` files are passed with `-x c++` (they would otherwise be headers).
    pub is_header: bool,
}

/// The default test roots that exist under `root`.
pub fn default_roots(root: &Path) -> Vec<PathBuf> {
    ["tests/conformance", "tests/differential"]
        .iter()
        .map(|d| root.join(d))
        .filter(|p| p.is_dir())
        .collect()
}

/// Expands `inputs` (files or directories) into a sorted, de-duplicated test
/// list, keeping names that contain `filter`.
pub fn discover(
    root: &Path,
    inputs: &[PathBuf],
    filter: Option<&str>,
) -> Result<Vec<TestCase>, String> {
    let mut out = Vec::new();
    for input in inputs {
        let meta = fs::metadata(input).map_err(|e| format!("{}: {e}", input.display()))?;
        if meta.is_dir() {
            walk(root, input, &mut out).map_err(|e| format!("{}: {e}", input.display()))?;
        } else {
            // Named explicitly: always a test, whatever the extension.
            out.push(case(root, input));
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.path == b.path);
    if let Some(f) = filter {
        out.retain(|t| t.name.contains(f));
    }
    Ok(out)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<TestCase>) -> std::io::Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for e in entries {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let path = e.path();
        let ty = e.file_type()?;
        if ty.is_dir() {
            // lit convention: `Inputs/` holds auxiliary files, never tests.
            if name != "Inputs" {
                walk(root, &path, out)?;
            }
            continue;
        }
        let is_test = match path.extension().and_then(|x| x.to_str()) {
            Some("cpp" | "c") => true,
            // A header is a test only if it carries a directive header;
            // otherwise it is an include file used by other tests.
            Some("h") => has_directives(&path),
            _ => false,
        };
        if is_test {
            out.push(case(root, &path));
        }
    }
    Ok(())
}

fn has_directives(path: &Path) -> bool {
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    String::from_utf8_lossy(&bytes)
        .lines()
        .map(str::trim)
        .take_while(|l| l.is_empty() || l.starts_with("//"))
        .any(|l| l.starts_with("//@"))
}

fn case(root: &Path, path: &Path) -> TestCase {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| path.to_path_buf(), |c| c.join(path))
    };
    let abs = fs::canonicalize(&abs).unwrap_or(abs);
    let root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let name = abs
        .strip_prefix(&root)
        .map_or_else(|_| abs.display().to_string(), |p| p.display().to_string());
    let ext = abs.extension().and_then(|x| x.to_str());
    TestCase {
        lang: if ext == Some("c") { Lang::C } else { Lang::Cxx },
        is_header: ext == Some("h"),
        name,
        path: abs,
    }
}
