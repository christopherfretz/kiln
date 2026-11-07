//! Class members in expressions (spec/sema-class.md §2.5/§2.6; WP-066b):
//! implicit `this->` ([expr.prim.id.general]/3), `e.m` / `e->m`
//! ([expr.ref]), calls of non-static member functions ([expr.call]/2,
//! [over.match.funcs]/4-5 for a single candidate), `&C::m`
//! ([expr.unary.op]/3) and `.*` / `->*` ([expr.mptr.oper]).

use kiln_ast::decl::{DeclFlags, DeclKind, FnStorage};
use kiln_ast::expr::{
    BinaryOp, CastKind, DeclRefExtra, ExprFlags, ExprKind, NonOdrUse, UnaryOp, ValueCategory,
};
use kiln_ast::types::{Quals, RefQual, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::conv::bases::{
    BaseSearch, ambiguous_paths_string, find_base, is_derived_from, path_list, same_class,
};
use crate::conv::{self};
use crate::expr::CallCallee;
use crate::init::InitList;
use crate::lookup::LookupKind;
use crate::type_diff::DiffArgs as _;

/// The object of a destructor member access (`x.~T`, `p->~T`).
#[derive(Clone, Copy, Debug)]
struct DtorObject {
    /// The object expression as written (after any `operator->` chain).
    base: ExprId,
    /// The converted object (the pointer for `->`).
    b: ExprId,
    /// The object type (the pointee for `->`; dependent if unknown).
    ty: QualType,
    /// The complete class of a class object.
    def: Option<DeclId>,
    /// `->` (cleared when `operator->` is not used).
    arrow: bool,
}

/// How a destructor name in a member access was qualified.
#[derive(Clone, Copy, Debug)]
enum DtorQual {
    /// `x.~T`.
    None,
    /// `x.N::~T` on a class object: the qualifier as written.
    Written(crate::using::Qualifier),
    /// `p->T::~T` on a scalar or dependent object, or such a name
    /// substituted: a class type is named through itself.
    Implied,
}

/// A name of a non-static member whose meaning depends on the operand it
/// appears in (spec/sema-class.md §2.6): `&C::m` forms a pointer to
/// member; anything else needs an object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MemberRef {
    /// The member.
    pub(crate) decl: DeclId,
    /// No object: an error unless `&` consumes the name
    /// ([expr.prim.id.general]/4).
    pub(crate) error: bool,
    /// Named by a qualified-id (only `&C::m` forms a pointer to member).
    pub(crate) qualified: bool,
    /// Where the qualified-id begins, when `e` is a `MemberExpr` (the
    /// `DeclRefExpr` of `&C::m` begins at its qualifier).
    pub(crate) begin: Option<Loc>,
}

const CV: Quals = Quals::CV;

impl Sema<'_, '_> {
    /// A static member function.
    pub(crate) fn is_static_method(&self, d: DeclId) -> bool {
        self.kind_of(d) == DeclKind::CXXMethod
            && self.ast.decls.function(d).is_some_and(|f| f.spec.storage() == FnStorage::Static)
    }

    /// A non-static data member (of a class, not of an anonymous union at
    /// namespace or block scope) or non-static member function.
    pub(crate) fn is_class_member_ref(&self, d: DeclId) -> bool {
        match self.kind_of(d) {
            DeclKind::Field | DeclKind::CXXConversion | DeclKind::CXXDestructor => true,
            DeclKind::IndirectField => self
                .ast
                .lists
                .iter::<DeclId>(self.ast.indirect_field_chain(d))
                .next()
                .is_some_and(|h| self.kind_of(h) == DeclKind::Field),
            DeclKind::CXXMethod => !self.is_static_method(d),
            _ => false,
        }
    }

    /// The type of `this` and the class it points to, where `this` may
    /// be used.
    pub(crate) fn this_class(&self) -> Option<(QualType, DeclId)> {
        let t = self.current_this()?;
        let p = self.pointee_sugared(t)?;
        let r = self.ast.types.as_record(p)?;
        Some((t, self.ast.definition(r).unwrap_or(r)))
    }

    /// The class of `*this` if it is (derived from) the class declaring
    /// member `d`: the object of an implicit member access.
    pub(crate) fn implicit_object_class(&self, d: DeclId) -> Option<DeclId> {
        let declaring = self.declaring_class(d)?;
        let (_, c) = self.this_class()?;
        self.derived_inclusive(c, declaring).then_some(c)
    }

    /// Whether an id-expression naming instance member `d` (or a member
    /// template of instance member functions) is an implicit
    /// `this->Q::name` resolved at instantiation: `d`'s class is not
    /// (yet) known to be a base of `*this`, whose class has a dependent
    /// base ([temp.dep.type]/6; clang `IMA_Dependent`).
    pub(crate) fn member_via_dependent_base(&self, d: DeclId) -> bool {
        let d = if self.kind_of(d) == DeclKind::FunctionTemplate {
            self.pattern_of(d).unwrap_or(d)
        } else {
            d
        };
        self.is_class_member_ref(d)
            && self.implicit_object_class(d).is_none()
            && self.this_class().is_some_and(|(_, c)| self.has_dependent_bases(c))
    }

    /// The type of a dependent `this` that may still turn out to point to
    /// a class derived from the class declaring member `d`
    /// ([class.mfct.non.static]/2, [temp.dep.type]/2): the class of
    /// `*this` is not (derived from) `d`'s class, but a base on the way is
    /// dependent, so the relation is decided at instantiation (clang's
    /// `IsProvablyNotDerivedFrom` is false and the access is
    /// `IMA_Instance`). libc++ `basic_ostream<C, T>` calls
    /// `ios_base::flags()` through its dependent base `basic_ios<C, T>`.
    pub(crate) fn dependent_implicit_this(&self, d: DeclId) -> Option<QualType> {
        self.declaring_class(d)?;
        let (this_ty, c) = self.this_class()?;
        let obj_ty = self.ast.types.pointee(this_ty)?;
        if !self.ast.types.is_dependent(obj_ty) {
            return None;
        }
        let mut seen = Vec::new();
        let mut stack = vec![c];
        while let Some(r) = stack.pop() {
            let Some(def) = self.ast.definition(r) else { continue };
            if seen.contains(&def) {
                continue;
            }
            seen.push(def);
            for b in self.ast.decls.bases(def, &self.ast.lists) {
                if self.is_dependent_type(b.ty) {
                    return Some(this_ty);
                }
                if let Some(br) = self.ast.types.as_record(b.ty) {
                    stack.push(br);
                }
            }
        }
        None
    }

    /// `this->N::m` for a member `m` of a class that `*this` may derive
    /// from only through a dependent base ([`Self::dependent_implicit_this`]):
    /// a `CXXDependentScopeMemberExpr` on `this`, resolved at
    /// instantiation like the explicit `this->N::m`.
    pub(crate) fn dependent_implicit_member(
        &mut self,
        this_ty: QualType,
        name: kiln_ast::names::DeclName,
        nns: Option<kiln_ast::NnsId>,
        loc: Loc,
    ) -> ExprId {
        self.note_this_use(loc);
        let this = self.builder().this_expr(this_ty, true, loc);
        let b = conv::decay_dependent(self, this);
        self.builder().dependent_member(Some(b), true, name, false, nns, None, loc)
    }

    /// An id-expression naming the non-static member `d`
    /// ([expr.prim.id.general]/3): `(*this).d` where `this` may be used
    /// and its class is (derived from) `d`'s class; otherwise a
    /// `DeclRefExpr` that only `&C::m` ([expr.unary.op]/3) or an
    /// unevaluated operand may use (spec/sema-class.md §2.6).
    pub(crate) fn implicit_member_ref(
        &mut self,
        d: DeclId,
        loc: Loc,
        shadow: Option<DeclId>,
        nns: Option<kiln_ast::NnsId>,
    ) -> ExprId {
        // As `member_ref_without_object`: an invalid field (`Ts m;`,
        // [temp.variadic]/12) names nothing, silently.
        if self.ast.decls.get(d).has(DeclFlags::INVALID)
            && !self.ast.decls.get(d).kind().is_function()
        {
            return self.recover_err(&[], loc);
        }
        let qualified = nns.is_some();
        // `&C::m` in a lambda: the operand is the name, not `this->C::m`,
        // whose `this` the lambda would capture (clang's
        // `BuildPossibleImplicitMemberExpr` with `isAddressOfOperand`).
        let addr_operand = std::mem::take(&mut self.address_operand);
        if addr_operand && qualified && !self.lambdas.is_empty() {
            return self.member_ref_without_object(d, loc, shadow, nns);
        }
        if let Some(class) = self.implicit_object_class(d)
            && let Some((this_ty, _)) = self.this_class()
        {
            self.note_this_use(loc);
            let this = self.builder().this_expr(this_ty, true, loc);
            let obj_ty = self.ast.types.pointee(this_ty).unwrap_or(QualType::ERROR);
            self.diagnose_deprecated(d, loc);
            self.check_sve_use(d, loc);
            let e = self.member_of_object(this, obj_ty, class, true, d, shadow, nns, loc);
            if qualified && !self.is_err_expr(e) {
                self.member_refs
                    .insert(e, MemberRef { decl: d, error: false, qualified, begin: None });
            }
            return e;
        }
        if let Some(this_ty) = self.dependent_implicit_this(d) {
            let name = self.ast.decls.get(d).name();
            let e = self.dependent_implicit_member(this_ty, name, nns, loc);
            if qualified {
                // `&N::m` still forms a pointer to member
                // (`act_on_unary`).
                self.member_refs
                    .insert(e, MemberRef { decl: d, error: false, qualified, begin: None });
            }
            return e;
        }
        self.member_ref_without_object(d, loc, shadow, nns)
    }

    /// `N::m` built as the implicit `this->N::m` through a dependent base
    /// (`e`, [`Sema::dependent_implicit_member`]) names the single member
    /// `d`: recorded so that `&N::m` is the qualified name itself
    /// ([expr.unary.op]/3; clang forms no implicit member access for an
    /// address-of operand, so `&Int::f` of an unrelated class `Int`
    /// inside a template with a dependent base is a pointer to member).
    pub(crate) fn note_qualified_dependent_member(&mut self, e: ExprId, d: DeclId, nns: Option<kiln_ast::NnsId>) {
        if nns.is_some() && self.ast.exprs.get(e).kind() == ExprKind::CXXDependentScopeMemberExpr
        {
            let r = MemberRef { decl: d, error: false, qualified: true, begin: None };
            self.member_refs.insert(e, r);
        }
    }

    /// `&N::m` where [`Self::implicit_member_ref`] built `this->N::m` for
    /// a dependent `this` ([`Self::dependent_implicit_member`]): the
    /// operand is the qualified name itself ([expr.unary.op]/3), as clang
    /// does not form an implicit member access for an address-of operand.
    pub(crate) fn undo_dependent_implicit_member(&mut self, e: ExprId) -> ExprId {
        let Some(r) = self.member_refs.get(&e).copied() else { return e };
        if !r.qualified {
            return e;
        }
        let nns = match self.ast.exprs.get(e).kind() {
            ExprKind::CXXDependentScopeMemberExpr => self.ast.templates.expr_nns.get(e).copied(),
            // `this->C::m` of the current instantiation whose type is
            // dependent: `act_on_unary` keeps the operand for
            // instantiation, which must see `C::m`, not the implicit
            // member access (WP-669).
            ExprKind::MemberExpr if self.ast.types.is_dependent(self.ety(e)) => {
                self.ast.exprs.decl_ref_extra(e).and_then(|x| x.qualifier)
            }
            _ => return e,
        };
        self.member_refs.remove(&e);
        let x = self.member_ref_without_object(r.decl, self.eloc(e), None, nns);
        if let Some(b) = r.begin
            && let Some(m) = self.member_refs.get_mut(&x)
        {
            m.begin = Some(b);
        }
        x
    }

    /// The `DeclRefExpr` of a non-static member named without an object:
    /// only `&C::m` ([expr.unary.op]/3) or an unevaluated operand may use
    /// it.
    pub(crate) fn member_ref_without_object(
        &mut self,
        d: DeclId,
        loc: Loc,
        shadow: Option<DeclId>,
        nns: Option<kiln_ast::NnsId>,
    ) -> ExprId {
        // clang `BuildDeclarationNameExpr`: an invalid declaration (a
        // field whose type failed) names nothing, silently.
        if self.ast.decls.get(d).has(DeclFlags::INVALID)
            && !self.ast.decls.get(d).kind().is_function()
        {
            return self.recover_err(&[], loc);
        }
        let qualified = nns.is_some();
        // The exception specification is part of the type ([dcl.fct]/14):
        // resolved before the reference takes it (clang's
        // `DiagnoseUseOfDecl`; `&A::operator==` of a defaulted `==`).
        if self.kind_of(d) == DeclKind::CXXMethod {
            self.resolve_exc_for_type(d, loc);
            // So is a placeholder return type: `&C::f<int>` instantiates
            // the definition of a member function template specialization
            // to deduce it ([dcl.spec.auto.general]/12, clang
            // `DeduceReturnType`).
            if self.check_undeduced_use(d, loc) {
                return self.recover_err(&[], loc);
            }
        }
        let declared = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        let ty = self.non_reference(declared);
        self.ast.decls.set_flags(d, DeclFlags::REFERENCED);
        if self.ast.decls.get(d).kind().is_function() {
            self.diagnose_deleted_use(d, loc);
        }
        let nou = if self.unevaluated > 0 { NonOdrUse::Unevaluated } else { NonOdrUse::None };
        // The qualifier is kept so that a substitution rebuilds `&C::m`
        // (a local class of a function template) as qualified.
        let extra = (shadow.is_some() || nns.is_some()).then_some(DeclRefExtra {
            qualifier: nns,
            template_args: None,
            found_decl: shadow,
        });
        let e = self.builder().decl_ref(d, ty, loc, extra, nou, ExprFlags::NONE);
        let error = self.unevaluated == 0 || self.kind_of(d) == DeclKind::CXXMethod;
        self.member_refs.insert(e, MemberRef { decl: d, error, qualified, begin: None });
        e
    }

    /// True if `b` is an implicit `this` and `decls` an overload set of
    /// non-static member functions (clang builds an implicit-access
    /// `UnresolvedMemberExpr` for it, diagnosed without an object).
    fn implicit_overload_without_object(&self, b: ExprId, decls: &[DeclId]) -> bool {
        let x = self.ignore_parens(b);
        let implicit = self.ast.exprs.get(x).kind() == ExprKind::CXXThisExpr
            && self.ast.exprs.this_is_implicit(x);
        let overloaded = decls.len() > 1
            || decls.first().is_some_and(|&d| self.kind_of(d) == DeclKind::FunctionTemplate);
        implicit
            && overloaded
            && decls.iter().all(|&d| {
                let f = if self.kind_of(d) == DeclKind::FunctionTemplate {
                    self.templated_function(d)
                } else {
                    Some(d)
                };
                !f.is_some_and(|f| self.is_static_method(f))
            })
    }

    /// `base.name` / `base->name` once the object type is known
    /// ([expr.ref]/2-7): class member lookup in `def` (the object's
    /// complete class), ambiguity and access checks, then the access.
    /// `b` is the (decayed, for `->`) object expression.
    #[allow(clippy::too_many_arguments, reason = "the operands of a member access")]
    pub(crate) fn member_access(
        &mut self,
        base: ExprId,
        b: ExprId,
        obj_ty: QualType,
        def: DeclId,
        arrow: bool,
        name: kiln_ast::names::DeclName,
        qual: Option<crate::using::Qualifier>,
        loc: Loc,
    ) -> ExprId {
        // `e.N::m` ([expr.ref]/4, [basic.lookup.qual]/1): `m` is looked
        // up in the qualifier's scope, which must be the object's class
        // or one of its bases.
        let look = qual.map_or(def, |q| q.ctx);
        self.declare_implicit_named(look, name);
        let found = match qual {
            Some(q) => self.lookup_qualified_dn(q.ctx, name, LookupKind::Ordinary),
            None => self.lookup_in_class_dn(def, name, LookupKind::Ordinary),
        };
        if self.diagnose_member_ambiguity(&found, name, loc) {
            return self.recover_err(&[base], loc);
        }
        let Some(&m) = found.decls.first() else {
            // The class is named without the object's cv-qualifiers
            // (clang prints the record, `in 'It'` for a `const It&`).
            let desc = if let Some(q) = qual {
                self.qualifier_desc(q.ctx)
            } else {
                let t = self.ast.types.unqualified(obj_ty);
                self.ty_arg(t)
            };
            let msg =
                self.diag(DiagId::sema_no_member, loc).arg(self.decl_name_str(name)).arg(desc);
            self.emit(msg);
            return self.recover_err(&[base], loc);
        };
        if let Some(q) = qual
            && !self.qualifier_names_base(q.ctx, def)
        {
            if self.implicit_overload_without_object(b, &found.decls) {
                // clang's `DiagnoseQualifiedMemberReference` of an
                // implicit-access overload set (`B::f()` through a
                // dependent base that is not `B`): `diagnoseInstanceReference`.
                let end = self.token_end(loc).unwrap_or(loc);
                let msg = self
                    .diag(DiagId::sema_member_call_without_object, loc)
                    .range(kiln_source::Span::new(q.loc, end));
                self.emit(msg);
                return self.recover_err(&[base], loc);
            }
            // clang's `CheckQualifiedMemberReference`.
            let msg = self
                .diag(DiagId::sema_qualified_member_of_unrelated, loc)
                .arg(format!("'{}'", self.qualified_decl_name(m)))
                .arg(self.ty_arg(obj_ty));
            self.emit(msg);
            return self.recover_err(&[base], loc);
        }
        let nns = qual.map(|q| q.nns);
        if found.decls.len() > 1 || self.kind_of(m) == DeclKind::FunctionTemplate {
            if self.placeholder_ambiguity(&found.decls) {
                // [basic.scope.scope]/5 (P2169): several members `_`.
                let text = self.decl_name_str(name);
                self.diagnose_placeholder_ambiguity(text, &found.decls, true, loc);
                return self.recover_err(&[base], loc);
            }
            if !self.is_member_overload_set(&found.decls) {
                self.unsupported("this kind of overloaded member access", loc);
                return self.recover_err(&[base], loc);
            }
            // spec/sema-overload.md §2.3: resolved by the call.
            let set = crate::overload::OvlMember {
                name,
                decls: found.decls.clone(),
                found: (0..found.decls.len())
                    .map(|i| found.found.get(i).copied().unwrap_or(found.decls[i]))
                    .collect(),
                naming: found.naming.unwrap_or(look),
                obj_ty,
                def,
                base: b,
                arrow,
                nns,
                // An instantiated implicit `this->f` stays implicit
                // (a static winner is a `DeclRefExpr`, [over.over]).
                implicit: self.ast.exprs.get(base).kind() == ExprKind::CXXThisExpr
                    && self.ast.exprs.this_is_implicit(base),
                targs: None,
            };
            return self.build_member_overload(set, loc);
        }
        if crate::lookup::is_type_kind(self.kind_of(m)) {
            let text = self.decl_name_str(name);
            let note = self
                .diag(DiagId::note_member_declared_named, self.ast.decls.get(m).loc())
                .arg(text.clone());
            let msg = self
                .diag(DiagId::sema_member_type_via_access, loc)
                .arg(text)
                .arg(self.ty_arg(obj_ty))
                .arg(u8::from(arrow))
                .with_note(note);
            self.emit(msg);
            return self.recover_err(&[base], loc);
        }
        let naming = found.naming.unwrap_or(look);
        let via = found.found.first().copied().unwrap_or(m);
        self.check_member_access(via, naming, Some(def), loc);
        // clang `DiagnoseUseOfDecl` in `BuildMemberReferenceExpr`.
        self.diagnose_deprecated(m, loc);
        self.check_sve_use(m, loc);
        self.member_of_object(b, obj_ty, def, arrow, m, found.shadow(0), nns, loc)
    }

    /// True if qualifier context `ctx` names class `def` or one of its
    /// bases ([expr.ref]/4: a qualified member of the object's class).
    pub(crate) fn qualifier_names_base(&self, ctx: DeclId, def: DeclId) -> bool {
        self.kind_of(ctx) == DeclKind::CXXRecord
            && (self.ast.first(ctx) == self.ast.first(def) || is_derived_from(self, def, ctx))
    }

    /// `base.~name` / `base->~name` ([expr.ref]/1 with a destructor name,
    /// [expr.prim.id.dtor]): `name` is looked up as a type in the object's
    /// class and then in the context of the expression
    /// ([basic.lookup.unqual]/6 for `~`). On a class object it names the
    /// destructor (a `MemberExpr` naming `~A`); on a scalar or a dependent
    /// object it is a `CXXPseudoDestructorExpr` (clang's
    /// `ActOnPseudoDestructorExpr`). `tilde` is the `~`, `loc` the name,
    /// `op_loc` the `.`/`->` (WP-121).
    pub fn act_on_member_destructor(
        &mut self,
        base: ExprId,
        name: kiln_source::symbol::Symbol,
        tilde: Loc,
        loc: Loc,
        op_loc: Loc,
        arrow: bool,
    ) -> ExprId {
        let obj = match self.destructor_object(base, loc, op_loc, arrow) {
            Ok(o) => o,
            Err(e) => return e,
        };
        // The type-name after `~`: in the object's class, then here.
        let in_class = obj.def.and_then(|d| {
            let found = self.lookup_in_class(d, name, LookupKind::Ordinary);
            self.type_named_by(&found)
        });
        let named = in_class.or_else(|| {
            let found = self.lookup_name(name, LookupKind::Ordinary);
            self.type_named_by(&found)
        });
        let named = self.injected_named(named);
        let Some((destroyed, td)) = named else {
            let text = self.ast.symbols().as_str(name).to_owned();
            let msg = if self.ast.types.is_dependent(obj.ty) {
                // Looked up in the object's class at instantiation
                // ([basic.lookup.qual.general]/4; clang keeps the
                // identifier in the `CXXPseudoDestructorExpr`).
                return self.dependent_destructor_name(obj, name, loc, op_loc);
            } else if obj.def.is_some() {
                self.diag(DiagId::sema_dtor_name_undeclared, loc).arg(text)
            } else {
                self.diag(DiagId::sema_pseudo_dtor_not_type, loc).arg(text).arg(self.ty_arg(obj.ty))
            };
            self.emit(msg);
            return self.recover_err(&[obj.base], loc);
        };
        // clang's destructor name starts at the `~` (`GetNameFromUnqualifiedId`):
        // a use of a deleted destructor is diagnosed there.
        let name_loc = if tilde.is_valid() { tilde } else { loc };
        self.finish_member_destructor(
            obj,
            destroyed,
            Some(td),
            DtorQual::None,
            loc,
            name_loc,
            op_loc,
        )
    }

    /// `p->~N()` on a dependent object where `N` names no type yet: a
    /// `CXXPseudoDestructorExpr` keeping the identifier
    /// ([`kiln_ast::templates::TemplateTables::pseudo_dtor_name`]).
    fn dependent_destructor_name(
        &mut self,
        obj: DtorObject,
        name: kiln_source::symbol::Symbol,
        loc: Loc,
        op_loc: Loc,
    ) -> ExprId {
        let e =
            self.builder().pseudo_destructor_q(obj.b, obj.arrow, false, QualType::DEPENDENT, loc);
        let _ = self.ast.templates.pseudo_dtor_name.insert(e, name);
        if op_loc != loc {
            let _ = self.ast.templates.member_op_loc.insert(e, op_loc);
        }
        e
    }

    /// The instantiation of [`Sema::dependent_destructor_name`]: `~name`
    /// looked up in the class of the substituted object `base` only
    /// (clang `getDestructorName` without a scope), as
    /// [`Sema::act_on_member_destructor`] otherwise.
    pub(crate) fn act_on_member_destructor_late(
        &mut self,
        base: ExprId,
        name: kiln_source::symbol::Symbol,
        loc: Loc,
        op_loc: Loc,
        arrow: bool,
    ) -> ExprId {
        let obj = match self.destructor_object(base, loc, op_loc, arrow) {
            Ok(o) => o,
            Err(e) => return e,
        };
        if self.ast.types.is_dependent(obj.ty) {
            return self.dependent_destructor_name(obj, name, loc, op_loc);
        }
        let named = obj.def.and_then(|d| {
            let found = self.lookup_in_class(d, name, LookupKind::Ordinary);
            self.type_named_by(&found)
        });
        let named = self.injected_named(named);
        let Some((destroyed, td)) = named else {
            let text = self.ast.symbols().as_str(name).to_owned();
            // clang `getDestructorName` finds nothing in any object type.
            let msg = self.diag(DiagId::sema_dtor_name_undeclared, loc).arg(text);
            self.emit(msg);
            return self.recover_err(&[obj.base], loc);
        };
        self.finish_member_destructor(obj, destroyed, Some(td), DtorQual::None, loc, loc, op_loc)
    }

    /// True if a member access on `base` that continues `type-name :: ~`
    /// is a pseudo-destructor-name ([expr.prim.id.dtor]/2): the object is
    /// not of class type, or is dependent. On a class object the
    /// nested-name-specifier is an ordinary qualifier ([expr.ref]/4;
    /// clang's `MayBePseudoDestructor` from `ActOnStartCXXMemberReference`).
    pub fn may_be_pseudo_destructor(&self, base: ExprId, arrow: bool) -> bool {
        if self.is_err_expr(base) {
            return false;
        }
        let bt = self.ety(base);
        let obj = if !arrow {
            bt
        } else if let Some(p) = self.ast.types.pointee(bt) {
            p
        } else {
            return self.ast.types.is_dependent(bt);
        };
        self.ast.types.is_dependent(obj) || self.ast.types.as_record(obj).is_none()
    }

    /// A qualified destructor name in a member access
    /// ([expr.prim.id.dtor], [basic.lookup.qual.general]/4):
    ///
    /// - `x.N::~Q` on a class object (`first` is `None`; `qual` is `N`, the
    ///   whole nested-name-specifier): `Q` is looked up in `N`, then in the
    ///   object's class, then here; the result names the destructor of
    ///   that class as a member of `N` ([expr.ref]/4), so `N` must be the
    ///   object's class or a base, and the call is not virtual
    ///   ([class.virtual]/16).
    /// - `p->S::F::~Q` / `p->F::~Q` on a scalar or dependent object
    ///   (`first` is `F`, `qual` the prefix `S` if any): `F` and `Q` are
    ///   looked up in `S` when present, else here, and both must name the
    ///   object's type (clang's `ActOnPseudoDestructorExpr` with a
    ///   `FirstTypeName`).
    ///
    /// `tilde` is the `~`, `loc` the name after it, `op_loc` the `.`/`->`.
    #[allow(clippy::too_many_arguments, reason = "the parts of a member access")]
    pub fn act_on_member_destructor_q(
        &mut self,
        base: ExprId,
        qual: Option<crate::using::Qualifier>,
        first: Option<(kiln_source::symbol::Symbol, Loc)>,
        name: kiln_source::symbol::Symbol,
        tilde: Loc,
        loc: Loc,
        op_loc: Loc,
        arrow: bool,
    ) -> ExprId {
        let Some((fname, floc)) = first else {
            let Some(q) = qual else {
                return self.act_on_member_destructor(base, name, tilde, loc, op_loc, arrow);
            };
            return self.member_destructor_in(base, q, name, tilde, loc, op_loc, arrow);
        };
        let obj = match self.destructor_object(base, loc, op_loc, arrow) {
            Ok(o) => o,
            Err(e) => return e,
        };
        let scope = qual.map(|q| q.ctx);
        if scope.is_some_and(|c| self.is_dependent_ctx(c)) {
            self.templ_unimplemented("a pseudo-destructor name in a dependent scope", floc);
            return self.recover_err(&[obj.base], loc);
        }
        let look = |s: &Self, n| {
            let found = match scope {
                Some(c) => s.lookup_qualified(c, n, LookupKind::Ordinary),
                None => s.lookup_name(n, LookupKind::Ordinary),
            };
            s.type_named_by(&found)
        };
        let dependent = self.ast.types.is_dependent(obj.ty);
        let t1 = look(self, fname);
        // `Q` where the destructor name appears; then in the class `F`
        // names (`p->T::~T` where only `T`'s scope declares `T`).
        let named = look(self, name).or_else(|| {
            let d = t1.and_then(|(t, _)| self.ast.types.as_record(t))?;
            let d = self.ast.definition(d)?;
            let found = self.lookup_in_class(d, name, LookupKind::Ordinary);
            self.type_named_by(&found)
        });
        let (t1, named) = (self.injected_named(t1), self.injected_named(named));
        // `p.F::~T()` with `p` a pointer to `T`: "use '->'" and the access
        // goes on as `p->F::~T()`, the scope type then checked against
        // `T` (clang `BuildPseudoDestructorExpr` checks the destroyed type
        // first; see `finish_member_destructor`).
        if let (Some((t, td)), Some((f, fd))) = (named, t1)
            && !arrow
            && !dependent
            && let Some(p) = self.ast.types.pointee(obj.ty)
            && !self.is_dependent_type(t)
            && self.same_unqual(p, t)
        {
            let msg = self
                .diag(DiagId::sema_member_reference_suggestion, op_loc)
                .arg(self.ty_arg(obj.ty))
                .arg("0");
            self.emit(msg);
            if !self.is_dependent_type(f) && !self.same_unqual(p, f) {
                let msg =
                    self.diag(DiagId::sema_pseudo_dtor_type_mismatch, floc).diff_args(self, p, f);
                self.emit(msg);
            }
            self.mark_type_referenced(fd);
            self.mark_type_referenced(td);
            let pb = conv::decay(self, obj.b);
            return self.builder().pseudo_destructor_q(pb, true, true, t, loc);
        }
        let mut bad = false;
        for (n, at, ty) in [(name, loc, named), (fname, floc, t1)] {
            match ty {
                None if dependent => {
                    self.templ_unimplemented("a dependent destructor name", at);
                    return self.recover_err(&[obj.base], loc);
                }
                None => {
                    let text = self.ast.symbols().as_str(n).to_owned();
                    let msg = self
                        .diag(DiagId::sema_pseudo_dtor_not_type, at)
                        .arg(text)
                        .arg(self.ty_arg(obj.ty));
                    self.emit(msg);
                    bad = true;
                }
                Some((t, _)) if obj.def.is_none() && !dependent && !self.is_dependent_type(t) => {
                    if self.is_scalar(obj.ty) && !self.same_unqual(obj.ty, t) {
                        let msg = self
                            .diag(DiagId::sema_pseudo_dtor_type_mismatch, at)
                            .diff_args(self, obj.ty, t);
                        self.emit(msg);
                        bad = true;
                    }
                }
                Some(_) => {}
            }
        }
        let Some((destroyed, td)) = named.filter(|_| !bad) else {
            return self.recover_err(&[obj.base], loc);
        };
        for (_, d) in t1.iter().chain(named.iter()) {
            self.mark_type_referenced(*d);
        }
        let name_loc = if tilde.is_valid() { tilde } else { loc };
        self.finish_member_destructor(
            obj,
            destroyed,
            Some(td),
            DtorQual::Implied,
            loc,
            name_loc,
            op_loc,
        )
    }

    /// `x.N::~Q` on a class object (see [`Sema::act_on_member_destructor_q`]).
    #[allow(clippy::too_many_arguments, reason = "the parts of a member access")]
    fn member_destructor_in(
        &mut self,
        base: ExprId,
        q: crate::using::Qualifier,
        name: kiln_source::symbol::Symbol,
        tilde: Loc,
        loc: Loc,
        op_loc: Loc,
        arrow: bool,
    ) -> ExprId {
        let obj = match self.destructor_object(base, loc, op_loc, arrow) {
            Ok(o) => o,
            Err(e) => return e,
        };
        let in_qual = || {
            let found = self.lookup_qualified(q.ctx, name, LookupKind::Ordinary);
            self.type_named_by(&found)
        };
        let named = (!self.is_dependent_ctx(q.ctx))
            .then(in_qual)
            .flatten()
            .or_else(|| {
                let found = self.lookup_in_class(obj.def?, name, LookupKind::Ordinary);
                self.type_named_by(&found)
            })
            .or_else(|| {
                let found = self.lookup_name(name, LookupKind::Ordinary);
                self.type_named_by(&found)
            });
        let named = self.injected_named(named);
        let Some((destroyed, td)) = named else {
            // `p->Base<T>::~Base()` with a dependent qualifier: `~Base`
            // names the class the qualifier denotes ([basic.lookup.qual]/
            // 4 — the injected-class-name of the same class template),
            // rebuilt as a qualified destructor member at instantiation.
            if self.is_dependent_ctx(q.ctx)
                && let kiln_ast::types::Nns::Type { ty, .. } = self.ast.types.nns_view(q.nns)
                && self.nns_last_name(q.nns) == Some(name)
            {
                let e = self.builder().pseudo_destructor_q(obj.b, obj.arrow, true, ty, loc);
                if op_loc != loc {
                    let _ = self.ast.templates.member_op_loc.insert(e, op_loc);
                }
                return e;
            }
            if self.ast.types.is_dependent(obj.ty) || self.is_dependent_ctx(q.ctx) {
                self.templ_unimplemented("a dependent destructor name", loc);
            } else {
                let text = self.ast.symbols().as_str(name).to_owned();
                let msg = self.diag(DiagId::sema_dtor_name_undeclared, loc).arg(text);
                self.emit(msg);
            }
            return self.recover_err(&[obj.base], loc);
        };
        let name_loc = if tilde.is_valid() { tilde } else { loc };
        self.finish_member_destructor(
            obj,
            destroyed,
            Some(td),
            DtorQual::Written(q),
            loc,
            name_loc,
            op_loc,
        )
    }

    /// True if `a` and `b` are the same type ignoring top-level cv.
    fn same_unqual(&self, a: QualType, b: QualType) -> bool {
        let x = self.ast.types.canonical(a);
        let y = self.ast.types.canonical(b);
        QualType::unqual(x.ty()) == QualType::unqual(y.ty())
    }

    /// A pseudo-destructor `base.~T` / `base->~T` formed again at
    /// instantiation with the substituted object and destroyed type: a
    /// class type now names its destructor (clang's
    /// `TreeTransform::RebuildCXXPseudoDestructorExpr`).
    /// `qualified`: the name was qualified (`p->T::~T`), so a class type
    /// names its destructor as a qualified member ([expr.ref]/4).
    pub fn rebuild_member_destructor(
        &mut self,
        base: ExprId,
        destroyed: QualType,
        qualified: bool,
        loc: Loc,
        op_loc: Loc,
        arrow: bool,
    ) -> ExprId {
        let qual = if qualified { DtorQual::Implied } else { DtorQual::None };
        match self.destructor_object(base, loc, op_loc, arrow) {
            Ok(obj) => self.finish_member_destructor(obj, destroyed, None, qual, loc, loc, op_loc),
            Err(e) => e,
        }
    }

    /// The object of a destructor member access: `->` through
    /// `operator->` ([over.ref]/1), the pointee for `->`, and a complete
    /// class. `Err` with the recovery expression after diagnosing.
    fn destructor_object(
        &mut self,
        base: ExprId,
        loc: Loc,
        op_loc: Loc,
        arrow: bool,
    ) -> Result<DtorObject, ExprId> {
        if self.is_err_expr(base) {
            return Err(self.recover_err(&[base], loc));
        }
        let bt = self.ety(base);
        let dependent = self.ast.types.is_dependent(bt);
        let (base, arrow) = if arrow && !dependent && self.ast.types.as_record(bt).is_some() {
            match self.arrow_base(base, op_loc) {
                crate::overload::arrow::ArrowBase::Arrow(e) => (e, true),
                crate::overload::arrow::ArrowBase::Dot => (base, false),
                crate::overload::arrow::ArrowBase::Err => {
                    return Err(self.recover_err(&[base], loc));
                }
            }
        } else {
            (base, arrow)
        };
        let bt = self.ety(base);
        let (b, ty) = if arrow {
            if self.ast.types.is_pointer(bt) {
                let p = self.pointee_sugared(bt).unwrap_or(QualType::ERROR);
                (conv::decay_dependent(self, base), p)
            } else if dependent {
                (base, QualType::DEPENDENT)
            } else {
                let msg = self
                    .diag(DiagId::sema_member_reference_suggestion, op_loc)
                    .arg(self.ty_arg(bt))
                    .arg("1");
                self.emit(msg);
                return Err(self.recover_err(&[base], loc));
            }
        } else if !dependent && (self.ast.types.is_array(bt) || self.ast.types.is_function(bt)) {
            let d = conv::decay(self, base);
            (d, self.ety(d))
        } else {
            (base, bt)
        };
        let rec = if self.ast.types.is_dependent(ty) { None } else { self.ast.types.as_record(ty) };
        let mut def = None;
        if let Some(r) = rec {
            let complete = self.this_outside_member_body(r) || self.require_complete(ty, op_loc);
            let Some(d) = self.ast.definition(r).filter(|_| complete) else {
                let msg = self
                    .diag(DiagId::sema_member_reference_incomplete, op_loc)
                    .arg(self.ty_arg(ty));
                self.emit(self.incomplete_note(msg, ty));
                return Err(self.recover_err(&[base], loc));
            };
            def = Some(d);
        }
        Ok(DtorObject { base, b, ty, def, arrow })
    }

    /// The destructor member access of `obj` naming `destroyed` (found as
    /// `td` by destructor name lookup, `None` when substituted). `name_loc`
    /// is where the destructor name begins (the `~`; the destroyed type
    /// when substituted, as clang's `RebuildCXXPseudoDestructorExpr`).
    #[allow(clippy::too_many_arguments, reason = "the parts of a member access")]
    fn finish_member_destructor(
        &mut self,
        obj: DtorObject,
        destroyed: QualType,
        td: Option<DeclId>,
        qual: DtorQual,
        loc: Loc,
        name_loc: Loc,
        op_loc: Loc,
    ) -> ExprId {
        let DtorObject { base, b, ty: obj_ty, def, arrow } = obj;
        if self.is_error_type(destroyed) {
            return self.recover_err(&[base], loc);
        }
        // Inside a class template, its injected-class-name names the
        // dependent `X<P...>` ([temp.local]/1), substituted with the class:
        // `this->~X()` in libstdc++ `_Sp_counted_ptr_inplace::_M_destroy`.
        let destroyed = match td {
            Some(td) if destroyed == self.tag_type(td) => {
                self.injected_tag_type(td).unwrap_or(destroyed)
            }
            _ => destroyed,
        };
        if self.ast.types.is_dependent(obj_ty) || self.ast.types.is_dependent(destroyed) {
            let qualified = !matches!(qual, DtorQual::None);
            let e = self.builder().pseudo_destructor_q(b, arrow, qualified, destroyed, loc);
            // The `.`/`->`, where an instantiation diagnoses the access.
            if op_loc != loc {
                let _ = self.ast.templates.member_op_loc.insert(e, op_loc);
            }
            return e;
        }
        let same = self.same_unqual(obj_ty, destroyed);
        if let Some(def) = def
            && !matches!(qual, DtorQual::None)
        {
            // `x.N::~Q`: the destructor of the class `Q` names, a member of
            // `N` (the class itself when the qualifier was substituted);
            // `member_access` checks that `N` is the object's class or a
            // base ([expr.ref]/4) and records the qualifier, so the call
            // is not virtual ([class.virtual]/16).
            let class = self.ast.types.as_record(destroyed).and_then(|r| self.ast.definition(r));
            let q = if let DtorQual::Written(q) = qual {
                q
            } else {
                let Some(c) = class else {
                    let msg = self
                        .diag(DiagId::sema_dtor_type_mismatch, loc)
                        .arg(self.ty_arg(destroyed))
                        .arg(self.ty_arg(obj_ty));
                    self.emit(msg);
                    return self.recover_err(&[base], loc);
                };
                let ty = self.class_type(c);
                let nns = self.ast.types.nns(kiln_ast::types::Nns::Type {
                    prefix: None,
                    ty,
                    template_kw: false,
                });
                crate::using::Qualifier { ctx: c, nns, loc: name_loc }
            };
            let canon = self.ast.types.canonical(destroyed).ty();
            if let Some(c) = class {
                self.ensure_implicit(c, crate::class::special::SpecialKind::Dtor);
            }
            let dn = self.ast.names.destructor(canon);
            return self.member_access(base, b, obj_ty, def, arrow, dn, Some(q), name_loc);
        }
        if let Some(def) = def {
            if !same {
                let mut msg = self
                    .diag(DiagId::sema_dtor_type_mismatch, loc)
                    .arg(self.ty_arg(destroyed))
                    .arg(self.ty_arg(obj_ty));
                if let Some(td) = td {
                    let note = self
                        .diag(DiagId::note_dtor_type_found, self.ast.decls.get(td).loc())
                        .arg(self.ty_arg(destroyed));
                    msg = msg.with_note(note);
                }
                self.emit(msg);
                return self.recover_err(&[base], loc);
            }
            self.ensure_implicit(def, crate::class::special::SpecialKind::Dtor);
            let canon = self.ast.types.canonical(self.class_type(def)).ty();
            let dn = self.ast.names.destructor(canon);
            return self.member_access(base, b, obj_ty, def, arrow, dn, None, name_loc);
        }
        if !self.is_scalar(obj_ty) {
            let msg = self
                .diag(DiagId::sema_pseudo_dtor_base_not_scalar, op_loc)
                .arg(self.ty_arg(obj_ty));
            self.emit(msg);
            return self.recover_err(&[base], loc);
        }
        if !same {
            // `p.~T()` with `p` a pointer to `T`: a pseudo-destructor of
            // the pointer only when the name denotes the pointer type
            // ([expr.prim.id.dtor]/2, [expr.ref]/2); naming the pointee
            // suggests `->` (clang `BuildPseudoDestructorExpr`), and the
            // access goes on as `p->~T()`: the call is a `void`
            // expression whatever the pointee (a class too, as clang
            // recovers it).
            if !arrow
                && self.ast.types.is_pointer(obj_ty)
                && self.ast.types.pointee(obj_ty).is_some_and(|p| self.same_unqual(p, destroyed))
            {
                let msg = self
                    .diag(DiagId::sema_member_reference_suggestion, op_loc)
                    .arg(self.ty_arg(obj_ty))
                    .arg("0");
                self.emit(msg);
                let qualified = !matches!(qual, DtorQual::None);
                let p = conv::decay(self, b);
                return self.builder().pseudo_destructor_q(p, true, qualified, destroyed, loc);
            }
            let msg = self
                .diag(DiagId::sema_pseudo_dtor_type_mismatch, loc)
                .diff_args(self, obj_ty, destroyed);
            self.emit(msg);
            return self.recover_err(&[base], loc);
        }
        let qualified = !matches!(qual, DtorQual::None);
        self.builder().pseudo_destructor_q(b, arrow, qualified, destroyed, loc)
    }

    /// A destructor's type-name found as the injected-class-name of a
    /// class template's pattern or partial specialization, inside it,
    /// names the injected-class-name type `X<P...>` ([temp.local]/1),
    /// which instantiation substitutes, not the pattern's own record
    /// (`this->~X()` in a member of `X<T>`, [expr.prim.id.dtor]/2).
    pub(crate) fn injected_named(
        &mut self,
        n: Option<(QualType, DeclId)>,
    ) -> Option<(QualType, DeclId)> {
        let (ty, d) = n?;
        let d0 = self.ast.strip_shadow(d);
        Some((self.injected_tag_type(d0).unwrap_or(ty), d))
    }

    /// The type and type declaration a lookup result names, if it names a
    /// type (the type-name of a destructor name).
    pub(crate) fn type_named_by(&self, found: &crate::lookup::Found) -> Option<(QualType, DeclId)> {
        let &d = found.decls.first()?;
        let d = self.ast.strip_shadow(d);
        let k = self.kind_of(d);
        if !crate::lookup::is_type_kind(k) {
            return None;
        }
        let ty =
            if matches!(k, DeclKind::Typedef | DeclKind::TypeAlias | DeclKind::TemplateTypeParm) {
                self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR)
            } else {
                self.tag_type(d)
            };
        Some((ty, d))
    }

    /// Member `m` of the object `b` (a pointer for `arrow`) of class type
    /// `obj_ty` (complete class `obj_class`), named with qualifier `nns`
    /// if any (recorded: a qualified call is not virtual,
    /// [class.virtual]/16).
    #[allow(clippy::too_many_arguments, reason = "the operands of a member access")]
    pub(crate) fn member_of_object(
        &mut self,
        b: ExprId,
        obj_ty: QualType,
        obj_class: DeclId,
        arrow: bool,
        m: DeclId,
        shadow: Option<DeclId>,
        nns: Option<kiln_ast::NnsId>,
        loc: Loc,
    ) -> ExprId {
        let quals = self.ast.types.effective_quals(obj_ty);
        let extra = (shadow.is_some() || nns.is_some()).then_some(DeclRefExtra {
            qualifier: nns,
            template_args: None,
            found_decl: shadow,
        });
        let kind = self.kind_of(m);
        if kind.is_function() && self.ast.decls.get(m).has(DeclFlags::INVALID) {
            // An already-invalid member function: erroneous, undiagnosed
            // (as a reference to it by name, expr.rs).
            return self.recover_err(&[b], loc);
        }
        match kind {
            DeclKind::Field | DeclKind::IndirectField => {
                let Some(target) = self.declaring_class(m) else {
                    self.unsupported("this kind of member access", loc);
                    return self.recover_err(&[b], loc);
                };
                let written = self.member_qualifier_type(nns, target);
                let b = self.object_to_class_as(
                    b,
                    arrow,
                    obj_class,
                    quals,
                    (target, written),
                    quals,
                    CastKind::UncheckedDerivedToBase,
                );
                if self.is_err_expr(b) {
                    return b;
                }
                if kind == DeclKind::Field {
                    // The qualifier as written stays on the access (clang's
                    // `MemberExpr`): instantiation looks `B::m` up in `B`
                    // only, never in the derived class ([class.qual]/1).
                    let q = nns.map(|q| DeclRefExtra {
                        qualifier: Some(q),
                        template_args: None,
                        found_decl: None,
                    });
                    return self.field_access_extra(b, m, quals, arrow, loc, q);
                }
                let chain: Vec<DeclId> =
                    self.ast.lists.iter::<DeclId>(self.ast.indirect_field_chain(m)).collect();
                self.ast.decls.set_flags(m, DeclFlags::REFERENCED);
                let mut cur = b;
                let mut arrow_now = arrow;
                let mut q = quals;
                for f in chain {
                    cur = self.field_access(cur, f, q, arrow_now, loc);
                    q = self.ast.types.effective_quals(self.ety(cur));
                    arrow_now = false;
                }
                cur
            }
            DeclKind::Var | DeclKind::EnumConstant | DeclKind::CXXMethod
                if kind != DeclKind::CXXMethod || self.is_static_method(m) =>
            {
                self.ast.decls.set_flags(m, DeclFlags::REFERENCED);
                if kind == DeclKind::Var {
                    self.note_var_referenced(m, loc);
                }
                if kind == DeclKind::CXXMethod {
                    if self.diagnose_unsatisfied_use(m, loc) {
                        return self.recover_err(&[b], loc);
                    }
                    self.diagnose_deleted_use(m, loc);
                    // `x.f` of a static member function takes its type:
                    // a placeholder return type is deduced first, as for
                    // `C::f` ([dcl.spec.auto.general]/12, clang
                    // `DeduceReturnType` in `BuildMemberExpr`).
                    if self.check_undeduced_use(m, loc) {
                        return self.recover_err(&[b], loc);
                    }
                }
                let ty = self.non_reference(self.ast.decls.get(m).ty().unwrap_or(QualType::ERROR));
                let e = self.build_member_expr(b, m, ty, arrow, loc, extra);
                if self.unevaluated == 0 && kind == DeclKind::CXXMethod {
                    self.mark_function_referenced(m, loc);
                } else if kind == DeclKind::CXXMethod {
                    self.exc_spec_needed(m, loc);
                } else if self.unevaluated == 0 && kind != DeclKind::EnumConstant {
                    self.mark_used_at(m, loc);
                }
                e
            }
            k if k.is_function() && k != DeclKind::Function => {
                self.ast.decls.set_flags(m, DeclFlags::REFERENCED);
                // A member whose constraints are not satisfied names
                // nothing ([expr.prim.id.general]/4; clang's failed
                // `DiagnoseUseOfDecl` yields `ExprError`): the call never
                // deduces its placeholder return type, so the definition
                // is not instantiated ([temp.inst]/5), and in a
                // requires-expression the requirement is just unsatisfied.
                if self.diagnose_unsatisfied_use(m, loc) {
                    return self.recover_err(&[b], loc);
                }
                self.diagnose_deleted_use(m, loc);
                self.build_member_expr(b, m, QualType::BOUND_MEMBER, arrow, loc, extra)
            }
            _ => {
                self.unsupported("this kind of member access", loc);
                self.recover_err(&[b], loc)
            }
        }
    }

    /// A prvalue class object `e` materialized to an xvalue
    /// ([conv.rval]) for a member access.
    fn materialize_object(&mut self, e: ExprId) -> ExprId {
        let n = *self.ast.exprs.get(e);
        if n.value_category() != ValueCategory::PRValue
            || self.ast.types.as_record(n.ty()).is_none()
        {
            return e;
        }
        self.materialize_temporary(e, n.ty(), false)
    }

    /// clang 22 (`PerformObjectMemberConversion`): a field access
    /// qualified by its declaring class casts the object to the
    /// qualifier's type as written, which keeps its own qualifier
    /// (`d.::B::m` casts to `::B`, without the object's qualifiers;
    /// spec/sema.md §1.1). `None` at 19 and
    /// for any other qualifier.
    fn member_qualifier_type(
        &mut self,
        nns: Option<kiln_ast::NnsId>,
        target: DeclId,
    ) -> Option<QualType> {
        if self.clang_compat < 22 {
            return None;
        }
        let kiln_ast::types::Nns::Type { prefix, ty, .. } = self.ast.types.nns_view(nns?) else {
            return None;
        };
        let r = self.ast.types.as_record(ty)?;
        if self.ast.definition(r).unwrap_or(r) != self.ast.definition(target).unwrap_or(target) {
            return None;
        }
        Some(match prefix {
            Some(p) => self.ast.types.elaborated(kiln_ast::types::TagKeyword::None, Some(p), ty),
            None => ty,
        })
    }

    /// Converts the object `b` (a pointer for `arrow`) of class
    /// `obj_class` with qualifiers `obj_q` to an object of its class or
    /// base class `target` with qualifiers `to_q`: a `NoOp` cast adding
    /// qualifiers, or a derived-to-base cast of kind `ck`
    /// (clang `PerformObjectMemberConversion` /
    /// `PerformImplicitObjectArgumentInitialization`).
    #[allow(clippy::too_many_arguments, reason = "the object and both class types")]
    fn object_to_class(
        &mut self,
        b: ExprId,
        arrow: bool,
        obj_class: DeclId,
        obj_q: Quals,
        target: DeclId,
        to_q: Quals,
        ck: CastKind,
    ) -> ExprId {
        self.object_to_class_as(b, arrow, obj_class, obj_q, (target, None), to_q, ck)
    }

    /// [`Self::object_to_class`] with the derived-to-base cast typed as
    /// `written` (the qualifier's type as written) when it is given.
    #[allow(clippy::too_many_arguments, reason = "the object and both class types")]
    pub(crate) fn object_to_class_as(
        &mut self,
        b: ExprId,
        arrow: bool,
        obj_class: DeclId,
        obj_q: Quals,
        (target, written): (DeclId, Option<QualType>),
        to_q: Quals,
        ck: CastKind,
    ) -> ExprId {
        let b = if arrow { b } else { self.materialize_object(b) };
        let loc = self.eloc(b);
        let vk =
            if arrow { ValueCategory::PRValue } else { self.ast.exprs.get(b).value_category() };
        let to_ty = |s: &mut Self, class: DeclId| {
            // clang converts to the bare `RecordType`, which prints scoped.
            let t = s.scoped_class_type(class);
            let t = if to_q.is_empty() { t } else { s.ast.types.qualify(t, to_q) };
            if arrow { s.ast.types.pointer_to(t) } else { t }
        };
        if same_class(self, obj_class, target) {
            if (obj_q & CV) == (to_q & CV) {
                return b;
            }
            let ty = to_ty(self, obj_class);
            // clang's `ImpCastExprToType` folds a no-op cast into the
            // one below it (the xvalue of an implicit move).
            let b = if !arrow
                && self.ast.exprs.get(b).kind() == ExprKind::ImplicitCastExpr
                && self.ast.exprs.cast_kind(b) == CastKind::NoOp
            {
                self.ast.exprs.operand(b)
            } else {
                b
            };
            return self.builder().cast(
                ExprKind::ImplicitCastExpr,
                CastKind::NoOp,
                b,
                ty,
                vk,
                None,
                loc,
                ExprFlags::NONE,
            );
        }
        match find_base(self, obj_class, target) {
            BaseSearch::Unique(p) => {
                if ck == CastKind::DerivedToBase {
                    let (d, t) = (self.tag_type(obj_class), self.tag_type(target));
                    self.check_base_access(d, t, &p, loc);
                }
                let list = path_list(self, &p);
                let ty = match written {
                    // clang's qualifier type carries no qualifiers.
                    Some(w) if arrow => self.ast.types.pointer_to(w),
                    Some(w) => w,
                    None => to_ty(self, target),
                };
                self.builder().cast(
                    ExprKind::ImplicitCastExpr,
                    ck,
                    b,
                    ty,
                    vk,
                    Some(list),
                    loc,
                    ExprFlags::NONE,
                )
            }
            BaseSearch::Ambiguous(paths) => {
                let d = self.tag_type(obj_class);
                let shown = ambiguous_paths_string(self, d, &paths);
                let msg = self
                    .diag(DiagId::sema_ambiguous_derived_to_base, loc)
                    .arg(self.ty_arg(d))
                    .arg(self.ty_arg(self.tag_type(target)))
                    .arg(shown);
                self.emit(msg);
                self.recover_err(&[b], loc)
            }
            BaseSearch::NotDerived => b,
        }
    }

    /// The implicit object argument `obj` (an object, not a pointer) of a
    /// call of the member operator function `m`, converted to `cv C` for
    /// `m`'s class `C` (clang `PerformImplicitObjectArgumentInitialization`
    /// after overload resolution checked the binding).
    pub(crate) fn operator_object(&mut self, obj: ExprId, m: DeclId) -> ExprId {
        let obj_ty = self.ety(obj);
        let Some(c) = self.ast.types.as_record(obj_ty) else { return obj };
        let obj_class = self.ast.definition(c).unwrap_or(c);
        let Some(declaring) = self.declaring_class(m) else { return obj };
        let (mq, rq) = self.method_object_quals(m);
        let oq = self.ast.types.effective_quals(obj_ty);
        let to_q = (oq & CV) | (mq & CV);
        // clang materializes a prvalue object as an lvalue unless `m` is
        // `&&`-qualified.
        let obj = if self.ast.exprs.get(obj).value_category() == ValueCategory::PRValue {
            self.materialize_temporary(obj, obj_ty, rq != RefQual::RValue)
        } else {
            obj
        };
        self.object_to_class(
            obj,
            false,
            obj_class,
            oq,
            declaring,
            to_q,
            CastKind::UncheckedDerivedToBase,
        )
    }

    /// A call whose callee is a bound non-static member function (a
    /// member access or `.*` / `->*`, [expr.call]/2), or names one with
    /// no object; `None` for other callees. `braced` as for
    /// [`Sema::convert_call_args`].
    pub(crate) fn try_member_call(
        &mut self,
        callee: ExprId,
        args: &[ExprId],
        braced: &mut [Option<InitList>],
        loc: Loc,
        rparen: Loc,
    ) -> Option<ExprId> {
        let mut kids = vec![callee];
        kids.extend_from_slice(args);
        let x = self.ignore_parens(callee);
        let resolved = self.ovl.resolved_member.take();
        if let Some(r) = self.member_refs.get(&x).copied()
            && r.error
            && self.kind_of(r.decl) == DeclKind::CXXMethod
        {
            self.member_refs.remove(&x);
            self.error(DiagId::sema_member_call_without_object, self.eloc(x));
            return Some(self.recover_err(&kids, loc));
        }
        if self.ety(callee) != QualType::BOUND_MEMBER {
            return None;
        }
        let n = *self.ast.exprs.get(x);
        let (call_callee, method, fn_ty) = match n.kind() {
            ExprKind::MemberExpr if self.has_explicit_object(self.ast.exprs.decl(x)) => {
                let m = self.ast.exprs.decl(x);
                let resolved = resolved == Some(x);
                return Some(self.explicit_object_call(x, m, args, braced, resolved, loc, rparen));
            }
            ExprKind::MemberExpr => {
                let m = self.ast.exprs.decl(x);
                self.check_pure_virtual_call_in_cdtor(x, m);
                let Some(mem) = self.bind_object_argument(x, m) else {
                    return Some(self.recover_err(&kids, loc));
                };
                // `(b.f)(…)` keeps its parentheses.
                let mem = if mem == x { callee } else { self.rebuild_parens(callee, mem) };
                let fn_ty = self.ast.decls.get(m).ty().unwrap_or(QualType::ERROR);
                (mem, Some(m), fn_ty)
            }
            ExprKind::BinaryOperator => {
                let kids2 = self.ast.exprs.children(x, &self.ast.lists);
                let rhs = kids2.get(1).copied().unwrap_or(x);
                let rt = self.ety(rhs);
                let Some((pointee, _)) = conv::scs::member_pointer_parts(self, rt) else {
                    return Some(self.recover_err(&kids, loc));
                };
                let lhs = kids2.first().copied().unwrap_or(x);
                let arrow = self.ast.exprs.binary_op(x) == BinaryOp::PtrMemI;
                self.check_ptrmem_call_quals(lhs, pointee, arrow, loc);
                (callee, None, pointee)
            }
            _ => return None,
        };
        let Some(proto) = self.sugared_proto(fn_ty) else {
            self.unsupported("calls through this function type", loc);
            return Some(self.recover_err(&kids, loc));
        };
        let named = method.map(|d| CallCallee { decl: d, overloadable: false, enable_if: true });
        let Some(conv_args) =
            self.convert_call_args(named, callee, args, braced, &proto, loc, rparen)
        else {
            return Some(self.recover_err(&kids, loc));
        };
        if let Some(m) = method {
            // At the member's name, as clang's `MarkMemberReferenced`.
            let at = if n.kind() == ExprKind::MemberExpr { n.loc() } else { loc };
            if self.unevaluated == 0 {
                self.mark_function_referenced(m, at);
            } else {
                self.exc_spec_needed(m, at);
            }
            // WP-186: `format(printf, F, A)` (`F` and `A` count `this`).
            self.check_format_attr(m, &conv_args);
        }
        let (ty, vk) = self.call_result(proto.ret(), loc);
        let call = self.builder().member_call(
            call_callee,
            &conv_args,
            ty,
            vk,
            loc,
            rparen,
            ExprFlags::NONE,
        );
        let call = self.maybe_bind_temporary(call);
        Some(match method {
            Some(m) => self.check_immediate_invocation(call, m, None),
            None => call,
        })
    }

    /// `obj.f(args)` for an explicit object member function `f` named by
    /// the member access `x` ([over.match.funcs.general]/4, P0847): an
    /// ordinary call of `f` whose first argument is the object expression
    /// (clang's `BuildCallToMemberFunction` with
    /// `isExplicitObjectMemberFunction`); `resolved` if overload
    /// resolution chose `f` (its `enable_if` conditions are checked).
    #[allow(clippy::too_many_arguments)]
    fn explicit_object_call(
        &mut self,
        x: ExprId,
        m: DeclId,
        args: &[ExprId],
        braced: &mut [Option<InitList>],
        resolved: bool,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let n = *self.ast.exprs.get(x);
        let base = self.ast.exprs.operand(x);
        let obj = if n.has(ExprFlags::ARROW) {
            let at = self.begin_loc(base);
            self.builtin_unary(UnaryOp::Deref, base, at)
        } else {
            base
        };
        let mut kids = vec![obj];
        kids.extend_from_slice(args);
        let mut all_braced = Vec::with_capacity(braced.len() + 1);
        all_braced.push(None);
        all_braced.extend(braced.iter_mut().map(Option::take));
        self.explicit_object_call_with(m, n.loc(), &kids, &mut all_braced, resolved, loc, rparen)
    }

    /// A call of explicit object member function `m` rebuilt by a
    /// template instantiation (spec/sema-templates.md §6): `args` are
    /// the object argument and then the call's arguments, as the
    /// definition's call holds them; `name` is where the definition named
    /// `m`. clang's `TreeTransform` rebuilds it as a call of the function
    /// itself, which is how [`Self::explicit_object_call`] built it.
    pub(crate) fn rebuild_explicit_object_call(
        &mut self,
        m: DeclId,
        name: Loc,
        args: &[ExprId],
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let mut braced = vec![None; args.len()];
        self.explicit_object_call_with(m, name, args, &mut braced, true, loc, rparen)
    }

    /// [`Self::explicit_object_call`] given the arguments `kids`, the
    /// object argument first, and where `m` is named.
    #[allow(clippy::too_many_arguments)]
    fn explicit_object_call_with(
        &mut self,
        m: DeclId,
        name: Loc,
        kids: &[ExprId],
        all_braced: &mut [Option<InitList>],
        resolved: bool,
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        let obj = kids[0];
        let fn_ty = self.ast.decls.get(m).ty().unwrap_or(QualType::ERROR);
        // The callee names the member function with no implicit object
        // (not an implicit member access, [expr.prim.id.general]/4).
        let nou = if self.unevaluated > 0 { NonOdrUse::Unevaluated } else { NonOdrUse::None };
        let r = self.builder().decl_ref(m, fn_ty, name, None, nou, ExprFlags::NONE);
        if self.unevaluated == 0 {
            self.mark_function_referenced(m, name);
        } else {
            self.exc_spec_needed(m, name);
        }
        let Some(proto) = self.sugared_proto(fn_ty) else {
            return self.recover_err(kids, loc);
        };
        let f = conv::decay(self, r);
        let named = Some(CallCallee { decl: m, overloadable: false, enable_if: !resolved });
        let conv_args = self.convert_call_args(named, r, kids, all_braced, &proto, loc, rparen);
        let Some(conv_args) = conv_args else {
            return self.recover_err(kids, loc);
        };
        if !self.check_lambda_explicit_object_call(m, self.begin_loc(obj)) {
            return self.recover_err(kids, loc);
        }
        let (ty, vk) = self.call_result(proto.ret(), loc);
        let call = self.builder().call(f, &conv_args, ty, vk, loc, rparen, ExprFlags::NONE);
        let call = self.maybe_bind_temporary(call);
        self.check_immediate_invocation(call, m, Some(r))
    }

    /// [class.cdtor]/4 (spec/sema-class.md §4.11, clang
    /// `BuildCallToMemberFunction`): a virtual call of a pure virtual
    /// function `m` on `this` from a constructor or destructor has
    /// undefined behavior. The member access `x` must name `m` without a
    /// qualifier (a qualified name does not dispatch virtually).
    fn check_pure_virtual_call_in_cdtor(&mut self, x: ExprId, m: DeclId) {
        if !matches!(self.kind_of(self.ctx), DeclKind::CXXConstructor | DeclKind::CXXDestructor)
            || !self.ast.decls.function(self.ast.first(m)).is_some_and(|f| f.spec.is_pure())
            || self.member_refs.get(&x).is_some_and(|r| r.qualified)
        {
            return;
        }
        let mut base = self.ast.exprs.operand(x);
        while matches!(
            self.ast.exprs.get(base).kind(),
            ExprKind::ParenExpr | ExprKind::ImplicitCastExpr
        ) {
            base = self.ast.exprs.operand(base);
        }
        if self.ast.exprs.get(base).kind() != ExprKind::CXXThisExpr {
            return;
        }
        let dtor = u8::from(self.kind_of(self.ctx) == DeclKind::CXXDestructor);
        let Some(class) = self.ast.decls.get(m).ctx() else { return };
        let name = self.decl_arg(m);
        let at =
            self.ast.decls.cold.begin_loc.get(m).copied().unwrap_or(self.ast.decls.get(m).loc());
        let note = self.diag(DiagId::note_entity_declared_at, at).arg(name.clone());
        let d = self
            .diag(DiagId::sema_pure_virtual_call_in_cdtor, self.ast.exprs.get(base).loc())
            .arg(name)
            .arg(dtor)
            .arg(self.decl_arg(class))
            .with_note(note);
        self.emit(d);
    }

    /// The implicit object argument of a call of method `m` through the
    /// member access `x` ([over.match.funcs]/4-5, [dcl.init.ref]):
    /// checks the cv- and ref-qualifiers and rebuilds the access with the
    /// object converted to `cv C` (clang `PerformImplicitObjectArgumentInitialization`).
    fn bind_object_argument(&mut self, x: ExprId, m: DeclId) -> Option<ExprId> {
        let n = *self.ast.exprs.get(x);
        let arrow = n.has(ExprFlags::ARROW);
        let base = self.ast.exprs.operand(x);
        let bt = self.ety(base);
        let obj_ty = if arrow { self.ast.types.pointee(bt).unwrap_or(bt) } else { bt };
        let obj_class = self.ast.types.as_record(obj_ty)?;
        let obj_class = self.ast.definition(obj_class).unwrap_or(obj_class);
        let declaring = self.declaring_class(m)?;
        let fty = self.ast.decls.get(m).ty().unwrap_or(QualType::ERROR);
        let (mq, rq) =
            self.ast.types.proto(fty).map_or((Quals::NONE, RefQual::None), |p| {
                (p.info().this_quals(), p.info().ref_qual())
            });
        let oq = self.ast.types.effective_quals(obj_ty);
        // [class.dtor]/5: a destructor can be invoked for a const,
        // volatile or const volatile object; its implicit object
        // parameter takes the object's cv-qualifiers (clang
        // `TryObjectArgumentInitialization`).
        let mq = if self.kind_of(m) == DeclKind::CXXDestructor { mq | (oq & CV) } else { mq };
        let at = self.begin_loc(base);
        let name = self.decl_name(m);
        let declared_here = |s: &Self| {
            s.diag(DiagId::note_entity_declared_here, s.ast.decls.get(m).loc())
                .arg(format!("'{name}'"))
        };
        let rvalue = !arrow && self.ast.exprs.get(base).value_category() != ValueCategory::LValue;
        let bad_ref = match rq {
            RefQual::LValue => rvalue && (mq & CV) != Quals::CONST,
            RefQual::RValue => !rvalue,
            RefQual::None => false,
        };
        if bad_ref {
            let msg = self
                .diag(DiagId::sema_member_fn_ref_qual_mismatch, at)
                .arg(self.decl_name(m))
                .arg(u8::from(rvalue))
                .arg(u8::from(rq == RefQual::RValue))
                .with_note(declared_here(self));
            self.emit(msg);
            return None;
        }
        let dropped = (oq & CV) - mq;
        if !dropped.is_empty() {
            let msg = self
                .diag(DiagId::sema_member_fn_drops_quals, at)
                .arg(self.decl_name(m))
                .arg(self.ty_arg(obj_ty))
                .arg(dropped.to_string())
                .with_note(declared_here(self));
            self.emit(msg);
            return None;
        }
        let to_q = (oq & CV) | (mq & CV);
        let b = self.object_to_class(
            base,
            arrow,
            obj_class,
            oq,
            declaring,
            to_q,
            CastKind::UncheckedDerivedToBase,
        );
        if self.is_err_expr(b) {
            return None;
        }
        if b == base {
            return Some(x);
        }
        let extra = self.ast.exprs.decl_ref_extra(x).copied();
        Some(self.build_member_expr(b, m, QualType::BOUND_MEMBER, arrow, n.loc(), extra))
    }

    /// `l .* r` / `l ->* r` ([expr.mptr.oper]).
    pub(crate) fn act_on_ptr_mem(
        &mut self,
        op: BinaryOp,
        l: ExprId,
        r: ExprId,
        loc: Loc,
    ) -> ExprId {
        let arrow = op == BinaryOp::PtrMemI;
        let spelling = op.spelling();
        let r2 = conv::decay(self, r);
        let Some((pointee, class)) = conv::scs::member_pointer_parts(self, self.ety(r2)) else {
            let msg = self
                .diag(DiagId::sema_ptrmem_rhs_not_member_pointer, loc)
                .arg(spelling)
                .arg(self.ty_arg(self.ety(r)));
            self.emit(msg);
            return self.recover_err(&[l, r], loc);
        };
        let class = conv::scs::class_decl(self, class);
        let class = class.map(|c| self.ast.definition(c).unwrap_or(c));
        let l2 = if arrow { conv::decay(self, l) } else { l };
        let lt = self.ety(l2);
        let obj_ty = if arrow {
            self.ast.types.is_pointer(lt).then(|| self.pointee_sugared(lt)).flatten()
        } else {
            Some(lt)
        };
        let obj = obj_ty.and_then(|t| {
            let c = self.ast.types.as_record(t)?;
            Some((t, self.ast.definition(c).unwrap_or(c)))
        });
        let (Some((obj_ty, obj_class)), Some(class)) = (obj, class) else {
            return self.ptrmem_lhs_error(spelling, arrow, l, r, loc);
        };
        if !self.derived_inclusive(obj_class, class) {
            return self.ptrmem_lhs_error(spelling, arrow, l, r, loc);
        }
        let oq = self.ast.types.effective_quals(obj_ty);
        let l3 = self.object_to_class(l2, arrow, obj_class, oq, class, oq, CastKind::DerivedToBase);
        if self.is_err_expr(l3) {
            return self.recover_err(&[l3, r2], loc);
        }
        if let Some(p) = self.ast.types.proto(pointee).copied() {
            self.check_ptrmem_ref_qual(p.info(), arrow, l3, r, loc);
            return self.builder().ptr_mem(
                op,
                l3,
                r2,
                QualType::BOUND_MEMBER,
                ValueCategory::PRValue,
                loc,
            );
        }
        let q = oq & CV;
        let ty = if q.is_empty() { pointee } else { self.ast.types.qualify(pointee, q) };
        let vk = if arrow || self.ast.exprs.get(l3).value_category() == ValueCategory::LValue {
            ValueCategory::LValue
        } else {
            ValueCategory::XValue
        };
        self.builder().ptr_mem(op, l3, r2, ty, vk, loc)
    }

    /// [expr.mptr.oper]/6 via [over.match.funcs]: the object of a call
    /// through a pointer to member function is no more cv-qualified than
    /// the function (clang's `err_pointer_to_member_call_drops_quals`, at
    /// the call's `(`; the call is still built).
    fn check_ptrmem_call_quals(&mut self, obj: ExprId, fn_ty: QualType, arrow: bool, lparen: Loc) {
        let Some(p) = self.ast.types.proto(fn_ty).copied() else { return };
        let ot = self.ety(obj);
        let ot = if arrow { self.pointee_sugared(ot).unwrap_or(ot) } else { ot };
        let oq = self.ast.types.effective_quals(ot) & CV;
        let diff = oq - (p.info().this_quals() & CV);
        if diff.is_empty() {
            return;
        }
        let s = crate::class::members::method_quals_spelling(diff, RefQual::None);
        let many = u8::from(s.contains(' '));
        let msg = self
            .diag(DiagId::sema_ptrmem_call_drops_quals, lparen)
            .arg(self.ty_arg(fn_ty))
            .arg(s)
            .arg(many);
        self.emit(msg);
    }

    /// [expr.mptr.oper]/6: a `&&`-qualified member function is called
    /// only on an rvalue object, a `&`-qualified one only on an lvalue
    /// (C++20 also allows `const &` on an rvalue). clang diagnoses and
    /// keeps the expression (a SFINAE error in a substitution).
    fn check_ptrmem_ref_qual(
        &mut self,
        info: kiln_ast::types::FuncInfo,
        arrow: bool,
        obj: ExprId,
        r: ExprId,
        loc: Loc,
    ) {
        use kiln_ast::types::RefQual;
        let lvalue = self.ast.exprs.get(obj).value_category() == ValueCategory::LValue;
        let which = match info.ref_qual() {
            RefQual::None => return,
            RefQual::LValue => {
                let q = info.this_quals() & CV;
                if arrow || lvalue || q == Quals::CONST {
                    return;
                }
                1u8
            }
            RefQual::RValue => {
                if !arrow && !lvalue {
                    return;
                }
                0u8
            }
        };
        let msg = self
            .diag(DiagId::sema_ptrmem_value_classify, loc)
            .arg(self.ty_arg(self.ety(r)))
            .arg(which);
        self.emit(msg);
    }

    fn ptrmem_lhs_error(
        &mut self,
        spelling: &str,
        arrow: bool,
        l: ExprId,
        r: ExprId,
        loc: Loc,
    ) -> ExprId {
        let msg = self
            .diag(DiagId::sema_ptrmem_lhs_incompatible, loc)
            .arg(spelling)
            .arg(u8::from(arrow))
            .arg(self.ty_arg(self.ety(l)));
        self.emit(msg);
        self.recover_err(&[l, r], loc)
    }

    /// `&C::m` ([expr.unary.op]/3): a pointer to member of `m`'s class, if
    /// `e` is a qualified name of a non-static member.
    pub(crate) fn member_pointer_of(&mut self, e: ExprId, loc: Loc) -> Option<ExprId> {
        let r = *self.member_refs.get(&e)?;
        if !r.qualified {
            return None;
        }
        let d = r.decl;
        let class = self.declaring_class(d)?;
        self.member_refs.remove(&e);
        if self.kind_of(d) == DeclKind::CXXMethod {
            let at = self.eloc(e);
            self.resolve_exc_for_type(d, at);
        }
        let declared = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        if self.ast.types.is_reference(declared) {
            self.unsupported("pointer to member of reference type", loc);
            return Some(self.recover_err(&[e], loc));
        }
        let operand = if self.ast.exprs.get(e).kind() == ExprKind::DeclRefExpr {
            e
        } else {
            let nou = if self.unevaluated > 0 { NonOdrUse::Unevaluated } else { NonOdrUse::None };
            let at = self.eloc(e);
            // The qualifier as written stays on the operand (clang's
            // `DeclRefExpr`), so that instantiation forms `&C::m` again
            // rather than the implicit member access `this->C::m`.
            let extra = self.ast.exprs.decl_ref_extra(e).and_then(|x| x.qualifier).map(|q| {
                DeclRefExtra { qualifier: Some(q), template_args: None, found_decl: None }
            });
            let x = self.builder().decl_ref(d, declared, at, extra, nou, ExprFlags::NONE);
            if let Some(b) = r.begin {
                let _ = self.ast.templates.expr_begin.insert(x, b);
            }
            x
        };
        if self.kind_of(d) == DeclKind::CXXMethod {
            let at = self.eloc(e);
            if self.unevaluated == 0 {
                self.mark_function_referenced(d, at);
            } else {
                self.exc_spec_needed(d, at);
            }
            // [expr.const]/18: `&C::f` of an immediate member function.
            self.note_immediate_ref(operand, d);
        }
        // [expr.unary.op]/3.1: `&C::f` of an explicit object member
        // function is an ordinary pointer to function.
        let mp = if self.has_explicit_object(d) {
            self.ast.types.pointer_to(declared)
        } else {
            // [temp.dep.type]/1: in a class template's pattern the class
            // is the current instantiation, its injected-class-name type
            // `X<P...>` (clang's `getTypeDeclType` of the parent), which
            // instantiation substitutes.
            let class_ty = self.this_class_type(class).ty();
            self.ast.types.member_pointer(declared, class_ty)
        };
        Some(self.builder().unary(UnaryOp::AddrOf, operand, mp, loc, ExprFlags::CANNOT_OVERFLOW))
    }

    /// Reports the names of non-static members used without an object
    /// ([expr.prim.id.general]/4) at the end of a full-expression; true
    /// if one was reported.
    ///
    /// Every pending reference is flushed, including those of an
    /// enclosing full-expression still being parsed when a nested one
    /// ends (a template argument's constant expression, a lambda body's
    /// statement). That is only an ordering difference: a pending
    /// reference with `error` set is ill-formed whatever follows (the
    /// uses it could still become, `&C::m` and an unevaluated operand,
    /// either cannot enclose a nested full-expression or clear `error`
    /// when the reference is formed), and the nested and enclosing
    /// expressions share one SFINAE context. WP-445 found no program
    /// whose answer changes, so it is left as is (spec/sema.md §3,
    /// members named without an object). An implicit instantiation is
    /// not such a nested full-expression: [`Sema::in_inst_context`] sets
    /// the pending references aside, so completing an associated class
    /// for a lookup neither reports nor consumes them.
    pub(crate) fn flush_member_refs(&mut self) -> bool {
        if self.member_refs.is_empty() {
            return false;
        }
        let mut refs: Vec<(ExprId, MemberRef)> = self.member_refs.drain().collect();
        refs.sort_by_key(|(e, _)| e.index());
        let in_static = self.func.as_ref().is_some_and(|f| self.is_static_method(f.decl));
        let mut reported = false;
        for (e, r) in refs {
            if !r.error {
                continue;
            }
            let loc = self.eloc(e);
            let msg = if self.kind_of(r.decl) == DeclKind::CXXMethod {
                self.diag(DiagId::sema_member_call_without_object, loc)
            } else if in_static {
                self.diag(DiagId::sema_invalid_member_in_static, loc).arg(self.decl_name(r.decl))
            } else {
                self.diag(DiagId::sema_invalid_nonstatic_member_use, loc)
                    .arg(self.decl_name(r.decl))
            };
            self.emit(msg);
            reported = true;
        }
        reported
    }

    /// A bound member function used other than as a callee
    /// ([expr.ref]/7.3.2); reports clang's "must be called".
    pub(crate) fn diagnose_bound_member(&mut self, e: ExprId) {
        let x = self.ignore_parens(e);
        if self.ast.exprs.get(x).kind() == ExprKind::CXXPseudoDestructorExpr {
            let msg = self.diag(DiagId::sema_pseudo_dtor_not_called, self.begin_loc(x));
            self.emit(msg);
            return;
        }
        let no_args = match self.ast.exprs.get(x).kind() {
            ExprKind::MemberExpr => self
                .ast
                .decls
                .function(self.ast.exprs.decl(x))
                .is_some_and(|f| self.ast.lists.get(f.params).is_empty()),
            // `obj.*pmf` / `ptr->*pmf`: the hint follows the pointee
            // function type (clang's `tryExprAsCall`).
            ExprKind::BinaryOperator => {
                let [_, r] = self.ast.exprs.operands(x);
                let rt = self.ast.types.canonical(self.ast.exprs.get(r).ty()).ty();
                match self.ast.types.view(rt) {
                    TypeView::MemberPointer { pointee, .. } => {
                        match self.ast.types.view(self.ast.types.canonical(pointee).ty()) {
                            TypeView::Function(p) => p.num_params() == 0 && !p.variadic(),
                            _ => false,
                        }
                    }
                    _ => false,
                }
            }
            _ => false,
        };
        let msg =
            self.diag(DiagId::sema_bound_member_not_called, self.eloc(x)).arg(u8::from(no_args));
        self.emit(msg);
    }

    /// Assignment to a member of `*this` in a const member function
    /// ([class.this]/1): clang's diagnostic; true if it applies.
    pub(crate) fn diagnose_const_this_member(&mut self, e: ExprId, loc: Loc) -> bool {
        let x = self.ignore_parens(e);
        if self.ast.exprs.get(x).kind() != ExprKind::MemberExpr {
            return false;
        }
        let field = self.ast.exprs.decl(x);
        let declared = self.ast.decls.get(field).ty().unwrap_or(QualType::ERROR);
        if self.ast.types.effective_quals(declared).is_const() {
            return false;
        }
        let mut b = self.ast.exprs.operand(x);
        loop {
            let k = self.ast.exprs.get(b).kind();
            if k.is_cast() || k == ExprKind::ParenExpr {
                b = self.ast.exprs.operand(b);
            } else {
                break;
            }
        }
        if self.ast.exprs.get(b).kind() != ExprKind::CXXThisExpr {
            return false;
        }
        let Some(mut f) = self.func.as_ref().map(|f| f.decl) else { return false };
        // In a lambda, `this` is the enclosing member function's
        // (spec/sema-lambda.md §3): the diagnostic names that function,
        // and only if it is const (a `*this` copy is const otherwise).
        let mut in_lambda = false;
        while let Some(l) = self.lambda_of_call_op(f) {
            let closure = self.ast.exprs.lambdas.get(l).closure;
            let Some(p) = self.ast.decls.get(closure).ctx() else { return false };
            f = p;
            in_lambda = true;
        }
        if !matches!(self.kind_of(f), DeclKind::CXXMethod | DeclKind::CXXConversion) {
            return false;
        }
        if in_lambda {
            let fty = self.ast.decls.get(f).ty().unwrap_or(QualType::ERROR);
            let konst = self
                .ast
                .types
                .proto(self.ast.types.canonical(fty))
                .is_some_and(|p| p.info().this_quals().is_const());
            if !konst {
                return false;
            }
        }
        let msg = self.diag(DiagId::sema_const_member_assign, loc).arg(self.decl_name(f));
        let note = self
            .diag(DiagId::note_member_fn_declared_const, self.ast.decls.get(f).loc())
            .arg(self.qualified_decl_name(f));
        self.emit(msg.with_note(note));
        true
    }
}
