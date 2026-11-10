//! clang's `enable_if` attribute (WP-460; spec/sema.md "The `enable_if`
//! attribute"): `__attribute__((enable_if(cond, "message")))` on a
//! function makes a call candidate viable only if `cond`, evaluated as a
//! constant expression with the call's converted arguments substituted
//! for the parameters, is true (clang `Sema::CheckEnableIf`); of two
//! viable candidates, the one whose conditions extend the other's wins
//! (clang `compareEnableIfAttrs`).

use kiln_ast::attr::{AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::DeclKind;
use kiln_ast::ids::{ListId, RawId};
use kiln_ast::{AttrId, DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::conv::{self, ConvCtx};
use crate::decl::AttrArg;
use crate::overload::{ArgInfo, Candidate, CandidateFailure};
use crate::templ::stack::{FrameKind, InstFrame};
use crate::templ::subst::{DeclOrPack, Substituter};

impl Sema<'_, '_> {
    /// `enable_if(cond, "message")` as written (clang
    /// `handleEnableIfAttr` / `checkFunctionConditionAttr`): exactly two
    /// arguments; `a` is the condition, contextually converted to `bool`
    /// unless it is type-dependent, `b` the message's string literal.
    /// The parser parses the arguments with the function's parameters in
    /// scope.
    pub(crate) fn enable_if_attribute(
        &mut self,
        syntax: AttrSyntax,
        args: &[AttrArg],
        loc: Loc,
    ) -> Option<AttrId> {
        let name = "'enable_if'";
        if args.len() != 2 {
            let msg =
                self.diag(DiagId::sema_attribute_wrong_number_arguments, loc).arg(name).arg("2");
            self.emit(msg);
            return None;
        }
        let (AttrArg::Expr(cond), AttrArg::Expr(msg)) = (args[0], args[1]) else {
            self.error(DiagId::parse_expected_expression, loc);
            return None;
        };
        let cond = if self.expr_is_type_dependent(cond) {
            cond
        } else {
            match conv::contextually_convert_to_bool(self, cond) {
                Ok(c) => c,
                Err(err) => {
                    self.emit_conv_error(err, self.ast.exprs.get(cond).loc());
                    return None;
                }
            }
        };
        let cond = self.act_on_constant_full_expr(cond);
        let Some(msg) = self.string_lit_of(msg) else {
            let at = self.ast.exprs.get(msg).loc();
            let d = self.diag(DiagId::parse_attribute_requires_string, at).arg("enable_if");
            self.emit(d);
            return None;
        };
        let node = AttrNode {
            kind: AttrKind::ENABLE_IF,
            syntax,
            flags: 0,
            loc,
            a: cond.raw(),
            b: msg.raw(),
        };
        Some(self.ast.attrs.push(node))
    }

    /// The `enable_if` attributes of function declaration `f` in
    /// declaration order, each with the declaration it is written on (`f`:
    /// every declaration of the function has the same ones, as
    /// [`Self::enable_if_list`] makes differing ones overloads).
    pub(crate) fn enable_if_attrs(&self, f: DeclId) -> Vec<(DeclId, AttrNode)> {
        self.ast
            .lists
            .iter::<AttrId>(self.ast.attrs.of_decl(f))
            .map(|a| *self.ast.attrs.get(a))
            .filter(|n| n.kind == AttrKind::ENABLE_IF)
            .map(|n| (f, n))
            .collect()
    }

    /// Whether function declaration `f` has an `enable_if` attribute.
    fn has_enable_if(&self, f: DeclId) -> bool {
        self.ast
            .lists
            .iter::<AttrId>(self.ast.attrs.of_decl(f))
            .any(|a| self.ast.attrs.get(a).kind == AttrKind::ENABLE_IF)
    }

    /// The conditions of the `enable_if` attributes among `attrs`, as a
    /// list ([`ListId::EMPTY`] if none), for [`crate::overload::redecl::NewFn`]:
    /// a function declaration whose conditions are not those of an earlier
    /// declaration in order (by equivalence) declares an overload, not a
    /// redeclaration (clang `Sema::IsOverload`).
    pub(crate) fn enable_if_list(&mut self, attrs: &[AttrId]) -> ListId {
        let conds: Vec<ExprId> = attrs
            .iter()
            .map(|&a| *self.ast.attrs.get(a))
            .filter(|n| n.kind == AttrKind::ENABLE_IF)
            .filter_map(|n| ExprId::from_raw(n.a))
            .collect();
        if conds.is_empty() { ListId::EMPTY } else { self.ast.lists.push(&conds) }
    }

    /// The `enable_if` conditions of the function declaration being
    /// declared with attributes `attrs`: those of the pattern it
    /// instantiates ([`Sema::inst_enable_if`]), else its own.
    pub(crate) fn new_fn_enable_if(&mut self, attrs: &[AttrId]) -> ListId {
        match self.inst_enable_if.take() {
            Some(l) => l,
            None => self.enable_if_list(attrs),
        }
    }

    /// The [`Self::enable_if_list`] of declaration `f`.
    pub(crate) fn enable_if_list_of(&mut self, f: DeclId) -> ListId {
        let attrs: Vec<AttrId> = self.ast.lists.iter::<AttrId>(self.ast.attrs.of_decl(f)).collect();
        self.enable_if_list(&attrs)
    }

    /// Whether function `old` has the `enable_if` conditions `new`, in
    /// order (clang `Sema::IsOverload`).
    pub(crate) fn same_enable_if(&self, old: DeclId, new: ListId) -> bool {
        let a: Vec<ExprId> = self
            .enable_if_attrs(old)
            .into_iter()
            .filter_map(|(_, n)| ExprId::from_raw(n.a))
            .collect();
        let b: Vec<ExprId> = self.ast.lists.iter::<ExprId>(new).collect();
        a.len() == b.len() && a.iter().zip(&b).all(|(&x, &y)| self.exprs_equivalent(x, y))
    }

    /// The message of `enable_if` attribute `node`, as the note prints it
    /// (clang: `<no message provided>` for an empty one).
    pub(crate) fn enable_if_message(&self, node: &AttrNode) -> String {
        let text = ExprId::from_raw(node.b).and_then(|e| self.string_msg(e)).unwrap_or_default();
        if text.is_empty() { "<no message provided>".to_owned() } else { text }
    }

    /// clang `Sema::CheckEnableIf` for viable candidate `cand` of a call
    /// with arguments `args`, `params` the candidate's parameter types and
    /// `skip` the parameters before the first argument's: each argument
    /// is converted to its parameter's type (an omitted one is the
    /// parameter's default argument), then each `enable_if` condition is
    /// evaluated with them; the first one that is not a constant
    /// expression or is false makes the candidate not viable
    /// ([`CandidateFailure::EnableIf`]).
    ///
    /// `strict` is clang's handling of a member function with an explicit
    /// object parameter found by overload resolution: clang passes the
    /// arguments without the object argument and binds them from the
    /// first parameter (the object parameter) on, so they shift by one;
    /// an argument that does not convert to the parameter it lands on,
    /// or a parameter left without an argument or a default argument,
    /// disables the candidate by its first `enable_if` attribute
    /// (`convertArgsForAvailabilityChecks` failing).
    pub(crate) fn check_enable_if(
        &mut self,
        cand: &mut Candidate,
        args: &[ArgInfo],
        params: &[QualType],
        skip: usize,
        strict: bool,
        loc: Loc,
    ) {
        let Some(f) = cand.decl else { return };
        if !self.has_enable_if(f) {
            return;
        }
        // The converted arguments, by parameter index (`None`: no value).
        let mut conv_args: Vec<Option<ExprId>> = Vec::with_capacity(params.len());
        let mut unbound = false;
        for (j, &p) in params.iter().enumerate().skip(skip) {
            let i = j - skip;
            let arg = if let Some(a) = args.get(i) {
                let r = a.list.is_none().then_some(a.expr).flatten().and_then(|e| {
                    let pt = conv::strip_cv(self, p);
                    let to = if self.ast.types.is_reference(p) { p } else { pt };
                    let (r, diags) =
                        self.capturing(|s| conv::implicit_convert(s, e, to, ConvCtx::Argument));
                    let s_failed = |s: &Self, d: &[kiln_diag::Diag]| s.stash_has_error(d);
                    let failed = s_failed(self, &diags);
                    r.ok().filter(|_| !failed)
                });
                unbound |= r.is_none();
                r
            } else {
                let r = self.enable_if_default_arg(f, j);
                unbound |= r.is_none() && !self.nth_param_has_default(f, j);
                r
            };
            conv_args.push(arg);
        }
        if strict && unbound {
            let first = self
                .ast
                .lists
                .iter::<AttrId>(self.ast.attrs.of_decl(f))
                .find(|&a| self.ast.attrs.get(a).kind == AttrKind::ENABLE_IF);
            if let Some(attr) = first {
                cand.viable = false;
                cand.failure = CandidateFailure::EnableIf { attr };
            }
            return;
        }
        if let Some(attr) = self.failed_enable_if(f, &conv_args, skip, loc) {
            cand.viable = false;
            cand.failure = CandidateFailure::EnableIf { attr };
        }
    }

    /// The first `enable_if` attribute of `f` whose condition does not
    /// hold for the converted arguments `args` (by parameter index from
    /// `skip`; `None`: no value).
    fn failed_enable_if(
        &mut self,
        f: DeclId,
        args: &[Option<ExprId>],
        skip: usize,
        loc: Loc,
    ) -> Option<AttrId> {
        let attrs: Vec<AttrId> = self
            .ast
            .lists
            .iter::<AttrId>(self.ast.attrs.of_decl(f))
            .filter(|&a| self.ast.attrs.get(a).kind == AttrKind::ENABLE_IF)
            .collect();
        for attr in attrs {
            let Some(cond) = ExprId::from_raw(self.ast.attrs.get(attr).a) else { continue };
            if !self.enable_if_holds(f, cond, args, skip, loc) {
                return Some(attr);
            }
        }
        None
    }

    /// clang `CheckEnableIf` on a call without overload resolution of
    /// function `f` through `callee` with the converted arguments `args`
    /// (default arguments included): diagnoses and returns `false` if a
    /// condition does not hold ("no matching function", or "no matching
    /// member function" for a member call, clang
    /// `BuildCallToMemberFunction`).
    pub(crate) fn check_enable_if_direct(
        &mut self,
        f: DeclId,
        callee: ExprId,
        args: &[ExprId],
        loc: Loc,
    ) -> bool {
        if !self.has_enable_if(f) {
            return true;
        }
        let args: Vec<Option<ExprId>> = args.iter().map(|&a| Some(a)).collect();
        let Some(attr) = self.failed_enable_if(f, &args, 0, loc) else { return true };
        let r = self.ignore_parens(callee);
        let member = self.ast.exprs.get(r).kind() == kiln_ast::expr::ExprKind::MemberExpr;
        let (id, at) = if member {
            (DiagId::sema_ovl_no_viable_member_function_in_call, self.ast.exprs.get(r).loc())
        } else if self.kind_of(f) == DeclKind::CXXMethod {
            // An explicit object member function called through a member
            // access, or a member named by a qualified id (clang
            // `BuildCallExpr`: `isa<CXXMethodDecl>`).
            (DiagId::sema_ovl_no_viable_member_function_in_call, self.begin_loc(callee))
        } else {
            (DiagId::sema_ovl_no_viable_function_in_call, self.begin_loc(callee))
        };
        let why = self.enable_if_note(attr, self.ast.decls.get(f).loc());
        let msg = self.diag(id, at).arg(self.decl_name(f)).range(self.expr_range(callee));
        self.emit(msg.with_note(why));
        false
    }

    /// clang `note_ovl_candidate_disabled_by_function_cond_attr` for the
    /// failed `enable_if` attribute `attr`, at `loc`.
    pub(crate) fn enable_if_note(&self, attr: AttrId, loc: Loc) -> kiln_diag::Diag {
        let node = *self.ast.attrs.get(attr);
        let msg = self.enable_if_message(&node);
        self.diag(DiagId::note_ovl_candidate_disabled_by_function_cond_attr, loc).arg(msg)
    }

    /// Whether parameter `j` of `f` has a default argument (instantiated
    /// or not).
    fn nth_param_has_default(&self, f: DeclId, j: usize) -> bool {
        self.ast
            .decls
            .function(f)
            .and_then(|x| self.ast.lists.iter::<DeclId>(x.params).nth(j))
            .is_some_and(|p| self.inst.lazy_defaults.contains_key(&p) || self.param_has_default(p))
    }

    /// The default argument of parameter `j` of `f` as a call omitting
    /// it would use it, if it has one that is instantiated.
    fn enable_if_default_arg(&mut self, f: DeclId, j: usize) -> Option<ExprId> {
        let p = self
            .ast
            .decls
            .function(f)
            .and_then(|x| self.ast.lists.iter::<DeclId>(x.params).nth(j))?;
        if self.inst.lazy_defaults.contains_key(&p) {
            return None;
        }
        let e = self.ast.decls.param(p)?.default_arg?;
        let x = *self.ast.exprs.get(e);
        let used = self.ctx;
        Some(self.builder().default_arg(p, used, x.ty(), x.value_category(), Loc::INVALID))
    }

    /// Whether condition `cond` of an `enable_if` attribute written on
    /// `holder` is a constant expression that is true with `args`
    /// substituted for the parameters (clang
    /// `Expr::EvaluateWithSubstitution`): the parameters of `holder` (and
    /// of the pattern it was instantiated from, which a non-dependent
    /// condition names) take the arguments; an argument that is not a
    /// constant expression leaves its parameter without a value.
    fn enable_if_holds(
        &mut self,
        holder: DeclId,
        cond: ExprId,
        args: &[Option<ExprId>],
        skip: usize,
        loc: Loc,
    ) -> bool {
        self.cond_with_args(holder, cond, args, skip, loc).unwrap_or(false)
    }

    /// The value of the condition `cond` of an `enable_if` or
    /// `diagnose_if` attribute written on `holder` with `args`
    /// substituted for the parameters (from index `skip`), as
    /// [`Self::enable_if_holds`] evaluates it; `None` if it is
    /// value-dependent or not a constant expression.
    pub(crate) fn cond_with_args(
        &mut self,
        holder: DeclId,
        cond: ExprId,
        args: &[Option<ExprId>],
        skip: usize,
        loc: Loc,
    ) -> Option<bool> {
        if self.expr_is_value_dependent(cond) {
            return None;
        }
        let mut owners = vec![holder];
        if let Some(pat) = self.ast.instantiated_from(holder) {
            owners.push(pat);
        }
        let mut binds = Vec::new();
        for o in owners {
            let Some(params) = self.ast.decls.function(o).map(|x| x.params) else { continue };
            let params: Vec<DeclId> = self.ast.lists.iter::<DeclId>(params).collect();
            for (j, p) in params.into_iter().enumerate().skip(skip) {
                if let Some(Some(a)) = args.get(j - skip) {
                    binds.push((p, *a));
                }
            }
        }
        let truth = |s: &mut Self, binds: &[(DeclId, ExprId)]| {
            s.eval_with_params(Some(cond), binds, cond, loc).and_then(|v| s.const_truth(v))
        };
        if let Some(t) = truth(self, &binds) {
            return Some(t);
        }
        // Some argument may not be a constant expression: drop those and
        // evaluate again.
        let kept: Vec<(DeclId, ExprId)> = binds
            .iter()
            .copied()
            .filter(|&(p, a)| self.eval_with_params(None, &[(p, a)], a, loc).is_some())
            .collect();
        if kept.len() == binds.len() {
            return None;
        }
        truth(self, &kept)
    }

    /// clang `compareEnableIfAttrs` ([over.match.best] extension):
    /// `Some(true)` if function `f1` is better than `f2` by their
    /// `enable_if` attributes, `Some(false)` if worse, `None` if neither.
    /// A function with the attributes beats one without; otherwise the
    /// conditions are compared in order, and `f1` is better only if `f2`'s
    /// are a proper prefix of its own (by equivalence, [temp.over.link]/5).
    pub(crate) fn compare_enable_if(&self, f1: DeclId, f2: DeclId) -> Option<bool> {
        let (h1, h2) = (self.has_enable_if(f1), self.has_enable_if(f2));
        if !h1 || !h2 {
            return (h1 != h2).then_some(h1);
        }
        let a1 = self.enable_if_attrs(f1);
        let a2 = self.enable_if_attrs(f2);
        let n = a1.len().max(a2.len());
        for i in 0..n {
            let (Some(x), Some(y)) = (a1.get(i), a2.get(i)) else {
                return Some(a1.len() > a2.len());
            };
            let (Some(c1), Some(c2)) = (ExprId::from_raw(x.1.a), ExprId::from_raw(y.1.a)) else {
                return Some(false);
            };
            if !self.exprs_equivalent(c1, c2) {
                return Some(false);
            }
        }
        None
    }
}

impl Substituter<'_, '_, '_> {
    /// clang `instantiateDependentEnableIfAttr`: the condition of
    /// `enable_if` attribute `node` of pattern function `def`, substituted
    /// for its instantiation `spec` with the pattern's parameters naming
    /// `spec`'s, then contextually converted to `bool`; `None` drops the
    /// attribute (a substitution or conversion failure).
    pub(crate) fn enable_if_cond(
        &mut self,
        spec: DeclId,
        def: DeclId,
        node: AttrNode,
    ) -> Option<AttrNode> {
        let cond = ExprId::from_raw(node.a)?;
        if !self.needs_rebuild(cond) {
            return Some(node);
        }
        let params = |s: &Self, f: DeclId| -> Vec<DeclId> {
            s.sema
                .ast
                .decls
                .function(f)
                .map(|x| s.sema.ast.lists.iter::<DeclId>(x.params).collect())
                .unwrap_or_default()
        };
        let (pp, fp) = (params(self, def), params(self, spec));
        let saved: Vec<(DeclId, Option<DeclOrPack>)> =
            pp.iter().map(|&p| (p, self.locals.remove(&p))).collect();
        self.map_params(&pp, &fp);
        let r = self.expr(cond);
        for (p, old) in saved {
            match old {
                Some(x) => {
                    self.locals.insert(p, x);
                }
                None => {
                    self.locals.remove(&p);
                }
            }
        }
        let c = r.ok()?;
        let c = if self.sema.expr_is_type_dependent(c)
            || self.sema.ast.types.canonical(self.sema.ety(c)) == QualType::BOOL
        {
            c
        } else {
            match conv::contextually_convert_to_bool(self.sema, c) {
                Ok(c) => c,
                Err(err) => {
                    let at = self.sema.ast.exprs.get(c).loc();
                    self.sema.emit_conv_error(err, at);
                    return None;
                }
            }
        };
        let node = AttrNode { a: c.raw(), ..node };
        let Some(fail) = self.sema.attr_cond_never_constant(spec, node) else {
            return Some(node);
        };
        // clang substitutes the attribute in the specialization's own
        // instantiation context, outside the immediate context of a
        // deduction: the error is a hard one, with "in instantiation of
        // function template specialization ... requested here" in place
        // of the deduction's note.
        let tmpl = self.sema.spec_args(spec).map(|(t, _)| t);
        let stack = &mut self.sema.inst.stack;
        let swapped = match (tmpl, stack.frames.last()) {
            (Some(t), Some(top)) if stack.is_sfinae() => {
                let poi = top.poi;
                let deduction = matches!(
                    top.kind,
                    FrameKind::DeducedArgSubstitution | FrameKind::ExplicitArgSubstitution
                );
                let saved = if deduction { stack.frames.pop() } else { None };
                stack.frames.push(InstFrame {
                    kind: FrameKind::FunctionSpec,
                    entity: spec,
                    template: t,
                    poi,
                    sfinae: Some(false),
                    args: None,
                    start: None,
                    start_bytes: 0,
                    start_nodes: 0,
                    child_ns: 0,
                    trace: None,
                });
                Some(saved)
            }
            _ => None,
        };
        self.sema.report_never_constant(node, fail);
        if let Some(saved) = swapped {
            let stack = &mut self.sema.inst.stack;
            stack.frames.pop();
            stack.frames.extend(saved);
        }
        None
    }
}
