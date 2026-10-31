//! Class members in expressions (spec/sema-class.md §2.5/§2.6; WP-066b):
//! implicit `this->` ([expr.prim.id.general]/3), `e.m` / `e->m`
//! ([expr.ref]), calls of non-static member functions ([expr.call]/2,
//! [over.match.funcs]/4-5 for a single candidate), `&C::m`
//! ([expr.unary.op]/3) and `.*` / `->*` ([expr.mptr.oper]).

use kiln_ast::decl::{DeclFlags, DeclKind, FnStorage};
use kiln_ast::expr::{
    BinaryOp, CastKind, DeclRefExtra, ExprFlags, ExprKind, NonOdrUse, UnaryOp, ValueCategory,
};
use kiln_ast::types::{Quals, RefQual};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::conv::bases::{BaseSearch, ambiguous_paths_string, find_base, path_list, same_class};
use crate::conv::{self};
use crate::expr::CallCallee;
use crate::init::InitList;
use crate::lookup::LookupKind;

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
            DeclKind::Field => true,
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
        let qualified = nns.is_some();
        if let Some(class) = self.implicit_object_class(d)
            && let Some((this_ty, _)) = self.this_class()
        {
            let this = self.builder().this_expr(this_ty, true, loc);
            let obj_ty = self.ast.types.pointee(this_ty).unwrap_or(QualType::ERROR);
            let e = self.member_of_object(this, obj_ty, class, true, d, shadow, nns, loc);
            if qualified && !self.is_err_expr(e) {
                self.member_refs.insert(e, MemberRef { decl: d, error: false, qualified });
            }
            return e;
        }
        let declared = self.ast.decls.get(d).ty().unwrap_or(QualType::ERROR);
        let ty = self.non_reference(declared);
        self.ast.decls.set_flags(d, DeclFlags::REFERENCED);
        if self.ast.decls.get(d).kind().is_function() {
            self.diagnose_deleted_use(d, loc);
        }
        let nou = if self.unevaluated > 0 { NonOdrUse::Unevaluated } else { NonOdrUse::None };
        let extra = shadow.map(|s| DeclRefExtra {
            qualifier: None,
            template_args: None,
            found_decl: Some(s),
        });
        let e = self.builder().decl_ref(d, ty, loc, extra, nou, ExprFlags::NONE);
        let error = self.unevaluated == 0 || self.kind_of(d) == DeclKind::CXXMethod;
        self.member_refs.insert(e, MemberRef { decl: d, error, qualified });
        e
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
        name: Symbol,
        loc: Loc,
    ) -> ExprId {
        let found = self.lookup_in_class(def, name, LookupKind::Ordinary);
        if self.diagnose_member_ambiguity(&found, name, loc) {
            return self.recover_err(&[base], loc);
        }
        let Some(&m) = found.decls.first() else {
            let msg = self
                .diag(DiagId::sema_no_member, loc)
                .arg(self.ast.symbols().as_str(name).to_owned())
                .arg(self.ty_arg(obj_ty));
            self.emit(msg);
            return self.recover_err(&[base], loc);
        };
        if found.decls.len() > 1 || self.kind_of(m) == DeclKind::FunctionTemplate {
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
                naming: found.naming.unwrap_or(def),
                obj_ty,
                def,
                base: b,
                arrow,
                nns: None,
                implicit: false,
                targs: None,
            };
            return self.build_member_overload(set, loc);
        }
        if crate::lookup::is_type_kind(self.kind_of(m)) {
            let text = self.ast.symbols().as_str(name).to_owned();
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
        let naming = found.naming.unwrap_or(def);
        let via = found.found.first().copied().unwrap_or(m);
        self.check_member_access(via, naming, Some(def), loc);
        self.member_of_object(b, obj_ty, def, arrow, m, found.shadow(0), None, loc)
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
        match kind {
            DeclKind::Field | DeclKind::IndirectField => {
                let Some(target) = self.declaring_class(m) else {
                    self.unsupported("this kind of member access", loc);
                    return self.recover_err(&[b], loc);
                };
                let b = self.object_to_class(
                    b,
                    arrow,
                    obj_class,
                    quals,
                    target,
                    quals,
                    CastKind::UncheckedDerivedToBase,
                );
                if self.is_err_expr(b) {
                    return b;
                }
                if kind == DeclKind::Field {
                    return self.field_access(b, m, quals, arrow, loc);
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
                    self.diagnose_deleted_use(m, loc);
                }
                let ty = self.non_reference(self.ast.decls.get(m).ty().unwrap_or(QualType::ERROR));
                let e = self.builder().member(b, m, ty, arrow, loc, extra);
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
                self.diagnose_deleted_use(m, loc);
                self.builder().member(b, m, QualType::BOUND_MEMBER, arrow, loc, extra)
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
        let b = if arrow { b } else { self.materialize_object(b) };
        let loc = self.eloc(b);
        let vk =
            if arrow { ValueCategory::PRValue } else { self.ast.exprs.get(b).value_category() };
        let to_ty = |s: &mut Self, class: DeclId| {
            let t = s.tag_type(class);
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
                let ty = to_ty(self, target);
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
                (callee, None, pointee)
            }
            _ => return None,
        };
        let Some(proto) = self.sugared_proto(fn_ty) else {
            self.unsupported("calls through this function type", loc);
            return Some(self.recover_err(&kids, loc));
        };
        let named = method.map(|d| CallCallee { decl: d, overloadable: false });
        let Some(conv_args) =
            self.convert_call_args(named, callee, args, braced, &proto, loc, rparen)
        else {
            return Some(self.recover_err(&kids, loc));
        };
        if let Some(m) = method {
            // At the member's name, as clang's `MarkMemberReferenced`.
            if self.unevaluated == 0 {
                self.mark_function_referenced(m, n.loc());
            } else {
                self.exc_spec_needed(m, n.loc());
            }
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
        Some(self.maybe_bind_temporary(call))
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
        Some(self.builder().member(b, m, QualType::BOUND_MEMBER, arrow, n.loc(), extra))
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
        if self.ast.types.is_function(pointee) {
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
            self.builder().decl_ref(d, declared, at, None, nou, ExprFlags::NONE)
        };
        if self.kind_of(d) == DeclKind::CXXMethod {
            let at = self.eloc(e);
            if self.unevaluated == 0 {
                self.mark_function_referenced(d, at);
            } else {
                self.exc_spec_needed(d, at);
            }
        }
        let class_ty = self.tag_type(class).ty();
        let mp = self.ast.types.member_pointer(declared, class_ty);
        Some(self.builder().unary(UnaryOp::AddrOf, operand, mp, loc, ExprFlags::CANNOT_OVERFLOW))
    }

    /// Reports the names of non-static members used without an object
    /// ([expr.prim.id.general]/4) at the end of a full-expression.
    pub(crate) fn flush_member_refs(&mut self) {
        if self.member_refs.is_empty() {
            return;
        }
        let mut refs: Vec<(ExprId, MemberRef)> = self.member_refs.drain().collect();
        refs.sort_by_key(|(e, _)| e.index());
        let in_static = self.func.as_ref().is_some_and(|f| self.is_static_method(f.decl));
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
        }
    }

    /// A bound member function used other than as a callee
    /// ([expr.ref]/7.3.2); reports clang's "must be called".
    pub(crate) fn diagnose_bound_member(&mut self, e: ExprId) {
        let x = self.ignore_parens(e);
        let no_args = (self.ast.exprs.get(x).kind() == ExprKind::MemberExpr)
            .then(|| self.ast.exprs.decl(x))
            .and_then(|m| self.ast.decls.function(m))
            .is_some_and(|f| self.ast.lists.get(f.params).is_empty());
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
        let Some(f) = self.func.as_ref().map(|f| f.decl) else { return false };
        if !matches!(self.kind_of(f), DeclKind::CXXMethod | DeclKind::CXXConversion) {
            return false;
        }
        let msg = self.diag(DiagId::sema_const_member_assign, loc).arg(self.decl_name(f));
        let note = self
            .diag(DiagId::note_member_fn_declared_const, self.ast.decls.get(f).loc())
            .arg(self.qualified_decl_name(f));
        self.emit(msg.with_note(note));
        true
    }
}
