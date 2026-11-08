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

use kiln_ast::decl::{
    Access, BaseSpec, CtorInit, CtorInitKind, DeclFlags, DeclKind, Defaulted, DefinitionBits,
    InitStyle, VarSpec,
};
use kiln_ast::expr::{
    BinaryOp, CastKind, ConstructInfo, ConstructionKind, ExprFlags, ExprKind, NonOdrUse, Radix,
    UnaryOp, ValueCategory,
};
use kiln_ast::names::DeclName;
use kiln_ast::stmt::LoopData;
use kiln_ast::types::{Quals, TypeView};
use kiln_ast::{DeclHead, DeclId, ExprId, QualType, StmtId, TypeId};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;
use kiln_source::fx::{FxHashMap, FxHashSet};

use super::SpecialKind;
use super::deletion::Selected;
use crate::Sema;
use crate::conv::{self, ConvCtx};
use crate::lookup::LookupKind;
use crate::type_diff::DiffArgs as _;

/// An implicit special member being defined (clang's
/// `CodeSynthesisContext::DefiningSynthesizedFunction`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SynthContext {
    /// The member (its first declaration; for a defaulted comparison, the
    /// declaration defaulted).
    pub member: DeclId,
    /// Which function it is.
    pub kind: SynthKind,
    /// The use that required the definition.
    pub loc: Loc,
    /// The instantiation frames already active when it was pushed: the
    /// ones pushed later are inside it, so their notes print before its
    /// own (clang keeps both kinds on one `CodeSynthesisContexts` stack).
    pub inst_depth: usize,
}

/// What [`SynthContext::member`] is (clang `DefaultedFunctionKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SynthKind {
    /// A special member function.
    Special(SpecialKind),
    /// A defaulted comparison operator ([class.compare.default]).
    Comparison(crate::class::compare::CmpKind),
    /// An inheriting constructor ([class.inhctor.init]; clang prints no
    /// note for it).
    Inheriting,
    /// The evaluation of a special member's implicit exception
    /// specification (clang `ExceptionSpecEvaluation`, [except.spec]/8).
    ExceptionSpec,
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
    /// with the candidate note; the cast's `%select` index (no viable
    /// or an ambiguous constructor: clang's "no matching conversion for
    /// ..." / "ambiguous conversion for ...", `tryDiagnoseOverloadedCast`).
    Cast(Diag, &'static str),
    /// An implicit conversion by a deleted converting constructor
    /// ("conversion function from ... invokes a deleted function"), with
    /// the note on the parameter the argument initializes, if any.
    Conversion(Option<Diag>),
    /// As `Conversion(None)`, for the operand of a `return`: without a
    /// viable constructor, clang's "no viable conversion from returned
    /// value of type ..." (`err_typecheck_nonviable_condition` with the
    /// `InitializedEntity::EK_Result` select; WP-183).
    Return,
    /// The [dcl.init.ref]/5.4.1 temporary of a non-class initializer
    /// ("attempt to use a deleted function", clang's ICS-performed
    /// conversion).
    Use,
}

/// The call a selected constructor makes: its arguments and the form
/// of the expression built.
#[derive(Clone, Copy, Debug)]
#[allow(clippy::struct_excessive_bools, reason = "independent flags of one construction")]
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
    /// The `)` of a parenthesized initializer of a `CXXConstructExpr`
    /// (`T x(a)`): kept so instantiation redoes it as written.
    pub rparen: Option<Loc>,
    /// Converted arguments of non-class type are checked for narrowing
    /// (list-initialization, [dcl.init.list]/3.7).
    pub narrowing: bool,
    /// `explicit` makes the initialization ill-formed (copy-list-init).
    pub reject_explicit: bool,
    /// A direct-initialization: the one argument of a copy or move
    /// constructor binds its reference through explicit conversion
    /// functions too ([over.match.copy]/1.2, [over.match.ref]/1.1; the
    /// candidate's sequence was formed so by
    /// [`crate::overload::IcsOptions::DIRECT_COPY_CTOR`]).
    pub direct: bool,
    /// A converting constructor of a user-defined conversion
    /// ([over.match.copy], [over.match.ref]): a deprecated one is
    /// diagnosed at `DiagSites::access`, the variable or argument (clang
    /// `SK_UserConversion`'s `Kind.getLocation()`).
    pub udc: bool,
}

impl<'a> CtorCall<'a> {
    /// A plain complete-object construction from `args`.
    pub(crate) fn new(args: &'a [ExprId]) -> Self {
        CtorCall {
            args,
            info: ConstructInfo::COMPLETE,
            value_init: false,
            temp_end: None,
            rparen: None,
            narrowing: false,
            reject_explicit: false,
            direct: false,
            udc: false,
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
pub(crate) struct Saved {
    ctx: DeclId,
    func: Option<crate::stmt::FnState>,
    cleanup: super::super::CleanupState,
    unevaluated: u32,
    pending_odr: Vec<ExprId>,
    imm: crate::consteval::SavedImm,
    initializing: Option<DeclId>,
    nsdmi_this: Vec<(DeclId, QualType)>,
    contract_ctx: Vec<crate::contracts::ContractCtx>,
    deferring: bool,
    /// An alias template's use context does not reach into the member
    /// being defined: its own subobject accesses are checked from it.
    access_ctx: Option<DeclId>,
}

impl Sema<'_, '_> {
    // -- the code-synthesis context ----------------------------------------

    /// The notes of the implicit definitions in progress for `d` (clang
    /// `PrintContextStack`): once per nesting depth, innermost first, and
    /// none for notes or diagnostics that are not emitted.
    pub(crate) fn synth_notes(&self, d: &Diag) -> Vec<Diag> {
        let n = self.synth.len();
        if n == 0 || self.synth_noted.get() == n {
            return Vec::new();
        }
        let Some(id) = d.id() else { return Vec::new() };
        if !self.diags.is_enabled_at(id, d.loc()) {
            return Vec::new();
        }
        let notes: Vec<Diag> = self
            .synth
            .iter()
            .rev()
            .filter(|c| c.kind != SynthKind::Inheriting)
            .map(|c| match c.kind {
                SynthKind::Special(kind) => {
                    // "defaulted" for an explicit `= default`, in the class,
                    // out of line, or on the pattern's out-of-line
                    // definition of an instantiated member.
                    let pattern = self.ast.instantiated_from(c.member);
                    let explicit = self
                        .ast
                        .redecls_of(c.member)
                        .chain(pattern.into_iter().flat_map(|p| self.ast.redecls_of(p)))
                        .any(|d| {
                            self.ast
                                .decls
                                .function(d)
                                .is_some_and(|x| x.spec.defaulted() == Defaulted::Explicit)
                        });
                    let class = self.ast.decls.get(c.member).ctx().unwrap_or(self.ast.tu);
                    self.diag(DiagId::note_member_synthesized_at, c.loc)
                        .arg(u8::from(explicit))
                        .arg(kind.index())
                        .arg(self.ty_arg(self.tag_type(class)))
                }
                SynthKind::ExceptionSpec => self
                    .diag(DiagId::note_exception_spec_evaluation_here, c.loc)
                    .arg(self.qualified_decl_name(c.member)),
                SynthKind::Comparison(_) | SynthKind::Inheriting => {
                    let SynthKind::Comparison(k) = c.kind else { unreachable!("filtered") };
                    let class = self.cmp_class(c.member).unwrap_or(self.ast.tu);
                    self.diag(DiagId::note_defaulted_cmp_synthesized_at, c.loc)
                        .arg(k.index())
                        .arg(self.ty_arg(self.tag_type(class)))
                }
            })
            .collect();
        self.synth_noted.set(n);
        notes
    }

    pub(crate) fn push_synth(&mut self, member: DeclId, kind: SynthKind, loc: Loc) -> Saved {
        let inst_depth = self.inst.stack.len();
        self.synth.push(SynthContext { member, kind, loc, inst_depth });
        let saved = Saved {
            ctx: self.ctx,
            func: self.func.take(),
            cleanup: std::mem::take(&mut self.cleanup),
            unevaluated: std::mem::take(&mut self.unevaluated),
            pending_odr: std::mem::take(&mut self.pending_odr),
            imm: self.take_immediate_state(true),
            initializing: self.initializing.take(),
            nsdmi_this: std::mem::take(&mut self.nsdmi_this),
            contract_ctx: std::mem::take(&mut self.contract_ctx),
            deferring: std::mem::take(&mut self.deferring_call_ref),
            access_ctx: self.access_ctx.take(),
        };
        self.ctx = member;
        let fty = self.ast.decls.get(member).ty().unwrap_or(QualType::ERROR);
        let ret = self.sugared_proto(fty).map_or(QualType::ERROR, |p| p.ret());
        let mut st = crate::stmt::FnState::new(member, ret);
        st.this_ty = self.method_this_type(member);
        self.func = Some(st);
        saved
    }

    pub(crate) fn pop_synth(&mut self, saved: Saved) {
        if self.synth_noted.get() == self.synth.len() {
            self.synth_noted.set(0);
        }
        self.synth.pop();
        self.ctx = saved.ctx;
        self.func = saved.func;
        self.cleanup = saved.cleanup;
        self.unevaluated = saved.unevaluated;
        self.pending_odr = saved.pending_odr;
        self.restore_immediate_state(saved.imm);
        self.initializing = saved.initializing;
        self.nsdmi_this = saved.nsdmi_this;
        self.contract_ctx = saved.contract_ctx;
        self.deferring_call_ref = saved.deferring;
        self.access_ctx = saved.access_ctx;
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
        // clang: the exception specification is needed because the
        // function is being defined (`ResolveExceptionSpec` before the
        // definition's context note), so the default member initializers
        // it instantiates note the exception-specification evaluation.
        self.resolve_exception_spec(f, loc);
        let saved = self.push_synth(f, SynthKind::Special(kind), loc);
        if matches!(kind, SpecialKind::CopyCtor | SpecialKind::CopyAssign)
            && self.std >= kiln_lex::Std::Cxx11
            && n.flags().contains(DeclFlags::IMPLICIT)
        {
            self.diagnose_deprecated_copy(class, kind == SpecialKind::CopyAssign);
        }
        let ok = match kind {
            SpecialKind::DefaultCtor => self.define_ctor(f, class, ImplicitInit::Default),
            SpecialKind::CopyCtor => self.define_ctor(f, class, ImplicitInit::Copy),
            SpecialKind::MoveCtor => self.define_ctor(f, class, ImplicitInit::Move),
            SpecialKind::CopyAssign | SpecialKind::MoveAssign => self.define_assign(f, class, kind),
            SpecialKind::Dtor => {
                let at = self.ast.decls.get(f).loc();
                self.mark_base_and_member_dtors_referenced(at, class);
                self.check_destructor_delete(f, class);
                self.mark_vtable_used(class, loc);
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

    /// [depr.impldec]/1: the definition of an implicitly declared copy
    /// constructor (`assign` false) or copy assignment operator of
    /// `class` is deprecated if the class has a user-declared destructor
    /// or the other copy operation is user-declared (clang's
    /// `diagnoseDeprecatedCopyOperation`, `-Wdeprecated-copy` and its
    /// `-with-dtor`/`-with-user-provided-*` siblings), at that member.
    fn diagnose_deprecated_copy(&mut self, class: DeclId, assign: bool) {
        let bits = self.defn_bits(class);
        let want = if bits.contains(DefinitionBits::USER_DECLARED_DTOR) {
            SpecialKind::Dtor
        } else if assign && bits.contains(DefinitionBits::USER_DECLARED_COPY_CTOR) {
            SpecialKind::CopyCtor
        } else if !assign && bits.contains(DefinitionBits::USER_DECLARED_COPY_ASSIGN) {
            SpecialKind::CopyAssign
        } else {
            return;
        };
        let Some(op) = self
            .ast
            .decls
            .members(class)
            .find(|&m| self.is_user_declared(m) && self.special_kind_of(m) == Some(want))
        else {
            return;
        };
        let id = match (self.is_user_provided(op), want == SpecialKind::Dtor) {
            (true, true) => DiagId::sema_deprecated_copy_with_user_provided_dtor,
            (true, false) => DiagId::sema_deprecated_copy_with_user_provided_copy,
            (false, true) => DiagId::sema_deprecated_copy_with_dtor,
            (false, false) => DiagId::sema_deprecated_copy,
        };
        let at = self.ast.decls.get(op).loc();
        // Once per implicit member, so the copy constructor's and the
        // copy assignment's warnings at the same member are both shown.
        let d = self.diag(id, at).arg(self.ty_arg(self.tag_type(class))).arg(u8::from(assign));
        self.emit(d.allow_duplicate());
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
        // clang `InstantiateFunctionDefinition` moves an instantiated
        // member to its pattern's definition, so an out-of-line one
        // locates the implicit initializations there.
        let loc = self
            .ast
            .instantiated_from(ctor)
            .and_then(|p| self.ast.definition(p))
            .map_or_else(|| self.ast.decls.get(ctor).loc(), |d| self.ast.decls.get(d).loc());
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
        }
        // [class.base.init]/12: every constructor potentially invokes the
        // destructors of the subobjects, written initializers or not.
        self.mark_base_and_member_dtors_referenced(loc, class);
        // [basic.def.odr]/8: a constructor definition uses the vtable (at
        // the use requiring an implicit definition).
        let use_loc = self.synth.last().filter(|c| c.member == ctor).map_or(loc, |c| c.loc);
        self.mark_vtable_used(class, use_loc);
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
                let idx = self.vbase_spec_index(class, vb).unwrap_or(0);
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
    fn vbase_spec_index(&self, r: DeclId, vb: DeclId) -> Option<u32> {
        let mut walked: FxHashSet<DeclId> = FxHashSet::default();
        self.vbase_spec_index_in(r, vb, &mut walked, 0)
    }

    /// [`Self::vbase_spec_index`] below `r`, skipping the classes already
    /// `walked` (they do not name `vb`; each class is walked once, WP-400).
    fn vbase_spec_index_in(
        &self,
        r: DeclId,
        vb: DeclId,
        walked: &mut FxHashSet<DeclId>,
        depth: usize,
    ) -> Option<u32> {
        if depth > 1024 {
            return None;
        }
        for (i, b) in self.indexed_bases(r) {
            let Some(bd) = self.base_class(&b) else { continue };
            if self.defn_bits(bd).contains(DefinitionBits::HAS_VIRTUAL_BASES)
                && walked.insert(self.ast.first(bd))
                && let Some(x) = self.vbase_spec_index_in(bd, vb, walked, depth + 1)
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
            // Its immediate invocations are evaluated here
            // ([expr.const]/16; clang's rebuilt `CXXDefaultInitExpr`).
            let ctor = info.ctor;
            if !self.in_template() && !self.is_consteval_kind(ctor) {
                let escalating = self.is_immediate_escalating(ctor).then_some(ctor);
                self.check_default_use(field, loc, escalating, true);
            }
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
        self.instantiate_nsdmi(field, loc);
        if self.ast.decls.get(field).has(DeclFlags::INVALID) {
            return None;
        }
        if self.ast.decls.field(field).and_then(|x| x.init).is_none() {
            self.default_member_init_not_yet_parsed(field, loc);
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
        let e = self.builder().default_init(field, ctor, ty, vk, loc);
        self.note_default_init_use(e, field, loc, ctor);
        Some(e)
    }

    /// clang `BuildCXXDefaultInitExpr` for an initializer not parsed yet
    /// (it is parsed at the end of the outermost enclosing class,
    /// [class.mem.general]/8): diagnosed at `loc` with the outermost
    /// lexically enclosing class, the note at the field's name; the field
    /// becomes invalid outside a SFINAE context so later uses are quiet.
    pub(crate) fn default_member_init_not_yet_parsed(&mut self, field: DeclId, loc: Loc) {
        let mut outer = self.ast.decls.get(field).ctx();
        while let Some(c) = outer
            && let Some(p) = self.ast.decls.lexical_ctx(c)
            && self.kind_of(p) == DeclKind::CXXRecord
        {
            outer = Some(p);
        }
        let class = outer.map(|c| self.decl_arg(c)).unwrap_or_default();
        let name = self.decl_arg(field);
        let at = self.ast.decls.get(field).loc();
        let note = self.diag(DiagId::note_default_member_init_not_yet_parsed, at);
        let d =
            self.diag(DiagId::sema_default_member_init_not_yet_parsed, loc).arg(class).arg(name);
        // One per field at the same constructor (clang does not
        // deduplicate; the field is invalid afterwards).
        self.emit(d.with_note(note).allow_duplicate());
        if !self.inst.stack.is_sfinae() {
            self.ast.decls.set_flags(field, DeclFlags::INVALID);
        }
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

    /// The end of an implicit mem-initializer `init`, a full-expression
    /// ([intro.execution]/5.4): its immediate invocations are evaluated,
    /// escalating the defaulted constructor being defined (clang's
    /// `SynthesizedFunctionScope` context, [expr.const]/18), and its
    /// temporaries destroyed.
    /// `field` names the member it initializes.
    fn finish_implicit_init(&mut self, init: ExprId, field: Option<DeclId>) -> ExprId {
        if let Some(f) = field {
            self.note_member_init_full_expr(f, false);
        }
        self.finish_immediate_invocations(init);
        self.finish_cleanups(init)
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
        let init = self.finish_implicit_init(init, None);
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
            return Ok(Some(self.finish_implicit_init(init, Some(field))));
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
            return Ok(Some(self.finish_implicit_init(init, Some(field))));
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
    pub(crate) fn init_subobject_from(
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
            // A class of a template definition (a local or member class
            // of a template) declares its special members only when
            // instantiated (clang `CanDeclareSpecialMemberFunction`): a
            // by-copy capture or structured binding of it in the pattern
            // is initialized when the template is instantiated.
            if self.ast.decls.get(c).has(DeclFlags::DEPENDENT_CTX) {
                return Ok(arg);
            }
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
        let named = self.ctor_diag_ty.take().unwrap_or(ty);
        match sel {
            Selected::Found(m) => {
                if call.reject_explicit && self.fn_spec(m).is_explicit() {
                    let note =
                        self.diag(DiagId::note_explicit_ctor_here, self.ast.decls.get(m).loc());
                    let mut msg = self
                        .diag(DiagId::sema_ctor_explicit_copy_list, sites.deleted)
                        .with_note(note);
                    if let DeletedAs::Param(n) = sites.deleted_as {
                        msg = msg.with_note(n);
                    }
                    self.emit(msg);
                    return None;
                }
                // An inheriting constructor is accessed, and takes its
                // arguments (with default arguments), as the base
                // constructor ([namespace.udecl]/19, [class.inhctor.init]).
                let bc = self.inherited_ctor_base(m);
                let am = bc.unwrap_or(m);
                let aclass = bc.and_then(|b| self.ast.decls.get(b).ctx()).unwrap_or(class);
                let mk = self.special_kind_of(m).filter(|x| x.is_ctor() && bc.is_none());
                let sel = mk.map_or(3, SpecialKind::index);
                let class_ty = self.ast.types.canonical(self.class_type(aclass));
                let msg = match entity {
                    // The object of an inheriting constructor's access
                    // is the derived class it constructs.
                    InitEntity::Var => {
                        self.special_member_access_on(am, aclass, class, |s, prot| {
                            s.diag(DiagId::sema_access_ctor, sites.access)
                                .arg(u8::from(prot))
                                .arg(s.ty_arg(class_ty))
                        })
                    }
                    InitEntity::Base { inherited_virtual } => {
                        self.subobject_member_access(am, true, |s, prot| {
                            s.diag(DiagId::sema_access_base_ctor, loc)
                                .arg(u8::from(inherited_virtual))
                                .arg(s.ty_arg(ty))
                                .arg(sel)
                                .arg(u8::from(prot))
                        })
                    }
                    InitEntity::Field(_) => self.subobject_member_access(am, false, |s, prot| {
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
                // [dcl.attr.deprecated]/4: the selected constructor is
                // used (an inheriting one is not checked here).
                if bc.is_none() {
                    self.warn_deprecated_use(m, if call.udc { sites.access } else { loc });
                }
                self.mark_function_referenced(m, loc);
                let explicit_first = call.direct
                    && bc.is_none()
                    && call.args.len() == 1
                    && !self.is_function_spec(m)
                    && self.copy_or_move_ctor_param(m, class);
                let args = self.ctor_args(am, call.args, call.narrowing, explicit_first, loc)?;
                self.check_diagnose_if_call(am, &args, loc);
                let zeroing = call.info.zeroing || call.value_init && !self.is_user_provided(m);
                let info = ConstructInfo { zeroing, ..call.info };
                let e = match call.temp_end {
                    Some(end) => self.builder().temporary_object(
                        ty,
                        m,
                        &args,
                        info,
                        ExprFlags::NONE,
                        loc,
                        end,
                    ),
                    None => self.builder().construct(
                        ty,
                        m,
                        &args,
                        info,
                        ExprFlags::NONE,
                        loc,
                        call.rparen,
                    ),
                };
                Some(self.check_immediate_invocation(e, m, None))
            }
            Selected::Deleted(m) => {
                if let Some(bc) = self.inherited_ctor_base(m)
                    && !self.is_deleted_fn(bc)
                {
                    // clang `DiagnoseUseOfDecl` of a deleted inheriting
                    // constructor.
                    let base = self.ast.decls.get(bc).ctx().unwrap_or(class);
                    let mut msg = self
                        .diag(DiagId::sema_deleted_inherited_ctor_use, sites.deleted)
                        .arg(self.decl_arg(class))
                        .arg(self.decl_arg(base));
                    for n in self.deleted_function_notes(m, 0) {
                        msg = msg.with_note(n);
                    }
                    self.emit(msg);
                    return None;
                }
                // A deleted inherited constructor is named as the base's.
                let m = self.inherited_ctor_base(m).unwrap_or(m);
                let spec =
                    self.ast.decls.function(self.ast.first(m)).map(|x| x.spec).unwrap_or_default();
                let mk = self.special_kind_of(m).filter(|x| x.is_ctor());
                let use_ = matches!(sites.deleted_as, DeletedAs::Use);
                let (deleted_as, param_note, conversion) = match sites.deleted_as {
                    DeletedAs::Call | DeletedAs::Use => (None, None, false),
                    DeletedAs::Param(n) => (None, Some(n), false),
                    DeletedAs::Cast(d, _) => (Some(d), None, false),
                    DeletedAs::Conversion(n) => (None, n, true),
                    DeletedAs::Return => (None, None, true),
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
                let mut msg = if use_ {
                    self.diag(DiagId::sema_use_deleted_function, sites.deleted)
                        .arg(self.delete_message_suffix(m))
                } else if conversion {
                    let from = call.args.first().map_or(QualType::ERROR, |&a| self.ety(a));
                    self.diag(DiagId::sema_conv_deleted_ctor, sites.deleted)
                        .diff_args(self, from, ty)
                } else if spec.defaulted() == Defaulted::No {
                    self.diag(DiagId::sema_call_deleted_ctor, sites.deleted)
                        .arg(self.ty_arg(named))
                        .arg(self.delete_message_suffix(m))
                } else {
                    self.diag(DiagId::sema_call_implicitly_deleted_ctor, sites.deleted)
                        .arg(mk.unwrap_or(k).index())
                        .arg(self.ty_arg(named))
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
                let ret = matches!(sites.deleted_as, DeletedAs::Return);
                let (conversion, param_note, cast) = match sites.deleted_as {
                    DeletedAs::Conversion(n) => (true, n, None),
                    DeletedAs::Return => (true, None, None),
                    DeletedAs::Cast(_, c) => (false, None, Some(c)),
                    _ => (false, None, None),
                };
                let msg = if let Some(c) = cast
                    && let [arg] = call.args
                {
                    let id = if ambiguous {
                        DiagId::sema_ovl_ambiguous_conversion_in_cast
                    } else {
                        DiagId::sema_ovl_no_conversion_in_cast
                    };
                    let from = self.ety(*arg);
                    self.diag(id, sites.deleted).arg(c).arg(self.ty_arg(from)).arg(self.ty_arg(ty))
                } else if conversion {
                    let from = call.args.first().map_or(QualType::ERROR, |&a| self.ety(a));
                    let id = if ambiguous {
                        DiagId::sema_ovl_ambiguous_conversion
                    } else if ret {
                        DiagId::sema_no_viable_conversion_return
                    } else {
                        DiagId::sema_no_viable_conversion
                    };
                    self.diag(id, sites.deleted).diff_args(self, from, ty)
                } else {
                    let id = if ambiguous {
                        DiagId::sema_ovl_ambiguous_ctor
                    } else {
                        DiagId::sema_ovl_no_viable_ctor
                    };
                    self.diag(id, sites.deleted).arg(self.ty_arg(named))
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
    /// of the constructor being defined, with its notes: one per
    /// subobject ([class.base.init]/9), so not deduplicated.
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
        // Every base and member is reported at the constructor's name
        // (clang's `err_missing_default_ctor` per subobject): exempt from
        // `(id, span)` deduplication. A template's constructor is not
        // defined twice for one instantiation, so no repeat is possible.
        self.emit(msg.allow_duplicate());
    }

    /// The arguments of a call of constructor `m` with `args`: each
    /// converted to its parameter's type (copy-initialization,
    /// [expr.call]/7; checked for narrowing if `narrowing`), then the
    /// default arguments (used at the construction `loc`), then the
    /// variadic arguments promoted and checked
    /// ([`Sema::check_variadic_args`]).
    pub(crate) fn ctor_args(
        &mut self,
        m: DeclId,
        args: &[ExprId],
        narrowing: bool,
        explicit_first: bool,
        loc: Loc,
    ) -> Option<Vec<ExprId>> {
        let params = self.param_decls(m);
        let mut out = Vec::new();
        for (i, &a) in args.iter().enumerate() {
            let Some(&p) = params.get(i) else {
                out.push(self.default_argument_promotion(a));
                continue;
            };
            let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
            // A reference parameter bound to a temporary narrows like the
            // temporary's initialization (`check_narrowing`).
            if narrowing {
                self.check_list_narrowing(a, pty);
            }
            // [over.match.ref]/1.1: the reference of a direct-
            // initialization's copy or move constructor.
            let ctx = if explicit_first && i == 0 && self.ast.types.is_reference(pty) {
                ConvCtx::DirectInitialization
            } else {
                ConvCtx::Argument
            };
            // A braced argument list-initializes the parameter; an
            // explicit or deleted constructor chosen for it is followed by
            // the parameter's note (clang `PrintInitLocationNote`).
            let e = if let Some(l) = self.take_braced_arg(a) {
                let to =
                    if self.ast.types.is_reference(pty) { pty } else { conv::strip_cv(self, pty) };
                let note = Some(self.param_decl_note(p));
                self.init_list_entity(to, &l, l.lbrace, ctx, note)
            } else {
                self.convert_for_init(a, pty, ctx)
            };
            if self.is_err_expr(e) {
                return None;
            }
            out.push(e);
        }
        if args.len() < params.len() {
            for &p in &params[args.len()..] {
                // [temp.inst]/12: instantiated at this use.
                if self.inst.lazy_defaults.contains_key(&p)
                    && !self.instantiate_lazy_default(m, p, loc)
                {
                    return None;
                }
                let Some(d) = self.ast.decls.param(p).and_then(|x| x.default_arg) else { break };
                let x = *self.ast.exprs.get(d);
                let used = self.ctx;
                let arg =
                    self.builder().default_arg(p, used, x.ty(), x.value_category(), Loc::INVALID);
                // Used where the construction is ([expr.const]/16), which
                // is also the location a source-location builtin in it
                // takes ([support.srcloc.cons]/2; clang
                // `CompleteConstructorCall`'s `GatherArgumentsForCall`).
                self.note_default_arg_use(arg, p, loc);
                self.note_default_use(arg, p, loc);
                out.push(arg);
            }
        }
        if args.len() > params.len() {
            self.check_variadic_args(&out[params.len()..], 3);
        }
        Some(out)
    }

    // -- assignment operators ----------------------------------------------

    /// clang `DefineImplicitCopyAssignment` / `DefineImplicitMoveAssignment`
    /// ([class.copy.assign]/12): each direct base, then each non-static
    /// data member, is assigned from the corresponding subobject of the
    /// parameter; then `return *this;`.
    fn define_assign(&mut self, f: DeclId, class: DeclId, kind: SpecialKind) -> bool {
        // clang `DefineImplicitCopyAssignment` / `...MoveAssignment`: the
        // body is located at the end of the declaration (the last token
        // of its declarator; an implicit declaration's is its location).
        let ends = &self.ast.decls.cold.declarator_end;
        let loc = ends
            .get(f)
            .or_else(|| self.ast.instantiated_from(f).and_then(|p| ends.get(p)))
            .copied()
            .unwrap_or_else(|| self.ast.decls.get(f).loc());
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
        self.assign_subobject_at(ty, to, from, 0, loc)
    }

    /// clang `buildSingleCopyAssignRecursively` at array nesting `depth`.
    fn assign_subobject_at(
        &mut self,
        ty: QualType,
        to: ExprId,
        from: ExprId,
        depth: u32,
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
            return self.assign_array_loop(ty, to, from, depth, loc);
        }
        if let Some(r) = self.ast.types.as_record(ty) {
            let c = self.ast.definition(r)?;
            let fq = self.ast.types.effective_quals(self.ety(from));
            let tq = self.ast.types.effective_quals(self.ety(to));
            let moving = self.ast.exprs.get(from).value_category() == ValueCategory::XValue;
            let k = if moving { SpecialKind::MoveAssign } else { SpecialKind::CopyAssign };
            let op = match self.select_special(c, k, fq, tq) {
                Selected::Found(op) => op,
                // Reached only where the implicit definition is not
                // checked for deletion first (an instantiated out-of-line
                // `= default`): clang builds the call and reports the use
                // of the deleted operator ([dcl.fct.def.delete]/2).
                Selected::Deleted(op) => {
                    self.diagnose_deleted_use(op, loc);
                    return None;
                }
                _ => {
                    self.unsupported("selecting the assignment operator", loc);
                    return None;
                }
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

    /// The element-wise loop of [class.copy.assign]/12.2 for an array
    /// subobject (clang `buildSingleCopyAssignRecursively`):
    /// `for (size_t __iD = 0; __iD != N; ++__iD) to[__iD] = from[__iD];`,
    /// the element assigned recursively (a move casts each element).
    fn assign_array_loop(
        &mut self,
        ty: QualType,
        to: ExprId,
        from: ExprId,
        depth: u32,
        loc: Loc,
    ) -> Option<StmtId> {
        let Some(n) = self.ast.types.array_size(ty) else {
            self.unsupported("implicit assignment of an array of this type", loc);
            return None;
        };
        let elem = self.ast.types.element(ty)?;
        let moving = self.ast.exprs.get(from).value_category() == ValueCategory::XValue;
        let size_t = self.size_type();
        let sym = self.intern(&format!("__i{depth}"));
        let mut h = DeclHead::new(self.ctx, DeclName::ident(sym), loc);
        h.flags = h.flags.with(DeclFlags::IMPLICIT);
        let spec = VarSpec::new().with_init_style(InitStyle::C);
        let v = self.builder().var(h, size_t, spec, None);
        let zero = self.builder().int_literal(size_t, 0, Radix::Decimal, loc);
        let _ = self.builder().set_var_init(v, zero);
        self.ast.decls.set_flags(v, DeclFlags::REFERENCED);
        let init = self.builder().decl_stmt(&[v], loc);
        let iref = |s: &mut Self| {
            let r = s.builder().decl_ref(v, size_t, loc, None, NonOdrUse::None, ExprFlags::NONE);
            conv::lvalue_to_rvalue(s, r)
        };
        let i = iref(self);
        let mut from_e = self.builtin_subscript(from, i, loc);
        if moving {
            from_e = self.cast_for_moving(from_e);
        }
        let i = iref(self);
        let to_e = self.builtin_subscript(to, i, loc);
        let body = self.assign_subobject_at(elem, to_e, from_e, depth + 1, loc)?;
        let i = iref(self);
        let bound = self.builder().int_literal(size_t, n, Radix::Decimal, loc);
        let cond = self.builder().binary(BinaryOp::NE, i, bound, QualType::BOOL, loc);
        let r = self.builder().decl_ref(v, size_t, loc, None, NonOdrUse::None, ExprFlags::NONE);
        let flags = if n == u64::MAX { ExprFlags::NONE } else { ExprFlags::CANNOT_OVERFLOW };
        let inc = self.builder().unary(UnaryOp::PreInc, r, size_t, loc, flags);
        let data = LoopData {
            init: Some(init),
            cond_var: None,
            cond: Some(cond),
            inc: Some(inc),
            body: Some(body),
        };
        Some(self.builder().for_stmt(data, loc))
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

    /// The `__builtin_memcpy` declaration the memberwise copy calls
    /// (clang `buildMemcpyForAssignmentOp`: ordinary lookup of the name in
    /// the translation unit, creating the builtin if nothing is found).
    /// There is one declaration per builtin (spec/sema.md §5.3): a written
    /// call and this implicit use share whichever was declared first, and
    /// a user redeclaration is found the same way.
    fn memcpy_decl(&mut self) -> DeclId {
        let name = self.ast.symbols().intern("__builtin_memcpy");
        let tu = self.ast.tu;
        let found = self.lookup_qualified_dn(tu, DeclName::ident(name), LookupKind::Ordinary);
        let f = match found.decls.as_slice() {
            &[d] if self.kind_of(d) == DeclKind::Function => Some(d),
            _ => None,
        };
        let f = f
            .or_else(|| self.lazily_create_builtin(name, Loc::INVALID))
            .unwrap_or_else(|| unreachable!("__builtin_memcpy is in the builtin table"));
        self.mark_used(f);
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
                || self.has_irrelevant_dtor(fc)
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
        if self.ast.decls.get(c).flags().contains(DeclFlags::INVALID) || self.has_irrelevant_dtor(c)
        {
            return None;
        }
        match self.select_special(c, SpecialKind::Dtor, Quals::NONE, Quals::NONE) {
            Selected::Found(d) | Selected::Deleted(d) => Some(d),
            _ => None,
        }
    }

    /// clang `hasIrrelevantDestructor`: the destructor of `c` is trivial,
    /// public and not deleted, so a use needs no access or deletion check.
    fn has_irrelevant_dtor(&mut self, c: DeclId) -> bool {
        if !self.defn_bits(c).contains(DefinitionBits::TRIVIAL_DTOR) {
            return false;
        }
        match self.select_special(c, SpecialKind::Dtor, Quals::NONE, Quals::NONE) {
            Selected::Found(d) => self.ast.decls.get(d).misc().access() == Access::Public,
            _ => false,
        }
    }
}
