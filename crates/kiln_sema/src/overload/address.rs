//! The address of an overloaded function ([over.over];
//! spec/sema-overload.md §2.9): an `UnresolvedLookupExpr` (possibly
//! parenthesized, possibly under `&`) used where a target type is known
//! resolves to the function whose type matches it; without a target it is
//! "reference to overloaded function could not be resolved".

use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::expr::{ExprKind, UnaryOp};
use kiln_ast::types::{ExcSpec, TypeView};
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

    /// The implicit member overload set (`f` for `this->f`, kiln's
    /// `UnresolvedMemberExpr`) an expression of overloaded-function type
    /// names (through parentheses and one `&`), and whether it is under
    /// `&`: with a target, [over.over]/1 selects a static member function
    /// from it, and the implicit object is dropped (clang's
    /// `FixOverloadedFunctionReference` of an implicit
    /// `UnresolvedMemberExpr`).
    pub(crate) fn implicit_member_operand(&self, e: ExprId) -> Option<(ExprId, bool)> {
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
        let x = self.member_overload(x)?;
        let m = self.ovl.members.get(&x)?;
        // An instantiated `f` is rebuilt as `this->f` with an implicit
        // `this` (spec/sema-overload.md §2.3).
        let implicit = m.implicit
            || (self.ast.exprs.get(m.base).kind() == ExprKind::CXXThisExpr
                && self.ast.exprs.this_is_implicit(m.base));
        (implicit && m.targs.is_none()).then_some((x, addr))
    }

    /// The matching static members of the implicit member overload set
    /// `x` for `target`, with their found declarations: a non-static
    /// member is named by an unqualified name only in a call
    /// ([expr.prim.id.general]/4, [expr.unary.op]/4). Without a matching
    /// non-template, the specializations of its static member function
    /// templates deduced from the target ([over.over]/3, /5).
    fn implicit_member_candidates(&mut self, x: ExprId, target: QualType) -> Vec<(DeclId, DeclId)> {
        let fnty = self.address_target(target);
        if fnty.1 {
            return Vec::new();
        }
        let cands = self.set_candidates(x);
        let direct: Vec<(DeclId, DeclId)> =
            cands.into_iter().filter(|&(d, _)| self.address_matches(d, fnty)).collect();
        if !direct.is_empty() {
            return direct;
        }
        self.address_template_candidates(x, target)
    }

    /// Whether the implicit member overload set `x` has a static member
    /// function or static member function template: clang's
    /// `UnresolvedMemberExpr` is then of overloaded type (not
    /// `hasOnlyNonStaticMemberFunctions`) and a target type resolves it;
    /// otherwise it is a bound member that can only be called.
    fn has_static_member(&self, x: ExprId) -> bool {
        self.member_overload_decls(x)
            .iter()
            .any(|&d| self.is_static_method(self.templated_or_self(d)))
    }

    /// The candidates (function, found declaration) of the overload set
    /// `key`, an `UnresolvedLookupExpr` or an implicit member overload
    /// set, in candidate order, without invalid declarations.
    fn set_candidates(&self, key: ExprId) -> Vec<(DeclId, DeclId)> {
        let Some(m) = self.ovl.members.get(&key) else { return self.ule_candidates(key) };
        m.decls
            .iter()
            .enumerate()
            .map(|(i, &d)| (d, m.found.get(i).copied().unwrap_or(d)))
            .filter(|&(d, _)| !self.ast.decls.get(d).has(DeclFlags::INVALID))
            .collect()
    }

    /// Whether the current context is in the body of a member function
    /// defined inside its class (lexically there, not out of line).
    fn in_inline_member_body(&self) -> bool {
        let Some(f) = self.enclosing_function(self.ctx) else { return false };
        let n = self.ast.decls.get(f);
        matches!(
            n.kind(),
            DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion
        ) && self.ast.decls.lexical_ctx(f) == n.ctx()
    }

    /// The name of the overload set `key` (see [`Self::set_candidates`]).
    fn set_name(&self, key: ExprId) -> String {
        match self.ovl.members.get(&key) {
            Some(m) => self.decl_name_str(m.name),
            None => self.decl_name_str(self.ast.unresolved_name(key)),
        }
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
    pub(crate) fn ule_candidates(&self, ule: ExprId) -> Vec<(DeclId, DeclId)> {
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
        // clang `checkAddressOfFunctionIsAvailable`: never a function
        // with `pass_object_size` parameters (WP-803).
        if !matches!(self.kind_of(d), DeclKind::Function | DeclKind::CXXMethod)
            || self.is_implicit_object_member(d) != member
            || !self.ast.pass_object_size_params(d).is_empty()
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
        crate::conv::is_function_conversion(self, t, fnty)
    }

    /// Whether the specialization `d` matches the function type `fnty`
    /// ignoring the exception specifications: deduction from the
    /// address of a function template selects it, and the conversion
    /// that follows checks the exception specification
    /// ([temp.deduct.funcaddr]; clang's `isSameOrCompatibleFunctionType`).
    fn address_matches_ignoring_exc(
        &mut self,
        d: DeclId,
        (fnty, member): (QualType, bool),
    ) -> bool {
        if self.address_matches(d, (fnty, member)) {
            return true;
        }
        if self.is_implicit_object_member(d) != member || !self.ast.types.is_function(fnty) {
            return false;
        }
        let Some(t) = self.ast.decls.get(d).ty() else { return false };
        let t = self.ast.types.canonical(t);
        let (a, b) = (self.without_exc_spec(t), self.without_exc_spec(fnty));
        self.ast.types.same_type(a, b)
    }

    /// The function type `f` with no exception specification.
    fn without_exc_spec(&mut self, f: QualType) -> QualType {
        let Some(p) = self.ast.types.proto(f).copied() else { return f };
        let params = self.ast.types.params(&p).to_vec();
        self.ast.types.function(p.ret(), &params, p.info(), ExcSpec::None)
    }

    /// The matching non-template candidates of `ule` for `target`.
    fn address_candidates(&mut self, ule: ExprId, target: QualType) -> Vec<(DeclId, DeclId)> {
        // [over.over]/3 via clang: explicit template arguments skip the
        // non-templates (only a typo-corrected set has both).
        if self.ast.templates.expr_targs.get(ule).is_some() {
            return Vec::new();
        }
        let cands = self.ule_candidates(ule);
        self.matching_candidates(cands, target)
    }

    /// Of `cands`, the non-templates matching `target`.
    fn matching_candidates(
        &mut self,
        cands: Vec<(DeclId, DeclId)>,
        target: QualType,
    ) -> Vec<(DeclId, DeclId)> {
        let fnty = self.address_target(target);
        cands.into_iter().filter(|&(d, _)| self.address_matches(d, fnty)).collect()
    }

    /// The specializations of the templates of `ule` (an overload set,
    /// see [`Self::set_candidates`]) deduced from the function type of
    /// `target` that match it, the most specialized kept ([over.over]/3,
    /// /5).
    fn address_template_candidates(
        &mut self,
        ule: ExprId,
        target: QualType,
    ) -> Vec<(DeclId, DeclId)> {
        let explicit = self.ast.templates.expr_targs.get(ule).copied();
        let loc = self.ast.exprs.get(ule).loc();
        let cands = self.set_candidates(ule);
        self.template_candidates(cands, explicit, loc, target)
    }

    /// Of `cands`, the specializations of the templates deduced from
    /// `target` (with the `explicit` template arguments) that match it,
    /// the most specialized kept.
    fn template_candidates(
        &mut self,
        cands: Vec<(DeclId, DeclId)>,
        explicit: Option<kiln_ast::ids::ArgListId>,
        loc: Loc,
        target: QualType,
    ) -> Vec<(DeclId, DeclId)> {
        let (fnty, member) = self.address_target(target);
        if !self.ast.types.is_function(fnty) {
            return Vec::new();
        }
        let mut out: Vec<(DeclId, DeclId, DeclId)> = Vec::new();
        for (d, f) in cands {
            if self.kind_of(d) != DeclKind::FunctionTemplate {
                continue;
            }
            let Ok(spec) = self.deduce_funcaddr(d, explicit, fnty, loc) else { continue };
            // A dependent noexcept-specifier left uninstantiated by the
            // deduction (DR1330) is part of the type compared now.
            self.resolve_exc_for_type(spec, loc);
            if self.address_matches_ignoring_exc(spec, (fnty, member)) {
                out.push((spec, f, d));
            }
        }
        if out.len() > 1 {
            let n = match self.ast.types.canonical_view(fnty) {
                TypeView::Function(p) => p.num_params() as usize,
                _ => 0,
            };
            let keep: Vec<bool> = (0..out.len())
                .map(|i| {
                    (0..out.len()).all(|j| i == j || self.more_specialized(out[i].2, out[j].2, n))
                })
                .collect();
            if let Some(i) = keep.iter().position(|&k| k) {
                return vec![(out[i].0, out[i].1)];
            }
        }
        out.into_iter().map(|(s, f, _)| (s, f)).collect()
    }

    /// The candidates of the overloaded member access `x` matching the
    /// non-member `target` ([over.over]/2-3; P2418's
    /// `__output_buffer<C>{..., __prepare_write, ...}`): only its static
    /// member functions can (a non-static one needs `&C::f`). Templates
    /// only when no non-template matches.
    fn member_address_candidates(&mut self, x: ExprId, target: QualType) -> Vec<(DeclId, DeclId)> {
        let Some(m) = self.ovl.members.get(&x) else { return Vec::new() };
        let (explicit, loc) = (m.targs, self.ast.exprs.get(x).loc());
        let cands: Vec<(DeclId, DeclId)> = m
            .decls
            .iter()
            .copied()
            .zip(m.found.iter().copied())
            .filter(|&(d, _)| !self.ast.decls.get(d).has(DeclFlags::INVALID))
            .collect();
        if self.address_target(target).1 {
            return Vec::new();
        }
        let found = if explicit.is_some() {
            Vec::new()
        } else {
            self.matching_candidates(cands.clone(), target)
        };
        if !found.is_empty() {
            return found;
        }
        self.template_candidates(cands, explicit, loc, target)
    }

    /// The function an expression of overloaded-function type `e`
    /// resolves to for `target`, silently (an implicit conversion
    /// sequence's query, [over.ics.scs]/… via [over.over]). Selects as
    /// [`Sema::resolve_address_of_overloaded`] does, so a function
    /// template's specialization deduced from each candidate's parameter
    /// type is a match too ([over.over]/3; clang's
    /// `ResolveAddressOfOverloadedFunction` from `TryImplicitConversion`).
    pub(crate) fn address_match(&mut self, e: ExprId, target: QualType) -> Option<DeclId> {
        if let Some((x, _)) = self.implicit_member_operand(e) {
            return match self.implicit_member_candidates(x, target).as_slice() {
                [(d, _)] => Some(*d),
                _ => None,
            };
        }
        if let Some(x) = self.member_overload(e) {
            return match self.member_address_candidates(x, target)[..] {
                [(d, _)] => Some(d),
                _ => None,
            };
        }
        let (ule, _) = self.overload_operand(e)?;
        let mut m = self.address_candidates(ule, target);
        if m.is_empty() {
            m = self.address_template_candidates(ule, target);
        }
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
    pub(crate) fn single_template_spec(&mut self, ule: ExprId) -> Option<(DeclId, DeclId)> {
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
            // A template-id names only templates: a non-template found
            // with them is not a candidate (clang's template-name lookup
            // filters it out, `FilterAcceptableTemplateNames`).
            if self.kind_of(d) != DeclKind::FunctionTemplate {
                continue;
            }
            let Ok(spec) = self.deduce_explicit_only(d, Some(explicit), loc) else { continue };
            if found.is_some() {
                return None;
            }
            found = Some((spec, f));
        }
        found
    }

    /// The one specialization the member template-id `x` (an overloaded
    /// member access with explicit template arguments) names without a
    /// call or a target type, with its found declaration: as
    /// [`Sema::single_template_spec`] over the member set (clang's
    /// `ResolveSingleFunctionTemplateSpecialization` of an
    /// `UnresolvedMemberExpr`).
    pub(crate) fn single_member_template_spec(&mut self, x: ExprId) -> Option<(DeclId, DeclId)> {
        let m = self.ovl.members.get(&x)?;
        let explicit = m.targs?;
        let cands: Vec<(DeclId, DeclId)> =
            m.decls.iter().copied().zip(m.found.iter().copied()).collect();
        let loc = self.ast.exprs.get(x).loc();
        let mut found = None;
        for (d, f) in cands {
            if self.kind_of(d) != DeclKind::FunctionTemplate
                || self.ast.decls.get(d).has(DeclFlags::INVALID)
            {
                continue;
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
        // The warnings its signature's substitution held back wait for the
        // use too: an initialization the specialization does not convert
        // to drops them ([`Sema::template_id_no_match`]).
        let held = if self.inst.stack.is_sfinae() {
            Vec::new()
        } else {
            self.take_suppressed_diags(d, loc)
        };
        let fresh = !self.inst.queued.contains(&d)
            && !self.ast.decls.get(self.ast.first(d)).has(DeclFlags::USED);
        let r = self.finish_overload_ref(d, f, naming, loc, nns, e, false);
        if fresh && self.inst.queued.contains(&d) {
            let _ = self.ovl.template_id_queued.insert(r, d);
        }
        if !held.is_empty() {
            if self.is_err_expr(r) {
                self.emit_held_warnings(held);
            } else {
                self.defer_template_id_warnings(r, d, held);
            }
        }
        // A deleted specialization waits for its use, qualified or not
        // (spec/sema-overload.md "Locations").
        if self.is_deleted_fn(d) && !self.is_err_expr(r) && !self.is_class_member_ref(d) {
            let at = self.begin_loc(e);
            self.defer_deleted_template_id(d, r, at);
        } else {
            self.note_function_name_use(d, r, loc, nns.is_none());
        }
        let _ = self.ovl.template_id_refs.insert(r, e);
        r
    }

    /// The overload set (its `UnresolvedLookupExpr`) the operand `e`
    /// is, possibly parenthesized and under `&`, a function template-id
    /// resolved early included (clang keeps it an overload set).
    pub(crate) fn overload_set_of(&self, e: ExprId) -> Option<ExprId> {
        if let Some((u, _)) = self.overload_operand(e) {
            return Some(u);
        }
        self.ovl.template_id_refs.get(&self.addr_operand(e)).copied()
    }

    /// `e` without parentheses and one `&` (the reference a function
    /// template-id resolved early is, under them).
    fn addr_operand(&self, e: ExprId) -> ExprId {
        let x = self.ignore_parens(e);
        if self.ast.exprs.get(x).kind() == ExprKind::UnaryOperator
            && self.ast.exprs.unary_op(x) == UnaryOp::AddrOf
        {
            return self.ignore_parens(self.ast.exprs.operand(x));
        }
        x
    }

    /// The name of the overload set an argument `e` is
    /// ([`Sema::overload_set_of`]): clang's
    /// `note_ovl_candidate_bad_overload`.
    pub(crate) fn overload_arg_name(&self, e: ExprId) -> Option<String> {
        self.overload_set_of(e).map(|u| self.decl_name_str(self.ast.unresolved_name(u)))
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
        self.resolve_address_of_overloaded_noted(e, target, complain, ctx, at, None)
    }

    /// [`Sema::resolve_address_of_overloaded`] with `extra` appended to
    /// the failure's notes (clang's `emitBadConversionNotes`: "passing
    /// argument to parameter 'p' here" for a default argument).
    pub(crate) fn resolve_address_of_overloaded_noted(
        &mut self,
        e: ExprId,
        target: QualType,
        complain: bool,
        ctx: AddrCtx,
        at: Loc,
        extra: Option<Diag>,
    ) -> Option<ExprId> {
        let (ule, matches) = if let Some((x, addr)) = self.implicit_member_operand(e) {
            let matches = self.implicit_member_candidates(x, target);
            if let [(d, f)] = matches[..] {
                let m = &self.ovl.members[&x];
                let (naming, nns) = (m.naming, m.nns);
                let loc = self.ast.exprs.get(x).loc();
                let r = self.finish_overload_ref(d, f, Some(naming), loc, nns, x, false);
                let at = self.begin_loc(e);
                self.note_function_name_use(d, r, at, nns.is_none());
                return Some(self.rebuild_over(e, x, r));
            }
            if !complain {
                return None;
            }
            // clang: `&f` is a lookup of the name and `f` with a static
            // member an overloaded `UnresolvedMemberExpr`, both resolved
            // against a non-member target as below; otherwise `f` is a
            // bound member function, which can only be called.
            if self.address_target(target).1 || !(addr || self.has_static_member(x)) {
                self.check_overload_use(e);
                return None;
            }
            (x, matches)
        } else {
            let Some((ule, _)) = self.overload_operand(e) else {
                if let Some(x) = self.member_overload(e) {
                    return self.resolve_member_address(e, x, target, complain, ctx, at);
                }
                if complain {
                    self.check_overload_use(e);
                }
                return None;
            };
            let mut matches = self.address_candidates(ule, target);
            if matches.is_empty() {
                // [over.over]/3: a function template's specialization
                // deduced from the target ([temp.deduct.funcaddr]); /5: a
                // non-template beats it, and of several the more
                // specialized.
                matches = self.address_template_candidates(ule, target);
            }
            if let [(d, f)] = matches[..] {
                let naming = self.ovl.ule_found.get(&ule).and_then(|x| x.naming);
                let nns = self.ast.templates.expr_nns.get(ule).copied();
                let loc = self.ast.exprs.get(ule).loc();
                let r = self.finish_overload_ref(d, f, naming, loc, nns, ule, false);
                // clang diagnoses a deleted function at the start of the
                // whole operand (its `&` or parenthesis).
                let at = self.begin_loc(e);
                self.note_function_name_use(d, r, at, nns.is_none());
                return Some(self.rebuild_over(e, ule, r));
            }
            if !complain {
                return None;
            }
            (ule, matches)
        };
        let mut cands = self.set_candidates(ule);
        if self.ovl.members.contains_key(&ule) && self.in_inline_member_body() {
            // clang finds the name through the class scope still open
            // around an inline member body (its `IdResolver` chain, the
            // last declared first); out of line, through the class's
            // lookup table, in declaration order.
            cands.reverse();
        }
        let name = self.set_name(ule);
        let loc = self.begin_loc(ule);
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
        let mut notes = self.addr_no_match_notes(ule, &cands, fnty, detailed, loc);
        notes.extend(extra);
        self.emit(notes.into_iter().fold(msg, Diag::with_note));
        None
    }

    /// The notes of "address of overloaded function does not match" for
    /// the overload set `ule` (see [`Self::set_candidates`]) with
    /// candidates `cands` against function type `fnty` (clang
    /// `ComplainNoMatchesFound`; `detailed`: an initialization, whose
    /// failed deductions and type mismatches are noted; `loc`, the name's
    /// start): with deduction failures, the non-template candidates in
    /// lookup order, then the failed templates by position
    /// ([temp.deduct.funcaddr]; clang's `DiagnoseBadDeduction`);
    /// otherwise every candidate in order.
    fn addr_no_match_notes(
        &mut self,
        ule: ExprId,
        cands: &[(DeclId, DeclId)],
        fnty: QualType,
        detailed: bool,
        loc: Loc,
    ) -> Vec<Diag> {
        let fn_target = detailed && self.ast.types.is_function(fnty);
        let explicit = self.ast.templates.expr_targs.get(ule).copied();
        let mut failed: Vec<(Loc, Diag)> = Vec::new();
        if detailed {
            for &(d, _) in cands {
                if self.kind_of(d) == DeclKind::FunctionTemplate
                    && let Err(f) = self.deduce_funcaddr(d, explicit, fnty, loc)
                    && f.tdk != crate::templ::deduce::Tdk::Invalid
                {
                    let at = self.ast.decls.get(d).loc();
                    failed.push((at, self.deduction_note(d, &f, at)));
                }
            }
        }
        let mut notes: Vec<Diag> = Vec::with_capacity(cands.len());
        for &(d, _) in cands {
            let tmpl = self.kind_of(d) == DeclKind::FunctionTemplate;
            if tmpl && !failed.is_empty() {
                continue;
            }
            notes.push(if fn_target && !tmpl {
                self.mismatch_note(d, fnty)
            } else {
                self.plain_candidate_note(d)
            });
        }
        failed.sort_by_key(|&(at, _)| at);
        notes.extend(failed.into_iter().map(|(_, n)| n));
        notes
    }

    /// [over.over]/1: an initialization (or `return`) of `to`, a
    /// pointer or reference to function or a pointer to member function,
    /// by `e`, a function template-id resolved early
    /// ([`Sema::resolve_single_template_id`]) whose specialization does
    /// not convert: clang keeps the template-id an overload set and finds
    /// no match for the target, so the specialization is never used (its
    /// held-back substitution warnings are dropped). The diagnostic's
    /// parts: the name's start, the name, the target function type and
    /// the notes.
    pub(crate) fn template_id_no_match(
        &mut self,
        e: ExprId,
        to: QualType,
    ) -> Option<(Loc, String, QualType, Vec<Diag>)> {
        let ule = self.overload_set_of(e)?;
        let (fnty, _) = self.address_target(to);
        if !self.ast.types.is_function(fnty) || self.ast.templates.expr_targs.get(ule).is_none() {
            return None;
        }
        self.drop_template_id_warnings(e);
        let loc = self.begin_loc(ule);
        let name = self.decl_name_str(self.ast.unresolved_name(ule));
        let cands = self.ule_candidates(ule);
        let notes = self.addr_no_match_notes(ule, &cands, fnty, true, loc);
        Some((loc, name, fnty, notes))
    }

    /// [over.over]/1: `e`, a function template-id resolved early to the
    /// one specialization its explicit arguments alone name
    /// ([`Sema::resolve_single_template_id`]), that does not convert to
    /// `to` (a pointer or reference to function or a pointer to member
    /// function): clang keeps the template-id an overload set, so the
    /// target selects among every template of the set, deducing the
    /// arguments the template-id leaves out from the target (`&f<1>` to
    /// `void (*)(void*, int)` with `f<int, class T>(void*, T)` beside
    /// `f<int>(void*)`). The resolved expression, or `None` (silently)
    /// when the target selects no single function.
    pub(crate) fn retarget_template_id(&mut self, e: ExprId, to: QualType) -> Option<ExprId> {
        let ule = self.overload_set_of(e)?;
        let (fnty, _) = self.address_target(to);
        if !self.ast.types.is_function(fnty)
            || self.ast.templates.expr_targs.get(ule).is_none()
            || self.ast.exprs.get(ule).ty() != QualType::OVERLOAD
        {
            return None;
        }
        let at = self.begin_loc(ule);
        let r = self.resolve_address_of_overloaded(ule, to, false, AddrCtx::Convert, at)?;
        self.drop_template_id_warnings(e);
        // The specialization resolved early is not used: its definition
        // is not instantiated ([temp.inst]/5), unless something else
        // queued it (`wrap<&C::f>(void*)` beside `wrap<&C::f, T0>(void*,
        // T0)`, its body ill-formed for a one-parameter `f`).
        if let Some(d) = self.ovl.template_id_queued.remove(&self.addr_operand(e))
            && self.referenced_decl_of_callee(r) != Some(d)
        {
            let _ = self.inst.queued.remove(&d);
            self.inst.pending.retain(|p| p.decl != d);
        }
        Some(r)
    }

    /// [`Sema::resolve_address_of_overloaded`] of the overloaded member
    /// access `x` (`e` is it, possibly parenthesized). As clang's
    /// `ResolveAddressOfOverloadedFunction`: a static winner named
    /// through an explicit object (`this->f`, `o.f`) is "cannot create a
    /// non-constant pointer to member function" at the access
    /// (`IsStaticMemberFunctionFromBoundPointer`), unless it is a
    /// template-id; no match lists every
    /// candidate, the last declared first.
    fn resolve_member_address(
        &mut self,
        e: ExprId,
        x: ExprId,
        target: QualType,
        complain: bool,
        ctx: AddrCtx,
        at: Loc,
    ) -> Option<ExprId> {
        let matches = self.member_address_candidates(x, target);
        let loc = self.ast.exprs.get(x).loc();
        if let [(d, f)] = matches[..] {
            let m = self.ovl.members.remove(&x)?;
            // A template-id is resolved to its specialization first
            // (clang's `ResolveSingleFunctionTemplateSpecialization`), so
            // `o.f<K>` names the static member without the error.
            if !m.implicit && m.targs.is_none() && self.is_static_method(d) {
                if !complain {
                    self.ovl.members.insert(x, m);
                    return None;
                }
                let b = self.begin_loc(x);
                self.emit(self.diag(DiagId::sema_nonconst_member_fn_ptr, b));
                return Some(self.recover_err(&[m.base], loc));
            }
            let r = self.finish_member_winner(&m, d, f, loc);
            return Some(self.rebuild_parens(e, r));
        }
        if !complain {
            return None;
        }
        // A set of only implicit object member functions has clang's
        // bound-member type: whatever the target, it can only be called
        // ([expr.ref]/7.3.2; `CheckPlaceholderExpr` diagnoses it before
        // the conversion, WP-788).
        if self.member_set_needs_call(x) {
            self.diagnose_member_set_not_called(x);
            return None;
        }
        let m = self.ovl.members.get(&x)?;
        let name = self.decl_name_str(m.name);
        let mut cands = m.decls.clone();
        cands.reverse();
        let b = self.begin_loc(x);
        let (fnty, _) = self.address_target(target);
        let (msg, detailed) = if matches.len() > 1 {
            (self.diag(DiagId::sema_addr_ovl_ambiguous, b).arg(name), false)
        } else {
            match ctx {
                AddrCtx::Convert => (
                    self.diag(DiagId::sema_addr_ovl_no_match, b).arg(name).arg(self.ty_arg(fnty)),
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
                    self.diag(DiagId::sema_addr_ovl_reinterpret, at)
                        .arg(name)
                        .arg(self.ty_arg(target)),
                    false,
                ),
            }
        };
        let fn_target = detailed && self.ast.types.is_function(fnty);
        let notes: Vec<Diag> = if matches.len() > 1 {
            matches.iter().map(|&(d, _)| self.plain_candidate_note(d)).collect()
        } else {
            cands
                .iter()
                .map(|&d| {
                    if fn_target && self.kind_of(d) != DeclKind::FunctionTemplate {
                        self.mismatch_note(d, fnty)
                    } else {
                        self.plain_candidate_note(d)
                    }
                })
                .collect()
        };
        self.emit(notes.into_iter().fold(msg, Diag::with_note));
        None
    }

    /// A simple assignment from the overload set `e` to `to` whose
    /// selection does not convert (none, several, or a specialization
    /// deduced ignoring its exception specification): clang's
    /// "assigning to 'T' from incompatible type '<overloaded function
    /// type>'" with every candidate noted (`NoteAllOverloadCandidates`:
    /// a non-template's type mismatch, a template plainly).
    pub(crate) fn diagnose_overload_assign(&mut self, e: ExprId, to: QualType) {
        let Some(ule) = self.overload_set_of(e) else { return };
        let loc = self.begin_loc(e);
        let msg = self
            .diag(DiagId::sema_assign_incompatible, loc)
            .arg(self.ty_arg(to))
            .arg(self.ty_arg(QualType::OVERLOAD));
        let notes = self.overload_assign_notes(ule, to);
        self.emit(notes.into_iter().fold(msg, Diag::with_note));
    }

    /// The candidate notes of an assignment from the overload set `ule`
    /// to `to` ([`Sema::diagnose_overload_assign`]).
    pub(crate) fn overload_assign_notes(&mut self, ule: ExprId, to: QualType) -> Vec<Diag> {
        let (fnty, _) = self.address_target(to);
        let fn_target = self.ast.types.is_function(fnty);
        // A template-id names only the templates.
        let tid = self.ast.templates.expr_targs.get(ule).is_some();
        self.ule_candidates(ule)
            .into_iter()
            .filter(|&(d, _)| !tid || self.kind_of(d) == DeclKind::FunctionTemplate)
            .map(|(d, _)| {
                if fn_target && self.kind_of(d) != DeclKind::FunctionTemplate {
                    self.mismatch_note(d, fnty)
                } else {
                    self.plain_candidate_note(d)
                }
            })
            .collect()
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
    pub(crate) fn plain_candidate_note(&self, d: DeclId) -> Diag {
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

    /// The one non-template function or static member function (and its
    /// found declaration) of the overload set `e`, explicit template
    /// arguments ignored: clang's `resolveAddressOfSingleOverloadCandidate`.
    /// Only typo correction builds a set of a non-template with template
    /// arguments (spec/sema.md "Undeclared template-ids").
    /// `e` may be parenthesized and under `&` ([`Sema::overload_operand`]);
    /// the result is the set's `UnresolvedLookupExpr`, the function and
    /// its found declaration.
    pub(crate) fn single_overload_candidate(&self, e: ExprId) -> Option<(ExprId, DeclId, DeclId)> {
        let (ule, _) = self.overload_operand(e)?;
        // Those whose address can be taken (WP-803: not one with
        // `pass_object_size` parameters).
        let mut cands = self.ule_candidates(ule);
        cands.retain(|&(d, _)| self.ast.pass_object_size_params(d).is_empty());
        let [(d, found)] = cands[..] else { return None };
        (matches!(self.kind_of(d), DeclKind::Function | DeclKind::CXXMethod)
            && !self.is_class_member_ref(d))
        .then_some((ule, d, found))
    }

    /// An operand `e` that is an overload set of one non-template
    /// function ([`Sema::single_overload_candidate`]): the reference to
    /// that function (clang's `resolveAndFixAddressOfSingleOverloadCandidate`
    /// in `CheckPlaceholderExpr`), else `e`.
    pub(crate) fn resolve_single_overload_candidate(&mut self, e: ExprId) -> ExprId {
        let Some((ule, d, found)) = self.single_overload_candidate(e) else { return e };
        let naming = self.ovl.ule_found.get(&ule).and_then(|x| x.naming);
        let nns = self.ast.templates.expr_nns.get(ule).copied();
        let loc = self.ast.exprs.get(ule).loc();
        let r = self.finish_overload_ref(d, found, naming, loc, nns, ule, true);
        self.rebuild_over(e, ule, r)
    }

    /// An expression of overloaded-function type used without a target
    /// ([over.over]/1): "reference to overloaded function could not be
    /// resolved" with a "possible target for call" per candidate. True
    /// if `e` was one (and is now diagnosed).
    pub(crate) fn check_overload_use(&mut self, e: ExprId) -> bool {
        let member =
            self.member_overload(e).or_else(|| self.implicit_member_operand(e).map(|p| p.0));
        if let Some(x) = member {
            self.diagnose_member_set_not_called(x);
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

    /// The member overload set `x` used other than called ([expr.ref]/7.3.2:
    /// a non-static member function can only be called): "reference to
    /// non-static member function must be called" with a "possible
    /// target for call" per member. The "did you mean to call it with no
    /// arguments?" hint follows clang's `isExprCallable`: the members
    /// callable with no arguments (a template's pattern counts) all
    /// return the same type, and there is one; never in a SFINAE
    /// context. True when the hint was given (clang then recovers by
    /// the call, [`Sema::recover_not_called`]).
    /// Whether member overload set `x` holds only implicit object member
    /// functions: clang gives it the bound-member type, so whatever the
    /// target it can only be called ([expr.ref]/7.3.2).
    pub(crate) fn member_set_needs_call(&self, x: ExprId) -> bool {
        self.member_overload_decls(x)
            .iter()
            .all(|&d| self.is_implicit_object_member(self.templated_or_self(d)))
    }

    pub(crate) fn diagnose_member_set_not_called(&mut self, x: ExprId) -> bool {
        self.diagnose_member_set_not_called_as(x, false)
    }

    /// [`Self::diagnose_member_set_not_called`]; `again` exempts it from
    /// deduplication, for the report clang repeats when the initializer
    /// it was deduced from is then converted (`auto a = g.f;`).
    pub(crate) fn diagnose_member_set_not_called_as(&mut self, x: ExprId, again: bool) -> bool {
        let decls = self.member_overload_decls(x).to_vec();
        let mut ret: Option<QualType> = None;
        let mut no_args = true;
        for &d in &decls {
            let f = self.templated_or_self(d);
            if self.fn_min_args(f) != 0 {
                continue;
            }
            let r = self.ast.decls.get(f).ty().and_then(|t| {
                let types = &self.ast.types;
                types
                    .proto_of(t.ty())
                    .or_else(|| types.proto(t))
                    .map(kiln_ast::types::FuncProto::ret)
            });
            let r = r.map(|r| self.ast.types.canonical(r));
            match (ret, r) {
                (None, Some(r)) => ret = Some(r),
                (Some(p), Some(r)) if p == r => {}
                _ => no_args = false,
            }
        }
        let no_args = no_args && ret.is_some() && !self.inst.stack.is_sfinae();
        let msg =
            self.diag(DiagId::sema_bound_member_not_called, self.eloc(x)).arg(u8::from(no_args));
        let msg = if again { msg.allow_duplicate() } else { msg };
        let notes: Vec<Diag> = decls
            .iter()
            .map(|&d| self.diag(DiagId::note_possible_target, self.ast.decls.get(d).loc()))
            .collect();
        self.emit(notes.into_iter().fold(msg, Diag::with_note));
        no_args
    }
}
