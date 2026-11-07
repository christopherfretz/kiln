//! GNU extensions kiln accepts in C++ (spec/sema.md §5.8): statement
//! expressions `({ ... })`, `__builtin_offsetof` and compound literals
//! `( T ) { ... }`. Each matches clang's behaviour for C++.

use kiln_ast::decl::{DeclKind, DefinitionBits};
pub use kiln_ast::expr::OffsetofComponent;
use kiln_ast::expr::{ExprKind, ValueCategory};
use kiln_ast::ids::RawId;
use kiln_ast::names::DeclName;
use kiln_ast::stmt::StmtKind;
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId, ListId, QualType, StmtId};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::class::InitStyle as CtorStyle;
use crate::conv::bases::{BaseSearch, find_base};
use crate::conv::{self, ConvCtx};
use crate::init::{Designator, InitList, InitValue};
use crate::lookup::LookupKind;
use crate::target;

/// The state saved across a statement expression
/// ([`Sema::act_on_start_stmt_expr`]): the enclosing full-expression's
/// pending potential odr-uses, which the statements inside must not
/// resolve.
#[derive(Debug)]
pub struct StmtExprScope {
    outer_odr: Vec<ExprId>,
    outer_imm: crate::consteval::SavedImm,
}

impl Sema<'_, '_> {
    /// Whether a statement expression may appear here: inside a function
    /// body (clang's `getFnParent()`; `err_stmtexpr_file_scope`
    /// otherwise, reported at the `(`, which is then the caller's to
    /// recover from). Returns `false` after diagnosing.
    pub fn check_stmt_expr_allowed(&self, lparen: Loc) -> bool {
        if self.func.is_some() {
            return true;
        }
        self.error(DiagId::sema_stmtexpr_file_scope, lparen);
        false
    }

    /// The `({` of a statement expression: its statements are separate
    /// full-expressions, so the enclosing one's pending odr-uses are set
    /// aside until [`Sema::act_on_stmt_expr`].
    pub fn act_on_start_stmt_expr(&mut self) -> StmtExprScope {
        StmtExprScope {
            outer_odr: std::mem::take(&mut self.pending_odr),
            outer_imm: self.take_immediate_state(false),
        }
    }

    /// The expression statement that ends a statement expression (clang's
    /// `ActOnStmtExprResult`): its value is the statement expression's.
    /// The usual array/function decay applies; a non-`void` result is
    /// then copy-initialized into a prvalue of its cv-unqualified type
    /// (an lvalue-to-rvalue conversion for a scalar). It is not
    /// discarded, so no unused-value warning is given.
    pub fn act_on_stmt_expr_result(&mut self, e: ExprId) -> StmtId {
        let e = self.stmt_expr_result_value(e);
        let e = self.act_on_full_expr(e);
        self.builder().expr_stmt(e)
    }

    fn stmt_expr_result_value(&mut self, e: ExprId) -> ExprId {
        if self.is_err_expr(e) {
            return e;
        }
        let e = conv::decay(self, e);
        let n = *self.ast.exprs.get(e);
        let ty = n.ty();
        if self.ast.types.is_void(ty) || self.ast.types.is_dependent(ty) {
            return e;
        }
        if self.ast.types.as_record(ty).is_some() {
            if n.value_category().is_glvalue() {
                // Copying a class glvalue needs its copy constructor.
                self.unsupported("a statement expression with a class lvalue result", n.loc());
                return self.recover(ty, &[e], n.loc());
            }
            return e;
        }
        let to = conv::strip_cv(self, ty);
        self.convert_for_init(e, to, ConvCtx::Initialization)
    }

    /// `( compound-statement )` (a GNU statement expression; clang's
    /// `ActOnStmtExpr`). Its type is that of the last statement that is
    /// not a null statement (under `-fclang-compat=22`: the last statement),
    /// looking through labels, when that is an expression statement, else
    /// `void`; it is always a prvalue. `lparen`
    /// and `rparen` are the parentheses.
    pub fn act_on_stmt_expr(
        &mut self,
        scope: StmtExprScope,
        body: StmtId,
        lparen: Loc,
        rparen: Loc,
    ) -> ExprId {
        let inner = std::mem::replace(&mut self.pending_odr, scope.outer_odr);
        self.pending_odr.extend(inner);
        self.restore_immediate_state(scope.outer_imm);
        let result = self.stmt_expr_result(body);
        let ty = result.map_or(QualType::VOID, |e| self.ast.exprs.get(e).ty());
        let dependent_ctx = self.in_template();
        self.builder().stmt_expr(body, ty, result, dependent_ctx, lparen, rparen)
    }

    /// The expression whose value a statement expression with body `body`
    /// yields (clang's `getStmtExprResult` + `ValueStmt::getExprStmt`).
    pub(crate) fn stmt_expr_result(&self, body: StmtId) -> Option<ExprId> {
        let stmts = &self.ast.stmts;
        let children = self.ast.lists.get(stmts.compound_body(body));
        if self.clang_compat >= 22 {
            return self.value_stmt_expr(StmtId::new(*children.last()?));
        }
        let last = children
            .iter()
            .rev()
            .map(|&s| StmtId::new(s))
            .find(|&s| stmts.get(s).kind() != StmtKind::NullStmt)
            .or_else(|| children.last().map(|&s| StmtId::new(s)))?;
        self.value_stmt_expr(last)
    }

    /// The expression of an expression statement, looking through labels
    /// and attributes.
    fn value_stmt_expr(&self, s: StmtId) -> Option<ExprId> {
        let stmts = &self.ast.stmts;
        let mut cur = s;
        for _ in 0..1024 {
            let n = stmts.get(cur);
            match n.kind() {
                StmtKind::ExprStmt => return Some(stmts.expr_stmt_expr(cur)),
                StmtKind::LabelStmt | StmtKind::AttributedStmt => cur = StmtId::new(n.b()),
                _ => return None,
            }
        }
        None
    }

    /// clang's `isUnusedResultAWarning` for a `StmtExpr` at `loc` (its
    /// `(`): the warning for its last statement when that is an expression
    /// (directly or under one label), else a warning unless it is `void`.
    pub(crate) fn unused_stmt_expr(
        &self,
        e: ExprId,
        loc: Loc,
    ) -> Option<(DiagId, Loc, Vec<String>)> {
        let stmts = &self.ast.stmts;
        let body = self.ast.exprs.stmt_expr_body(e);
        let children = self.ast.lists.get(stmts.compound_body(body));
        if let Some(&last) = children.last() {
            let mut s = StmtId::new(last);
            if stmts.get(s).kind() == StmtKind::LabelStmt {
                s = StmtId::new(stmts.get(s).b());
            }
            if stmts.get(s).kind() == StmtKind::ExprStmt {
                return self.unused_warning(stmts.expr_stmt_expr(s));
            }
        }
        (!self.ast.types.is_void(self.ast.exprs.get(e).ty())).then_some((
            DiagId::sema_unused_expr,
            loc,
            Vec::new(),
        ))
    }

    /// `__builtin_choose_expr(cond, lhs, rhs)` at `loc` (clang
    /// `ActOnChooseExpr`): `cond` must be an integral constant expression
    /// (`'__builtin_choose_expr' requires a constant expression`, with the
    /// evaluation's notes); a nonzero value picks `lhs`, zero `rhs`, and
    /// the result is the chosen operand unconverted: its type, value
    /// category and object kind. A type- or value-dependent `cond` leaves
    /// the result type-dependent until substitution.
    pub fn act_on_choose_expr(
        &mut self,
        loc: Loc,
        cond: ExprId,
        lhs: ExprId,
        rhs: ExprId,
        _rparen: Loc,
    ) -> ExprId {
        if [cond, lhs, rhs].iter().any(|&x| self.is_err_expr(x)) {
            return self.recover_err(&[cond, lhs, rhs], loc);
        }
        // A substituted condition was already wrapped once.
        let mut cond = cond;
        while self.ast.exprs.get(cond).kind() == ExprKind::ConstantExpr {
            cond = self.ast.exprs.operand(cond);
        }
        let deps = self.ast.exprs.get(cond).deps();
        if deps.intersects(kiln_ast::expr::ExprDeps::TYPE.with(kiln_ast::expr::ExprDeps::VALUE)) {
            return self.builder().choose_expr(cond, lhs, rhs, None, loc);
        }
        let at = self.begin_loc(cond);
        let c = conv::lvalue_to_rvalue(self, cond);
        let ty = self.ast.exprs.get(c).ty();
        let r = if conv::is_integral_or_unscoped_enum(self, ty) {
            let c = self.act_on_constant_full_expr(c);
            self.eval_int(c, kiln_oven::EvalContext::new(kiln_oven::EvalKind::Other, at))
                .map(|v| (c, v))
        } else {
            Err(kiln_oven::EvalFailure::silent(kiln_oven::FailKind::NotConstant))
        };
        match r {
            Ok((c, v)) => {
                let ty = self.ast.exprs.get(c).ty();
                let cv = self.int_const(ty, v);
                let wrapped = self.builder().constant_expr(c, Some(cv));
                self.builder().choose_expr(wrapped, lhs, rhs, Some(v != 0), loc)
            }
            Err(mut f) => {
                let at = Self::fold_lone_invalid(&mut f, at);
                let d = self.diag(DiagId::sema_choose_expr_requires_constant, at);
                let d = Self::with_failure_notes(d, f);
                self.emit(d);
                self.recover_err(&[cond, lhs, rhs], loc)
            }
        }
    }

    /// `__builtin_offsetof(ty, comps...)` at `loc` (clang
    /// `BuildBuiltinOffsetOf`): a `std::size_t` prvalue holding the byte
    /// offset of the designated subobject. Indices must be constant.
    pub fn act_on_builtin_offsetof(
        &mut self,
        loc: Loc,
        ty: QualType,
        comps: &[OffsetofComponent],
        rparen: Loc,
    ) -> ExprId {
        // A dependent operand type or a value-dependent index defers the
        // whole designator to instantiation (clang `BuildBuiltinOffsetOf`
        // builds a dependent `OffsetOfExpr`; [temp.dep.constexpr]).
        let dependent = self.ast.types.is_dependent(ty)
            || comps.iter().any(|c| match *c {
                OffsetofComponent::Index { index, .. } => self
                    .ast
                    .exprs
                    .get(index)
                    .deps()
                    .intersects(kiln_ast::expr::ExprDeps::TYPE_VALUE_INST),
                OffsetofComponent::Field { .. } => false,
            });
        if dependent {
            let st = self.size_type();
            return self.builder().dependent_offsetof(st, ty, comps, loc, rparen);
        }
        let mut indices = Vec::new();
        let mut invalid = false;
        let offset = self.offsetof_value(loc, ty, comps, &mut indices, &mut invalid);
        let fits = offset.and_then(|v| u32::try_from(v).ok());
        if offset.is_some() && fits.is_none() {
            self.unsupported("a '__builtin_offsetof' value outside 0 to 2^32 - 1", loc);
        }
        match fits {
            Some(v) => {
                let st = self.size_type();
                self.builder().offsetof(st, &indices, v, invalid, loc, rparen)
            }
            None => self.recover_err(&indices, loc),
        }
    }

    /// The offset in bytes `__builtin_offsetof(ty, comps...)` designates,
    /// or `None` after an error. The converted indices go to `indices`;
    /// `invalid` is set when the designator passes through an invalid
    /// class (the value is then not a constant, clang `ExprConstant`).
    fn offsetof_value(
        &mut self,
        loc: Loc,
        ty: QualType,
        comps: &[OffsetofComponent],
        indices: &mut Vec<ExprId>,
        invalid: &mut bool,
    ) -> Option<i128> {
        if self.is_error_type(ty) {
            return None;
        }
        if self.ast.types.as_record(ty).is_none() {
            let d = self.diag(DiagId::sema_offsetof_record_type, loc).arg(self.ty_arg(ty));
            self.emit(d);
            return None;
        }
        if !self.require_complete(ty, loc) {
            let d = self.diag(DiagId::sema_offsetof_incomplete_type, loc).arg(self.ty_arg(ty));
            self.emit(self.incomplete_note(d, ty));
            return None;
        }
        *invalid |= self.is_invalid_class(ty);
        let mut cur = ty;
        let mut bits: i128 = 0;
        let mut warned = false;
        for &c in comps {
            match c {
                OffsetofComponent::Index { index, end, .. } => {
                    let Some(elem) =
                        self.ast.types.is_array(cur).then(|| self.ast.types.element(cur)).flatten()
                    else {
                        let d =
                            self.diag(DiagId::sema_offsetof_array_type, end).arg(self.ty_arg(cur));
                        self.emit(d);
                        return None;
                    };
                    let idx = conv::lvalue_to_rvalue(self, index);
                    indices.push(idx);
                    let it = self.ast.exprs.get(idx).ty();
                    if self.is_error_type(it) {
                        return None;
                    }
                    if !self.ast.types.is_integral(it) && !conv::is_unscoped_enum(self, it) {
                        self.error(DiagId::sema_subscript_not_integer, self.begin_loc(idx));
                        return None;
                    }
                    let Some(i) = self.fold_int(idx) else {
                        self.unsupported(
                            "a non-constant array index in '__builtin_offsetof'",
                            self.begin_loc(idx),
                        );
                        return None;
                    };
                    let size = target::size_align(&self.ast, elem, &self.abi).map_or(0, |x| x.0);
                    bits += i * i128::from(size) * 8;
                    cur = elem;
                }
                OffsetofComponent::Field { name, start, end } => {
                    let (b, next) = self.offsetof_field(loc, cur, name, start, end, &mut warned)?;
                    *invalid |= self.is_invalid_class(cur);
                    bits += b;
                    cur = next;
                }
            }
        }
        Some(bits / 8)
    }

    /// One `name` component of an offsetof designator into `cur`: the
    /// member's bit offset and type, or `None` after an error.
    fn offsetof_field(
        &mut self,
        loc: Loc,
        cur: QualType,
        name: Symbol,
        start: Loc,
        end: Loc,
        warned: &mut bool,
    ) -> Option<(i128, QualType)> {
        if !self.require_complete(cur, start) {
            let d = self.diag(DiagId::sema_offsetof_incomplete_type, start).arg(self.ty_arg(cur));
            self.emit(self.incomplete_note(d, cur));
            return None;
        }
        let Some(def) = self.ast.types.as_record(cur).and_then(|r| self.ast.definition(r)) else {
            let d = self.diag(DiagId::sema_offsetof_record_type, end).arg(self.ty_arg(cur));
            self.emit(d);
            return None;
        };
        // [support.types.layout]/1: conditionally-supported unless
        // standard-layout (clang warns once, in evaluated contexts).
        if !*warned && self.unevaluated == 0 && !self.is_standard_layout(def) {
            let d = self.diag(DiagId::sema_offsetof_non_standard_layout, loc).arg(self.ty_arg(cur));
            self.emit(d);
            *warned = true;
        }
        // Class member lookup ([class.member.lookup]): a member of a base
        // class is found too, as clang's `LookupQualifiedName`.
        let found = self.lookup_qualified(def, name, LookupKind::Ordinary);
        if self.diagnose_member_ambiguity(&found, DeclName::ident(name), start) {
            return None;
        }
        let member = found.decls.iter().copied().find(|&m| {
            matches!(self.ast.decls.get(m).kind(), DeclKind::Field | DeclKind::IndirectField)
        });
        let chain: Vec<DeclId> = match member {
            Some(m) if self.ast.decls.get(m).kind() == DeclKind::IndirectField => {
                ListId::from_raw(self.ast.decls.get(m).data())
                    .map(|l| self.ast.lists.iter::<DeclId>(l).collect())
                    .unwrap_or_default()
            }
            Some(m) => vec![m],
            None => Vec::new(),
        };
        let Some(&field) = chain.last() else {
            let d = self
                .diag(DiagId::sema_no_member, loc)
                .arg(self.ast.symbols().as_str(name).to_owned())
                .arg(self.ty_arg(cur));
            self.emit(d);
            return None;
        };
        if self.ast.decls.field(field).is_some_and(|f| f.bit_width.is_some()) {
            let note = self.diag(DiagId::note_bitfield_decl, self.ast.decls.get(field).loc());
            let d = self
                .diag(DiagId::sema_offsetof_bitfield, end)
                .arg(self.ast.symbols().as_str(name).to_owned())
                .with_note(note);
            self.emit(d);
            return None;
        }
        let mut bits = 0i128;
        if let Some(&outer) = chain.first()
            && let Some(parent) = self.ast.decls.get(outer).ctx()
        {
            bits = self.offsetof_base_bits(def, parent, end)?;
        }
        for f in chain {
            bits += i128::from(self.field_bit_offset(f));
        }
        let fty = self.ast.decls.get(field).ty().unwrap_or(QualType::ERROR);
        let fty = if self.ast.types.is_reference(fty) {
            self.ast.types.pointee(fty).unwrap_or(fty)
        } else {
            fty
        };
        Some((bits, fty))
    }

    /// The bit offset in class `def` of its base class subobject
    /// `parent` holding a member an offsetof designator names (0 when
    /// `parent` is `def`), or `None` after diagnosing a member of a
    /// virtual base, whose offset is not fixed (clang
    /// `BuildBuiltinOffsetOf`: "invalid application of 'offsetof' to a
    /// field of a virtual base"). Lookup already rejected an ambiguous
    /// base ([class.member.lookup]/6).
    fn offsetof_base_bits(&mut self, def: DeclId, parent: DeclId, end: Loc) -> Option<i128> {
        let BaseSearch::Unique(path) = find_base(self, def, parent) else {
            return Some(0);
        };
        if path.last_virtual.is_some() {
            self.error(DiagId::sema_offsetof_field_of_virtual_base, end);
            return None;
        }
        let mut bytes = 0u64;
        let mut cur = def;
        for &c in &path.classes {
            let first = self.ast.first(c);
            let bases = kiln_abi::ensure_layout(&mut self.ast, cur, &self.abi)?.bases.clone();
            bytes += bases.iter().find(|&&(b, _)| self.ast.first(b) == first)?.1;
            cur = self.ast.definition(c).unwrap_or(c);
        }
        Some(i128::from(bytes) * 8)
    }

    /// The bit offset of non-static data member `f` in its class.
    pub(crate) fn field_bit_offset(&mut self, f: DeclId) -> u64 {
        let Some(parent) = self.ast.decls.get(f).ctx() else { return 0 };
        let def = self.ast.definition(parent).unwrap_or(parent);
        let Some(i) = self
            .ast
            .members(def)
            .filter(|&m| self.ast.decls.get(m).kind() == DeclKind::Field)
            .position(|m| m == f)
        else {
            return 0;
        };
        kiln_abi::ensure_layout(&mut self.ast, def, &self.abi)
            .and_then(|l| l.fields.get(i).copied())
            .unwrap_or(0)
    }

    /// Whether class definition `def` is standard-layout ([class.prop]/3).
    /// Kiln's definition bits do not track it yet (base classes and access
    /// are not supported), so this computes it from the members and bases.
    pub(crate) fn is_standard_layout(&self, def: DeclId) -> bool {
        let rec = self.ast.decls.record(def);
        if rec.is_some_and(|r| r.defn.contains(DefinitionBits::POLYMORPHIC)) {
            return false;
        }
        let mut bases_with_fields = 0;
        for b in self.ast.decls.bases(def, &self.ast.lists) {
            let Some(bd) = self.ast.types.as_record(b.ty).and_then(|r| self.ast.definition(r))
            else {
                return false;
            };
            if b.is_virtual || !self.is_standard_layout(bd) {
                return false;
            }
            if self.has_fields(bd) {
                bases_with_fields += 1;
            }
        }
        let mut access = None;
        for m in self.ast.members(def) {
            if self.ast.decls.get(m).kind() != DeclKind::Field {
                continue;
            }
            let a = self.ast.decls.get(m).misc().access();
            if access.is_some_and(|x| x != a) {
                return false;
            }
            access = Some(a);
            let mut t = self.ast.decls.get(m).ty().unwrap_or(QualType::INT);
            if self.ast.types.is_reference(t) {
                return false;
            }
            while let Some(e) =
                self.ast.types.is_array(t).then(|| self.ast.types.element(t)).flatten()
            {
                t = e;
            }
            if let Some(fd) = self.ast.types.as_record(t).and_then(|r| self.ast.definition(r))
                && fd != def
                && !self.is_standard_layout(fd)
            {
                return false;
            }
        }
        bases_with_fields + usize::from(access.is_some()) <= 1
    }

    /// Whether class definition `def` or one of its bases has a
    /// non-static data member.
    fn has_fields(&self, def: DeclId) -> bool {
        self.ast.members(def).any(|m| self.ast.decls.get(m).kind() == DeclKind::Field)
            || self.ast.decls.bases(def, &self.ast.lists).any(|b| {
                self.ast
                    .types
                    .as_record(b.ty)
                    .and_then(|r| self.ast.definition(r))
                    .is_some_and(|d| self.has_fields(d))
            })
    }

    /// `( T ) { ... }`, a GNU/C99 compound literal in C++ (clang's
    /// `BuildCompoundLiteralExpr`): a prvalue of the written type `T`
    /// holding the list-initialization of cv-unqualified `T`, done as if
    /// by a C-style cast at `lparen`. Hence a reference `T` cannot bind
    /// to the list and more than one element for a scalar is reported
    /// as a function-style cast's (a vector's list is its elements); a
    /// function type is an illegal initializer type. Class and array
    /// literals need temporary materialization; a namespace-scope literal
    /// needs a constant initializer ([`Self::check_constant_literal_init`])
    /// and a namespace-scope array literal (an lvalue in clang) is
    /// unsupported.
    pub fn act_on_compound_literal(&mut self, ty: QualType, l: &InitList, lparen: Loc) -> ExprId {
        let flat = Self::list_exprs(l);
        if self.is_error_type(ty) {
            return self.recover(ty, &flat, lparen);
        }
        if self.ast.types.is_function(ty) {
            // A function type cannot be list-initialized (clang's
            // `InitListChecker::CheckListElementTypes`), diagnosed at the
            // `{`; a dependent one waits for instantiation.
            if !self.is_dependent_type(ty) {
                let msg =
                    self.diag(DiagId::sema_illegal_initializer_type, l.lbrace).arg(self.ty_arg(ty));
                self.emit(msg);
                return self.recover(ty, &flat, lparen);
            }
            let init = self.init_from_list_at(ty, l, Some(lparen));
            return self.builder().compound_literal(ty, init, lparen);
        }
        if self.is_dependent_type(ty) || self.init_list_type_dependent(l) {
            if self.func.is_none() && !self.check_dependent_literal_elems(l) {
                return self.recover(ty, &flat, lparen);
            }
            // Initialized at instantiation (clang's dependent
            // `InitializationSequence` keeps the list as written).
            let il = self.dependent_init_list(l);
            return self.builder().compound_literal(ty, il, lparen);
        }
        if self.ast.types.is_array(ty) {
            return self.array_compound_literal(ty, l, lparen, &flat);
        }
        if self.ast.types.is_reference(ty) {
            let to = self.non_reference(ty);
            let msg =
                self.diag(DiagId::sema_reference_bind_to_init_list, lparen).arg(self.ty_arg(to));
            self.emit(msg);
            return self.recover(ty, &flat, lparen);
        }
        if !self.require_complete(ty, lparen) {
            let msg =
                self.diag(DiagId::sema_typecheck_decl_incomplete_type, lparen).arg(self.ty_arg(ty));
            self.emit(self.incomplete_note(msg, ty));
            return self.recover(ty, &flat, lparen);
        }
        let class = self.ast.types.as_record(ty).is_some();
        if class && self.require_non_abstract_alloc(lparen, ty) {
            return self.recover(ty, &flat, lparen);
        }
        if !class && let Some(what) = self.temporary_unsupported(ty) {
            self.unsupported(&format!("compound literals of {what}"), lparen);
            return self.recover(ty, &flat, lparen);
        }
        // A vector or `_Complex` literal takes a list of its elements (clang
        // `InitListChecker::CheckVectorType` / `CheckComplexType`).
        if l.items.len() > 1
            && !class
            && !self.is_vector_type(ty)
            && self.ast.types.as_complex(ty).is_none()
        {
            self.error(DiagId::sema_builtin_func_cast_more_than_one_arg, lparen);
            return self.recover(ty, &flat, lparen);
        }
        let to = conv::strip_cv(self, ty);
        // A class with constructors: [over.match.list] as for an explicit
        // cast, a `CXXTemporaryObjectExpr` (clang `CreateCStyleCast` with
        // `InitList`, entity `EK_CompoundLiteralInit`).
        let constructed = if class {
            self.list_construct(to, l, CtorStyle::DirectList, lparen, Some(l.rbrace))
        } else {
            None
        };
        let init = constructed.unwrap_or_else(|| self.init_from_list_at(to, l, Some(lparen)));
        if self.is_err_expr(init) {
            return self.recover(ty, &[init], lparen);
        }
        if self.func.is_none() && !self.check_constant_literal_init(ty, init, lparen, l.lbrace) {
            return self.recover(ty, &[init], lparen);
        }
        let e = self.builder().compound_literal(ty, init, lparen);
        // A class literal is a temporary ([class.temporary]/4).
        if class { self.maybe_bind_temporary(e) } else { e }
    }

    /// An array compound literal: a prvalue of the written array type, its
    /// unknown bound deduced from the list ([dcl.init.aggr]/4; clang's
    /// `BuildCompoundLiteralExpr`).
    fn array_compound_literal(
        &mut self,
        ty: QualType,
        l: &InitList,
        lparen: Loc,
        flat: &[ExprId],
    ) -> ExprId {
        if !matches!(
            self.ast.types.canonical_view(ty),
            TypeView::ConstantArray(_) | TypeView::IncompleteArray(_)
        ) {
            let what = self.temporary_unsupported(ty).unwrap_or("a temporary");
            self.unsupported(&format!("compound literals of {what}"), lparen);
            return self.recover(ty, flat, lparen);
        }
        // clang's `isFileScope` literal: outside a function (at namespace
        // or class scope) an array literal is an lvalue naming an object
        // of static storage duration, an internal `.compoundliteral`
        // global (WP-466).
        let file_scope = self.func.is_none();
        let init = self.init_from_list_at(ty, l, Some(lparen));
        if self.is_err_expr(init) {
            return self.recover(ty, &[init], lparen);
        }
        let ty = if matches!(self.ast.types.canonical_view(ty), TypeView::IncompleteArray(_)) {
            self.ety(init)
        } else {
            ty
        };
        if file_scope {
            if !self.check_constant_literal_init(ty, init, lparen, l.lbrace) {
                // An lvalue, as the literal: its decay is no temporary.
                return self.builder().recovery(Some(ty), ValueCategory::LValue, &[init], lparen);
            }
            // The object's context is the enclosing namespace.
            let mut ctx = self.ctx;
            while !matches!(
                self.ast.decls.get(ctx).kind(),
                DeclKind::Namespace
                    | DeclKind::TranslationUnit
                    | DeclKind::LinkageSpec
                    | DeclKind::Export
            ) && let Some(c) = self.ast.decls.get(ctx).ctx()
            {
                ctx = c;
            }
            return self.builder().file_scope_compound_literal(ctx, ty, init, lparen).0;
        }
        self.builder().compound_literal(ty, init, lparen)
    }

    /// clang's `CheckForConstantInitializer` for a namespace-scope
    /// compound literal (C99 6.5.2.5p3, [GNU]): the literal's
    /// initializer must be a constant initializer
    /// (`Expr::isConstantInitializer`). The culprit is the first
    /// element that does not fold, walking aggregate lists; a scalar
    /// or vector literal is itself the culprit, reported at its `{`.
    /// Reports and returns `false` on failure.
    fn check_constant_literal_init(
        &mut self,
        ty: QualType,
        init: ExprId,
        lparen: Loc,
        lbrace: Loc,
    ) -> bool {
        let aggregate = self.ast.types.is_array(ty) || self.ast.types.as_record(ty).is_some();
        let culprit = if aggregate && self.ast.exprs.get(init).kind() == ExprKind::InitListExpr {
            self.constant_init_culprit(init)
        } else if aggregate {
            // A constructor call begins at the type-id after the `(`.
            let at = match self.culprit_loc(init) {
                at if at == lparen => self.next_token_loc(lparen),
                at => at,
            };
            self.try_fold(init).is_none().then_some(at)
        } else if self.clang_compat >= 22 {
            // clang 22 looks into a scalar, vector or complex literal's
            // list for the element.
            self.constant_init_culprit(init)
        } else {
            self.try_fold(init).is_none().then_some(lbrace)
        };
        let Some(at) = culprit else { return true };
        self.error(DiagId::sema_init_element_not_constant, at);
        false
    }

    /// The location of the first element of `e` that is not a constant
    /// (an init list's elements recursively, else `e` itself).
    fn constant_init_culprit(&mut self, e: ExprId) -> Option<Loc> {
        if self.ast.exprs.get(e).kind() == ExprKind::InitListExpr {
            let elems: Vec<ExprId> =
                self.ast.lists.iter(self.ast.exprs.init_list_inits(e)).collect();
            return elems.into_iter().find_map(|x| self.constant_init_culprit(x));
        }
        if self.ast.exprs.get(e).kind() == ExprKind::ImplicitValueInitExpr {
            return None;
        }
        self.try_fold(e).is_none().then(|| self.element_culprit_loc(e))
    }

    /// Where a non-constant element is reported: clang 22 names the
    /// element itself at its `getExprLoc` (an operator, else where it
    /// begins, an implicit conversion included); clang 19 looks through
    /// parentheses and no-op casts first ([`Self::culprit_loc`]).
    fn element_culprit_loc(&self, e: ExprId) -> Loc {
        if self.clang_compat >= 22 { self.expr_loc(e) } else { self.culprit_loc(e) }
    }

    /// clang 22 also checks a namespace-scope literal of dependent type
    /// when its written elements are not dependent, at the template
    /// definition: the first element that does not fold is reported and
    /// the literal is invalid, so instantiation does not report it again.
    /// Clang 19 waits for instantiation. Returns `false` on failure.
    fn check_dependent_literal_elems(&mut self, l: &InitList) -> bool {
        if self.clang_compat < 22 || self.init_list_value_dependent(l) {
            return true;
        }
        let Some(at) = self.list_culprit(l) else { return true };
        self.error(DiagId::sema_init_element_not_constant, at);
        false
    }

    /// The first written element of `l` (nested lists included) that
    /// does not fold.
    fn list_culprit(&mut self, l: &InitList) -> Option<Loc> {
        l.items.iter().find_map(|it| match &it.value {
            InitValue::Expr(e) => self.try_fold(*e).is_none().then(|| self.expr_loc(*e)),
            InitValue::List(n) => self.list_culprit(n),
        })
    }

    /// Whether a written element or designator of `l` is value-dependent.
    fn init_list_value_dependent(&self, l: &InitList) -> bool {
        l.items.iter().any(|it| {
            it.designator.iter().any(|d| match *d {
                Designator::Index(e, _) => self.expr_is_value_dependent(e),
                Designator::Range(a, b, _, _) => {
                    self.expr_is_value_dependent(a) || self.expr_is_value_dependent(b)
                }
                Designator::Field(..) => false,
            }) || match &it.value {
                InitValue::Expr(e) => self.expr_is_value_dependent(*e),
                InitValue::List(n) => self.init_list_value_dependent(n),
            }
        })
    }

    /// clang's `getExprLoc` of a culprit: the operator of a binary or
    /// member expression (kiln's node location), else its first token.
    pub(crate) fn culprit_loc(&self, mut e: ExprId) -> Loc {
        while self.ast.exprs.get(e).kind() == ExprKind::ImplicitCastExpr {
            e = self.ast.exprs.operand(e);
        }
        match self.ast.exprs.get(e).kind() {
            ExprKind::BinaryOperator | ExprKind::CompoundAssignOperator | ExprKind::MemberExpr => {
                self.ast.exprs.get(e).loc()
            }
            _ => self.begin_loc(e),
        }
    }
}
