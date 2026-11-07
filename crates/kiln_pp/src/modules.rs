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

use crate::directive::Dir;
use crate::include::{IncludeKind, IncludeRequest};
use crate::macros::{MacroDef, MacroTable, SymMap};
use crate::preprocessor::{Frame, Marker, Preprocessor};

/// Where module interface sources are found (spec/modules.md §2).
#[derive(Clone, Debug, Default)]
pub struct ModuleOptions {
    /// `-fmodule-file=<name>=<path>`: the source of module `<name>`
    /// (`M:P` for a partition).
    pub module_files: Vec<(String, PathBuf)>,
    /// `-fprebuilt-module-path=<dir>`, in order: `<dir>/<name>.cppm`,
    /// `.ixx`, `.cpp` (`M-P.*` for partition `M:P`).
    pub prebuilt_paths: Vec<PathBuf>,
    /// The libc++ module source directory (`<llvm>/share/libc++/v1`),
    /// holding `std.cppm` and `std.compat.cppm`.
    pub std_dir: Option<PathBuf>,
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
}

impl ModState {
    pub(crate) fn new(opts: ModuleOptions) -> ModState {
        ModState { opts, ..ModState::default() }
    }
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
            DirectiveKind::Import { header: true, key, name_span, .. } => {
                Some((key.clone(), *name_span))
            }
            _ => None,
        };
        self.queue(Marker::Module { span: t.span, export, import, directive: Some(d) });
        if let Some((key, span)) = perform {
            self.import_module(&key, span);
        }
        if let Some((key, span)) = header {
            self.import_header(key, span);
        }
    }

    /// A directive starting at `at` in the current file, at include depth
    /// 0: records the unit's first directive and whether one that may not
    /// precede a module directive was seen (`exempt`: a null directive or
    /// a line marker).
    pub(crate) fn note_unit_directive(&mut self, at: Loc, dir: Option<Dir>, exempt: bool) {
        let builtin = self.modules.as_ref().and_then(|m| m.builtin_file);
        if self.cur_file_ref().is_none_or(|f| Some(f.file) == builtin) {
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
        let res = self.resolver.resolve(self.sm, &req);
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

    /// The interface source of module `name` (spec/modules.md §2).
    fn find_module(&self, name: &str) -> Option<PathBuf> {
        let o = &self.modules.as_ref()?.opts;
        if let Some((_, p)) = o.module_files.iter().rev().find(|(n, _)| n == name) {
            return Some(p.clone());
        }
        let stem = name.replace(':', "-");
        for dir in &o.prebuilt_paths {
            for ext in ["cppm", "ixx", "cpp"] {
                let p = dir.join(format!("{stem}.{ext}"));
                if p.is_file() {
                    return Some(p);
                }
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

    /// Imports named module (or partition) `key`.
    fn import_module(&mut self, key: &str, span: Span) {
        if self.already_built(key, span) {
            return;
        }
        let Some(path) = self.find_module(key) else {
            self.emit(Diag::new(DiagId::pp_module_not_found, span).arg(key));
            return;
        };
        if let Ok(file) = self.sm.load_file(Path::new(&path)) {
            self.begin_unit(key.to_string(), false, file, false, span);
        } else {
            let p = path.display().to_string();
            self.emit(Diag::new(DiagId::pp_file_not_found, span).arg(p));
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
        self.begin_unit(key, true, file, system, span);
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
    /// then pushes the file and the predefines over it.
    fn begin_unit(&mut self, key: String, header: bool, file: FileId, system: bool, span: Span) {
        let Some(m) = &mut self.modules else { return };
        m.building.push(key.clone());
        let builtin = m.builtin_file;
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
        let builtin = self.modules.as_ref().and_then(|m| m.builtin_file);
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
                            .is_some_and(|(f, _)| Some(f) != builtin)
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
