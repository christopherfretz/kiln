//! Braced-init-lists in expressions and explicit type conversions
//! (spec/sema.md §6.1): `f({...})` ([expr.call]/7, [dcl.init.list]),
//! `return {...};` ([stmt.return]/2), `x = {...}` ([expr.assign]/8) and
//! `T()`, `T{...}`, `T(a, b)` ([expr.type.conv]).

use kiln_ast::decl::TagKind;
use kiln_ast::expr::{BinaryOp, CastKind, ExprFlags, ExprKind, ValueCategory};
use kiln_ast::{DeclId, ExprId, QualType, StmtId};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::conv;
use crate::init::{InitItem, InitList, InitValue};

impl Sema<'_, '_> {
    /// The top-level expressions of `l` (nested lists and designators
    /// dropped), for placeholders and recovery.
    pub(crate) fn list_exprs(l: &InitList) -> Vec<ExprId> {
        l.items
            .iter()
            .filter_map(|i| match i.value {
                InitValue::Expr(e) => Some(e),
                InitValue::List(_) => None,
            })
            .collect()
    }

    /// A braced-init-list as a call argument (`f({1, 2})`): a placeholder
    /// `InitListExpr` of type `void` that [`Sema::act_on_call`] replaces
    /// by the list-initialization of the parameter.
    pub fn act_on_braced_arg(&mut self, l: InitList) -> ExprId {
        let flat = Self::list_exprs(&l);
        let e = self.builder().init_list(QualType::VOID, &flat, None, None, l.lbrace, l.rbrace);
        self.braced_args.push((e, l));
        e
    }

    /// The braced-init-list behind placeholder `e`, if `e` is one.
    pub(crate) fn take_braced_arg(&mut self, e: ExprId) -> Option<InitList> {
        let i = self.braced_args.iter().position(|(x, _)| *x == e)?;
        Some(self.braced_args.swap_remove(i).1)
    }

    /// List-initialization of parameter `i` of type `pt` from argument
    /// `l` of a call to `named` (if the callee names a function). `None`
    /// after reporting the candidate as not viable (clang's "cannot
    /// convert initializer list argument").
    pub(crate) fn braced_argument(
        &mut self,
        named: Option<DeclId>,
        callee: ExprId,
        l: &InitList,
        pt: QualType,
    ) -> Option<ExprId> {
        let to = if self.ast.types.is_reference(pt) { pt } else { conv::strip_cv(self, pt) };
        if let Some(d) = named
            && !self.list_viable(to, l)
        {
            let msg = self
                .diag(DiagId::sema_ovl_no_viable_function_in_call, self.begin_loc(callee))
                .arg(self.decl_name(d));
            let detail = format!("cannot convert initializer list argument to {}", self.ty_arg(pt));
            let fl = self.ast.decls.get(d).loc();
            let note = self.diag(DiagId::note_ovl_candidate_not_viable, fl).arg(detail);
            self.emit(msg.with_note(note));
            return None;
        }
        Some(self.init_list_entity(to, l, l.lbrace))
    }

    /// Whether list-initialization of `ty` from `l` is a viable implicit
    /// conversion sequence ([over.ics.list]): a scalar takes at most one
    /// convertible, unbraced element; an aggregate at most as many
    /// elements as it has scalar subobjects (brace elision).
    fn list_viable(&mut self, ty: QualType, l: &InitList) -> bool {
        if self.is_error_type(ty) || self.ast.types.is_reference(ty) {
            return true;
        }
        if self.is_aggregate(ty) {
            return self.scalar_leaves(ty).is_none_or(|m| l.items.len() as u64 <= m);
        }
        match l.items.as_slice() {
            [] => true,
            [InitItem { value: InitValue::Expr(e), designator }] if designator.is_empty() => {
                let e = *e;
                self.is_err_expr(e)
                    || conv::implicit_convert(self, e, ty, conv::ConvCtx::Initialization).is_ok()
            }
            _ => false,
        }
    }

    /// The number of scalar subobjects of aggregate `ty` that brace
    /// elision can reach; `None` when unbounded or unknown.
    fn scalar_leaves(&self, ty: QualType) -> Option<u64> {
        if self.ast.types.is_array(ty) {
            let n = self.ast.types.array_size(ty)?;
            let elem = self.ast.types.element(ty)?;
            return self.scalar_leaves(elem)?.checked_mul(n);
        }
        let Some(r) = self.ast.types.as_record(ty) else { return Some(1) };
        let def = self.ast.definition(r).unwrap_or(r);
        let union = self.ast.decls.record(def).is_some_and(|x| x.tag == TagKind::Union);
        let fields = self.init_fields(def);
        let mut total: u64 = 0;
        for f in fields {
            let ft = self.ast.decls.get(f).ty()?;
            let n = self.scalar_leaves(ft)?;
            if union {
                return Some(n);
            }
            total = total.checked_add(n)?;
        }
        Some(total)
    }

    /// Reports a braced-init-list argument for the ellipsis of a
    /// variadic function ([expr.call]/12 has no such conversion).
    pub(crate) fn braced_variadic_argument(&mut self, l: &InitList) {
        let msg = self.diag(DiagId::sema_init_list_variadic, l.lbrace);
        self.emit(msg);
    }

    /// `return { ... };` ([stmt.return]/2): copy-list-initialization of
    /// the returned object. `loc` is the `return` keyword.
    pub fn act_on_return_list(&mut self, l: &InitList, loc: Loc) -> StmtId {
        let flat = Self::list_exprs(l);
        let Some((fdecl, ret, auto)) =
            self.func.as_ref().map(|f| (f.decl, f.ret, f.auto_ret.is_some()))
        else {
            let e = self.recover_err(&flat, l.lbrace);
            return self.builder().return_stmt(Some(e), None, loc);
        };
        if auto {
            // [dcl.type.auto.deduct]/2.1 (clang marks the function
            // invalid and leaves the placeholder undeduced).
            let msg = self.diag(DiagId::sema_auto_fn_init_list, l.lbrace);
            self.emit(msg);
            self.ast.decls.set_flags(fdecl, kiln_ast::decl::DeclFlags::INVALID);
            if let Some(st) = self.func.as_mut() {
                st.deduced = true;
                st.ret = QualType::ERROR;
            }
            let e = self.recover_err(&flat, l.lbrace);
            return self.builder().return_stmt(Some(e), None, loc);
        }
        if self.ast.types.is_void(ret) {
            let msg = self.diag(DiagId::sema_return_init_list, loc).arg(self.decl_name(fdecl));
            self.emit(msg);
            let e = self.recover_err(&flat, l.lbrace);
            return self.builder().return_stmt(Some(e), None, loc);
        }
        let e = self.init_list_entity(ret, l, l.lbrace);
        let e = self.act_on_full_expr(e);
        self.builder().return_stmt(Some(e), None, loc)
    }

    /// `x op= { ... }` ([expr.assign]/8): the list initializes a
    /// temporary of the left operand's type, which is then assigned.
    pub fn act_on_assign_list(
        &mut self,
        op: BinaryOp,
        lhs: ExprId,
        l: &InitList,
        op_loc: Loc,
    ) -> ExprId {
        let flat = Self::list_exprs(l);
        if self.is_err_expr(lhs) {
            let mut kids = vec![lhs];
            kids.extend(flat);
            return self.recover_err(&kids, op_loc);
        }
        let lt = self.ast.exprs.get(lhs).ty();
        let lt = conv::strip_cv(self, self.non_reference(lt));
        if self.ast.types.as_record(lt).is_some() {
            self.unsupported("assignment of a braced-init-list to a class object", l.lbrace);
            let mut kids = vec![lhs];
            kids.extend(flat);
            return self.recover_err(&kids, op_loc);
        }
        let r = self.init_from_list_at(lt, l, Some(l.lbrace));
        self.act_on_binary(op, lhs, r, op_loc)
    }

    /// A braced-init-list as an operand of an operator other than
    /// assignment (`a + {1}`, `c ? 1 : {2}`): ill-formed. `right`: the
    /// list follows the operator spelled `op`, at `op_loc`.
    pub fn act_on_misplaced_init_list(
        &mut self,
        lhs: ExprId,
        l: &InitList,
        right: bool,
        op: &str,
        op_loc: Loc,
    ) -> ExprId {
        let msg = self
            .diag(DiagId::parse_init_list_bin_op, op_loc)
            .arg(u8::from(right))
            .arg(op.to_owned());
        self.emit(msg);
        let mut kids = vec![lhs];
        kids.extend(Self::list_exprs(l));
        self.recover_err(&kids, op_loc)
    }

    /// `T()` ([expr.type.conv]/2): a value-initialized prvalue of
    /// non-class type `T` (`void()` is a void prvalue).
    pub fn act_on_value_init_cast(&mut self, ty: QualType, loc: Loc, rparen: Loc) -> ExprId {
        if self.is_error_type(ty) {
            return self.recover(ty, &[], loc);
        }
        // [class.abstract]/3 (clang `BuildCXXTypeConstructExpr`).
        if self.require_non_abstract_alloc(loc, ty) {
            return self.recover(QualType::ERROR, &[], loc);
        }
        if let Some(what) = self.temporary_unsupported(ty) {
            self.unsupported(&format!("value-initialization of {what}"), loc);
            return self.recover(ty, &[], loc);
        }
        self.builder().scalar_value_init(ty, loc, rparen)
    }

    /// `T{ ... }` ([expr.type.conv]/2): direct-list-initialization of a
    /// prvalue of non-class type `T`. `loc` is the start of the type.
    pub fn act_on_list_cast(&mut self, ty: QualType, l: &InitList, loc: Loc) -> ExprId {
        if self.is_error_type(ty) {
            let flat = Self::list_exprs(l);
            return self.recover(ty, &flat, loc);
        }
        if self.require_non_abstract_alloc(loc, ty) {
            let flat = Self::list_exprs(l);
            return self.recover(QualType::ERROR, &flat, loc);
        }
        if let Some(what) = self
            .temporary_unsupported(ty)
            .or_else(|| self.ast.types.is_void(ty).then_some("a void temporary"))
        {
            self.unsupported(&format!("list-initialization of {what}"), l.lbrace);
            let flat = Self::list_exprs(l);
            return self.recover(ty, &flat, loc);
        }
        let to = conv::strip_cv(self, ty);
        let init = self.init_from_list_at(to, l, Some(loc));
        if self.is_err_expr(init) {
            return self.recover(ty, &[init], loc);
        }
        // clang's `RParenLoc` of `T{...}` is the `}`.
        self.builder().functional_cast(
            CastKind::NoOp,
            init,
            to,
            ValueCategory::PRValue,
            None,
            loc,
            l.rbrace,
            ExprFlags::NONE,
        )
    }

    /// `T(a, b, ...)` ([expr.type.conv]/2) with several operands: a
    /// non-class `T` is direct-initialized from a parenthesized list,
    /// which is ill-formed for a scalar ([dcl.init.general]/16.9).
    pub fn act_on_paren_list_cast(&mut self, ty: QualType, args: &[ExprId], loc: Loc) -> ExprId {
        if self.is_error_type(ty) || args.iter().any(|&a| self.is_err_expr(a)) {
            return self.recover(ty, args, loc);
        }
        if self.require_non_abstract_alloc(loc, ty) {
            return self.recover(QualType::ERROR, args, loc);
        }
        if let Some(what) = self.temporary_unsupported(ty) {
            self.unsupported(&format!("direct-initialization of {what}"), loc);
            return self.recover(ty, args, loc);
        }
        let msg = self.diag(DiagId::sema_excess_initializers, loc).arg("2");
        self.emit(msg);
        self.recover(ty, args, loc)
    }

    /// GNU `x ?: y`: `x ? x : y` with `x` evaluated once (clang's
    /// `BinaryConditionalOperator`). Unless both operands are glvalues
    /// of the same type, `x` undergoes the usual unary conversions
    /// before it is shared. `loc` is the `?`.
    pub fn act_on_binary_conditional(
        &mut self,
        common: ExprId,
        rhs: ExprId,
        loc: Loc,
        _colon: Loc,
    ) -> ExprId {
        if self.is_err_expr(common) || self.is_err_expr(rhs) {
            return self.recover_err(&[common, rhs], loc);
        }
        let (cn, rn) = (*self.ast.exprs.get(common), *self.ast.exprs.get(rhs));
        if self.ast.types.as_record(cn.ty()).is_some()
            || self.ast.types.as_record(rn.ty()).is_some()
        {
            self.unsupported("the GNU '?:' operator on class types", loc);
            return self.recover_err(&[common, rhs], loc);
        }
        let keep = cn.value_category().is_glvalue()
            && cn.value_category() == rn.value_category()
            && self.ast.types.same_type(cn.ty(), rn.ty());
        let common = if keep { common } else { conv::usual_unary_conversions(self, common) };
        let ov = self.builder().opaque_value(common);
        let c = self.act_on_conditional(ov, ov, rhs, loc);
        if self.ast.exprs.get(c).kind() != ExprKind::ConditionalOperator {
            return c;
        }
        let kids = self.ast.exprs.children(c, &self.ast.lists);
        let n = *self.ast.exprs.get(c);
        self.builder().binary_conditional(
            common,
            kids[0],
            kids[1],
            kids[2],
            n.ty(),
            n.value_category(),
            loc,
        )
    }

    /// Why a temporary of type `ty` cannot be created yet (class types
    /// need constructors, WP-065; arrays and references are rare in
    /// functional casts), or `None`.
    pub(crate) fn temporary_unsupported(&self, ty: QualType) -> Option<&'static str> {
        if self.ast.types.as_record(ty).is_some() {
            Some("a class temporary")
        } else if self.ast.types.is_array(ty) {
            Some("an array temporary")
        } else if self.ast.types.is_reference(ty) {
            Some("a reference type")
        } else if self.ast.types.is_function(ty) {
            Some("a function type")
        } else {
            None
        }
    }
}
