//! [`Overlay`]: the files of one run that differ from the disk:
//! replaced, created or removed (spec/source.md "Overlay").

use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::{fmt, fs, io};

use crate::manager::{FileKey, file_key};

/// The files of a run that differ from the disk, as final states: a file
/// that exists with other contents (replaced), a file that does not exist
/// on disk (created) and a file that exists on disk and not in the run
/// (removed).
///
/// The reference is the same command over a copy of the tree in which
/// each replaced file holds the overlay's bytes, each created file exists
/// with them (its missing parent directories made) and each removed file
/// is unlinked, its directory left in place.
///
/// - A replaced or removed file is keyed by the identity of the file its
///   path names when the entry is added (device and inode on Unix, as
///   [`SourceManager::load_file`](crate::SourceManager::load_file)
///   identifies files; the canonical path elsewhere, best-effort), never
///   by the path's spelling: the same file reached through another
///   spelling, a symbolic link or a hard link is the entry's file.
/// - A created file is keyed by the identity of its deepest ancestor
///   directory that exists on disk (the anchor) and the path below it:
///   two spellings of one created path are one file. The directories
///   below the anchor are the overlay's (implied by its files: there are
///   no empty created directories).
///
/// A source manager given the overlay
/// ([`SourceManager::set_overlay`](crate::SourceManager::set_overlay))
/// loads through it; header search sees it through `kiln_pp::OverlayFs`.
/// Every path is resolved when the entry is added, so the tree must not
/// change under the overlay's paths until the run ends.
#[derive(Clone, Default)]
pub struct Overlay {
    entries: HashMap<FileKey, Entry>,
    /// Per directory (an existing one by its identity, a created one by
    /// its created key), the names the overlay creates in it, each with
    /// the number of created files at or below it.
    children: HashMap<FileKey, HashMap<OsString, u32>>,
    /// Every component of every created path (it never shrinks): a path
    /// missing on disk whose last component is not here is missing in
    /// the overlay too, with no system call.
    names: HashSet<OsString>,
    /// The number of created files.
    created: usize,
}

#[derive(Clone)]
struct Entry {
    /// The path the entry was added with.
    path: PathBuf,
    state: State,
}

#[derive(Clone)]
enum State {
    Replaced(Content),
    Removed,
    Created(Content),
}

/// What the overlay holds for a path ([`Overlay::state`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntryState {
    /// A file on disk whose content the overlay replaces.
    Replaced,
    /// A file the overlay creates.
    Created,
    /// A file on disk the overlay removes.
    Removed,
}

/// A replaced or created file's content produced on first read
/// ([`Overlay::replace_with`], [`Overlay::create_with`]). It is called at
/// most once per overlay, by the first run that reads the entry; a run
/// reading it meanwhile waits for it. Its result, bytes or error, is kept
/// for the overlay's life. It must not start a run that reads the same
/// entry.
pub type Loader = Box<dyn Fn() -> io::Result<Arc<[u8]>> + Send + Sync>;

/// The content of a replaced or created file: bytes, or a [`Loader`]'s.
#[derive(Clone)]
pub struct Content(Inner);

#[derive(Clone)]
enum Inner {
    Bytes(Arc<[u8]>),
    Lazy(Arc<Lazy>),
}

struct Lazy {
    /// The path the entry was added with (for [`LoadFailures`]).
    path: PathBuf,
    loader: Loader,
    cell: OnceLock<Result<Arc<[u8]>, String>>,
}

impl Content {
    fn of(bytes: Arc<[u8]>) -> Content {
        Content(Inner::Bytes(bytes))
    }

    fn lazy(path: &Path, loader: Loader) -> Content {
        let lazy = Lazy { path: path.to_owned(), loader, cell: OnceLock::new() };
        Content(Inner::Lazy(Arc::new(lazy)))
    }

    /// The bytes, the loader's called if this is the first read of a lazy
    /// content.
    ///
    /// # Errors
    /// The loader failed (now or on an earlier read): its error's text.
    pub fn bytes(&self) -> Result<Arc<[u8]>, String> {
        match &self.0 {
            Inner::Bytes(b) => Ok(Arc::clone(b)),
            Inner::Lazy(l) => {
                l.cell.get_or_init(|| (l.loader)().map_err(|e| e.to_string())).clone()
            }
        }
    }

    /// [`bytes`](Self::bytes), a failure noted in `failures`.
    pub fn bytes_noting(&self, failures: &LoadFailures) -> Option<Arc<[u8]>> {
        match self.bytes() {
            Ok(b) => Some(b),
            Err(why) => {
                if let Inner::Lazy(l) = &self.0 {
                    failures.note(&l.path, &why);
                }
                None
            }
        }
    }

    /// The loader's error, if it was called and failed.
    fn failure(&self) -> Option<(&Path, &str)> {
        match &self.0 {
            Inner::Lazy(l) => match l.cell.get() {
                Some(Err(why)) => Some((&l.path, why)),
                _ => None,
            },
            Inner::Bytes(_) => None,
        }
    }

    fn describe(&self) -> String {
        match &self.0 {
            Inner::Bytes(b) => format!("{} bytes", b.len()),
            Inner::Lazy(l) => match l.cell.get() {
                None => "lazy, not loaded".to_owned(),
                Some(Ok(b)) => format!("lazy, {} bytes", b.len()),
                Some(Err(_)) => "lazy, failed".to_owned(),
            },
        }
    }
}

impl fmt::Debug for Content {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Content").field(&self.describe()).finish()
    }
}

/// The entries whose [`Loader`] failed that one run asked for, in the
/// order asked, each once: a handle, its clones sharing one list (the
/// source manager's, `SourceManager::overlay_failures`).
#[derive(Clone, Debug, Default)]
pub struct LoadFailures(Arc<Mutex<Vec<(PathBuf, String)>>>);

impl LoadFailures {
    /// Notes that the entry added as `path` failed to load with `why`.
    pub fn note(&self, path: &Path, why: &str) {
        let mut v = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if !v.iter().any(|(p, _)| p == path) {
            v.push((path.to_owned(), why.to_owned()));
        }
    }

    /// The failures noted so far.
    pub fn list(&self) -> Vec<(PathBuf, String)> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

/// The identity of a file or directory the overlay creates (an opaque
/// key: equal for every spelling of the same created path).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CreatedKey(pub(crate) FileKey);

/// What a path names under an overlay ([`Overlay::resolve`]).
pub enum Resolved<'o> {
    /// It exists on disk (any kind: a directory, a regular file, a device)
    /// and the overlay does not remove it: its metadata (symbolic links
    /// followed) and, for a replaced file, the overlay's content.
    Disk {
        /// The metadata of what the path names.
        meta: fs::Metadata,
        /// The replacement content of a replaced file.
        replaced: Option<&'o Content>,
        /// The path it was found at when that is not the path asked: a
        /// `..` out of a directory the overlay creates, followed
        /// lexically (open this one).
        redirected: Option<PathBuf>,
    },
    /// A file the overlay creates.
    Created {
        /// Its identity.
        key: CreatedKey,
        /// Its content.
        content: &'o Content,
    },
    /// A directory the overlay creates (an ancestor of a created file).
    CreatedDir(CreatedDir<'o>),
    /// Nothing: not on disk and not created, or a removed file.
    Missing,
}

impl fmt::Debug for Resolved<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Resolved::Disk { meta, replaced, redirected } => f
                .debug_struct("Disk")
                .field("dir", &meta.is_dir())
                .field("replaced", &replaced.map(Content::describe))
                .field("redirected", redirected)
                .finish(),
            Resolved::Created { key, content } => {
                f.debug_struct("Created").field("key", key).field("content", content).finish()
            }
            Resolved::CreatedDir(d) => f.debug_tuple("CreatedDir").field(&d.key).finish(),
            Resolved::Missing => f.write_str("Missing"),
        }
    }
}

/// A directory the overlay creates ([`Resolved::CreatedDir`]).
#[derive(Clone, Copy)]
pub struct CreatedDir<'o> {
    key: &'o FileKey,
    overlay: &'o Overlay,
}

impl fmt::Debug for CreatedDir<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CreatedDir").field(self.key).finish()
    }
}

impl<'o> CreatedDir<'o> {
    /// Its identity as header search compares directories: equal for two
    /// spellings of it, distinct from every directory on disk's.
    pub fn identity(&self) -> String {
        self.key.identity()
    }

    /// The names of its entries (each a created file or directory).
    pub fn names(&self) -> impl Iterator<Item = &'o OsStr> + 'o {
        self.overlay.names_in(self.key)
    }
}

/// What [`Overlay::resolve`] learned about directories, for one run: each
/// directory path asked about is `stat`ed once (the tree does not change
/// during a run). Create one per run; never share it between overlays.
#[derive(Debug, Default)]
pub struct DirCache {
    stats: HashMap<PathBuf, Stat>,
}

#[derive(Clone, Debug)]
enum Stat {
    Dir(FileKey),
    Other,
    Absent,
}

impl DirCache {
    fn stat(&mut self, p: &Path) -> Stat {
        let p = if p.as_os_str().is_empty() { Path::new(".") } else { p };
        if let Some(s) = self.stats.get(p) {
            return s.clone();
        }
        let s = match fs::metadata(p) {
            Ok(m) if m.is_dir() => file_key(p, &m).map_or(Stat::Other, Stat::Dir),
            Ok(_) => Stat::Other,
            Err(_) => Stat::Absent,
        };
        self.stats.insert(p.to_owned(), s.clone());
        s
    }
}

/// How many `..` steps out of created directories one resolution follows
/// (each one starts the resolution again).
const MAX_RESTARTS: u32 = 64;

fn invalid(why: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, why.to_owned())
}

/// The error of a path that names nothing (the operating system's).
pub(crate) fn not_found() -> io::Error {
    io::Error::from(io::ErrorKind::NotFound)
}

impl Overlay {
    /// An empty overlay (changes nothing).
    pub fn new() -> Overlay {
        Overlay::default()
    }

    /// Replaces the content of the file at `path` with `bytes`. `path` is
    /// resolved by the operating system (a relative one against the
    /// process's current directory; pass absolute paths) and must name an
    /// existing regular file, symbolic links followed, or a file this
    /// overlay creates; the entry is keyed by that file's identity, now.
    /// A second entry for the same file replaces the first; a file the
    /// overlay removes becomes replaced.
    ///
    /// # Errors
    /// `path` does not exist or cannot be examined, or names something
    /// other than a regular file ([`io::ErrorKind::InvalidInput`]).
    pub fn replace(&mut self, path: &Path, bytes: Arc<[u8]>) -> io::Result<()> {
        self.replace_content(path, Content::of(bytes))
    }

    /// [`replace`](Self::replace) with content `loader` produces when a
    /// run first reads the file (never for its existence: `__has_include`,
    /// listings). The path's checks and the file's identity are taken now.
    ///
    /// # Errors
    /// As [`replace`](Self::replace).
    pub fn replace_with(&mut self, path: &Path, loader: Loader) -> io::Result<()> {
        self.replace_content(path, Content::lazy(path, loader))
    }

    fn replace_content(&mut self, path: &Path, content: Content) -> io::Result<()> {
        let meta = match fs::metadata(path) {
            Ok(m) => m,
            Err(e) => {
                let key = match self.resolve(path, &mut DirCache::default()) {
                    Resolved::Created { key, .. } => key.0,
                    _ => return Err(e),
                };
                if let Some(entry) = self.entries.get_mut(&key) {
                    entry.state = State::Created(content);
                }
                return Ok(());
            }
        };
        if !meta.is_file() {
            return Err(invalid("not a regular file"));
        }
        let key = file_key(path, &meta)?;
        let state = State::Replaced(content);
        self.entries.insert(key, Entry { path: path.to_owned(), state });
        Ok(())
    }

    /// Creates the file at `path` with `bytes`: a path that names nothing
    /// on disk (its missing parent directories are the overlay's), or the
    /// path of a file this overlay removes (which becomes replaced).
    /// `path` is resolved as for [`replace`](Self::replace).
    ///
    /// # Errors
    /// [`io::ErrorKind::AlreadyExists`]: a regular file is there (on disk
    /// and not removed, or created by this overlay).
    /// [`io::ErrorKind::InvalidInput`]: something other than a regular
    /// file is there (a directory, a dangling symbolic link, a device);
    /// an ancestor exists and is not a directory, or is a file this
    /// overlay creates; a `.` or `..` component below the deepest
    /// existing ancestor; the path is an ancestor of a file this overlay
    /// creates. Otherwise the error of examining the path.
    pub fn create(&mut self, path: &Path, bytes: Arc<[u8]>) -> io::Result<()> {
        self.create_content(path, Content::of(bytes))
    }

    /// [`create`](Self::create) with content `loader` produces when a run
    /// first reads the file (never for its existence, as
    /// [`replace_with`](Self::replace_with)).
    ///
    /// # Errors
    /// As [`create`](Self::create).
    pub fn create_with(&mut self, path: &Path, loader: Loader) -> io::Result<()> {
        self.create_content(path, Content::lazy(path, loader))
    }

    fn create_content(&mut self, path: &Path, content: Content) -> io::Result<()> {
        match fs::metadata(path) {
            Ok(meta) => {
                if !meta.is_file() {
                    return Err(invalid("not a regular file"));
                }
                let key = file_key(path, &meta)?;
                return match self.entries.get_mut(&key) {
                    Some(e) if matches!(e.state, State::Removed) => {
                        e.state = State::Replaced(content);
                        path.clone_into(&mut e.path);
                        Ok(())
                    }
                    _ => Err(io::Error::from(io::ErrorKind::AlreadyExists)),
                };
            }
            Err(e) if e.kind() == io::ErrorKind::NotADirectory => {
                return Err(invalid("an ancestor is not a directory"));
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        if fs::symlink_metadata(path).is_ok() {
            return Err(invalid("a dangling symbolic link"));
        }
        let (anchor, rel) = anchor_of(path)?;
        let key = FileKey::created(&anchor, &rel);
        if self.entries.contains_key(&key) {
            return Err(io::Error::from(io::ErrorKind::AlreadyExists));
        }
        if self.children.contains_key(&key) {
            return Err(invalid("a directory the overlay creates"));
        }
        for k in 1..rel.len() {
            if self.entries.contains_key(&FileKey::created(&anchor, &rel[..k])) {
                return Err(invalid("an ancestor is a file the overlay creates"));
            }
        }
        for k in 0..rel.len() {
            let dir = if k == 0 { anchor.clone() } else { FileKey::created(&anchor, &rel[..k]) };
            *self.children.entry(dir).or_default().entry(rel[k].clone()).or_default() += 1;
        }
        self.names.extend(rel.iter().cloned());
        self.created += 1;
        let state = State::Created(content);
        self.entries.insert(key, Entry { path: path.to_owned(), state });
        Ok(())
    }

    /// [`replace`](Self::replace) when `path` names a regular file (on
    /// disk and not removed, or created), else [`create`](Self::create).
    ///
    /// # Errors
    /// As [`create`](Self::create), but never `AlreadyExists`.
    pub fn write(&mut self, path: &Path, bytes: Arc<[u8]>) -> io::Result<()> {
        match self.create(path, Arc::clone(&bytes)) {
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => self.replace(path, bytes),
            r => r,
        }
    }

    /// Removes the file at `path`: a regular file on disk (keyed by its
    /// identity, as [`replace`](Self::replace)) or a file this overlay
    /// creates (whose entry is dropped). Removing a replaced file makes
    /// it removed.
    ///
    /// # Errors
    /// [`io::ErrorKind::NotFound`]: no regular file is there (none on
    /// disk, or one this overlay already removes, and none created).
    /// [`io::ErrorKind::InvalidInput`]: the path's last component is a
    /// symbolic link, or the file has other hard links (unlinking the
    /// name would leave the file reachable, which an entry keyed by
    /// identity cannot express), or it is not a regular file. Otherwise
    /// the error of examining the path.
    pub fn remove(&mut self, path: &Path) -> io::Result<()> {
        match fs::symlink_metadata(path) {
            Ok(m) if m.file_type().is_symlink() => Err(invalid("a symbolic link")),
            Ok(m) if !m.is_file() => Err(invalid("not a regular file")),
            Ok(m) => {
                if hard_links(&m) > 1 {
                    return Err(invalid("a file with other hard links"));
                }
                let key = file_key(path, &m)?;
                if self.entries.get(&key).is_some_and(|e| matches!(e.state, State::Removed)) {
                    return Err(not_found());
                }
                self.entries.insert(key, Entry { path: path.to_owned(), state: State::Removed });
                Ok(())
            }
            Err(e)
                if matches!(e.kind(), io::ErrorKind::NotFound | io::ErrorKind::NotADirectory) =>
            {
                let key = match self.resolve(path, &mut DirCache::default()) {
                    Resolved::Created { key, .. } => key.0,
                    _ => return Err(e),
                };
                self.drop_created(&key);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Drops the created file `key` and its directories' counts.
    fn drop_created(&mut self, key: &FileKey) {
        let FileKey::Created(anchor, rel) = key else { return };
        let rel: Vec<OsString> = rel.iter().map(OsStr::to_owned).collect();
        for k in 0..rel.len() {
            let dir = if k == 0 { (**anchor).clone() } else { FileKey::created(anchor, &rel[..k]) };
            if let Some(names) = self.children.get_mut(&dir) {
                if let Some(n) = names.get_mut(&rel[k]) {
                    *n -= 1;
                    if *n == 0 {
                        names.remove(&rel[k]);
                    }
                }
                if names.is_empty() {
                    self.children.remove(&dir);
                }
            }
        }
        self.entries.remove(key);
        self.created -= 1;
    }

    /// True if the overlay changes nothing.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The number of entries (replaced, created and removed files).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if the overlay creates a file.
    pub fn creates_files(&self) -> bool {
        self.created > 0
    }

    /// The replacement content of the file at `path`, whose metadata is
    /// `meta` (already fetched by the caller, symbolic links followed),
    /// if the overlay replaces that file. One map lookup on Unix.
    ///
    /// The inner `Err` is a failed [`Loader`]'s error text.
    pub fn bytes_of(&self, path: &Path, meta: &fs::Metadata) -> Option<Result<Arc<[u8]>, String>> {
        let key = file_key(path, meta).ok()?;
        match self.entries.get(&key).map(|e| &e.state) {
            Some(State::Replaced(c)) => Some(c.bytes()),
            _ => None,
        }
    }

    /// The entries whose [`Loader`] has failed (in any run), sorted by
    /// path, with the error text. A failure is kept: every later read
    /// fails the same way.
    pub fn load_failures(&self) -> Vec<(PathBuf, String)> {
        let mut v: Vec<(PathBuf, String)> = self
            .entries
            .values()
            .filter_map(|e| match &e.state {
                State::Replaced(c) | State::Created(c) => c.failure(),
                State::Removed => None,
            })
            .map(|(p, why)| (p.to_owned(), why.to_owned()))
            .collect();
        v.sort();
        v
    }

    /// What the overlay holds for the file at `path` (resolved as the
    /// operating system resolves it, then as [`resolve`](Self::resolve)
    /// does), if anything.
    pub fn state(&self, path: &Path) -> Option<EntryState> {
        match fs::metadata(path) {
            Ok(m) => match self.get(&file_key(path, &m).ok()?)? {
                State::Replaced(_) => Some(EntryState::Replaced),
                State::Removed => Some(EntryState::Removed),
                State::Created(_) => None,
            },
            Err(_) => match self.absent(path, &mut DirCache::default(), 0) {
                Resolved::Created { .. } => Some(EntryState::Created),
                _ => None,
            },
        }
    }

    /// True if the overlay replaces, creates or removes the file at
    /// `path` ([`state`](Self::state)).
    pub fn contains(&self, path: &Path) -> bool {
        self.state(path).is_some()
    }

    /// The paths the entries were added with, in no particular order.
    pub fn paths(&self) -> impl Iterator<Item = &Path> + '_ {
        self.entries.values().map(|e| e.path.as_path())
    }

    /// The paths of the created files, as added, sorted.
    pub fn created_paths(&self) -> Vec<&Path> {
        let mut v: Vec<&Path> = self
            .entries
            .values()
            .filter(|e| matches!(e.state, State::Created(_)))
            .map(|e| e.path.as_path())
            .collect();
        v.sort();
        v
    }

    /// What `path` names under the overlay: `stat` it (symbolic links
    /// followed); a file on disk is the disk's unless the overlay removes
    /// it ([`Resolved::Missing`]); a path the disk does not have is
    /// resolved by [`absent`](Self::absent).
    pub fn resolve(&self, path: &Path, cache: &mut DirCache) -> Resolved<'_> {
        self.resolve_from(path, cache, 0)
    }

    fn resolve_from(&self, path: &Path, cache: &mut DirCache, restarts: u32) -> Resolved<'_> {
        match fs::metadata(path) {
            Ok(meta) => match self.on_disk(path, meta) {
                Resolved::Disk { meta, replaced, redirected: None } if restarts > 0 => {
                    Resolved::Disk { meta, replaced, redirected: Some(path.to_owned()) }
                }
                r => r,
            },
            Err(_) => self.absent(path, cache, restarts),
        }
    }

    /// [`resolve`](Self::resolve) of `path`, which the operating system
    /// found with metadata `meta`: the disk's, or missing when the
    /// overlay removes it. No system call on Unix.
    pub fn on_disk(&self, path: &Path, meta: fs::Metadata) -> Resolved<'_> {
        let plain = |meta| Resolved::Disk { meta, replaced: None, redirected: None };
        if self.created == self.entries.len() || !meta.is_file() {
            return plain(meta);
        }
        let Ok(key) = file_key(path, &meta) else { return plain(meta) };
        match self.get(&key) {
            Some(State::Removed) => Resolved::Missing,
            Some(State::Replaced(c)) => {
                Resolved::Disk { meta, replaced: Some(c), redirected: None }
            }
            _ => plain(meta),
        }
    }

    /// [`resolve`](Self::resolve) of `path`, which the operating system
    /// did not find: a file or directory the overlay creates, or nothing.
    /// No system call unless the overlay creates a file whose path has
    /// `path`'s last component (or `path` has a `..`); then one `stat`
    /// per missing ancestor up to the deepest existing one (each
    /// directory once per `cache`). A `..` after a created directory
    /// leaves it lexically (a created directory holds no symbolic link),
    /// and the result is resolved again from the start.
    pub fn absent_path(&self, path: &Path, cache: &mut DirCache) -> Resolved<'_> {
        self.absent(path, cache, 0)
    }

    fn absent(&self, path: &Path, cache: &mut DirCache, restarts: u32) -> Resolved<'_> {
        if self.created == 0 || restarts > MAX_RESTARTS {
            return Resolved::Missing;
        }
        let comps: Vec<Component<'_>> = path.components().collect();
        let dotdot = comps.contains(&Component::ParentDir);
        let named =
            |c: &Component<'_>| matches!(c, Component::Normal(n) if self.names.contains(*n));
        // The deepest existing ancestor: `comps[..i]`, `comps[..=i]`
        // missing.
        let mut i = comps.len();
        let anchor = loop {
            if i == 0 {
                return Resolved::Missing;
            }
            i -= 1;
            // Below the anchor, every component of a created path is a
            // created name (unless a later `..` cancels it).
            if !dotdot && !named(&comps[i]) {
                return Resolved::Missing;
            }
            let prefix: PathBuf = comps[..i].iter().collect();
            match cache.stat(&prefix) {
                Stat::Dir(k) => break k,
                Stat::Other => return Resolved::Missing,
                Stat::Absent => {}
            }
        };
        let mut below: Vec<&OsStr> = Vec::new();
        let mut collapsed = false;
        for (j, c) in comps[i..].iter().enumerate() {
            match c {
                Component::Normal(n) => below.push(n),
                Component::CurDir => {}
                Component::ParentDir => {
                    if below.is_empty() {
                        // Out of the anchor: the disk's again.
                        let mut p: PathBuf = comps[..i].iter().collect();
                        p.push("..");
                        p.extend(&comps[i + j + 1..]);
                        return self.resolve_from(&p, cache, restarts + 1);
                    }
                    if !self.children.contains_key(&FileKey::created(&anchor, &below)) {
                        return Resolved::Missing;
                    }
                    below.pop();
                    collapsed = true;
                }
                Component::RootDir | Component::Prefix(_) => return Resolved::Missing,
            }
        }
        if collapsed {
            let mut p: PathBuf = comps[..i].iter().collect();
            p.extend(&below);
            return self.resolve_from(&p, cache, restarts + 1);
        }
        let key = FileKey::created(&anchor, &below);
        if let Some((k, e)) = self.entries.get_key_value(&key)
            && let State::Created(c) = &e.state
        {
            return Resolved::Created { key: CreatedKey(k.clone()), content: c };
        }
        match self.children.get_key_value(&key) {
            Some((k, _)) => Resolved::CreatedDir(CreatedDir { key: k, overlay: self }),
            None => Resolved::Missing,
        }
    }

    /// The names the overlay creates in the directory on disk at `path`
    /// (none when it creates nothing there). No system call when the
    /// overlay creates no file; else a `stat` of `path` (once per
    /// `cache`).
    pub fn created_in(&self, path: &Path, cache: &mut DirCache) -> Vec<&OsStr> {
        if self.created == 0 {
            return Vec::new();
        }
        match cache.stat(path) {
            Stat::Dir(k) => self.names_in(&k).collect(),
            Stat::Other | Stat::Absent => Vec::new(),
        }
    }

    fn names_in<'o>(&'o self, dir: &FileKey) -> impl Iterator<Item = &'o OsStr> + 'o {
        self.children.get(dir).into_iter().flat_map(|m| m.keys().map(OsString::as_os_str))
    }

    fn get(&self, key: &FileKey) -> Option<&State> {
        self.entries.get(key).map(|e| &e.state)
    }

    /// What the overlay holds for the file on disk identified by `key`.
    pub(crate) fn lookup(&self, key: &FileKey) -> Option<Held<'_>> {
        match self.get(key)? {
            State::Replaced(c) => Some(Held::Replaced(c)),
            State::Removed => Some(Held::Removed),
            State::Created(_) => None,
        }
    }
}

/// [`Overlay::lookup`].
pub(crate) enum Held<'o> {
    Replaced(&'o Content),
    Removed,
}

/// The number of hard links of the file whose metadata is `m` (1 where
/// the platform does not say).
#[cfg(unix)]
fn hard_links(m: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    m.nlink()
}

#[cfg(not(unix))]
fn hard_links(_m: &fs::Metadata) -> u64 {
    1
}

/// The deepest existing ancestor directory of `path` (which does not
/// exist) and the components below it, checked for
/// [`Overlay::create`].
fn anchor_of(path: &Path) -> io::Result<(FileKey, Vec<OsString>)> {
    let comps: Vec<Component<'_>> = path.components().collect();
    let mut i = comps.len();
    let anchor = loop {
        if i == 0 {
            return Err(not_found());
        }
        i -= 1;
        let prefix: PathBuf = comps[..i].iter().collect();
        let prefix = if prefix.as_os_str().is_empty() { PathBuf::from(".") } else { prefix };
        match fs::metadata(&prefix) {
            Ok(m) if m.is_dir() => break file_key(&prefix, &m)?,
            Ok(_) => return Err(invalid("an ancestor is not a directory")),
            Err(e) if e.kind() == io::ErrorKind::NotADirectory => {
                return Err(invalid("an ancestor is not a directory"));
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if fs::symlink_metadata(&prefix).is_ok() {
                    return Err(invalid("an ancestor is a dangling symbolic link"));
                }
            }
            Err(e) => return Err(e),
        }
    };
    let mut rel = Vec::new();
    for c in &comps[i..] {
        match c {
            Component::Normal(n) => rel.push((*n).to_owned()),
            _ => {
                return Err(invalid(
                    "a '.' or '..' component below the deepest existing directory",
                ));
            }
        }
    }
    Ok((anchor, rel))
}

impl fmt::Debug for Overlay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut files: Vec<_> = self
            .entries
            .values()
            .map(|e| {
                let what = match &e.state {
                    State::Replaced(c) => format!("replaced, {}", c.describe()),
                    State::Created(c) => format!("created, {}", c.describe()),
                    State::Removed => "removed".to_owned(),
                };
                (&e.path, what)
            })
            .collect();
        files.sort();
        f.debug_struct("Overlay").field("files", &files).finish_non_exhaustive()
    }
}
