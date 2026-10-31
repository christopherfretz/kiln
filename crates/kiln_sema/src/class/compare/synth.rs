//! The implicit definition of a defaulted comparison operator
//! ([class.eq]/3, [class.spaceship]/3, [class.compare.secondary]/2;
//! clang `DefineDefaultedComparison` and `DefaultedComparisonSynthesizer`,
//! spec/sema-class.md §8.3) and its implicit exception specification
//! ([except.spec]/9; clang `ComputeDefaultedComparisonExceptionSpec`).
//!
//! The body is clang's: every expression and statement sits at the last
//! character of the `default` keyword.
//! - `==`: the subobject comparisons, each contextually converted to
//!   `bool`, joined with `&&` (right fold); an array becomes
//!   `for (size_t iN = 0; iN != n; ++iN) if (!(a[iN] == b[iN])) return false;`
//!   and the comparisons before it `if (!(...)) return false;`; the
//!   comparisons after the last array are returned, else `return true;`.
//! - `<=>`: `if (R cmp = static_cast<R>(a <=> b); cmp != 0) return cmp;`
//!   per subobject (arrays as above), then
//!   `return static_cast<R>(std::strong_ordering::equal);`.
//! - `!=`, `<`, `<=`, `>`, `>=`: `return x @ y;` on the complete objects.
//!
//! A base subobject is the complete object converted to the
//! *unqualified* base type (`DerivedToBase`), as clang 19 does; the
//! selected operator's object conversion then adds the qualifiers.

use kiln_ast::class::{CmpCategory, CmpResult};
use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits, FieldData, VarSpec};
use kiln_ast::expr::{BinaryOp, CastKind, UnaryOp, ValueCategory};
use kiln_ast::names::DeclName;
use kiln_ast::stmt::{IfKind, LoopData, StmtKind};
use kiln_ast::types::{ExcSpec, NoexceptValue, Quals};
use kiln_ast::{DeclHead, DeclId, ExprId, QualType, StmtId};
use kiln_source::Loc;

use super::{CmpKind, CmpUsage};
use crate::Sema;
use crate::class::special::SynthKind;
use crate::conv;
use crate::expr::CastStyle;
use crate::init::Initializer;
use crate::overload::oper::binary_of;
use crate::scope::{Scope, ScopeKind};

/// The column offset of the last character of `default` from its first.
const DEFAULT_KW_END: u32 = 6;

/// The recursion bound of nested arrays and anonymous structs.
const MAX_DEPTH: u32 = 256;

/// A synthesized subobject comparison: an expression (`==` and the
/// secondary operators, before they are joined) or a statement.
#[derive(Clone, Copy)]
enum Piece {
    Expr(ExprId),
    Stmt(StmtId),
}

/// The state of one synthesis.
struct Synth {
    /// The defaulted declaration.
    f: DeclId,
    k: CmpKind,
    /// Where the body is placed.
    loc: Loc,
    /// The nesting depth of arrays (the iteration variable's number).
    depth: u32,
}

impl Sema<'_, '_> {
    /// Defines defaulted comparison `f` (checked, not deleted), first
    /// required at `use_loc` (clang `DefineDefaultedComparison`): its body
    /// goes on the declaration defaulted; a body that cannot be built
    /// (an error diagnosed with the note `in defaulted ... comparison
    /// operator for 'T' first required here`) marks it invalid. Resolves
    /// its exception specification.
    pub(crate) fn define_defaulted_comparison(&mut self, f: DeclId, use_loc: Loc) {
        let Some((d, k, loc)) = self.cmp_to_define(f) else { return };
        let first = self.ast.first(d);
        let saved = self.push_synth(d, SynthKind::Comparison(k), use_loc);
        self.special.cmp.defining.push(first);
        let body = self.build_cmp_body(d, k, loc);
        let _ = self.special.cmp.defining.pop();
        match body {
            Some(b) => {
                let _ = self.builder().set_body(d, b);
                self.mark_used(d);
            }
            None => self.ast.decls.set_flags(d, DeclFlags::INVALID),
        }
        self.pop_synth(saved);
        self.resolve_exception_spec(d);
    }

    /// The defaulted comparison to define for a use of `f`: the
    /// declaration defaulted, its kind and the body location; `None` when
    /// it is defined (or being defined), deleted, invalid, or its class
    /// is not complete and non-dependent.
    fn cmp_to_define(&self, f: DeclId) -> Option<(DeclId, CmpKind, Loc)> {
        let first = self.ast.first(f);
        let &(d, dl) = self.special.cmp.default_loc.get(&first)?;
        let k = self.cmp_kind(d)?;
        if self.cmp_has_body(d)
            || self.ast.decls.get(d).has(DeclFlags::INVALID)
            || self.ast.decls.get(d).has(DeclFlags::DEPENDENT_CTX)
            || self.special.cmp.invalid.contains(&first)
            || self.special.cmp.defining.contains(&first)
            || self.is_deleted_fn(d)
            || self.fn_spec(first).is_deleted()
        {
            return None;
        }
        let class = self.class_def(self.cmp_class(d)?);
        let cflags = self.ast.decls.get(class).flags();
        if cflags.contains(DeclFlags::INVALID)
            || cflags.contains(DeclFlags::DEPENDENT_CTX)
            || !self.defn_bits(class).contains(DefinitionBits::COMPLETE)
        {
            return None;
        }
        let loc = if dl.is_valid() {
            dl.with_offset(DEFAULT_KW_END)
        } else {
            self.ast.decls.get(d).loc()
        };
        Some((d, k, loc))
    }

    /// Whether some declaration of `f` has a body.
    fn cmp_has_body(&self, f: DeclId) -> bool {
        self.ast.redecls_of(f).any(|r| self.ast.decls.function(r).is_some_and(|x| x.body.is_some()))
    }

    /// Builds the body of defaulted comparison `d` (the synthesis context
    /// pushed): its subobject comparisons exclude `d` and use the
    /// operator lookups captured at its `= default`.
    fn build_cmp_body(&mut self, d: DeclId, k: CmpKind, loc: Loc) -> Option<StmtId> {
        let first = self.ast.first(d);
        let saved_ex = self.special.cmp.exclude.replace((first, self.ctx));
        let saved_scopes =
            std::mem::replace(&mut self.scopes, vec![Scope::new(ScopeKind::Function, Some(d))]);
        let mut sx = Synth { f: d, k, loc, depth: 0 };
        let r = self.cmp_build(&mut sx);
        self.scopes = saved_scopes;
        self.special.cmp.exclude = saved_ex;
        r
    }

    /// clang `DefaultedComparisonSynthesizer::build`.
    fn cmp_build(&mut self, sx: &mut Synth) -> Option<StmtId> {
        let loc = sx.loc;
        let mut pieces = self.cmp_visit(sx)?;
        let mut stmts: Vec<StmtId> = Vec::new();
        let ret = match sx.k {
            CmpKind::Equal => {
                let mut ret = None;
                let mut so_far: Option<ExprId> = None;
                for p in pieces.into_iter().rev() {
                    match p {
                        Piece::Stmt(s) => {
                            if let Some(prior) = so_far.take() {
                                if ret.is_none() && stmts.is_empty() {
                                    ret = Some(prior);
                                } else {
                                    let s = self.if_not_return_false(prior, loc)?;
                                    stmts.push(s);
                                }
                            }
                            stmts.push(s);
                        }
                        Piece::Expr(e) => {
                            so_far = Some(match so_far {
                                None => e,
                                Some(r) => {
                                    let x = self.builtin_binary(BinaryOp::LAnd, e, r, loc);
                                    self.ok_expr(x)?
                                }
                            });
                        }
                    }
                }
                if let Some(prior) = so_far.take() {
                    if ret.is_none() && stmts.is_empty() {
                        ret = Some(prior);
                    } else {
                        let s = self.if_not_return_false(prior, loc)?;
                        stmts.push(s);
                    }
                }
                stmts.reverse();
                ret.unwrap_or_else(|| self.builder().bool_literal(QualType::BOOL, true, loc))
            }
            CmpKind::ThreeWay => {
                stmts = pieces
                    .into_iter()
                    .map(|p| match p {
                        Piece::Stmt(s) => s,
                        Piece::Expr(e) => self.builder().expr_stmt(e),
                    })
                    .collect();
                self.cmp_category_type(CmpCategory::Strong, loc, CmpUsage::Defaulted)?;
                let equal = self
                    .ast
                    .cmp_categories
                    .get(CmpCategory::Strong)
                    .and_then(|i| i.values[CmpResult::Equal as usize])?;
                let e = self.build_decl_ref(equal, loc);
                let e = self.ok_expr(e)?;
                self.static_cast_to_ret(sx.f, e, loc)?
            }
            CmpKind::NotEqual | CmpKind::Relational => match pieces.pop() {
                Some(Piece::Expr(e)) => e,
                _ => return None,
            },
        };
        let r = self.act_on_return(Some(ret), loc);
        if self.ast.stmts.return_value(r).is_some_and(|e| self.is_err_expr(e)) {
            return None;
        }
        stmts.push(r);
        Some(self.builder().compound(&stmts, loc, loc))
    }

    /// `e` unless it is an error.
    fn ok_expr(&self, e: ExprId) -> Option<ExprId> {
        (!self.is_err_expr(e)).then_some(e)
    }

    /// clang `DefaultedComparisonVisitor::visit`.
    fn cmp_visit(&mut self, sx: &mut Synth) -> Option<Vec<Piece>> {
        let params = self.param_decls(sx.f);
        let last = *params.last()?;
        let pty = self.ast.decls.get(last).ty().unwrap_or(QualType::ERROR);
        let plval = self.non_reference(pty);
        let mut out = Vec::new();
        if sx.k.is_primary() {
            let class = self.class_def(self.cmp_class(sx.f)?);
            let quals = self.ast.types.effective_quals(plval);
            self.cmp_visit_subobjects(sx, class, quals, &mut out)?;
        } else {
            let obj = self.cmp_complete_object(sx)?;
            out.push(self.cmp_visit_expanded(sx, plval, obj)?);
        }
        Some(out)
    }

    /// clang `visitSubobjects`: the direct bases, then the non-static
    /// data members (unnamed bit-fields skipped, anonymous structs
    /// flattened).
    fn cmp_visit_subobjects(
        &mut self,
        sx: &mut Synth,
        record: DeclId,
        quals: Quals,
        out: &mut Vec<Piece>,
    ) -> Option<()> {
        for (idx, spec) in self.indexed_bases(record) {
            let t = self.ast.types.qualify(spec.ty, quals);
            let (l, r) = self.cmp_complete_object(sx)?;
            let path = self.ast.lists.push_raw(&[idx]);
            let l = conv::raw_cast(
                self,
                CastKind::DerivedToBase,
                l,
                spec.ty,
                ValueCategory::LValue,
                Some(path),
            );
            let r = conv::raw_cast(
                self,
                CastKind::DerivedToBase,
                r,
                spec.ty,
                ValueCategory::LValue,
                Some(path),
            );
            out.push(self.cmp_visit_subobject(sx, t, (l, r))?);
        }
        let fields: Vec<DeclId> = self
            .ast
            .decls
            .members(record)
            .filter(|&m| self.kind_of(m) == DeclKind::Field)
            .collect();
        for fd in fields {
            let data = self.ast.decls.field(fd).copied();
            let n = *self.ast.decls.get(fd);
            let fty = n.ty().unwrap_or(QualType::ERROR);
            if data.is_some_and(|x| x.bit_width.is_some()) && n.name().is_empty() {
                continue;
            }
            if let Some(anon) = self.ast.types.as_record(fty)
                && self.defn_bits(anon).contains(DefinitionBits::ANONYMOUS)
            {
                if sx.depth > MAX_DEPTH {
                    return None;
                }
                let anon = self.class_def(anon);
                self.cmp_visit_subobjects(sx, anon, quals, out)?;
                continue;
            }
            let mut q = quals;
            if data.is_some_and(|x| x.flags & FieldData::MUTABLE != 0) {
                q = Quals::from_bits(q.bits() & !Quals::CONST.bits());
            }
            let t = self.ast.types.qualify(fty, q);
            let (l, r) = self.cmp_complete_object(sx)?;
            let lq = self.ast.types.effective_quals(self.ety(l));
            let rq = self.ast.types.effective_quals(self.ety(r));
            let l = self.field_access(l, fd, lq, false, sx.loc);
            let r = self.field_access(r, fd, rq, false, sx.loc);
            out.push(self.cmp_visit_subobject(sx, t, (l, r))?);
        }
        Some(())
    }

    /// clang `getCompleteObject`: `*this` (an implicit object member
    /// function) or the first parameter, and the last parameter.
    fn cmp_complete_object(&mut self, sx: &Synth) -> Option<(ExprId, ExprId)> {
        let loc = sx.loc;
        let params = self.param_decls(sx.f);
        let implicit_obj =
            self.kind_of(sx.f) == DeclKind::CXXMethod && !self.has_explicit_object(sx.f);
        let (l, rp) = if implicit_obj {
            let this = self.act_on_this(loc);
            let this = self.ok_expr(this)?;
            (self.builtin_unary(UnaryOp::Deref, this, loc), *params.first()?)
        } else {
            let l = self.build_decl_ref(*params.first()?, loc);
            (l, *params.get(1)?)
        };
        let r = self.build_decl_ref(rp, loc);
        Some((self.ok_expr(l)?, self.ok_expr(r)?))
    }

    /// clang `visitSubobject`: an array subobject is compared element by
    /// element.
    fn cmp_visit_subobject(
        &mut self,
        sx: &mut Synth,
        t: QualType,
        obj: (ExprId, ExprId),
    ) -> Option<Piece> {
        if let Some(n) = self.ast.types.array_size(t) {
            let q = self.ast.types.effective_quals(t);
            let elem = self.ast.types.element(t)?;
            let elem = self.ast.types.qualify(elem, q);
            return self.cmp_visit_array(sx, elem, n, obj);
        }
        self.cmp_visit_expanded(sx, t, obj)
    }

    /// clang `visitSubobjectArray`:
    /// `for (size_t iN = 0; iN != n; ++iN) <compare a[iN] and b[iN]>`.
    fn cmp_visit_array(
        &mut self,
        sx: &mut Synth,
        elem: QualType,
        n: u64,
        obj: (ExprId, ExprId),
    ) -> Option<Piece> {
        if sx.depth > MAX_DEPTH {
            return None;
        }
        let loc = sx.loc;
        let sz = self.size_type();
        let name = self.intern(&format!("i{}", sx.depth));
        let v = self.builder().var(
            DeclHead::new(sx.f, DeclName::ident(name), loc),
            sz,
            VarSpec::new(),
            None,
        );
        let zero = self.builder().int_literal(sz, 0, kiln_ast::expr::Radix::Decimal, loc);
        let _ = self.builder().set_var_init(v, zero);
        let init = self.builder().decl_stmt(&[v], loc);
        let i = self.build_decl_ref(v, loc);
        let bound = self.builder().int_literal(sz, n, kiln_ast::expr::Radix::Decimal, loc);
        let cond = self.builtin_binary(BinaryOp::NE, i, bound, loc);
        let i = self.build_decl_ref(v, loc);
        let inc = self.builtin_unary(UnaryOp::PreInc, i, loc);
        let i = self.build_decl_ref(v, loc);
        let a = self.builtin_subscript(obj.0, i, loc);
        let i = self.build_decl_ref(v, loc);
        let b = self.builtin_subscript(obj.1, i, loc);
        let (a, b) = (self.ok_expr(a)?, self.ok_expr(b)?);
        sx.depth += 1;
        let sub = self.cmp_visit_subobject(sx, elem, (a, b));
        sx.depth -= 1;
        let body = match sub? {
            Piece::Expr(e) => self.if_not_return_false(e, loc)?,
            Piece::Stmt(s) => s,
        };
        let data = LoopData {
            init: Some(init),
            cond_var: None,
            cond: Some(cond),
            inc: Some(inc),
            body: Some(body),
        };
        Some(Piece::Stmt(self.builder().for_stmt(data, loc)))
    }

    /// clang `visitExpandedSubobject`: `x @ y` for the operator of the
    /// defaulted comparison, in the form of its kind.
    fn cmp_visit_expanded(
        &mut self,
        sx: &mut Synth,
        _t: QualType,
        (l, r): (ExprId, ExprId),
    ) -> Option<Piece> {
        let loc = sx.loc;
        let op = binary_of(self.fn_operator(sx.f)?)?;
        let e = self.binary_operator(op, l, r, None, loc);
        let e = self.ok_expr(e)?;
        match sx.k {
            CmpKind::Equal => match conv::contextually_convert_to_bool(self, e) {
                Ok(c) => Some(Piece::Expr(c)),
                Err(err) => {
                    self.emit(err.diag_at(self, loc));
                    None
                }
            },
            CmpKind::ThreeWay => {
                let ret = self.fn_ret(sx.f);
                let e = self.static_cast_to_ret(sx.f, e, loc)?;
                let name = self.intern("cmp");
                let v = self.builder().var(
                    DeclHead::new(sx.f, DeclName::ident(name), loc),
                    ret,
                    VarSpec::new(),
                    None,
                );
                self.act_on_init(v, Initializer::Expr(e));
                let init_ok = self
                    .ast
                    .decls
                    .var(v)
                    .and_then(|x| x.init)
                    .is_some_and(|i| !self.is_err_expr(i));
                if !init_ok || self.ast.decls.get(v).has(DeclFlags::INVALID) {
                    return None;
                }
                let init = self.builder().decl_stmt(&[v], loc);
                let cmp = self.build_decl_ref(v, loc);
                let zero = self.builder().int_literal(
                    QualType::INT,
                    0,
                    kiln_ast::expr::Radix::Decimal,
                    loc,
                );
                let ne = self.binary_operator(BinaryOp::NE, cmp, zero, None, loc);
                let ne = self.ok_expr(ne)?;
                let cond = self.act_on_condition(ne);
                let cond = self.ok_expr(cond)?;
                let cmp = self.build_decl_ref(v, loc);
                let ret = self.act_on_return(Some(cmp), loc);
                if self.ast.stmts.return_value(ret).is_some_and(|x| self.is_err_expr(x)) {
                    return None;
                }
                Some(Piece::Stmt(self.act_on_if(
                    IfKind::Ordinary,
                    Some(init),
                    None,
                    Some(cond),
                    ret,
                    None,
                    loc,
                )))
            }
            CmpKind::NotEqual | CmpKind::Relational => Some(Piece::Expr(e)),
        }
    }

    /// clang `buildStaticCastToR`: `static_cast<R>(e)` for the return type
    /// `R` of `f`, omitted for a prvalue of type `R`.
    fn static_cast_to_ret(&mut self, f: DeclId, e: ExprId, loc: Loc) -> Option<ExprId> {
        let ret = self.fn_ret(f);
        let prvalue = self.ast.exprs.get(e).value_category() == ValueCategory::PRValue;
        if prvalue && self.ast.types.same_type(self.ety(e), ret) {
            return Some(e);
        }
        let c = self.act_on_cast(CastStyle::Static, ret, e, loc, loc);
        self.ok_expr(c)
    }

    /// clang `buildIfNotCondReturnFalse`: `if (!(cond)) return false;`.
    fn if_not_return_false(&mut self, cond: ExprId, loc: Loc) -> Option<StmtId> {
        let not = self.builtin_unary(UnaryOp::LNot, cond, loc);
        let not = self.ok_expr(not)?;
        let f = self.builder().bool_literal(QualType::BOOL, false, loc);
        let ret = self.act_on_return(Some(f), loc);
        let c = self.act_on_condition(not);
        Some(self.act_on_if(IfKind::Ordinary, None, None, Some(c), ret, None, loc))
    }

    // -- the exception specification ----------------------------------------

    /// The implicit exception specification of defaulted comparison `f`
    /// (its first declaration; [except.spec]/9, clang
    /// `ComputeDefaultedComparisonExceptionSpec`): non-throwing unless an
    /// expression of its body may throw. Without a body, the body is
    /// built in an unevaluated context and discarded. Sets it on every
    /// declaration; returns whether it is non-throwing.
    pub(crate) fn resolve_cmp_exc(&mut self, first: DeclId) -> bool {
        if self.special.cmp.defining.contains(&first) {
            return false;
        }
        let def = self
            .ast
            .redecls_of(first)
            .find_map(|r| self.ast.decls.function(r).and_then(|x| x.body));
        let nothrow = if let Some(body) = def {
            !self.stmt_can_throw(body, 0)
        } else if let Some((d, k, loc)) = self.cmp_to_define(first) {
            let saved = self.push_synth(d, SynthKind::Comparison(k), loc);
            self.unevaluated += 1;
            self.special.cmp.defining.push(first);
            let body = self.build_cmp_body(d, k, loc);
            let _ = self.special.cmp.defining.pop();
            self.pop_synth(saved);
            body.is_none_or(|b| !self.stmt_can_throw(b, 0))
        } else {
            true
        };
        let spec = if nothrow {
            ExcSpec::BasicNoexcept
        } else {
            ExcSpec::NoexceptExpr { expr: self.noexcept_false(), value: NoexceptValue::False }
        };
        let redecls: Vec<DeclId> = self.ast.redecls_of(first).collect();
        for d in redecls {
            let Some(ty) = self.ast.decls.get(d).ty() else { continue };
            let t = self.with_exc_spec(ty, spec);
            self.ast.decls.set_ty(d, t);
        }
        nothrow
    }

    /// Whether an expression of the synthesized statement `s` may throw.
    fn stmt_can_throw(&self, s: StmtId, depth: u32) -> bool {
        if depth > MAX_DEPTH * 4 {
            return true;
        }
        let st = &self.ast.stmts;
        let var_init = |d: DeclId| self.ast.decls.var(d).and_then(|v| v.init);
        let expr = |e: Option<ExprId>| e.is_some_and(|e| self.can_throw(e));
        let sub = |s: Option<StmtId>| s.is_some_and(|s| self.stmt_can_throw(s, depth + 1));
        match st.get(s).kind() {
            StmtKind::CompoundStmt => {
                self.ast.lists.iter(st.compound_body(s)).any(|c| self.stmt_can_throw(c, depth + 1))
            }
            StmtKind::IfStmt => {
                let d = st.if_data(s);
                sub(d.init)
                    || expr(d.cond_var.and_then(var_init))
                    || expr(d.cond)
                    || sub(d.then)
                    || sub(d.els)
            }
            StmtKind::ForStmt => {
                let d = st.loop_data(s);
                sub(d.init) || expr(d.cond) || expr(d.inc) || sub(d.body)
            }
            StmtKind::ReturnStmt => expr(st.return_value(s)),
            StmtKind::DeclStmt => {
                self.ast.lists.iter::<DeclId>(st.decl_stmt_decls(s)).any(|d| expr(var_init(d)))
            }
            StmtKind::ExprStmt => self.can_throw(st.expr_stmt_expr(s)),
            _ => false,
        }
    }
}
