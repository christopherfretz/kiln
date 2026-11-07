//! Braced-init-lists in expressions and explicit type conversions
//! (spec/sema.md §6.1): `f({...})` ([expr.call]/7, [dcl.init.list]),
//! `return {...};` ([stmt.return]/2), `x = {...}` ([expr.assign]/8) and
//! `T()`, `T{...}`, `T(a, b)` ([expr.type.conv]).

use kiln_ast::decl::TagKind;
use kiln_ast::expr::{BinaryOp, CastKind, ExprFlags, ExprKind, ValueCategory};
use kiln_ast::{DeclId, ExprId, QualType, StmtId};
use kiln_diag::DiagId;
use kiln_source::{Loc, Span};

use crate::Sema;
use crate::conv::{self, ConvCtx};
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
        if self.list_has_expr_error(&l) {
            // clang's `ParseBraceInitializer` fails the list (WP-408).
            let e = self.recover_err(&flat, l.lbrace);
            return self.mark_expr_error(e);
        }
        let e = self.builder().init_list(QualType::VOID, &flat, None, None, l.lbrace, l.rbrace);
        self.braced_args.push((e, l));
        e
    }

    /// Arguments `args` kept as written for instantiation: a braced
    /// placeholder becomes its whole dependent `InitListExpr`, nested
    /// lists included (`T m({{1, 2}, {3, 4}})`; the placeholder holds
    /// only the top-level expressions).
    pub(crate) fn dependent_args(&mut self, args: &[ExprId]) -> Vec<ExprId> {
        args.iter()
            .map(|&a| self.take_braced_arg(a).map_or(a, |l| self.dependent_init_list(&l)))
            .collect()
    }

    /// Whether `e` is a braced-init-list placeholder of
    /// [`Sema::act_on_braced_arg`] (not yet taken).
    pub fn is_braced_arg(&self, e: ExprId) -> bool {
        self.braced_args.iter().any(|(x, _)| *x == e)
    }

    /// Whether `e` is a braced-init-list placeholder whose list is
    /// type-dependent ([temp.dep.expr]/3).
    pub(crate) fn braced_arg_type_dependent(&self, e: ExprId) -> bool {
        self.braced_args.iter().any(|(x, l)| *x == e && self.init_list_type_dependent(l))
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
        (i, param): (usize, Option<DeclId>),
    ) -> Option<ExprId> {
        let to = if self.ast.types.is_reference(pt) { pt } else { conv::strip_cv(self, pt) };
        if let Some(d) = named
            && !self.list_viable(to, l)
        {
            // [over.ics.list]/10: a single element for a non-class
            // parameter is the element's own conversion sequence.
            if let [InitItem { value: InitValue::Expr(x), designator }] = l.items.as_slice()
                && designator.is_empty()
                && self.ast.types.as_record(self.ety(*x)).is_some()
                && self.ast.types.as_record(to).is_none()
                && !self.ast.types.is_reference(to)
            {
                self.bad_argument(d, callee, *x, pt, i);
                return None;
            }
            let msg = self
                .diag(DiagId::sema_ovl_no_viable_function_in_call, self.begin_loc(callee))
                .arg(self.decl_name(d))
                .range(self.expr_range(callee));
            let ics = self.list_arg_ics(to, l);
            let detail = self.bad_conversion_detail(&ics, Some(i), None);
            let fl = self.ast.decls.get(d).loc();
            let note = self.diag(DiagId::note_ovl_candidate_not_viable, fl).arg(detail);
            self.emit(msg.with_note(note));
            return None;
        }
        let note = param.map(|p| self.param_decl_note(p));
        Some(self.init_list_entity(to, l, l.lbrace, ConvCtx::Argument, note))
    }

    /// Whether list-initialization of `ty` from `l` is a viable implicit
    /// conversion sequence ([over.ics.list]): the one-candidate set's
    /// argument conversion.
    fn list_viable(&mut self, ty: QualType, l: &InitList) -> bool {
        self.is_error_type(ty) || !self.list_arg_ics(ty, l).is_bad()
    }

    /// The conversion sequence of braced argument `l` to parameter type
    /// `ty` ([over.ics.list]).
    fn list_arg_ics(&mut self, ty: QualType, l: &InitList) -> crate::overload::Ics {
        let idx = u32::try_from(self.ovl.lists.len()).unwrap_or(u32::MAX);
        self.ovl.lists.push(l.clone());
        let a = crate::overload::ArgInfo {
            list: Some(idx),
            ..crate::overload::ArgInfo::of_type(QualType::VOID, ValueCategory::PRValue)
        };
        let ics = self.try_implicit_conversion(&a, ty, crate::overload::IcsOptions::CANDIDATE_ARG);
        self.ovl.lists.pop();
        ics
    }

    /// The most initializer-clauses a braced-init-list for aggregate `ty`
    /// can hold once brace elision is applied ([dcl.init.aggr]/16):
    /// the sum over its elements (bases, then fields; the first member
    /// of a union) of [`Self::element_list_capacity`]. `None` when
    /// unbounded or unknown.
    pub(crate) fn aggregate_list_capacity(&self, ty: QualType) -> Option<u64> {
        if self.ast.types.is_array(ty) {
            let n = self.ast.types.array_size(ty)?;
            let elem = self.ast.types.element(ty)?;
            return self.element_list_capacity(elem)?.checked_mul(n);
        }
        let Some(r) = self.ast.types.as_record(ty) else { return Some(1) };
        let def = self.ast.definition(r).unwrap_or(r);
        let union = self.ast.decls.record(def).is_some_and(|x| x.tag == TagKind::Union);
        let mut total: u64 = 0;
        for b in self.init_bases(def) {
            total = total.checked_add(self.element_list_capacity(b)?)?;
        }
        for f in self.init_fields(def) {
            let n = self.element_list_capacity(self.ast.decls.get(f).ty()?)?;
            if union {
                return Some(n);
            }
            total = total.checked_add(n)?;
        }
        Some(total)
    }

    /// The most initializer-clauses one element of type `ty` can take
    /// from an enclosing list: a scalar or non-aggregate class takes
    /// one; a subaggregate takes its own elements by brace elision, but
    /// always at least one (a braced list or an expression of its type),
    /// so an empty class or `std::array<T, 0>` member still takes `{}`.
    fn element_list_capacity(&self, ty: QualType) -> Option<u64> {
        if !self.is_aggregate(ty) {
            return Some(1);
        }
        Some(self.aggregate_list_capacity(ty)?.max(1))
    }

    /// Reports a braced-init-list argument for the ellipsis of a
    /// variadic function ([expr.call]/12 has no such conversion); `kind`
    /// names the callee as [`Sema::variadic_call_kind`] does.
    pub(crate) fn braced_variadic_argument(&mut self, l: &InitList, kind: u8) {
        let msg = self.diag(DiagId::sema_init_list_variadic, l.lbrace).arg(kind);
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
        if self.ast.types.is_dependent(ret) || self.init_list_type_dependent(l) {
            // Initialized at instantiation (clang's dependent
            // `InitializationSequence`): the list is kept as written.
            let e = self.dependent_init_list(l);
            let e = self.act_on_full_expr(e);
            return self.builder().return_stmt(Some(e), None, loc);
        }
        let e = self.init_list_entity(ret, l, l.lbrace, ConvCtx::Return, None);
        if self.returns_list_backing_array(e) {
            // clang's `checkExprLifetime` for a return: the
            // `std::initializer_list`'s backing array is a local temporary
            // ([dcl.init.list]/6).
            let msg = self.diag(DiagId::sema_return_local_temp_addr, l.lbrace);
            self.emit(msg);
        }
        let e = self.act_on_full_expr(e);
        self.builder().return_stmt(Some(e), None, loc)
    }

    /// Whether the returned `e` is a `std::initializer_list` built over a
    /// braced list (through implicit no-op conversions and cleanups).
    fn returns_list_backing_array(&self, mut e: ExprId) -> bool {
        for _ in 0..64 {
            let n = self.ast.exprs.get(e);
            match n.kind() {
                ExprKind::CXXStdInitializerListExpr => return true,
                ExprKind::ExprWithCleanups
                | ExprKind::ParenExpr
                | ExprKind::CXXBindTemporaryExpr
                | ExprKind::MaterializeTemporaryExpr
                | ExprKind::ConstantExpr => e = self.ast.exprs.operand(e),
                k if k.is_cast() && matches!(self.ast.exprs.cast_kind(e), CastKind::NoOp) => {
                    e = self.ast.exprs.operand(e);
                }
                _ => return false,
            }
        }
        false
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
        // [temp.dep.expr]/3: a type-dependent left operand or list
        // element makes the assignment dependent; the written list is
        // kept and assigned again at instantiation (clang's
        // `BuildOverloadedBinOp` on a type-dependent operand).
        if self.expr_is_type_dependent(lhs) || self.init_list_type_dependent(l) {
            let r = self.dependent_init_list(l);
            return self.builder().dependent_binary(op, lhs, r, op_loc);
        }
        let lt = self.ast.exprs.get(lhs).ty();
        let lt = conv::strip_cv(self, self.non_reference(lt));
        if self.ast.types.as_record(lt).is_some() {
            // [expr.assign]/8: the list is the argument of the
            // assignment operator function ([over.match.oper]).
            let r = self.act_on_braced_arg(l.clone());
            let br = self.take_braced_arg(r);
            return self.binary_operator(op, lhs, r, br, op_loc);
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

    /// `T()`, `T{...}` or `T(a, b)` of a function type `T`: `cannot create
    /// object of function type 'T'` at the type with the whole expression's
    /// range (clang `BuildCXXTypeConstructExpr`, `err_init_for_function_type`,
    /// after the single-operand cast and before completeness; WP-465).
    fn function_type_construct(&mut self, ty: QualType, loc: Loc, end: Loc) -> bool {
        if !self.ast.types.is_function(ty) {
            return false;
        }
        let msg = self
            .diag(DiagId::sema_init_for_function_type, loc)
            .arg(self.ty_arg(ty))
            .range(Span::new(loc, self.token_end(end).unwrap_or(end)));
        self.emit(msg);
        true
    }

    /// Whether `T(...)`/`T{...}` of class `ty` fails quietly because the
    /// instantiation of `ty`, triggered here, fails (clang's
    /// `RequireCompleteType` in `BuildCXXTypeConstructExpr` returns an
    /// error the first time). Only inside a template instantiation: there
    /// clang's `ExprError` makes the whole construction vanish (libc++
    /// `expected::transform_error`); at parse time clang keeps a
    /// `RecoveryExpr` of type `T`, whose members are still looked up
    /// (`S<int>().m()` says "no member named 'm'"), which kiln gets by
    /// initializing the invalid class as usual.
    fn construction_instantiation_fails(&mut self, ty: QualType, loc: Loc) -> bool {
        self.inst.stack.in_instantiation_record() && self.instantiation_fails(ty, loc)
    }
    /// `T()` ([expr.type.conv]/2): a value-initialized prvalue of type
    /// `T` (`void()` is a void prvalue; a class `T` is constructed by
    /// [`Sema::value_init_class_temporary`]).
    pub fn act_on_value_init_cast(&mut self, ty: QualType, loc: Loc, rparen: Loc) -> ExprId {
        let ty = self.auto_cast_type(ty, &[], None, loc);
        let ty = self.ctad_cast_type(ty, &[], loc);
        if self.is_error_type(ty) {
            return self.recover(ty, &[], loc);
        }
        if self.ast.types.is_dependent(ty) {
            return self.builder().unresolved_construct(ty, &[], false, loc, rparen);
        }
        if self.function_type_construct(ty, loc, rparen) {
            return self.recover(QualType::ERROR, &[], loc);
        }
        // clang `BuildCXXTypeConstructExpr`'s `RequireCompleteType`: in a
        // template instantiation, a class template specialization whose
        // instantiation, here, fails (a failed `static_assert` in it,
        // [temp.inst]/2) is not initialized, quietly
        // (`construction_instantiation_fails`).
        if self.construction_instantiation_fails(ty, loc) {
            return self.recover(QualType::ERROR, &[], loc);
        }
        // [class.abstract]/3 (clang `BuildCXXTypeConstructExpr`).
        if self.require_non_abstract_alloc(loc, ty) {
            return self.recover(QualType::ERROR, &[], loc);
        }
        if let Some(e) = self.value_init_class_temporary(ty, loc, rparen) {
            return e;
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
        let r = self.build_list_cast(ty, l, loc);
        self.note_cast_written_type(r, ty);
        r
    }

    /// The result `x` of `T{...}` built by `list_construct`, with its
    /// temporary bound. `T{y}` whose copy is elided to `y`'s conversion
    /// function (`operator T()`, [dcl.init.list]/3.7, [class.copy.elision])
    /// keeps, as clang, a no-op functional cast to the written type `T`
    /// over the conversion, so an instantiation of a pattern holding it
    /// converts to `T` again (not to the enclosing initialization's type).
    fn finish_list_construct(&mut self, x: ExprId, ty: QualType, loc: Loc, rbrace: Loc) -> ExprId {
        let elided = self.ast.exprs.get(x).kind() == ExprKind::ImplicitCastExpr
            && self.ast.types.as_record(ty).is_some();
        let x = self.maybe_bind_temporary(x);
        if !elided {
            return x;
        }
        self.builder().functional_cast(
            CastKind::NoOp,
            x,
            ty,
            ValueCategory::PRValue,
            None,
            loc,
            rbrace,
            ExprFlags::NONE,
        )
    }

    /// [`Sema::act_on_list_cast`] before the type as written is recorded.
    fn build_list_cast(&mut self, ty: QualType, l: &InitList, loc: Loc) -> ExprId {
        if self.list_has_expr_error(l) {
            // clang's `ParseBraceInitializer` fails the list (WP-408).
            let e = self.recover_err(&Self::list_exprs(l), loc);
            return self.mark_expr_error(e);
        }
        let ty = if self.is_ctad_placeholder(ty) {
            self.act_on_ctad_list_cast_type(ty, l, loc)
        } else {
            self.auto_cast_type(ty, &[], Some(l), loc)
        };
        if self.is_error_type(ty) {
            let flat = Self::list_exprs(l);
            return self.recover(ty, &flat, loc);
        }
        if self.ast.types.is_dependent(ty) || self.init_list_type_dependent(l) {
            let il = self.dependent_init_list(l);
            return self.builder().unresolved_construct(ty, &[il], true, loc, l.rbrace);
        }
        // As `T()` (`act_on_value_init_cast`).
        if self.function_type_construct(ty, loc, l.rbrace)
            || self.construction_instantiation_fails(ty, loc)
            || self.require_non_abstract_alloc(loc, ty)
        {
            let flat = Self::list_exprs(l);
            return self.recover(QualType::ERROR, &flat, loc);
        }
        // [expr.type.conv]/2, [expr.prop]/[basic.lval]/7: a class prvalue
        // keeps the cv-qualifiers (and the sugar) of the written type, so
        // `CU{}` is a `const U` prvalue; only a non-class prvalue drops them.
        if let Some(x) =
            self.list_construct(ty, l, crate::class::InitStyle::DirectList, loc, Some(l.rbrace))
        {
            return if self.is_err_expr(x) {
                x
            } else {
                self.finish_list_construct(x, ty, loc, l.rbrace)
            };
        }
        if self.ast.types.as_record(ty).is_some() && self.is_complete(ty) {
            // An aggregate temporary: the list, destroyed at the end of
            // the full-expression, then a no-op functional cast.
            let init = self.init_from_list_at(ty, l, Some(loc));
            if self.is_err_expr(init) {
                return self.recover(ty, &[init], loc);
            }
            let init = self.maybe_bind_temporary(init);
            return self.builder().functional_cast(
                CastKind::NoOp,
                init,
                ty,
                ValueCategory::PRValue,
                None,
                loc,
                l.rbrace,
                ExprFlags::NONE,
            );
        }
        // A prvalue array keeps its cv-qualifiers ([expr.type]/2).
        if self.ast.types.is_array(ty)
            && !matches!(
                self.ast.types.canonical_view(ty),
                kiln_ast::types::TypeView::VariableArray { .. }
            )
        {
            return self.array_list_cast(ty, l, loc);
        }
        if self.ast.types.is_reference(ty) {
            return self.reference_list_cast(ty, l, loc);
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
        // `E{v}` for a fixed-underlying-type enumeration is
        // direct-list-initialization too ([dcl.init.list]/3.8). The
        // initialized entity is a temporary: "a value".
        let entity = self.init_entity.replace(crate::init::entity::VALUE);
        let init = match self.fixed_enum_list_init(to, l) {
            Some(e) => e,
            None => {
                match self.class_element_list_init(to, l, ConvCtx::DirectInitialization, loc, None)
                {
                    Some(e) => e,
                    None => self.init_from_list_at(to, l, Some(loc)),
                }
            }
        };
        self.init_entity = entity;
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
    pub fn act_on_paren_list_cast(
        &mut self,
        ty: QualType,
        args: &[ExprId],
        loc: Loc,
        rparen: Loc,
    ) -> ExprId {
        // Braced-init-list operands (`T(a, {b})`) are placeholders of
        // [`Sema::act_on_braced_arg`] until the initialization is known.
        if self.is_error_type(ty) || args.iter().any(|&a| self.is_err_expr(a)) {
            return self.recover_dropping_braced(ty, args, loc);
        }
        let ty = self.auto_cast_type(ty, args, None, loc);
        let ty = self.ctad_cast_type(ty, args, loc);
        if self.is_error_type(ty) {
            return self.recover_dropping_braced(ty, args, loc);
        }
        let braced_dep = args.iter().any(|&a| self.braced_arg_type_dependent(a));
        if braced_dep
            || self.ast.types.is_dependent(ty)
            || args.iter().any(|&a| self.expr_is_type_dependent(a))
        {
            // Kept as written; list-initializes at instantiation
            // ([temp.dep.expr]/3).
            let args = self.dependent_args(args);
            return self.builder().unresolved_construct(ty, &args, false, loc, rparen);
        }
        // As `T()` (`act_on_value_init_cast`).
        if self.function_type_construct(ty, loc, rparen)
            || self.construction_instantiation_fails(ty, loc)
            || self.require_non_abstract_alloc(loc, ty)
        {
            return self.recover_dropping_braced(QualType::ERROR, args, loc);
        }
        // A class: a temporary direct-initialized by a constructor
        // ([expr.type.conv]/2, clang's `CXXTemporaryObjectExpr`).
        if self.ast.types.as_record(ty).is_some() {
            let mut site = crate::class::ConstructSite::at(loc);
            site.temp_end = Some(rparen);
            // The temporary has the written type, qualifiers and sugar
            // included (clang: `CV(1, 2)` is a `'CV':'const V'`).
            let to = ty;
            if let Some(x) = self.construct_class(to, args, crate::class::InitStyle::Direct, site) {
                if self.is_err_expr(x) {
                    return x;
                }
                let paren_list = self.ast.exprs.get(x).kind() == ExprKind::CXXParenListInitExpr;
                let x = self.maybe_bind_temporary(x);
                if !paren_list {
                    return x;
                }
                // `A(1, 2)` for an aggregate `A` (P0960): clang wraps the
                // list in a no-op functional cast.
                return self.builder().functional_cast(
                    CastKind::NoOp,
                    x,
                    to,
                    ValueCategory::PRValue,
                    None,
                    loc,
                    rparen,
                    ExprFlags::NONE,
                );
            }
        }
        if let [a] = args
            && self.is_braced_arg(*a)
        {
            // clang's FK_ParenthesizedListInitForScalar: `int({1})`.
            let msg =
                self.diag(DiagId::sema_list_init_in_parens, loc).arg(0u8).arg(self.ty_arg(ty));
            self.emit(msg);
            return self.recover_dropping_braced(ty, args, loc);
        }
        if let Some(what) = self.temporary_unsupported(ty) {
            self.unsupported(&format!("direct-initialization of {what}"), loc);
            return self.recover_dropping_braced(ty, args, loc);
        }
        let msg = self.diag(DiagId::sema_excess_initializers, loc).arg("2");
        self.emit(msg);
        self.recover_dropping_braced(ty, args, loc)
    }

    /// [`Sema::recover`] after discarding the braced-init-list
    /// placeholders among `args`.
    fn recover_dropping_braced(&mut self, ty: QualType, args: &[ExprId], loc: Loc) -> ExprId {
        for &a in args {
            let _ = self.take_braced_arg(a);
        }
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

    /// `R{ ... }` for a reference type `R` ([expr.type.conv]/2): the
    /// reference is direct-list-initialized ([dcl.init.list]/3.9-10) and
    /// the cast is an lvalue (an xvalue for an rvalue reference to an
    /// object) of the referenced type ([expr.type.conv]/2, [expr.prop]
    /// [basic.lval]/1; clang `BuildCXXTypeConstructExpr`).
    fn reference_list_cast(&mut self, ty: QualType, l: &InitList, loc: Loc) -> ExprId {
        let entity = self.init_entity.replace(crate::init::entity::VALUE);
        let init = self.reference_list_init(ty, l, Some(loc), crate::class::InitStyle::DirectList);
        self.init_entity = entity;
        if self.is_err_expr(init) {
            return self.recover(ty, &[init], loc);
        }
        let r = self.pointee_sugared(ty).unwrap_or(QualType::ERROR);
        let rvalue_ref =
            matches!(self.ast.types.canonical_view(ty), kiln_ast::types::TypeView::RValueRef(_));
        let vc = if rvalue_ref && !self.ast.types.is_function(r) {
            ValueCategory::XValue
        } else {
            ValueCategory::LValue
        };
        self.builder().functional_cast(
            CastKind::NoOp,
            init,
            r,
            vc,
            None,
            loc,
            l.rbrace,
            ExprFlags::NONE,
        )
    }

    /// `A{ ... }` for an array type `A` ([expr.type.conv]/2): the
    /// aggregate-initialized array temporary (an array of unknown bound
    /// takes the bound of the list, [dcl.init.aggr]/4), bound to its
    /// element destructor, then a no-op functional cast (clang
    /// `BuildCXXTypeConstructExpr` with an `InitListExpr`).
    fn array_list_cast(&mut self, to: QualType, l: &InitList, loc: Loc) -> ExprId {
        let init = self.init_from_list_at(to, l, Some(loc));
        if self.is_err_expr(init) {
            return self.recover(to, &[init], loc);
        }
        let ty = self.ast.exprs.get(init).ty();
        let init = self.maybe_bind_temporary(init);
        self.builder().functional_cast(
            CastKind::NoOp,
            init,
            ty,
            ValueCategory::PRValue,
            None,
            loc,
            l.rbrace,
            ExprFlags::NONE,
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

    /// `auto(x)` / `auto{x}` ([expr.type.conv]/1, P0849): the type of a
    /// functional cast whose type is the placeholder `ty`, deduced as for
    /// `auto v(x);` (so the decayed type of `x`: a prvalue decay-copy);
    /// any other `ty` unchanged. `args` are the parenthesized operands
    /// (`list` the braced-init-list for `auto{...}`). The error type after
    /// clang's `BuildCXXTypeConstructExpr` diagnostics at `loc`: `auto`
    /// before C++23 and `decltype(auto)` are not allowed here, the
    /// initializer must be one expression and not a braced list. A
    /// type-dependent operand keeps `ty` (deduced at instantiation).
    pub(crate) fn auto_cast_type(
        &mut self,
        ty: QualType,
        args: &[ExprId],
        list: Option<&InitList>,
        loc: Loc,
    ) -> QualType {
        use kiln_ast::types::{AutoKeyword, TypeView};
        let TypeView::Auto { keyword, deduced: None, concept: None, .. } =
            self.ast.types.view(ty.ty())
        else {
            return ty;
        };
        let decltype_auto = keyword == AutoKeyword::DecltypeAuto;
        if decltype_auto || self.std < kiln_lex::Std::Cxx23 {
            // clang `err_auto_not_allowed` ("here").
            let msg = self
                .diag(DiagId::sema_auto_not_allowed, loc)
                .arg(u8::from(decltype_auto))
                .arg(crate::templ::ctad_site::site::HERE);
            self.emit(msg);
            return QualType::ERROR;
        }
        let (items, nested): (Vec<ExprId>, Option<Loc>) = match list {
            Some(l) => {
                let mut v = Vec::new();
                let mut nested = None;
                for it in &l.items {
                    match &it.value {
                        InitValue::Expr(e) => v.push(*e),
                        InitValue::List(n) => {
                            nested.get_or_insert(n.lbrace);
                        }
                    }
                }
                (v, nested)
            }
            None => (args.to_vec(), None),
        };
        let n = list.map_or(args.len(), |l| l.items.len());
        let name = self.ty_arg(ty);
        let fail = if n == 0 {
            Some(self.diag(DiagId::sema_auto_expr_init_no_expression, loc).arg(name.clone()))
        } else if n > 1 {
            let second = match list {
                Some(l) => match &l.items[1].value {
                    InitValue::Expr(e) => self.ast.exprs.get(*e).loc(),
                    InitValue::List(x) => x.lbrace,
                },
                None => self.ast.exprs.get(args[1]).loc(),
            };
            Some(
                self.diag(DiagId::sema_auto_expr_init_multiple_expressions, second)
                    .arg(name.clone()),
            )
        } else if let Some(at) = nested {
            Some(self.diag(DiagId::sema_auto_expr_init_paren_braces, at).arg(1u8).arg(name.clone()))
        } else if list.is_none() && self.is_braced_arg(args[0]) {
            let at = self.ast.exprs.get(args[0]).loc();
            Some(self.diag(DiagId::sema_auto_expr_init_paren_braces, at).arg(0u8).arg(name.clone()))
        } else {
            None
        };
        if let Some(msg) = fail {
            self.emit(msg);
            return QualType::ERROR;
        }
        let e = items[0];
        if self.is_err_expr(e) {
            return QualType::ERROR;
        }
        if self.expr_is_type_dependent(e) {
            return ty;
        }
        let deduced = if self.ast.exprs.get(e).ty() == QualType::OVERLOAD {
            None
        } else {
            self.deduce_auto(ty, &[Some(e)], crate::templ::auto::AutoInit::Expr).ok()
        };
        if let Some(t) = deduced {
            return t;
        }
        let msg = self
            .diag(DiagId::sema_auto_expr_deduction_failure, loc)
            .arg(name)
            .arg(self.ty_arg(self.ast.exprs.get(e).ty()));
        self.emit(msg);
        QualType::ERROR
    }
}
