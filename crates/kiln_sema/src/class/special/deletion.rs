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
    /// For an inheriting constructor ([class.inhctor.init]/1, clang
    /// `InheritedConstructorInfo`): the bases it initializes by an
    /// inherited constructor (first declaration, constructor called).
    inherited: Option<Vec<(DeclId, DeclId)>>,
}

impl Sema<'_, '_> {
    /// `d` as clang prints a declaration argument: `'name'` (unqualified;
    /// a class or variable template specialization with its arguments,
    /// clang `getNameForDiagnostic`).
    pub(crate) fn decl_arg(&self, d: DeclId) -> String {
        use kiln_ast::types::TypeNames as _;
        let mut s = String::new();
        let names = crate::names::SemaNames::new(self);
        names.write_decl_name(d, false, &mut s);
        if matches!(self.kind_of(d), DeclKind::CXXRecord | DeclKind::Var)
            && let Some((_, info)) = self.ast.spec_info(d)
        {
            kiln_ast::types::write_arg_list(&self.ast.types, &names, info.print_args, &mut s);
        }
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
        if k.is_assign() && bits.contains(DefinitionBits::HAS_INHERITED_ASSIGN) {
            return Selected::Unsupported;
        }
        // clang `LookupSpecialMember`: an argument of the class type (an
        // lvalue for copy, an xvalue for move) without user-defined
        // conversions, and for an assignment an lvalue object.
        let cty = self.class_type(c);
        let mut set = self.begin_set(SetKind::Normal, Loc::INVALID);
        self.defer_templates(&mut set);
        if k != K::DefaultCtor {
            let vk = if k.is_move() { ValueCategory::XValue } else { ValueCategory::LValue };
            let ty = self.ast.types.qualify(cty, rhs);
            set.args.push(ArgInfo::of_type(ty, vk));
        }
        if k.is_ctor() {
            let mut found = std::mem::take(&mut set.found);
            found.clear();
            found.extend(self.usable_ctors(c));
            found.extend(self.ctor_shadows(c));
            self.add_found_candidates(
                &mut set,
                &found,
                CandidateKind::Ctor,
                IcsOptions::SUPPRESS_UDC,
                false,
            );
            set.found = found;
        } else {
            let ty = self.ast.types.qualify(cty, lhs);
            set.object = Some(ArgInfo::of_type(ty, ValueCategory::LValue));
            let mut found = std::mem::take(&mut set.found);
            found.clear();
            found.extend(
                self.ast.lookup_local(c, DeclName::operator(OverloadedOperator::Equal)).iter(),
            );
            self.add_found_candidates(
                &mut set,
                &found,
                CandidateKind::Method,
                IcsOptions::SUPPRESS_UDC,
                false,
            );
            set.found = found;
        }
        let out = self.resolve_set(&mut set);
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

    /// Whether inheriting constructor `dc` is deleted beyond its base
    /// constructor being deleted ([class.inhctor.init]/1 with
    /// [class.default.ctor]/2; clang `findInheritingConstructor`'s
    /// `ShouldDeleteSpecialMember`). Undecidable selections answer no.
    pub(crate) fn inheriting_ctor_should_delete(&mut self, dc: DeclId) -> bool {
        self.deletion(dc, SpecialKind::DefaultCtor, false, 0).0 == DeleteReason::Yes
    }

    /// The notes explaining why defaulted special member `f` is deleted.
    pub(crate) fn deletion_notes(&mut self, f: DeclId, k: SpecialKind, depth: usize) -> Vec<Diag> {
        self.deletion(f, k, true, depth).1
    }

    /// The notes of clang `NoteDeletedFunction` for deleted function `m`.
    pub(crate) fn deleted_function_notes(&mut self, m: DeclId, depth: usize) -> Vec<Diag> {
        // clang `NoteDeletedInheritingConstructor`: a deleted inherited
        // constructor is explained at the base, otherwise the deletion
        // of the inheriting constructor as a default constructor's.
        if let Some(bc) = self.inherited_ctor_base(m) {
            if self.is_deleted_fn(bc) {
                return self.deleted_function_notes(bc, depth);
            }
            if depth >= MAX_NOTE_DEPTH {
                return Vec::new();
            }
            return self.deletion_notes(m, SpecialKind::DefaultCtor, depth + 1);
        }
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
            } else if depth < MAX_NOTE_DEPTH
                && let Some(k) = self.cmp_kind(m)
                && let Some(rd) = self.cmp_class(m)
            {
                // clang `DiagnoseDeletedDefaultedFunction` for a
                // comparison: the analyzer explaining the deletion.
                out.extend(self.analyze_cmp(m, k, rd, rd, true).1);
            }
            return out;
        }
        // An explicit specialization is noted at its own declaration.
        let at = self.deleted_note_decl(m).0;
        vec![
            self.diag(DiagId::note_explicitly_deleted_here, self.ast.decls.get(at).loc())
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
        if bits.contains(DefinitionBits::LAMBDA)
            && matches!(k, K::DefaultCtor | K::CopyAssign)
            && !self.closure_default_constructible(class)
        {
            // [expr.prim.lambda.closure]/13: a closure type with a
            // lambda-capture has a deleted default constructor and copy
            // assignment operator (clang `note_lambda_decl`).
            let notes = if diagnose {
                vec![self.diag(DiagId::note_lambda_decl, self.ast.decls.get(class).loc())]
            } else {
                Vec::new()
            };
            return (DeleteReason::Yes, notes);
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
        // [class.dtor]/7.3: a virtual destructor whose deallocation
        // function lookup finds nothing usable, or an inaccessible one, is
        // deleted (clang `ShouldDeleteSpecialMember` →
        // `FindDeallocationFunction` without diagnostics).
        if k == K::Dtor
            && crate::class::is_virtual(&self.ast, f)
            && !self.virtual_dtor_delete_usable(class)
        {
            if diagnose {
                notes.push(self.diag(
                    DiagId::note_deleted_dtor_no_operator_delete,
                    self.ast.decls.get(class).loc(),
                ));
            }
            return (DeleteReason::Yes, notes);
        }
        let const_arg = matches!(k, K::CopyCtor | K::CopyAssign) && self.copy_param_is_const_ref(f);
        // An inheriting constructor is deleted as a defaulted default
        // constructor would be, except that the inherited bases use their
        // inherited constructors (clang `ShouldDeleteSpecialMember` with
        // an `InheritedConstructorInfo`).
        let inherited = self
            .inherited_ctor_base(f)
            .map(|_| self.inherited_bases(f).into_iter().map(|ib| (ib.base, ib.ctor)).collect());
        let mut w = Walk {
            member: f,
            class,
            kind: k,
            const_arg,
            all_fields_const: true,
            notes: diagnose.then(Vec::new),
            depth,
            inherited,
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
            // [class.inhctor.init]/1: an inherited base is initialized by
            // its inherited constructor; only its deletedness counts (clang
            // checks neither access nor the destructor on this path).
            let first = self.ast.first(bc);
            if let Some(&(_, ctor)) =
                w.inherited.as_ref().and_then(|v| v.iter().find(|e| e.0 == first))
            {
                if !self.is_deleted_fn(ctor) {
                    continue;
                }
                if w.notes.is_some() {
                    let d = self
                        .diag(DiagId::note_deleted_subobject, b.loc)
                        .arg(w.head())
                        .arg(self.decl_arg(w.class))
                        .arg(0u8)
                        .arg(self.ty_arg(b.ty))
                        .arg(1u8)
                        .arg("corresponding constructor");
                    let nested = self.deleted_function_notes(ctor, w.depth + 1);
                    if let Some(n) = w.notes.as_mut() {
                        n.push(d);
                        n.extend(nested);
                    }
                }
                return DeleteReason::Yes;
            }
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
        // [class.default.ctor]/2 before C++26: a union whose members are
        // all const (P3074R7 removes the bullet).
        if self.is_union(w.class)
            && !self.trivial_unions()
            && w.kind == SpecialKind::DefaultCtor
            && w.all_fields_const
            && !self.special_fields(w.class).is_empty()
        {
            if let Some(n) = w.notes.as_mut() {
                let d = self
                    .diag(DiagId::note_deleted_all_const, self.ast.decls.get(w.class).loc())
                    .arg(u8::from(w.inherited.is_some()))
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
        let head = u8::from(w.inherited.is_some());
        match w.kind {
            K::DefaultCtor => {
                if is_ref && !init {
                    self.note(w, |s| {
                        s.diag(DiagId::note_deleted_uninit_field, floc)
                            .arg(head)
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
                            .arg(head)
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
            if w.kind == K::DefaultCtor
                && all_const
                && !ufields.is_empty()
                && !self.trivial_unions()
            {
                let rloc = self.ast.decls.get(rec).loc();
                self.note(w, |s| {
                    s.diag(DiagId::note_deleted_all_const, rloc)
                        .arg(head)
                        .arg(s.decl_arg(class))
                        .arg(1u8)
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
        if let Some(f) = field
            && matches!(w.kind, K::DefaultCtor | K::Dtor)
            && self.trivial_unions()
            && self.is_variant_member(f)
        {
            return self.delete_for_variant_member(w, rec, f, has_init);
        }
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

    /// P3074R7 with CWG3041 (C++26, spec/sema-class.md §4.2): whether
    /// variant member `f` of class type `rec` deletes the defaulted
    /// default constructor or destructor being walked. The default
    /// constructor initializes no variant member without a default member
    /// initializer ([class.default.ctor]/2.3–2.4); the union's destructor
    /// is deleted only when `rec`'s destructor is deleted, inaccessible or
    /// non-trivial and either `f` has a default member initializer or
    /// default-initializing the union fails or selects a deleted or
    /// user-provided constructor ([class.dtor]/7.2 as CWG3041 corrects
    /// "not trivial" to "user-provided"). An anonymous union has no
    /// user-provided constructor.
    fn delete_for_variant_member(
        &mut self,
        w: &mut Walk,
        rec: DeclId,
        f: DeclId,
        has_init: bool,
    ) -> DeleteReason {
        let sub = Subobject::Field(f);
        let dtor = self.select_special(rec, SpecialKind::Dtor, Quals::NONE, Quals::NONE);
        if w.kind == SpecialKind::DefaultCtor {
            return if has_init {
                self.delete_for_call(w, sub, dtor, true)
            } else {
                DeleteReason::No
            };
        }
        let non_trivial = match dtor {
            Selected::Unsupported => return DeleteReason::Unsupported,
            Selected::Found(m) => {
                let is_ok = self.special_accessible_from(m, w.member_class(), false);
                if is_ok && self.fn_spec(m).is_trivial() {
                    return DeleteReason::No;
                }
                is_ok
            }
            _ => false,
        };
        let union = self.ast.decls.get(f).ctx().map_or(w.class, |p| self.class_def(p));
        let anonymous = self.defn_bits(union).contains(DefinitionBits::ANONYMOUS);
        let ctor_bad =
            !(has_init && non_trivial || anonymous) && self.union_default_init_bad(union);
        if !(has_init || ctor_bad) {
            return DeleteReason::No;
        }
        if non_trivial {
            // The variant member's destructor is usable but non-trivial.
            let floc = self.ast.decls.get(f).loc();
            let class = w.class;
            let head = w.head();
            self.note(w, |s| {
                s.diag(DiagId::note_deleted_subobject, floc)
                    .arg(head)
                    .arg(s.decl_arg(class))
                    .arg(2u8)
                    .arg(s.decl_arg(f))
                    .arg(4u8)
                    .arg("destructor")
            });
            return DeleteReason::Yes;
        }
        self.delete_for_call(w, sub, dtor, false)
    }

    /// [class.dtor]/7.2.1 with CWG3041: whether default-initializing
    /// union `u` fails or selects a deleted or user-provided constructor.
    /// Decided from the declared constructors rather than by overload
    /// resolution, which may not be possible while `u`'s implicit members
    /// are being declared: no default constructor when the user declared
    /// a constructor, more than one (ambiguous), or the one there is
    /// deleted or user-provided.
    fn union_default_init_bad(&self, u: DeclId) -> bool {
        let d = self.class_def(u);
        let mut ctors = self
            .ast
            .constructors(d)
            .iter()
            .filter(|&c| self.special_kind_of(c) == Some(SpecialKind::DefaultCtor));
        match (ctors.next(), ctors.next()) {
            (None, _) => {
                self.defn_bits(d).contains(DefinitionBits::USER_DECLARED_CTOR)
                    || self.known_deleted(d, SpecialKind::DefaultCtor)
            }
            (Some(c), None) => self.is_user_provided(c) || self.is_deleted_fn(c),
            (Some(_), Some(_)) => true,
        }
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
            // clang `note_deleted_special_member_class_subobject`: an
            // inheriting constructor says "corresponding constructor" for
            // a deleted one, "default constructor" otherwise.
            let what = if dtor_in_ctor {
                "destructor"
            } else if w.inherited.is_some() && kind == 1 {
                "corresponding constructor"
            } else {
                w.kind.spelling()
            };
            let what = if kind == 2 { format!("{what}s") } else { what.to_owned() };
            let class = w.class;
            let d = match sub {
                Subobject::Field(f) => self
                    .diag(DiagId::note_deleted_subobject, self.ast.decls.get(f).loc())
                    .arg(w.head())
                    .arg(self.decl_arg(class))
                    .arg(if kind == 4 { 2u8 } else { 1u8 })
                    .arg(self.decl_arg(f)),
                Subobject::Base(b) => self
                    .diag(DiagId::note_deleted_subobject, b.loc)
                    .arg(w.head())
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
        let _ = self.check_defaulted_impl(f, k, Some(loc), true);
    }

    /// An out-of-line `= default` ([dcl.fct.def.default]/5): ill-formed
    /// if it would be deleted. True when `f` can be defined.
    pub(crate) fn check_out_of_line_defaulted(&mut self, f: DeclId, k: SpecialKind) -> bool {
        self.check_defaulted_impl(f, k, None, false)
    }

    #[allow(clippy::too_many_lines, reason = "the [dcl.fct.def.default] checks in clang's order")]
    fn check_defaulted_impl(
        &mut self,
        f: DeclId,
        k: SpecialKind,
        dloc: Option<Loc>,
        first: bool,
    ) -> bool {
        use SpecialKind as K;
        let Some(ctx) = self.ast.decls.get(f).ctx() else { return false };
        let class = self.class_def(ctx);
        let cflags = self.ast.decls.get(class).flags();
        if cflags.contains(DeclFlags::DEPENDENT_CTX)
            || self.ast.decls.get(f).has(DeclFlags::INVALID)
        {
            return false;
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
        let mut constexpr_ok = self.defaulted_is_constexpr(class, k, has_const_param);
        // CWG1358: a defaulted member of an instantiation stays constexpr
        // (clang `isTemplateInstantiation`); kiln keeps the virtual-base
        // rule there.
        let instantiated = self.ast.instantiated_from(f).is_some();
        if instantiated && (k.is_assign() || !self.vbases_forbid_constexpr(bits)) {
            constexpr_ok = true;
        }
        let spec = self.fn_spec(f);
        let checked = match self.std {
            s if s >= kiln_lex::Std::Cxx20 => true,
            s if s >= kiln_lex::Std::Cxx14 => k != K::Dtor,
            _ => k.is_ctor(),
        };
        let ck = spec.constexpr_kind();
        if ck != kiln_ast::decl::ConstexprKind::None && !constexpr_ok && checked && !instantiated {
            let at = self.constexpr_begin_locs.get(&f).copied().unwrap_or(floc);
            let consteval = ck == kiln_ast::decl::ConstexprKind::Consteval;
            if !consteval && bits.contains(DefinitionBits::HAS_VIRTUAL_BASES) {
                // [dcl.fct.def.default]/3 with [dcl.constexpr]/3 before
                // C++26 (clang `err_incorrect_defaulted_constexpr_with_vb`,
                // at the declaration's start, a note per virtual base).
                let mut msg =
                    self.diag(DiagId::sema_defaulted_constexpr_with_vbase, at).arg(k.index());
                for (_, b) in self.virtual_bases(class) {
                    msg = msg.with_note(self.diag(DiagId::note_constexpr_virtual_base_here, b.loc));
                }
                self.emit(msg);
            } else {
                // A subobject's member is not constexpr before C++23
                // (P2448R2; clang `err_incorrect_defaulted_constexpr`).
                let msg = self
                    .diag(DiagId::sema_defaulted_constexpr_incorrect, at)
                    .arg(k.index())
                    .arg(u8::from(consteval));
                self.emit(msg);
            }
            // clang `setInvalidDecl`: overload resolution skips it.
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
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
            return !had_error;
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
        false
    }
}

impl Walk {
    /// The member the notes name: the special member kind, or 6
    /// ("constructor inherited by") for an inheriting constructor.
    fn head(&self) -> u8 {
        if self.inherited.is_some() { 6 } else { self.kind.index() }
    }

    fn member_class(&self) -> DeclId {
        let _ = self.member;
        self.class
    }
}
