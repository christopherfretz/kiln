//! Header search: the [`IncludeResolver`] that finds headers the way
//! clang 19's `HeaderSearch::LookupFile` does, over a [`SearchList`] and a
//! [`HeaderFs`]. See `spec/pp.md` "Header search".
//!
//! - An absolute name is opened as is (`#include_next` of one fails).
//! - A `"..."` include (not `#include_next` from a searched header) first
//!   tries the includer's directory, named as clang names it
//!   (`parent(includer)/name`, `./name` for a file without a directory
//!   part); the header inherits the includer's characteristic and has no
//!   search index.
//! - Then the search list from index 0 (`"..."`), from
//!   [`SearchList::angled_start`] (`<...>`) or after the includer's index
//!   (`#include_next`); the header is named `dir/name` and takes the
//!   directory's characteristic.
//! - `#embed` (clang `LookupEmbedFile`): an absolute name as is; for
//!   `"..."` the includer's directory, then the working directory; then
//!   the `--embed-dir` directories.

use std::collections::{HashMap, HashSet};

use kiln_source::{FileId, SourceManager};

use crate::include::{
    EmbedData, EmbedRequest, IncludeKind, IncludeRequest, IncludeResolver, ResolvedInclude,
};
use crate::search::{
    Characteristic, HeaderFs, SearchList, join_path, parent_dir, suggest_include_name,
};

/// Something header search found that the `-M` dependency output wants
/// but the preprocessor's file-change callbacks do not see.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchEvent<'s> {
    /// `__has_include` / `__has_include_next` found `file`.
    HasInclude {
        /// The header.
        file: FileId,
        /// The path it was found by ([`ResolvedInclude::path`]).
        path: &'s str,
        /// Its characteristic.
        characteristic: Characteristic,
    },
    /// `#embed` / `__has_embed` found the resource at `path`.
    Embed {
        /// The resource's name as found.
        path: &'s str,
    },
}

/// An observer of [`SearchEvent`]s.
pub type SearchObserver = Box<dyn FnMut(&SourceManager, SearchEvent<'_>)>;

/// clang-compatible header search.
pub struct HeaderSearch<F: HeaderFs> {
    fs: F,
    list: SearchList,
    embed_dirs: Vec<String>,
    /// Path tried -> the file there (clang `FileManager::SeenFileEntries`).
    cache: HashMap<String, Option<FileId>>,
    /// Directory path -> whether it exists (clang
    /// `FileManager::SeenDirEntries`): a header under a missing directory
    /// is not looked for.
    dirs: HashMap<String, bool>,
    /// `(name, first search index)` -> the index the search found it at,
    /// `None` when it failed (clang `HeaderSearch::LookupFileCache`).
    lookups: HashMap<(String, usize), Option<usize>>,
    /// Directory path (with its trailing `/`) -> what kiln knows of its
    /// entries, to answer repeated misses in it without a system call.
    listings: HashMap<String, Listing>,
    /// clang `HeaderFileInfo::DirInfo`: how each header was last found.
    dir_info: HashMap<FileId, Characteristic>,
    observer: Option<SearchObserver>,
}

impl<F: HeaderFs> std::fmt::Debug for HeaderSearch<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeaderSearch").field("list", &self.list).finish_non_exhaustive()
    }
}

impl<F: HeaderFs> HeaderSearch<F> {
    /// Header search over `list` through `fs`.
    pub fn new(fs: F, list: SearchList) -> HeaderSearch<F> {
        HeaderSearch {
            fs,
            list,
            embed_dirs: Vec::new(),
            cache: HashMap::new(),
            dirs: HashMap::new(),
            lookups: HashMap::new(),
            listings: HashMap::new(),
            dir_info: HashMap::new(),
            observer: None,
        }
    }

    /// Sets the `--embed-dir` directories (searched by `#embed` after the
    /// includer's and the working directory).
    #[must_use]
    pub fn with_embed_dirs(mut self, dirs: Vec<String>) -> HeaderSearch<F> {
        self.embed_dirs = dirs;
        self
    }

    /// Installs an observer for `__has_include` and `#embed` lookups.
    pub fn set_observer(&mut self, o: SearchObserver) {
        self.observer = Some(o);
    }

    /// The file system it searches.
    pub fn fs(&self) -> &F {
        &self.fs
    }

    /// The search list.
    pub fn search_list(&self) -> &SearchList {
        &self.list
    }

    /// How `file` was found (user if it was not found by this search).
    pub fn characteristic(&self, file: FileId) -> Characteristic {
        self.dir_info.get(&file).copied().unwrap_or_default()
    }

    fn open(&mut self, sm: &SourceManager, path: &str) -> Option<FileId> {
        if let Some(&r) = self.cache.get(path) {
            return r;
        }
        let r = self.fs.open(sm, path);
        self.cache.insert(path.to_owned(), r);
        r
    }

    /// Whether the directory `path` exists, asked once per path.
    fn dir_exists(&mut self, path: &str) -> bool {
        if let Some(&r) = self.dirs.get(path) {
            return r;
        }
        let r = self.fs.is_dir(path);
        self.dirs.insert(path.to_owned(), r);
        r
    }

    /// The file `name` in the existing directory `dir`, opened only when
    /// every directory `name` passes through exists (clang
    /// `FileManager::getFileRef` checks the parent directory first; kiln
    /// checks each intermediate one, outermost first, so `dir/boost/x`
    /// missing rules out every header under it with one `stat`). Each
    /// component is first looked up in its directory's listing, if that
    /// was read ([`Listing`]).
    fn open_in(&mut self, sm: &SourceManager, dir: &str, name: &str) -> Option<FileId> {
        let path = join_path(dir, name);
        if let Some(&r) = self.cache.get(&path) {
            return r;
        }
        let mut start = path.len() - name.len();
        loop {
            let end = path[start..].find('/').map(|i| start + i);
            let last = end.is_none();
            let end = end.unwrap_or(path.len());
            let (parent, leaf) = (&path[..start], &path[start..end]);
            if self.listed(parent, leaf) == Some(false) {
                self.cache.insert(path, None);
                return None;
            }
            if last {
                let r = self.open(sm, &path);
                if r.is_none() {
                    self.note_miss(start, &path);
                }
                return r;
            }
            if !leaf.is_empty() && !self.dir_exists(&path[..end]) {
                self.note_miss(start, &path);
                self.cache.insert(path, None);
                return None;
            }
            start = end + 1;
        }
    }

    /// Whether the listing of `parent` (a directory path ending in `/`)
    /// has `leaf`; `None` when that is not known.
    fn listed(&self, parent: &str, leaf: &str) -> Option<bool> {
        if !listable(parent, leaf) {
            return None;
        }
        let entries = self.listings.get(parent)?.entries.as_ref()?;
        Some(entries.contains(&leaf.to_ascii_lowercase()))
    }

    /// Notes that the component of `path` starting at `start` is missing
    /// from its directory, `path[..start]`; reads the directory's listing
    /// on its [`LIST_AFTER_MISSES`]th miss.
    fn note_miss(&mut self, start: usize, path: &str) {
        let parent = &path[..start];
        let leaf = path[start..].split('/').next().unwrap_or("");
        if !listable(parent, leaf) {
            return;
        }
        let listing = self.listings.entry(parent.to_owned()).or_default();
        if listing.entries.is_some() || listing.misses >= LIST_AFTER_MISSES {
            return;
        }
        listing.misses += 1;
        if listing.misses == LIST_AFTER_MISSES {
            let dir = if parent.len() > 1 { &parent[..parent.len() - 1] } else { parent };
            listing.entries = self.fs.list_dir(dir).and_then(|names| {
                names
                    .iter()
                    .all(|n| n.is_ascii())
                    .then(|| names.into_iter().map(|n| n.to_ascii_lowercase()).collect())
            });
        }
    }

    /// clang `HeaderSearch::LookupFile`.
    fn lookup(&mut self, sm: &SourceManager, req: &IncludeRequest) -> Option<ResolvedInclude> {
        let next = matches!(req.kind, IncludeKind::IncludeNext | IncludeKind::HasIncludeNext);
        let from = if next { req.lookup_from } else { None };
        let name = req.name.as_str();
        if name.starts_with('/') {
            if from.is_some() {
                return None;
            }
            let file = self.open(sm, name)?;
            let ch = self.characteristic(file);
            return Some(resolved(file, None, ch, name.to_owned()));
        }
        if !req.angled && from.is_none() {
            // The includer's directory (the working directory for the
            // predefines buffer).
            let dir = req.includer.map_or(".", |f| parent_dir(sm.file_name(f)));
            if let Some(file) = self.open_in(sm, dir, name) {
                let ch = req.includer.map_or(Characteristic::User, |f| self.characteristic(f));
                self.dir_info.insert(file, ch);
                return Some(resolved(file, None, ch, join_path(dir, name)));
            }
        }
        let start = match from {
            Some(i) => i as usize + 1,
            None if req.angled => self.list.angled_start(),
            None => 0,
        };
        let key = (name.to_owned(), start);
        let hit = match self.lookups.get(&key) {
            Some(&hit) => hit,
            None => {
                let hit = self.search(sm, start, name);
                self.lookups.insert(key, hit);
                hit
            }
        };
        let i = hit?;
        let path = join_path(&self.list.dirs()[i].path, name);
        let file = self.open(sm, &path)?;
        let ch = self.list.dirs()[i].characteristic;
        self.dir_info.insert(file, ch);
        Some(resolved(file, u32::try_from(i).ok(), ch, path))
    }

    /// The index of the first search directory from `start` holding `name`.
    fn search(&mut self, sm: &SourceManager, start: usize, name: &str) -> Option<usize> {
        for i in start..self.list.dirs().len() {
            let dir = self.list.dirs()[i].path.clone();
            if self.open_in(sm, &dir, name).is_some() {
                return Some(i);
            }
        }
        None
    }

    /// clang `Preprocessor::LookupEmbedFile`: the path of the resource.
    fn lookup_embed(
        &mut self,
        sm: &SourceManager,
        req: &EmbedRequest,
    ) -> Option<(String, Vec<u8>)> {
        let name = req.name.as_str();
        if name.starts_with('/') {
            return self.fs.read(name).map(|b| (name.to_owned(), b));
        }
        let mut tries: Vec<String> = Vec::new();
        if !req.angled {
            if let Some(f) = req.includer {
                tries.push(join_path(parent_dir(sm.file_name(f)), name));
            }
            tries.push(name.to_owned());
        }
        tries.extend(self.embed_dirs.iter().map(|d| join_path(d, name)));
        tries.into_iter().find_map(|p| self.fs.read(&p).map(|b| (p, b)))
    }
}

/// Misses in one directory after which its listing is read: from then on
/// a name it does not have is a miss without a system call (reading a
/// directory costs about as much as four failed `open`s).
const LIST_AFTER_MISSES: u32 = 4;

/// What header search knows of a directory's entries.
#[derive(Debug, Default)]
struct Listing {
    /// Failed opens in it so far.
    misses: u32,
    /// Its entry names, ASCII-lowercased, once read: a name not among
    /// them (compared ignoring ASCII case, so that a case-insensitive file
    /// system still finds `Foo.h` as `foo.h`) is not there. `None` until
    /// read, and for good when it could not be read or has a non-ASCII
    /// name (Unicode case folding and normalization are left to `open`).
    entries: Option<HashSet<String>>,
}

/// Whether `leaf` may be looked up in the listing of `parent`: a plain
/// ASCII name (not `.`, `..` or empty) in a named directory.
fn listable(parent: &str, leaf: &str) -> bool {
    parent.ends_with('/') && !leaf.is_empty() && leaf != "." && leaf != ".." && leaf.is_ascii()
}

fn resolved(
    file: FileId,
    lookup_index: Option<u32>,
    ch: Characteristic,
    path: String,
) -> ResolvedInclude {
    ResolvedInclude {
        file,
        lookup_index,
        system: ch.is_system(),
        extern_c: ch == Characteristic::ExternCSystem,
        path: Some(path),
    }
}

impl<F: HeaderFs> IncludeResolver for HeaderSearch<F> {
    fn resolve(&mut self, sm: &SourceManager, req: &IncludeRequest) -> Option<ResolvedInclude> {
        let r = self.lookup(sm, req)?;
        if matches!(req.kind, IncludeKind::HasInclude | IncludeKind::HasIncludeNext)
            && let Some(o) = self.observer.as_mut()
        {
            let characteristic = self.dir_info.get(&r.file).copied().unwrap_or_default();
            let path = r.path.as_deref().unwrap_or_else(|| sm.file_name(r.file));
            o(sm, SearchEvent::HasInclude { file: r.file, path, characteristic });
        }
        Some(r)
    }

    fn resolve_embed(&mut self, sm: &SourceManager, req: &EmbedRequest) -> Option<EmbedData> {
        let (path, all) = self.lookup_embed(sm, req)?;
        if let Some(o) = self.observer.as_mut() {
            o(sm, SearchEvent::Embed { path: &path });
        }
        let start = usize::try_from(req.offset.unwrap_or(0)).unwrap_or(usize::MAX).min(all.len());
        let rest = &all[start..];
        let len = req
            .limit
            .map_or(rest.len(), |l| usize::try_from(l).unwrap_or(usize::MAX).min(rest.len()));
        Some(EmbedData { bytes: rest[..len].to_vec() })
    }

    fn suggest_include(
        &self,
        sm: &SourceManager,
        file: FileId,
        main_dir: Option<&str>,
    ) -> Option<String> {
        let cwd = std::env::current_dir().ok()?;
        // The suggestion names a header of an imported unit, which clang 22
        // and 19 read back from the BMI as an absolute path and compare
        // with the search directories as written: a relative directory
        // never matches (the main file's directory is absolute there).
        let dirs = self
            .list
            .dirs()
            .iter()
            .filter(|d| d.path.starts_with('/'))
            .map(|d| (d.path.as_str(), d.characteristic.is_system()));
        Some(suggest_include_name(sm.file_name(file), dirs, main_dir, &cwd.to_string_lossy()))
    }
}
