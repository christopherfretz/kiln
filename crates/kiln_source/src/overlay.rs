//! [`Overlay`]: replacement contents for files that exist, for one run
//! (spec/source.md "Overlay").

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fmt, fs, io};

use crate::manager::{FileKey, file_key};

/// Replacement contents for files that exist, keyed by file identity.
///
/// An entry is keyed by the identity of the file its path names when the
/// entry is added (device and inode on Unix, as
/// [`SourceManager::load_file`](crate::SourceManager::load_file)
/// identifies files; the canonical path elsewhere, best-effort), never by
/// the path's spelling: the same file reached through another spelling, a
/// symbolic link or a hard link is the overlaid file. A source manager
/// given the overlay ([`SourceManager::set_overlay`](crate::SourceManager::set_overlay))
/// reads an overlaid file's bytes from here instead of the disk. Stage 1:
/// no file is created or removed, no directory changes.
#[derive(Clone, Default)]
pub struct Overlay {
    entries: HashMap<FileKey, Entry>,
}

#[derive(Clone)]
struct Entry {
    /// The path the entry was added with.
    path: PathBuf,
    bytes: Arc<[u8]>,
}

impl Overlay {
    /// An empty overlay (reads nothing from memory).
    pub fn new() -> Overlay {
        Overlay::default()
    }

    /// Replaces the content of the file at `path` with `bytes`. `path` is
    /// resolved by the operating system (a relative one against the
    /// process's current directory; pass absolute paths) and must name an
    /// existing regular file, symbolic links followed; the entry is keyed
    /// by that file's identity, now. A second entry for the same file
    /// replaces the first.
    ///
    /// # Errors
    /// `path` does not exist or cannot be examined, or names something
    /// other than a regular file ([`io::ErrorKind::InvalidInput`]).
    pub fn replace(&mut self, path: &Path, bytes: Arc<[u8]>) -> io::Result<()> {
        let meta = fs::metadata(path)?;
        if !meta.is_file() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a regular file"));
        }
        let key = file_key(path, &meta)?;
        self.entries.insert(key, Entry { path: path.to_owned(), bytes });
        Ok(())
    }

    /// True if no file is overlaid.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The number of overlaid files.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// The replacement bytes of the file at `path`, whose metadata is
    /// `meta` (already fetched by the caller, symbolic links followed),
    /// if that file is overlaid. One map lookup on Unix.
    pub fn bytes_of(&self, path: &Path, meta: &fs::Metadata) -> Option<&Arc<[u8]>> {
        let key = file_key(path, meta).ok()?;
        self.get(&key)
    }

    /// True if the file at `path` (resolved by the operating system,
    /// symbolic links followed) is overlaid. A `stat`.
    pub fn contains(&self, path: &Path) -> bool {
        fs::metadata(path).is_ok_and(|m| self.bytes_of(path, &m).is_some())
    }

    /// The paths the entries were added with, in no particular order.
    pub fn paths(&self) -> impl Iterator<Item = &Path> + '_ {
        self.entries.values().map(|e| e.path.as_path())
    }

    pub(crate) fn get(&self, key: &FileKey) -> Option<&Arc<[u8]>> {
        self.entries.get(key).map(|e| &e.bytes)
    }
}

impl fmt::Debug for Overlay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut paths: Vec<_> = self.entries.values().map(|e| (&e.path, e.bytes.len())).collect();
        paths.sort();
        f.debug_struct("Overlay").field("files", &paths).finish()
    }
}
