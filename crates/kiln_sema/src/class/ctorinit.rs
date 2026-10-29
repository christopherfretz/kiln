//! Ctor-initializers ([class.base.init]; spec/sema-class.md §4.9): the
//! mem-initializers of a constructor definition, resolved to members,
//! bases or the constructor's own class (delegation), checked for
//! duplicates and completed with the implicit initializers; clang's
//! `-Wreorder-ctor`, `-Wuninitialized` (fields used before their
//! initialization), `-Wdangling-field`, and the delegation-cycle check at
//! the end of the translation unit.
//!
//! The parser hands each mem-initializer over as soon as it is parsed
//! ([`Sema::act_on_mem_initializer`]), so its temporaries and errors are
//! its own (each is a full-expression, [class.base.init]/7), then ends the
//! list with [`Sema::act_on_mem_initializers`].

use kiln_ast::decl::{CtorInit, CtorInitKind, DeclFlags, DeclKind, DefinitionBits};
use kiln_ast::expr::{BinaryOp, CastKind, ConstructionKind, ExprKind, TraitKind, UnaryOp};
use kiln_ast::{DeclId, ExprId, QualType, TypeId};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::fx::{FxHashMap, FxHashSet};

use super::special::{ImplicitInit, InitEntity, SpecialKind};
use super::temporary::BindingSite;
use super::{MemInitId, MemInitInfo};
use crate::lookup::LookupKind;
use crate::{Initializer, Sema};

/// The mem-initializers of the constructor being defined, built one at a
/// time by [`Sema::act_on_mem_initializer`].
#[derive(Debug, Default)]
pub(crate) struct MemInitState {
    /// The constructor.
    ctor: Option<DeclId>,
    /// The mem-initializers built so far, in written order.
    inits: Vec<CtorInit>,
    /// The number of mem-initializers seen (built or not).
    count: usize,
    /// A mem-initializer failed (clang's `AnyErrors`): no implicit
    /// initializers are added.
    any_errors: bool,
    /// The list is not checked (an invalid constructor, a template).
    skip: bool,
}

/// What a mem-initializer-id names.
enum Named {
    /// A non-static data member (or a member of an anonymous member).
    Member(DeclId),
    /// A class type (a base, or the class itself).
    Class(QualType),
    /// Nothing usable; diagnosed.
    Error,
}

/// A subobject in initialization order, for `-Wreorder-ctor`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OrderKey {
    Base(DeclId),
    Field(DeclId),
}

/// The state of clang's `UninitializedFieldVisitor`.
struct Uninit {
    /// The fields not initialized yet.
    fields: FxHashSet<DeclId>,
    /// The direct bases not initialized yet (first declarations).
    bases: FxHashSet<DeclId>,
    /// Fields assigned in the current initializer, initialized from the
    /// next one on.
    remove: Vec<DeclId>,
    /// The constructor to name in a note, for a default member
    /// initializer: its location and whether it is the implicit default
    /// constructor.
    note: Option<(Loc, bool)>,
    /// The member whose initializer is a braced list (clang checks its
    /// sub-members element by element; kiln does not warn on it).
    list_field: Option<DeclId>,
}

impl Sema<'_, '_> {
    /// One mem-initializer of constructor definition `ctor`, right after it
    /// is parsed ([class.base.init]/2-7, clang `ActOnMemInitializer`):
    /// resolves its id and initializes the member, base or (delegation)
    /// the class from it as a full-expression. Errors are reported now;
    /// the list is checked by [`Sema::act_on_mem_initializers`].
    pub fn act_on_mem_initializer(&mut self, ctor: DeclId, info: &MemInitInfo) {
        if self.mem_inits.ctor != Some(ctor) {
            self.mem_inits = MemInitState { ctor: Some(ctor), ..MemInitState::default() };
            self.mem_inits.skip = !self.check_mem_inits_of(ctor);
        }
        let index = u8::try_from(self.mem_inits.count).unwrap_or(CtorInit::IMPLICIT - 1);
        self.mem_inits.count += 1;
        if self.mem_inits.skip {
            self.cleanup = super::CleanupState::default();
            return;
        }
        if info.pack_expansion {
            // todo(WP-070): pack expansions of base initializers.
            self.unsupported("a mem-initializer pack expansion", info.loc);
            self.cleanup = super::CleanupState::default();
            self.mem_inits.any_errors = true;
            return;
        }
        let Some(class) = self.ast.decls.get(ctor).ctx() else { return };
        let built = match self.mem_init_named(class, info) {
            Named::Member(m) => Some(self.build_member_init(m, info, index)),
            Named::Class(ty) => self.build_class_init(ctor, class, ty, info, index),
            Named::Error => None,
        };
        self.cleanup = super::CleanupState::default();
        match built {
            Some(ci) => self.mem_inits.inits.push(ci),
            None => self.mem_inits.any_errors = true,
        }
    }

    /// The ctor-initializer of constructor definition `ctor`
    /// ([class.base.init], spec/sema-class.md §4.9), between
    /// `act_on_start_function_def` and the body: `inits` are
    /// mem-initializers not yet passed to [`Sema::act_on_mem_initializer`]
    /// (the parser passes none). Checks the list (duplicates, delegation),
    /// completes it with the implicit initializers and stores it. The
    /// parser calls it for every constructor definition, with the `:`
    /// location, or the body's `{` when there is no ctor-initializer.
    pub fn act_on_mem_initializers(&mut self, ctor: DeclId, inits: &[MemInitInfo], colon_loc: Loc) {
        let _ = colon_loc;
        for info in inits {
            self.act_on_mem_initializer(ctor, info);
        }
        let st = if self.mem_inits.ctor == Some(ctor) {
            std::mem::take(&mut self.mem_inits)
        } else {
            MemInitState { skip: !self.check_mem_inits_of(ctor), ..MemInitState::default() }
        };
        if st.skip {
            return;
        }
        let Some(class) = self.ast.decls.get(ctor).ctx() else { return };
        self.finish_mem_inits(ctor, class, &st.inits, st.any_errors);
    }

    /// Whether the mem-initializers of `ctor` are checked: not for an
    /// invalid constructor, nor in a template (WP-070).
    fn check_mem_inits_of(&mut self, ctor: DeclId) -> bool {
        let Some(class) = self.ast.decls.get(ctor).ctx() else { return false };
        !self.ast.decls.get(ctor).flags().contains(DeclFlags::INVALID)
            && !self.ast.decls.get(class).flags().contains(DeclFlags::DEPENDENT_CTX)
            && !self.in_template()
    }

    /// clang `ActOnMemInitializers` after the initializers are built.
    fn finish_mem_inits(
        &mut self,
        ctor: DeclId,
        class: DeclId,
        inits: &[CtorInit],
        any_errors: bool,
    ) {
        let mut members: FxHashMap<DeclId, usize> = FxHashMap::default();
        let mut bases: FxHashMap<Option<DeclId>, usize> = FxHashMap::default();
        let mut unions: FxHashMap<DeclId, (DeclId, usize)> = FxHashMap::default();
        let mut had_error = false;
        for (i, ci) in inits.iter().enumerate() {
            let prev = match ci.kind {
                CtorInitKind::Field | CtorInitKind::IndirectField => {
                    let leaf = self.init_leaf_field(ci);
                    if let Some(&p) = members.get(&leaf) {
                        let d = self
                            .diag(DiagId::sema_init_multiple_member, ci.loc)
                            .arg(self.decl_arg(leaf));
                        Some((d, p))
                    } else {
                        members.insert(leaf, i);
                        had_error |= self.redundant_union_init(leaf, i, inits, &mut unions);
                        None
                    }
                }
                CtorInitKind::Base | CtorInitKind::VirtualBase => {
                    let ty = QualType::unqual(TypeId::new(ci.target));
                    let key = self.ast.types.as_record(ty).map(|r| self.ast.first(r));
                    if let Some(&p) = bases.get(&key) {
                        let d =
                            self.diag(DiagId::sema_init_multiple_base, ci.loc).arg(self.ty_arg(ty));
                        Some((d, p))
                    } else {
                        bases.insert(key, i);
                        None
                    }
                }
                CtorInitKind::Delegating => {
                    if inits.len() != 1 {
                        let msg = self.diag(DiagId::sema_init_delegating_alone, ci.loc);
                        self.emit(msg);
                    }
                    self.set_delegating_init(ctor, *ci);
                    return;
                }
            };
            if let Some((d, p)) = prev {
                let note = self.diag(DiagId::note_init_previous, inits[p].loc);
                self.emit(d.with_note(note));
                had_error = true;
            }
        }
        if had_error {
            return;
        }
        self.check_init_order(class, inits);
        let _ = self.complete_ctor_inits(ctor, class, inits, ImplicitInit::Default, any_errors);
        self.check_uninitialized_fields(ctor);
    }

    /// The field a member initializer initializes (the last of an
    /// indirect field's chain).
    fn init_leaf_field(&self, ci: &CtorInit) -> DeclId {
        let d = DeclId::new(ci.target);
        if ci.kind == CtorInitKind::IndirectField {
            self.ast.lists.iter::<DeclId>(self.ast.indirect_field_chain(d)).last().unwrap_or(d)
        } else {
            d
        }
    }

    /// clang `CheckRedundantUnionInit`: a second member of a union (or of
    /// anonymous members nested in it) initialized by `inits[i]`.
    fn redundant_union_init(
        &mut self,
        field: DeclId,
        i: usize,
        inits: &[CtorInit],
        unions: &mut FxHashMap<DeclId, (DeclId, usize)>,
    ) -> bool {
        let mut child = field;
        let mut parent = self.ast.decls.get(field).ctx();
        for _ in 0..64 {
            let Some(p) = parent else { break };
            let anonymous = self
                .ast
                .decls
                .record(self.class_def(p))
                .is_some_and(|x| x.defn.contains(DefinitionBits::ANONYMOUS));
            let union = self.is_union(p);
            if !anonymous && !union {
                break;
            }
            if union {
                let key = self.ast.first(p);
                match unions.get(&key) {
                    Some(&(c, prev)) if c != child => {
                        let note = self.diag(DiagId::note_init_previous, inits[prev].loc);
                        let msg = self
                            .diag(DiagId::sema_init_multiple_union, inits[i].loc)
                            .with_note(note);
                        self.emit(msg);
                        return true;
                    }
                    Some(_) => {}
                    None => {
                        unions.insert(key, (child, i));
                    }
                }
                if !anonymous {
                    return false;
                }
            }
            child = p;
            parent = self.ast.decls.get(p).ctx();
        }
        false
    }

    /// clang `SetDelegatingInitializer`: `ci` is the whole list; the
    /// delegation is checked for cycles at the end of the translation
    /// unit.
    fn set_delegating_init(&mut self, ctor: DeclId, ci: CtorInit) {
        let _ = self.builder().set_ctor_inits(ctor, &[ci]);
        if let Some(target) = self.constructor_called(ci.init) {
            self.delegating.push((self.ast.first(ctor), target, ci.loc));
        }
    }

    /// The constructor a (full-expression) constructor call calls.
    fn constructor_called(&self, mut e: ExprId) -> Option<DeclId> {
        for _ in 0..8 {
            match self.ast.exprs.get(e).kind() {
                ExprKind::ExprWithCleanups | ExprKind::CXXBindTemporaryExpr => {
                    e = self.ast.exprs.operand(e);
                }
                ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr => {
                    return Some(self.ast.exprs.constructor(e));
                }
                _ => return None,
            }
        }
        None
    }

    // -- building ------------------------------------------------------------------

    /// clang `BuildMemInitializer`'s lookup: a member of the class itself,
    /// else a type named by ordinary lookup.
    fn mem_init_named(&mut self, class: DeclId, info: &MemInitInfo) -> Named {
        let name = match info.id {
            MemInitId::Type(t) => return Named::Class(t),
            MemInitId::Ident(n) => n,
        };
        let def = self.class_def(class);
        let found = self.lookup_in(def, name, LookupKind::Ordinary);
        if let Some(&m) = found
            .decls
            .iter()
            .find(|&&d| matches!(self.kind_of(d), DeclKind::Field | DeclKind::IndirectField))
        {
            return Named::Member(m);
        }
        if let crate::NameClass::Type(ty, d) = self.classify_name(name, None) {
            self.mark_type_referenced(d);
            return Named::Class(ty);
        }
        let msg = self
            .diag(DiagId::sema_init_mem_not_member_or_base, info.loc)
            .arg(crate::decl::sym_str(self, name));
        self.emit(msg);
        Named::Error
    }

    /// clang `BuildMemberInitializer`: the member `m` (a field or an
    /// indirect field) direct-initialized from the mem-initializer. A
    /// failed initialization keeps its arguments in a recovery expression.
    fn build_member_init(&mut self, m: DeclId, info: &MemInitInfo, index: u8) -> CtorInit {
        let indirect = self.kind_of(m) == DeclKind::IndirectField;
        let field = if indirect {
            self.ast.lists.iter::<DeclId>(self.ast.indirect_field_chain(m)).last().unwrap_or(m)
        } else {
            m
        };
        let fty = self.ast.decls.get(field).ty().unwrap_or(QualType::ERROR);
        self.ctor_entity = Some((InitEntity::Field(field), ConstructionKind::Complete));
        let e = self.mem_init_value(fty, info);
        self.ctor_entity = None;
        if self.ast.types.is_reference(fty) {
            self.check_dangling_temporary(BindingSite::Member(field), e);
        }
        self.check_dangling_field(field, fty, e);
        let e = self.act_on_full_expr(e);
        CtorInit {
            target: m.index(),
            kind: if indirect { CtorInitKind::IndirectField } else { CtorInitKind::Field },
            written_index: index,
            init: e,
            loc: info.loc,
        }
    }

    /// The initialization of an object of type `ty` from a
    /// mem-initializer: direct-initialization, direct-list-initialization,
    /// or value-initialization for `()` ([dcl.init.general]/16.4).
    fn mem_init_value(&mut self, ty: QualType, info: &MemInitInfo) -> ExprId {
        match &info.init {
            Initializer::Paren(v, l, _) if v.is_empty() => {
                if self.is_error_type(ty) {
                    return self.recover(QualType::ERROR, &[], *l);
                }
                if self.ast.types.is_reference(ty) {
                    // todo(WP-067b follow-up): clang's "reference to type
                    // requires an initializer".
                    self.unsupported("value-initialization of a reference member", info.loc);
                    return self.recover(ty, &[], info.loc);
                }
                self.paren_value_init(ty, info.loc)
            }
            init => {
                let list = matches!(init, Initializer::List(_));
                self.perform_init(ty, init.clone(), info.loc, list)
            }
        }
    }

    /// clang `BuildBaseInitializer` and `BuildDelegatingInitializer` for
    /// the class type `ty` named by a mem-initializer of a constructor of
    /// `class`. `None` when diagnosed (no initializer is stored).
    fn build_class_init(
        &mut self,
        ctor: DeclId,
        class: DeclId,
        ty: QualType,
        info: &MemInitInfo,
        index: u8,
    ) -> Option<CtorInit> {
        if self.is_error_type(ty) {
            return None;
        }
        let Some(r) = self.ast.types.as_record(ty) else {
            let msg = self.diag(DiagId::sema_init_not_class, info.loc).arg(self.ty_arg(ty));
            self.emit(msg);
            return None;
        };
        let target = self.ast.first(r);
        if target == self.ast.first(class) {
            return Some(self.build_delegating_init(ctor, class, info, index));
        }
        let direct = self
            .indexed_bases(class)
            .into_iter()
            .map(|(_, b)| b)
            .find(|b| self.base_class(b).is_some_and(|c| self.ast.first(c) == target));
        let vbase = if direct.is_some_and(|b| b.is_virtual) {
            None
        } else {
            self.virtual_bases(class).into_iter().find(|(c, _)| self.ast.first(*c) == target)
        };
        let (spec, inherited) = match (direct, vbase) {
            (None, None) => {
                let msg = self
                    .diag(DiagId::sema_init_not_base, info.loc)
                    .arg(self.ty_arg(ty))
                    .arg(self.ty_arg(self.class_type(class)));
                self.emit(msg);
                return None;
            }
            (Some(_), Some(_)) => {
                let msg = self
                    .diag(DiagId::sema_init_base_direct_and_virtual, info.loc)
                    .arg(self.ty_arg(ty));
                self.emit(msg);
                return None;
            }
            (Some(b), None) => (b, false),
            (None, Some((_, b))) => (b, true),
        };
        let is_virtual = spec.is_virtual || inherited;
        let ck = if is_virtual {
            ConstructionKind::VirtualBase
        } else {
            ConstructionKind::NonVirtualBase
        };
        self.ctor_entity = Some((InitEntity::Base { inherited_virtual: inherited }, ck));
        let e = self.mem_init_value(spec.ty, info);
        self.ctor_entity = None;
        if self.is_err_expr(e) {
            return None;
        }
        let e = self.act_on_full_expr(e);
        Some(CtorInit {
            target: ty.ty().index(),
            kind: if is_virtual { CtorInitKind::VirtualBase } else { CtorInitKind::Base },
            written_index: index,
            init: e,
            loc: info.loc,
        })
    }

    /// clang `BuildDelegatingInitializer` ([class.base.init]/6): the
    /// object is initialized by another constructor of its class.
    fn build_delegating_init(
        &mut self,
        ctor: DeclId,
        class: DeclId,
        info: &MemInitInfo,
        index: u8,
    ) -> CtorInit {
        let _ = ctor;
        let ty = self.class_type(class);
        self.ctor_entity = Some((InitEntity::Var, ConstructionKind::Delegating));
        let e = self.mem_init_value(ty, info);
        self.ctor_entity = None;
        let e = self.act_on_full_expr(e);
        CtorInit {
            target: 0,
            kind: CtorInitKind::Delegating,
            written_index: index,
            init: e,
            loc: info.loc,
        }
    }

    /// clang's `-Wdangling-field` for a member initialized from a
    /// parameter: a reference member bound to a by-value parameter, a
    /// pointer member initialized with its address.
    fn check_dangling_field(&mut self, field: DeclId, fty: QualType, init: ExprId) {
        let pointer = self.ast.types.is_pointer(fty);
        if !pointer && !self.ast.types.is_reference(fty) {
            return;
        }
        let mut e = init;
        for _ in 0..64 {
            let n = self.ast.exprs.get(e);
            match n.kind() {
                ExprKind::ParenExpr => e = self.ast.exprs.operand(e),
                ExprKind::ImplicitCastExpr
                    if matches!(
                        self.ast.exprs.cast_kind(e),
                        CastKind::NoOp | CastKind::DerivedToBase | CastKind::UncheckedDerivedToBase
                    ) =>
                {
                    e = self.ast.exprs.operand(e);
                }
                ExprKind::UnaryOperator
                    if pointer && self.ast.exprs.unary_op(e) == UnaryOp::AddrOf =>
                {
                    e = self.ast.exprs.operand(e);
                    while self.ast.exprs.get(e).kind() == ExprKind::ParenExpr {
                        e = self.ast.exprs.operand(e);
                    }
                    break;
                }
                _ => {
                    if pointer {
                        return;
                    }
                    break;
                }
            }
        }
        if self.ast.exprs.get(e).kind() != ExprKind::DeclRefExpr {
            return;
        }
        let p = self.ast.exprs.decl(e);
        let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        if self.kind_of(p) != DeclKind::ParmVar || self.ast.types.is_reference(pty) {
            return;
        }
        let id = if pointer {
            DiagId::sema_dangling_field_param_addr
        } else {
            DiagId::sema_dangling_field_param
        };
        let note = self
            .diag(DiagId::note_ref_or_ptr_member_here, self.ast.decls.get(field).loc())
            .arg(u8::from(pointer));
        let msg = self
            .diag(id, self.ast.exprs.get(e).loc())
            .arg(self.decl_arg(field))
            .arg(self.decl_arg(p))
            .with_note(note);
        self.emit(msg);
    }

    // -- -Wreorder-ctor ------------------------------------------------------------

    /// clang `DiagnoseBaseOrMemInitializerOrder`: the written
    /// mem-initializers `inits` against the initialization order
    /// ([class.base.init]/13).
    fn check_init_order(&mut self, class: DeclId, inits: &[CtorInit]) {
        if !inits.iter().any(|ci| self.diags.is_enabled_at(DiagId::sema_reorder_ctor, ci.loc)) {
            return;
        }
        let ideal = self.init_order_keys(class);
        let keys: Vec<Option<OrderKey>> = inits.iter().map(|ci| self.order_key(ci)).collect();
        let warn = out_of_order(&ideal, &keys);
        let Some(&first) = warn.first() else { return };
        let describe = |s: &Self, ci: &CtorInit| -> (u8, String) {
            match ci.kind {
                CtorInitKind::Field | CtorInitKind::IndirectField => {
                    (0, s.decl_arg(s.init_leaf_field(ci)))
                }
                _ => (1, s.ty_arg(QualType::unqual(TypeId::new(ci.target)))),
            }
        };
        let with_pair = |s: &Self, d: kiln_diag::Diag, i: usize| {
            let (pk, pn) = describe(s, &inits[i - 1]);
            let (ck, cn) = describe(s, &inits[i]);
            d.arg(pk).arg(pn).arg(ck).arg(cn)
        };
        if warn.len() == 1 {
            let d = self.diag(DiagId::sema_reorder_ctor, inits[first - 1].loc);
            let d = with_pair(self, d, first);
            self.emit(d);
            return;
        }
        let mut msg = self.diag(DiagId::sema_reorder_ctor_multiple, inits[first - 1].loc);
        for &i in &warn {
            let n = self.diag(DiagId::note_reorder_ctor, inits[i - 1].loc);
            msg = msg.with_note(with_pair(self, n, i));
        }
        self.emit(msg);
    }

    /// The subobjects of `class` in initialization order: virtual bases,
    /// direct non-virtual bases, then the fields (members of anonymous
    /// members flattened; no unnamed bit-fields).
    fn init_order_keys(&self, class: DeclId) -> Vec<OrderKey> {
        let mut keys: Vec<OrderKey> = self
            .virtual_bases(class)
            .into_iter()
            .map(|(c, _)| OrderKey::Base(self.ast.first(c)))
            .collect();
        for (_, b) in self.indexed_bases(class) {
            if !b.is_virtual
                && let Some(c) = self.base_class(&b)
            {
                keys.push(OrderKey::Base(self.ast.first(c)));
            }
        }
        self.field_order_keys(self.class_def(class), &mut keys, 0);
        keys
    }

    fn field_order_keys(&self, r: DeclId, keys: &mut Vec<OrderKey>, depth: usize) {
        if depth > 64 {
            return;
        }
        for f in self.ast.decls.members(r).collect::<Vec<_>>() {
            if self.kind_of(f) != DeclKind::Field || self.is_unnamed_bit_field(f) {
                continue;
            }
            if self.is_anonymous_member(f)
                && let Some(c) =
                    self.ast.decls.get(f).ty().and_then(|t| self.ast.types.as_record(t))
            {
                self.field_order_keys(self.class_def(c), keys, depth + 1);
            } else {
                keys.push(OrderKey::Field(f));
            }
        }
    }

    fn order_key(&self, ci: &CtorInit) -> Option<OrderKey> {
        match ci.kind {
            CtorInitKind::Field | CtorInitKind::IndirectField => {
                Some(OrderKey::Field(self.init_leaf_field(ci)))
            }
            CtorInitKind::Base | CtorInitKind::VirtualBase => self
                .ast
                .types
                .as_record(QualType::unqual(TypeId::new(ci.target)))
                .map(|r| OrderKey::Base(self.ast.first(r))),
            CtorInitKind::Delegating => None,
        }
    }

    // -- -Wuninitialized ------------------------------------------------------------

    /// clang `DiagnoseUninitializedFields`: a field (or base) of the class
    /// of constructor `ctor` used by an initializer that runs before its
    /// own ([class.base.init]/13).
    pub(crate) fn check_uninitialized_fields(&mut self, ctor: DeclId) {
        let loc = self.ast.decls.get(ctor).loc();
        if !self.diags.is_enabled_at(DiagId::sema_field_uninit_use, loc)
            || self.ast.decls.get(ctor).flags().contains(DeclFlags::INVALID)
        {
            return;
        }
        let Some(class) = self.ast.decls.get(ctor).ctx() else { return };
        if self.ast.decls.get(class).flags().contains(DeclFlags::DEPENDENT_CTX) {
            return;
        }
        let def = self.class_def(class);
        let mut st = Uninit {
            fields: FxHashSet::default(),
            bases: FxHashSet::default(),
            remove: Vec::new(),
            note: None,
            list_field: None,
        };
        for m in self.ast.decls.members(def).collect::<Vec<_>>() {
            match self.kind_of(m) {
                DeclKind::Field => {
                    st.fields.insert(m);
                }
                DeclKind::IndirectField => {
                    if let Some(f) =
                        self.ast.lists.iter::<DeclId>(self.ast.indirect_field_chain(m)).last()
                    {
                        st.fields.insert(f);
                    }
                }
                _ => {}
            }
        }
        for (_, b) in self.indexed_bases(class) {
            if let Some(c) = self.ast.types.as_record(b.ty) {
                st.bases.insert(self.ast.first(c));
            }
        }
        let implicit_default = self.ast.decls.get(ctor).flags().contains(DeclFlags::IMPLICIT)
            && self.special_kind_of(ctor) == Some(SpecialKind::DefaultCtor);
        let inits: Vec<CtorInit> = self.ast.ctor_inits(ctor).copied().collect();
        for ci in inits {
            if st.fields.is_empty() && st.bases.is_empty() {
                break;
            }
            let (field, base) = match ci.kind {
                CtorInitKind::Field | CtorInitKind::IndirectField => {
                    (Some(self.init_leaf_field(&ci)), None)
                }
                CtorInitKind::Base | CtorInitKind::VirtualBase => (
                    None,
                    self.ast
                        .types
                        .as_record(QualType::unqual(TypeId::new(ci.target)))
                        .map(|r| self.ast.first(r)),
                ),
                CtorInitKind::Delegating => (None, None),
            };
            let mut e = ci.init;
            while self.ast.exprs.get(e).kind() == ExprKind::ExprWithCleanups {
                e = self.ast.exprs.operand(e);
            }
            let note = if self.ast.exprs.get(e).kind() == ExprKind::CXXDefaultInitExpr {
                let f = self.ast.exprs.default_init_field(e);
                let Some(x) = self.ast.decls.field(f).and_then(|x| x.init) else { continue };
                e = x;
                Some((loc, implicit_default))
            } else {
                e = ci.init;
                None
            };
            for d in st.remove.drain(..) {
                st.fields.remove(&d);
            }
            st.note = note;
            st.list_field = field.filter(|_| {
                let mut x = e;
                while self.ast.exprs.get(x).kind() == ExprKind::ExprWithCleanups {
                    x = self.ast.exprs.operand(x);
                }
                self.ast.exprs.get(x).kind() == ExprKind::InitListExpr
            });
            self.uninit_visit(&mut st, e, 0);
            if let Some(f) = field {
                st.fields.remove(&f);
            }
            if let Some(b) = base {
                st.bases.remove(&b);
            }
        }
    }

    /// `EvaluatedExprVisitor::Visit` with clang's `UninitializedFieldVisitor`
    /// overrides.
    fn uninit_visit(&mut self, st: &mut Uninit, e: ExprId, depth: u32) {
        if depth > 512 {
            return;
        }
        let n = *self.ast.exprs.get(e);
        match n.kind() {
            ExprKind::ImplicitCastExpr
                if self.ast.exprs.cast_kind(e) == CastKind::LValueToRValue =>
            {
                self.uninit_value(st, self.ast.exprs.operand(e), false, depth + 1);
            }
            ExprKind::MemberExpr => self.uninit_member(st, e, true, false, depth + 1),
            ExprKind::CXXConstructExpr | ExprKind::CXXTemporaryObjectExpr
                if self.special_kind_of(self.ast.exprs.constructor(e))
                    == Some(SpecialKind::CopyCtor) =>
            {
                let args: Vec<ExprId> =
                    self.ast.lists.iter::<ExprId>(self.ast.exprs.construct_args(e)).collect();
                let Some(&a) = args.first() else { return };
                let mut a = a;
                if self.ast.exprs.get(a).kind() == ExprKind::InitListExpr {
                    let inits: Vec<ExprId> =
                        self.ast.lists.iter::<ExprId>(self.ast.exprs.init_list_inits(a)).collect();
                    if let [x] = inits.as_slice() {
                        a = *x;
                    }
                }
                if self.ast.exprs.get(a).kind() == ExprKind::ImplicitCastExpr
                    && self.ast.exprs.cast_kind(a) == CastKind::NoOp
                {
                    a = self.ast.exprs.operand(a);
                }
                self.uninit_value(st, a, false, depth + 1);
            }
            ExprKind::CXXMemberCallExpr => {
                let callee = self.ast.exprs.callee(e);
                if self.ast.exprs.get(callee).kind() == ExprKind::MemberExpr {
                    self.uninit_value(st, callee, false, depth + 1);
                }
                self.uninit_children(st, e, depth);
            }
            ExprKind::BinaryOperator => {
                if self.ast.exprs.binary_op(e) == BinaryOp::Assign {
                    let [lhs, _] = self.ast.exprs.operands(e);
                    if self.ast.exprs.get(lhs).kind() == ExprKind::MemberExpr {
                        let f = self.ast.exprs.decl(lhs);
                        let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
                        if self.kind_of(f) == DeclKind::Field && !self.ast.types.is_reference(fty) {
                            st.remove.push(f);
                        }
                    }
                }
                self.uninit_children(st, e, depth);
            }
            ExprKind::CompoundAssignOperator => {
                let kids = self.ast.exprs.children(e, &self.ast.lists);
                if let [lhs, rhs] = kids.as_slice() {
                    self.uninit_value(st, *lhs, false, depth + 1);
                    self.uninit_visit(st, *rhs, depth + 1);
                }
            }
            ExprKind::UnaryOperator => {
                let op = self.ast.exprs.unary_op(e);
                let sub = self.ast.exprs.operand(e);
                if op.is_inc_dec() {
                    self.uninit_value(st, sub, false, depth + 1);
                } else if op == UnaryOp::AddrOf
                    && self.ast.exprs.get(sub).kind() == ExprKind::MemberExpr
                {
                    self.uninit_value(st, self.ast.exprs.operand(sub), true, depth + 1);
                } else {
                    self.uninit_children(st, e, depth);
                }
            }
            // Unevaluated operands ([expr.context]/1).
            ExprKind::UnaryExprOrTypeTraitExpr
                if self.ast.exprs.trait_kind(e) != TraitKind::VecStep => {}
            ExprKind::CXXNoexceptExpr => {}
            ExprKind::CXXTypeidExpr if !self.ast.exprs.typeid_potentially_evaluated(e) => {}
            _ => self.uninit_children(st, e, depth),
        }
    }

    fn uninit_children(&mut self, st: &mut Uninit, e: ExprId, depth: u32) {
        for c in self.ast.exprs.children(e, &self.ast.lists) {
            self.uninit_visit(st, c, depth + 1);
        }
    }

    /// clang `HandleValue`: `e` is used as a value (or, `addr_of`, its
    /// address is taken).
    fn uninit_value(&mut self, st: &mut Uninit, e: ExprId, addr_of: bool, depth: u32) {
        if depth > 512 {
            return;
        }
        let e = self.strip_parens(e);
        match self.ast.exprs.get(e).kind() {
            ExprKind::MemberExpr => self.uninit_member(st, e, false, addr_of, depth + 1),
            ExprKind::ConditionalOperator => {
                let [c, _] = self.ast.exprs.operands(e);
                let [t, f] = self.ast.exprs.cond_arms(e);
                self.uninit_visit(st, c, depth + 1);
                self.uninit_value(st, t, addr_of, depth + 1);
                self.uninit_value(st, f, addr_of, depth + 1);
            }
            ExprKind::BinaryConditionalOperator => {
                let [common, _, _, f] =
                    self.ast.exprs.binary_conditional_operands(e, &self.ast.lists);
                self.uninit_visit(st, common, depth + 1);
                self.uninit_value(st, f, addr_of, depth + 1);
            }
            ExprKind::OpaqueValueExpr => {
                if let Some(s) = self.ast.exprs.opaque_source(e) {
                    self.uninit_visit(st, s, depth + 1);
                }
            }
            ExprKind::BinaryOperator => {
                let [l, r] = self.ast.exprs.operands(e);
                match self.ast.exprs.binary_op(e) {
                    BinaryOp::PtrMemD | BinaryOp::PtrMemI => {
                        self.uninit_value(st, l, addr_of, depth + 1);
                        self.uninit_visit(st, r, depth + 1);
                    }
                    BinaryOp::Comma => {
                        self.uninit_visit(st, l, depth + 1);
                        self.uninit_value(st, r, addr_of, depth + 1);
                    }
                    _ => self.uninit_visit(st, e, depth + 1),
                }
            }
            _ => self.uninit_visit(st, e, depth + 1),
        }
    }

    fn strip_parens(&self, mut e: ExprId) -> ExprId {
        while self.ast.exprs.get(e).kind() == ExprKind::ParenExpr {
            e = self.ast.exprs.operand(e);
        }
        e
    }

    fn strip_paren_imp_casts(&self, mut e: ExprId) -> ExprId {
        loop {
            match self.ast.exprs.get(e).kind() {
                ExprKind::ParenExpr | ExprKind::ImplicitCastExpr => e = self.ast.exprs.operand(e),
                _ => return e,
            }
        }
    }

    /// clang `HandleMemberExpr`: the use of the class member access `me`;
    /// `ref_only` warns only for reference members (a use that is not a
    /// value use).
    fn uninit_member(
        &mut self,
        st: &mut Uninit,
        me: ExprId,
        ref_only: bool,
        addr_of: bool,
        depth: u32,
    ) {
        let mut field_me = me;
        let mut all_pod = self.is_pod_for_uninit_check(self.ety(me));
        let mut base = me;
        loop {
            let sub = self.strip_paren_imp_casts(base);
            if self.ast.exprs.get(sub).kind() != ExprKind::MemberExpr {
                break;
            }
            let d = self.ast.exprs.decl(sub);
            match self.kind_of(d) {
                DeclKind::Var => return,
                DeclKind::Field if !self.is_anonymous_member(d) => field_me = sub,
                _ => {}
            }
            if !self.is_pod_for_uninit_check(self.ety(field_me)) {
                all_pod = false;
            }
            base = self.ast.exprs.operand(sub);
        }
        if self.ast.exprs.get(self.strip_paren_imp_casts(base)).kind() != ExprKind::CXXThisExpr {
            self.uninit_visit(st, base, depth + 1);
            return;
        }
        if addr_of && all_pod {
            return;
        }
        let found = self.ast.exprs.decl(field_me);
        let at = self.ast.exprs.get(field_me).loc();
        if self.ast.exprs.get(base).kind() == ExprKind::ImplicitCastExpr {
            let mut cast = base;
            while self.ast.exprs.get(self.ast.exprs.operand(cast)).kind()
                == ExprKind::ImplicitCastExpr
            {
                cast = self.ast.exprs.operand(cast);
            }
            if self.ast.exprs.cast_kind(cast) == CastKind::UncheckedDerivedToBase
                && let Some(p) = self.ast.types.pointee(self.ety(cast))
                && let Some(r) = self.ast.types.as_record(p)
                && st.bases.contains(&self.ast.first(r))
            {
                let msg = self
                    .diag(DiagId::sema_base_uninit_use, at)
                    .arg(self.ty_arg(p))
                    .arg(self.qualified_decl_arg(found));
                self.emit(msg);
            }
        }
        if !st.fields.contains(&found) {
            return;
        }
        let fty = self.ast.decls.get(found).ty().unwrap_or(QualType::ERROR);
        let is_ref = self.ast.types.is_reference(fty);
        if st.list_field == Some(found) && !addr_of {
            // todo(WP-067b follow-up): clang's `IsInitListMemberExprInitialized`.
            return;
        }
        if ref_only && !is_ref {
            return;
        }
        let id = if is_ref { DiagId::sema_ref_uninit_use } else { DiagId::sema_field_uninit_use };
        let mut msg = self.diag(id, at).arg(self.decl_arg(found));
        if let Some((loc, implicit)) = st.note {
            msg =
                msg.with_note(self.diag(DiagId::note_uninit_in_ctor, loc).arg(u8::from(implicit)));
        }
        self.emit(msg);
    }

    /// clang `QualType::isPODType` (C++11 POD: scalar, or a trivial
    /// standard-layout class, or an array of such).
    fn is_pod_for_uninit_check(&self, ty: QualType) -> bool {
        let t = self.base_element(ty);
        if self.ast.types.is_reference(t) {
            return false;
        }
        match self.ast.types.as_record(t) {
            None => !self.ast.types.is_void(t),
            Some(r) => self.ast.definition(r).is_some_and(|d| {
                let bits = self.defn_bits(d);
                bits.contains(DefinitionBits::COMPLETE)
                    && bits.contains(DefinitionBits::STANDARD_LAYOUT)
                    && bits.contains(DefinitionBits::TRIVIALLY_COPYABLE)
                    && bits.contains(DefinitionBits::TRIVIAL_DEFAULT_CTOR)
            }),
        }
    }

    /// `d` with its qualified name, as clang prints a `%q` argument.
    fn qualified_decl_arg(&self, d: DeclId) -> String {
        use kiln_ast::types::TypeNames as _;
        let mut s = String::new();
        crate::names::SemaNames::new(self).write_decl_name(d, true, &mut s);
        format!("'{s}'")
    }

    // -- delegation cycles ------------------------------------------------------------

    /// The end of the translation unit (clang `ActOnEndOfTranslationUnit`):
    /// the checks that need every definition, the delegation cycles of
    /// constructors ([class.base.init]/6).
    pub fn act_on_end_of_translation_unit(&mut self) {
        self.check_delegating_cycles();
    }

    /// The end of the translation unit: clang `CheckDelegatingCtorCycles`
    /// ([class.base.init]/6: a constructor that delegates to itself,
    /// directly or indirectly, is ill-formed, no diagnostic required).
    fn check_delegating_cycles(&mut self) {
        let list = std::mem::take(&mut self.delegating);
        let mut valid: FxHashSet<DeclId> = FxHashSet::default();
        let mut invalid: FxHashSet<DeclId> = FxHashSet::default();
        let mut current: Vec<DeclId> = Vec::new();
        for &(ctor, _, _) in &list {
            self.delegation_cycle(ctor, &list, &mut valid, &mut invalid, &mut current, 0);
        }
        for c in invalid {
            self.ast.decls.set_flags(c, DeclFlags::INVALID);
        }
        self.delegating = list;
    }

    /// The target of delegating constructor `ctor` (a first declaration)
    /// and the definition it reaches, if it has one.
    fn delegation_target(ctor: DeclId, list: &[(DeclId, DeclId, Loc)]) -> Option<(DeclId, Loc)> {
        list.iter().find(|x| x.0 == ctor).map(|&(_, t, l)| (t, l))
    }

    /// The definition of constructor `f`, if it has a body.
    fn defined_ctor(&self, f: DeclId) -> Option<DeclId> {
        let d = self.ast.definition(f)?;
        self.ast.decls.function(d).and_then(|x| x.body).map(|_| d)
    }

    /// clang `DelegatingCycleHelper`.
    fn delegation_cycle(
        &mut self,
        ctor: DeclId,
        list: &[(DeclId, DeclId, Loc)],
        valid: &mut FxHashSet<DeclId>,
        invalid: &mut FxHashSet<DeclId>,
        current: &mut Vec<DeclId>,
        depth: usize,
    ) {
        if depth > 4096 || self.ast.decls.get(ctor).flags().contains(DeclFlags::INVALID) {
            return;
        }
        let canonical = self.ast.first(ctor);
        let Some((target, init_loc)) = Self::delegation_target(canonical, list) else { return };
        let target_def = self.defined_ctor(target).unwrap_or(target);
        let tcanon = self.ast.first(target);
        current.push(canonical);
        let target_delegates = list.iter().any(|x| x.0 == tcanon);
        if !target_delegates
            || self.ast.decls.get(target_def).flags().contains(DeclFlags::INVALID)
            || valid.contains(&tcanon)
        {
            valid.extend(current.drain(..));
        } else if tcanon == canonical || invalid.contains(&tcanon) || current.contains(&tcanon) {
            if !invalid.contains(&tcanon) {
                let mut msg = self
                    .diag(DiagId::sema_delegating_ctor_cycle, init_loc)
                    .arg(self.decl_arg(canonical));
                if tcanon != canonical {
                    msg =
                        msg.with_note(self.diag(
                            DiagId::note_it_delegates_to,
                            self.ast.decls.get(target_def).loc(),
                        ));
                }
                let mut c = tcanon;
                for _ in 0..4096 {
                    if c == canonical {
                        break;
                    }
                    let Some((next, _)) = Self::delegation_target(c, list) else { break };
                    let def = self.defined_ctor(next).unwrap_or(next);
                    msg = msg.with_note(
                        self.diag(DiagId::note_which_delegates_to, self.ast.decls.get(def).loc()),
                    );
                    c = self.ast.first(next);
                }
                self.emit(msg);
            }
            invalid.extend(current.drain(..));
        } else {
            self.delegation_cycle(target_def, list, valid, invalid, current, depth + 1);
        }
    }
}

/// The indices of the mem-initializers out of the initialization order
/// `ideal` (clang's `WarnIndexes`): each is not found after the previous
/// one's position. `keys[i]` is `None` for a delegating initializer.
fn out_of_order(ideal: &[OrderKey], keys: &[Option<OrderKey>]) -> Vec<usize> {
    let mut warn = Vec::new();
    let mut at = 0;
    for (i, k) in keys.iter().enumerate() {
        let Some(k) = k else { continue };
        while at != ideal.len() && ideal[at] != *k {
            at += 1;
        }
        if at == ideal.len() && i != 0 {
            warn.push(i);
            at = ideal.iter().position(|x| x == k).unwrap_or(0);
        }
    }
    warn
}

#[cfg(test)]
mod tests {
    use super::{OrderKey, out_of_order};
    use kiln_ast::DeclId;

    const NONE: Vec<usize> = Vec::new();

    fn f(n: u32) -> OrderKey {
        OrderKey::Field(DeclId::new(n))
    }

    fn b(n: u32) -> OrderKey {
        OrderKey::Base(DeclId::new(n))
    }

    #[test]
    fn in_order_and_gaps_are_fine() {
        let ideal = [b(1), f(2), f(3), f(4)];
        assert_eq!(out_of_order(&ideal, &[Some(b(1)), Some(f(2)), Some(f(4))]), NONE);
        assert_eq!(out_of_order(&ideal, &[Some(f(3))]), NONE);
        assert_eq!(out_of_order(&ideal, &[]), NONE);
    }

    #[test]
    fn one_swap_is_one_warning() {
        let ideal = [f(1), f(2), f(3)];
        assert_eq!(out_of_order(&ideal, &[Some(f(2)), Some(f(1)), Some(f(3))]), vec![1]);
        // A base written after a field.
        let ideal = [b(9), f(1)];
        assert_eq!(out_of_order(&ideal, &[Some(f(1)), Some(b(9))]), vec![1]);
    }

    #[test]
    fn several_out_of_order() {
        let ideal = [f(1), f(2), f(3)];
        // c, b, a: b is before c, then a is before b.
        assert_eq!(out_of_order(&ideal, &[Some(f(3)), Some(f(2)), Some(f(1))]), vec![1, 2]);
        // After an out-of-order item the scan restarts from its position.
        assert_eq!(out_of_order(&ideal, &[Some(f(2)), Some(f(1)), Some(f(3))]), vec![1]);
    }
}
