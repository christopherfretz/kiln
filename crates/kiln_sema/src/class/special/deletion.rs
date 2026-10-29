//! Deleted special members: `should_delete` ([class.default.ctor]/2,
//! [class.copy.ctor]/10, [class.copy.assign]/7, [class.dtor]/7,
//! [class.union.general]/3) with clang's notes, the selection of a
//! subobject's special member (spec/sema-class.md §4.2, the Q7
//! unique-candidate approximation until WP-068a), and the checks of an
//! explicitly defaulted special member ([dcl.fct.def.default]/2–5; clang
//! `ShouldDeleteSpecialMember`, `CheckExplicitlyDefaultedSpecialMember`).


use kiln_ast::decl::{BaseSpec, DeclFlags, DeclKind, Defaulted, DefinitionBits, FnSpec};
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::types::{ExcSpec, FuncProto, Quals, TypeView};
use kiln_ast::{DeclId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;

use super::SpecialKind;
use crate::Sema;
use crate::overload::{ArgInfo, CandidateKind, IcsOptions, SetKind};
use kiln_ast::expr::ValueCategory;

/// How deep the explanation of a deleted member recurses into the
/// members it selects (each step is a strictly smaller subobject).
const MAX_NOTE_DEPTH: usize = 64;

/// The special member of a class that a use selects (clang
/// `SpecialMemberOverloadResult`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Selected {
    /// A usable member.
    Found(DeclId),
    /// No viable member.
    Missing,
    /// The best viable member is deleted.
    Deleted(DeclId),
    /// No best viable member.
    Ambiguous,
    /// Selecting needs overload resolution kiln does not do yet
    /// (todo(WP-068a): templates, constraints, inherited members,
    /// derived-to-base ranking).
    Unsupported,
}

/// The answer of [`Sema::should_delete`]'s walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeleteReason {
    /// Not deleted.
    No,
    /// Deleted (the notes, if asked for, say why).
    Yes,
    /// Undecidable without overload resolution (Q7).
    Unsupported,
}

/// A potentially constructed subobject.
#[derive(Clone, Copy)]
enum Subobject {
    Base(BaseSpec),
    Field(DeclId),
}

/// The walk state of one `should_delete` (clang
/// `SpecialMemberDeletionInfo`).
struct Walk {
    member: DeclId,
    class: DeclId,
    kind: SpecialKind,
    const_arg: bool,
    all_fields_const: bool,
    notes: Option<Vec<Diag>>,
    depth: usize,
}

impl Sema<'_, '_> {
    /// `d` as clang prints a declaration argument: `'name'` (unqualified).
    pub(crate) fn decl_arg(&self, d: DeclId) -> String {
        use kiln_ast::types::TypeNames as _;
        let mut s = String::new();
        crate::names::SemaNames::new(self).write_decl_name(d, false, &mut s);
        format!("'{s}'")
    }

    /// Selects the special member `k` of complete class `class` for an
    /// argument (copy/move) of the class type with qualifiers `rhs` and,
    /// for an assignment, an object with qualifiers `lhs` (clang
    /// `LookupSpecialMember`); declares the implicit members it needs.
    pub(crate) fn select_special(
        &mut self,
        class: DeclId,
        k: SpecialKind,
        rhs: Quals,
        lhs: Quals,
    ) -> Selected {
        use SpecialKind as K;
        let c = self.class_def(class);
        let bits = self.defn_bits(c);
        match k {
            K::DefaultCtor => self.ensure_implicit(c, K::DefaultCtor),
            K::CopyCtor | K::MoveCtor => {
                self.ensure_implicit(c, K::CopyCtor);
                self.ensure_implicit(c, K::MoveCtor);
            }
            K::CopyAssign | K::MoveAssign => {
                self.ensure_implicit(c, K::CopyAssign);
                self.ensure_implicit(c, K::MoveAssign);
            }
            K::Dtor => self.ensure_implicit(c, K::Dtor),
        }
        if k == K::Dtor {
            return match self.ast.destructor(c) {
                None => Selected::Missing,
                Some(d) if self.fn_spec(d).is_deleted() => Selected::Deleted(d),
                Some(d) => Selected::Found(d),
            };
        }
        if k.is_ctor() && bits.contains(DefinitionBits::HAS_INHERITED_CTOR)
            || k.is_assign() && bits.contains(DefinitionBits::HAS_INHERITED_ASSIGN)
        {
            return Selected::Unsupported;
        }
        // clang `LookupSpecialMember`: an argument of the class type (an
        // lvalue for copy, an xvalue for move) without user-defined
        // conversions, and for an assignment an lvalue object.
        let cty = self.class_type(c);
        let mut set = self.begin_set(SetKind::Normal, Loc::INVALID);
        if k != K::DefaultCtor {
            let vk = if k.is_move() { ValueCategory::XValue } else { ValueCategory::LValue };
            let ty = self.ast.types.qualify(cty, rhs);
            set.args.push(ArgInfo::of_type(ty, vk));
        }
        if k.is_ctor() {
            let mut found = std::mem::take(&mut set.found);
            found.clear();
            found.extend(self.ast.constructors(c).iter());
            self.add_found_candidates(&mut set, &found, CandidateKind::Ctor, IcsOptions::SUPPRESS_UDC, false);
            set.found = found;
        } else {
            let ty = self.ast.types.qualify(cty, lhs);
            set.object = Some(ArgInfo::of_type(ty, ValueCategory::LValue));
            let mut found = std::mem::take(&mut set.found);
            found.clear();
            found.extend(self.ast.lookup_local(c, DeclName::operator(OverloadedOperator::Equal)).iter());
            self.add_found_candidates(&mut set, &found, CandidateKind::Method, IcsOptions::SUPPRESS_UDC, false);
            set.found = found;
        }
        let out = self.best_viable(&mut set);
        let r = self.selected_of(set, out);
        // Special-member selection keeps no notes.
        if let Some(old) = self.ovl.failed.take() {
            self.end_set(old);
        }
        r
    }

    pub(crate) fn fn_spec(&self, f: DeclId) -> FnSpec {
        self.ast.decls.function(f).map(|x| x.spec).unwrap_or_default()
    }

    fn fn_info(&self, f: DeclId) -> kiln_ast::types::FuncInfo {
        let ty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        self.ast.types.proto(ty).map(FuncProto::info).unwrap_or_default()
    }

    pub(crate) fn fn_variadic(&self, f: DeclId) -> bool {
        self.fn_info(f).variadic()
    }

    /// Whether the special member `f` (of kind `k`) is deleted when
    /// defaulted (clang `ShouldDeleteSpecialMember`). A selection that
    /// needs overload resolution reports `sema_unsupported` (Q7) and
    /// answers no.
    pub(crate) fn should_delete(&mut self, f: DeclId, k: SpecialKind, diagnose: bool) -> bool {
        match self.deletion(f, k, false, 0).0 {
            DeleteReason::Yes => true,
            DeleteReason::No => false,
            DeleteReason::Unsupported => {
                if !diagnose {
                    let loc = self.ast.decls.get(f).loc();
                    let what =
                        format!("deciding whether the {} needs overload resolution", k.spelling());
                    self.unsupported(&what, loc);
                }
                false
            }
        }
    }

    /// The notes explaining why defaulted special member `f` is deleted.
    pub(crate) fn deletion_notes(&mut self, f: DeclId, k: SpecialKind, depth: usize) -> Vec<Diag> {
        self.deletion(f, k, true, depth).1
    }

    /// The notes of clang `NoteDeletedFunction` for deleted function `m`.
    pub(crate) fn deleted_function_notes(&mut self, m: DeclId, depth: usize) -> Vec<Diag> {
        let spec = self.fn_spec(m);
        if spec.defaulted() != Defaulted::No {
            let mut out = Vec::new();
            if !self.ast.decls.get(m).has(DeclFlags::IMPLICIT) {
                out.push(
                    self.diag(DiagId::note_defaulted_deleted_here, self.ast.decls.get(m).loc()),
                );
            }
            if depth < MAX_NOTE_DEPTH
                && let Some(k) = self.special_kind_of(m)
            {
                out.extend(self.deletion_notes(m, k, depth + 1));
            }
            return out;
        }
        vec![
            self.diag(DiagId::note_explicitly_deleted_here, self.ast.decls.get(m).loc())
                .arg(self.decl_arg(m)),
        ]
    }

    fn deletion(
        &mut self,
        f: DeclId,
        k: SpecialKind,
        diagnose: bool,
        depth: usize,
    ) -> (DeleteReason, Vec<Diag>) {
        use SpecialKind as K;
        let Some(ctx) = self.ast.decls.get(f).ctx() else { return (DeleteReason::No, Vec::new()) };
        let class = self.class_def(ctx);
        let cflags = self.ast.decls.get(class).flags();
        if self.ast.decls.get(f).has(DeclFlags::INVALID)
            || cflags.contains(DeclFlags::INVALID)
            || cflags.contains(DeclFlags::DEPENDENT_CTX)
        {
            return (DeleteReason::No, Vec::new());
        }
        let bits = self.defn_bits(class);
        if bits.contains(DefinitionBits::LAMBDA) {
            // todo(WP-074): closure types ([expr.prim.lambda.closure]/13).
            let r = if matches!(k, K::DefaultCtor | K::CopyAssign) {
                DeleteReason::Unsupported
            } else {
                DeleteReason::No
            };
            return (r, Vec::new());
        }
        if k != K::DefaultCtor && k != K::Dtor && bits.contains(DefinitionBits::ANONYMOUS) {
            return (DeleteReason::No, Vec::new());
        }
        let mut notes = Vec::new();
        if self.ast.decls.get(f).has(DeclFlags::IMPLICIT)
            && matches!(k, K::CopyCtor | K::CopyAssign)
        {
            let moves = if bits.contains(DefinitionBits::USER_DECLARED_MOVE_CTOR) {
                Some((K::MoveCtor, 0u8))
            } else if bits.contains(DefinitionBits::USER_DECLARED_MOVE_ASSIGN) {
                Some((K::MoveAssign, 1u8))
            } else {
                None
            };
            if let Some((mk, sel)) = moves {
                if diagnose {
                    let m = self.find_user_declared(class, mk);
                    let loc = m.map_or_else(
                        || self.ast.decls.get(class).loc(),
                        |m| self.ast.decls.get(m).loc(),
                    );
                    notes.push(
                        self.diag(DiagId::note_deleted_copy_user_declared_move, loc)
                            .arg(u8::from(k == K::CopyAssign))
                            .arg(self.decl_arg(class))
                            .arg(sel),
                    );
                }
                return (DeleteReason::Yes, notes);
            }
        }
        // todo: [class.dtor]/7.3, the deallocation function of a virtual
        // destructor, is not checked (spec/sema-class.md §4.2).
        let const_arg = matches!(k, K::CopyCtor | K::CopyAssign) && self.copy_param_is_const_ref(f);
        let mut w = Walk {
            member: f,
            class,
            kind: k,
            const_arg,
            all_fields_const: true,
            notes: diagnose.then(Vec::new),
            depth,
        };
        let r = self.walk_subobjects(&mut w);
        (r, w.notes.unwrap_or_default())
    }

    /// The first user-declared member of kind `k` of class `c`.
    fn find_user_declared(&self, c: DeclId, k: SpecialKind) -> Option<DeclId> {
        self.ast
            .members(c)
            .find(|&m| self.special_kind_of(m) == Some(k) && self.is_user_declared(m))
    }

    /// Whether the first parameter of `f` is a reference to `const`.
    pub(super) fn copy_param_is_const_ref(&self, f: DeclId) -> bool {
        let Some(&p) = self.param_decls(f).first() else { return false };
        let ty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        match self.ast.types.canonical_view(ty) {
            TypeView::LValueRef(t) | TypeView::RValueRef(t) => {
                self.ast.types.effective_quals(t).is_const()
            }
            _ => false,
        }
    }

    fn walk_subobjects(&mut self, w: &mut Walk) -> DeleteReason {
        let bases: Vec<BaseSpec> =
            self.ast.decls.bases(w.class, &self.ast.lists).copied().collect();
        let mut subs: Vec<BaseSpec> = Vec::new();
        if w.kind.is_assign() {
            subs.extend(bases.iter().copied());
        } else {
            subs.extend(bases.iter().filter(|b| !b.is_virtual).copied());
            if !self.defn_bits(w.class).contains(DefinitionBits::ABSTRACT) {
                subs.extend(self.virtual_bases(w.class).into_iter().map(|x| x.1));
            }
        }
        for b in subs {
            let Some(bc) = self.base_class(&b) else { continue };
            match self.delete_for_class_subobject(w, bc, Subobject::Base(b), Quals::NONE) {
                DeleteReason::No => {}
                r => return r,
            }
        }
        let fields = self.special_fields(w.class);
        for f in fields {
            if self.ast.decls.get(f).has(DeclFlags::INVALID) {
                continue;
            }
            match self.delete_for_field(w, f) {
                DeleteReason::No => {}
                r => return r,
            }
        }
        // [class.default.ctor]/2: a union whose members are all const.
        if self.is_union(w.class)
            && w.kind == SpecialKind::DefaultCtor
            && w.all_fields_const
            && !self.special_fields(w.class).is_empty()
        {
            if let Some(n) = w.notes.as_mut() {
                let d = self
                    .diag(DiagId::note_deleted_all_const, self.ast.decls.get(w.class).loc())
                    .arg(self.decl_arg(w.class))
                    .arg(0u8);
                n.push(d);
            }
            return DeleteReason::Yes;
        }
        DeleteReason::No
    }

    fn note(&self, w: &mut Walk, d: impl FnOnce(&Self) -> Diag) {
        if w.notes.is_some() {
            let d = d(self);
            if let Some(n) = w.notes.as_mut() {
                n.push(d);
            }
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one case per [class.default.ctor]/2 and [class.copy.ctor]/10 bullet"
    )]
    fn delete_for_field(&mut self, w: &mut Walk, f: DeclId) -> DeleteReason {
        use SpecialKind as K;
        let full = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        let fty = self.base_element(full);
        let is_ref = self.ast.types.is_reference(fty);
        let rec = if is_ref { None } else { self.element_class(fty) };
        let in_union = self.is_union(w.class);
        let init = self.field_has_nsdmi(f);
        let quals = self.ast.types.effective_quals(fty);
        let floc = self.ast.decls.get(f).loc();
        let class = w.class;
        match w.kind {
            K::DefaultCtor => {
                if is_ref && !init {
                    self.note(w, |s| {
                        s.diag(DiagId::note_deleted_uninit_field, floc)
                            .arg(s.decl_arg(class))
                            .arg(s.decl_arg(f))
                            .arg(0u8)
                            .arg(s.ty_arg(fty))
                    });
                    return DeleteReason::Yes;
                }
                if !in_union
                    && quals.is_const()
                    && !init
                    && rec.is_none_or(|r| {
                        !self.defn_bits(r).contains(DefinitionBits::CAN_CONST_DEFAULT_INIT)
                    })
                {
                    self.note(w, |s| {
                        s.diag(DiagId::note_deleted_uninit_field, floc)
                            .arg(s.decl_arg(class))
                            .arg(s.decl_arg(f))
                            .arg(1u8)
                            .arg(s.ty_arg(full))
                    });
                    return DeleteReason::Yes;
                }
                if in_union && !quals.is_const() {
                    w.all_fields_const = false;
                }
            }
            K::CopyCtor => {
                if matches!(self.ast.types.canonical_view(fty), TypeView::RValueRef(_)) {
                    self.note(w, |s| {
                        s.diag(DiagId::note_deleted_rvalue_ref_field, floc)
                            .arg(s.decl_arg(class))
                            .arg(s.decl_arg(f))
                            .arg(s.ty_arg(fty))
                    });
                    return DeleteReason::Yes;
                }
            }
            K::CopyAssign | K::MoveAssign => {
                let mv = u8::from(w.kind == K::MoveAssign);
                if is_ref {
                    self.note(w, |s| {
                        s.diag(DiagId::note_deleted_assign_field, floc)
                            .arg(mv)
                            .arg(s.decl_arg(class))
                            .arg(s.decl_arg(f))
                            .arg(0u8)
                            .arg(s.ty_arg(fty))
                    });
                    return DeleteReason::Yes;
                }
                if rec.is_none() && quals.is_const() {
                    self.note(w, |s| {
                        s.diag(DiagId::note_deleted_assign_field, floc)
                            .arg(mv)
                            .arg(s.decl_arg(class))
                            .arg(s.decl_arg(f))
                            .arg(1u8)
                            .arg(s.ty_arg(full))
                    });
                    return DeleteReason::Yes;
                }
            }
            K::MoveCtor | K::Dtor => {}
        }
        let Some(rec) = rec else { return DeleteReason::No };
        if !in_union && self.is_union(rec) && self.is_anonymous_member(f) {
            let mut all_const = true;
            let ufields: Vec<DeclId> = self
                .ast
                .decls
                .members(rec)
                .filter(|&x| self.kind_of(x) == DeclKind::Field)
                .collect();
            for uf in &ufields {
                let ut = self.base_element(self.ast.decls.get(*uf).ty().unwrap_or(QualType::ERROR));
                let uq = self.ast.types.effective_quals(ut);
                if !uq.is_const() {
                    all_const = false;
                }
                if !self.ast.types.is_reference(ut)
                    && let Some(urec) = self.element_class(ut)
                {
                    match self.delete_for_class_subobject(w, urec, Subobject::Field(*uf), uq) {
                        DeleteReason::No => {}
                        r => return r,
                    }
                }
            }
            if w.kind == K::DefaultCtor && all_const && !ufields.is_empty() {
                let rloc = self.ast.decls.get(rec).loc();
                self.note(w, |s| {
                    s.diag(DiagId::note_deleted_all_const, rloc).arg(s.decl_arg(class)).arg(1u8)
                });
                return DeleteReason::Yes;
            }
            return DeleteReason::No;
        }
        self.delete_for_class_subobject(w, rec, Subobject::Field(f), quals)
    }

    fn delete_for_class_subobject(
        &mut self,
        w: &mut Walk,
        rec: DeclId,
        sub: Subobject,
        quals: Quals,
    ) -> DeleteReason {
        use SpecialKind as K;
        let field = match sub {
            Subobject::Field(f) => Some(f),
            Subobject::Base(_) => None,
        };
        let mutable = field.is_some_and(|f| self.field_is_mutable(f));
        let has_init = field.is_some_and(|f| self.field_has_nsdmi(f));
        if !(w.kind == K::DefaultCtor && has_init) {
            let lhs = if w.kind.is_assign() { quals } else { Quals::NONE };
            let rhs = if matches!(w.kind, K::DefaultCtor | K::Dtor) {
                Quals::NONE
            } else if w.const_arg && !mutable {
                quals | Quals::CONST
            } else {
                quals
            };
            let sel = self.select_special(rec, w.kind, rhs & Quals::CV, lhs & Quals::CV);
            match self.delete_for_call(w, sub, sel, false) {
                DeleteReason::No => {}
                r => return r,
            }
        }
        if w.kind.is_ctor() {
            let sel = self.select_special(rec, K::Dtor, Quals::NONE, Quals::NONE);
            return self.delete_for_call(w, sub, sel, true);
        }
        DeleteReason::No
    }

    fn delete_for_call(
        &mut self,
        w: &mut Walk,
        sub: Subobject,
        sel: Selected,
        dtor_in_ctor: bool,
    ) -> DeleteReason {
        let (kind, decl) = match sel {
            Selected::Unsupported => return DeleteReason::Unsupported,
            Selected::Missing => (0u8, None),
            Selected::Deleted(m) => (1, Some(m)),
            Selected::Ambiguous => (2, None),
            Selected::Found(m) => {
                let is_base = matches!(sub, Subobject::Base(_));
                if !self.special_accessible_from(m, w.member_class(), is_base) {
                    (3, Some(m))
                } else if let Subobject::Field(f) = sub
                    && !dtor_in_ctor
                    && self.ast.decls.get(f).ctx().is_some_and(|p| self.is_union(p))
                    && !self.fn_spec(m).is_trivial()
                {
                    let parent = self.ast.decls.get(f).ctx().map(|p| self.class_def(p));
                    let parent_init = parent.is_some_and(|p| {
                        self.defn_bits(p).contains(DefinitionBits::HAS_IN_CLASS_INIT)
                    });
                    if w.kind != SpecialKind::DefaultCtor || !parent_init {
                        (4, Some(m))
                    } else {
                        return DeleteReason::No;
                    }
                } else {
                    return DeleteReason::No;
                }
            }
        };
        if w.notes.is_some() {
            let what = if dtor_in_ctor { "destructor" } else { w.kind.spelling() };
            let what = if kind == 2 { format!("{what}s") } else { what.to_owned() };
            let class = w.class;
            let d = match sub {
                Subobject::Field(f) => self
                    .diag(DiagId::note_deleted_subobject, self.ast.decls.get(f).loc())
                    .arg(w.kind.index())
                    .arg(self.decl_arg(class))
                    .arg(if kind == 4 { 2u8 } else { 1u8 })
                    .arg(self.decl_arg(f)),
                Subobject::Base(b) => self
                    .diag(DiagId::note_deleted_subobject, b.loc)
                    .arg(w.kind.index())
                    .arg(self.decl_arg(class))
                    .arg(0u8)
                    .arg(self.ty_arg(b.ty)),
            };
            let d = d.arg(kind).arg(what);
            let nested = match (kind, decl) {
                (1, Some(m)) => self.deleted_function_notes(m, w.depth + 1),
                _ => Vec::new(),
            };
            if let Some(n) = w.notes.as_mut() {
                n.push(d);
                n.extend(nested);
            }
        }
        DeleteReason::Yes
    }

    /// Checks explicitly defaulted special member `f` on its first
    /// declaration once its class is complete ([dcl.fct.def.default]/2–3,
    /// spec/sema-class.md §4.4): the signature, `constexpr`, the deferred
    /// exception specification, and deletion with
    /// `-Wdefaulted-function-deleted`. `loc` is the `default` keyword.
    pub(crate) fn check_defaulted(&mut self, f: DeclId, k: SpecialKind, loc: Loc) {
        self.check_defaulted_impl(f, k, Some(loc), true);
    }

    /// An out-of-line `= default` ([dcl.fct.def.default]/5): ill-formed
    /// if it would be deleted.
    pub(crate) fn check_out_of_line_defaulted(&mut self, f: DeclId, k: SpecialKind) {
        self.check_defaulted_impl(f, k, None, false);
    }

    #[allow(clippy::too_many_lines, reason = "the [dcl.fct.def.default] checks in clang's order")]
    fn check_defaulted_impl(&mut self, f: DeclId, k: SpecialKind, dloc: Option<Loc>, first: bool) {
        use SpecialKind as K;
        let Some(ctx) = self.ast.decls.get(f).ctx() else { return };
        let class = self.class_def(ctx);
        let cflags = self.ast.decls.get(class).flags();
        if cflags.contains(DeclFlags::DEPENDENT_CTX)
            || self.ast.decls.get(f).has(DeclFlags::INVALID)
        {
            return;
        }
        let floc = self.ast.decls.get(f).loc();
        let bits = self.defn_bits(class);
        let mut had_error = false;
        let mut mismatch = false;
        let params = self.param_decls(f);
        let expected = usize::from(!matches!(k, K::DefaultCtor | K::Dtor));
        let info = self.fn_info(f);
        if params.len() != expected {
            let d = self.diag(DiagId::sema_defaulted_default_args, floc).arg(k.index());
            self.emit(d);
            had_error = true;
        } else if info.variadic() {
            if first {
                mismatch = true;
            } else {
                self.unsupported("a variadic explicitly-defaulted special member", floc);
                had_error = true;
            }
        }
        let can_const = match k {
            K::CopyCtor => bits.contains(DefinitionBits::IMPLICIT_COPY_CTOR_CONST_PARAM),
            K::CopyAssign => bits.contains(DefinitionBits::IMPLICIT_COPY_ASSIGN_CONST_PARAM),
            _ => false,
        };
        let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
        if k.is_assign() {
            let ret = self.ast.types.proto(fty).map_or(QualType::ERROR, FuncProto::ret);
            let class_ty = self.class_type(class);
            let expected_ret = self.ast.types.lvalue_ref_to(class_ty);
            if !self.ast.types.same_type(ret, expected_ret) {
                let d = self
                    .diag(DiagId::sema_defaulted_assign_return, floc)
                    .arg(u8::from(k == K::MoveAssign))
                    .arg(self.ty_arg(expected_ret));
                self.emit(d);
                had_error = true;
            }
            let tq = info.this_quals();
            if tq.is_const() || tq.is_volatile() {
                if first {
                    mismatch = true;
                } else {
                    self.unsupported("a cv-qualified explicitly-defaulted assignment", floc);
                    had_error = true;
                }
            }
        }
        let mut has_const_param = false;
        if expected == 1 && params.len() == 1 {
            let pty = self.ast.decls.get(params[0]).ty().unwrap_or(QualType::ERROR);
            match self.ast.types.canonical_view(pty) {
                TypeView::LValueRef(t) | TypeView::RValueRef(t) => {
                    let rq = self.ast.types.effective_quals(t);
                    has_const_param = rq.is_const();
                    if rq.is_volatile() || has_const_param && !can_const {
                        if first {
                            mismatch = true;
                        } else {
                            self.unsupported(
                                "this parameter of an out-of-line explicitly-defaulted special member",
                                floc,
                            );
                            had_error = true;
                        }
                    }
                }
                _ => {
                    self.error(DiagId::sema_defaulted_copy_assign_not_ref, floc);
                    had_error = true;
                }
            }
        }
        let _ = has_const_param;
        let constexpr_ok = Self::defaulted_is_constexpr(bits, k);
        let spec = self.fn_spec(f);
        if spec.constexpr_kind() != kiln_ast::decl::ConstexprKind::None && !constexpr_ok {
            // todo: `default constructor cannot be 'constexpr' in a class
            // with virtual base class` + `virtual base class declared here`.
            self.unsupported(
                "a constexpr defaulted special member of a class with virtual bases",
                floc,
            );
            had_error = true;
        }
        if first {
            if constexpr_ok && spec.constexpr_kind() == kiln_ast::decl::ConstexprKind::None {
                self.ast.decls.mark_function(
                    f,
                    FnSpec::new().with_constexpr_kind(kiln_ast::decl::ConstexprKind::Constexpr),
                );
            }
            if self.written_exc_spec(fty) == ExcSpec::None {
                let t = self.with_exc_spec(fty, ExcSpec::Deferred(f));
                self.ast.decls.set_ty(f, t);
            }
        }
        let deleted = mismatch || self.should_delete(f, k, false);
        if !deleted {
            return;
        }
        if first {
            self.ast.decls.mark_function(f, FnSpec::new().with_deleted(true));
            // Not inside an instantiation (clang `!inTemplateInstantiation()`).
            if !had_error && self.inst.stack.is_empty() {
                let mut d = self.diag(DiagId::sema_defaulted_deleted, floc).arg(k.index());
                if mismatch {
                    d = d.with_note(
                        self.diag(DiagId::note_defaulted_type_mismatch, floc).arg(k.index()),
                    );
                } else {
                    for n in self.deletion_notes(f, k, 0) {
                        d = d.with_note(n);
                    }
                    if let Some(l) = dloc {
                        d = d.with_note(self.diag(DiagId::note_replace_default_with_delete, l));
                    }
                }
                self.emit(d);
            }
        } else {
            let mut d = self.diag(DiagId::sema_out_of_line_default_deletes, floc).arg(k.index());
            for n in self.deletion_notes(f, k, 0) {
                d = d.with_note(n);
            }
            self.emit(d);
        }
    }
}

impl Walk {
    fn member_class(&self) -> DeclId {
        let _ = self.member;
        self.class
    }
}
