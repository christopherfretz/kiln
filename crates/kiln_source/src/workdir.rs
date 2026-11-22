//! The working directory a translation unit's relative paths open
//! against (spec/driver.md "Working directory"; WP-2121): clang's
//! `FileSystemOptions::WorkingDir` (`-working-directory`), a value that is
//! passed along, never the process's current directory changed.
//!
//! Only the path used to open, read, list or write a file is joined: a
//! name as written (a buffer's [`file_name`](crate::SourceManager::file_name),
//! `__FILE__`, a diagnostic's path, a dependency file's entry, a `#line`
//! name) stays as written.

use std::borrow::Cow;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Where relative paths resolve: the process's current directory at the
/// time of each file-system call ([`WorkDir::process`], the default,
/// which joins nothing), or a fixed absolute directory ([`WorkDir::at`]).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct WorkDir(Option<Arc<Path>>);

impl WorkDir {
    /// The process's current directory, as the operating system resolves
    /// a relative path: nothing is joined.
    pub const fn process() -> WorkDir {
        WorkDir(None)
    }

    /// The directory `dir`, made absolute against the process's current
    /// directory when it is relative (not canonicalized: `..` and
    /// symbolic links are kept, as clang's `make_absolute`).
    ///
    /// # Errors
    /// `dir` is relative and the current directory cannot be read.
    pub fn at(dir: &Path) -> io::Result<WorkDir> {
        WorkDir::process().resolve(dir)
    }

    /// The process's current directory read once, now: later changes of
    /// it do not move this one. [`WorkDir::process`] when it cannot be
    /// read.
    pub fn of_process() -> WorkDir {
        std::env::current_dir().map_or_else(|_| WorkDir::process(), |d| WorkDir(Some(d.into())))
    }

    /// The directory `dir`, a relative one joined onto this one.
    ///
    /// # Errors
    /// As [`WorkDir::at`], for [`WorkDir::process`].
    pub fn resolve(&self, dir: &Path) -> io::Result<WorkDir> {
        Ok(WorkDir(Some(self.absolute(dir)?.into())))
    }

    /// The fixed directory; `None` for [`WorkDir::process`].
    pub fn dir(&self) -> Option<&Path> {
        self.0.as_deref()
    }

    /// The path to open for `path`: joined onto the fixed directory when
    /// relative, else `path` itself.
    pub fn join<'p>(&self, path: &'p Path) -> Cow<'p, Path> {
        match &self.0 {
            Some(d) if path.is_relative() && !path.as_os_str().is_empty() => {
                Cow::Owned(d.join(path))
            }
            _ => Cow::Borrowed(path),
        }
    }

    /// [`join`](Self::join) of a `str` path.
    pub fn join_str<'p>(&self, path: &'p str) -> Cow<'p, Path> {
        self.join(Path::new(path))
    }

    /// The directory itself: the fixed one, or the process's current one,
    /// read now.
    ///
    /// # Errors
    /// The current directory cannot be read.
    pub fn current(&self) -> io::Result<PathBuf> {
        match &self.0 {
            Some(d) => Ok(d.to_path_buf()),
            None => std::env::current_dir(),
        }
    }

    /// `path` made absolute ([`std::path::absolute`]) against this
    /// directory.
    ///
    /// # Errors
    /// As [`std::path::absolute`].
    pub fn absolute(&self, path: &Path) -> io::Result<PathBuf> {
        std::path::absolute(self.join(path))
    }

    /// `path` canonicalized ([`std::fs::canonicalize`]) against this
    /// directory.
    ///
    /// # Errors
    /// As [`std::fs::canonicalize`].
    pub fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        std::fs::canonicalize(self.join(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_joins_nothing_and_a_fixed_directory_joins_relative_paths() {
        let p = WorkDir::process();
        assert_eq!(p.join(Path::new("a/b.h")), Path::new("a/b.h"));
        assert!(p.dir().is_none());
        let w = WorkDir::at(Path::new("/w")).unwrap();
        assert_eq!(w.join(Path::new("a/b.h")), Path::new("/w/a/b.h"));
        assert_eq!(w.join(Path::new("/abs/c.h")), Path::new("/abs/c.h"));
        assert_eq!(w.join(Path::new("")), Path::new(""));
        assert_eq!(w.resolve(Path::new("sub")).unwrap().dir(), Some(Path::new("/w/sub")));
        assert_eq!(w.resolve(Path::new("/x")).unwrap().dir(), Some(Path::new("/x")));
        assert_eq!(w.absolute(Path::new("./m.pcm")).unwrap(), Path::new("/w/m.pcm"));
        assert_eq!(w.current().unwrap(), Path::new("/w"));
        // A relative directory is made absolute against the process's.
        let r = WorkDir::at(Path::new("rel")).unwrap();
        assert!(r.dir().is_some_and(Path::is_absolute));
        assert_eq!(WorkDir::of_process().current().ok(), std::env::current_dir().ok());
    }
}
