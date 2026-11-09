//! Module and import directives with modules enabled (spec/modules.md §3):
//! the directive is consumed here and reported as a [`ModuleDirective`];
//! an import builds the imported unit in-process by pushing its source
//! with fresh preprocessor state, bracketed by [`Marker::UnitBegin`] /
//! [`Marker::UnitEnd`].

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use kiln_diag::{Diag, DiagId};
use kiln_lex::{Punct, TokKind, Token};
use kiln_source::symbol::Symbol;
use kiln_source::{FileId, Loc, Span};

use crate::bmi::{
    self, BmiError, Capsule, CapsuleFile, EmbedEntry, EmbedKey, IncludeEntry, IncludeHit,
    IncludeKey, UnitKind,
};
use crate::directive::Dir;
use crate::include::{EmbedData, EmbedRequest, IncludeKind, IncludeRequest, ResolvedInclude};
use crate::macros::{MacroDef, MacroTable, SymMap};
use crate::preprocessor::{Frame, Marker, Preprocessor};

/// Where module interfaces are found (spec/modules.md §2, §11).
#[derive(Clone, Debug, Default)]
pub struct ModuleOptions {
    /// `-fmodule-file=<name>=<path>`: the source or module file of module
    /// `<name>` (`M:P` for a partition).
    pub module_files: Vec<(String, PathBuf)>,
    /// `-fprebuilt-module-path=<dir>`, in order: `<dir>/<name>.pcm`, then
    /// `.cppm`, `.ixx`, `.cpp` (`M-P.*` for partition `M:P`).
    pub prebuilt_paths: Vec<PathBuf>,
    /// This compilation's flags fingerprint, compared with a module
    /// file's (spec/modules.md §11.4).
    pub fingerprint: Vec<(String, String)>,
    /// Record the main unit's inputs for a module file
    /// ([`Preprocessor::take_capsule`]).
    pub record: bool,
    /// The libc++ module source directory (`<llvm>/share/libc++/v1`),
    /// holding `std.cppm` and `std.compat.cppm`.
    pub std_dir: Option<PathBuf>,
    /// Dependency scanning (`kiln-scan-deps`, spec/modules.md §11.9):
    /// module directives are recorded ([`Preprocessor::scanned_imports`])
    /// and no import is performed; a header-unit import is an error.
    pub scan: bool,
    /// The toolchain's module-file cache for libc++'s `std` and
    /// `std.compat` (spec/modules.md §11.11): asked wherever one of them
    /// would otherwise be built from its source in `std_dir`.
    pub std_cache: Option<StdModuleCache>,
}

/// A lookup in the toolchain's module-file cache (spec/modules.md
/// §11.11): given `std` or `std.compat` and its libc++ source, the module
/// file to read instead (built and stored on a miss), or `None` to build
/// the unit from its source in-process (part A).
#[derive(Clone)]
pub struct StdModuleCache(pub Rc<dyn Fn(&str, &Path) -> Option<PathBuf>>);

impl std::fmt::Debug for StdModuleCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StdModuleCache")
    }
}

/// A consumed module or import directive ([cpp.module], [cpp.import]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleDirective {
    /// The first token (`export`, `module` or `import`).
    pub span: Span,
    /// Starts with `export`.
    pub export: bool,
    /// The `module` / `import` keyword (`span` unless `export`).
    pub kw: Span,
    /// What it declares.
    pub kind: DirectiveKind,
    /// The `#` of the unit's first directive, when a directive other than
    /// a null directive, a line marker, `#line`, `#pragma`, `#error`,
    /// `#warning` or `#ident` preceded this one in the unit's own file
    /// ([cpp.pre]/1 as amended by P3868R1; clang 22's first pp-token).
    pub after_directive: Option<Loc>,
}

/// The kinds of [`ModuleDirective`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DirectiveKind {
    /// `module;`: starts the global module fragment.
    GlobalFragment,
    /// `module M.N;` / `module M:P;` (with `export`: an interface).
    Module {
        /// The module name, dotted (`M.N`).
        name: String,
        /// The partition name, if any (`P`).
        partition: Option<String>,
        /// The name's first token.
        name_span: Span,
    },
    /// `module :private;`.
    Private {
        /// The `private` token.
        span: Span,
    },
    /// `import M;`, `import :P;`, `import M:P;`, `import <h>;`.
    Import {
        /// The unit's key: the module name (`M`, `M:P`), the partition
        /// as written (`:P`) outside a named module, or `"` plus the
        /// path of a header unit.
        key: String,
        /// As written (for diagnostics).
        display: String,
        /// A header unit.
        header: bool,
        /// The name's first token.
        name_span: Span,
    },
    /// A malformed directive (already diagnosed).
    Invalid,
}

/// The importer's preprocessor state, saved while an imported unit is
/// read (a unit starts with only the predefined macros).
pub(crate) struct UnitSave {
    key: String,
    header: bool,
    macros: MacroTable,
    once_files: HashSet<FileId>,
    guards: HashMap<FileId, Symbol>,
    poisoned: SymMap<Span>,
    pushed: SymMap<Vec<Option<Rc<MacroDef>>>>,
    if_ndef: Option<Symbol>,
    include_depth: u32,
    first_hash: Option<Loc>,
    pre_directive: bool,
    module: Option<String>,
}

/// Per-invocation module state.
#[derive(Debug, Default)]
pub(crate) struct ModState {
    pub(crate) opts: ModuleOptions,
    /// Units being built, outermost first.
    building: Vec<String>,
    /// Units built, and the macros a header unit exports.
    built: HashMap<String, Vec<Rc<MacroDef>>>,
    /// The named module of the current unit (after its module
    /// declaration).
    module: Option<String>,
    /// The predefines buffer (`<built-in>`), re-entered by every unit.
    pub(crate) builtin_file: Option<FileId>,
    /// Header units by key: the file and whether it is a system header.
    header_files: HashMap<String, (FileId, bool)>,
    /// The main unit's inputs, recorded for a module file.
    pub(crate) record: Option<Box<Recorder>>,
    /// Module files being or having been replayed.
    replays: Vec<Replay>,
    /// Parallel to `building`: the replay of each unit read from a module
    /// file.
    unit_replay: Vec<Option<usize>>,
    /// The replay of the main unit, when it is read from a module file.
    main_replay: Option<usize>,
    /// The main unit's module declaration: its key and, when it is
    /// importable, its kind.
    main_unit: Option<(String, Option<UnitKind>)>,
    /// With [`ModuleOptions::scan`]: the main unit's named-module imports
    /// (`M`, `M:P`), in order, without duplicates.
    scanned: Vec<String>,
}

impl ModState {
    pub(crate) fn new(opts: ModuleOptions) -> ModState {
        let record = opts.record.then(Box::default);
        ModState { opts, record, ..ModState::default() }
    }
}

/// The resolver lookups of the main unit, for its module file
/// (spec/modules.md §11.2).
#[derive(Debug, Default)]
pub(crate) struct Recorder {
    files: Vec<FileId>,
    index: HashMap<FileId, u32>,
    includes: Vec<IncludeEntry>,
    include_keys: HashSet<IncludeKey>,
    embeds: Vec<EmbedEntry>,
    embed_keys: HashSet<EmbedKey>,
    imports: Vec<(String, String)>,
}

impl Recorder {
    pub(crate) fn file(&mut self, f: FileId) -> u32 {
        if let Some(&i) = self.index.get(&f) {
            return i;
        }
        let i = u32::try_from(self.files.len()).unwrap_or(bmi::INCLUDER_NONE - 1);
        self.files.push(f);
        self.index.insert(f, i);
        i
    }

    fn includer(&mut self, f: Option<FileId>) -> u32 {
        f.map_or(bmi::INCLUDER_NONE, |f| self.file(f))
    }
}

/// A module file read back: its lookups answered from the logs and its
/// files loaded on first use (spec/modules.md §11.2).
struct Replay {
    /// The module file's path.
    path: String,
    /// Each file's path and, until it is loaded, its text.
    files: Vec<(String, Option<String>)>,
    ids: Vec<Option<FileId>>,
    index: HashMap<FileId, u32>,
    includes: HashMap<IncludeKey, Option<IncludeHit>>,
    embeds: HashMap<EmbedKey, Option<Vec<u8>>>,
    /// The unit's own `<built-in>` buffer.
    builtin: Option<FileId>,
    imports: Vec<(String, String)>,
}

impl std::fmt::Debug for Replay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Replay").field("path", &self.path).finish_non_exhaustive()
    }
}

impl Replay {
    fn new(path: String, c: Capsule, builtin: Option<FileId>) -> Replay {
        let ids = vec![None; c.files.len()];
        Replay {
            path,
            files: c.files.into_iter().map(|f| (f.path, Some(f.text))).collect(),
            ids,
            index: HashMap::new(),
            includes: c.includes.into_iter().map(|e| (e.key, e.hit)).collect(),
            embeds: c.embeds.into_iter().map(|e| (e.key, e.bytes)).collect(),
            builtin,
            imports: c.imports,
        }
    }

    fn includer(&self, f: Option<FileId>) -> u32 {
        match f {
            None => bmi::INCLUDER_NONE,
            Some(f) => self.index.get(&f).copied().unwrap_or(bmi::INCLUDER_NONE - 1),
        }
    }
}

/// Whose lookups a resolver request belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ctx {
    /// The main unit (or a header unit it imports): the header search,
    /// recorded when a module file is requested.
    Main,
    /// A unit read from a module file.
    Replay(usize),
    /// A named module built from its source (part A): the header search.
    Source,
}

fn is_pcm(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "pcm")
}

impl std::fmt::Debug for UnitSave {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnitSave").field("key", &self.key).finish_non_exhaustive()
    }
}

/// True if `text` has a line starting with `module`, `import` or `export`
/// followed by a non-identifier character: a cheap upper bound for "this
/// file contains a module or import directive".
pub fn has_module_lines(text: &str) -> bool {
    text.lines().any(|l| {
        let l = l.trim_start();
        ["module", "import", "export"].iter().any(|k| {
            l.strip_prefix(k).is_some_and(|r| {
                r.chars().next().is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '$'))
            })
        })
    })
}

impl Preprocessor<'_, '_> {
    /// With modules enabled, reads a module or import directive whose
    /// first token `t` (`export`, `module`, `import`) was just lexed and
    /// whose next token is `kw` (`module` / `import`) if `t` is `export`;
    /// queues its marker and performs the import.
    pub(crate) fn module_directive(&mut self, t: Token, export: bool, import: bool) {
        self.start_directive();
        let kw_span = if export { self.lex_unexpanded().tok.span } else { t.span };
        let kw = if import { "import" } else { "module" };
        let kind = self.read_directive_body(import, kw);
        let after_directive = if self.unit_pre_directive { self.unit_first_hash } else { None };
        let d = ModuleDirective { span: t.span, export, kw: kw_span, kind, after_directive };
        self.finish_directive();
        let perform = match &d.kind {
            DirectiveKind::Module { name, partition, name_span } => {
                if let Some(m) = &mut self.modules {
                    m.module = Some(name.clone());
                    if m.building.is_empty() && m.main_unit.is_none() {
                        let key = match partition {
                            Some(p) => format!("{name}:{p}"),
                            None => name.clone(),
                        };
                        let kind = match (export, partition.is_some()) {
                            (true, false) => Some(UnitKind::Interface),
                            (true, true) => Some(UnitKind::InterfacePartition),
                            (false, true) => Some(UnitKind::ImplementationPartition),
                            (false, false) => None,
                        };
                        m.main_unit = Some((key, kind));
                    }
                }
                // [module.unit]/8: an implementation unit implicitly
                // imports its primary interface.
                (!export && partition.is_none()).then(|| (name.clone(), *name_span))
            }
            DirectiveKind::Import { key, header: false, name_span, .. }
                if !key.starts_with(':') =>
            {
                Some((key.clone(), *name_span))
            }
            _ => None,
        };
        let header = match &d.kind {
            DirectiveKind::Import { header: true, key, display, name_span } => {
                Some((key.clone(), display.clone(), *name_span))
            }
            _ => None,
        };
        self.queue(Marker::Module { span: t.span, export, import, directive: Some(d) });
        if let Some(m) = self.modules.as_mut().filter(|m| m.opts.scan) {
            // Scanning records what the unit needs and reads nothing.
            if let Some((key, _)) = perform
                && !m.scanned.contains(&key)
            {
                m.scanned.push(key);
            }
            if let Some((_, shown, span)) = header {
                self.emit(Diag::new(DiagId::pp_scan_header_unit, span).arg(shown));
            }
            return;
        }
        if let Some((key, span)) = perform {
            self.import_module(&key, span);
        }
        if let Some((key, _, span)) = header {
            self.import_header(key, span);
        }
    }

    /// A directive starting at `at` in the current file, at include depth
    /// 0: records the unit's first directive and whether one that may not
    /// precede a module directive was seen (`exempt`: a null directive or
    /// a line marker).
    pub(crate) fn note_unit_directive(&mut self, at: Loc, dir: Option<Dir>, exempt: bool) {
        if self.cur_file_ref().is_none_or(|f| self.is_builtin_file(f.file)) {
            return;
        }
        self.unit_first_hash.get_or_insert(at);
        let allowed = matches!(
            dir,
            None | Some(
                Dir::Line | Dir::Pragma | Dir::Error | Dir::Warning | Dir::Ident | Dir::Sccs
            )
        );
        if !exempt && !allowed {
            self.unit_pre_directive = true;
        }
    }

    /// The directive after `module` / `import`, through the `;`.
    fn read_directive_body(&mut self, import: bool, kw: &str) -> DirectiveKind {
        let mut t = self.lex_unexpanded().tok;
        let kind = match t.kind {
            TokKind::Punct(Punct::Semi) if !import => return DirectiveKind::GlobalFragment,
            TokKind::Punct(Punct::Less) | TokKind::StringLit(_) | TokKind::HeaderName if import => {
                self.unlex_file_tok(t);
                match self.read_header_import() {
                    Some(k) => k,
                    None => return DirectiveKind::Invalid,
                }
            }
            TokKind::Punct(Punct::Colon) => {
                let c = self.lex_unexpanded().tok;
                if !import
                    && let TokKind::Identifier(s) = c.kind
                    && self.interner.as_str(s) == "private"
                {
                    DirectiveKind::Private { span: c.span }
                } else {
                    t = c;
                    let Some((p, _)) = self.read_module_name(&mut t, kw) else {
                        return DirectiveKind::Invalid;
                    };
                    self.unlex_file_tok(t);
                    match self.modules.as_ref().and_then(|m| m.module.clone()) {
                        Some(m) if import => DirectiveKind::Import {
                            key: format!("{m}:{p}"),
                            display: format!(":{p}"),
                            header: false,
                            name_span: c.span,
                        },
                        _ if import => DirectiveKind::Import {
                            key: format!(":{p}"),
                            display: format!(":{p}"),
                            header: false,
                            name_span: c.span,
                        },
                        _ => {
                            self.emit(Diag::new(DiagId::pp_module_expected_name, c.span).arg(kw));
                            return DirectiveKind::Invalid;
                        }
                    }
                }
            }
            _ => {
                let at = t.span;
                let Some((name, _)) = self.read_module_name(&mut t, kw) else {
                    return DirectiveKind::Invalid;
                };
                let mut partition = None;
                if t.kind == TokKind::Punct(Punct::Colon) {
                    t = self.lex_unexpanded().tok;
                    let Some((p, _)) = self.read_module_name(&mut t, kw) else {
                        return DirectiveKind::Invalid;
                    };
                    partition = Some(p);
                }
                self.unlex_file_tok(t);
                if import {
                    let key = match &partition {
                        Some(p) => format!("{name}:{p}"),
                        None => name,
                    };
                    DirectiveKind::Import {
                        display: key.clone(),
                        key,
                        header: false,
                        name_span: at,
                    }
                } else {
                    DirectiveKind::Module { name, partition, name_span: at }
                }
            }
        };
        self.skip_to_semi();
        kind
    }

    /// `M.N` starting at `t`; leaves the token after it in `t`.
    fn read_module_name(&mut self, t: &mut Token, kw: &str) -> Option<(String, Span)> {
        let start = t.span;
        let mut name = String::new();
        loop {
            let TokKind::Identifier(s) = t.kind else {
                self.emit(Diag::new(DiagId::pp_module_expected_name, t.span).arg(kw));
                self.unlex_file_tok(*t);
                self.skip_to_semi();
                return None;
            };
            if self.macros.get(s).is_some_and(|d| !d.function_like) {
                let sp = self.interner.as_str(s).to_string();
                self.emit(Diag::new(DiagId::pp_module_name_is_macro, t.span).arg(sp));
            }
            name.push_str(self.interner.as_str(s));
            *t = self.lex_unexpanded().tok;
            if t.kind != TokKind::Punct(Punct::Period) {
                return Some((name, start));
            }
            name.push('.');
            *t = self.lex_unexpanded().tok;
        }
    }

    /// Skips attributes to the `;` (diagnosing a missing one).
    fn skip_to_semi(&mut self) {
        let mut depth = 0u32;
        loop {
            let t = self.lex_unexpanded().tok;
            match t.kind {
                TokKind::Punct(Punct::Semi) if depth == 0 => return,
                TokKind::Punct(Punct::LSquare | Punct::LParen) => depth += 1,
                TokKind::Punct(Punct::RSquare | Punct::RParen) => depth = depth.saturating_sub(1),
                TokKind::Eod | TokKind::Eof => {
                    self.emit(Diag::new(DiagId::pp_module_expected_semi, t.span));
                    return;
                }
                _ => {}
            }
        }
    }

    /// `import <h>;` / `import "h";`: resolves the header like
    /// `#include`.
    fn read_header_import(&mut self) -> Option<DirectiveKind> {
        let Ok(ft) = self.lex_header_name() else { return None };
        if ft.kind() != TokKind::HeaderName {
            self.emit(Diag::new(DiagId::pp_include_expects_file, ft.tok.span));
            return None;
        }
        let (name, angled) = self.include_name(&ft.tok, ft.tok.span)?;
        let req = IncludeRequest {
            kind: IncludeKind::Include,
            name,
            angled,
            includer: self.includer(),
            lookup_from: None,
            name_span: ft.tok.span,
            directive_span: ft.tok.span,
        };
        let res = self.resolve_include(&req);
        self.callbacks.inclusion_directive(&req, res.as_ref());
        let Some(r) = res else {
            self.emit(Diag::new(DiagId::pp_file_not_found, ft.tok.span).arg(&req.name));
            return None;
        };
        let display = self.spelling(&ft.tok).into_owned();
        let key = format!("\"{}", self.sm.file_name(r.file));
        if let Some(m) = &mut self.modules {
            m.header_files.insert(key.clone(), (r.file, r.system));
        }
        Some(DirectiveKind::Import { key, display, header: true, name_span: ft.tok.span })
    }

    /// The module file or interface source of module `name`
    /// (spec/modules.md §2, §11.6).
    fn find_module(&self, name: &str) -> Option<PathBuf> {
        let m = self.modules.as_ref()?;
        let o = &m.opts;
        if let Some((_, p)) = o.module_files.iter().rev().find(|(n, _)| n == name) {
            return Some(p.clone());
        }
        let stem = name.replace(':', "-");
        for dir in &o.prebuilt_paths {
            for ext in ["pcm", "cppm", "ixx", "cpp"] {
                let p = dir.join(format!("{stem}.{ext}"));
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        // The paths the module files read so far recorded for their
        // imports (clang reads the module files a BMI names).
        for r in m.replays.iter().rev() {
            if let Some((_, p)) = r.imports.iter().find(|(n, _)| n == name) {
                return Some(PathBuf::from(p));
            }
        }
        if matches!(name, "std" | "std.compat")
            && let Some(d) = &o.std_dir
        {
            let p = d.join(format!("{name}.cppm"));
            if p.is_file() {
                return Some(p);
            }
        }
        None
    }

    /// The module file the toolchain cache gives for `std` or
    /// `std.compat` when `path` is libc++'s own source of it
    /// (spec/modules.md §11.11), else `path`. `std.compat` imports `std`,
    /// so it is taken from the cache only when `std` is libc++'s too.
    fn cached_std(&self, key: &str, path: PathBuf) -> PathBuf {
        let Some(m) = &self.modules else { return path };
        let (Some(cache), Some(dir)) = (&m.opts.std_cache, &m.opts.std_dir) else { return path };
        let source = |name: &str| dir.join(format!("{name}.cppm"));
        if !matches!(key, "std" | "std.compat") || path != source(key) {
            return path;
        }
        if key == "std.compat" && self.find_module("std") != Some(source("std")) {
            return path;
        }
        let cache = Rc::clone(&cache.0);
        cache(key, &path).unwrap_or(path)
    }

    /// Imports named module (or partition) `key`: from a module file
    /// (spec/modules.md §11) or from its source (§1).
    fn import_module(&mut self, key: &str, span: Span) {
        if self.already_built(key, span) {
            return;
        }
        let Some(source) = self.find_module(key) else {
            self.emit(Diag::new(DiagId::pp_module_not_found, span).arg(key));
            return;
        };
        // A module file records the path it found, never the cache's: an
        // importer of it looks the cache up again (§11.11).
        if self.unit_ctx() == Ctx::Main
            && let Some(rec) = self.modules.as_mut().and_then(|m| m.record.as_mut())
            && !rec.imports.iter().any(|(k, _)| k == key)
        {
            rec.imports.push((key.to_owned(), source.display().to_string()));
        }
        let path = self.cached_std(key, source);
        let shown = path.display().to_string();
        match bmi::sniff(&path) {
            bmi::Sniffed::Kiln => {
                let read = bmi::read_file(&path);
                let Some(c) = self.load_capsule(&shown, read, Some(key), span) else { return };
                self.begin_capsule_unit(key.to_owned(), shown, c, span);
            }
            bmi::Sniffed::Clang => {
                self.emit(Diag::new(DiagId::pp_module_file_clang, span).arg(shown));
            }
            bmi::Sniffed::Missing if is_pcm(&path) => {
                self.emit(Diag::new(DiagId::pp_module_file_not_found, span).arg(shown));
            }
            // A `.pcm` is never read as a module source (§11.4).
            bmi::Sniffed::Source if is_pcm(&path) => {
                self.emit(Diag::new(DiagId::pp_not_module_file, span).arg(shown));
            }
            bmi::Sniffed::Source | bmi::Sniffed::Missing => {
                if let Ok(file) = self.sm.load_file(Path::new(&path)) {
                    self.begin_unit(key.to_string(), false, file, false, span, None);
                } else {
                    self.emit(Diag::new(DiagId::pp_file_not_found, span).arg(shown));
                }
            }
        }
    }

    /// Checks the module file `path` as read (`read`), expected to hold
    /// module `expected` when that is known (spec/modules.md §11.4); every
    /// failure is diagnosed at `span`.
    fn load_capsule(
        &mut self,
        path: &str,
        read: Result<Capsule, BmiError>,
        expected: Option<&str>,
        span: Span,
    ) -> Option<Capsule> {
        let c = match read {
            Ok(c) => c,
            Err(BmiError::NotKiln { clang: true }) => {
                self.emit(Diag::new(DiagId::pp_module_file_clang, span).arg(path));
                return None;
            }
            Err(BmiError::NotKiln { clang: false }) => {
                self.emit(Diag::new(DiagId::pp_not_module_file, span).arg(path));
                return None;
            }
            Err(BmiError::Format(v)) => {
                let d = Diag::new(DiagId::pp_module_file_format, span)
                    .arg(path)
                    .arg(v.to_string())
                    .arg(bmi::FORMAT_VERSION.to_string());
                self.emit(d);
                return None;
            }
            Err(BmiError::Corrupt(why)) => {
                self.emit(Diag::new(DiagId::pp_module_file_corrupt, span).arg(path).arg(why));
                return None;
            }
        };
        if c.version != bmi::KILN_VERSION {
            let d = Diag::new(DiagId::pp_module_file_version, span)
                .arg(path)
                .arg(&c.version)
                .arg(bmi::KILN_VERSION);
            self.emit(d);
            return None;
        }
        if let Some(e) = expected
            && c.module != e
        {
            let d = Diag::new(DiagId::pp_module_file_wrong_module, span)
                .arg(path)
                .arg(&c.module)
                .arg(e);
            self.emit(d);
            return None;
        }
        let current = self.modules.as_ref().map(|m| m.opts.fingerprint.clone()).unwrap_or_default();
        let diffs = bmi::mismatches(&c.fingerprint, &current);
        if !diffs.is_empty() {
            for (was, now) in diffs {
                let d = Diag::new(DiagId::pp_module_file_config, span).arg(path).arg(was).arg(now);
                self.emit(d);
            }
            self.emit(Diag::new(DiagId::pp_module_file_config_mismatch, span).arg(path));
            return None;
        }
        Some(c)
    }

    /// Starts reading unit `key` from the module file `path`.
    fn begin_capsule_unit(&mut self, key: String, path: String, c: Capsule, span: Span) {
        let builtin = self.sm.add_virtual("<built-in>", c.predefines.clone()).ok();
        let Some(m) = &mut self.modules else { return };
        m.replays.push(Replay::new(path.clone(), c, builtin));
        let i = m.replays.len() - 1;
        let Some(file) = self.replay_file(i, 0) else {
            self.emit(Diag::new(DiagId::pp_module_file_not_found, span).arg(path));
            return;
        };
        self.begin_unit(key, false, file, false, span, Some(i));
    }

    /// Enters the module file `path` (`bytes`) as the main unit: its main
    /// file under its own predefines, its lookups answered from its logs
    /// (a `.pcm` input, spec/modules.md §11.6). False after diagnosing a
    /// file that cannot be used; nothing is entered then.
    pub fn enter_main_capsule(&mut self, path: &str, bytes: &[u8]) -> bool {
        if self.modules.is_none() {
            // Modules are off before C++20, and a module file needs them.
            let d = Diag::new(DiagId::pp_module_file_config_mismatch, Span::INVALID).arg(path);
            self.emit(d);
            return false;
        }
        let Some(c) = self.load_capsule(path, bmi::read(bytes), None, Span::INVALID) else {
            return false;
        };
        let builtin = self.sm.add_virtual("<built-in>", c.predefines.clone()).ok();
        let Some(m) = &mut self.modules else { return false };
        m.replays.push(Replay::new(path.to_owned(), c, builtin));
        let i = m.replays.len() - 1;
        m.main_replay = Some(i);
        m.builtin_file = builtin;
        // A module file is not recorded again.
        m.record = None;
        let Some(file) = self.replay_file(i, 0) else {
            self.emit(Diag::new(DiagId::pp_module_file_not_found, Span::INVALID).arg(path));
            return false;
        };
        self.main_file = Some(file);
        self.push_file(file, None, None, None, false, false);
        if let Some(b) = builtin {
            self.push_file(b, None, None, None, false, false);
        }
        self.module_lines = has_module_lines(self.sm.text(file));
        true
    }

    /// The main unit's recorded inputs as a module file, when recording
    /// was requested ([`ModuleOptions::record`]) and the main unit is an
    /// importable module unit (spec/modules.md §11.2); `None` otherwise.
    /// Call once, after the unit was read to its end.
    pub fn take_capsule(&mut self) -> Option<Capsule> {
        let m = self.modules.as_mut()?;
        let rec = m.record.take()?;
        let (module, Some(kind)) = m.main_unit.clone()? else { return None };
        let predefines = m.builtin_file.map(|b| self.sm.text(b).to_owned()).unwrap_or_default();
        let files = rec
            .files
            .iter()
            .map(|&f| CapsuleFile {
                path: self.sm.file_name(f).to_owned(),
                text: self.sm.text(f).to_owned(),
            })
            .collect();
        Some(Capsule {
            version: bmi::KILN_VERSION.to_owned(),
            sha: bmi::kiln_sha().to_owned(),
            module,
            kind,
            predefines,
            fingerprint: m.opts.fingerprint.clone(),
            imports: rec.imports,
            files,
            includes: rec.includes,
            embeds: rec.embeds,
        })
    }

    /// The main unit's module declaration: its key (`M`, `M:P`) and, when
    /// it is importable, its kind. `None` before one was read or without
    /// one.
    pub fn main_module_unit(&self) -> Option<(String, Option<UnitKind>)> {
        self.modules.as_ref()?.main_unit.clone()
    }

    /// With [`ModuleOptions::scan`]: the named modules the main unit
    /// imports (`M`, `M:P`; an implementation unit's implicit import of
    /// its interface included), in order, without duplicates.
    pub fn scanned_imports(&self) -> &[String] {
        self.modules.as_ref().map_or(&[], |m| &m.scanned)
    }

    /// Whose lookups a request made now belongs to: that of the innermost
    /// named-module unit being read (header units belong to the unit that
    /// imports them).
    fn unit_ctx(&self) -> Ctx {
        let Some(m) = &self.modules else { return Ctx::Main };
        for (key, r) in m.building.iter().zip(&m.unit_replay).rev() {
            if key.starts_with('"') {
                continue;
            }
            return r.map_or(Ctx::Source, Ctx::Replay);
        }
        m.main_replay.map_or(Ctx::Main, Ctx::Replay)
    }

    /// The predefines buffer of the main unit or of a module file.
    pub(crate) fn is_builtin_file(&self, f: FileId) -> bool {
        self.modules.as_ref().is_some_and(|m| {
            m.builtin_file == Some(f) || m.replays.iter().any(|r| r.builtin == Some(f))
        })
    }

    /// Finds the header `req` names: from the module file whose unit is
    /// being read, else through the header search (recorded for the main
    /// unit's module file).
    pub(crate) fn resolve_include(&mut self, req: &IncludeRequest) -> Option<ResolvedInclude> {
        match self.unit_ctx() {
            Ctx::Replay(i) => self.replay_include(i, req),
            ctx => {
                let res = self.resolver.resolve(self.sm, req);
                if ctx == Ctx::Main
                    && let Some(rec) = self.modules.as_mut().and_then(|m| m.record.as_mut())
                {
                    let key = IncludeKey {
                        includer: rec.includer(req.includer),
                        name: req.name.clone(),
                        angled: req.angled,
                        kind: req.kind,
                        lookup_from: req.lookup_from,
                    };
                    if rec.include_keys.insert(key.clone()) {
                        let hit = res.as_ref().map(|r| IncludeHit {
                            file: rec.file(r.file),
                            lookup_index: r.lookup_index,
                            system: r.system,
                            extern_c: r.extern_c,
                            path: r.path.clone(),
                        });
                        rec.includes.push(IncludeEntry { key, hit });
                    }
                }
                res
            }
        }
    }

    /// [`resolve_include`](Self::resolve_include) for `#embed` /
    /// `__has_embed`.
    pub(crate) fn resolve_embed_req(&mut self, req: &EmbedRequest) -> Option<EmbedData> {
        match self.unit_ctx() {
            Ctx::Replay(i) => {
                let r = self.modules.as_ref()?.replays.get(i)?;
                let key = embed_key(r.includer(req.includer), req);
                let bytes = r.embeds.get(&key)?.clone()?;
                Some(EmbedData { bytes })
            }
            ctx => {
                let res = self.resolver.resolve_embed(self.sm, req);
                if ctx == Ctx::Main
                    && let Some(rec) = self.modules.as_mut().and_then(|m| m.record.as_mut())
                {
                    let key = embed_key(rec.includer(req.includer), req);
                    if rec.embed_keys.insert(key.clone()) {
                        let bytes = res.as_ref().map(|d| d.bytes.clone());
                        rec.embeds.push(EmbedEntry { key, bytes });
                    }
                }
                res
            }
        }
    }

    fn replay_include(&mut self, i: usize, req: &IncludeRequest) -> Option<ResolvedInclude> {
        let r = self.modules.as_ref()?.replays.get(i)?;
        let key = IncludeKey {
            includer: r.includer(req.includer),
            name: req.name.clone(),
            angled: req.angled,
            kind: req.kind,
            lookup_from: req.lookup_from,
        };
        let hit = r.includes.get(&key)?.clone()?;
        let file = self.replay_file(i, hit.file)?;
        Some(ResolvedInclude {
            file,
            lookup_index: hit.lookup_index,
            system: hit.system,
            extern_c: hit.extern_c,
            path: hit.path,
        })
    }

    /// The `FileId` of file `idx` of replay `i`, loaded on first use: the
    /// file on disk when it still has the recorded text (sharing its
    /// `FileId` with the importer, for §6 dedup), else the recorded text
    /// as a buffer of the recorded name.
    fn replay_file(&mut self, i: usize, idx: u32) -> Option<FileId> {
        let m = self.modules.as_mut()?;
        let r = m.replays.get_mut(i)?;
        let k = idx as usize;
        if let Some(id) = *r.ids.get(k)? {
            return Some(id);
        }
        let (path, text) = r.files.get_mut(k)?;
        let text = text.take()?;
        // The file itself when it still holds the recorded text (its
        // FileId is then shared with any textual inclusion of it, §6);
        // otherwise the recorded text under the file's name (§11.3).
        let id = match self.sm.load_regular_file_expecting(Path::new(path.as_str()), text) {
            Ok((id, None)) => id,
            Ok((id, Some(text))) if self.sm.text(id) == text => id,
            Ok((_, Some(text))) | Err((_, text)) => self.sm.add_virtual(path, text).ok()?,
        };
        r.ids[k] = Some(id);
        r.index.insert(id, idx);
        Some(id)
    }

    /// Imports the header unit `key` (resolved by `read_header_import`).
    fn import_header(&mut self, key: String, span: Span) {
        let Some(&(file, system)) = self.modules.as_ref().and_then(|m| m.header_files.get(&key))
        else {
            return;
        };
        if let Some(defs) = self.modules.as_ref().and_then(|m| m.built.get(&key)) {
            let defs = defs.clone();
            self.apply_header_macros(&defs);
            return;
        }
        if self.already_built(&key, span) {
            return;
        }
        self.begin_unit(key, true, file, system, span, None);
    }

    /// True if `key` is built (nothing to do) or being built (a cycle,
    /// diagnosed).
    fn already_built(&mut self, key: &str, span: Span) -> bool {
        let Some(m) = &self.modules else { return true };
        if m.built.contains_key(key) {
            return true;
        }
        if let Some(i) = m.building.iter().position(|b| b == key) {
            let mut chain: Vec<&str> = m.building[i..].iter().map(String::as_str).collect();
            chain.push(key);
            let d = Diag::new(DiagId::pp_module_cycle, span).arg(key).arg(chain.join(" -> "));
            self.emit(d);
            return true;
        }
        false
    }

    /// Starts reading unit `key` from `file`: saves the importer's state,
    /// then pushes the file and the predefines over it. `replay` is the
    /// module file it is read from; the predefines are its own then, those
    /// of the importing unit for a header unit, and the main unit's
    /// otherwise.
    fn begin_unit(
        &mut self,
        key: String,
        header: bool,
        file: FileId,
        system: bool,
        span: Span,
        replay: Option<usize>,
    ) {
        let ctx = self.unit_ctx();
        let Some(m) = &mut self.modules else { return };
        let builtin = match (replay, ctx) {
            (Some(i), _) => m.replays.get(i).and_then(|r| r.builtin),
            (None, Ctx::Replay(i)) if header => m.replays.get(i).and_then(|r| r.builtin),
            _ => m.builtin_file,
        };
        m.building.push(key.clone());
        m.unit_replay.push(replay);
        let module = m.module.take();
        let save = UnitSave {
            key: key.clone(),
            header,
            macros: std::mem::take(&mut self.macros),
            once_files: std::mem::take(&mut self.once_files),
            guards: std::mem::take(&mut self.guards),
            poisoned: std::mem::take(&mut self.poisoned),
            pushed: std::mem::take(&mut self.pushed),
            if_ndef: self.if_ndef.take(),
            include_depth: std::mem::replace(&mut self.include_depth, 0),
            first_hash: self.unit_first_hash.take(),
            pre_directive: std::mem::take(&mut self.unit_pre_directive),
            module,
        };
        self.define_builtins();
        self.queue(Marker::UnitBegin { key, header, span });
        self.push_file(file, Some(span.lo), None, None, system, false);
        if let Some(Frame::File(f)) = self.frames.last_mut() {
            f.unit = Some(Box::new(save));
        }
        if let Some(b) = builtin {
            self.push_file(b, None, None, None, false, false);
        }
    }

    /// The file of a unit ended: restores the importer's state and
    /// queues [`Marker::UnitEnd`].
    pub(crate) fn end_unit(&mut self, save: Box<UnitSave>) {
        let save = *save;
        let exported: Vec<Rc<MacroDef>> = if save.header {
            let mut v: Vec<Rc<MacroDef>> = self
                .macros
                .map
                .values()
                .filter(|d| {
                    d.builtin.is_none()
                        && self
                            .sm
                            .offset_of(d.name_span.lo)
                            .is_some_and(|(f, _)| !self.is_builtin_file(f))
                })
                .cloned()
                .collect();
            v.sort_by_key(|d| d.name_span.lo);
            v
        } else {
            Vec::new()
        };
        self.macros = save.macros;
        self.once_files = save.once_files;
        self.guards = save.guards;
        self.poisoned = save.poisoned;
        self.pushed = save.pushed;
        self.if_ndef = save.if_ndef;
        self.include_depth = save.include_depth;
        self.unit_first_hash = save.first_hash;
        self.unit_pre_directive = save.pre_directive;
        if let Some(m) = &mut self.modules {
            m.module = save.module;
            m.building.pop();
            m.unit_replay.pop();
            m.built.insert(save.key.clone(), exported.clone());
        }
        self.apply_header_macros(&exported);
        self.queue(Marker::UnitEnd { key: save.key });
    }

    /// [cpp.import]/5: the macros of a header unit become visible.
    fn apply_header_macros(&mut self, defs: &[Rc<MacroDef>]) {
        for d in defs {
            self.macros.map.insert(d.name, d.clone());
        }
    }
}

fn embed_key(includer: u32, req: &EmbedRequest) -> EmbedKey {
    EmbedKey {
        includer,
        name: req.name.clone(),
        angled: req.angled,
        limit: req.limit,
        offset: req.offset,
        query_only: req.query_only,
    }
}
