//! C++20 modules, part A (spec/modules.md): the module preamble and unit
//! kinds ([module.unit], [module.global.frag], [module.private.frag],
//! [module.import]), export declarations ([module.interface]), and the
//! ownership / visibility rules ([module.context], [module.reach],
//! [basic.link]/8-11) consulted by name lookup, ADL and redeclaration
//! lookup. The data lives in [`kiln_ast::modules::ModuleTable`]; this
//! module keeps the per-unit parse state.

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclFlags, DeclKind, FnStorage, VarStorage};
use kiln_ast::modules::{Owner, UnitInfo, UnitKind};
use kiln_ast::names::DeclName;
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::fx::FxHashMap;

use crate::Sema;
use crate::lookup::{Found, LookupKind};
use crate::scope::{Scope, ScopeKind};

/// A module or import directive, as the preprocessor parsed it
/// (`kiln_pp::DirectiveKind`, converted by the parser).
#[derive(Clone, Copy, Debug)]
pub enum Directive<'s> {
    /// `module;`.
    GlobalFragment,
    /// `[export] module M[:P];`.
    Module {
        /// The dotted module name.
        name: &'s str,
        /// The partition.
        partition: Option<&'s str>,
        /// The `module` keyword's location (clang reports preamble
        /// errors there).
        kw_loc: Loc,
    },
    /// `module :private;`, with the `private` token's location.
    Private(Loc),
    /// `[export] import ...;`.
    Import {
        /// The unit key (spec/modules.md §3).
        key: &'s str,
        /// The name as written.
        display: &'s str,
    },
}

/// Where the current unit is in its preamble ([module.unit]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Phase {
    /// Before any module directive.
    #[default]
    Start,
    /// In the global module fragment (`module;`).
    Gmf,
    /// In the module purview.
    Purview,
    /// In the private module fragment (`module :private;`).
    Private,
}

/// The parse state of one unit being read.
#[derive(Clone, Debug, Default)]
struct Frame {
    unit: u16,
    phase: Phase,
    /// The first token (`export` / `module`) of the module declaration.
    module_kw: Loc,
    /// The module declaration's `module` keyword.
    module_loc: Loc,
    /// The `private` of the private module fragment.
    private_loc: Loc,
    /// The first declaration of the unit (any phase).
    first_decl: Loc,
    /// A declaration was seen in the current purview / private fragment.
    decl_seen: bool,
    /// Open export declarations: (decl, `export` location).
    exports: Vec<(DeclId, Loc)>,
}

/// Module state of [`Sema`].
#[derive(Debug, Default)]
pub(crate) struct ModSema {
    /// The units being read, innermost last; `frames[0]` is the main file.
    frames: Vec<Frame>,
    /// Unit index by key.
    keys: FxHashMap<String, u16>,
    /// Per unit: visible (imported, directly or transitively) from the
    /// current unit.
    visible: Vec<bool>,
    /// Per unit: a unit of the current unit's named module.
    same: Vec<bool>,
    /// The `{` of each unnamed namespace opened while modules are active
    /// (clang reports unnamed-namespace export errors there).
    anon_braces: FxHashMap<DeclId, Loc>,
    /// Set while a failed lookup is redone ignoring visibility, to
    /// diagnose a missing import ([`Sema::module_hidden_lookup`]).
    see_hidden: std::cell::Cell<bool>,
}

impl ModSema {
    fn cur(&self) -> u16 {
        self.frames.last().map_or(0, |f| f.unit)
    }
}

/// True for a function or variable declared `static` at namespace scope,
/// or any declaration in an unnamed namespace ([basic.link]/3).
fn internal(s: &Sema<'_, '_>, d: DeclId) -> bool {
    let ast = &s.ast;
    let first = ast.first(d);
    let n = ast.decls.get(first);
    let by_storage = match n.kind() {
        DeclKind::Function => {
            ast.decls.function(first).is_some_and(|f| f.spec.storage() == FnStorage::Static)
        }
        DeclKind::Var => {
            ast.decls.var(first).is_some_and(|v| v.spec.storage() == VarStorage::Static)
        }
        DeclKind::FunctionTemplate | DeclKind::VarTemplate => {
            return ast
                .template_info(first)
                .and_then(|t| t.pattern)
                .is_some_and(|t| internal(s, t));
        }
        _ => false,
    };
    by_storage || anon_ns_of(s, first).is_some()
}

/// The innermost unnamed namespace enclosing `d`.
fn anon_ns_of(s: &Sema<'_, '_>, d: DeclId) -> Option<DeclId> {
    let mut c = s.ast.decls.get(d).ctx();
    while let Some(x) = c {
        let n = s.ast.decls.get(x);
        if n.kind() == DeclKind::Namespace && n.name() == DeclName::EMPTY {
            return Some(x);
        }
        c = n.ctx();
    }
    None
}

impl Sema<'_, '_> {
    // -- activation and units ---------------------------------------------------

    /// Starts tracking module ownership (the main file may hold module
    /// directives, or one was reached). Idempotent.
    pub fn module_activate(&mut self) {
        if self.modules.frames.is_empty() {
            self.ast.modules.active = true;
            if self.ast.modules.units.is_empty() {
                self.ast.modules.units.push(UnitInfo::default());
            }
            self.modules.frames.push(Frame::default());
            self.module_recompute();
        }
    }

    /// True while modules are tracked.
    pub fn modules_active(&self) -> bool {
        self.ast.modules.active
    }

    /// The tokens of the imported unit `key` start (spec/modules.md §3).
    pub fn act_on_unit_begin(&mut self, key: &str, header: bool) {
        self.module_activate();
        let idx = u16::try_from(self.ast.modules.units.len()).unwrap_or(u16::MAX);
        let kind = if header { UnitKind::Header } else { UnitKind::NonModule };
        self.ast.modules.units.push(UnitInfo { key: key.to_owned(), kind, ..UnitInfo::default() });
        self.modules.keys.insert(key.to_owned(), idx);
        self.modules.frames.push(Frame { unit: idx, ..Frame::default() });
        self.module_owner_changed();
        self.module_recompute();
    }

    /// The tokens of the imported unit `key` ended.
    pub fn act_on_unit_end(&mut self, _key: &str) {
        if self.modules.frames.len() > 1 {
            self.modules.frames.pop();
        }
        self.module_owner_changed();
        self.module_recompute();
    }

    /// The owner of the declarations made from now on.
    fn module_owner(&self) -> Owner {
        let Some(f) = self.modules.frames.last() else { return Owner::default() };
        let purview = matches!(f.phase, Phase::Purview | Phase::Private);
        Owner { unit: f.unit, purview, exported: purview && !f.exports.is_empty() }
    }

    /// Whether a declaration made now in lexical context `lexical` is
    /// attached to a named module ([module.unit]/7): in a purview and in
    /// no linkage specification.
    pub(crate) fn attaches_to_named_module(&self, lexical: DeclId) -> bool {
        if !self.module_owner().purview {
            return false;
        }
        let mut c = Some(lexical);
        while let Some(x) = c {
            match self.ast.decls.get(x).kind() {
                DeclKind::LinkageSpec => return false,
                DeclKind::Export => c = self.ast.decls.get(x).ctx(),
                _ => return true,
            }
        }
        true
    }

    pub(crate) fn module_owner_changed(&mut self) {
        let o = self.module_owner();
        let next = self.ast.decls.next_id();
        self.ast.modules.set_owner_from(next, o);
    }

    /// Recomputes the units visible from the current one: its imports,
    /// transitively through `export import` ([module.import]/7), and all
    /// imports of the units of its own named module ([module.import]/8).
    fn module_recompute(&mut self) {
        let units = &self.ast.modules.units;
        let n = units.len();
        let cur = usize::from(self.modules.cur());
        let cur_mod =
            units.get(cur).filter(|u| u.kind.is_named()).and_then(|u| u.module.as_deref());
        let same: Vec<bool> = units
            .iter()
            .map(|u| u.kind.is_named() && cur_mod.is_some() && u.module.as_deref() == cur_mod)
            .collect();
        let mut vis = vec![false; n];
        if let Some(v) = vis.get_mut(cur) {
            *v = true;
        }
        let mut stack = vec![cur];
        while let Some(u) = stack.pop() {
            let Some(info) = units.get(u) else { continue };
            for (key, exported) in &info.imports {
                if u != cur && !exported && !same[u] {
                    continue;
                }
                let Some(&v) = self.modules.keys.get(key) else { continue };
                let v = usize::from(v);
                if v < n && !vis[v] {
                    vis[v] = true;
                    stack.push(v);
                }
            }
        }
        self.modules.visible = vis;
        self.modules.same = same;
    }

    // -- directives ----------------------------------------------------------------

    /// A module or import directive ([cpp.module], [cpp.import]) reached
    /// at the start of a namespace-scope declaration.
    /// `loc` is the directive's first token, `export` whether it starts
    /// with `export`.
    pub fn act_on_module_directive(&mut self, loc: Loc, export: bool, d: Directive<'_>) {
        self.module_activate();
        let cur = usize::from(self.modules.cur());
        let Some(f) = self.modules.frames.last().cloned() else { return };
        match d {
            Directive::GlobalFragment => {
                if f.phase != Phase::Start || f.first_decl.is_valid() {
                    let m = self.diag(DiagId::sema_module_gmf_not_first, loc);
                    self.emit(m);
                    return;
                }
                if let Some(f) = self.modules.frames.last_mut() {
                    f.phase = Phase::Gmf;
                }
            }
            Directive::Module { name, partition, kw_loc } => {
                if f.module_loc.is_valid() {
                    let note = self.diag(DiagId::note_module_previous_decl, f.module_loc);
                    let m = self.diag(DiagId::sema_module_multiple_decls, kw_loc);
                    self.emit(m.with_note(note));
                    return;
                }
                if f.phase == Phase::Start && f.first_decl.is_valid() {
                    let note = self.diag(DiagId::note_module_add_gmf, f.first_decl);
                    let m = self.diag(DiagId::sema_module_decl_not_first, kw_loc);
                    self.emit(m.with_note(note));
                }
                let kind = match (export, partition.is_some()) {
                    (true, false) => UnitKind::Interface,
                    (false, false) => UnitKind::Implementation,
                    (true, true) => UnitKind::PartitionInterface,
                    (false, true) => UnitKind::PartitionImplementation,
                };
                if let Some(u) = self.ast.modules.units.get_mut(cur) {
                    u.kind = kind;
                    u.module = Some(name.to_owned());
                    u.partition = partition.map(str::to_owned);
                    // [module.unit]/8: an implementation unit imports its
                    // primary interface.
                    if kind == UnitKind::Implementation {
                        u.imports.push((name.to_owned(), false));
                    }
                }
                if let Some(f) = self.modules.frames.last_mut() {
                    f.phase = Phase::Purview;
                    f.module_kw = loc;
                    f.module_loc = kw_loc;
                    f.decl_seen = false;
                }
                self.module_owner_changed();
                self.module_recompute();
            }
            Directive::Private(ploc) => {
                let kind = self.ast.modules.units.get(cur).map_or(UnitKind::NonModule, |u| u.kind);
                if !f.module_loc.is_valid() {
                    let m = self.diag(DiagId::sema_module_private_no_module, ploc);
                    self.emit(m);
                } else if !kind.is_interface() {
                    let m = self.diag(DiagId::sema_module_private_in_impl, ploc);
                    self.emit(m);
                } else if f.phase == Phase::Private {
                    let note = self.diag(DiagId::note_previous_definition, f.private_loc);
                    let m = self.diag(DiagId::sema_module_private_redefined, ploc);
                    self.emit(m.with_note(note));
                } else {
                    if let Some(f) = self.modules.frames.last_mut() {
                        f.phase = Phase::Private;
                        f.private_loc = loc;
                        f.decl_seen = false;
                    }
                    self.module_owner_changed();
                }
            }
            Directive::Import { key, display } => {
                if self.ctx != self.ast.tu {
                    let within = self.decl_name_str(self.ast.decls.get(self.ctx).name());
                    let m = self
                        .diag(DiagId::sema_module_import_not_file_scope, loc)
                        .arg(display)
                        .arg(format!("namespace '{within}'"));
                    self.emit(m);
                }
                if key.starts_with(':') {
                    let m = self.diag(DiagId::sema_module_partition_import_outside, loc);
                    self.emit(m);
                    return;
                }
                if matches!(f.phase, Phase::Purview | Phase::Private) && f.decl_seen {
                    let m = self.diag(DiagId::sema_module_import_not_first, loc);
                    self.emit(m);
                } else if export && !(f.phase == Phase::Purview && self.module_unit_is_interface())
                {
                    let m = self.diag(DiagId::sema_module_export_not_interface, loc);
                    self.emit(m);
                }
                if let Some(u) = self.ast.modules.units.get_mut(cur) {
                    u.imports.push((key.to_owned(), export));
                }
                self.module_recompute();
            }
        }
    }

    fn module_unit_is_interface(&self) -> bool {
        let cur = usize::from(self.modules.cur());
        self.ast.modules.units.get(cur).is_some_and(|u| u.kind.is_interface())
    }

    /// A preprocessing directive that may not precede a module directive
    /// ([cpp.pre]/1; P3868R1 allows only `#line` and the like) preceded
    /// the unit's `module;` or module declaration; `loc` is the unit's
    /// first directive. clang 22 then treats it as the first declaration,
    /// rejecting the directive (clang 19 accepts).
    pub fn module_note_pp_directive(&mut self, loc: Loc) {
        if self.clang_compat < 22 {
            return;
        }
        self.module_activate();
        if let Some(f) = self.modules.frames.last_mut()
            && f.phase == Phase::Start
            && !f.first_decl.is_valid()
        {
            f.first_decl = loc;
        }
    }

    /// A namespace-scope declaration starts at `loc` (for the import
    /// ordering rule of [module.import]/1).
    pub fn module_note_decl(&mut self, loc: Loc) {
        if let Some(f) = self.modules.frames.last_mut() {
            if !f.first_decl.is_valid() {
                f.first_decl = loc;
            }
            f.decl_seen = true;
        }
    }

    // -- export declarations ---------------------------------------------------

    /// `export` declaration / `export { ... }` ([module.interface]/1):
    /// opens the transparent export context.
    pub fn act_on_start_export(&mut self, loc: Loc) -> DeclId {
        self.module_activate();
        let f = self.modules.frames.last().cloned().unwrap_or_default();
        if let Some(&(_, outer)) = f.exports.last() {
            let note = self.diag(DiagId::note_module_export_block_begins, outer);
            let m = self.diag(DiagId::sema_module_export_nested, loc);
            self.emit(m.with_note(note));
        } else if f.phase == Phase::Private {
            let note = self.diag(DiagId::note_module_private_begins, f.private_loc);
            let m = self.diag(DiagId::sema_module_export_in_private, loc);
            self.emit(m.with_note(note));
        } else if f.phase != Phase::Purview || !self.module_unit_is_interface() {
            let mut m = self.diag(DiagId::sema_module_export_not_interface, loc);
            if f.phase == Phase::Purview {
                m = m.with_note(self.diag(DiagId::note_module_add_export, f.module_kw));
            }
            self.emit(m);
        } else if let Some(ns) = self.module_enclosing_anon_ns() {
            let note = self.diag(DiagId::note_module_anon_ns_begins, self.module_anon_loc(ns));
            let m = self.diag(DiagId::sema_module_export_in_anon_ns, loc);
            self.emit(m.with_note(note));
        }
        let ctx = self.ctx;
        let e = self.builder().export_decl(ctx, loc);
        self.scopes.push(Scope::new(ScopeKind::LinkageSpec, Some(e)));
        self.ctx = e;
        if let Some(f) = self.modules.frames.last_mut() {
            f.exports.push((e, loc));
        }
        self.module_owner_changed();
        e
    }

    /// Records the `{` of the unnamed namespace `ns`.
    pub fn module_note_anon_namespace(&mut self, ns: DeclId, lbrace: Loc) {
        self.modules.anon_braces.insert(ns, lbrace);
    }

    /// Where unnamed namespace `ns` is reported: its `{` if known.
    fn module_anon_loc(&self, ns: DeclId) -> Loc {
        self.modules.anon_braces.get(&ns).copied().unwrap_or_else(|| self.ast.decls.get(ns).loc())
    }

    /// The end of the export declaration `e`: checks what it exported
    /// ([module.interface]/3, /2). `block` is true for `export { ... }`
    /// (clang adds the "export block begins here" note only then).
    pub fn act_on_finish_export(&mut self, e: DeclId, block: bool) {
        self.scopes.pop();
        self.ctx = self.ast.decls.get(e).ctx().unwrap_or(self.ast.tu);
        let mut loc = self.ast.decls.get(e).loc();
        if let Some(f) = self.modules.frames.last_mut()
            && let Some((_, l)) = f.exports.pop()
        {
            loc = l;
        }
        self.module_owner_changed();
        self.module_check_exported(e, block.then_some(loc));
    }

    /// True if the current context is an export declaration, seen
    /// through linkage specifications only (clang's `CurContext` in
    /// `diagnoseQualifiedDeclaration`): `export void S::f() {}` and
    /// `export extern "C++" { void S::f() {} }`, not `export namespace N {
    /// void S::f() {} }`.
    pub(crate) fn module_in_export_decl(&self) -> bool {
        let mut c = self.ctx;
        for _ in 0..10_000 {
            match self.kind_of(c) {
                DeclKind::Export => return true,
                DeclKind::LinkageSpec => match self.ast.decls.get(c).ctx() {
                    Some(p) => c = p,
                    None => return false,
                },
                _ => return false,
            }
        }
        false
    }

    /// [module.interface]/1, /6: an export-declaration declares namespace
    /// members only, so a qualified redeclaration of a member of the
    /// class (or enumeration) `dc` directly in one is diagnosed at the
    /// declarator-id `loc` (clang's `err_export_non_namespace_scope_name`).
    /// The declaration is still declared as written (clang drops a member
    /// function or variable, then diagnoses uses of the class's members
    /// in its body; kiln does not cascade). True if diagnosed.
    pub(crate) fn module_check_export_member(&mut self, dc: DeclId, name: &str, loc: Loc) -> bool {
        if matches!(self.kind_of(dc), DeclKind::Namespace | DeclKind::TranslationUnit)
            || !self.module_in_export_decl()
        {
            return false;
        }
        let m = self.diag(DiagId::sema_module_export_non_namespace_scope_name, loc).arg(name);
        self.emit(m);
        true
    }

    /// The innermost unnamed namespace the current context is in.
    fn module_enclosing_anon_ns(&self) -> Option<DeclId> {
        let n = self.ast.decls.get(self.ctx);
        if n.kind() == DeclKind::Namespace && n.name() == DeclName::EMPTY {
            return Some(self.ctx);
        }
        anon_ns_of(self, self.ctx)
    }

    /// [module.interface]/3 (C++20 wording, as clang 22): an exported
    /// declaration shall not have internal linkage; [module.interface]/2:
    /// no unnamed namespace is exported.
    fn module_check_exported(&mut self, e: DeclId, block_loc: Option<Loc>) {
        let lo = e.index() + 1;
        let hi = self.ast.decls.next_id().index();
        for i in lo..hi {
            let d = DeclId::new(i);
            let n = self.ast.decls.get(d);
            // Using-declarations are checked through their (implicit) shadows.
            let implicit = n.has(DeclFlags::IMPLICIT) && n.kind() != DeclKind::UsingShadow;
            if implicit || !self.exported_by(d, e) {
                continue;
            }
            match n.kind() {
                DeclKind::Namespace if n.name() == DeclName::EMPTY => {
                    let m = self.diag(DiagId::sema_module_export_anon_ns, self.module_anon_loc(d));
                    self.emit(m);
                }
                DeclKind::UsingShadow => {
                    let t = self.ast.strip_shadow(d);
                    let mt = &self.ast.modules;
                    // Module linkage: owned by a named module's purview and
                    // never exported.
                    let module_linkage = || {
                        let o = mt.owner(t);
                        o.purview && !o.exported && !mt.owner(self.ast.first(t)).exported
                    };
                    let linkage = if t == d {
                        None
                    } else if internal(self, t) {
                        Some(0u8)
                    } else if module_linkage() {
                        Some(1)
                    } else {
                        None
                    };
                    if let Some(k) = linkage {
                        let note = self
                            .diag(DiagId::note_module_using_target, self.ast.decls.get(t).loc());
                        let name = self.decl_name_str(self.ast.decls.get(t).name());
                        let m = self
                            .diag(DiagId::sema_module_export_using_internal, n.loc())
                            .arg(name)
                            .arg(k);
                        self.emit(m.with_note(note));
                    }
                }
                DeclKind::Function
                | DeclKind::Var
                | DeclKind::FunctionTemplate
                | DeclKind::VarTemplate
                    if internal(self, d) && anon_ns_of(self, d).is_none() =>
                {
                    let name = self.decl_name_str(n.name());
                    let mut m = self.diag(DiagId::sema_module_export_internal, n.loc()).arg(name);
                    if let Some(l) = block_loc {
                        m = m.with_note(self.diag(DiagId::note_module_export_block_begins, l));
                    }
                    self.emit(m);
                }
                _ => {}
            }
        }
    }

    /// True if `d` is directly in export `e`, possibly through namespaces
    /// and linkage specifications.
    fn exported_by(&self, d: DeclId, e: DeclId) -> bool {
        // A namespace through its lexical context: a reopened namespace's
        // semantic context is its first declaration's (an unnamed
        // namespace reopened inside `export`). Anything else through its
        // semantic one (an out-of-line member is in its class).
        let n = self.ast.decls.get(d);
        let mut c =
            if n.kind() == DeclKind::Namespace { self.ast.decls.lexical_ctx(d) } else { n.ctx() };
        while let Some(x) = c {
            if x == e {
                return true;
            }
            match self.ast.decls.get(x).kind() {
                DeclKind::Namespace | DeclKind::LinkageSpec => c = self.ast.decls.lexical_ctx(x),
                _ => return false,
            }
        }
        false
    }

    // -- visibility --------------------------------------------------------------

    /// The namespace-scope declaration whose owner decides whether `raw`
    /// is visible: `raw` itself, or the enumeration of an unscoped
    /// enumerator; `None` for anything not at namespace scope (a scoped
    /// enumerator included), and namespaces. A using-declaration's shadow
    /// is implicit but has the using-declaration's owner: a shadow of a
    /// non-exported using-declaration is not visible to an importer even
    /// when its target is ([module.interface]/7, [namespace.udecl]/1).
    fn module_scope_decl(&self, raw: DeclId) -> Option<DeclId> {
        let mut d = raw;
        let n = self.ast.decls.get(d);
        let implicit = n.has(DeclFlags::IMPLICIT) && n.kind() != DeclKind::UsingShadow;
        if n.kind() == DeclKind::Namespace || implicit || !n.loc().is_valid() {
            return None;
        }
        let mut c = n.ctx()?;
        loop {
            match self.ast.decls.get(c).kind() {
                DeclKind::LinkageSpec | DeclKind::Export => c = self.ast.decls.get(c).ctx()?,
                DeclKind::Namespace | DeclKind::TranslationUnit => return Some(d),
                // A scoped enumeration's enumerator is found only by a
                // qualified lookup naming the enumeration, which is then
                // reachable: the enumerator is a member, visible with it
                // ([module.reach]/5, clang `isVisible` of a member of a
                // reachable scope).
                DeclKind::Enum if d == raw && self.is_scoped_enum(c) => return None,
                DeclKind::Enum if d == raw => {
                    d = c;
                    c = self.ast.decls.get(c).ctx()?;
                }
                _ => return None,
            }
        }
    }

    /// True if `raw` (a lookup result before looking through using
    /// shadows) is visible from the current unit (spec/modules.md §5).
    pub(crate) fn module_visible(&self, raw: DeclId) -> bool {
        let mt = &self.ast.modules;
        if !mt.active || !self.inst.stack.is_empty() || self.modules.see_hidden.get() {
            return true;
        }
        let Some(d) = self.module_scope_decl(raw) else { return true };
        self.module_visible_owner(d, mt.owner(d))
    }

    /// `raw` when it is visible, else a visible redeclaration of its
    /// entity (clang `LookupResult::getAcceptableDecl` /
    /// `findAcceptableDecl`): a lookup table holds only the latest
    /// declaration, which may belong to a unit the current one does not
    /// see while an earlier redeclaration is its own or merged into it
    /// (the global module is one module: [module.global.frag]/1,
    /// spec/modules.md §6). `None` when no declaration is visible.
    pub(crate) fn module_visible_redecl(&self, raw: DeclId) -> Option<DeclId> {
        if self.module_visible(raw) {
            return Some(raw);
        }
        // A template's redeclarations are its pattern's ([temp.decls]/1).
        if let Some(pattern) = self.ast.template_info(raw).and_then(|t| t.pattern) {
            return self
                .ast
                .redecls_of(pattern)
                .filter_map(|p| self.ast.described_template(p))
                .find(|&t| t != raw && self.module_visible(t));
        }
        self.ast.redecls_of(raw).find(|&x| x != raw && self.module_visible(x))
    }

    /// The declarations a lookup of `name` that found nothing would find
    /// if every reachable declaration were visible: the
    /// non-visible declarations clang's typo correction offers as the
    /// same name and diagnoses as a missing import
    /// (`Sema::diagnoseMissingImport`, [module.reach]/1). `qualifier`
    /// is the namespace of a qualified lookup. An unqualified lookup
    /// does not offer a declaration with module linkage of another
    /// module (clang 22 reports `use of undeclared identifier`), and a
    /// class scope never hides anything. `None` when nothing is found.
    pub(crate) fn module_hidden_lookup(
        &self,
        name: DeclName,
        qualifier: Option<DeclId>,
        kind: LookupKind,
    ) -> Option<Found> {
        if !self.ast.modules.active || !self.inst.stack.is_empty() {
            return None;
        }
        if let Some(q) = qualifier
            && !matches!(self.kind_of(q), DeclKind::Namespace | DeclKind::TranslationUnit)
        {
            return None;
        }
        self.modules.see_hidden.set(true);
        let found = match qualifier {
            Some(q) => self.lookup_qualified_dn(q, name, kind),
            None => self.lookup_name_dn(name, kind),
        };
        self.modules.see_hidden.set(false);
        let raw = *found.found.first()?;
        if qualifier.is_none() && !self.module_hidden_unqualified(raw) {
            return None;
        }
        Some(found)
    }

    /// A type-name `name` (qualified by the namespace `qualifier`) that
    /// names nothing visible but a reachable type: diagnosed as a missing
    /// import ([`Sema::module_diagnose_hidden`]) and classified as that
    /// type for recovery. `None` (nothing reported) otherwise.
    pub fn module_hidden_type(
        &mut self,
        name: kiln_source::symbol::Symbol,
        qualifier: Option<DeclId>,
        loc: Loc,
    ) -> Option<crate::decl::NameClass> {
        let dn = DeclName::ident(name);
        let found = self.module_hidden_lookup(dn, qualifier, LookupKind::Ordinary)?;
        let class = self.classify_found(&found);
        if !matches!(class, crate::decl::NameClass::Type(..)) {
            return None;
        }
        self.module_diagnose_hidden(&found, dn, loc);
        Some(class)
    }

    /// Lets lookup find non-visible declarations while a use already
    /// diagnosed by [`Sema::module_diagnose_hidden`] is rebuilt.
    pub(crate) fn modules_see_hidden(&self, on: bool) {
        self.modules.see_hidden.set(on);
    }

    /// True if an unqualified lookup offers the non-visible `raw` as a
    /// missing import: a global-module or an exported declaration, not
    /// one with module linkage of another module.
    fn module_hidden_unqualified(&self, raw: DeclId) -> bool {
        let mt = &self.ast.modules;
        let Some(d) = self.module_scope_decl(raw) else { return false };
        let o = mt.owner(d);
        !o.purview || o.exported || mt.owner(self.ast.first(d)).exported
    }

    /// Reports the use at `loc` of the non-visible declaration `found`
    /// (from [`Sema::module_hidden_lookup`]) as clang 22 does: "declaration
    /// of 'x' must be imported from module 'M' before it is required" for a
    /// declaration in M's purview, "missing '#include'; 'x' must be
    /// declared before it is used" for a global-module one, with a note at
    /// the declaration. Recovery uses the declaration; an exported one
    /// makes its unit visible from now on (clang's
    /// `createImplicitModuleImportForErrorRecovery`), so it is reported
    /// once.
    pub(crate) fn module_diagnose_hidden(&mut self, found: &Found, name: DeclName, loc: Loc) {
        let (Some(&raw), Some(&d)) = (found.found.first(), found.decls.first()) else { return };
        let text = self.decl_name_str(name);
        // A declaration found through a using-declaration is noted at
        // the using-declaration's shadow (clang 22).
        let at = if self.kind_of(raw) == DeclKind::UsingShadow { raw } else { d };
        let note = self.diag(DiagId::note_module_decl_not_visible, self.ast.decls.get(at).loc());
        let mt = &self.ast.modules;
        let scope = self.module_scope_decl(raw);
        let o = scope.map(|s| mt.owner(s)).unwrap_or_default();
        // A partition's declarations name the module (clang 22).
        let module = o
            .purview
            .then(|| mt.units.get(usize::from(o.unit)))
            .flatten()
            .and_then(|u| u.module.clone());
        let exported = scope.is_some_and(|s| o.exported || mt.owner(self.ast.first(s)).exported);
        let msg = match module {
            Some(m) => self.diag(DiagId::sema_module_unimported_use, loc).arg(text).arg(m),
            None => match self.module_header_hint(self.ast.decls.get(d).loc()) {
                Some(h) => {
                    self.diag(DiagId::sema_module_unimported_use_header_name, loc).arg(text).arg(h)
                }
                None => self.diag(DiagId::sema_module_unimported_use_header, loc).arg(text),
            },
        };
        self.emit(msg.with_note(note));
        if o.purview && exported {
            self.module_make_visible(usize::from(o.unit));
        }
    }

    /// The header to suggest including for a declaration at `decl` (clang
    /// `Preprocessor::getHeaderToIncludeForDiagnostics`): walking out from
    /// the file holding `decl` along its `#include`s, the first
    /// include-guarded file, named as the preprocessor suggested
    /// ([`SourceManager::include_guarded_suggestion`]). The walk stops at
    /// the module unit, which lexes in its own range (no include
    /// location); an `#include` written with a macro is walked through,
    /// its include location being the macro name.
    ///
    /// [`SourceManager::include_guarded_suggestion`]:
    ///     kiln_source::SourceManager::include_guarded_suggestion
    fn module_header_hint(&self, decl: Loc) -> Option<Box<str>> {
        let sm = self.diags.sm();
        let mut loc = sm.expansion_loc(decl);
        loop {
            let inc = sm.include_loc(loc);
            if !inc.is_valid() {
                return None;
            }
            let (file, _) = sm.offset_of(loc)?;
            if let Some(h) = sm.include_guarded_suggestion(file) {
                return Some(h);
            }
            loc = inc;
        }
    }

    /// Makes unit `u` and the units it re-exports visible from the current
    /// unit ([module.import]/7), for error recovery.
    fn module_make_visible(&mut self, u: usize) {
        let units = &self.ast.modules.units;
        let mut stack = vec![u];
        while let Some(u) = stack.pop() {
            let Some(v) = self.modules.visible.get_mut(u) else { continue };
            if *v {
                continue;
            }
            *v = true;
            let Some(info) = units.get(u) else { continue };
            for (key, exported) in &info.imports {
                if *exported && let Some(&w) = self.modules.keys.get(key) {
                    stack.push(usize::from(w));
                }
            }
        }
    }

    fn module_visible_owner(&self, d: DeclId, o: Owner) -> bool {
        let mt = &self.ast.modules;
        let cur = self.modules.cur();
        if o.unit == cur {
            return true;
        }
        let i = d.index();
        if let Some(u) = mt.units.get(usize::from(cur)) {
            let k = u.merged.partition_point(|&(_, hi)| hi <= i);
            if u.merged.get(k).is_some_and(|&(lo, _)| lo <= i) {
                return true;
            }
        }
        let u = usize::from(o.unit);
        if !self.modules.visible.get(u).copied().unwrap_or(false) {
            return false;
        }
        let Some(info) = mt.units.get(u) else { return true };
        if !info.kind.is_named() {
            return true;
        }
        if !o.purview {
            return false;
        }
        o.exported
            || self.modules.same.get(u).copied().unwrap_or(false)
            || mt.owner(self.ast.first(d)).exported
    }

    /// ADL ([basic.lookup.argdep]/4): `raw` is found when it is visible,
    /// or when it is exported, attached to a named module `M`, and has the
    /// same innermost enclosing non-inline namespace as an associated
    /// entity (class or enumeration of `assoc`) attached to `M` (reachable
    /// through an import the current unit does not see). As clang 22, a
    /// non-exported function of M's interface is not found by an
    /// importer's ADL.
    ///
    /// During an instantiation everything reachable is visible (the
    /// instantiation context, [module.context]), except a TU-local
    /// (internal-linkage) declaration of another module unit (not a header
    /// unit): naming it would be ill-formed ([basic.link]/14), and clang 22
    /// does not find it.
    pub(crate) fn module_adl_visible(
        &self,
        raw: DeclId,
        assoc: &crate::overload::adl::AssocSet,
    ) -> bool {
        let mt = &self.ast.modules;
        if mt.active
            && !self.inst.stack.is_empty()
            && let Some(d) = self.module_scope_decl(raw)
            && let o = mt.owner(d)
            && o.unit != self.modules.cur()
            && mt.units.get(usize::from(o.unit)).is_some_and(|u| u.kind.is_named())
            && internal(self, self.ast.strip_shadow(d))
        {
            return false;
        }
        if self.module_visible(raw) {
            return true;
        }
        let exported = self
            .module_scope_decl(raw)
            .is_some_and(|d| mt.owner(d).exported || mt.owner(self.ast.first(d)).exported);
        if !exported {
            return false;
        }
        let Some(m) = mt.purview_module(raw) else { return false };
        let ns = self.innermost_non_inline_namespace(self.ast.strip_shadow(raw));
        assoc.classes.iter().chain(&assoc.enums).any(|&e| {
            mt.purview_module(e) == Some(m) && self.innermost_non_inline_namespace(e) == ns
        })
    }

    /// The innermost enclosing namespace of `d` that is not inline (its
    /// primary context; the translation unit for the global namespace).
    fn innermost_non_inline_namespace(&self, d: DeclId) -> Option<DeclId> {
        let mut ns = self.innermost_namespace(d)?;
        while self.kind_of(ns) == DeclKind::Namespace
            && self
                .ast
                .decls
                .namespace(self.ast.first(ns))
                .is_some_and(kiln_ast::decl::NamespaceData::is_inline)
        {
            let p = self.ast.decls.get(ns).ctx()?;
            ns = self.ast.primary_context(p);
        }
        Some(ns)
    }

    /// Redeclaration lookup ([basic.link]/8): drops the declarations of
    /// another named module that are not exported (module linkage: a
    /// different entity) and internal-linkage declarations of other units.
    pub(crate) fn module_redecl_candidate(&self, raw: DeclId) -> bool {
        let mt = &self.ast.modules;
        if !mt.active {
            return true;
        }
        let Some(d) = self.module_scope_decl(raw) else { return true };
        let o = mt.owner(d);
        let cur = self.modules.cur();
        if o.unit == cur {
            return true;
        }
        if internal(self, self.ast.strip_shadow(d)) {
            return false;
        }
        if !o.purview || self.modules.same.get(usize::from(o.unit)).copied().unwrap_or(false) {
            return true;
        }
        o.exported || mt.owner(self.ast.first(d)).exported
    }

    /// A redeclaration in the current unit of `prev`, an entity of the
    /// global module declared only by units it does not see
    /// ([module.global.frag]/1: the global module is one module), that
    /// creates no declaration of its own (an opaque enum redeclaration):
    /// `prev` becomes visible here as if redeclared (clang gives the
    /// redeclaration its own visible decl). True if it did.
    pub(crate) fn module_merge_hidden_redecl(&mut self, prev: DeclId) -> bool {
        if !self.module_hidden_global(prev) {
            return false;
        }
        self.module_merge_one(prev);
        true
    }

    /// A definition in the current unit of the entity `def` (a previous
    /// definition) defines, when `def` belongs to the global module of
    /// another unit: not a redefinition ([basic.def.odr]/15, the same
    /// entity defined in two units' global module fragments) but a merge
    /// (clang `Sema::makeMergedDefinitionVisible`, which demotes or skips
    /// the second definition). `def` (and the template it describes)
    /// becomes visible here and true is returned (spec/modules.md §6).
    pub(crate) fn module_merge_hidden_definition(&mut self, def: DeclId) -> bool {
        if !self.module_hidden_global(def) {
            return false;
        }
        self.module_merge_one(def);
        if let Some(t) = self.ast.described_template(def) {
            self.module_merge_one(t);
        }
        true
    }

    /// True if `d` is a namespace-scope declaration of the global module
    /// that the current unit does not see: one of another named unit's
    /// global module fragment (never visible except by a merge,
    /// [module.global.frag]/3: the GMF is not exported), or one of a
    /// non-module unit or header unit the current unit does not import —
    /// the importer's own declarations before the `import` that built
    /// the current module unit in-process (another translation unit,
    /// [basic.def.odr]/15; WP-1156).
    fn module_hidden_global(&self, d: DeclId) -> bool {
        let mt = &self.ast.modules;
        if !mt.active || !self.inst.stack.is_empty() {
            return false;
        }
        let Some(s) = self.module_scope_decl(d) else { return false };
        let o = mt.owner(s);
        if o.unit == self.modules.cur() || o.purview {
            return false;
        }
        let u = usize::from(o.unit);
        mt.units.get(u).is_some_and(|info| {
            info.kind.is_named() || !self.modules.visible.get(u).copied().unwrap_or(false)
        })
    }

    /// Merges the one declaration `d` into the current unit.
    fn module_merge_one(&mut self, d: DeclId) {
        let i = d.index();
        self.module_merge_range(DeclId::new(i), DeclId::new(i + 1));
    }

    /// Merges the declarations `[lo, hi)` of another unit into the current
    /// one (spec/modules.md §6): they become visible as if declared here.
    pub fn module_merge_range(&mut self, lo: DeclId, hi: DeclId) {
        let cur = usize::from(self.modules.cur());
        let (lo, hi) = (lo.index(), hi.index());
        if lo >= hi {
            return;
        }
        let Some(u) = self.ast.modules.units.get_mut(cur) else { return };
        let v = &mut u.merged;
        let k = v.partition_point(|&(a, _)| a < lo);
        v.insert(k, (lo, hi));
        // Coalesce with the neighbours.
        let i = if k > 0 && v[k - 1].1 >= v[k].0 { k - 1 } else { k };
        while i + 1 < v.len() && v[i].1 >= v[i + 1].0 {
            v[i].1 = v[i].1.max(v[i + 1].1);
            v.remove(i + 1);
        }
    }

    /// The main unit's module declaration, for the declaration image
    /// (spec/modules.md §11.11 "Part 2a"): its first token and `module`
    /// keyword, and whether a private module fragment follows.
    pub(crate) fn module_main_frame(&self) -> Option<(Loc, Loc, bool)> {
        let f = self.modules.frames.first()?;
        Some((f.module_kw, f.module_loc, f.private_loc.is_valid()))
    }

    /// The current unit's index (spec/modules.md §6 dedup records).
    pub fn module_current_unit(&self) -> u16 {
        self.modules.cur()
    }

    // -- [basic.link]/11 -----------------------------------------------------------

    /// Checks the redeclarations made since the last call: a redeclaration
    /// is attached to the same module as the previous declaration
    /// ([basic.link]/11), and an exported redeclaration does not follow
    /// one with module linkage ([module.interface]/6).
    pub fn module_check_links(&mut self) {
        if self.ast.modules.links.is_empty() {
            return;
        }
        let links = std::mem::take(&mut self.ast.modules.links);
        if !self.inst.stack.is_empty() {
            return;
        }
        for (d, prev) in links {
            let (dn, pn) = (self.ast.decls.get(d), self.ast.decls.get(prev));
            if dn.kind() == DeclKind::Namespace
                || dn.has(DeclFlags::IMPLICIT)
                || pn.has(DeclFlags::IMPLICIT)
                || self.module_scope_decl(d).is_none()
            {
                continue;
            }
            let (a, b) = (self.module_attachment(d), self.module_attachment(prev));
            let name = self.decl_name_str(dn.name());
            if a != b {
                let say = |m: &Option<String>| {
                    m.as_ref()
                        .map_or_else(|| "the global module".to_owned(), |m| format!("module {m}"))
                };
                let note = self.diag(DiagId::note_previous_declaration, pn.loc());
                let m = self
                    .diag(DiagId::sema_module_redecl_attachment, dn.loc())
                    .arg(name)
                    .arg(say(&a))
                    .arg(say(&b));
                self.emit(m.with_note(note));
                continue;
            }
            let mt = &self.ast.modules;
            let (od, op) = (mt.owner(d), mt.owner(self.ast.first(prev)));
            if od.exported && op.purview && !op.exported && a.is_some() {
                let note = self.diag(DiagId::note_previous_declaration, pn.loc());
                let m =
                    self.diag(DiagId::sema_module_export_redecl_module_linkage, dn.loc()).arg(name);
                self.emit(m.with_note(note));
            }
        }
    }

    /// The named module `d` is attached to ([module.unit]/7): its purview's
    /// module unless it is in a linkage specification.
    fn module_attachment(&self, d: DeclId) -> Option<String> {
        self.ast.module_attachment(d).map(str::to_owned)
    }
}
