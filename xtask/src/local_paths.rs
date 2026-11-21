//! The local-path lint (WP-1886; spec/harness.md "CI", step 0): no tracked
//! text file holds an absolute path of a developer's machine.
//!
//! A *local path* is an absolute path that starts with `/Volumes/`,
//! `/Users/`, `/home/<user>/`, `/root/` or `/tmp/claude` and goes on
//! (a bare prefix, as in this sentence, is not one), where the `/`
//! starts a path: at the start of a line, after a character that cannot
//! be part of a path (a space, a quote, a backtick, `=`, `(`, ...), or
//! after a one-letter flag (`-I/home/...`). `a/root/b`, `<sysroot>/root`
//! and `$(pwd)/root` are not local paths.
//!
//! [`ALLOWED`] lists the prefixes that name no one's machine (Homebrew's
//! fixed Linux prefix `/home/linuxbrew/`), and [`ALLOWED_FILES`] the files
//! that must spell a local path (this lint's own tests). Files that are
//! not UTF-8 or that contain a NUL byte are binary and skipped.

use std::path::Path;
use std::process::Command;

/// The prefixes that make a path local.
pub const PREFIXES: &[&str] = &["/Volumes/", "/Users/", "/home/", "/root/", "/tmp/claude"];

/// Prefixes under [`PREFIXES`] that are the same on every machine.
pub const ALLOWED: &[&str] = &[
    // Homebrew on Linux installs into this fixed prefix (kiln's LLVM
    // search, spec/driver.md "LLVM installation"); it is no one's home.
    "/home/linuxbrew/",
];

/// Files (relative to the repository root) that may hold local paths.
pub const ALLOWED_FILES: &[&str] = &["xtask/src/local_paths.rs"];

/// Every local path in the tracked text files under `root`, as
/// `file:line: path` lines; `Err` also when `git ls-files` cannot run.
pub fn check(root: &Path) -> Result<(), Vec<String>> {
    // `safe.directory`: CI's container checks the repository out as
    // another user, where git refuses to read it ("dubious ownership").
    let out = Command::new("git")
        .args(["-c", "safe.directory=*", "ls-files", "-z"])
        .current_dir(root)
        .output()
        .map_err(|e| vec![format!("cannot run `git ls-files`: {e}")])?;
    if !out.status.success() {
        return Err(vec![format!(
            "`git ls-files` failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )]);
    }
    let mut problems = Vec::new();
    for rel in out.stdout.split(|&b| b == 0).filter(|r| !r.is_empty()) {
        let rel = String::from_utf8_lossy(rel);
        if ALLOWED_FILES.contains(&rel.as_ref()) {
            continue;
        }
        // A tracked file deleted in the working tree, or a submodule.
        let Ok(bytes) = std::fs::read(root.join(rel.as_ref())) else { continue };
        if bytes.contains(&0) {
            continue;
        }
        let Ok(text) = std::str::from_utf8(&bytes) else { continue };
        for (i, line) in text.lines().enumerate() {
            for p in local_paths(line) {
                problems.push(format!("{rel}:{}: local absolute path `{p}`", i + 1));
            }
        }
    }
    if problems.is_empty() { Ok(()) } else { Err(problems) }
}

/// The local paths on `line`, each up to the next character that ends a
/// path.
pub fn local_paths(line: &str) -> Vec<&str> {
    let b = line.as_bytes();
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(off) = line[at..].find('/') {
        let i = at + off;
        at = i + 1;
        if !starts_path(b, i) {
            continue;
        }
        let rest = &line[i..];
        let Some(prefix) = PREFIXES.iter().find(|p| rest.starts_with(**p)) else { continue };
        // A bare prefix (`/Users/` in prose about this lint) names no path.
        if rest[prefix.len()..].bytes().next().is_none_or(|c| !is_path_byte(c)) {
            continue;
        }
        // `/home/<user>/`: a lower-case user name, then a slash.
        if *prefix == "/home/" {
            let user = &rest[prefix.len()..];
            let n = user.bytes().take_while(|c| is_user_byte(*c)).count();
            if n == 0
                || !user.as_bytes()[0].is_ascii_lowercase()
                || user.as_bytes().get(n) != Some(&b'/')
            {
                continue;
            }
        }
        if ALLOWED.iter().any(|a| rest.starts_with(a)) {
            continue;
        }
        let end = rest.bytes().position(ends_path).unwrap_or(rest.len());
        found.push(&rest[..end]);
        at = i + end.max(1);
    }
    found
}

/// Whether the `/` at `i` starts an absolute path.
fn starts_path(b: &[u8], i: usize) -> bool {
    let Some(&prev) = i.checked_sub(1).and_then(|j| b.get(j)) else { return true };
    // The end of a placeholder or an expansion: `<sysroot>/root`,
    // `${HOME}/x`, `$(pwd)/x`.
    if matches!(prev, b'>' | b'}' | b')' | b'$' | b'*') {
        return false;
    }
    if !is_path_byte(prev) {
        return true;
    }
    // A one-letter flag: `-I/home/x`, ` -L/Users/x`.
    prev.is_ascii_alphabetic() && i >= 2 && b[i - 2] == b'-' && (i == 2 || !is_path_byte(b[i - 3]))
}

fn is_path_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'/' | b'.' | b'_' | b'-' | b'~' | b'+' | b'@')
}

fn is_user_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-')
}

fn ends_path(c: u8) -> bool {
    c.is_ascii_whitespace() || matches!(c, b'"' | b'\'' | b'`' | b')' | b',' | b';' | b'>' | b']')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_local_paths() {
        assert_eq!(local_paths(r#""compiler": "/Volumes/W/x/kiln++","#), ["/Volumes/W/x/kiln++"]);
        assert_eq!(local_paths("cd /home/alice/src && make"), ["/home/alice/src"]);
        assert_eq!(
            local_paths("-I/home/bob/inc -L/Users/c/lib"),
            ["/home/bob/inc", "/Users/c/lib"]
        );
        assert_eq!(local_paths("`/root/.cargo/bin`"), ["/root/.cargo/bin"]);
        assert_eq!(local_paths("/tmp/claude-0/s/x.h"), ["/tmp/claude-0/s/x.h"]);
        assert_eq!(local_paths("KILN_TARGET_ROOT=/Volumes/A/.build"), ["/Volumes/A/.build"]);
    }

    #[test]
    fn leaves_other_paths() {
        for l in [
            "/home/linuxbrew/.linuxbrew/opt/llvm",
            "<sysroot>/root/x and a/root/b",
            "/home/ alone, /home/Upper/x, /home/x",
            "/tmp/x/y.h /usr/bin/clang++-22 /mnt/work",
            "target/release/kiln++",
            "the prefixes `/Volumes/`, `/Users/`, `/root/` or `/tmp/claude`",
        ] {
            assert!(local_paths(l).is_empty(), "{l}: {:?}", local_paths(l));
        }
    }

    /// The committed tree is clean (what `ci --step lints` checks).
    #[test]
    fn the_tree_has_no_local_paths() {
        if let Err(p) = check(&crate::util::repo_root()) {
            // Outside a git checkout (a source archive) there is nothing to list.
            assert!(p.iter().any(|m| m.contains("git ls-files")), "{}", p.join("\n"));
        }
    }
}
