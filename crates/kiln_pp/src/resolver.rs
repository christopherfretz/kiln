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

use std::collections::HashMap;

use kiln_source::{FileId, SourceManager};

use crate::include::{
    EmbedData, EmbedRequest, IncludeKind, IncludeRequest, IncludeResolver, ResolvedInclude,
};
use crate::search::{Characteristic, HeaderFs, SearchList, join_path, parent_dir};

/// Something header search found that the `-M` dependency output wants
/// but the preprocessor's file-change callbacks do not see.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchEvent<'s> {
    /// `__has_include` / `__has_include_next` found `file`.
    HasInclude {
        /// The header.
        file: FileId,
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
    /// Path tried -> the file there.
    cache: HashMap<String, Option<FileId>>,
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
            return Some(resolved(file, None, ch));
        }
        if !req.angled && from.is_none() {
            // The includer's directory (the working directory for the
            // predefines buffer).
            let dir = req.includer.map_or(".", |f| parent_dir(sm.file_name(f)));
            let path = join_path(dir, name);
            if let Some(file) = self.open(sm, &path) {
                let ch = req.includer.map_or(Characteristic::User, |f| self.characteristic(f));
                self.dir_info.insert(file, ch);
                return Some(resolved(file, None, ch));
            }
        }
        let start = match from {
            Some(i) => i as usize + 1,
            None if req.angled => self.list.angled_start(),
            None => 0,
        };
        for i in start..self.list.dirs().len() {
            let d = &self.list.dirs()[i];
            let ch = d.characteristic;
            let path = join_path(&d.path, name);
            if let Some(file) = self.open(sm, &path) {
                self.dir_info.insert(file, ch);
                let idx = u32::try_from(i).ok();
                return Some(resolved(file, idx, ch));
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

fn resolved(file: FileId, lookup_index: Option<u32>, ch: Characteristic) -> ResolvedInclude {
    ResolvedInclude {
        file,
        lookup_index,
        system: ch.is_system(),
        extern_c: ch == Characteristic::ExternCSystem,
    }
}

impl<F: HeaderFs> IncludeResolver for HeaderSearch<F> {
    fn resolve(&mut self, sm: &SourceManager, req: &IncludeRequest) -> Option<ResolvedInclude> {
        let r = self.lookup(sm, req)?;
        if matches!(req.kind, IncludeKind::HasInclude | IncludeKind::HasIncludeNext)
            && let Some(o) = self.observer.as_mut()
        {
            let characteristic = self.dir_info.get(&r.file).copied().unwrap_or_default();
            o(sm, SearchEvent::HasInclude { file: r.file, characteristic });
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
}
