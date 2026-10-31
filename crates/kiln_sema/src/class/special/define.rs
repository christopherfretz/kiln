//! Implicit definitions of defaulted special member functions
//! ([class.default.ctor]/4, [class.copy.ctor]/12-15,
//! [class.copy.assign]/10-12, [class.dtor]/10; spec/sema-class.md §4.5):
//! a defaulted, non-deleted special member is defined when it is
//! odr-used ([`Sema::mark_function_referenced`]), with clang's bodies:
//! the complete ctor-initializer list of a constructor
//! (`SetCtorInitializers`, shared with written constructors through
//! [`Sema::complete_ctor_inits`]), the memberwise assignments of an
//! assignment operator (`__builtin_memcpy` for trivially copyable
//! arrays), and an empty destructor body whose subobject destructors
//! are odr-used. Diagnostics emitted meanwhile carry the note `in
//! implicit ... for 'T' first required here`.

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::{
    BaseSpec, CtorInit, CtorInitKind, DeclFlags, DeclKind, Defaulted, DefinitionBits, FnSpec,
    FnStorage, LinkageLang, builtin,
};
use kiln_ast::expr::{
    BinaryOp, CastKind, ConstructInfo, ConstructionKind, ExprFlags, ExprKind, NonOdrUse, Radix,
    UnaryOp, ValueCategory,
};
use kiln_ast::names::DeclName;
use kiln_ast::types::{ExcSpec, FuncInfo, Quals, TypeView};
use kiln_ast::{DeclHead, DeclId, ExprId, QualType, StmtId, TypeId};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;
use kiln_source::fx::FxHashMap;

use super::SpecialKind;
use super::deletion::Selected;
use crate::Sema;
use crate::conv::{self, ConvCtx};

/// An implicit special member being defined (clang's
/// `CodeSynthesisContext::DefiningSynthesizedFunction`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SynthContext {
    /// The member (its first declaration).
    pub member: DeclId,
    /// Which special member it is.
    pub kind: SpecialKind,
    /// The use that required the definition.
    pub loc: Loc,
}

/// How a constructor initializes the subobjects it has no
/// mem-initializer for (clang `ImplicitInitializerKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImplicitInit {
    /// Default-initialization ([class.base.init]/9).
    Default,
    /// Memberwise copy of the parameter ([class.copy.ctor]/14).
    Copy,
    /// Memberwise move of the parameter ([class.copy.ctor]/14).
    Move,
}

impl ImplicitInit {
    fn copy_or_move(self) -> bool {
        self != ImplicitInit::Default
    }
}

/// What a constructor call initializes, for its diagnostics (clang's
/// `InitializedEntity` kinds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InitEntity {
    /// A variable or temporary.
    Var,
    /// A base class subobject; `inherited_virtual` for a virtual base that
    /// is not a direct base.
    Base { inherited_virtual: bool },
    /// The member subobject `field`.
    Field(DeclId),
}

/// Where a constructor call's errors are reported (clang's
/// `InitializationKind` and `InitializedEntity` locations).
#[derive(Clone, Debug)]
pub(crate) struct DiagSites {
    /// The access error.
    pub access: Loc,
    /// The deleted-constructor error.
    pub deleted: Loc,
    /// How a deleted constructor is reported.
    pub deleted_as: DeletedAs,
}

/// The form of the deleted-constructor error (clang's
/// `InitializationSequence::Diagnose`).
#[derive(Clone, Debug)]
pub(crate) enum DeletedAs {
    /// "call to deleted constructor", with the deletion notes.
    Call,
    /// As `Call`, plus a note on the parameter an argument initializes.
    Param(Diag),
    /// The error of an explicit cast ("... uses deleted function"),
    /// with the candidate note.
    Cast(Diag),
    /// An implicit conversion by a deleted converting constructor
    /// ("conversion function from ... invokes a deleted function"), with
    /// the note on the parameter the argument initializes, if any.
    Conversion(Option<Diag>),
}

/// The call a selected constructor makes: its arguments and the form
/// of the expression built.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CtorCall<'a> {
    /// The arguments, unconverted.
    pub args: &'a [ExprId],
    /// The construction's flags; `zeroing` is set by [`CtorCall::value_init`].
    pub info: ConstructInfo,
    /// Value-initialization: zero-initialize first unless the
    /// constructor is user-provided ([dcl.init.general]/9.1.2).
    pub value_init: bool,
    /// A `CXXTemporaryObjectExpr` ending at this location (`T(a, b)`,
    /// `T{...}`); otherwise a `CXXConstructExpr`.
    pub temp_end: Option<Loc>,
    /// Converted arguments of non-class type are checked for narrowing
    /// (list-initialization, [dcl.init.list]/3.7).
    pub narrowing: bool,
    /// `explicit` makes the initialization ill-formed (copy-list-init).
    pub reject_explicit: bool,
}

impl<'a> CtorCall<'a> {
    /// A plain complete-object construction from `args`.
    pub(crate) fn new(args: &'a [ExprId]) -> Self {
        CtorCall {
            args,
            info: ConstructInfo::COMPLETE,
            value_init: false,
            temp_end: None,
            narrowing: false,
            reject_explicit: false,
        }
    }
}

/// The state clang's `BaseAndFieldInfo` keeps while completing a
/// ctor-initializer list.
struct InitInfo {
    ctor: DeclId,
    iik: ImplicitInit,
    any_errors: bool,
    /// Written member initializers by field (the last field of an
    /// indirect member's chain).
    fields: FxHashMap<DeclId, CtorInit>,
    /// The active member of each union (first declaration) a written
    /// initializer names.
    active: FxHashMap<DeclId, DeclId>,
    out: Vec<CtorInit>,
}

/// The state saved around an implicit definition.
struct Saved {
    ctx: DeclId,
    func: Option<crate::stmt::FnState>,
    cleanup: super::super::CleanupState,
    unevaluated: u32,
    pending_odr: Vec<ExprId>,
    initializing: Option<DeclId>,
    nsdmi_this: Vec<(DeclId, QualType)>,
    deferring: bool,
}

impl Sema<'_, '_> {
    // -- the code-synthesis context ----------------------------------------

    /// Adds the notes of the implicit definitions in progress to `d`
    /// (clang `PrintContextStack`): once per nesting depth, innermost
    /// first, and not to notes or diagnostics that are not emitted.
    pub(crate) fn with_synth_note(&self, d: Diag) -> Diag {
        let n = self.synth.len();
        if n == 0 || self.synth_noted.get() == n {
            return d;
        }
        let Some(id) = d.id() else { return d };
        if !self.diags.is_enabled_at(id, d.loc()) {
            return d;
        }
        let notes: Vec<Diag> = self
            .synth
            .iter()
            .rev()
            .map(|c| {
                let spec = self.ast.decls.function(c.member).map(|x| x.spec).unwrap_or_default();
                let class = self.ast.decls.get(c.member).ctx().unwrap_or(self.ast.tu);
                self.diag(DiagId::note_member_synthesized_at, c.loc)
                    .arg(u8::from(spec.defaulted() == Defaulted::Explicit))
                    .arg(c.kind.index())
                    .arg(self.ty_arg(self.tag_type(class)))
            })
            .collect();
        self.synth_noted.set(n);
        d.with_context_notes(notes)
    }

    fn push_synth(&mut self, member: DeclId, kind: SpecialKind, loc: Loc) -> Saved {
        self.synth.push(SynthContext { member, kind, loc });
        let saved = Saved {
            ctx: self.ctx,
            func: self.func.take(),
            cleanup: std::mem::take(&mut self.cleanup),
            unevaluated: std::mem::take(&mut self.unevaluated),
            pending_odr: std::mem::take(&mut self.pending_odr),
            initializing: self.initializing.take(),
            nsdmi_this: std::mem::take(&mut self.nsdmi_this),
            deferring: std::mem::take(&mut self.deferring_call_ref),
        };
        self.ctx = member;
        let fty = self.ast.decls.get(member).ty().unwrap_or(QualType::ERROR);
        let ret =
            self.ast.types.proto(fty).map_or(QualType::ERROR, kiln_ast::types::FuncProto::ret);
        let mut st = crate::stmt::FnState::new(member, ret);
        st.this_ty = self.method_this_type(member);
        self.func = Some(st);
        saved
    }

    fn pop_synth(&mut self, saved: Saved) {
        if self.synth_noted.get() == self.synth.len() {
            self.synth_noted.set(0);
        }
        self.synth.pop();
        self.ctx = saved.ctx;
        self.func = saved.func;
        self.cleanup = saved.cleanup;
        self.unevaluated = saved.unevaluated;
        self.pending_odr = saved.pending_odr;
        self.initializing = saved.initializing;
        self.nsdmi_this = saved.nsdmi_this;
        self.deferring_call_ref = saved.deferring;
    }

    // -- implicit definitions ------------------------------------------------

    /// Defines the defaulted special member `f` (its first declaration),
    /// a `kind`, odr-used at `loc` (clang `DefineImplicitDefaultConstructor`
    /// and its siblings). An invalid definition marks `f` invalid and
    /// gives it no body.
    pub(crate) fn define_implicit_special(&mut self, f: DeclId, kind: SpecialKind, loc: Loc) {
        let n = *self.ast.decls.get(f);
        if self.ast.definition(f).is_some()
            || n.flags().contains(DeclFlags::INVALID)
            || self.is_deleted_fn(f)
            || self.synth.iter().any(|c| c.member == f)
        {
            return;
        }
        let Some(class) = n.ctx() else { return };
        let class = self.class_def(class);
        let cflags = self.ast.decls.get(class).flags();
        if cflags.contains(DeclFlags::INVALID)
            || cflags.contains(DeclFlags::DEPENDENT_CTX)
            || !self.defn_bits(class).contains(DefinitionBits::COMPLETE)
        {
            return;
        }
        let saved = self.push_synth(f, kind, loc);
        let ok = match kind {
            SpecialKind::DefaultCtor => self.define_ctor(f, class, ImplicitInit::Default),
            SpecialKind::CopyCtor => self.define_ctor(f, class, ImplicitInit::Copy),
            SpecialKind::MoveCtor => self.define_ctor(f, class, ImplicitInit::Move),
            SpecialKind::CopyAssign | SpecialKind::MoveAssign => self.define_assign(f, class, kind),
            SpecialKind::Dtor => {
                let at = self.ast.decls.get(f).loc();
                self.mark_base_and_member_dtors_referenced(at, class);
                self.check_destructor_delete(f, class);
                true
            }
        };
        if ok {
            let at = self.ast.decls.get(f).loc();
            let body = self.builder().compound(&[], at, at);
            let _ = self.builder().set_body(f, body);
        } else {
            self.ast.decls.set_flags(f, DeclFlags::INVALID);
        }
        self.pop_synth(saved);
    }

    fn define_ctor(&mut self, f: DeclId, class: DeclId, iik: ImplicitInit) -> bool {
        let ok = !self.complete_ctor_inits(f, class, &[], iik, false);
        if ok && iik == ImplicitInit::Default {
            self.check_uninitialized_fields(f);
        }
        ok
    }

    /// clang `SetCtorInitializers`: completes the ctor-initializer list of
    /// constructor `ctor` of `class` from the `written` mem-initializers
    /// ([class.base.init]/9, /13): virtual bases, direct bases, then
    /// members in declaration order, each from its mem-initializer, its
    /// default member initializer or `iik`. Stores the list and odr-uses
    /// the subobject destructors ([class.base.init]/12). Returns whether
    /// an error was diagnosed.
    pub(crate) fn complete_ctor_inits(
        &mut self,
        ctor: DeclId,
        class: DeclId,
        written: &[CtorInit],
        iik: ImplicitInit,
        any_errors: bool,
    ) -> bool {
        let loc = self.ast.decls.get(ctor).loc();
        let mut info = InitInfo {
            ctor,
            iik,
            any_errors,
            fields: FxHashMap::default(),
            active: FxHashMap::default(),
            out: Vec::new(),
        };
        let mut bases: Vec<(DeclId, CtorInit)> = Vec::new();
        for ci in written {
            match ci.kind {
                CtorInitKind::Base | CtorInitKind::VirtualBase => {
                    let t = QualType::unqual(TypeId::new(ci.target));
                    if let Some(r) = self.ast.types.as_record(t) {
                        bases.push((self.ast.first(r), *ci));
                    }
                }
                CtorInitKind::Field => {
                    let f = DeclId::new(ci.target);
                    info.fields.insert(f, *ci);
                    self.note_active_union_member(&mut info, f);
                }
                CtorInitKind::IndirectField => {
                    let chain: Vec<DeclId> = self
                        .ast
                        .lists
                        .iter::<DeclId>(self.ast.indirect_field_chain(DeclId::new(ci.target)))
                        .collect();
                    if let Some(&last) = chain.last() {
                        info.fields.insert(last, *ci);
                    }
                    for f in chain {
                        self.note_active_union_member(&mut info, f);
                    }
                }
                CtorInitKind::Delegating => {}
            }
        }
        let mut had_error = self.complete_base_inits(&mut info, class, bases, loc);
        let members: Vec<DeclId> = self.ast.decls.members(class).collect();
        for m in members {
            match self.kind_of(m) {
                DeclKind::Field => {
                    if self.is_unnamed_bit_field(m)
                        || self.is_anonymous_member(m) && !info.iik.copy_or_move()
                    {
                        continue;
                    }
                    had_error |= self.collect_field_init(&mut info, m, None, loc);
                }
                DeclKind::IndirectField if !info.iik.copy_or_move() => {
                    let ty = self.ast.decls.get(m).ty().unwrap_or(QualType::ERROR);
                    if matches!(self.ast.types.canonical_view(ty), TypeView::IncompleteArray(_)) {
                        continue;
                    }
                    let chain = self.ast.indirect_field_chain(m);
                    let Some(last) = self.ast.lists.iter::<DeclId>(chain).last() else {
                        continue;
                    };
                    had_error |= self.collect_field_init(&mut info, last, Some(m), loc);
                }
                _ => {}
            }
        }
        if !info.out.is_empty() {
            let _ = self.builder().set_ctor_inits(ctor, &info.out);
            self.mark_base_and_member_dtors_referenced(loc, class);
        }
        had_error
    }

    /// The base class initializers of `info.ctor` of `class`, in
    /// initialization order: the written ones (`bases`, by first
    /// declaration of the base class), else implicit ones unless a
    /// mem-initializer failed. Returns whether an error was diagnosed.
    fn complete_base_inits(
        &mut self,
        info: &mut InitInfo,
        class: DeclId,
        mut bases: Vec<(DeclId, CtorInit)>,
        loc: Loc,
    ) -> bool {
        let any_errors = info.any_errors;
        let mut had_error = false;
        let abstract_class = self.defn_bits(class).contains(DefinitionBits::ABSTRACT);
        let direct = self.indexed_bases(class);
        for (vb, spec) in self.virtual_bases(class) {
            let first = self.ast.first(vb);
            if let Some(i) = bases.iter().position(|b| b.0 == first) {
                // todo(WP-067b): warn_abstract_vbase_init_ignored.
                info.out.push(bases.remove(i).1);
            } else if !any_errors && !abstract_class {
                let inherited = !direct.iter().any(|(_, b)| {
                    b.is_virtual && self.base_class(b).is_some_and(|c| self.ast.first(c) == first)
                });
                let idx = self.vbase_spec_index(class, vb, 0).unwrap_or(0);
                match self.implicit_base_init(info.iik, spec, idx, true, inherited, loc) {
                    Some(ci) => info.out.push(ci),
                    None => had_error = true,
                }
            }
        }
        for &(idx, spec) in &direct {
            if spec.is_virtual {
                continue;
            }
            let first = self.base_class(&spec).map(|c| self.ast.first(c));
            if let Some(i) = bases.iter().position(|b| Some(b.0) == first) {
                info.out.push(bases.remove(i).1);
            } else if !any_errors {
                match self.implicit_base_init(info.iik, spec, idx, false, false, loc) {
                    Some(ci) => info.out.push(ci),
                    None => had_error = true,
                }
            }
        }
        had_error
    }

    /// Records `f` as the active member of its union, if its parent is
    /// one and no member is active yet.
    fn note_active_union_member(&self, info: &mut InitInfo, f: DeclId) {
        if let Some(p) = self.ast.decls.get(f).ctx()
            && self.is_union(p)
        {
            info.active.entry(self.ast.first(p)).or_insert(f);
        }
    }

    /// The direct base-specifiers of `class` with their path indices.
    pub(crate) fn indexed_bases(&self, class: DeclId) -> Vec<(u32, BaseSpec)> {
        let def = self.class_def(class);
        let Some(rec) = self.ast.decls.record(def) else { return Vec::new() };
        let idx: Vec<u32> = self.ast.lists.get(rec.bases).to_vec();
        let specs: Vec<BaseSpec> = self.ast.decls.bases(def, &self.ast.lists).copied().collect();
        idx.into_iter().zip(specs).collect()
    }

    /// The path index of the base-specifier naming virtual base `vb`
    /// that [`Sema::virtual_bases`] reports for `r`.
    fn vbase_spec_index(&self, r: DeclId, vb: DeclId, depth: usize) -> Option<u32> {
        if depth > 1024 {
            return None;
        }
        for (i, b) in self.indexed_bases(r) {
            let Some(bd) = self.base_class(&b) else { continue };
            if self.defn_bits(bd).contains(DefinitionBits::HAS_VIRTUAL_BASES)
                && let Some(x) = self.vbase_spec_index(bd, vb, depth + 1)
            {
                return Some(x);
            }
            if b.is_virtual && self.ast.first(bd) == self.ast.first(vb) {
                return Some(i);
            }
        }
        None
    }

    /// clang `isInactiveUnionMember`.
    fn is_inactive_union_member(&self, info: &InitInfo, field: DeclId) -> bool {
        let Some(parent) = self.ast.decls.get(field).ctx() else { return false };
        if !self.is_union(parent) {
            return false;
        }
        if let Some(&a) = info.active.get(&self.ast.first(parent)) {
            return a != field;
        }
        if info.iik.copy_or_move() {
            return true;
        }
        if self.field_has_nsdmi(field) {
            return false;
        }
        if !self.is_anonymous_member(field) {
            return true;
        }
        let ty = self.ast.decls.get(field).ty().unwrap_or(QualType::ERROR);
        !self.ast.types.as_record(ty).is_some_and(|r| self.class_has_nsdmi(r, 0))
    }

    /// clang `CXXRecordDecl::hasInClassInitializer`: a member (or a member
    /// of an anonymous member) has a default member initializer.
    fn class_has_nsdmi(&self, r: DeclId, depth: usize) -> bool {
        if depth > 64 {
            return false;
        }
        self.special_fields(r).into_iter().any(|f| {
            self.field_has_nsdmi(f)
                || self.is_anonymous_member(f)
                    && self
                        .ast
                        .decls
                        .get(f)
                        .ty()
                        .and_then(|t| self.ast.types.as_record(t))
                        .is_some_and(|c| self.class_has_nsdmi(c, depth + 1))
        })
    }

    /// clang `CollectFieldInitializer`: the initializer of `field` (named
    /// through `indirect`, if a member of an anonymous member). Returns
    /// whether an error was diagnosed.
    fn collect_field_init(
        &mut self,
        info: &mut InitInfo,
        field: DeclId,
        indirect: Option<DeclId>,
        loc: Loc,
    ) -> bool {
        if self.ast.decls.get(field).flags().contains(DeclFlags::INVALID) {
            return false;
        }
        if let Some(ci) = info.fields.remove(&field) {
            info.out.push(ci);
            return false;
        }
        let inactive = match indirect {
            None => self.is_inactive_union_member(info, field),
            Some(ind) => {
                let chain: Vec<DeclId> =
                    self.ast.lists.iter::<DeclId>(self.ast.indirect_field_chain(ind)).collect();
                chain.into_iter().any(|f| {
                    self.kind_of(f) == DeclKind::Field && self.is_inactive_union_member(info, f)
                })
            }
        };
        if inactive {
            return false;
        }
        let (target, kind) = match indirect {
            Some(i) => (i, CtorInitKind::IndirectField),
            None => (field, CtorInitKind::Field),
        };
        if self.field_has_nsdmi(field) && !info.iik.copy_or_move() {
            let Some(init) = self.default_member_init(field, info.ctor, loc) else {
                return true;
            };
            info.out.push(CtorInit {
                target: target.index(),
                kind,
                written_index: CtorInit::IMPLICIT,
                init,
                loc,
            });
            return false;
        }
        let fty = self.ast.decls.get(field).ty().unwrap_or(QualType::ERROR);
        if matches!(self.ast.types.canonical_view(fty), TypeView::IncompleteArray(_))
            || self.ast.types.is_array(fty) && self.ast.types.array_size(fty) == Some(0)
        {
            return false;
        }
        if info.any_errors {
            return false;
        }
        match self.implicit_field_init(info.ctor, info.iik, field, loc) {
            Ok(Some(init)) => {
                info.out.push(CtorInit {
                    target: target.index(),
                    kind,
                    written_index: CtorInit::IMPLICIT,
                    init,
                    loc,
                });
                false
            }
            Ok(None) => false,
            Err(()) => true,
        }
    }

    /// clang `BuildCXXDefaultInitExpr`: the use of `field`'s default
    /// member initializer by constructor `ctor` ([class.base.init]/9.1).
    pub(crate) fn default_member_init(
        &mut self,
        field: DeclId,
        ctor: DeclId,
        loc: Loc,
    ) -> Option<ExprId> {
        if self.ast.decls.field(field).and_then(|x| x.init).is_none() {
            // todo(WP-067b): clang's `default member initializer for 'x'
            // needed within definition of enclosing class` (the
            // initializer is not parsed yet).
            self.unsupported("a default member initializer used before it is parsed", loc);
            return None;
        }
        let fty = self.ast.decls.get(field).ty().unwrap_or(QualType::ERROR);
        let (ty, vk) = match self.ast.types.canonical_view(fty) {
            TypeView::LValueRef(_) => (self.non_reference(fty), ValueCategory::LValue),
            TypeView::RValueRef(_) => (self.non_reference(fty), ValueCategory::XValue),
            _ if self.ast.types.as_record(fty).is_some() || self.ast.types.is_array(fty) => {
                (fty, ValueCategory::PRValue)
            }
            _ => (conv::strip_cv(self, fty), ValueCategory::PRValue),
        };
        Some(self.builder().default_init(field, ctor, ty, vk, loc))
    }

    /// A reference to the first parameter of the special member being
    /// defined, as an lvalue of type `ty`.
    fn param_ref(&mut self, p: DeclId, ty: QualType, loc: Loc) -> ExprId {
        self.ast.decls.set_flags(p, DeclFlags::REFERENCED);
        self.mark_used(p);
        self.builder().decl_ref(p, ty, loc, None, NonOdrUse::None, ExprFlags::NONE)
    }

    /// clang `CastForMoving`: `static_cast<T&&>(e)`, an xvalue.
    fn cast_for_moving(&mut self, e: ExprId) -> ExprId {
        let ty = self.non_reference(self.ety(e));
        let loc = self.eloc(e);
        self.builder().cast(
            ExprKind::CXXStaticCastExpr,
            CastKind::NoOp,
            e,
            ty,
            ValueCategory::XValue,
            None,
            loc,
            ExprFlags::NONE,
        )
    }

    /// The first parameter of the special member being defined and its
    /// referenced type.
    fn special_param(&self) -> Option<(DeclId, QualType)> {
        let f = self.func.as_ref()?.decl;
        let p = *self.param_decls(f).first()?;
        let ty = self.ast.decls.get(p).ty()?;
        Some((p, self.non_reference(ty)))
    }

    /// clang `BuildImplicitBaseInitializer` for the base-specifier `spec`
    /// (path index `idx`) of the constructor being defined.
    fn implicit_base_init(
        &mut self,
        iik: ImplicitInit,
        spec: BaseSpec,
        idx: u32,
        is_virtual: bool,
        inherited_virtual: bool,
        loc: Loc,
    ) -> Option<CtorInit> {
        let bclass = self.base_class(&spec)?;
        let ck = if is_virtual {
            ConstructionKind::VirtualBase
        } else {
            ConstructionKind::NonVirtualBase
        };
        let entity = InitEntity::Base { inherited_virtual };
        let init = match iik {
            ImplicitInit::Default => self.construct_special(
                spec.ty,
                bclass,
                SpecialKind::DefaultCtor,
                None,
                ck,
                loc,
                entity,
            )?,
            ImplicitInit::Copy | ImplicitInit::Move => {
                let moving = iik == ImplicitInit::Move;
                let (p, pty) = self.special_param()?;
                let mut arg = self.param_ref(p, pty, loc);
                if moving {
                    arg = self.cast_for_moving(arg);
                }
                let q = self.ast.types.effective_quals(pty);
                let arg_ty =
                    if q.is_empty() { spec.ty } else { self.ast.types.qualify(spec.ty, q) };
                let path = self.ast.lists.push_raw(&[idx]);
                let vk = if moving { ValueCategory::XValue } else { ValueCategory::LValue };
                let arg = conv::raw_cast(
                    self,
                    CastKind::UncheckedDerivedToBase,
                    arg,
                    arg_ty,
                    vk,
                    Some(path),
                );
                let k = if moving { SpecialKind::MoveCtor } else { SpecialKind::CopyCtor };
                self.construct_special(spec.ty, bclass, k, Some(arg), ck, loc, entity)?
            }
        };
        let init = self.finish_cleanups(init);
        Some(CtorInit {
            target: spec.ty.ty().index(),
            kind: if is_virtual { CtorInitKind::VirtualBase } else { CtorInitKind::Base },
            written_index: CtorInit::IMPLICIT,
            init,
            loc,
        })
    }

    /// clang `BuildImplicitMemberInitializer` for `field` of the
    /// constructor `ctor` being completed: `Ok(None)` if the member is
    /// left uninitialized, `Err` after an error.
    fn implicit_field_init(
        &mut self,
        ctor: DeclId,
        iik: ImplicitInit,
        field: DeclId,
        loc: Loc,
    ) -> Result<Option<ExprId>, ()> {
        let fty = self.ast.decls.get(field).ty().unwrap_or(QualType::ERROR);
        if iik.copy_or_move() {
            let moving = iik == ImplicitInit::Move;
            let (p, pty) = self.special_param().ok_or(())?;
            let mut base = self.param_ref(p, pty, loc);
            if moving {
                base = self.cast_for_moving(base);
            }
            let q = self.ast.types.effective_quals(self.ety(base));
            let mut mem = self.field_access(base, field, q, false, loc);
            if matches!(self.ast.types.canonical_view(fty), TypeView::RValueRef(_)) {
                mem = self.cast_for_moving(mem);
            }
            let init = self.init_subobject_from(fty, mem, InitEntity::Field(field), loc)?;
            return Ok(Some(self.finish_cleanups(init)));
        }
        let elem = self.base_element(fty);
        if let Some(r) = self.ast.types.as_record(elem) {
            let Some(c) = self.ast.definition(r) else { return Err(()) };
            let init = self
                .construct_special(
                    fty,
                    c,
                    SpecialKind::DefaultCtor,
                    None,
                    ConstructionKind::Complete,
                    loc,
                    InitEntity::Field(field),
                )
                .ok_or(())?;
            return Ok(Some(self.finish_cleanups(init)));
        }
        let parent_union = self.ast.decls.get(field).ctx().is_some_and(|p| self.is_union(p));
        if !parent_union {
            let what = if self.ast.types.is_reference(fty) {
                Some(0u8)
            } else if self.ast.types.effective_quals(elem).is_const() {
                Some(1)
            } else {
                None
            };
            if let Some(what) = what {
                let class = self.ast.decls.get(ctor).ctx().unwrap_or(self.ast.tu);
                let implicit = self.ast.decls.get(ctor).flags().contains(DeclFlags::IMPLICIT);
                let msg = self
                    .diag(DiagId::sema_init_uninit_member, self.ast.decls.get(ctor).loc())
                    .arg(u8::from(implicit))
                    .arg(self.ty_arg(self.tag_type(class)))
                    .arg(what)
                    .arg(self.decl_arg(field))
                    .with_note(
                        self.diag(DiagId::note_declared_at, self.ast.decls.get(field).loc()),
                    );
                self.emit(msg);
                return Err(());
            }
        }
        Ok(None)
    }

    /// Direct-initialization of a subobject of type `ty` from the glvalue
    /// `arg` of the same type (the memberwise copy or move of
    /// [class.copy.ctor]/14): a reference binds, a class is copied or
    /// moved by its constructor, an array element-wise
    /// (`ArrayInitLoopExpr`), a scalar converted to a prvalue.
    fn init_subobject_from(
        &mut self,
        ty: QualType,
        arg: ExprId,
        entity: InitEntity,
        loc: Loc,
    ) -> Result<ExprId, ()> {
        if self.ast.types.is_reference(ty) {
            return Ok(arg);
        }
        if self.ast.types.is_array(ty) {
            let ove = self.builder().opaque_value(arg);
            let size_t = self.size_type();
            let idx = self.builder().array_init_index(size_t, loc);
            let pty = conv::array_decayed(self, self.ety(ove));
            let decayed = conv::raw_cast(
                self,
                CastKind::ArrayToPointerDecay,
                ove,
                pty,
                ValueCategory::PRValue,
                None,
            );
            let ety = self.ast.types.pointee(pty).unwrap_or(QualType::ERROR);
            let sub = self.builder().subscript(decayed, idx, ety, loc);
            let elem_ty = self.ast.types.element(ty).unwrap_or(QualType::ERROR);
            let elem = self.init_subobject_from(elem_ty, sub, entity, loc)?;
            return Ok(self.builder().array_init_loop(ty, ove, elem));
        }
        if let Some(r) = self.ast.types.as_record(ty) {
            let c = self.ast.definition(r).ok_or(())?;
            let k = if self.ast.exprs.get(arg).value_category() == ValueCategory::XValue {
                SpecialKind::MoveCtor
            } else {
                SpecialKind::CopyCtor
            };
            return self
                .construct_special(ty, c, k, Some(arg), ConstructionKind::Complete, loc, entity)
                .ok_or(());
        }
        Ok(conv::lvalue_to_rvalue(self, arg))
    }

    /// The construction of an object of type `ty` (of class `class`, or an
    /// array of it) by its default (`arg` = `None`), copy or move
    /// constructor `k` from `arg` (clang's `InitializationSequence` for
    /// these forms, with the Q7 selection of [`Sema::select_special`]):
    /// checks access and deletedness, odr-uses the constructor and
    /// builds the `CXXConstructExpr`. `None` after an error.
    #[allow(clippy::too_many_arguments, reason = "clang's InitializationSequence inputs")]
    pub(crate) fn construct_special(
        &mut self,
        ty: QualType,
        class: DeclId,
        k: SpecialKind,
        arg: Option<ExprId>,
        ck: ConstructionKind,
        loc: Loc,
        entity: InitEntity,
    ) -> Option<ExprId> {
        let sites = DiagSites { access: loc, deleted: loc, deleted_as: DeletedAs::Call };
        self.construct_special_at(ty, class, k, arg, ck, loc, entity, sites)
    }

    /// [`Sema::construct_special`] with clang's separate locations for
    /// the access and deletion errors (and the "passing argument to
    /// parameter here" note of an argument).
    #[allow(clippy::too_many_arguments, reason = "clang's InitializationSequence inputs")]
    pub(crate) fn construct_special_at(
        &mut self,
        ty: QualType,
        class: DeclId,
        k: SpecialKind,
        arg: Option<ExprId>,
        ck: ConstructionKind,
        loc: Loc,
        entity: InitEntity,
        sites: DiagSites,
    ) -> Option<ExprId> {
        let rq = arg.map_or(Quals::NONE, |a| self.ast.types.effective_quals(self.ety(a)));
        let sel = self.select_special(class, k, rq, Quals::NONE);
        let args: &[ExprId] = match &arg {
            Some(a) => std::slice::from_ref(a),
            None => &[],
        };
        let call = CtorCall {
            info: ConstructInfo { kind: ck, ..ConstructInfo::COMPLETE },
            ..CtorCall::new(args)
        };
        self.use_ctor(sel, ty, class, k, call, loc, entity, sites)
    }

    /// Diagnoses and builds the call of the constructor selection `sel`
    /// for an object of type `ty` of class `class` (clang's
    /// `InitializationSequence::Perform` / `Diagnose` for constructor
    /// initialization). `k` is the special member asked for (the wording
    /// of a missing default constructor). `None` after an error.
    #[allow(clippy::too_many_arguments, reason = "clang's InitializationSequence inputs")]
    #[allow(clippy::too_many_lines, reason = "one arm per selection outcome")]
    pub(crate) fn use_ctor(
        &mut self,
        sel: Selected,
        ty: QualType,
        class: DeclId,
        k: SpecialKind,
        call: CtorCall<'_>,
        loc: Loc,
        entity: InitEntity,
        sites: DiagSites,
    ) -> Option<ExprId> {
        match sel {
            Selected::Found(m) => {
                if call.reject_explicit && self.fn_spec(m).is_explicit() {
                    let note =
                        self.diag(DiagId::note_explicit_ctor_here, self.ast.decls.get(m).loc());
                    let msg = self
                        .diag(DiagId::sema_ctor_explicit_copy_list, sites.deleted)
                        .with_note(note);
                    self.emit(msg);
                    return None;
                }
                let mk = self.special_kind_of(m).filter(|x| x.is_ctor());
                let sel = mk.map_or(3, SpecialKind::index);
                let class_ty = self.ast.types.canonical(self.class_type(class));
                let msg = match entity {
                    InitEntity::Var => self.special_member_access(m, class, |s, prot| {
                        s.diag(DiagId::sema_access_ctor, sites.access)
                            .arg(u8::from(prot))
                            .arg(s.ty_arg(class_ty))
                    }),
                    InitEntity::Base { inherited_virtual } => {
                        self.subobject_member_access(m, true, |s, prot| {
                            s.diag(DiagId::sema_access_base_ctor, loc)
                                .arg(u8::from(inherited_virtual))
                                .arg(s.ty_arg(ty))
                                .arg(sel)
                                .arg(u8::from(prot))
                        })
                    }
                    InitEntity::Field(_) => self.subobject_member_access(m, false, |s, prot| {
                        s.diag(DiagId::sema_access_field_ctor, loc)
                            .arg(s.ty_arg(s.base_element(ty)))
                            .arg(sel)
                            .arg(u8::from(prot))
                    }),
                };
                if let Some(msg) = msg {
                    self.emit(msg);
                    return None;
                }
                self.mark_function_referenced(m, loc);
                let args = self.ctor_args(m, call.args, call.narrowing)?;
                let zeroing = call.info.zeroing || call.value_init && !self.is_user_provided(m);
                let info = ConstructInfo { zeroing, ..call.info };
                Some(match call.temp_end {
                    Some(end) => self.builder().temporary_object(
                        ty,
                        m,
                        &args,
                        info,
                        ExprFlags::NONE,
                        loc,
                        end,
                    ),
                    None => {
                        self.builder().construct(ty, m, &args, info, ExprFlags::NONE, loc, None)
                    }
                })
            }
            Selected::Deleted(m) => {
                let spec =
                    self.ast.decls.function(self.ast.first(m)).map(|x| x.spec).unwrap_or_default();
                let mk = self.special_kind_of(m).filter(|x| x.is_ctor());
                let (deleted_as, param_note, conversion) = match sites.deleted_as {
                    DeletedAs::Call => (None, None, false),
                    DeletedAs::Param(n) => (None, Some(n), false),
                    DeletedAs::Cast(d) => (Some(d), None, false),
                    DeletedAs::Conversion(n) => (None, n, true),
                };
                if let Some(d) = deleted_as {
                    let Some(mk) = mk else {
                        // todo(WP-068a): clang notes every candidate here.
                        self.unsupported("a cast calling a deleted constructor", loc);
                        return None;
                    };
                    let implicit = if self.ast.decls.get(m).has(DeclFlags::IMPLICIT) {
                        match mk {
                            SpecialKind::DefaultCtor => 1u8,
                            SpecialKind::CopyCtor => 2,
                            _ => 3,
                        }
                    } else {
                        0
                    };
                    let note = self
                        .diag(DiagId::note_ctor_candidate_deleted, self.ast.decls.get(m).loc())
                        .arg(implicit)
                        .arg(u8::from(spec.defaulted() != Defaulted::No));
                    self.emit(d.with_note(note));
                    return None;
                }
                let mut msg = if conversion {
                    let from = call.args.first().map_or(QualType::ERROR, |&a| self.ety(a));
                    self.diag(DiagId::sema_conv_deleted_ctor, sites.deleted)
                        .arg(self.ty_arg(from))
                        .arg(self.ty_arg(ty))
                } else if spec.defaulted() == Defaulted::No {
                    self.diag(DiagId::sema_call_deleted_ctor, sites.deleted)
                        .arg(self.ty_arg(ty))
                        .arg(self.delete_message_suffix(m))
                } else {
                    self.diag(DiagId::sema_call_implicitly_deleted_ctor, sites.deleted)
                        .arg(mk.unwrap_or(k).index())
                        .arg(self.ty_arg(ty))
                };
                for n in self.deleted_function_notes(m, 0) {
                    msg = msg.with_note(n);
                }
                if let Some(n) = param_note {
                    msg = msg.with_note(n);
                }
                self.emit(msg);
                None
            }
            Selected::Missing
                if k == SpecialKind::DefaultCtor
                    && entity != InitEntity::Var
                    && !call.value_init =>
            {
                self.missing_default_ctor(ty, class, entity, loc);
                None
            }
            Selected::Missing | Selected::Ambiguous => {
                let ambiguous = matches!(sel, Selected::Ambiguous);
                let (conversion, param_note) = match sites.deleted_as {
                    DeletedAs::Conversion(n) => (true, n),
                    _ => (false, None),
                };
                let msg = if conversion {
                    let from = call.args.first().map_or(QualType::ERROR, |&a| self.ety(a));
                    let id = if ambiguous {
                        DiagId::sema_ovl_ambiguous_conversion
                    } else {
                        DiagId::sema_no_viable_conversion
                    };
                    self.diag(id, sites.deleted).arg(self.ty_arg(from)).arg(self.ty_arg(ty))
                } else {
                    let id = if ambiguous {
                        DiagId::sema_ovl_ambiguous_ctor
                    } else {
                        DiagId::sema_ovl_no_viable_ctor
                    };
                    self.diag(id, sites.deleted).arg(self.ty_arg(ty))
                };
                let mut msg = match self.ovl.failed.take() {
                    Some(set) => {
                        let m = self.with_ovl_notes(msg, &set, ambiguous);
                        self.end_set(set);
                        m
                    }
                    None => msg,
                };
                if let Some(n) = param_note {
                    msg = msg.with_note(n);
                }
                self.emit(msg);
                None
            }
            Selected::Unsupported => {
                self.unsupported("selecting the constructor", loc);
                None
            }
        }
    }

    /// clang's `err_missing_default_ctor` for a base or member subobject
    /// of the constructor being defined, with its notes.
    fn missing_default_ctor(&mut self, ty: QualType, class: DeclId, entity: InitEntity, loc: Loc) {
        let Some(ctor) = self.func.as_ref().map(|f| f.decl) else { return };
        let outer = self.ast.decls.get(ctor).ctx().unwrap_or(self.ast.tu);
        let implicit = self.ast.decls.get(ctor).flags().contains(DeclFlags::IMPLICIT);
        let ctor_loc = self.ast.decls.get(ctor).loc();
        let msg = self
            .diag(DiagId::sema_init_missing_default_ctor, ctor_loc)
            .arg(u8::from(implicit))
            .arg(self.ty_arg(self.tag_type(outer)));
        let msg = match entity {
            InitEntity::Field(f) => msg
                .arg(1u8)
                .arg(self.decl_arg(f))
                .with_note(self.diag(DiagId::note_member_declared_at, self.ast.decls.get(f).loc())),
            _ => msg.arg(0u8).arg(self.ty_arg(ty)),
        };
        let msg = msg.with_note(
            self.diag(DiagId::note_entity_declared_at, self.ast.decls.get(class).loc())
                .arg(self.decl_arg(class)),
        );
        let _ = loc;
        self.emit(msg);
    }

    /// The arguments of a call of constructor `m` with `args`: each
    /// converted to its parameter's type (copy-initialization,
    /// [expr.call]/7; checked for narrowing if `narrowing`), then the
    /// default arguments, then the variadic arguments promoted.
    pub(crate) fn ctor_args(
        &mut self,
        m: DeclId,
        args: &[ExprId],
        narrowing: bool,
    ) -> Option<Vec<ExprId>> {
        let params = self.param_decls(m);
        let mut out = Vec::new();
        for (i, &a) in args.iter().enumerate() {
            let Some(&p) = params.get(i) else {
                out.push(self.default_argument_promotion(a));
                continue;
            };
            let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
            if narrowing && !self.ast.types.is_reference(pty) {
                self.check_list_narrowing(a, pty);
            }
            let e = self.convert_for_init(a, pty, ConvCtx::Argument);
            if self.is_err_expr(e) {
                return None;
            }
            out.push(e);
        }
        if args.len() < params.len() {
            for &p in &params[args.len()..] {
                let Some(d) = self.ast.decls.param(p).and_then(|x| x.default_arg) else { break };
                let x = *self.ast.exprs.get(d);
                let used = self.ctx;
                out.push(self.builder().default_arg(
                    p,
                    used,
                    x.ty(),
                    x.value_category(),
                    Loc::INVALID,
                ));
            }
        }
        Some(out)
    }

    // -- assignment operators ----------------------------------------------

    /// clang `DefineImplicitCopyAssignment` / `DefineImplicitMoveAssignment`
    /// ([class.copy.assign]/12): each direct base, then each non-static
    /// data member, is assigned from the corresponding subobject of the
    /// parameter; then `return *this;`.
    fn define_assign(&mut self, f: DeclId, class: DeclId, kind: SpecialKind) -> bool {
        let loc = self.ast.decls.get(f).loc();
        let moving = kind == SpecialKind::MoveAssign;
        let Some((p, other_ty)) = self.special_param() else { return false };
        let Some(this_ty) = self.func.as_ref().and_then(|s| s.this_ty) else { return false };
        let obj_ty = self.ast.types.pointee(this_ty).unwrap_or(QualType::ERROR);
        let obj_q = self.ast.types.effective_quals(obj_ty);
        let other_q = self.ast.types.effective_quals(other_ty);
        let mut stmts: Vec<StmtId> = Vec::new();
        let mut invalid = false;
        for (idx, spec) in self.indexed_bases(class) {
            if self.base_class(&spec).is_none() {
                invalid = true;
                continue;
            }
            let path = self.ast.lists.push_raw(&[idx]);
            let from = self.param_ref(p, other_ty, loc);
            let from_ty =
                if other_q.is_empty() { spec.ty } else { self.ast.types.qualify(spec.ty, other_q) };
            let vk = if moving { ValueCategory::XValue } else { ValueCategory::LValue };
            let from = conv::raw_cast(
                self,
                CastKind::UncheckedDerivedToBase,
                from,
                from_ty,
                vk,
                Some(path),
            );
            let this = self.builder().this_expr(this_ty, false, loc);
            let obj =
                self.builder().unary(UnaryOp::Deref, this, obj_ty, loc, ExprFlags::CANNOT_OVERFLOW);
            let to_ty =
                if obj_q.is_empty() { spec.ty } else { self.ast.types.qualify(spec.ty, obj_q) };
            let to = conv::raw_cast(
                self,
                CastKind::UncheckedDerivedToBase,
                obj,
                to_ty,
                ValueCategory::LValue,
                Some(path),
            );
            match self.assign_subobject(spec.ty, to, from, loc) {
                Some(s) => stmts.push(s),
                None => invalid = true,
            }
        }
        let fields: Vec<DeclId> = if self.is_union(class) {
            Vec::new()
        } else {
            self.ast.decls.members(class).filter(|&m| self.kind_of(m) == DeclKind::Field).collect()
        };
        for m in fields {
            if self.is_unnamed_bit_field(m) {
                continue;
            }
            if self.ast.decls.get(m).flags().contains(DeclFlags::INVALID) {
                invalid = true;
                continue;
            }
            let fty = self.ast.decls.get(m).ty().unwrap_or(QualType::ERROR);
            let elem = self.base_element(fty);
            if self.ast.types.is_reference(fty)
                || self.ast.types.as_record(elem).is_none()
                    && self.ast.types.effective_quals(elem).is_const()
            {
                // An implicit assignment operator of such a class is
                // deleted ([class.copy.assign]/7), so never defined.
                self.unsupported("assigning a reference or const member", loc);
                invalid = true;
                continue;
            }
            if matches!(self.ast.types.canonical_view(fty), TypeView::IncompleteArray(_)) {
                continue;
            }
            let mut from = self.param_ref(p, other_ty, loc);
            if moving {
                from = self.cast_for_moving(from);
            }
            let fq = self.ast.types.effective_quals(self.ety(from));
            let from = self.field_access(from, m, fq, false, loc);
            let this = self.builder().this_expr(this_ty, false, loc);
            let to = self.field_access(this, m, obj_q, true, loc);
            match self.assign_subobject(fty, to, from, loc) {
                Some(s) => stmts.push(s),
                None => invalid = true,
            }
        }
        if invalid {
            return false;
        }
        let this = self.builder().this_expr(this_ty, false, loc);
        let obj =
            self.builder().unary(UnaryOp::Deref, this, obj_ty, loc, ExprFlags::CANNOT_OVERFLOW);
        stmts.push(self.builder().return_stmt(Some(obj), None, loc));
        let body = self.builder().compound(&stmts, loc, loc);
        let _ = self.builder().set_body(f, body);
        true
    }

    /// clang `buildSingleCopyAssign`: the assignment of the subobject `to`
    /// of type `ty` from `from`.
    fn assign_subobject(
        &mut self,
        ty: QualType,
        to: ExprId,
        from: ExprId,
        loc: Loc,
    ) -> Option<StmtId> {
        if self.ast.types.is_array(ty) {
            let q = self.ast.types.effective_quals(ty);
            if !q.is_const() && !q.is_volatile() && self.is_trivially_copyable(ty) {
                return Some(self.memcpy_assign(ty, to, from, loc));
            }
            // An array of a class whose selected assignment is trivial is
            // still copied by `memcpy`; anything else needs clang's loop.
            let elem = self.base_element(ty);
            if let Some(c) = self.ast.types.as_record(elem).and_then(|r| self.ast.definition(r)) {
                let fq = self.ast.types.effective_quals(self.ety(from));
                let moving = self.ast.exprs.get(from).value_category() == ValueCategory::XValue;
                let k = if moving { SpecialKind::MoveAssign } else { SpecialKind::CopyAssign };
                if let Selected::Found(op) = self.select_special(c, k, fq, q)
                    && self.ast.decls.function(op).is_some_and(|x| x.spec.is_trivial())
                {
                    self.mark_function_referenced(op, loc);
                    return Some(self.memcpy_assign(ty, to, from, loc));
                }
            }
            // todo(WP-067b): the element-wise loop of
            // [class.copy.assign]/12.2 (clang's `ArrayInitIndexExpr` loop).
            self.unsupported("implicit assignment of an array of this type", loc);
            return None;
        }
        if let Some(r) = self.ast.types.as_record(ty) {
            let c = self.ast.definition(r)?;
            let fq = self.ast.types.effective_quals(self.ety(from));
            let tq = self.ast.types.effective_quals(self.ety(to));
            let moving = self.ast.exprs.get(from).value_category() == ValueCategory::XValue;
            let k = if moving { SpecialKind::MoveAssign } else { SpecialKind::CopyAssign };
            let Selected::Found(op) = self.select_special(c, k, fq, tq) else {
                self.unsupported("selecting the assignment operator", loc);
                return None;
            };
            self.mark_function_referenced(op, loc);
            let callee = self.builder().member(to, op, QualType::BOUND_MEMBER, false, loc, None);
            let params = self.param_decls(op);
            let pty =
                params.first().and_then(|&x| self.ast.decls.get(x).ty()).unwrap_or(QualType::ERROR);
            let arg = self.convert_for_init(from, pty, ConvCtx::Argument);
            let fty = self.ast.decls.get(op).ty().unwrap_or(QualType::ERROR);
            let ret =
                self.ast.types.proto(fty).map_or(QualType::ERROR, kiln_ast::types::FuncProto::ret);
            let (rt, rvk) = self.call_result(ret, loc);
            let call =
                self.builder().member_call(callee, &[arg], rt, rvk, loc, loc, ExprFlags::NONE);
            let call = self.finish_cleanups(call);
            return Some(self.builder().expr_stmt(call));
        }
        let rhs = conv::lvalue_to_rvalue(self, from);
        let lt = self.ety(to);
        let e = self.builder().binary(BinaryOp::Assign, to, rhs, lt, loc);
        Some(self.builder().expr_stmt(e))
    }

    /// clang `QualType::isTriviallyCopyableType` ([basic.types.general]/9).
    pub(crate) fn is_trivially_copyable(&self, ty: QualType) -> bool {
        let elem = self.base_element(ty);
        match self.ast.types.as_record(elem) {
            Some(r) => self.defn_bits(r).contains(DefinitionBits::TRIVIALLY_COPYABLE),
            None => !self.ast.types.is_reference(elem),
        }
    }

    /// clang `buildMemcpyForAssignmentOp`:
    /// `__builtin_memcpy(&to, &from, sizeof(ty))`.
    fn memcpy_assign(&mut self, ty: QualType, to: ExprId, from: ExprId, loc: Loc) -> StmtId {
        let f = self.memcpy_decl();
        let callee = self.builder().decl_ref(
            f,
            QualType::BUILTIN_FN,
            loc,
            None,
            NonOdrUse::None,
            ExprFlags::NONE,
        );
        let callee = self.builtin_fn_to_ptr(callee, f);
        let vp = self.ast.types.pointer_to(QualType::VOID);
        let cv = self.ast.types.qualify(QualType::VOID, Quals::CONST);
        let cvp = self.ast.types.pointer_to(cv);
        let addr = |s: &mut Self, e: ExprId, to_ty: QualType| {
            let pt = s.ast.types.pointer_to(s.ety(e));
            let a = s.builder().unary(UnaryOp::AddrOf, e, pt, loc, ExprFlags::CANNOT_OVERFLOW);
            conv::raw_cast(s, CastKind::BitCast, a, to_ty, ValueCategory::PRValue, None)
        };
        let a0 = addr(self, to, vp);
        let a1 = addr(self, from, cvp);
        let size = crate::target::size_align(&self.ast, ty, &self.abi).map_or(0, |x| x.0);
        let size_t = self.size_type();
        let a2 = self.builder().int_literal(size_t, size, Radix::Decimal, loc);
        let call = self.builder().call(
            callee,
            &[a0, a1, a2],
            vp,
            ValueCategory::PRValue,
            loc,
            loc,
            ExprFlags::NONE,
        );
        self.builder().expr_stmt(call)
    }

    /// The internal `__builtin_memcpy` declaration (clang declares it on
    /// first use, as an implicit builtin in an implicit `extern "C"`).
    fn memcpy_decl(&mut self) -> DeclId {
        if let Some(f) = self.memcpy_decl {
            return f;
        }
        let loc = Loc::INVALID;
        let tu = self.ast.tu;
        let lh = DeclHead::new(tu, DeclName::EMPTY, loc).flags(DeclFlags::IMPLICIT);
        let ls = self.builder().linkage_spec_head(lh, LinkageLang::C);
        let vp = self.ast.types.pointer_to(QualType::VOID);
        let cv = self.ast.types.qualify(QualType::VOID, Quals::CONST);
        let cvp = self.ast.types.pointer_to(cv);
        let size_t = self.size_type();
        let params = [vp, cvp, size_t];
        let ty = self.ast.types.function(vp, &params, FuncInfo::new(), ExcSpec::BasicNoexcept);
        let name = self.ast.symbols().intern("__builtin_memcpy");
        let h = DeclHead::new(ls, DeclName::ident(name), loc).flags(DeclFlags::IMPLICIT);
        let spec = FnSpec::new().with_storage(FnStorage::Extern);
        let f = self.builder().function(h, DeclKind::Function, ty, spec, None);
        let _ = self.ast.decls.cold.builtin_id.insert(f, builtin::MEMCPY);
        let ps: Vec<DeclId> = params
            .iter()
            .enumerate()
            .map(|(i, &t)| {
                #[allow(clippy::cast_possible_truncation, reason = "three parameters")]
                self.builder().param(f, DeclName::EMPTY, loc, t, i as u16, 0)
            })
            .collect();
        let _ = self.builder().set_params(f, &ps);
        let implicit = |kind, a| AttrNode { kind, syntax: AttrSyntax::Gnu, flags: 1, loc, a, b: 0 };
        let attrs = [
            self.ast.attrs.push(implicit(AttrKind::BUILTIN, builtin::MEMCPY)),
            self.ast.attrs.push(implicit(AttrKind::NOTHROW, 0)),
        ];
        let _ = self.builder().attach_attrs(f, &attrs);
        self.mark_used(f);
        self.memcpy_decl = Some(f);
        f
    }

    // -- destructors -------------------------------------------------------

    /// clang `MarkBaseAndMemberDestructorsReferenced` ([class.base.init]/12,
    /// [class.dtor]/13): the destructors of the potentially constructed
    /// subobjects of `class` are odr-used at `loc`, and must be
    /// accessible and not deleted.
    pub(crate) fn mark_base_and_member_dtors_referenced(&mut self, loc: Loc, class: DeclId) {
        let class = self.class_def(class);
        if self.ast.decls.get(class).flags().contains(DeclFlags::DEPENDENT_CTX)
            || self.is_union(class)
        {
            return;
        }
        let fields: Vec<DeclId> =
            self.ast.decls.members(class).filter(|&m| self.kind_of(m) == DeclKind::Field).collect();
        for f in fields {
            if self.ast.decls.get(f).flags().contains(DeclFlags::INVALID) {
                continue;
            }
            let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
            if matches!(self.ast.types.canonical_view(fty), TypeView::IncompleteArray(_))
                || self.ast.types.is_array(fty) && self.ast.types.array_size(fty) == Some(0)
            {
                continue;
            }
            let Some(fc) = self.element_class(fty) else { continue };
            if self.ast.decls.get(fc).flags().contains(DeclFlags::INVALID)
                || self.defn_bits(fc).contains(DefinitionBits::TRIVIAL_DTOR)
                || self.is_union(fc) && self.is_anonymous_member(f)
            {
                continue;
            }
            let (Selected::Found(dtor) | Selected::Deleted(dtor)) =
                self.select_special(fc, SpecialKind::Dtor, Quals::NONE, Quals::NONE)
            else {
                continue;
            };
            let floc = self.ast.decls.get(f).loc();
            let elem = self.base_element(fty);
            let msg = self.special_member_access(dtor, fc, |s, prot| {
                s.diag(DiagId::sema_dtor_access_field, floc).arg(s.ty_arg(elem)).arg(u8::from(prot))
            });
            if let Some(msg) = msg {
                self.emit(msg);
            }
            self.mark_function_referenced(dtor, loc);
            self.diagnose_deleted_use(dtor, loc);
        }
        let visit_virtual = !self.defn_bits(class).contains(DefinitionBits::ABSTRACT);
        let mut direct_virtual: Vec<DeclId> = Vec::new();
        for (_, spec) in self.indexed_bases(class) {
            let Some(bc) = self.base_class(&spec) else { continue };
            if spec.is_virtual {
                if !visit_virtual {
                    continue;
                }
                direct_virtual.push(self.ast.first(bc));
            }
            if let Some(dtor) = self.subobject_dtor(bc) {
                let msg = self.subobject_member_access(dtor, true, |s, prot| {
                    s.diag(DiagId::sema_dtor_access_base, spec.loc)
                        .arg(s.ty_arg(spec.ty))
                        .arg(u8::from(prot))
                });
                if let Some(msg) = msg {
                    self.emit(msg);
                }
                self.mark_function_referenced(dtor, loc);
                self.diagnose_deleted_use(dtor, loc);
            }
        }
        if !visit_virtual {
            return;
        }
        for (vb, _) in self.virtual_bases(class) {
            if direct_virtual.contains(&self.ast.first(vb)) {
                continue;
            }
            if let Some(dtor) = self.subobject_dtor(vb) {
                // todo(WP-067b): err_access_dtor_vbase.
                self.mark_function_referenced(dtor, loc);
                self.diagnose_deleted_use(dtor, loc);
            }
        }
    }

    /// The destructor of subobject class `c` whose use is relevant (not
    /// trivial, `c` valid).
    fn subobject_dtor(&mut self, c: DeclId) -> Option<DeclId> {
        if self.ast.decls.get(c).flags().contains(DeclFlags::INVALID)
            || self.defn_bits(c).contains(DefinitionBits::TRIVIAL_DTOR)
        {
            return None;
        }
        match self.select_special(c, SpecialKind::Dtor, Quals::NONE, Quals::NONE) {
            Selected::Found(d) | Selected::Deleted(d) => Some(d),
            _ => None,
        }
    }
}
