//! Response files (`@file`), as clang's driver expands them
//! (`llvm::cl::ExpansionContext::expandResponseFiles` with the GNU
//! tokenizer): spec/driver.md "Response files".
//!
//! [`expand_with`] reads each file through a hook the embedder supplies
//! (clang's expansion over a virtual file system) and reports the files it
//! read; [`expand`] and [`expand_in`] read the host's file system.

use std::fmt;
use std::path::{Path, PathBuf};

/// A nesting depth no chain of distinct files reaches: past it, the
/// expansion is reported as recursive (a hook that answers each read with
/// new names would otherwise never end; clang has no such bound).
const MAX_DEPTH: usize = 512;

/// What a read hook answers for one response file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RspFile {
    /// The file's bytes.
    Bytes(Vec<u8>),
    /// There is no such file: the `@file` stays as written (it is then an
    /// input that does not exist), as in clang and libiberty.
    Missing,
    /// The file exists but cannot be read (a directory, no permission, a
    /// path through a file): the expansion stops with
    /// [`RspError::Unreadable`]. The text is the reason as clang prints
    /// it (`strerror`: `Is a directory`).
    Unreadable(String),
}

/// The result of [`expand_with`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Expansion {
    /// The arguments with every readable `@file` replaced by its words.
    pub words: Vec<String>,
    /// The response files read, in the order read (a nested one after the
    /// file that names it), each as the path the hook was asked for.
    pub files: Vec<PathBuf>,
}

/// Why an expansion stopped (clang prints the text and exits with 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RspError {
    /// A response file exists but could not be read.
    Unreadable {
        /// The path the hook was asked for.
        path: PathBuf,
        /// The reason ([`RspFile::Unreadable`]).
        reason: String,
    },
    /// A response file names itself, directly or through other ones.
    Recursive {
        /// The path of the `@file` that would be read again.
        path: PathBuf,
    },
}

impl fmt::Display for RspError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // clang's own wording, its typo included.
        match self {
            RspError::Unreadable { path, reason } => {
                write!(f, "cannot not open file '{}': {reason}", path.display())
            }
            RspError::Recursive { path } => {
                write!(f, "recursive expansion of: '{}'", path.display())
            }
        }
    }
}

impl std::error::Error for RspError {}

/// `argv` with each `@file` argument replaced by the arguments in `file`,
/// recursively, read from the process's current directory. An `@file`
/// that cannot be read stays as it is ([`expand_checked`] reports it).
#[must_use]
pub fn expand(argv: &[String]) -> Vec<String> {
    expand_in(argv, &kiln_source::WorkDir::process())
}

/// [`expand`] with every relative `@file` read from `work_dir` (a library
/// entry's working directory, spec/driver.md "Working directory"). On an
/// error, `argv` is returned as it is.
#[must_use]
pub fn expand_in(argv: &[String], work_dir: &kiln_source::WorkDir) -> Vec<String> {
    expand_checked(argv, work_dir).map_or_else(|_| argv.to_vec(), |e| e.words)
}

/// [`expand_with`] over the host's file system, relative names joined
/// onto `work_dir` (made absolute, as clang does, when it can be read).
///
/// # Errors
/// As [`expand_with`].
pub fn expand_checked(
    argv: &[String],
    work_dir: &kiln_source::WorkDir,
) -> Result<Expansion, RspError> {
    if !has_response_file(argv) {
        return Ok(Expansion { words: argv.to_vec(), files: Vec::new() });
    }
    let dir = work_dir.current().unwrap_or_default();
    expand_with(argv, &dir, &mut |p: &Path| read_host(work_dir, p))
}

/// Whether some argument of `argv` is an `@file`.
#[must_use]
pub fn has_response_file(argv: &[String]) -> bool {
    argv.iter().any(|a| a.len() > 1 && a.starts_with('@'))
}

/// One response file from the host's file system: clang keeps an `@file`
/// whose `stat` fails with "no such file", and reports every other
/// failure.
fn read_host(work_dir: &kiln_source::WorkDir, path: &Path) -> RspFile {
    match std::fs::read(work_dir.join(path)) {
        Ok(b) => RspFile::Bytes(b),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => RspFile::Missing,
        Err(e) => RspFile::Unreadable(strerror(&e)),
    }
}

/// An I/O error's text without Rust's ` (os error N)` suffix, as clang's
/// `std::error_code::message()` prints it.
fn strerror(e: &std::io::Error) -> String {
    let s = e.to_string();
    match s.rsplit_once(" (os error ") {
        Some((text, _)) => text.to_owned(),
        None => s,
    }
}

/// `argv` with each `@file` argument replaced by the words of `file`,
/// recursively, as clang's driver expands them over a virtual file system
/// (`ExpansionContext` with `RelativeNames` off): a relative name,
/// nested ones too, is joined onto `dir` (an empty `dir` joins nothing),
/// and `read` is asked for that path. A [`RspFile::Missing`] file stays
/// as written; an empty `@` is no response file.
///
/// # Errors
/// The first file `read` answers [`RspFile::Unreadable`] for, or a file
/// that names itself, directly or through others (compared by path, `.`
/// components aside).
pub fn expand_with(
    argv: &[String],
    dir: &Path,
    read: &mut dyn FnMut(&Path) -> RspFile,
) -> Result<Expansion, RspError> {
    let mut x = Expander { dir, read, out: Expansion::default(), stack: Vec::new() };
    x.expand(argv)?;
    Ok(x.out)
}

struct Expander<'a> {
    dir: &'a Path,
    read: &'a mut dyn FnMut(&Path) -> RspFile,
    out: Expansion,
    /// The files being expanded, outermost first, normalized.
    stack: Vec<PathBuf>,
}

impl Expander<'_> {
    fn expand(&mut self, argv: &[String]) -> Result<(), RspError> {
        for a in argv {
            let Some(name) = a.strip_prefix('@').filter(|p| !p.is_empty()) else {
                self.out.words.push(a.clone());
                continue;
            };
            let path = self.dir.join(name);
            let key: PathBuf = path.components().collect();
            if self.stack.contains(&key) || self.stack.len() >= MAX_DEPTH {
                return Err(RspError::Recursive { path });
            }
            match (self.read)(&path) {
                RspFile::Missing => self.out.words.push(a.clone()),
                RspFile::Unreadable(reason) => return Err(RspError::Unreadable { path, reason }),
                RspFile::Bytes(bytes) => {
                    self.out.files.push(path);
                    let words = tokenize_gnu(&String::from_utf8_lossy(&bytes));
                    self.stack.push(key);
                    self.expand(&words)?;
                    self.stack.pop();
                }
            }
        }
        Ok(())
    }
}

/// Splits `s` as `llvm::cl::TokenizeGNUCommandLine`: whitespace separates
/// arguments; `'` and `"` quote (an empty pair is an empty argument); a
/// backslash takes the next character literally, inside quotes too.
#[must_use]
pub fn tokenize_gnu(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut tok = String::new();
    let mut in_tok = false;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if in_tok {
                    out.push(std::mem::take(&mut tok));
                    in_tok = false;
                }
            }
            '\\' => {
                in_tok = true;
                if let Some(n) = chars.next() {
                    tok.push(n);
                }
            }
            '\'' | '"' => {
                in_tok = true;
                while let Some(n) = chars.next() {
                    if n == c {
                        break;
                    }
                    if n == '\\'
                        && let Some(m) = chars.next()
                    {
                        tok.push(m);
                        continue;
                    }
                    tok.push(n);
                }
            }
            c => {
                in_tok = true;
                tok.push(c);
            }
        }
    }
    if in_tok {
        out.push(tok);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn words(s: &[&str]) -> Vec<String> {
        s.iter().map(|&w| w.to_owned()).collect()
    }

    /// A hook over an in-memory tree; every path it was asked for.
    fn tree<'t>(
        files: &'t HashMap<&'t str, RspFile>,
        asked: &'t mut Vec<String>,
    ) -> impl FnMut(&Path) -> RspFile + 't {
        move |p: &Path| {
            let p = p.to_string_lossy().into_owned();
            let r = files.get(p.as_str()).cloned().unwrap_or(RspFile::Missing);
            asked.push(p);
            r
        }
    }

    #[test]
    fn tokenizes_like_gnu() {
        assert_eq!(tokenize_gnu("a b\n\tc"), ["a", "b", "c"]);
        assert_eq!(tokenize_gnu("'a b' \"c d\" e\\ f"), ["a b", "c d", "e f"]);
        assert_eq!(tokenize_gnu("x\"\" '' y"), ["x", "", "y"]);
        assert_eq!(tokenize_gnu("\"a\\\"b\" -DX=\\\"s\\\""), ["a\"b", "-DX=\"s\""]);
        assert!(tokenize_gnu("  \n").is_empty());
    }

    #[test]
    fn expands_nested_files() {
        let dir = std::env::temp_dir();
        let inner = dir.join(format!("kiln-rsp-inner-{}", std::process::id()));
        let outer = dir.join(format!("kiln-rsp-outer-{}", std::process::id()));
        std::fs::write(&inner, "b.o 'c d.o'\n").unwrap();
        std::fs::write(&outer, format!("a.o @{}\n-o x", inner.display())).unwrap();
        let v = vec!["-c".to_owned(), format!("@{}", outer.display()), "e.o".to_owned()];
        let got = expand(&v);
        let checked = expand_checked(&v, &kiln_source::WorkDir::process());
        let _ = std::fs::remove_file(&inner);
        let _ = std::fs::remove_file(&outer);
        assert_eq!(got, ["-c", "a.o", "b.o", "c d.o", "-o", "x", "e.o"]);
        assert_eq!(checked.map(|e| e.files), Ok(vec![outer, inner]));
    }

    #[test]
    fn missing_file_and_plain_at_stay() {
        let v = vec!["@".to_owned(), "@/nonexistent/kiln-rsp".to_owned(), "-c".to_owned()];
        assert_eq!(expand(&v), v);
        let e = expand_checked(&v, &kiln_source::WorkDir::process()).unwrap();
        assert_eq!(e.words, v);
        assert!(e.files.is_empty());
    }

    /// A directory exists but cannot be read as a file: clang's error.
    #[test]
    fn host_directory_is_unreadable() {
        let dir = std::env::temp_dir();
        let wd = kiln_source::WorkDir::at(&dir).unwrap();
        let v = words(&["-c", "@."]);
        let e = expand_checked(&v, &wd).unwrap_err();
        assert_eq!(
            e.to_string(),
            format!("cannot not open file '{}': Is a directory", dir.join(".").display())
        );
        // The lenient wrapper keeps the command line as it was.
        assert_eq!(expand_in(&v, &wd), v);
    }

    /// Through the hook: nested files are relative to `dir`, not to the
    /// file naming them; every file read is reported, in order.
    #[test]
    fn hook_nested_relative_quoted() {
        let files: HashMap<&str, RspFile> = [
            ("/w/sub/outer.rsp", RspFile::Bytes(b"-DA=1 @inner.rsp 'x y'".to_vec())),
            ("/w/inner.rsp", RspFile::Bytes(b"-DTOP \"-DQ=\\\"s\\\"\" @sub/last.rsp".to_vec())),
            ("/w/sub/inner.rsp", RspFile::Bytes(b"-DSUB".to_vec())),
            ("/w/sub/last.rsp", RspFile::Bytes(Vec::new())),
            ("/abs.rsp", RspFile::Bytes(b"-c".to_vec())),
        ]
        .into_iter()
        .collect();
        let mut asked = Vec::new();
        let mut hook = tree(&files, &mut asked);
        let v = words(&["@sub/outer.rsp", "t.cpp", "@/abs.rsp", "@gone.rsp", "@"]);
        let e = expand_with(&v, Path::new("/w"), &mut hook).unwrap();
        assert_eq!(
            e.words,
            ["-DA=1", "-DTOP", "-DQ=\"s\"", "x y", "t.cpp", "-c", "@gone.rsp", "@"]
        );
        let read: Vec<_> = e.files.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        assert_eq!(read, ["/w/sub/outer.rsp", "/w/inner.rsp", "/w/sub/last.rsp", "/abs.rsp"]);
        drop(hook);
        assert_eq!(asked.last().map(String::as_str), Some("/w/gone.rsp"));
    }

    #[test]
    fn hook_unreadable_stops() {
        let files: HashMap<&str, RspFile> = [
            ("/w/a.rsp", RspFile::Bytes(b"-DA @d".to_vec())),
            ("/w/d", RspFile::Unreadable("Is a directory".to_owned())),
        ]
        .into_iter()
        .collect();
        let mut asked = Vec::new();
        let e = expand_with(&words(&["@a.rsp"]), Path::new("/w"), &mut tree(&files, &mut asked))
            .unwrap_err();
        assert_eq!(e.to_string(), "cannot not open file '/w/d': Is a directory");
    }

    #[test]
    fn hook_recursion_is_an_error() {
        let files: HashMap<&str, RspFile> = [
            ("/w/self.rsp", RspFile::Bytes(b"-DS @./self.rsp".to_vec())),
            ("/w/a.rsp", RspFile::Bytes(b"@b.rsp".to_vec())),
            ("/w/b.rsp", RspFile::Bytes(b"@a.rsp".to_vec())),
            ("/w/twice.rsp", RspFile::Bytes(b"@leaf.rsp @leaf.rsp".to_vec())),
            ("/w/leaf.rsp", RspFile::Bytes(b"-x".to_vec())),
        ]
        .into_iter()
        .collect();
        let mut asked = Vec::new();
        let mut hook = tree(&files, &mut asked);
        let w = Path::new("/w");
        let e = expand_with(&words(&["@self.rsp"]), w, &mut hook).unwrap_err();
        assert_eq!(e.to_string(), "recursive expansion of: '/w/./self.rsp'");
        let e = expand_with(&words(&["@a.rsp"]), w, &mut hook).unwrap_err();
        assert_eq!(e, RspError::Recursive { path: PathBuf::from("/w/a.rsp") });
        // The same file twice side by side is no recursion.
        let e = expand_with(&words(&["@twice.rsp"]), w, &mut hook).unwrap();
        assert_eq!(e.words, ["-x", "-x"]);
        assert_eq!(e.files.len(), 3);
    }
}
