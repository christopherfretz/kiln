//! The header search list: clang's `InitHeaderSearch` (`AddPath`,
//! `Realize`, `RemoveDuplicates`) and the file-system seam
//! ([`HeaderFs`]) the resolver looks files up through.
//!
//! The driver turns its command line into an ordered list of
//! [`SearchDirSpec`]s (user `-iquote` / `-I` / `-isystem` / `-idirafter` in
//! command-line order, then the toolchain's internal directories, exactly
//! as clang's driver passes them to `cc1`); [`SearchList::build`] then
//! drops nonexistent and duplicate directories and orders the groups the
//! way clang does. See `spec/pp.md` "Header search".

use std::collections::{HashMap, HashSet};
use std::path::Path;

use kiln_source::{FileId, SourceManager};

/// The group a search directory was given in (clang
/// `frontend::IncludeDirGroup`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DirGroup {
    /// `-iquote`: searched for `"..."` includes only.
    Quoted,
    /// `-I`: user directories.
    Angled,
    /// `-isystem`, `-internal-isystem`: system directories.
    System,
    /// `-internal-externc-isystem`: system directories whose headers are
    /// implicitly `extern "C"` (line-marker flag 4).
    ExternCSystem,
    /// `-c-isystem`: system directory for C only.
    CSystem,
    /// `-cxx-isystem`: system directory for C++ only.
    CxxSystem,
    /// `-idirafter`: system directories searched last.
    After,
}

/// How a directory (and the headers found in it) is classified (clang
/// `SrcMgr::CharacteristicKind`), ordered `User < System < ExternCSystem`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum Characteristic {
    /// A user header.
    #[default]
    User,
    /// A system header (warnings suppressed; line-marker flag 3).
    System,
    /// An `extern "C"` system header (flags 3 4).
    ExternCSystem,
}

impl Characteristic {
    /// System or extern-C system.
    pub fn is_system(self) -> bool {
        self != Characteristic::User
    }

    /// The characteristic of a directory in `group`.
    pub fn of_group(group: DirGroup) -> Characteristic {
        match group {
            DirGroup::Quoted | DirGroup::Angled => Characteristic::User,
            DirGroup::ExternCSystem => Characteristic::ExternCSystem,
            DirGroup::System | DirGroup::CSystem | DirGroup::CxxSystem | DirGroup::After => {
                Characteristic::System
            }
        }
    }
}

/// One requested search directory.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SearchDirSpec {
    /// The path as given (it is used verbatim to build header names).
    pub path: String,
    /// Its group.
    pub group: DirGroup,
}

impl SearchDirSpec {
    /// A spec.
    pub fn new(path: impl Into<String>, group: DirGroup) -> SearchDirSpec {
        SearchDirSpec { path: path.into(), group }
    }
}

/// One directory of the realized search list.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SearchDir {
    /// The path as given.
    pub path: String,
    /// The classification of headers found here.
    pub characteristic: Characteristic,
    /// The directory's identity (two specs naming the same directory are
    /// duplicates).
    identity: String,
}

/// The realized search list: `"..."` includes search all of it, `<...>`
/// includes start at [`angled_start`](Self::angled_start).
#[derive(Clone, Debug, Default)]
pub struct SearchList {
    dirs: Vec<SearchDir>,
    angled_start: usize,
    messages: Vec<String>,
}

impl SearchList {
    /// Builds the list from `specs` (in clang `cc1` order), for C++ when
    /// `cplusplus` (else C), checking existence through `fs` (clang
    /// `InitHeaderSearch::AddPath` + `Realize`).
    pub fn build(specs: &[SearchDirSpec], cplusplus: bool, fs: &mut dyn HeaderFs) -> SearchList {
        let mut messages = Vec::new();
        let mut existing: Vec<(DirGroup, SearchDir)> = Vec::new();
        for s in specs {
            match fs.dir_identity(&s.path) {
                Some(identity) => existing.push((
                    s.group,
                    SearchDir {
                        path: s.path.clone(),
                        characteristic: Characteristic::of_group(s.group),
                        identity,
                    },
                )),
                None => messages.push(format!("ignoring nonexistent directory \"{}\"", s.path)),
            }
        }
        let pick = |g: &[DirGroup]| -> Vec<SearchDir> {
            existing.iter().filter(|(eg, _)| g.contains(eg)).map(|(_, d)| d.clone()).collect()
        };
        let mut list = pick(&[DirGroup::Quoted]);
        remove_duplicates(&mut list, 0, &mut messages);
        let num_quoted = list.len();
        list.extend(pick(&[DirGroup::Angled]));
        remove_duplicates(&mut list, num_quoted, &mut messages);
        let lang_system = if cplusplus { DirGroup::CxxSystem } else { DirGroup::CSystem };
        // clang adds System / ExternCSystem / the language's system group in
        // their order of appearance, then After.
        list.extend(pick(&[DirGroup::System, DirGroup::ExternCSystem, lang_system]));
        list.extend(pick(&[DirGroup::After]));
        remove_duplicates(&mut list, num_quoted, &mut messages);
        SearchList { dirs: list, angled_start: num_quoted, messages }
    }

    /// The directories, in search order.
    pub fn dirs(&self) -> &[SearchDir] {
        &self.dirs
    }

    /// Index of the first directory searched for `<...>` includes (the
    /// number of `-iquote` directories).
    pub fn angled_start(&self) -> usize {
        self.angled_start
    }

    /// The `ignoring ...` notes clang prints under `-v`, in order.
    pub fn messages(&self) -> &[String] {
        &self.messages
    }

    /// clang's `-v` search-list report: the `ignoring ...` notes, then
    /// `#include "..." search starts here:` ... `End of search list.`
    pub fn verbose_text(&self) -> String {
        let mut s = String::new();
        for m in &self.messages {
            s.push_str(m);
            s.push('\n');
        }
        s.push_str("#include \"...\" search starts here:\n");
        for (i, d) in self.dirs.iter().enumerate() {
            if i == self.angled_start {
                s.push_str("#include <...> search starts here:\n");
            }
            s.push(' ');
            s.push_str(&d.path);
            s.push('\n');
        }
        s.push_str("End of search list.\n");
        s
    }
}

/// clang `RemoveDuplicates` over `list[first..]`: a later duplicate is
/// dropped, except that a user directory duplicated by a later system
/// directory is dropped in favor of the system one. Returns the number of
/// user directories removed that way.
fn remove_duplicates(list: &mut Vec<SearchDir>, first: usize, messages: &mut Vec<String>) -> usize {
    let mut seen: HashSet<String> = HashSet::new();
    let mut non_system_removed = 0;
    let mut i = first;
    while i < list.len() {
        if seen.insert(list[i].identity.clone()) {
            i += 1;
            continue;
        }
        let mut to_remove = i;
        if list[i].characteristic != Characteristic::User {
            let first_dup = (first..i).find(|&j| list[j].identity == list[i].identity);
            if let Some(j) = first_dup
                && list[j].characteristic == Characteristic::User
            {
                to_remove = j;
            }
        }
        messages.push(format!("ignoring duplicate directory \"{}\"", list[i].path));
        if to_remove != i {
            messages.push(
                "  as it is a non-system directory that duplicates a system directory".to_owned(),
            );
            non_system_removed += 1;
        }
        list.remove(to_remove);
    }
    non_system_removed
}

/// Joins a header name onto a directory the way `llvm::sys::path::append`
/// does (no separator doubled, nothing normalized).
pub fn join_path(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        return name.to_owned();
    }
    let mut s = String::with_capacity(dir.len() + 1 + name.len());
    s.push_str(dir);
    if !dir.ends_with('/') && !name.starts_with('/') {
        s.push('/');
    }
    s.push_str(name);
    s
}

/// The directory part of a file name as clang's `FileManager` names it:
/// `llvm::sys::path::parent_path`, or `.` when there is none.
pub fn parent_dir(name: &str) -> &str {
    let trimmed = name.trim_end_matches(|c| c != '/');
    if trimmed.is_empty() {
        return ".";
    }
    let p = trimmed.trim_end_matches('/');
    if p.is_empty() { "/" } else { p }
}

/// The file system as header search sees it. [`RealFs`] is the disk;
/// [`MemoryFs`] is a virtual tree for tests.
pub trait HeaderFs {
    /// The identity of the directory at `path` (equal for two paths naming
    /// the same directory), or `None` if it does not exist.
    fn dir_identity(&mut self, path: &str) -> Option<String>;

    /// Loads the regular file at `path` into `sm` (the buffer is named
    /// `path`; the same file reached by another path is the same buffer),
    /// or `None` if there is no such file.
    fn open(&mut self, sm: &SourceManager, path: &str) -> Option<FileId>;

    /// The bytes of the regular file at `path` (`#embed`).
    fn read(&mut self, path: &str) -> Option<Vec<u8>>;
}

/// The real file system. Relative paths are relative to the process's
/// working directory.
#[derive(Clone, Debug, Default)]
pub struct RealFs;

impl HeaderFs for RealFs {
    fn dir_identity(&mut self, path: &str) -> Option<String> {
        let p = Path::new(path);
        if !p.is_dir() {
            return None;
        }
        std::fs::canonicalize(p).ok().map(|c| c.to_string_lossy().into_owned())
    }

    fn open(&mut self, sm: &SourceManager, path: &str) -> Option<FileId> {
        let p = Path::new(path);
        if !std::fs::metadata(p).is_ok_and(|m| m.is_file()) {
            return None;
        }
        sm.load_file(p).ok()
    }

    fn read(&mut self, path: &str) -> Option<Vec<u8>> {
        let p = Path::new(path);
        if !std::fs::metadata(p).is_ok_and(|m| m.is_file()) {
            return None;
        }
        std::fs::read(p).ok()
    }
}

/// An in-memory file tree for tests. Paths are normalized lexically
/// (`.` and `dir/..` collapse); relative paths are relative to an
/// implicit working directory. Every ancestor of a file is a directory.
#[derive(Clone, Debug, Default)]
pub struct MemoryFs {
    files: HashMap<String, Vec<u8>>,
    dirs: HashSet<String>,
    loaded: HashMap<String, FileId>,
}

impl MemoryFs {
    /// An empty tree.
    pub fn new() -> MemoryFs {
        MemoryFs::default()
    }

    /// Adds a file (and its ancestor directories).
    #[must_use]
    pub fn with_file(mut self, path: &str, text: &str) -> MemoryFs {
        self.add_file(path, text.as_bytes());
        self
    }

    /// Adds an empty directory (and its ancestors).
    #[must_use]
    pub fn with_dir(mut self, path: &str) -> MemoryFs {
        let mut d = normalize(path);
        loop {
            self.dirs.insert(d.clone());
            match d.rfind('/') {
                Some(0) if d.len() > 1 => d.truncate(1),
                Some(i) if i > 0 => d.truncate(i),
                _ => break,
            }
        }
        self.dirs.insert(".".to_owned());
        self
    }

    /// Adds a file (and its ancestor directories).
    pub fn add_file(&mut self, path: &str, bytes: &[u8]) {
        let n = normalize(path);
        let parent = parent_dir(&n).to_owned();
        *self = std::mem::take(self).with_dir(&parent);
        self.files.insert(n, bytes.to_vec());
    }
}

/// Lexical normalization: drops `.` components and collapses `x/..`.
fn normalize(path: &str) -> String {
    let abs = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for c in path.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|p| *p != "..") {
                    parts.pop();
                } else if !abs {
                    parts.push("..");
                }
            }
            c => parts.push(c),
        }
    }
    let body = parts.join("/");
    if abs {
        format!("/{body}")
    } else if body.is_empty() {
        ".".to_owned()
    } else {
        body
    }
}

impl HeaderFs for MemoryFs {
    fn dir_identity(&mut self, path: &str) -> Option<String> {
        let n = normalize(path);
        self.dirs.contains(&n).then_some(n)
    }

    fn open(&mut self, sm: &SourceManager, path: &str) -> Option<FileId> {
        let n = normalize(path);
        if let Some(&f) = self.loaded.get(&n) {
            return Some(f);
        }
        let bytes = self.files.get(&n)?;
        let text = String::from_utf8_lossy(bytes).into_owned();
        let f = sm.add_virtual(path, text).ok()?;
        self.loaded.insert(n, f);
        Some(f)
    }

    fn read(&mut self, path: &str) -> Option<Vec<u8>> {
        self.files.get(&normalize(path)).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_and_parent() {
        assert_eq!(join_path("a", "b.h"), "a/b.h");
        assert_eq!(join_path("a/", "b.h"), "a/b.h");
        assert_eq!(join_path("/", "b.h"), "/b.h");
        assert_eq!(parent_dir("a.cpp"), ".");
        assert_eq!(parent_dir("sub/a.h"), "sub");
        assert_eq!(parent_dir("/a.h"), "/");
        assert_eq!(parent_dir("../x/y/a.h"), "../x/y");
        assert_eq!(parent_dir("a//b.h"), "a");
    }

    #[test]
    fn normalization() {
        assert_eq!(normalize("./a/../b/./c.h"), "b/c.h");
        assert_eq!(normalize("/usr/lib/../include"), "/usr/include");
        assert_eq!(normalize("../a"), "../a");
        assert_eq!(normalize("."), ".");
    }
}
