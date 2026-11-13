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
use crate::callbacks::ModuleFileRead;
use crate::directive::Dir;
use crate::include::{EmbedData, EmbedRequest, IncludeKind, IncludeRequest, ResolvedInclude};
use crate::macros::{MacroDef, MacroTable, SymMap};
use crate::preprocessor::{Frame, Marker, Preprocessor};

/// Where module interfaces are found (spec/modules.md §2, §11).
#[derive(Clone, Debug, Default)]
#[allow(clippy::struct_excessive_bools, reason = "independent command-line switches")]
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
    /// The hidden `-fkiln-bmi-image`: build an imported unit from its
    /// module file's declaration image when it has one and the name gate
    /// passes (spec/modules.md §11.11 "Part 2a"); otherwise replay.
    pub image: bool,
    /// The toolchain's module-file cache for libc++'s `std` and
    /// `std.compat` (spec/modules.md §11.12): asked wherever one of them
    /// would otherwise be built from its source in `std_dir`.
    pub std_cache: Option<StdModuleCache>,
    /// A header unit's exported macros are made visible as they are,
    /// not through a write and read of the preprocessor-state record
    /// (spec/modules.md §11.13 "Header units"). Off by default.
    pub no_unit_record: bool,
}

/// A lookup in the toolchain's module-file cache (spec/modules.md
/// §11.12): given `std` or `std.compat` and its libc++ source, the module
/// file to read instead (built and stored on a miss), or `None` to build
/// the unit from its source in-process (part A).
#[derive(Clone)]
pub struct StdModuleCache(pub Rc<StdModuleLookup>);

/// The function behind a [`StdModuleCache`]: module name and libc++
/// source to the module file to read.
pub type StdModuleLookup = dyn Fn(&str, &Path) -> Option<StdModuleFile>;

/// A module file the [`StdModuleCache`] gives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StdModuleFile {
    /// The module file (absolute).
    pub path: PathBuf,
    /// The files its build read, its libc++ source first: the cache
    /// entry's stamp, so the same on a hit and on a miss (spec/driver.md
    /// "Dependency output").
    pub inputs: Vec<String>,
}

/// Where [`Preprocessor::find_module`] found a module.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Found {
    /// `-fmodule-file=<name>=<path>`.
    ModuleFile,
    /// A `-fprebuilt-module-path` directory.
    Prebuilt,
    /// The path a module file read so far recorded for its import.
    Recorded,
    /// libc++'s module directory (`std`, `std.compat`).
    StdDir,
}

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
    /// The offset just past the newline ending the directive, for one of
    /// the main unit's own main file outside any `#include` and macro
    /// expansion (the preamble of a declaration image, spec/modules.md
    /// §11.11 "Imports, partitions and using-declarations"); `None`
    /// otherwise.
    pub end: Option<u32>,
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

/// An imported unit whose image takes over after its preamble (S6,
/// spec/modules.md §11.11 "Imports, partitions and using-declarations"):
/// what [`Preprocessor::image_stop_reached`] needs at the preamble's end.
pub(crate) struct ImageStop {
    key: String,
    path: String,
    bytes: Vec<u8>,
    preamble: u32,
    gated: Vec<String>,
    span: Span,
}

/// How an imported module file that holds a declaration image was read,
/// with the image enabled (spec/modules.md §11.11 "Part 2a"): the
/// driver's `-v` notes ([`Preprocessor::image_imports`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageImport {
    /// The unit's key (`M`, `M:P`).
    pub key: String,
    /// `None` when the unit was built from its image; else why it was
    /// replayed.
    pub replayed: Option<String>,
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
    /// The unit is read from a module file's record
    /// ([`PpCallbacks::module_unit`](crate::PpCallbacks::module_unit)).
    from_module_file: bool,
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
    /// The carried constants of the main unit's module file (a `.pcm`
    /// input of kind 2), until the parser takes them.
    main_carried: Option<Rc<bmi::CarriedSection>>,
    /// The main unit's module declaration: its key and, when it is
    /// importable, its kind.
    main_unit: Option<(String, Option<UnitKind>)>,
    /// With [`ModuleOptions::scan`]: the main unit's named-module imports
    /// (`M`, `M:P`), in order, without duplicates.
    scanned: Vec<String>,
    /// The imports that met the image gate, in order.
    image_log: Vec<ImageImport>,
}

impl ModState {
    pub(crate) fn new(opts: ModuleOptions) -> ModState {
        let record = opts.record.then(Box::default);
        ModState { opts, record, ..ModState::default() }
    }

    /// The first field that keeps a snapshot from being taken (spec/pp.md
    /// "Snapshots"): only `opts` (configuration) and `builtin_file`
    /// (state) are carried, so every other field must be as a fresh
    /// state has it: no unit read, built, imported or recorded.
    pub(crate) fn snap_unready(&self) -> Option<&'static str> {
        let ModState {
            opts: _,
            building,
            built,
            module,
            builtin_file: _,
            header_files,
            record,
            replays,
            unit_replay,
            main_replay,
            main_carried,
            main_unit,
            scanned,
            image_log,
        } = self;
        let field = if !building.is_empty() || !unit_replay.is_empty() {
            "building: inside an imported unit"
        } else if !built.is_empty() || !header_files.is_empty() || !image_log.is_empty() {
            "built: a unit was imported"
        } else if module.is_some() || main_unit.is_some() {
            "module: a module declaration was read"
        } else if record.is_some() {
            "record: a module file is being recorded"
        } else if !replays.is_empty() || main_replay.is_some() || main_carried.is_some() {
            "replays: a module file was replayed"
        } else if !scanned.is_empty() {
            "scanned: an import was scanned"
        } else {
            return None;
        };
        Some(field)
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
        let mut d =
            ModuleDirective { span: t.span, export, kw: kw_span, kind, after_directive, end: None };
        if matches!(d.kind, DirectiveKind::Invalid) {
            self.finish_directive();
        } else {
            self.end_directive_at_semi();
        }
        d.end = self.main_directive_end();
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

    /// The offset just past the newline ending the directive just read,
    /// when it is the main unit's own, in its main file at include depth
    /// 0 with no token lexed ahead ([`ModuleDirective::end`]).
    fn main_directive_end(&self) -> Option<u32> {
        let m = self.modules.as_ref()?;
        if !m.building.is_empty() || self.include_depth != 0 {
            return None;
        }
        let Some(Frame::File(f)) = self.frames.last() else { return None };
        if Some(f.file) != self.main_file || !f.back.is_empty() {
            return None;
        }
        // The directive ends after its `;`; the rest of its line may hold
        // blanks and a `//` comment. Anything else on the line (`import M;
        // int x;`) leaves the end unknown, so the image falls back.
        let off = f.lx.offset() as usize;
        let text = self.sm.text(f.file).as_bytes();
        let mut i = off + text.get(off..)?.iter().take_while(|&&c| c == b' ' || c == b'\t').count();
        if text.get(i..).is_some_and(|r| r.starts_with(b"//")) {
            i += text[i..].iter().take_while(|&&c| c != b'\n' && c != b'\r').count();
        }
        let rest = &text[i..];
        let end = if rest.starts_with(b"\r\n") {
            i + 2
        } else if rest.starts_with(b"\n") || rest.starts_with(b"\r") {
            i + 1
        } else if rest.is_empty() {
            i
        } else {
            return None;
        };
        u32::try_from(end).ok()
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

    /// `M.N` starting at `t`; leaves the token after it in `t`. The
    /// components are no tokens the parser sees: the image gate does not
    /// count them as seen (spec/modules.md §11.11, "ODR merging").
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
        self.find_module_where(name).map(|(p, _)| p)
    }

    /// [`find_module`](Self::find_module), with where it was found.
    fn find_module_where(&self, name: &str) -> Option<(PathBuf, Found)> {
        let m = self.modules.as_ref()?;
        let o = &m.opts;
        if let Some((_, p)) = o.module_files.iter().rev().find(|(n, _)| n == name) {
            return Some((p.clone(), Found::ModuleFile));
        }
        let stem = name.replace(':', "-");
        for dir in &o.prebuilt_paths {
            for ext in ["pcm", "cppm", "ixx", "cpp"] {
                let p = dir.join(format!("{stem}.{ext}"));
                if p.is_file() {
                    return Some((p, Found::Prebuilt));
                }
            }
        }
        // The paths the module files read so far recorded for their
        // imports (clang reads the module files a BMI names).
        for r in m.replays.iter().rev() {
            if let Some((_, p)) = r.imports.iter().find(|(n, _)| n == name) {
                return Some((PathBuf::from(p), Found::Recorded));
            }
        }
        if matches!(name, "std" | "std.compat")
            && let Some(d) = &o.std_dir
        {
            let p = d.join(format!("{name}.cppm"));
            if p.is_file() {
                return Some((p, Found::StdDir));
            }
        }
        None
    }

    /// The module file the toolchain cache gives for `std` or
    /// `std.compat` when `path` is libc++'s own source of it
    /// (spec/modules.md §11.12), else `path`. `std.compat` imports `std`,
    /// so it is taken from the cache only when `std` is libc++'s too.
    fn cached_std(&self, key: &str, path: &Path) -> Option<StdModuleFile> {
        let m = self.modules.as_ref()?;
        let (Some(cache), Some(dir)) = (&m.opts.std_cache, &m.opts.std_dir) else { return None };
        let source = |name: &str| dir.join(format!("{name}.cppm"));
        if !matches!(key, "std" | "std.compat") || path != source(key) {
            return None;
        }
        if key == "std.compat" && self.find_module("std") != Some(source("std")) {
            return None;
        }
        let cache = Rc::clone(&cache.0);
        cache(key, path)
    }

    /// Imports named module (or partition) `key`: from a module file
    /// (spec/modules.md §11) or from its source (§1).
    fn import_module(&mut self, key: &str, span: Span) {
        if self.already_built(key, span) {
            return;
        }
        let Some((source, found)) = self.find_module_where(key) else {
            self.emit(Diag::new(DiagId::pp_module_not_found, span).arg(key));
            return;
        };
        // A module file records the path it found, never the cache's: an
        // importer of it looks the cache up again (§11.12).
        if self.unit_ctx() == Ctx::Main
            && let Some(rec) = self.modules.as_mut().and_then(|m| m.record.as_mut())
            && !rec.imports.iter().any(|(k, _)| k == key)
        {
            rec.imports.push((key.to_owned(), source.display().to_string()));
        }
        let cached = self.cached_std(key, &source);
        let path = cached.as_ref().map_or(source, |c| c.path.clone());
        let shown = path.display().to_string();
        match bmi::sniff(&path) {
            bmi::Sniffed::Kiln => {
                // Named in a dependency file as clang names it: a
                // prebuilt-path find joined onto the working directory
                // as spelled, `./` kept (clang `getPrebuiltModuleFileName`
                // then `makeAbsolute`), the cache's already absolute.
                let dep_path = match found {
                    Found::Prebuilt if path.is_relative() => {
                        std::env::current_dir().map_or_else(|_| path.clone(), |cwd| cwd.join(&path))
                    }
                    _ => path.clone(),
                };
                self.callbacks.module_file_read(&ModuleFileRead {
                    path: &dep_path.display().to_string(),
                    module: key,
                    cache_inputs: cached.as_ref().map(|c| c.inputs.as_slice()),
                });
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
        if let Some(c) = self.try_image_unit(&key, &path, c, span) {
            self.replay_capsule_unit(key, path, c, span);
        }
    }

    /// Builds unit `key` from its module file's declaration image when
    /// `-fkiln-bmi-image` is on, the capsule has one and none of its
    /// declared names has been seen yet (spec/modules.md §11.11 "Part
    /// 2a", "ODR merging"): queues [`Marker::UnitBegin`] carrying the
    /// image and [`Marker::UnitEnd`], lexes nothing, and returns `None`.
    /// Otherwise returns the capsule to replay, without its image section.
    fn try_image_unit(
        &mut self,
        key: &str,
        path: &str,
        mut c: Capsule,
        span: Span,
    ) -> Option<Capsule> {
        let take = |c: &mut Capsule| {
            let i = c.sections.iter().position(|s| s.tag == bmi::SECTION_IMAGE)?;
            Some(c.sections.remove(i).bytes)
        };
        let on = self.modules.as_ref().is_some_and(|m| m.opts.image);
        let Some(bytes) = take(&mut c).filter(|_| on) else {
            return Some(c);
        };
        if c.files.is_empty() {
            self.log_image(key, Some("it has no main file".to_owned()));
            return Some(c);
        }
        // An image names its unit's main file; one that is not the
        // capsule's first file, or that declares a global-scope name the
        // importer may have declared there (§11.11 "Part 2a", ODR
        // merging), replays. A header that does not decode is used
        // anyway: sema then reports the file as corrupt ("The reader").
        if let Some(h) = bmi::image_header(&bytes) {
            if c.files[0].path != h.main {
                self.log_image(key, Some("its image names another main file".to_owned()));
                return Some(c);
            }
            if let Some(n) = h.gated.iter().find(|n| self.name_seen(n)) {
                self.log_image(key, Some(format!("its image declares '{n}', already seen")));
                return Some(c);
            }
            // S6: the unit's module and import directives replay first,
            // up to the preamble's end, where the image takes over
            // (§11.11 "Imports, partitions and using-declarations").
            if let Some(p) = h.preamble {
                let text = &c.files[0].text;
                if p as usize > text.len() || !text.is_char_boundary(p as usize) {
                    self.log_image(key, Some("its image has a bad preamble".to_owned()));
                    return Some(c);
                }
                let gated: Vec<String> = h.gated.iter().map(|&n| n.to_owned()).collect();
                let stop = ImageStop {
                    key: key.to_owned(),
                    path: path.to_owned(),
                    bytes,
                    preamble: p,
                    gated,
                    span,
                };
                self.replay_capsule_unit(key.to_owned(), path.to_owned(), c, span);
                self.set_image_stop(stop);
                return None;
            }
            if !c.imports.is_empty() {
                self.log_image(key, Some("it imports other units".to_owned()));
                return Some(c);
            }
            self.see_gated(&h.gated);
        } else if !c.imports.is_empty() {
            self.log_image(key, Some("it imports other units".to_owned()));
            return Some(c);
        }
        let (main_path, main_text) = (c.files[0].path.clone(), c.files[0].text.clone());
        let Some(main_file) = self.load_recorded(&main_path, main_text) else {
            self.emit(Diag::new(DiagId::pp_module_file_not_found, span).arg(path));
            return None;
        };
        // S7: the image's other files, without reading them; one that
        // changed replays (§11.11 "File table").
        let files = match bmi::image_header(&bytes) {
            Some(h) => match self.image_files(path, &h.files, main_file) {
                Ok(files) => files,
                Err(why) => {
                    self.log_image(key, Some(why));
                    return Some(c);
                }
            },
            None => vec![main_file],
        };
        self.log_image(key, None);
        let carried = take_carried(path, &mut c);
        drop(c);
        let image = Rc::new(bmi::ImageSection { path: path.to_owned(), bytes, files });
        let Some(m) = &mut self.modules else { return None };
        m.built.insert(key.to_owned(), Vec::new());
        self.queue(Marker::UnitBegin {
            key: key.to_owned(),
            header: false,
            span,
            carried,
            image: Some(image),
        });
        self.queue(Marker::UnitEnd { key: key.to_owned(), image: None });
        None
    }

    /// Marks `names` (an image's gated names) seen from now on, so that a
    /// later import in the same lookahead whose image declares one of them
    /// replays instead.
    fn see_gated<S: AsRef<str>>(&mut self, names: &[S]) {
        for n in names {
            let s = self.interner.intern(n.as_ref());
            if let Some(g) = &mut self.name_gate {
                g.see(s);
            }
        }
    }

    /// Cuts the main file of the unit [`Self::replay_capsule_unit`] just
    /// entered at `stop.preamble` (S6): its lexer reads the preamble only,
    /// and [`Self::image_stop_reached`] decides at its end.
    fn set_image_stop(&mut self, stop: ImageStop) {
        let interner = self.interner;
        let opts = self.lex_opts;
        let sm = self.sm;
        let record = self.comment_handler.is_some();
        let unit_file = self.frames.iter_mut().rev().find_map(|f| match f {
            Frame::File(f) if f.unit.is_some() => Some(f),
            _ => None,
        });
        let Some(f) = unit_file else { return };
        f.lx = kiln_lex::Lexer::new_in_range(sm, interner, f.file, 0..stop.preamble, f.base, opts);
        if record {
            f.lx.set_record_comments(true);
        }
        f.image_stop = Some(Box::new(stop));
    }

    /// The main file of a unit cut by [`Self::set_image_stop`] reached the
    /// preamble's end (the top frame). With no conditional open and none of
    /// the image's gated names seen meanwhile (the preamble's imports may
    /// have declared one), the unit ends here and its image is built:
    /// returns true. Otherwise the lexer goes on through the rest of the
    /// file, and the unit replays.
    pub(crate) fn image_stop_reached(&mut self) -> bool {
        let Some(Frame::File(f)) = self.frames.last_mut() else { return false };
        let Some(stop) = f.image_stop.take() else { return false };
        let open = !f.conds.is_empty();
        let main = f.file;
        let mut why = if open {
            Some("its preamble ends in a conditional".to_owned())
        } else {
            stop.gated
                .iter()
                .find(|n| self.name_seen(n))
                .map(|n| format!("its image declares '{n}', already seen"))
        };
        let mut files = vec![main];
        if why.is_none()
            && let Some(h) = bmi::image_header(&stop.bytes)
        {
            match self.image_files(&stop.path, &h.files, main) {
                Ok(v) => files = v,
                Err(w) => why = Some(w),
            }
        }
        if let Some(why) = why {
            self.log_image(&stop.key, Some(why));
            let interner = self.interner;
            let opts = self.lex_opts;
            let sm = self.sm;
            let record = self.comment_handler.is_some();
            let Some(Frame::File(f)) = self.frames.last_mut() else { return false };
            let len = sm.text(f.file).len() as u32;
            f.lx = kiln_lex::Lexer::new_in_range(
                sm,
                interner,
                f.file,
                stop.preamble..len,
                f.base,
                opts,
            );
            if record {
                f.lx.set_record_comments(true);
            }
            return false;
        }
        self.see_gated(&stop.gated);
        self.log_image(&stop.key, None);
        let Some(Frame::File(f)) = self.frames.last_mut() else { return false };
        let image = Rc::new(bmi::ImageSection { path: stop.path, bytes: stop.bytes, files });
        f.image_done = Some((image, stop.span));
        true
    }

    /// Whether the importer may have declared the name `n` at global
    /// namespace scope (spec/modules.md §11.11, "ODR merging"): an
    /// identifier token `n` reached the parser there
    /// ([`crate::name_gate::NameGate`]).
    fn name_seen(&self, n: &str) -> bool {
        let Some(g) = &self.name_gate else { return true };
        self.interner.get(n).is_some_and(|s| g.seen(s))
    }

    /// S7: the buffers of the file table `files` of the image of module
    /// file `path`, whose entry 0 is the unit's main file, loaded as
    /// `main` (spec/modules.md §11.11 "File table"). An in-memory entry
    /// becomes a buffer with its text; a file the importer has loaded must
    /// hold the recorded text; any other becomes a lazy buffer, read only
    /// when a diagnostic needs it, with the module file's copy as its
    /// fallback. `Err` is why the unit replays instead: a file that is
    /// missing or changed size, or a loaded one with other text.
    fn image_files(
        &self,
        path: &str,
        files: &[bmi::ImageFile<'_>],
        main: FileId,
    ) -> Result<Vec<FileId>, String> {
        use kiln_source::{LazyCopy, LazyFile};
        let changed = |n: &str| format!("'{n}' changed since the module file was built");
        let mut ids = Vec::with_capacity(files.len());
        for (i, f) in files.iter().enumerate() {
            if i == 0 {
                // The unit's own text is the module file's recorded copy,
                // as replay shows it.
                ids.push(main);
                continue;
            }
            let id = if let Some(t) = f.text {
                self.sm.add_virtual(f.name, t.to_owned()).map_err(|e| e.to_string())?
            } else {
                let (mp, name) = (path.to_owned(), f.name.to_owned());
                let copy: LazyCopy = Box::new(move || {
                    let c = bmi::read_file(Path::new(&mp)).ok()?;
                    c.files.into_iter().find(|c| c.path == name).map(|c| c.text)
                });
                match self.sm.add_lazy_file(f.name, Path::new(f.path), f.len, f.hash, Some(copy)) {
                    LazyFile::Added(id) => id,
                    LazyFile::Loaded(id) if self.sm.same_content(id, f.len, f.hash) => id,
                    LazyFile::Loaded(_) | LazyFile::Changed => return Err(changed(f.name)),
                    LazyFile::Missing => return Err(format!("'{}' is missing", f.name)),
                    LazyFile::Exhausted => {
                        return Err(kiln_source::LocSpaceExhausted.to_string());
                    }
                }
            };
            ids.push(id);
        }
        Ok(ids)
    }

    fn log_image(&mut self, key: &str, replayed: Option<String>) {
        if let Some(m) = &mut self.modules {
            m.image_log.push(ImageImport { key: key.to_owned(), replayed });
        }
    }

    /// The imports so far whose module file holds a declaration image,
    /// read with the image enabled: built from it, or replayed and why
    /// (spec/modules.md §11.11 "Part 2a").
    pub fn image_imports(&self) -> &[ImageImport] {
        self.modules.as_ref().map_or(&[], |m| &m.image_log)
    }

    /// Starts replaying unit `key` from the capsule `c` of module file
    /// `path` (§11.2).
    fn replay_capsule_unit(&mut self, key: String, path: String, mut c: Capsule, span: Span) {
        let builtin = self.sm.add_virtual("<built-in>", c.predefines.clone()).ok();
        let carried = take_carried(&path, &mut c);
        let Some(m) = &mut self.modules else { return };
        m.replays.push(Replay::new(path.clone(), c, builtin));
        let i = m.replays.len() - 1;
        let Some(file) = self.replay_file(i, 0) else {
            self.emit(Diag::new(DiagId::pp_module_file_not_found, span).arg(path));
            return;
        };
        self.begin_unit(key, false, file, false, span, Some(i));
        if let Some(c) = carried {
            self.set_unit_carried(c);
        }
    }

    /// Attaches `c` to the [`Marker::UnitBegin`] just queued.
    fn set_unit_carried(&mut self, c: Rc<bmi::CarriedSection>) {
        let last = self.markers.iter_mut().rev().find(|m| matches!(m, Marker::UnitBegin { .. }));
        if let Some(Marker::UnitBegin { carried, .. }) = last {
            *carried = Some(c);
        }
    }

    /// The carried constants of a `.pcm` main input of kind 2
    /// (spec/modules.md §11.11), once.
    pub fn take_main_carried(&mut self) -> Option<Rc<bmi::CarriedSection>> {
        self.modules.as_mut()?.main_carried.take()
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
        let Some(mut c) = self.load_capsule(path, bmi::read(bytes), None, Span::INVALID) else {
            return false;
        };
        let builtin = self.sm.add_virtual("<built-in>", c.predefines.clone()).ok();
        let carried = take_carried(path, &mut c);
        let Some(m) = &mut self.modules else { return false };
        m.main_carried = carried;
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
            sections: Vec::new(),
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
        let path = path.clone();
        let id = self.load_recorded(&path, text)?;
        let r = self.modules.as_mut()?.replays.get_mut(i)?;
        r.ids[k] = Some(id);
        r.index.insert(id, idx);
        Some(id)
    }

    /// Loads a capsule file `path` recorded with `text`: the file itself
    /// when it still holds the recorded text (its `FileId` is then shared
    /// with any textual inclusion of it, §6); otherwise the recorded text
    /// under the file's name (§11.3).
    fn load_recorded(&mut self, path: &str, text: String) -> Option<FileId> {
        match self.sm.load_regular_file_expecting(Path::new(path), text) {
            Ok((id, None)) => Some(id),
            Ok((id, Some(text))) if self.sm.text(id) == text => Some(id),
            Ok((_, Some(text))) | Err((_, text)) => self.sm.add_virtual(path, text).ok(),
        }
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
        // A header unit imported by a unit read from an imported module
        // file is found through that file's record too (a `.pcm` main
        // input's own files stay its inputs).
        let from_module_file = replay.is_some()
            || (header && matches!(ctx, Ctx::Replay(i) if m.main_replay != Some(i)));
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
            from_module_file,
        };
        self.callbacks.module_unit(true, from_module_file);
        self.define_builtins();
        self.queue(Marker::UnitBegin { key, header, span, carried: None, image: None });
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
    pub(crate) fn end_unit(
        &mut self,
        unit: FileId,
        save: Box<UnitSave>,
        image: Option<(Rc<bmi::ImageSection>, Span)>,
    ) {
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
            // spec/modules.md §11.13 "Header units": the export goes
            // through the record; a unit it cannot carry keeps its
            // in-process definitions.
            if self.modules.as_ref().is_some_and(|m| !m.opts.no_unit_record)
                && let Some(back) = self.unit_round_trip(unit, &v)
            {
                v = back;
            }
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
        self.callbacks.module_unit(false, save.from_module_file);
        self.queue(Marker::UnitEnd { key: save.key, image });
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

/// Moves the carried-constants section out of the kind-2 capsule `c`
/// read from `path` (spec/modules.md §11.11).
fn take_carried(path: &str, c: &mut Capsule) -> Option<Rc<bmi::CarriedSection>> {
    let i = c.sections.iter().position(|s| s.tag == bmi::SECTION_CONSTS)?;
    let s = c.sections.swap_remove(i);
    Some(Rc::new(bmi::CarriedSection { path: path.to_owned(), bytes: s.bytes }))
}
