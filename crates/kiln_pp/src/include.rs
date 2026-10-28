//! The `#include` / `#embed` hook ([cpp.include], [cpp.embed]).
//!
//! The core parses the directive into an [`IncludeRequest`] /
//! [`EmbedRequest`] and asks the installed [`IncludeResolver`] for the
//! file. Search paths, header maps and the file cache are WP-014
//! (`#include`) and WP-016 (`#embed`); they plug in by implementing
//! this trait. [`MemoryResolver`] is an in-memory resolver for tests.

use std::collections::HashMap;

use kiln_source::{FileId, SourceManager, Span};

/// Which construct asked for a file.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum IncludeKind {
    /// `#include`.
    Include,
    /// `#include_next` (GNU): continue the search after the directory the
    /// current file was found in.
    IncludeNext,
    /// `#import` (Objective-C / GNU; `#include` with implicit `#pragma once`).
    Import,
    /// `__has_include(...)`: only existence is asked; the file is not entered.
    HasInclude,
    /// `__has_include_next(...)`.
    HasIncludeNext,
}

/// One request to find a header ([cpp.include]/2-4).
#[derive(Clone, Debug)]
pub struct IncludeRequest {
    /// The directive or operator.
    pub kind: IncludeKind,
    /// The name between the delimiters (`<` `>` or `"` `"`), as written
    /// (escape sequences are not processed, [lex.header]).
    pub name: String,
    /// `<...>` form.
    pub angled: bool,
    /// The physical file containing the directive (`None` for the
    /// predefines buffer or an unknown context).
    pub includer: Option<FileId>,
    /// For `#include_next` / `__has_include_next`: the
    /// [`ResolvedInclude::lookup_index`] of the file containing the
    /// directive, `None` if it was not found through a search path (the
    /// main file), in which case the search starts at the beginning, as
    /// for `#include` (clang warns `pp_include_next_in_primary`).
    pub lookup_from: Option<u32>,
    /// Span of the header name (the whole `<...>` / `"..."` token or token
    /// sequence).
    pub name_span: Span,
    /// Span of the directive name (`include`, `__has_include`, ...).
    pub directive_span: Span,
}

/// A found header.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ResolvedInclude {
    /// The buffer, already loaded into the source manager.
    pub file: FileId,
    /// Position in the search path where it was found (for
    /// `#include_next`); `None` if found relative to the includer.
    pub lookup_index: Option<u32>,
    /// Found in a system directory, or relative to a system header
    /// (suppresses warnings; line-marker flag 3).
    pub system: bool,
    /// Found in an `extern "C"` system directory (clang
    /// `C_ExternCSystem`, `-internal-externc-isystem`; line-marker flag
    /// 4). Implies `system`.
    pub extern_c: bool,
}

/// One `#embed` request ([cpp.embed]). The resource name is resolved like
/// an include; the parameters were already parsed by the core.
#[derive(Clone, Debug)]
pub struct EmbedRequest {
    /// The name between the delimiters.
    pub name: String,
    /// `<...>` form.
    pub angled: bool,
    /// The physical file containing the directive.
    pub includer: Option<FileId>,
    /// `limit(N)`: read at most this many bytes.
    pub limit: Option<u64>,
    /// `clang::offset(N)`: skip this many bytes first.
    pub offset: Option<u64>,
    /// Span of the resource name.
    pub name_span: Span,
    /// True for `__has_embed`: only existence and emptiness are asked.
    pub query_only: bool,
}

/// The contents of an embedded resource.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EmbedData {
    /// The bytes, already cut to `offset`/`limit`.
    pub bytes: Vec<u8>,
}

/// Finds headers and resources for the preprocessor.
///
/// Implemented by WP-014's header search (and WP-016 for `#embed`).
/// Methods take `&mut self` so implementations can cache freely.
pub trait IncludeResolver {
    /// Finds and loads the header named by `req`, or `None` if it does not
    /// exist (the core reports `pp_file_not_found`, a fatal error, for
    /// `#include`; `__has_include` evaluates to 0).
    fn resolve(&mut self, sm: &SourceManager, req: &IncludeRequest) -> Option<ResolvedInclude>;

    /// Finds and reads the resource named by `req`, or `None` if it does
    /// not exist. The default supports no resources.
    fn resolve_embed(&mut self, sm: &SourceManager, req: &EmbedRequest) -> Option<EmbedData> {
        let _ = (sm, req);
        None
    }
}

/// The resolver installed by default: finds nothing. `#include` then
/// reports `pp_file_not_found`.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoIncludes;

impl IncludeResolver for NoIncludes {
    fn resolve(&mut self, _sm: &SourceManager, _req: &IncludeRequest) -> Option<ResolvedInclude> {
        None
    }
}

/// An in-memory resolver for tests: a flat map from header name to text
/// (both `"name"` and `<name>` look in the same map; `system_headers`
/// lists names found through the angled path only, with
/// `lookup_index = Some(1)`; others get `Some(0)`). Each header is added
/// to the source manager as a virtual buffer the first time it is found.
///
/// `#include_next` from a header found at index `i` finds only
/// `next_headers` entries (a second "directory" at index `i + 1`).
#[derive(Clone, Debug, Default)]
pub struct MemoryResolver {
    headers: HashMap<String, String>,
    system_headers: HashMap<String, String>,
    next_headers: HashMap<String, String>,
    resources: HashMap<String, Vec<u8>>,
    loaded: HashMap<(u8, String), FileId>,
}

impl MemoryResolver {
    /// An empty resolver.
    pub fn new() -> MemoryResolver {
        MemoryResolver::default()
    }

    /// Adds a header found by both `"name"` and `<name>`.
    #[must_use]
    pub fn with_header(mut self, name: &str, text: &str) -> MemoryResolver {
        self.headers.insert(name.into(), text.into());
        self
    }

    /// Adds a header found only by `<name>`, as a system header.
    #[must_use]
    pub fn with_system_header(mut self, name: &str, text: &str) -> MemoryResolver {
        self.system_headers.insert(name.into(), text.into());
        self
    }

    /// Adds a header found only by `#include_next` from a header.
    #[must_use]
    pub fn with_next_header(mut self, name: &str, text: &str) -> MemoryResolver {
        self.next_headers.insert(name.into(), text.into());
        self
    }

    /// Adds an `#embed` resource.
    #[must_use]
    pub fn with_resource(mut self, name: &str, bytes: &[u8]) -> MemoryResolver {
        self.resources.insert(name.into(), bytes.to_vec());
        self
    }

    fn load(&mut self, sm: &SourceManager, dir: u8, name: &str, text: &str) -> Option<FileId> {
        if let Some(&f) = self.loaded.get(&(dir, name.to_string())) {
            return Some(f);
        }
        let f = sm.add_virtual(name, text.to_string()).ok()?;
        self.loaded.insert((dir, name.to_string()), f);
        Some(f)
    }
}

impl IncludeResolver for MemoryResolver {
    fn resolve(&mut self, sm: &SourceManager, req: &IncludeRequest) -> Option<ResolvedInclude> {
        let next = matches!(req.kind, IncludeKind::IncludeNext | IncludeKind::HasIncludeNext)
            && req.lookup_from.is_some();
        if next {
            let text = self.next_headers.get(&req.name)?.clone();
            let file = self.load(sm, 2, &req.name, &text)?;
            return Some(ResolvedInclude { file, lookup_index: Some(2), system: false, extern_c: false });
        }
        if let Some(text) = self.headers.get(&req.name).cloned() {
            let file = self.load(sm, 0, &req.name, &text)?;
            return Some(ResolvedInclude { file, lookup_index: Some(0), system: false, extern_c: false });
        }
        if req.angled
            && let Some(text) = self.system_headers.get(&req.name).cloned()
        {
            let file = self.load(sm, 1, &req.name, &text)?;
            return Some(ResolvedInclude { file, lookup_index: Some(1), system: true, extern_c: false });
        }
        None
    }

    fn resolve_embed(&mut self, _sm: &SourceManager, req: &EmbedRequest) -> Option<EmbedData> {
        let all = self.resources.get(&req.name)?;
        let start = usize::try_from(req.offset.unwrap_or(0)).unwrap_or(usize::MAX).min(all.len());
        let rest = &all[start..];
        let len = req
            .limit
            .map_or(rest.len(), |l| usize::try_from(l).unwrap_or(usize::MAX).min(rest.len()));
        Some(EmbedData { bytes: rest[..len].to_vec() })
    }
}
