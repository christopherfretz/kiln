//! The address of an overloaded function ([over.over];
//! spec/sema-overload.md §2.9): an `UnresolvedLookupExpr` (possibly
//! parenthesized, possibly under `&`) used where a target type is known
//! resolves to the function whose type matches it; without a target it is
//! "reference to overloaded function could not be resolved".

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::{ExprKind, UnaryOp};
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::{Diag, DiagId};
use kiln_source::Loc;

use crate::Sema;

/// How a failed resolution is reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AddrCtx {
    /// An implicit conversion (initialization, assignment, argument,
    /// return, C-style cast): "does not match required type".
    Convert,
    /// `static_cast` / a cast to `void`: "cannot be {cast} to type"
    /// (the selector of `sema_addr_ovl_cast`: 0 `cast`, 1 `static_cast`).
    Cast(u8),
    /// `reinterpret_cast`: "`reinterpret_cast` cannot resolve overloaded
    /// function".
    Reinterpret,
}

impl Sema<'_, '_> {
    /// The `UnresolvedLookupExpr` an expression of overloaded-function
    /// type names (through parentheses and one `&`), and whether it is
    /// under `&`.
    pub(crate) fn overload_operand(&self, e: ExprId) -> Option<(ExprId, bool)> {
        if self.ast.exprs.get(e).ty() != QualType::OVERLOAD {
            return None;
        }
        let mut x = self.ignore_parens(e);
        let mut addr = false;
        if self.ast.exprs.get(x).kind() == ExprKind::UnaryOperator
            && self.ast.exprs.unary_op(x) == UnaryOp::AddrOf
        {
            addr = true;
            x = self.ignore_parens(self.ast.exprs.operand(x));
        }
        (self.ast.exprs.get(x).kind() == ExprKind::UnresolvedLookupExpr).then_some((x, addr))
    }

    /// The function type a target type asks an overload set for
    /// ([over.over]/1: the pointee of a pointer or reference to function
    /// or of a pointer to member function, or a function type), else the
    /// target itself; and whether it is a pointer to member.
    fn address_target(&self, target: QualType) -> (QualType, bool) {
        let t = &self.ast.types;
        let mut c = t.canonical(target);
        if let TypeView::LValueRef(p) | TypeView::RValueRef(p) = t.canonical_view(c) {
            c = t.canonical(p);
        }
        match t.canonical_view(c) {
            TypeView::Pointer(p) if t.is_function(p) => (t.canonical(p).without_quals(), false),
            TypeView::MemberPointer { pointee, .. } if t.is_function(pointee) => {
                (t.canonical(pointee).without_quals(), true)
            }
            _ => (c.without_quals(), false),
        }
    }

    /// The candidates of `ule` (function, found declaration), in
    /// candidate order, without invalid declarations.
    fn ule_candidates(&self, ule: ExprId) -> Vec<(DeclId, DeclId)> {
        let extra = self.ovl.ule_found.get(&ule);
        self.ast
            .unresolved_decls(ule)
            .iter()
            .enumerate()
            .map(|(i, &d)| (d, extra.and_then(|x| x.found.get(i).copied()).unwrap_or(d)))
            .filter(|&(d, _)| !self.ast.decls.get(d).has(DeclFlags::INVALID))
            .collect()
    }

    /// Whether the non-template function `d` matches the function type
    /// `fnty` ([over.over]/2, /5): equal, or equal after the function
    /// pointer conversion. Non-static members match only a pointer to
    /// member function, the others only a non-member target.
    fn address_matches(&mut self, d: DeclId, (fnty, member): (QualType, bool)) -> bool {
        if !matches!(self.kind_of(d), DeclKind::Function | DeclKind::CXXMethod)
            || self.is_instance_member(d) != member
        {
            return false;
        }
        if !self.ast.types.is_function(fnty) {
            return false;
        }
        let Some(t) = self.ast.decls.get(d).ty() else { return false };
        let t = self.ast.types.canonical(t);
        if self.ast.types.same_type(t, fnty) {
            return true;
        }
        crate::conv::drop_noexcept(self, t).is_some_and(|x| self.ast.types.same_type(x, fnty))
    }

    /// The matching non-template candidates of `ule` for `target`.
    fn address_candidates(&mut self, ule: ExprId, target: QualType) -> Vec<(DeclId, DeclId)> {
        let fnty = self.address_target(target);
        let cands = self.ule_candidates(ule);
        cands.into_iter().filter(|&(d, _)| self.address_matches(d, fnty)).collect()
    }

    /// The function an expression of overloaded-function type `e`
    /// resolves to for `target`, silently (an implicit conversion
    /// sequence's query, [over.ics.scs]/… via [over.over]).
    pub(crate) fn address_match(&mut self, e: ExprId, target: QualType) -> Option<DeclId> {
        let (ule, _) = self.overload_operand(e)?;
        let m = self.address_candidates(ule, target);
        match m.as_slice() {
            [(d, _)] => Some(*d),
            _ => None,
        }
    }

    /// The one specialization the function template-id `ule` names
    /// without a call or a target type ([over.over]/3 via
    /// [temp.arg.explicit]/4): of its templates, exactly one has every
    /// template argument explicit or defaulted (clang's
    /// `ResolveSingleFunctionTemplateSpecialization`).
    fn single_template_spec(&mut self, ule: ExprId) -> Option<(DeclId, DeclId)> {
        let explicit = self.ast.templates.expr_targs.get(ule).copied()?;
        if self
            .ast
            .types
            .arg_list_flags(explicit)
            .intersects(kiln_ast::types::TypeFlags::DEPENDENT_INSTANTIATION)
        {
            return None;
        }
        let loc = self.ast.exprs.get(ule).loc();
        let mut found = None;
        for (d, f) in self.ule_candidates(ule) {
            if self.kind_of(d) != DeclKind::FunctionTemplate {
                return None;
            }
            let Ok(spec) = self.deduce_explicit_only(d, Some(explicit), loc) else { continue };
            if found.is_some() {
                return None;
            }
            found = Some((spec, f));
        }
        found
    }

    /// `e`, a function template-id not called: the `DeclRefExpr` naming
    /// the one specialization it names ([over.over]/3), else `e`.
    pub fn resolve_single_template_id(&mut self, e: ExprId) -> ExprId {
        if self.ast.exprs.get(e).kind() != ExprKind::UnresolvedLookupExpr {
            return e;
        }
        let Some((d, f)) = self.single_template_spec(e) else { return e };
        let naming = self.ovl.ule_found.get(&e).and_then(|x| x.naming);
        let nns = self.ast.templates.expr_nns.get(e).copied();
        let loc = self.ast.exprs.get(e).loc();
        let r = self.finish_overload_ref(d, f, naming, loc, nns, e, false);
        self.note_function_name_use(d, r, loc, nns.is_none());
        r
    }

    /// [over.over]: resolves the overload set `e` (a ULE, possibly in
    /// parentheses and under `&`) against `target`: the expression with
    /// the ULE replaced by a `DeclRefExpr` to the selected function, or
    /// `None` (after diagnosing when `complain`, as `ctx` says).
    pub(crate) fn resolve_address_of_overloaded(
        &mut self,
        e: ExprId,
        target: QualType,
        complain: bool,
        ctx: AddrCtx,
        at: Loc,
    ) -> Option<ExprId> {
        let Some((ule, _)) = self.overload_operand(e) else {
            if complain {
                self.check_overload_use(e);
            }
            return None;
        };
        let matches = self.address_candidates(ule, target);
        if let [(d, f)] = matches[..] {
            let naming = self.ovl.ule_found.get(&ule).and_then(|x| x.naming);
            let nns = self.ast.templates.expr_nns.get(ule).copied();
            let loc = self.ast.exprs.get(ule).loc();
            let r = self.finish_overload_ref(d, f, naming, loc, nns, ule, false);
            self.note_function_name_use(d, r, loc, nns.is_none());
            return Some(self.rebuild_over(e, ule, r));
        }
        if !complain {
            return None;
        }
        let cands = self.ule_candidates(ule);
        let name = self.ast.symbols().as_str(self.ast.unresolved_name(ule)).to_owned();
        let loc = self.begin_loc(ule);
        if matches.is_empty()
            && cands.iter().any(|&(d, _)| self.kind_of(d) == DeclKind::FunctionTemplate)
        {
            self.templ_unimplemented("the address of a function template", loc);
            return None;
        }
        if matches.len() > 1 {
            let msg = self.diag(DiagId::sema_addr_ovl_ambiguous, loc).arg(name);
            let notes: Vec<Diag> =
                matches.iter().map(|&(d, _)| self.plain_candidate_note(d)).collect();
            self.emit(notes.into_iter().fold(msg, Diag::with_note));
            return None;
        }
        let (fnty, _) = self.address_target(target);
        let (msg, detailed) = match ctx {
            AddrCtx::Convert => (
                self.diag(DiagId::sema_addr_ovl_no_match, loc).arg(name).arg(self.ty_arg(fnty)),
                true,
            ),
            AddrCtx::Cast(which) => (
                self.diag(DiagId::sema_addr_ovl_cast, at)
                    .arg(name)
                    .arg(which)
                    .arg(self.ty_arg(target)),
                false,
            ),
            AddrCtx::Reinterpret => (
                self.diag(DiagId::sema_addr_ovl_reinterpret, at).arg(name).arg(self.ty_arg(target)),
                false,
            ),
        };
        let fn_target = detailed && self.ast.types.is_function(fnty);
        let notes: Vec<Diag> =
            cands
                .iter()
                .map(|&(d, _)| {
                    if fn_target {
                        self.mismatch_note(d, fnty)
                    } else {
                        self.plain_candidate_note(d)
                    }
                })
                .collect();
        self.emit(notes.into_iter().fold(msg, Diag::with_note));
        None
    }

    /// `e` with its overload set `ule` replaced by `r` (parentheses and
    /// `&` rebuilt around it).
    fn rebuild_over(&mut self, e: ExprId, ule: ExprId, r: ExprId) -> ExprId {
        if e == ule {
            return r;
        }
        let n = *self.ast.exprs.get(e);
        match n.kind() {
            ExprKind::ParenExpr => {
                let inner = self.ast.exprs.operand(e);
                let x = self.rebuild_over(inner, ule, r);
                let rp = self.ast.exprs.end_loc(e).unwrap_or(n.loc());
                self.builder().paren(x, n.loc(), rp)
            }
            ExprKind::UnaryOperator => {
                let inner = self.ast.exprs.operand(e);
                let x = self.rebuild_over(inner, ule, r);
                // `&C::m` of a non-static member: a pointer to member.
                self.act_on_unary(UnaryOp::AddrOf, x, n.loc())
            }
            _ => r,
        }
    }

    /// `e` (parentheses around an overloaded name) with the name replaced
    /// by its resolution `r`.
    pub(crate) fn rebuild_parens(&mut self, e: ExprId, r: ExprId) -> ExprId {
        let n = *self.ast.exprs.get(e);
        if n.kind() != ExprKind::ParenExpr {
            return r;
        }
        let x = self.rebuild_parens(self.ast.exprs.operand(e), r);
        let rp = self.ast.exprs.end_loc(e).unwrap_or(n.loc());
        self.builder().paren(x, n.loc(), rp)
    }

    /// "candidate function" at `d`.
    fn plain_candidate_note(&self, d: DeclId) -> Diag {
        let loc = self.ast.decls.get(d).loc();
        let kind = if self.kind_of(d) == DeclKind::FunctionTemplate {
            "function template"
        } else {
            "function"
        };
        self.diag(DiagId::note_ovl_candidate, loc).arg(kind)
    }

    /// The note for candidate `d` whose type does not match the function
    /// type `want` (clang `HandleFunctionTypeMismatch`): the parameter
    /// count, then the first differing parameter, then the return type,
    /// then the exception specification; otherwise plain.
    fn mismatch_note(&self, d: DeclId, want: QualType) -> Diag {
        let loc = self.ast.decls.get(d).loc();
        let t = &self.ast.types;
        let has = self.ast.decls.get(d).ty().map(|x| t.canonical(x));
        let (Some(hp), Some(wp)) = (has.and_then(|h| t.proto(h)).copied(), t.proto(want).copied())
        else {
            return self.plain_candidate_note(d);
        };
        let (hs, ws) = (t.params(&hp), t.params(&wp));
        if hs.len() != ws.len() {
            return self
                .diag(DiagId::note_ovl_candidate_fn_arity, loc)
                .arg("function")
                .arg(ws.len().to_string())
                .arg(hs.len().to_string());
        }
        for (i, (&h, &w)) in hs.iter().zip(ws).enumerate() {
            if !t.same_unqualified(h, w) {
                return self
                    .diag(DiagId::note_ovl_candidate_fn_param, loc)
                    .arg("function")
                    .arg(crate::expr::ordinal(i + 1))
                    .arg(self.ty_arg(w))
                    .arg(self.ty_arg(h));
            }
        }
        if !t.same_type(hp.ret(), wp.ret()) {
            return self
                .diag(DiagId::note_ovl_candidate_fn_return, loc)
                .arg("function")
                .arg(self.ty_arg(wp.ret()))
                .arg(self.ty_arg(hp.ret()));
        }
        if t.exc_spec(&hp).is_nothrow() != t.exc_spec(&wp).is_nothrow() {
            return self.diag(DiagId::note_ovl_candidate_fn_noexcept, loc).arg("function");
        }
        self.plain_candidate_note(d)
    }

    /// An expression of overloaded-function type used without a target
    /// ([over.over]/1): "reference to overloaded function could not be
    /// resolved" with a "possible target for call" per candidate. True
    /// if `e` was one (and is now diagnosed).
    pub(crate) fn check_overload_use(&mut self, e: ExprId) -> bool {
        if let Some(x) = self.member_overload(e) {
            // [expr.ref]/7.3.2: a member function can only be called.
            let no_args = self.member_overload_decls(x).iter().any(|&d| {
                self.ast.decls.function(d).is_some_and(|f| self.ast.lists.get(f.params).is_empty())
            });
            let msg = self
                .diag(DiagId::sema_bound_member_not_called, self.eloc(x))
                .arg(u8::from(no_args));
            let notes: Vec<Diag> = self
                .member_overload_decls(x)
                .iter()
                .map(|&d| self.diag(DiagId::note_possible_target, self.ast.decls.get(d).loc()))
                .collect();
            self.emit(notes.into_iter().fold(msg, Diag::with_note));
            return true;
        }
        let Some((ule, _)) = self.overload_operand(e) else { return false };
        let loc = self.begin_loc(e);
        let msg = self.diag(DiagId::sema_ovl_unresolvable, loc).arg(0u8);
        let notes: Vec<Diag> = self
            .ule_candidates(ule)
            .iter()
            .map(|&(d, _)| self.diag(DiagId::note_possible_target, self.ast.decls.get(d).loc()))
            .collect();
        self.emit(notes.into_iter().fold(msg, Diag::with_note));
        true
    }
}
