//! GNU extensions kiln accepts in C++ (spec/sema.md §5.8): statement
//! expressions `({ ... })`, `__builtin_offsetof` and compound literals
//! `( T ) { ... }`. Each matches clang's behaviour for C++.

use kiln_ast::decl::{DeclKind, DefinitionBits};
use kiln_ast::ids::RawId;
use kiln_ast::stmt::StmtKind;
use kiln_ast::{DeclId, ExprId, ListId, QualType, StmtId};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::conv::{self, ConvCtx};
use crate::init::InitList;
use crate::lookup::LookupKind;
use crate::target;

/// The state saved across a statement expression
/// ([`Sema::act_on_start_stmt_expr`]): the enclosing full-expression's
/// pending potential odr-uses, which the statements inside must not
/// resolve.
#[derive(Debug)]
pub struct StmtExprScope {
    outer_odr: Vec<ExprId>,
}

/// One part of a `__builtin_offsetof` designator (clang
/// `Sema::OffsetOfComponent`).
#[derive(Clone, Copy, Debug)]
pub enum OffsetofComponent {
    /// A member name: the leading identifier (`start` = `end` = its
    /// location) or `.identifier` (`start` is the `.`).
    Field {
        /// The member name.
        name: Symbol,
        /// The `.`, or the name for the leading identifier.
        start: Loc,
        /// The name.
        end: Loc,
    },
    /// `[ index ]`.
    Index {
        /// The index expression.
        index: ExprId,
        /// The `[`.
        start: Loc,
        /// The `]`.
        end: Loc,
    },
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
        StmtExprScope { outer_odr: std::mem::take(&mut self.pending_odr) }
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
    /// not a null statement, looking through labels, when that is an
    /// expression statement, else `void`; it is always a prvalue. `lparen`
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
        let ty = self.stmt_expr_result(body).map_or(QualType::VOID, |e| self.ast.exprs.get(e).ty());
        self.builder().stmt_expr(body, ty, lparen, rparen)
    }

    /// The expression whose value a statement expression with body `body`
    /// yields (clang's `getStmtExprResult` + `ValueStmt::getExprStmt`).
    pub(crate) fn stmt_expr_result(&self, body: StmtId) -> Option<ExprId> {
        let stmts = &self.ast.stmts;
        let children = self.ast.lists.get(stmts.compound_body(body));
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
        let mut indices = Vec::new();
        let offset = self.offsetof_value(loc, ty, comps, &mut indices);
        let fits = offset.and_then(|v| u32::try_from(v).ok());
        if offset.is_some() && fits.is_none() {
            self.unsupported("a '__builtin_offsetof' value outside 0 to 2^32 - 1", loc);
        }
        match fits {
            Some(v) => {
                let st = self.size_type();
                self.builder().offsetof(st, &indices, v, loc, rparen)
            }
            None => self.recover_err(&indices, loc),
        }
    }

    /// The offset in bytes `__builtin_offsetof(ty, comps...)` designates,
    /// or `None` after an error. The converted indices go to `indices`.
    fn offsetof_value(
        &mut self,
        loc: Loc,
        ty: QualType,
        comps: &[OffsetofComponent],
        indices: &mut Vec<ExprId>,
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
        let found = self.lookup_in(def, name, LookupKind::Ordinary);
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

    /// The bit offset of non-static data member `f` in its class.
    fn field_bit_offset(&mut self, f: DeclId) -> u64 {
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
        kiln_abi::ensure_layout(&mut self.ast, def, &self.abi).fields.get(i).copied().unwrap_or(0)
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
    /// as a function-style cast's. Class and array literals need
    /// temporary materialization and namespace-scope literals constant
    /// elements; both are unsupported.
    pub fn act_on_compound_literal(&mut self, ty: QualType, l: &InitList, lparen: Loc) -> ExprId {
        let flat = Self::list_exprs(l);
        if self.is_error_type(ty) {
            return self.recover(ty, &flat, lparen);
        }
        if self.ast.types.is_array(ty) || self.ast.types.is_function(ty) {
            let what = self.temporary_unsupported(ty).unwrap_or("a temporary");
            self.unsupported(&format!("compound literals of {what}"), lparen);
            return self.recover(ty, &flat, lparen);
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
        if let Some(what) = self.temporary_unsupported(ty) {
            self.unsupported(&format!("compound literals of {what}"), lparen);
            return self.recover(ty, &flat, lparen);
        }
        if l.items.len() > 1 {
            self.error(DiagId::sema_builtin_func_cast_more_than_one_arg, lparen);
            return self.recover(ty, &flat, lparen);
        }
        if self.func.is_none() {
            self.unsupported("compound literals at namespace scope", lparen);
            return self.recover(ty, &flat, lparen);
        }
        let to = conv::strip_cv(self, ty);
        let init = self.init_from_list_at(to, l, Some(lparen));
        if self.is_err_expr(init) {
            return self.recover(ty, &[init], lparen);
        }
        self.builder().compound_literal(ty, init, lparen)
    }
}
