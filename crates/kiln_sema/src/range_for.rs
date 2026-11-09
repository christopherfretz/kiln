//! Range-based `for` ([stmt.ranged], WP-089): clang's desugaring into the
//! implicit `__range`/`__begin`/`__end` variables (`ActOnCXXForRangeStmt`,
//! `BuildCXXForRangeStmt`, `BuildNonArrayForRange`), its diagnostics, and
//! P2718 lifetime extension of the for-range-initializer's temporaries
//! (spec/sema.md §7.8, spec/sema-class.md §5.4).

use kiln_ast::decl::{DeclFlags, DeclKind, InitStyle, VarSpec};
use kiln_ast::expr::{BinaryOp, ExprKind, Radix, StorageDuration, UnaryOp};
use kiln_ast::names::DeclName;
use kiln_ast::stmt::RangeForData;
use kiln_ast::types::AutoKeyword;
use kiln_ast::{DeclHead, DeclId, ExprId, QualType, StmtId};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::declspec::{DeclSpec, StorageSpec};
use crate::init::{InitList, Initializer};
use crate::lookup::{Found, LookupKind};
use crate::overload::{CandidateKind, IcsOptions, OvlOutcome, SetKind};
use crate::templ::auto::{AutoFail, AutoInit};

/// A for-range-initializer as parsed.
#[derive(Debug)]
pub enum RangeInit {
    /// An expression.
    Expr(ExprId),
    /// A braced-init-list.
    List(InitList),
}

/// The P2718 collection state of the enclosing context, set aside while
/// a for-range-initializer is parsed ([`Sema::begin_for_range_init`]).
#[must_use]
#[derive(Debug)]
pub struct RangeCollect(Option<Vec<ExprId>>);

/// A range-based `for` whose head was analysed; the body completes it
/// ([`Sema::act_on_finish_for_range`]).
#[derive(Clone, Copy, Debug)]
pub struct RangeForHead {
    init: Option<StmtId>,
    range: DeclId,
    begin: Option<DeclId>,
    end: Option<DeclId>,
    cond: Option<ExprId>,
    inc: Option<ExprId>,
    loop_var: DeclId,
    colon: Loc,
    loc: Loc,
}

/// clang's `BeginEndFunction` (the `%select{begin|end}` argument).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Bef {
    Begin,
    End,
}

impl Bef {
    fn index(self) -> u8 {
        match self {
            Bef::Begin => 0,
            Bef::End => 1,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Bef::Begin => "begin",
            Bef::End => "end",
        }
    }
}

/// clang's `ForRangeStatus`.
enum Status {
    Success,
    /// No viable function; the candidate notes.
    NoViable(Vec<Diag>),
    DiagnosticIssued,
}

/// The parts of a head being built (clang's `BuildCXXForRangeStmt`
/// arguments).
struct Parts {
    init: Option<StmtId>,
    range: DeclId,
    loop_var: DeclId,
    colon: Loc,
    loc: Loc,
    temps: Vec<ExprId>,
    depth: usize,
    /// clang `BFRK_Build`: the array-parameter and dereference recoveries
    /// apply (not while checking the dereferenced range).
    build: bool,
}

impl Sema<'_, '_> {
    // -- scope depth ----------------------------------------------------------

    /// clang's `Scope::getDepth()` of the innermost scope: the
    /// translation unit is 0. The scopes re-entered for a member
    /// function defined out of its class's scope do not count.
    pub(crate) fn clang_scope_depth(&self) -> usize {
        let pushed: usize = self.fn_saved.iter().map(|s| s.pushed).sum();
        self.scopes.len().saturating_sub(1 + pushed)
    }

    // -- P2718 collection -----------------------------------------------------

    /// Starts parsing a for-range-initializer: with `extend` (C++23,
    /// P2718R0) every temporary materialized in it is collected for
    /// extension to `__range` (clang `InLifetimeExtendingContext`).
    pub fn begin_for_range_init(&mut self, extend: bool) -> RangeCollect {
        RangeCollect(std::mem::replace(&mut self.extend_collector, extend.then(Vec::new)))
    }

    /// Ends a for-range-initializer: the temporaries collected since the
    /// matching [`Sema::begin_for_range_init`].
    pub fn end_for_range_init(&mut self, saved: RangeCollect) -> Vec<ExprId> {
        std::mem::replace(&mut self.extend_collector, saved.0).unwrap_or_default()
    }

    // -- diagnostics capture ----------------------------------------------------

    /// Runs `f` with its diagnostics stash back; returns them in order.
    pub(crate) fn capturing<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> (R, Vec<Diag>) {
        let old = self.diag_capture.replace(Some((self.inst.stack.len(), Vec::new())));
        let r = f(self);
        let stash = self.diag_capture.replace(old).map(|c| c.1).unwrap_or_default();
        (r, stash)
    }

    /// Emits `stash`, the last with `notes` attached (clang's standalone
    /// notes continue the diagnostic before them).
    pub(crate) fn release(&self, mut stash: Vec<Diag>, notes: Vec<Diag>) {
        if let Some(mut last) = stash.pop() {
            for n in notes {
                last = last.with_note(n);
            }
            stash.push(last);
        }
        for d in stash {
            self.emit(d);
        }
    }

    // -- the for-range-declaration --------------------------------------------

    /// clang `ActOnCXXForRangeDecl` and the single-declaration check of
    /// `ActOnCXXForRangeStmt`: `v`, declared by `ds`, is a
    /// for-range-declaration. False if the statement is to be dropped
    /// (a type is defined in it).
    pub fn act_on_for_range_decl(&mut self, ds: &DeclSpec, v: DeclId) -> bool {
        if self.kind_of(v) == DeclKind::Var {
            self.ast.decls.mark_var(v, VarSpec::new().with_cxx_for_range(true));
            // [stmt.ranged]/2: no storage class.
            let n = if ds.thread_local.is_some() {
                Some(6)
            } else {
                match ds.storage.map(|(s, _)| s) {
                    Some(StorageSpec::Extern) => Some(0),
                    Some(StorageSpec::Static) => Some(1),
                    Some(StorageSpec::Register) => Some(4),
                    _ => None,
                }
            };
            if let Some(n) = n {
                let msg = self
                    .diag(DiagId::sema_for_range_storage_class, ds.loc)
                    .arg(self.decl_name(v))
                    .arg(n);
                self.emit(msg);
                self.ast.decls.set_flags(v, DeclFlags::INVALID);
            }
        } else {
            let loc = self.ast.decls.get(v).loc();
            self.error(DiagId::sema_for_range_decl_must_be_var, loc);
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
        }
        if ds.tag_declares && ds.tag.is_some() {
            // The declaration group holds the class too (clang:
            // `!DS->isSingleDecl()`).
            self.error(DiagId::sema_type_defined_in_for_range, ds.loc);
            return false;
        }
        true
    }

    /// clang `ActOnInitializerError` for the loop variable `v` of a
    /// range-based `for` that is dropped.
    pub fn act_on_for_range_error(&mut self, v: DeclId) {
        self.for_range_error_var(v);
        // A decomposition's bindings are not decomposed: uses of them
        // raise no further errors.
        if self.ast.decls.decomp_bindings(v).is_some() {
            self.invalidate_bindings(v);
        }
    }

    fn for_range_error_var(&mut self, v: DeclId) {
        let n = *self.ast.decls.get(v);
        if n.kind() != DeclKind::Var || n.has(DeclFlags::INVALID) {
            return;
        }
        let ty = n.ty().unwrap_or(QualType::ERROR);
        if self.contains_undeduced_auto(ty) {
            self.ast.decls.set_ty(v, QualType::ERROR);
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
            return;
        }
        if self.is_dependent_type(ty) {
            return;
        }
        let mut base = ty;
        while let Some(e) = self.ast.types.element(base) {
            base = e;
        }
        if !self.require_complete(base, n.loc()) {
            let msg = self
                .diag(DiagId::sema_typecheck_decl_incomplete_type, n.loc())
                .arg(self.ty_arg(base));
            let msg = self.incomplete_note(msg, base);
            self.emit(msg);
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
        }
    }

    // -- the statement ----------------------------------------------------------

    /// clang `ActOnCXXForRangeStmt`: `for ( init loop_var : range )`
    /// at `loc`, `colon` the `:`; `temps` the temporaries collected in
    /// `range` (P2718). `None` after an error (the body is still parsed).
    pub fn act_on_for_range(
        &mut self,
        init: Option<StmtId>,
        loop_var: DeclId,
        range: RangeInit,
        temps: Vec<ExprId>,
        colon: Loc,
        loc: Loc,
    ) -> Option<RangeForHead> {
        let depth = self.clang_scope_depth() / 2;
        let parts =
            Parts { init, range: loop_var, loop_var, colon, loc, temps, depth, build: true };
        let name = self.intern(&format!("__range{depth}"));
        self.for_range_from_init(parts, range, name)
    }

    /// Declares `__range` initialized by `range`, then builds the
    /// statement (`parts.range` is replaced).
    fn for_range_from_init(
        &mut self,
        mut parts: Parts,
        range: RangeInit,
        name: Symbol,
    ) -> Option<RangeForHead> {
        let loop_var = parts.loop_var;
        // [temp.variadic]/12: clang's `ActOnCXXForRangeStmt` checks the
        // range as an "expression" at its begin (a braced list's `{`).
        let unexpanded = match &range {
            RangeInit::Expr(e) => self.diagnose_unexpanded_written(*e, "expression", None),
            RangeInit::List(l) => Self::list_exprs(l)
                .into_iter()
                .any(|e| self.diagnose_unexpanded_written(e, "expression", Some(l.lbrace))),
        };
        if unexpanded || self.ast.decls.get(loop_var).has(DeclFlags::INVALID) {
            self.discard_range_init(range);
            self.act_on_for_range_error(loop_var);
            return None;
        }
        let range_loc = match &range {
            RangeInit::Expr(e) => self.begin_loc(*e),
            RangeInit::List(l) => l.lbrace,
        };
        let auto = self.ast.types.auto_type(AutoKeyword::Auto, None, None, None, false);
        let rref = self.ast.types.rvalue_ref_to(auto);
        let range_var = self.implicit_range_var(name, rref, range_loc);
        let init = match range {
            // `auto&& __range` is deduced from, and bound to, a range of
            // unknown bound completed by its definition (WP-907).
            RangeInit::Expr(e) => Initializer::Expr(self.complete_expr_array_bound(e)),
            RangeInit::List(l) => Initializer::List(l),
        };
        if self.finish_range_var(
            range_var,
            init,
            range_loc,
            DiagId::sema_for_range_deduction_failure,
        ) {
            self.act_on_for_range_error(loop_var);
            return None;
        }
        parts.range = range_var;
        let head = self.build_for_range(&parts);
        if head.is_none() {
            self.act_on_for_range_error(loop_var);
        }
        head
    }

    /// The parts of a for-range-initializer not used after an error,
    /// ended as full-expressions (their cleanups do not leak).
    fn discard_range_init(&mut self, range: RangeInit) {
        let es = match range {
            RangeInit::Expr(e) => vec![e],
            RangeInit::List(l) => Self::list_exprs(&l),
        };
        let outer = std::mem::replace(&mut self.discarding_init, true);
        for e in es {
            let _ = self.act_on_full_expr(e);
        }
        self.discarding_init = outer;
    }

    /// clang `BuildForRangeVarDecl`: an implicit for-range variable.
    fn implicit_range_var(&mut self, sym: Symbol, ty: QualType, loc: Loc) -> DeclId {
        let mut h = DeclHead::new(self.ctx, DeclName::ident(sym), loc);
        h.flags = h.flags.with(DeclFlags::IMPLICIT);
        let spec = VarSpec::new().with_cxx_for_range(true).with_init_style(InitStyle::C);
        self.builder().var(h, ty, spec, None)
    }

    /// clang `FinishForRangeVarDecl`: deduces the type of the implicit
    /// variable `v` from `init` (diagnosing a failure with `id` at
    /// `loc`) and initializes it. True on failure (`v` is invalid).
    pub(crate) fn finish_range_var(
        &mut self,
        v: DeclId,
        init: Initializer,
        loc: Loc,
        id: DiagId,
    ) -> bool {
        let declared = self.ast.decls.get(v).ty().unwrap_or(QualType::ERROR);
        let (items, kind, init_ty) = match &init {
            Initializer::Expr(e) => (vec![Some(*e)], AutoInit::Expr, self.ety(*e)),
            Initializer::List(l) => {
                (crate::templ::auto::auto_list_items(l, true), AutoInit::CopyList, QualType::VOID)
            }
            Initializer::Paren(..) => (Vec::new(), AutoInit::Paren, QualType::ERROR),
        };
        let is_list = matches!(init, Initializer::List(_));
        let deduced =
            if (!is_list && self.ast.types.is_void(init_ty)) || init_ty == QualType::OVERLOAD {
                let msg = self.diag(id, loc).arg(self.ty_arg(init_ty));
                self.emit(msg);
                None
            } else {
                match self.deduce_auto(declared, &items, kind) {
                    Ok(t) => Some(t),
                    Err(AutoFail::Error) => None,
                    Err(AutoFail::NoInitializerList) => {
                        let at = match &init {
                            Initializer::List(l) => l.lbrace,
                            _ => loc,
                        };
                        self.error(DiagId::sema_auto_init_list_deduction, at);
                        None
                    }
                    Err(_) => {
                        let msg = self.diag(id, loc).arg(self.ty_arg(init_ty));
                        self.emit(msg);
                        None
                    }
                }
            };
        let Some(t) = deduced else {
            self.ast.decls.set_ty(v, QualType::ERROR);
            self.ast.decls.set_flags(v, DeclFlags::INVALID);
            return true;
        };
        self.ast.decls.set_ty(v, t);
        self.act_on_init(v, init);
        false
    }

    /// clang `BuildCXXForRangeStmt` for a statement without `__begin`
    /// and `__end` yet: `parts.range` is the initialized `__range`.
    #[allow(
        clippy::too_many_lines,
        reason = "one step per statement of clang's BuildCXXForRangeStmt"
    )]
    fn build_for_range(&mut self, parts: &Parts) -> Option<RangeForHead> {
        let range_var = parts.range;
        let loop_var = parts.loop_var;
        let colon = parts.colon;
        let range_var_ty = self.ast.decls.get(range_var).ty().unwrap_or(QualType::ERROR);
        let mut head = RangeForHead {
            init: parts.init,
            range: range_var,
            begin: None,
            end: None,
            cond: None,
            inc: None,
            loop_var,
            colon,
            loc: parts.loc,
        };
        if self.is_dependent_type(range_var_ty) {
            // The range is implicitly used as a placeholder; the loop
            // variable's `auto` is deduced at instantiation.
            self.ast.decls.set_flags(range_var, DeclFlags::USED);
            let n = *self.ast.decls.get(loop_var);
            if !n.has(DeclFlags::INVALID)
                && let Some(t) = n.ty()
                && self.contains_undeduced_auto(t)
            {
                let d = self.dependent_auto(t);
                self.ast.decls.set_ty(loop_var, d);
            }
            if self.ast.decls.decomp_bindings(loop_var).is_some() {
                self.complete_decomposition(loop_var);
            }
            return Some(head);
        }
        let range_loc = self.ast.decls.get(range_var).loc();
        let range_nonref = self.non_reference(range_var_ty);
        let begin_range = self.build_decl_ref(range_var, colon);
        let end_range = self.build_decl_ref(range_var, colon);
        let range_init = self.ast.decls.var(range_var).and_then(|x| x.init)?;
        let range_ty = self.ety(range_init);
        if !self.require_complete(range_ty, range_loc) {
            let msg = self
                .diag(DiagId::sema_for_range_incomplete_type, range_loc)
                .arg(self.ty_arg(range_ty));
            let msg = self.incomplete_note(msg, range_ty);
            self.emit(msg);
            return None;
        }
        // P2718R0: the temporaries of the for-range-initializer live as
        // long as `__range` ([class.temporary]/6.4, spec/sema-class.md
        // §5.4).
        for (i, &mte) in parts.temps.iter().enumerate() {
            if self.ast.exprs.get(mte).kind() != ExprKind::MaterializeTemporaryExpr
                || self.ast.exprs.lifetime_extended_temporary(mte).is_some()
            {
                continue;
            }
            let n = *self.ast.exprs.get(mte);
            let index = u32::try_from(i).unwrap_or(u32::MAX);
            let let_decl =
                self.builder().lifetime_extended_temporary(range_var, index, n.ty(), n.loc());
            let _ = self.ast.exprs.extend_temporary(mte, let_decl, StorageDuration::Automatic);
        }
        let auto = self.ast.types.auto_type(AutoKeyword::Auto, None, None, None, false);
        let begin_sym = self.intern(&format!("__begin{}", parts.depth));
        let end_sym = self.intern(&format!("__end{}", parts.depth));
        let begin_var = self.implicit_range_var(begin_sym, auto, colon);
        let end_var = self.implicit_range_var(end_sym, auto, colon);
        let (begin_expr, end_expr) = if self.ast.types.is_array(range_ty) {
            // [stmt.ranged]/1.1: `__range` and `__range + __bound`.
            let (failed, stash) = self.capturing(|s| {
                s.finish_range_var(
                    begin_var,
                    Initializer::Expr(begin_range),
                    colon,
                    DiagId::sema_for_range_iter_deduction_failure,
                )
            });
            if failed {
                let notes = self.begin_end_note(begin_range, Bef::Begin);
                self.release(stash, notes);
                return None;
            }
            self.release(stash, Vec::new());
            let Some(bound) = self.ast.types.array_size(range_ty) else {
                self.unsupported("range-based for over a variable length array", range_loc);
                return None;
            };
            let pd = QualType::unqual(self.target.ptrdiff_t);
            let lit = self.builder().int_literal(pd, bound, Radix::Decimal, range_loc);
            let end = self.act_on_binary(BinaryOp::Add, end_range, lit, colon);
            if self.is_err_expr(end) {
                return None;
            }
            let (failed, stash) = self.capturing(|s| {
                s.finish_range_var(
                    end_var,
                    Initializer::Expr(end),
                    colon,
                    DiagId::sema_for_range_iter_deduction_failure,
                )
            });
            if failed {
                let notes = self.begin_end_note(end, Bef::End);
                self.release(stash, notes);
                return None;
            }
            self.release(stash, Vec::new());
            (begin_range, end)
        } else {
            let (status, bef, exprs) = self.build_non_array_range(
                begin_range,
                end_range,
                range_ty,
                begin_var,
                end_var,
                colon,
            );
            if let Status::NoViable(notes) = status {
                if parts.build && bef == Bef::Begin {
                    if self.range_on_array_parameter(range_init) {
                        return None;
                    }
                    if let Some(h) = self.rebuild_with_dereference(parts, range_init, range_loc) {
                        return Some(h);
                    }
                }
                let at = if bef == Bef::End { end_range } else { begin_range };
                let mut msg = self
                    .diag(DiagId::sema_for_range_invalid, self.begin_loc(at))
                    .arg(self.ty_arg(self.ety(at)))
                    .arg(bef.index());
                for n in notes {
                    msg = msg.with_note(n);
                }
                self.emit(msg);
                return None;
            }
            match (status, exprs) {
                (Status::Success, Some(x)) => x,
                _ => return None,
            }
        };
        let begin_ty = self.ast.decls.get(begin_var).ty().unwrap_or(QualType::ERROR);
        let end_ty = self.ast.decls.get(end_var).ty().unwrap_or(QualType::ERROR);
        let same = self.ast.types.canonical(begin_ty) == self.ast.types.canonical(end_ty);
        head.begin = Some(begin_var);
        head.end = Some(end_var);
        // `__begin != __end`, contextually converted to `bool`.
        let (cond, stash) = self.capturing(|s| {
            let b = s.build_decl_ref(begin_var, colon);
            let e = s.build_decl_ref(end_var, colon);
            let ne = s.act_on_binary(BinaryOp::NE, b, e, colon);
            if s.is_err_expr(ne) {
                return None;
            }
            match crate::conv::contextually_convert_to_bool(s, ne) {
                Ok(c) => Some(s.act_on_full_expr(c)),
                Err(err) => {
                    s.emit(err.diag_at(s, colon));
                    None
                }
            }
        });
        let Some(cond) = cond else {
            let mut notes = vec![self.invalid_iterator_note(range_loc, 0, range_nonref)];
            notes.extend(self.begin_end_note(begin_expr, Bef::Begin));
            if !same {
                notes.extend(self.begin_end_note(end_expr, Bef::End));
            }
            self.release(stash, notes);
            return None;
        };
        self.release(stash, Vec::new());
        // `++__begin`.
        let (inc, stash) = self.capturing(|s| {
            let b = s.build_decl_ref(begin_var, colon);
            let inc = s.act_on_unary(UnaryOp::PreInc, b, colon);
            if s.is_err_expr(inc) { None } else { Some(s.act_on_full_expr(inc)) }
        });
        let Some(inc) = inc else {
            let mut notes = vec![self.invalid_iterator_note(range_loc, 2, range_nonref)];
            notes.extend(self.begin_end_note(begin_expr, Bef::Begin));
            self.release(stash, notes);
            return None;
        };
        self.release(stash, Vec::new());
        // `*__begin`.
        let (deref, stash) = self.capturing(|s| {
            let b = s.build_decl_ref(begin_var, colon);
            let d = s.act_on_unary(UnaryOp::Deref, b, colon);
            (!s.is_err_expr(d)).then_some(d)
        });
        let Some(deref) = deref else {
            let mut notes = vec![self.invalid_iterator_note(range_loc, 1, range_nonref)];
            notes.extend(self.begin_end_note(begin_expr, Bef::Begin));
            self.release(stash, notes);
            return None;
        };
        self.release(stash, Vec::new());
        head.cond = Some(cond);
        head.inc = Some(inc);
        if !self.ast.decls.get(loop_var).has(DeclFlags::INVALID) {
            let ((), stash) = self.capturing(|s| s.act_on_init(loop_var, Initializer::Expr(deref)));
            let bad = self.ast.decls.get(loop_var).has(DeclFlags::INVALID)
                || self
                    .ast
                    .decls
                    .var(loop_var)
                    .and_then(|x| x.init)
                    .is_some_and(|e| self.has_errors_in(e));
            let notes = if bad { self.begin_end_note(begin_expr, Bef::Begin) } else { Vec::new() };
            self.release(stash, notes);
        }
        Some(head)
    }

    /// clang `note_for_range_invalid_iterator`.
    fn invalid_iterator_note(&self, at: Loc, op: u8, range_ty: QualType) -> Diag {
        self.diag(DiagId::note_for_range_invalid_iterator, at).arg(op).arg(self.ty_arg(range_ty))
    }

    /// clang `NoteForRangeBeginEndFunction`: the `begin`/`end` function
    /// a call `e` selected.
    fn begin_end_note(&self, e: ExprId, bef: Bef) -> Vec<Diag> {
        let kind = self.ast.exprs.get(e).kind();
        if !matches!(kind, ExprKind::CallExpr | ExprKind::CXXMemberCallExpr) {
            return Vec::new();
        }
        let callee = self.ignore_paren_imp_casts(self.ast.exprs.callee(e));
        if !matches!(
            self.ast.exprs.get(callee).kind(),
            ExprKind::DeclRefExpr | ExprKind::MemberExpr
        ) {
            return Vec::new();
        }
        let f = self.ast.exprs.decl(callee);
        if !matches!(
            self.kind_of(f),
            DeclKind::Function
                | DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion
        ) {
            return Vec::new();
        }
        let (template, desc) = match self.ast.spec_info(f) {
            Some((_, info)) => (1u8, self.template_args_desc(info.template, info.args)),
            None => (0u8, String::new()),
        };
        vec![
            self.diag(DiagId::note_for_range_begin_end, self.ast.decls.get(f).loc())
                .arg(bef.index())
                .arg(template)
                .arg(desc)
                .arg(self.ty_arg(self.ety(e))),
        ]
    }

    /// The `begin` and `end` variables of an iterating expansion
    /// statement ([stmt.expand]/5.2): `begin_var` and `end_var` (of a
    /// deduced type) are initialized from the begin-expr and end-expr
    /// [stmt.ranged]/1.3 forms for a non-array range of type `range_ty`
    /// (named by `begin_range` and `end_range`), diagnosed as for a
    /// range-based `for`. False after an error.
    pub(crate) fn expansion_begin_end(
        &mut self,
        begin_range: ExprId,
        end_range: ExprId,
        range_ty: QualType,
        vars: (DeclId, DeclId),
        colon: Loc,
    ) -> bool {
        let (status, bef, exprs) =
            self.build_non_array_range(begin_range, end_range, range_ty, vars.0, vars.1, colon);
        match status {
            Status::Success => exprs.is_some(),
            Status::NoViable(notes) => {
                let at = if bef == Bef::End { end_range } else { begin_range };
                let mut msg = self
                    .diag(DiagId::sema_for_range_invalid, self.begin_loc(at))
                    .arg(self.ty_arg(self.ety(at)))
                    .arg(bef.index());
                for n in notes {
                    msg = msg.with_note(n);
                }
                self.emit(msg);
                false
            }
            Status::DiagnosticIssued => false,
        }
    }

    /// Whether `range` is expansion-iterable ([stmt.expand]/3, P1306R5):
    /// not an array, and either a class with a member `begin` or `end`
    /// (the member form), or `begin(range)` and `end(range)` each have a
    /// viable candidate found by argument-dependent lookup (silently).
    pub(crate) fn expansion_iterable(&mut self, range: ExprId, colon: Loc) -> bool {
        let ty = self.non_reference(self.ety(range));
        if self.ast.types.is_array(ty) {
            return false;
        }
        if let Some(rec) = self.ast.types.as_record(ty)
            && self.kind_of(rec) == DeclKind::CXXRecord
        {
            let class = self.ast.definition(rec).unwrap_or(rec);
            for name in ["begin", "end"] {
                let sym = self.intern(name);
                if !self.lookup_in_class(class, sym, LookupKind::Ordinary).decls.is_empty() {
                    return true;
                }
            }
        }
        let ((b, e), _) = self.capturing(|s| {
            let b = s.begin_end_call(Bef::Begin, false, range, colon).0;
            let e = s.begin_end_call(Bef::End, false, range, colon).0;
            (b, e)
        });
        matches!((b, e), (Status::Success, Status::Success))
    }

    /// clang `BuildNonArrayForRange`: `begin-expr` and `end-expr` for a
    /// class or non-array range ([stmt.ranged]/1.2-1.3). Returns the
    /// status, the function the last attempt was for, and the
    /// expressions on success.
    #[allow(clippy::type_complexity, reason = "clang's out-parameters")]
    fn build_non_array_range(
        &mut self,
        begin_range: ExprId,
        end_range: ExprId,
        range_ty: QualType,
        begin_var: DeclId,
        end_var: DeclId,
        colon: Loc,
    ) -> (Status, Bef, Option<(ExprId, ExprId)>) {
        let begin_sym = self.intern("begin");
        let end_sym = self.intern("end");
        let mut members: (Option<Found>, Option<Found>) = (None, None);
        if let Some(rec) = self.ast.types.as_record(range_ty)
            && self.kind_of(rec) == DeclKind::CXXRecord
        {
            let class = self.ast.definition(rec).unwrap_or(rec);
            let b = self.lookup_in_class(class, begin_sym, LookupKind::Ordinary);
            if self.diagnose_member_ambiguity(&b, DeclName::ident(begin_sym), colon) {
                return (Status::DiagnosticIssued, Bef::Begin, None);
            }
            let e = self.lookup_in_class(class, end_sym, LookupKind::Ordinary);
            if self.diagnose_member_ambiguity(&e, DeclName::ident(end_sym), colon) {
                return (Status::DiagnosticIssued, Bef::Begin, None);
            }
            members = ((!b.decls.is_empty()).then_some(b), (!e.decls.is_empty()).then_some(e));
        }
        let ctx = BeCtx { begin_range, end_range, begin_var, end_var, colon };
        match members {
            (Some(found), None) => self.build_nonmember(&ctx, Bef::Begin, &found, range_ty),
            (None, Some(found)) => self.build_nonmember(&ctx, Bef::End, &found, range_ty),
            (b, e) => {
                let (st, be) = self.build_begin_end(&ctx, Bef::Begin, b.is_some());
                let Status::Success = st else { return (st, Bef::Begin, None) };
                let (st, ee) = self.build_begin_end(&ctx, Bef::End, e.is_some());
                let Status::Success = st else { return (st, Bef::End, None) };
                match (be, ee) {
                    (Some(b), Some(e)) => (Status::Success, Bef::End, Some((b, e))),
                    _ => (Status::DiagnosticIssued, Bef::End, None),
                }
            }
        }
    }

    /// clang's `BuildNonmember`: only `found_bef` was found as a member;
    /// both are looked up by ADL, the missing one first.
    fn build_nonmember(
        &mut self,
        ctx: &BeCtx,
        found_bef: Bef,
        found: &Found,
        range_ty: QualType,
    ) -> (Status, Bef, Option<(ExprId, ExprId)>) {
        let other = if found_bef == Bef::Begin { Bef::End } else { Bef::Begin };
        let (st, x) = self.build_begin_end(ctx, other, false);
        let Status::Success = st else { return (st, other, None) };
        let ignored = |s: &Self| -> Vec<Diag> {
            found
                .decls
                .iter()
                .map(|&d| {
                    s.diag(
                        DiagId::note_for_range_member_begin_end_ignored,
                        s.ast.decls.get(d).loc(),
                    )
                    .arg(s.ty_arg(s.ety(ctx.begin_range)))
                    .arg(found_bef.index())
                })
                .collect()
        };
        let ((st, y), stash) = self.capturing(|s| s.build_begin_end(ctx, found_bef, false));
        match st {
            Status::Success => {
                self.release(stash, Vec::new());
                let (Some(x), Some(y)) = (x, y) else {
                    return (Status::DiagnosticIssued, found_bef, None);
                };
                let pair = if found_bef == Bef::Begin { (y, x) } else { (x, y) };
                (Status::Success, found_bef, Some(pair))
            }
            Status::NoViable(notes) => {
                self.release(stash, Vec::new());
                let _ = range_ty;
                let mut msg = self
                    .diag(DiagId::sema_for_range_invalid, self.begin_loc(ctx.begin_range))
                    .arg(self.ty_arg(self.ety(ctx.begin_range)))
                    .arg(found_bef.index());
                for n in notes {
                    msg = msg.with_note(n);
                }
                for n in ignored(self) {
                    msg = msg.with_note(n);
                }
                self.emit(msg);
                (Status::DiagnosticIssued, found_bef, None)
            }
            Status::DiagnosticIssued => {
                let notes = ignored(self);
                self.release(stash, notes);
                (Status::DiagnosticIssued, found_bef, None)
            }
        }
    }

    /// clang's `BuildBegin`/`BuildEnd`: the call (a member call if
    /// `member`) and its `__begin`/`__end` variable.
    fn build_begin_end(&mut self, ctx: &BeCtx, bef: Bef, member: bool) -> (Status, Option<ExprId>) {
        let (range, var) = match bef {
            Bef::Begin => (ctx.begin_range, ctx.begin_var),
            Bef::End => (ctx.end_range, ctx.end_var),
        };
        let ((st, call), stash) =
            self.capturing(|s| s.begin_end_call(bef, member, range, ctx.colon));
        match st {
            Status::Success => self.release(stash, Vec::new()),
            Status::DiagnosticIssued => {
                let note = self
                    .diag(DiagId::note_in_for_range, self.begin_loc(range))
                    .arg(bef.index())
                    .arg(self.ty_arg(self.ety(range)));
                self.release(stash, vec![note]);
                return (Status::DiagnosticIssued, None);
            }
            Status::NoViable(n) => {
                self.release(stash, Vec::new());
                return (Status::NoViable(n), None);
            }
        }
        let Some(call) = call else { return (Status::DiagnosticIssued, None) };
        let (failed, stash) = self.capturing(|s| {
            s.finish_range_var(
                var,
                Initializer::Expr(call),
                ctx.colon,
                DiagId::sema_for_range_iter_deduction_failure,
            )
        });
        if failed {
            let notes = self.begin_end_note(call, bef);
            self.release(stash, notes);
            return (Status::DiagnosticIssued, None);
        }
        self.release(stash, Vec::new());
        (Status::Success, Some(call))
    }

    /// clang `BuildForRangeBeginEndCall`: `range.begin()` (with `member`)
    /// or `begin(range)` found by argument-dependent lookup only.
    fn begin_end_call(
        &mut self,
        bef: Bef,
        member: bool,
        range: ExprId,
        colon: Loc,
    ) -> (Status, Option<ExprId>) {
        let sym = self.intern(bef.name());
        if member {
            let m = self.act_on_member(range, sym, colon, colon, false);
            if self.is_err_expr(m) {
                return (Status::DiagnosticIssued, None);
            }
            let call = self.act_on_call(m, &[], colon, colon);
            if self.is_err_expr(call) {
                return (Status::DiagnosticIssued, None);
            }
            return (Status::Success, Some(call));
        }
        let mut cands = Vec::new();
        self.adl_lookup_dn(DeclName::ident(sym), &[range], colon, &mut cands);
        cands.retain(|&(d, _)| !self.ast.decls.get(d).has(DeclFlags::INVALID));
        if cands.is_empty() {
            return (Status::NoViable(Vec::new()), None);
        }
        let mut set = self.begin_set(SetKind::Normal, colon);
        self.defer_templates(&mut set);
        let base = self.push_call_args(&mut set, &[range], &[None]);
        for &(d, f) in &cands {
            if self.kind_of(d) == DeclKind::FunctionTemplate {
                if set.two_phase {
                    set.deferred.push(crate::overload::defer::Deferred::Template {
                        ft: d,
                        found: f,
                        kind: CandidateKind::Function,
                        first_opts: IcsOptions::NONE,
                        loc: colon,
                        poi: self.templ.deduce_poi,
                        adl_only: true,
                    });
                    continue;
                }
                self.add_template_candidate(
                    &mut set,
                    d,
                    f,
                    None,
                    CandidateKind::Function,
                    IcsOptions::NONE,
                    colon,
                );
            } else {
                self.add_candidate(&mut set, d, f, CandidateKind::Function, IcsOptions::NONE);
            }
            if let Some(c) = set.cands.last_mut() {
                c.adl_only = true;
            }
        }
        let out = self.resolve_set(&mut set);
        self.ovl.lists.truncate(base);
        if matches!(out, OvlOutcome::NoViable) {
            let notes = self.ovl_notes(&set, false);
            self.end_set(set);
            return (Status::NoViable(notes), None);
        }
        self.end_set(set);
        let ule = self.build_overload_ref(DeclName::ident(sym), &[], &[], None, true, None, colon);
        let call = self.act_on_call(ule, &[range], colon, colon);
        if self.is_err_expr(call) {
            return (Status::DiagnosticIssued, None);
        }
        (Status::Success, Some(call))
    }

    /// clang's `err_range_on_array_parameter`: the range names a function
    /// parameter declared with array type. True when diagnosed.
    fn range_on_array_parameter(&mut self, range_init: ExprId) -> bool {
        if self.ast.exprs.get(range_init).kind() != ExprKind::DeclRefExpr {
            return false;
        }
        let p = self.ast.exprs.decl(range_init);
        if self.kind_of(p) != DeclKind::ParmVar {
            return false;
        }
        let Some(array_ty) = self.ast.decls.param(p).and_then(|x| x.original_ty) else {
            return false;
        };
        let Some(ptr_ty) = self.ast.decls.get(p).ty() else { return false };
        if !self.ast.types.is_pointer(ptr_ty) || !self.ast.types.is_array(array_ty) {
            return false;
        }
        let msg = self
            .diag(DiagId::sema_range_on_array_parameter, self.begin_loc(range_init))
            .arg(self.decl_name(p))
            .arg(self.ty_arg(array_ty))
            .arg(self.ty_arg(ptr_ty))
            .with_note(self.diag(DiagId::note_declared_at, self.ast.decls.get(p).loc()));
        self.emit(msg);
        true
    }

    /// clang `RebuildForRangeWithDereference`: if `*range` makes a valid
    /// loop, diagnose the missing `*` and use it.
    fn rebuild_with_dereference(
        &mut self,
        parts: &Parts,
        range_init: ExprId,
        range_loc: Loc,
    ) -> Option<RangeForHead> {
        let range_ty = self.ety(range_init);
        let (head, stash) = self.capturing(|s| {
            let adjusted = s.act_on_unary(UnaryOp::Deref, range_init, range_loc);
            if s.is_err_expr(adjusted) {
                return None;
            }
            let retry = Parts {
                init: parts.init,
                range: parts.range,
                loop_var: parts.loop_var,
                colon: parts.colon,
                loc: parts.loc,
                temps: Vec::new(),
                depth: parts.depth,
                build: false,
            };
            s.for_range_from_init_unchecked(retry, adjusted)
        });
        let head = head?;
        let msg =
            self.diag(DiagId::sema_for_range_dereference, range_loc).arg(self.ty_arg(range_ty));
        self.emit(msg);
        self.release(stash, Vec::new());
        Some(head)
    }

    /// [`Sema::for_range_from_init`] for the dereferenced range of
    /// [`Sema::rebuild_with_dereference`] (the loop variable is left as
    /// is on failure).
    fn for_range_from_init_unchecked(
        &mut self,
        mut parts: Parts,
        e: ExprId,
    ) -> Option<RangeForHead> {
        let range_loc = self.begin_loc(e);
        let auto = self.ast.types.auto_type(AutoKeyword::Auto, None, None, None, false);
        let rref = self.ast.types.rvalue_ref_to(auto);
        let name = self.ast.decls.get(parts.range).name().as_ident();
        let name = name.unwrap_or_else(|| self.intern("__range"));
        let range_var = self.implicit_range_var(name, rref, range_loc);
        if self.finish_range_var(
            range_var,
            Initializer::Expr(e),
            range_loc,
            DiagId::sema_for_range_deduction_failure,
        ) {
            return None;
        }
        parts.range = range_var;
        self.build_for_range(&parts)
    }

    /// clang `FinishCXXForRangeStmt`: the statement with its body.
    pub fn act_on_finish_for_range(&mut self, head: RangeForHead, body: StmtId) -> StmtId {
        let data = RangeForData {
            init: head.init,
            range: head.range,
            begin: head.begin,
            end: head.end,
            cond: head.cond,
            inc: head.inc,
            loop_var: head.loop_var,
            body,
            colon: head.colon,
        };
        self.builder().cxx_for_range_stmt(data, head.loc)
    }

    /// Instantiates a range-based `for` ([temp.inst]; clang
    /// `TransformCXXForRangeStmt`): `range` is the substituted
    /// for-range-initializer, `range_name` the pattern's `__range`
    /// name, `loop_var` the instantiated loop variable (not initialized)
    /// and `depth` the number of the `__begin`/`__end` variables.
    #[allow(clippy::too_many_arguments, reason = "the statement's slots")]
    pub(crate) fn instantiate_for_range(
        &mut self,
        init: Option<StmtId>,
        range_name: Symbol,
        range: RangeInit,
        temps: Vec<ExprId>,
        loop_var: DeclId,
        colon: Loc,
        loc: Loc,
        depth: usize,
    ) -> Option<RangeForHead> {
        let parts =
            Parts { init, range: loop_var, loop_var, colon, loc, temps, depth, build: true };
        self.for_range_from_init(parts, range, range_name)
    }

    /// The depth for `__begin`/`__end` of an instantiation at the
    /// current point (clang numbers them from the scope depth where the
    /// instantiation's `BuildCXXForRangeStmt` runs).
    pub(crate) fn range_for_inst_depth(&self) -> usize {
        self.range_for_outer_depth.unwrap_or(0) / 2
    }
}

/// The shared inputs of `BuildBegin`/`BuildEnd`.
struct BeCtx {
    begin_range: ExprId,
    end_range: ExprId,
    begin_var: DeclId,
    end_var: DeclId,
    colon: Loc,
}
