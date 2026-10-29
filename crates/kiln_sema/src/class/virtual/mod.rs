//! Virtual functions (WP-078a; [class.virtual], [class.abstract],
//! [expr.dynamic.cast], spec/sema-class.md §3.5 and §8): what a member
//! function overrides and the checks on it at its declaration
//! (this file), the subobject graph and final overriders (`graph`), the
//! checks at a class's `}` and after its complete-class contexts
//! (`complete`), abstract-class use sites (`abstract_`) and
//! `dynamic_cast` (`cast`).
//!
//! Virtual-ness and the virt-specifier bits are fixed when the function
//! is created (`FnSpec` is set once), so [`Sema::virtual_decl`] runs
//! before `DeclBuilder::function` and [`Sema::finish_virtual_decl`]
//! right after it.

mod abstract_;
mod cast;
mod complete;
mod graph;

use kiln_ast::decl::{DeclFlags, DeclKind, FnSpec, FnStorage};
use kiln_ast::names::DeclName;
use kiln_ast::types::{ExcSpec, FuncProto, Quals, TypeView};
use kiln_ast::{DeclId, ListId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;
use kiln_source::fx::FxHashSet;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::conv::bases::{self, BaseSearch};
use crate::declspec::{DeclSpec, Declarator};

pub use graph::{MAX_SUBOBJECTS, compute_virtuals, ensure_virtuals, subobject_graph};
pub(crate) use abstract_::AbstractUse;
pub(crate) use graph::{is_method_kind, is_pure, is_virtual, overridden};

/// Where a member function is declared, for [`Sema::virtual_decl`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct VirtualSite {
    /// The class (its definition) for a member function.
    pub class: DeclId,
    /// A member function (declared in, or defined outside, its class).
    pub method: bool,
    /// Declared inside its class definition (not a friend).
    pub in_class: bool,
    /// A friend declaration.
    pub friend: bool,
    /// The previous declaration it redeclares, if any.
    pub prev: Option<DeclId>,
    /// The declared name.
    pub name: Symbol,
}

/// What [`Sema::virtual_decl`] decided, applied by
/// [`Sema::finish_virtual_decl`] once the function exists.
#[derive(Clone, Debug, Default)]
pub(crate) struct VirtualDecl {
    /// The virtual functions it overrides (first declarations).
    overridden: Vec<DeclId>,
    /// The overridden list of the previous declaration, shared by a
    /// redeclaration.
    inherited: Option<ListId>,
    /// The declaration is invalid (`override`/`final` on a function that
    /// hides a virtual function).
    invalid: bool,
}

impl Sema<'_, '_> {
    /// The virtual-function processing of a function declaration
    /// ([class.virtual], [class.abstract]/2, [dcl.fct.spec]): sets the
    /// `virtual`/`pure`/`override`/`final` bits of `spec`, finds the
    /// functions it overrides and runs the checks clang runs at the
    /// declaration (`AddOverriddenMethods`, `CheckOverrideControl`, the
    /// covariant return and `final` checks).
    pub(crate) fn virtual_decl(
        &mut self,
        ds: &DeclSpec,
        d: &Declarator,
        ty: QualType,
        site: VirtualSite,
        spec: &mut FnSpec,
    ) -> VirtualDecl {
        let mut out = VirtualDecl::default();
        let is_static = spec.storage() == FnStorage::Static;
        // `virtual` ([dcl.fct.spec]/2, [class.virtual]).
        if let Some(vloc) = ds.virtual_ {
            if site.friend {
                let msg = self.diag(DiagId::parse_friend_decl_spec, vloc).arg("virtual");
                self.emit(msg);
            } else if !site.method || is_static {
                self.error(DiagId::sema_virtual_not_member_fn, vloc);
            } else if !site.in_class {
                self.error(DiagId::sema_virtual_out_of_class, vloc);
            } else {
                *spec = spec.with_virtual(true).with_virtual_written(true);
            }
        }
        // A redeclaration (out-of-class definition) of a virtual function.
        if let Some(p) = site.prev
            && site.method
            && is_virtual(&self.ast, p)
        {
            *spec = spec.with_virtual(true);
            if is_pure(&self.ast, p) {
                *spec = spec.with_pure(true);
            }
            out.inherited = self.ast.decls.cold.overrides.get(self.ast.first(p)).copied();
        }
        // [class.virtual]/2: a function with the same name,
        // parameter-type-list, cv- and ref-qualifiers as a virtual
        // function of a base overrides it.
        // A static member function also "overrides" (diagnosed at `}`).
        if site.method
            && site.in_class
            && !site.friend
            && d.op_name.is_none()
            && d.explicit_object.is_none()
            && site.prev.is_none()
        {
            out.overridden = self.overridden_methods(site.class, DeclName::ident(site.name), ty);
            if !out.overridden.is_empty() && !is_static {
                *spec = spec.with_virtual(true);
            }
            for &o in &out.overridden.clone() {
                // clang 19 runs both checks for every overridden function.
                self.check_override_return(site.name, d.name_loc(), ty, o);
                self.check_overrides_final(site.name, d.name_loc(), o);
            }
        }
        self.check_override_control(d, ty, site, spec, &mut out);
        // [class.abstract]/2: a pure-specifier on a virtual function.
        if let Some(_zero) = d.pure {
            if site.friend {
                self.unsupported("pure virtual friend functions", d.name_loc());
            } else if spec.is_virtual() {
                *spec = spec.with_pure(true);
            } else if !out.invalid {
                let msg = self
                    .diag(DiagId::sema_pure_not_virtual, d.name_loc())
                    .arg(crate::decl::sym_str(self, site.name));
                self.emit(msg);
            }
        }
        out
    }

    /// `virtual`, `override` and `final` on a declarator that does not
    /// declare a function ([dcl.fct.spec]/2, [class.mem.general]/7).
    pub(crate) fn virtual_on_non_function(&mut self, ds: &DeclSpec, d: &Declarator) {
        if let Some(l) = ds.virtual_ {
            self.error(DiagId::sema_virtual_not_member_fn, l);
        }
        for (at, kw) in [(d.virt_override, "override"), (d.virt_final, "final")] {
            if let Some(l) = at {
                let msg = self.diag(DiagId::sema_virt_spec_not_virtual, l).arg(kw);
                self.emit(msg);
            }
        }
    }

    /// Records what `f` overrides (spec/sema-class.md `overrides`) and
    /// marks it invalid if [`Sema::virtual_decl`] said so.
    pub(crate) fn finish_virtual_decl(&mut self, f: DeclId, v: VirtualDecl) {
        if !v.overridden.is_empty() {
            let _ = self.builder().set_overrides(f, &v.overridden);
        } else if let Some(l) = v.inherited
            && !self.ast.decls.cold.overrides.contains(f)
        {
            let _ = self.ast.decls.cold.overrides.insert(f, l);
        }
        if v.invalid {
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
        }
    }

    /// clang `CheckOverrideControl`: `override`/`final` on a function that
    /// is not virtual.
    fn check_override_control(
        &mut self,
        d: &Declarator,
        ty: QualType,
        site: VirtualSite,
        spec: &mut FnSpec,
        out: &mut VirtualDecl,
    ) {
        let (ov, fin) = (d.virt_override, d.virt_final);
        if ov.is_none() && fin.is_none() {
            return;
        }
        if site.method && !spec.is_virtual() {
            let hidden = self.hidden_virtuals(site.class, DeclName::ident(site.name), ty, &[]);
            if !hidden.is_empty() {
                let (loc, kw) = match ov {
                    Some(l) => (l, "override"),
                    None => (fin.unwrap_or_else(|| d.name_loc()), "final"),
                };
                let msg = self
                    .diag(DiagId::sema_virt_spec_hides_virtual, loc)
                    .arg(kw)
                    .arg(u8::from(hidden.len() > 1));
                let msg = self.hidden_virtual_notes(msg, ty, &hidden);
                self.emit(msg);
                *spec = spec.with_override(ov.is_some()).with_final(fin.is_some());
                out.invalid = true;
                return;
            }
        }
        if !site.method || !spec.is_virtual() {
            for (at, kw) in [(ov, "override"), (fin, "final")] {
                if let Some(l) = at {
                    let msg = self.diag(DiagId::sema_virt_spec_not_virtual, l).arg(kw);
                    self.emit(msg);
                }
            }
            return;
        }
        *spec = spec.with_override(ov.is_some()).with_final(fin.is_some());
        if ov.is_some() && out.overridden.is_empty() && out.inherited.is_none() && site.in_class {
            let msg = self
                .diag(DiagId::sema_override_not_overriding, d.name_loc())
                .arg(crate::decl::sym_str(self, site.name));
            self.emit(msg);
        }
    }

    /// The virtual functions a member function named `name` of type `ty`
    /// declared in `class` overrides ([class.virtual]/2; clang
    /// `AddOverriddenMethods`): for each base, the first virtual function
    /// with that name that it does not overload; the search does not go
    /// below a base where one is found. First declarations.
    pub(crate) fn overridden_methods(
        &self,
        class: DeclId,
        name: DeclName,
        ty: QualType,
    ) -> Vec<DeclId> {
        let mut out = Vec::new();
        let mut seen_v = FxHashSet::default();
        self.overridden_walk(class, &mut seen_v, 0, &mut |s, b| {
            for x in s.ast.lookup_local(b, name).iter() {
                if !is_method_kind(s.kind_of(x)) {
                    continue;
                }
                let m = s.ast.first(x);
                if !is_virtual(&s.ast, m) {
                    continue;
                }
                let mty = s.ast.decls.get(m).ty().unwrap_or(QualType::ERROR);
                if s.is_method_overload(ty, mty) {
                    continue;
                }
                if !out.contains(&m) {
                    out.push(m);
                }
                return true;
            }
            false
        });
        out
    }

    /// The virtual destructors a destructor of `class` overrides
    /// ([class.virtual]/7; for WP-067a's destructor declarations): each
    /// base's destructor, if virtual.
    #[allow(dead_code, reason = "used by WP-067a's destructor declarations")]
    pub(crate) fn overridden_destructors(&self, class: DeclId) -> Vec<DeclId> {
        let mut out = Vec::new();
        let mut seen_v = FxHashSet::default();
        self.overridden_walk(class, &mut seen_v, 0, &mut |s, b| {
            match s.ast.destructor(b).map(|x| s.ast.first(x)) {
                Some(m) if is_virtual(&s.ast, m) => {
                    if !out.contains(&m) {
                        out.push(m);
                    }
                    true
                }
                _ => false,
            }
        });
        out
    }

    /// Visits the bases of `class` depth-first, left to right (a virtual
    /// base once), not descending below a base where `visit` returns
    /// true.
    fn overridden_walk(
        &self,
        class: DeclId,
        seen_v: &mut FxHashSet<DeclId>,
        depth: usize,
        visit: &mut dyn FnMut(&Self, DeclId) -> bool,
    ) {
        if depth > 1024 {
            return;
        }
        for b in graph::direct_bases(&self.ast, class) {
            if b.is_virtual && !seen_v.insert(b.class) {
                continue;
            }
            let Some(def) = self.ast.definition(b.class) else { continue };
            if !visit(self, def) {
                self.overridden_walk(def, seen_v, depth + 1, visit);
            }
        }
    }

    /// clang `IsOverload` for two member functions with the same name:
    /// true if their parameter-type-lists, variadic-ness, cv- or
    /// ref-qualifiers differ (the return type does not matter).
    pub(crate) fn is_method_overload(&self, a: QualType, b: QualType) -> bool {
        let t = &self.ast.types;
        let (Some(pa), Some(pb)) = (t.proto(a), t.proto(b)) else { return true };
        let (ia, ib) = (pa.info(), pb.info());
        let (xa, xb) = (t.params(pa), t.params(pb));
        xa.len() != xb.len()
            || !xa.iter().zip(xb).all(|(x, y)| t.same_type(*x, *y))
            || ia.variadic() != ib.variadic()
            || ia.this_quals() != ib.this_quals()
            || ia.ref_qual() != ib.ref_qual()
    }

    /// `declaration of 'f' overrides a 'final' function`
    /// ([class.virtual]/4).
    fn check_overrides_final(&mut self, name: Symbol, at: Loc, o: DeclId) {
        let fin = self.ast.decls.function(o).is_some_and(|f| f.spec.is_final());
        if fin {
            let msg = self
                .diag(DiagId::sema_overrides_final, at)
                .arg(crate::decl::sym_str(self, name))
                .with_note(self.overridden_note(o));
            self.emit(msg);
        }
    }

    /// `overridden virtual function is here`.
    pub(crate) fn overridden_note(&self, o: DeclId) -> Diag {
        self.diag(DiagId::note_overridden_virtual, self.ast.decls.get(o).loc())
    }

    /// The written (sugared) return type of function type `q`.
    pub(crate) fn written_ret(&self, q: QualType) -> Option<QualType> {
        let t = &self.ast.types;
        t.proto_of(q.ty()).or_else(|| t.proto(q)).map(FuncProto::ret)
    }

    /// clang `CheckOverridingFunctionReturnType` ([class.virtual]/8):
    /// false (and diagnosed) if the return type of the overrider (`ty`)
    /// is neither the same as nor covariant with that of `o`.
    fn check_override_return(&mut self, name: Symbol, at: Loc, ty: QualType, o: DeclId) -> bool {
        let oty = self.ast.decls.get(o).ty().unwrap_or(QualType::ERROR);
        let (Some(nr), Some(or)) = (self.written_ret(ty), self.written_ret(oty)) else {
            return true;
        };
        if self.ast.types.same_type(nr, or) || self.is_error_type(nr) || self.is_error_type(or) {
            return true;
        }
        let nm = crate::decl::sym_str(self, name);
        let classes = match (self.ast.types.canonical_view(nr), self.ast.types.canonical_view(or)) {
            (TypeView::Pointer(a), TypeView::Pointer(b))
            | (TypeView::LValueRef(a), TypeView::LValueRef(b))
            | (TypeView::RValueRef(a), TypeView::RValueRef(b)) => Some((a, b)),
            _ => None,
        };
        let is_class = |s: &Self, q: QualType| {
            s.ast.types.as_record(q).is_some_and(|r| {
                s.ast.decls.record(r).is_some_and(|x| x.tag != kiln_ast::decl::TagKind::Union)
            })
        };
        let Some((nc, oc)) = classes.filter(|&(a, _)| is_class(self, a)) else {
            let msg = self
                .diag(DiagId::sema_override_return_differs, at)
                .arg(nm)
                .arg(self.ty_arg(nr))
                .arg(self.ty_arg(or))
                .with_note(self.overridden_note(o));
            self.emit(msg);
            return false;
        };
        if !self.ast.types.same_unqualified(nc, oc) {
            let nrec = self.ast.types.as_record(nc).unwrap_or(o);
            let being_defined = self.defining.iter().any(|&x| self.ast.first(x) == self.ast.first(nrec));
            if !being_defined && !self.is_complete(nc) {
                let msg = self
                    .diag(DiagId::sema_covariant_return_incomplete, at)
                    .arg(nm)
                    .arg(self.ty_arg(nc));
                let msg = self.incomplete_note(msg, nc);
                self.emit(msg);
                return false;
            }
            let Some(orec) = self.ast.types.as_record(oc) else { return true };
            match bases::find_base(self, nrec, orec) {
                BaseSearch::NotDerived => {
                    let msg = self
                        .diag(DiagId::sema_covariant_return_not_derived, at)
                        .arg(nm)
                        .arg(self.ty_arg(nc))
                        .arg(self.ty_arg(oc))
                        .with_note(self.overridden_note(o));
                    self.emit(msg);
                    return false;
                }
                BaseSearch::Ambiguous(paths) => {
                    let ncu = self.ast.types.canonical(nc).without_quals();
                    let ocu = self.ast.types.canonical(oc).without_quals();
                    let msg = self
                        .diag(DiagId::sema_covariant_return_ambiguous, at)
                        .arg(nm)
                        .arg(self.ty_arg(ncu))
                        .arg(self.ty_arg(ocu))
                        .arg(bases::ambiguous_paths_string(self, nc, &paths))
                        .with_note(self.overridden_note(o));
                    self.emit(msg);
                    return false;
                }
                BaseSearch::Unique(path) => {
                    let ncu = self.ast.types.canonical(nc).without_quals();
                    let ocu = self.ast.types.canonical(oc).without_quals();
                    if !self.check_covariant_base_access(ncu, ocu, &path, at) {
                        return false;
                    }
                }
            }
        }
        let t = &self.ast.types;
        if t.canonical(nr).quals() != t.canonical(or).quals() {
            let msg = self
                .diag(DiagId::sema_covariant_return_quals, at)
                .arg(nm)
                .arg(self.ty_arg(nr))
                .arg(self.ty_arg(or))
                .with_note(self.overridden_note(o));
            self.emit(msg);
            return false;
        }
        let (nq, oq) = (t.effective_quals(nc), t.effective_quals(oc));
        if nq != oq && nq.contains(oq) {
            let msg = self
                .diag(DiagId::sema_covariant_return_more_qualified, at)
                .arg(nm)
                .arg(self.ty_arg(nr))
                .arg(self.ty_arg(or))
                .with_note(self.overridden_note(o));
            self.emit(msg);
            return false;
        }
        true
    }

    /// The virtual functions of the bases of `class` that a member
    /// function named `name` of type `ty` hides without overriding them
    /// (clang `FindHiddenVirtualMethods`; identifier names only).
    /// `own` are the class's own declarations of the name to consider
    /// (empty: look them up).
    pub(crate) fn hidden_virtuals(
        &self,
        class: DeclId,
        name: DeclName,
        ty: QualType,
        own: &[DeclId],
    ) -> Vec<DeclId> {
        if name.as_ident().is_none() {
            return Vec::new();
        }
        // The roots of what the class's own declarations override, and
        // the targets of its using-declarations: not hidden.
        let mut keep: Vec<DeclId> = Vec::new();
        let own: Vec<DeclId> = if own.is_empty() {
            self.ast.lookup_local(class, name).iter().collect()
        } else {
            own.to_vec()
        };
        for x in own {
            let x = self.ast.strip_shadow(x);
            if is_method_kind(self.kind_of(x)) {
                self.most_overridden(self.ast.first(x), &mut keep, 0);
            }
        }
        let mut out: Vec<DeclId> = Vec::new();
        self.hidden_walk(class, name, ty, &keep, &mut out, 0);
        out
    }

    fn hidden_walk(
        &self,
        class: DeclId,
        name: DeclName,
        ty: QualType,
        keep: &[DeclId],
        out: &mut Vec<DeclId>,
        depth: usize,
    ) {
        if depth > 1024 {
            return;
        }
        'bases: for b in graph::direct_bases(&self.ast, class) {
            let Some(def) = self.ast.definition(b.class) else { continue };
            let mut found = false;
            let mut here: Vec<DeclId> = Vec::new();
            for x in self.ast.lookup_local(def, name).iter() {
                if !is_method_kind(self.kind_of(x)) {
                    continue;
                }
                let m = self.ast.first(x);
                found = true;
                if !is_virtual(&self.ast, m) {
                    continue;
                }
                let mty = self.ast.decls.get(m).ty().unwrap_or(QualType::ERROR);
                if !self.is_method_overload(ty, mty) {
                    continue 'bases;
                }
                if !self.reaches(m, keep, 0) {
                    here.push(m);
                }
            }
            if found {
                for m in here {
                    if !out.contains(&m) {
                        out.push(m);
                    }
                }
            } else {
                self.hidden_walk(def, name, ty, keep, out, depth + 1);
            }
        }
    }

    /// clang `AddMostOverridenMethods`: the methods `m` overrides that
    /// override nothing themselves (or `m`).
    fn most_overridden(&self, m: DeclId, out: &mut Vec<DeclId>, depth: usize) {
        let os: Vec<DeclId> = overridden(&self.ast, m).collect();
        if os.is_empty() || depth > 1024 {
            if !out.contains(&m) {
                out.push(m);
            }
            return;
        }
        for o in os {
            self.most_overridden(self.ast.first(o), out, depth + 1);
        }
    }

    /// clang `CheckMostOverridenMethods`: whether `m` or a method it
    /// overrides is in `set`.
    fn reaches(&self, m: DeclId, set: &[DeclId], depth: usize) -> bool {
        if set.contains(&m) {
            return true;
        }
        depth < 1024 && overridden(&self.ast, m).any(|o| self.reaches(self.ast.first(o), set, depth + 1))
    }

    /// clang `NoteHiddenVirtualMethods`: a note per hidden function,
    /// saying how its type differs from `ty` (clang
    /// `HandleFunctionTypeMismatch`, the base's first).
    pub(crate) fn hidden_virtual_notes(&self, mut msg: Diag, ty: QualType, hidden: &[DeclId]) -> Diag {
        for &h in hidden {
            let hty = self.ast.decls.get(h).ty().unwrap_or(QualType::ERROR);
            let suffix = self.fn_type_mismatch(hty, ty);
            msg = msg.with_note(
                self.diag(DiagId::note_hidden_overloaded_virtual, self.ast.decls.get(h).loc())
                    .arg(self.qualified_decl_name(h))
                    .arg(suffix),
            );
        }
        msg
    }

    /// How function type `to` (the hidden base function) differs from
    /// `from`, as clang's `ft_*` note suffixes.
    fn fn_type_mismatch(&self, to: QualType, from: QualType) -> String {
        let t = &self.ast.types;
        let (Some(pt), Some(pf)) = (t.proto(to), t.proto(from)) else { return String::new() };
        let (xt, xf) = (t.params(pt), t.params(pf));
        if xt.len() != xf.len() {
            return format!(": different number of parameters ({} vs {})", xt.len(), xf.len());
        }
        // The written parameter types print with their sugar.
        let wt = t.proto_of(to.ty()).map_or(xt, |p| t.params(p));
        let wf = t.proto_of(from.ty()).map_or(xf, |p| t.params(p));
        for (i, (a, b)) in xt.iter().zip(xf).enumerate() {
            if !t.same_type(*a, *b) {
                let (sa, sb) = (wt.get(i).copied().unwrap_or(*a), wf.get(i).copied().unwrap_or(*b));
                return format!(
                    ": type mismatch at {} parameter ({} vs {})",
                    crate::expr::ordinal(i + 1),
                    self.ty_arg(sa),
                    self.ty_arg(sb)
                );
            }
        }
        if !t.same_type(pt.ret(), pf.ret()) {
            let rt = self.written_ret(to).unwrap_or(pt.ret());
            let rf = self.written_ret(from).unwrap_or(pf.ret());
            return format!(
                ": different return type ({} vs {})",
                self.ty_arg(rt),
                self.ty_arg(rf)
            );
        }
        let (qt, qf) = (pt.info().this_quals(), pf.info().this_quals());
        if qt != qf {
            let q = |x: Quals| {
                if x.is_empty() {
                    "unqualified".to_owned()
                } else {
                    let mut s = String::new();
                    x.write(&mut s);
                    format!("'{}'", s.trim())
                }
            };
            return format!(": different qualifiers ({} vs {})", q(qt), q(qf));
        }
        let nothrow = |p: &FuncProto| t.exc_spec(p).is_nothrow();
        if nothrow(pt) != nothrow(pf) {
            return ": different exception specifications".to_owned();
        }
        String::new()
    }

    /// `exception specification of overriding function is more lax than
    /// base version` ([except.spec]/8): a potentially-throwing overrider
    /// of a non-throwing virtual function. Checked once the class's
    /// complete-class contexts (and so `noexcept(expr)`) are parsed.
    pub(crate) fn check_override_exc_spec(&mut self, m: DeclId) {
        let mty = self.ast.decls.get(m).ty().unwrap_or(QualType::ERROR);
        let Some(mp) = self.ast.types.proto(mty) else { return };
        let mexc = self.ast.types.exc_spec(mp);
        if mexc.is_nothrow() || matches!(mexc, ExcSpec::Deferred(_)) {
            return;
        }
        let os: Vec<DeclId> = overridden(&self.ast, m).collect();
        for o in os {
            let oty = self.ast.decls.get(o).ty().unwrap_or(QualType::ERROR);
            let Some(op) = self.ast.types.proto(oty) else { continue };
            if self.ast.types.exc_spec(op).is_nothrow() {
                let msg = self
                    .diag(DiagId::sema_override_exception_spec, self.ast.decls.get(m).loc())
                    .with_note(self.overridden_note(o));
                self.emit(msg);
                return;
            }
        }
    }
}

/// Whether `k` is a member-function kind for clippy-free reuse.
#[allow(dead_code, reason = "kept for WP-067a's destructor declarations")]
pub(crate) fn is_destructor(k: DeclKind) -> bool {
    k == DeclKind::CXXDestructor
}
