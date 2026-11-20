//! Substitution into a lambda-expression ([temp.inst]/2,
//! [expr.prim.lambda.closure]/2; clang `TreeTransform::TransformLambdaExpr`,
//! spec/sema-lambda.md §9): each instantiation of the enclosing template
//! gets a closure type of its own. The lambda is replayed through the
//! actions the parser calls: the explicit captures (init-capture
//! initializers substituted), the call operator's signature with
//! substituted parameter and trailing return types, then the body, whose
//! rebuilt uses capture implicitly again.

use kiln_ast::ExprId;
use kiln_ast::decl::{ConstexprKind, DeclKind, FnStorage, InitStyle};
use kiln_ast::expr::ExprKind;
use kiln_ast::lambda::LambdaCapture;
use kiln_ast::names::OverloadedOperator;
use kiln_ast::types::{ArgView, Quals, RefQual, TemplateArg, TypeView};
use kiln_source::Loc;

use super::{DeclOrPack, SubstFail, SubstResult, Substituter};
use crate::declspec::{Chunk, Declarator, DeclaratorContext};
use crate::lambda::{LambdaCaptureSpec, LambdaIntroducer, LambdaSpecifiers};
use crate::templ::args::{LevelMode, MultiLevelArgs};
use crate::templ::pack::{Expansion, PackOperand};
use crate::templ::stack::FrameKind;

impl Substituter<'_, '_, '_> {
    /// The lambda-expression `e` of the pattern.
    pub(crate) fn lambda_expr(&mut self, e: ExprId, loc: Loc) -> SubstResult<ExprId> {
        let id = self.sema.ast.exprs.lambda_id(e);
        let data = self.sema.ast.exprs.lambdas.get(id).clone();
        let (Some(call_op), Some(body)) = (data.call_op, data.body) else {
            return Err(SubstFail);
        };
        // A generic lambda: its template parameter list is substituted
        // (its own parameters one level shallower per substituted level,
        // as a member template's are) and its body rebuilt as a pattern.
        let head = match self.sema.ast.described_template(call_op) {
            Some(t) => Some((t, self.sema.ast.template_info(t).ok_or(SubstFail)?.params)),
            None => None,
        };
        // The explicit captures; the implicit ones are found again in the
        // rebuilt body (clang stops at the first implicit capture).
        let mut intro = LambdaIntroducer {
            lbracket: data.lbracket,
            rbracket: data.rbracket,
            default: data.default,
            default_loc: data.default_loc,
            captures: Vec::new(),
        };
        let init_vars = self.lambda_captures(&data.captures, &mut intro)?;
        let never_dependent = self.sema.substituted_lambda_never_dependent();
        let lambda = self.sema.act_on_lambda_start_in(&intro, never_dependent);
        // The init-capture variables, in order (several pattern
        // variables share a name after an earlier expansion of an
        // init-capture pack); an expanded pack maps to all its elements.
        // Mapped before the declarator, where they are in scope (P2036).
        let caps = self.sema.ast.exprs.lambdas.get(lambda).captures.clone();
        let mut news = caps.iter().filter(|c| c.init).filter_map(|c| c.var);
        for (v, n) in init_vars {
            let vars: Vec<kiln_ast::DeclId> = news.by_ref().take(n.unwrap_or(1)).collect();
            match n {
                Some(_) => {
                    let l = self.sema.ast.lists.push(&vars);
                    self.locals.insert(v, DeclOrPack::Pack(l));
                }
                None => {
                    if let Some(&x) = vars.first() {
                        self.locals.insert(v, DeclOrPack::Decl(x));
                    }
                }
            }
        }
        if let Some((_, params)) = head
            && self.open_substituted_head(params).is_err()
        {
            self.sema.act_on_lambda_error(lambda);
            return Err(SubstFail);
        }
        let Some(d) = self.lambda_declarator(call_op, &data, loc) else {
            self.sema.act_on_lambda_error(lambda);
            self.forget_pattern_params(call_op);
            return Err(SubstFail);
        };
        let specs = self.lambda_specifiers(call_op, &data, loc);
        self.sema.act_on_lambda_declarator(lambda, d, &specs);
        let new = self.sema.ast.exprs.lambdas.get(lambda);
        let (closure, new_op) = (new.closure, new.call_op);
        self.locals.insert(data.closure, DeclOrPack::Decl(closure));
        let Some(new_op) = new_op else {
            self.sema.act_on_lambda_error(lambda);
            self.forget_pattern_params(call_op);
            return Err(SubstFail);
        };
        self.locals.insert(call_op, DeclOrPack::Decl(new_op));
        // The pattern call operator's attributes (`always_inline`,
        // `noinline`, `flatten`, `cold`, ...), as for any instantiated
        // function (clang `TransformLambdaExpr` → `InstantiateAttrs`): the
        // rebuilt specifiers carry none. A generic lambda's new call
        // operator is itself a pattern, whose specializations take them
        // from it (`function_spec_attrs`).
        self.class_attrs(new_op, call_op);
        // A generic lambda's dependent access checks are replayed by the
        // instantiations of the new call operator.
        self.sema.copy_dependent_access(call_op, new_op);
        if head.is_some() {
            self.sema.copy_template_arg_uses(call_op, new_op);
        }
        if self.lambda_constraints(head.map(|h| h.0), call_op, new_op, loc).is_err() {
            self.sema.act_on_lambda_error(lambda);
            self.forget_pattern_params(call_op);
            return Err(SubstFail);
        }
        self.lambda_default_args(call_op, new_op);
        let rbrace = self.sema.ast.stmts.rbrace(body);
        // The body is substituted under a frame of its own, so an error in
        // it says "while substituting into a lambda expression here" (at
        // the body's `{`) above the enclosing specialization's notes, and
        // is never a substitution failure (CWG2672; clang
        // `TransformLambdaExpr`, spec/sema-templates.md §10.1).
        let lbrace = self.sema.ast.stmts.get(body).loc();
        if !self.sema.push_inst_frame(FrameKind::LambdaSubstitution, new_op, new_op, lbrace) {
            self.sema.act_on_lambda_error(lambda);
            self.forget_pattern_params(call_op);
            return Err(SubstFail);
        }
        let (errors, body_errors) = (self.sema.diags.error_count(), self.sema.inst.body_errors);
        let checked = self.sema.inst.checked_decl_errors;
        let outer_return_failed = std::mem::take(&mut self.lambda_return_failed);
        self.lambda_bodies += 1;
        let b = self.body(body);
        self.lambda_bodies -= 1;
        let return_failed = std::mem::replace(&mut self.lambda_return_failed, outer_return_failed);
        self.sema.pop_inst_frame();
        self.forget_pattern_params(call_op);
        // A body that reported an error of its own (not one of another
        // body instantiated meanwhile) fails the lambda-expression, as
        // clang's `TransformLambdaExpr` returns `ExprError` for an invalid
        // body: no closure with a `void` call operator is left to cascade
        // ("cannot initialize ... with an rvalue of type 'void'"), and the
        // enclosing statement fails like any other (§5.5). A failed
        // `static_assert` is no such error: its declaration is kept, the
        // body is valid, and a constant evaluation of the lambda goes on
        // to report the next independent error (WP-1247). Nor is a failed
        // consteval block, a declaration of the same kind (WP-1257).
        let nested = self.sema.inst.body_errors.saturating_sub(body_errors)
            + self.sema.inst.checked_decl_errors.saturating_sub(checked);
        // So does a `return` that failed without an error of its own in a
        // call operator with a placeholder return type (clang marks it
        // invalid; `Substituter::lambda_return_failed`).
        if self.sema.diags.error_count().saturating_sub(errors) > nested || return_failed {
            self.sema.act_on_lambda_error(lambda);
            return Err(SubstFail);
        }
        if self.probe_done() {
            // The body reached the pack a probe looks for (the statement
            // failed and was recovered): the probe fails here instead of
            // completing a lambda with a truncated body.
            self.sema.act_on_lambda_error(lambda);
            return Err(SubstFail);
        }
        Ok(self.sema.act_on_lambda_end(lambda, b, rbrace))
    }

    /// The explicit captures of the pattern into `intro`; the
    /// init-capture variables of the pattern, in order, each with
    /// the number of captures an expanded init-capture pack became.
    fn lambda_captures(
        &mut self,
        captures: &[LambdaCapture],
        intro: &mut LambdaIntroducer,
    ) -> SubstResult<Vec<(kiln_ast::DeclId, Option<usize>)>> {
        let mut init_vars = Vec::new();
        for c in captures.iter().filter(|c| !c.implicit) {
            if c.ellipsis.is_valid()
                && let Some(v) = c.var.filter(|_| c.init)
            {
                // An init-capture pack `...xs = init`
                // ([expr.prim.lambda.capture]/17): one init-capture per
                // element of the expanded pattern, all named `xs`; kept
                // as a pack when the expansion stays unexpanded.
                let (i, style) = self.init_capture_init(v)?;
                let name = self.sema.ast.decls.get(v).name().as_ident().map(|s| (s, c.loc));
                let elems = match self.expand_pattern_at(Some(c.ellipsis), |s| {
                    s.initializer(i, style)?.ok_or(SubstFail)
                })? {
                    Expansion::Elements(xs) => {
                        init_vars.push((v, Some(xs.len())));
                        xs.into_iter().map(|x| (None, x)).collect()
                    }
                    Expansion::Unexpanded(x) => {
                        init_vars.push((v, None));
                        vec![(Some(c.ellipsis), x)]
                    }
                };
                for (ellipsis, x) in elems {
                    intro.captures.push(LambdaCaptureSpec {
                        kind: c.kind,
                        name,
                        loc: c.loc,
                        end: Loc::INVALID,
                        ellipsis,
                        init: Some((style, x)),
                        var: Some(v),
                    });
                }
                continue;
            }
            if c.ellipsis.is_valid() {
                // A simple-capture pack `ts...` / `&ts...`
                // ([expr.prim.lambda.capture]/18): one capture per
                // parameter the function parameter pack expanded to.
                // A capture pack names a variable (the parser rejected
                // anything else, so it has none only after an error).
                let Some(pv) = c.var.filter(|_| !c.init) else { return Err(SubstFail) };
                let params = match self.locals.get(&pv) {
                    Some(&DeclOrPack::Pack(l)) => {
                        self.sema.ast.lists.iter::<kiln_ast::DeclId>(l).collect::<Vec<_>>()
                    }
                    // Keep mode: the pack belongs to a retained level (a
                    // generic lambda's parameter pack while the enclosing
                    // template is instantiated) and stays a capture pack.
                    Some(&DeclOrPack::Decl(v)) if self.sema.is_pack_decl(v) => {
                        intro.captures.push(self.capture_pack_kept(c, v));
                        continue;
                    }
                    // Not a local of this substitution: the pack as
                    // instantiated elsewhere (clang `FindInstantiatedDecl`),
                    // or a failed local's (a structured binding pack whose
                    // declaration failed, already diagnosed: the capture
                    // fails too).
                    _ => {
                        let v = self.find_instantiated_decl(pv, c.loc)?;
                        if self.sema.is_pack_decl(v) {
                            intro.captures.push(self.capture_pack_kept(c, v));
                            continue;
                        }
                        vec![v]
                    }
                };
                for v in params {
                    let name = self.sema.ast.decls.get(v).name().as_ident().map(|s| (s, c.loc));
                    intro.captures.push(LambdaCaptureSpec {
                        kind: c.kind,
                        name,
                        loc: c.loc,
                        end: Loc::INVALID,
                        ellipsis: None,
                        init: None,
                        var: Some(v),
                    });
                }
                continue;
            }
            let name = c
                .var
                .and_then(|v| self.sema.ast.decls.get(v).name().as_ident())
                .map(|s| (s, c.loc));
            let init = match c.var.filter(|_| c.init) {
                Some(v) => {
                    let (i, style) = self.init_capture_init(v)?;
                    init_vars.push((v, None));
                    match self.initializer(i, style)? {
                        Some(x) => Some((style, x)),
                        // `initializer` is `None` only for a
                        // default-initialized variable (a construction with
                        // neither parentheses nor arguments).
                        None => unreachable!(
                            "an init-capture's initializer is written: its type is deduced \
                             from it ([expr.prim.lambda.capture]/6), and one without is invalid \
                             and has no `init`"
                        ),
                    }
                }
                None => None,
            };
            // A simple capture names its variable again directly: the
            // name may not be in scope (a nested lambda instantiated with
            // its enclosing generic lambda's call operator).
            // An init-capture names the pattern's variable (no duplicate
            // check: an expanded pack's elements share a name).
            let var = match c.var {
                Some(v) if c.init => Some(v),
                Some(v) => Some(self.find_instantiated_decl(v, c.loc)?),
                None => None,
            };
            intro.captures.push(LambdaCaptureSpec {
                kind: c.kind,
                name,
                loc: c.loc,
                end: Loc::INVALID,
                ellipsis: None,
                init,
                var,
            });
        }
        Ok(init_vars)
    }

    /// Simple-capture pack `c` kept as a capture pack of `v` (a pack of
    /// a level not substituted yet).
    fn capture_pack_kept(&self, c: &LambdaCapture, v: kiln_ast::DeclId) -> LambdaCaptureSpec {
        let name = self.sema.ast.decls.get(v).name().as_ident().map(|s| (s, c.loc));
        LambdaCaptureSpec {
            kind: c.kind,
            name,
            loc: c.loc,
            end: Loc::INVALID,
            ellipsis: Some(c.ellipsis),
            init: None,
            var: Some(v),
        }
    }

    /// The initializer of pattern init-capture variable `v` with its
    /// syntax ([expr.prim.lambda.capture]/6: as for a variable).
    fn init_capture_init(&self, v: kiln_ast::DeclId) -> SubstResult<(ExprId, InitStyle)> {
        let x = self.sema.ast.decls.var(v).ok_or(SubstFail)?;
        Ok((x.init.ok_or(SubstFail)?, x.spec.init_style()))
    }

    /// The lambda-declarator of pattern call operator `call_op`: the
    /// parameters (created now, mapped from the pattern's), the
    /// exception specification and the trailing return type
    /// substituted. `None` after a failure (diagnosed).
    fn lambda_declarator(
        &mut self,
        call_op: kiln_ast::DeclId,
        data: &kiln_ast::lambda::LambdaData,
        loc: Loc,
    ) -> Option<Declarator> {
        let t = self.decl_ty(call_op);
        let TypeView::Function(p) = self.sema.ast.types.view(t.ty()) else { return None };
        let info = p.info();
        let params = self.param_infos(call_op).ok()?;
        let trailing =
            if data.explicit_result { Some((self.ty(p.ret(), loc).ok()?, loc)) } else { None };
        // The noexcept-specifier is substituted with the declaration but
        // is never in the immediate context of a deduction that reached
        // the lambda ([temp.deduct.general]/8, P4149R1): inside a SFINAE
        // context an error in it is an error, under an exception
        // specification frame. (clang 22 still treats it as a
        // substitution failure; outside SFINAE nothing changes.)
        let dependent = matches!(
            self.sema.ast.types.exc_spec(&p),
            kiln_ast::types::ExcSpec::NoexceptExpr {
                value: kiln_ast::types::NoexceptValue::Dependent(_),
                ..
            } | kiln_ast::types::ExcSpec::DependentNoexcept(_)
        );
        let framed = dependent
            && self.sema.inst.stack.is_sfinae()
            && self.sema.push_inst_frame(FrameKind::ExceptionSpec, call_op, call_op, loc);
        let exc = self.exc_spec(&p, call_op, loc);
        if framed {
            self.sema.pop_inst_frame();
        }
        let exc = exc.ok()?;
        let mut d = Declarator::new(DeclaratorContext::Member, data.lbracket);
        d.op_name = Some(OverloadedOperator::Call);
        // A lambda's call operator: its return type is the implied `auto`
        // of [expr.prim.lambda.closure]/3, not a written decl-specifier,
        // so the C++11 deduced-return gate exempts it as in the pattern
        // (WP-1213).
        d.lambda = true;
        // The trailing requires-clause, unsubstituted (checked through
        // `member_constraints`).
        d.trailing_requires = self.sema.ast.templates.trailing_requires.get(call_op).copied();
        d.chunks.push(Chunk::Function {
            params,
            variadic: info.variadic(),
            loc: data.lbracket,
            rparen: data.lbracket,
            exc,
            quals: Quals::NONE,
            ref_qual: RefQual::None,
            trailing,
            ambiguous: false,
            no_proto: false,
        });
        Some(d)
    }

    /// The lambda-specifier-seq of the pattern lambda `data` with call
    /// operator `call_op`.
    fn lambda_specifiers(
        &self,
        call_op: kiln_ast::DeclId,
        data: &kiln_ast::lambda::LambdaData,
        loc: Loc,
    ) -> LambdaSpecifiers {
        let ast = &self.sema.ast;
        let spec = ast.decls.function(call_op).map(|f| f.spec);
        // `constexpr` as written shows on the static invoker only.
        let explicit_constexpr = ast
            .closure_invoker(data.closure)
            .and_then(|i| ast.decls.function(i))
            .is_some_and(|f| f.spec.constexpr_kind() == ConstexprKind::Constexpr);
        LambdaSpecifiers {
            mutable: data.mutable.then_some(loc),
            constexpr: explicit_constexpr.then_some(loc),
            consteval: spec
                .is_some_and(|s| s.constexpr_kind() == ConstexprKind::Consteval)
                .then_some(loc),
            static_: spec.is_some_and(|s| s.storage() == FnStorage::Static).then_some(loc),
            explicit_params: data.explicit_params,
            attrs: Vec::new(),
        }
    }

    /// Maps the init-capture variables of pattern call operator `pat` to
    /// those of `new`, the call operator substituted from it, for a
    /// constraint of `new` (`entity`, `Sema::constraint_lambda_captures`)
    /// that names them (P2036, [basic.scope.lambda];
    /// clang `LambdaScopeForCallOperatorInstantiationRAII` adds the
    /// instantiated captures to the scope). An expanded init-capture pack
    /// `...xs = init` maps to its elements, the run of captures named `xs`
    /// in `new` ([expr.prim.lambda.capture]/17).
    pub(crate) fn seed_init_captures(&mut self, entity: kiln_ast::DeclId) {
        let Some((pat, new)) = self.sema.constraint_lambda_captures(entity) else { return };
        let (Some(lp), Some(ln)) =
            (self.sema.lambda_of_call_op(pat), self.sema.lambda_of_call_op(new))
        else {
            return;
        };
        let inits = |s: &Self, l| -> Vec<(kiln_ast::DeclId, bool)> {
            let caps = &s.sema.ast.exprs.lambdas.get(l).captures;
            caps.iter()
                .filter(|c| c.init)
                .filter_map(|c| Some((c.var?, c.ellipsis.is_valid())))
                .collect()
        };
        let (olds, news) = (inits(self, lp), inits(self, ln));
        let mut j = 0;
        let mut map = Vec::with_capacity(olds.len());
        for (o, pack) in olds {
            if !pack || news.get(j).is_some_and(|&(_, p)| p) {
                let Some(&(n, _)) = news.get(j) else { return };
                j += 1;
                map.push((o, DeclOrPack::Decl(n)));
                continue;
            }
            let name = self.sema.ast.decls.get(o).name();
            let start = j;
            while news.get(j).is_some_and(|&(n, p)| !p && self.sema.ast.decls.get(n).name() == name)
            {
                j += 1;
            }
            let elems: Vec<kiln_ast::DeclId> = news[start..j].iter().map(|x| x.0).collect();
            map.push((o, DeclOrPack::Pack(self.sema.ast.lists.push(&elems))));
        }
        for (o, n) in map {
            if !matches!(n, DeclOrPack::Decl(d) if d == o) {
                self.locals.insert(o, n);
            }
        }
    }

    /// Maps the parameters of the functions around the pattern of
    /// constrained `entity` (a lambda's call operator or its template, a
    /// member function of a local class) to those of the functions around
    /// `entity`, outward while the two contexts correspond: a constraint
    /// may name a parameter of an enclosing generic lambda in an
    /// unevaluated operand (`[](auto v) { return []() requires
    /// C<decltype(v)> {}; }`), which names the enclosing specialization's
    /// parameter ([temp.constr.atomic]/3; clang's
    /// `LambdaScopeForCallOperatorInstantiationRAII` adds the enclosing
    /// lambdas' instantiated parameters to the scope).
    pub(crate) fn seed_enclosing_params(&mut self, entity: kiln_ast::DeclId) {
        let Some(&(pat, _)) = self.sema.inst.concepts.member_constraints.get(&entity) else {
            return;
        };
        let function_of = |s: &Self, d: kiln_ast::DeclId| match s.sema.kind_of(d) {
            DeclKind::FunctionTemplate => s.sema.templated_function(d),
            k if k.is_function() => Some(d),
            _ => None,
        };
        let (Some(mut p), Some(mut n)) = (function_of(self, pat), function_of(self, entity)) else {
            return;
        };
        loop {
            let (Some(pc), Some(nc)) =
                (self.sema.ast.decls.get(p).ctx(), self.sema.ast.decls.get(n).ctx())
            else {
                return;
            };
            (p, n) = (pc, nc);
            if p == n || self.sema.kind_of(p) != self.sema.kind_of(n) {
                return;
            }
            if self.sema.kind_of(p).is_function() {
                self.map_enclosing_params(p, n);
            }
        }
    }

    /// Maps pattern function `p`'s parameters to those of `n`, a
    /// specialization of it: one to one, a trailing pack to the
    /// parameters it expanded to. Parameters already mapped stay.
    fn map_enclosing_params(&mut self, p: kiln_ast::DeclId, n: kiln_ast::DeclId) {
        let params = |s: &Self, f: kiln_ast::DeclId| -> Vec<kiln_ast::DeclId> {
            s.sema
                .ast
                .decls
                .function(f)
                .map(|x| s.sema.ast.lists.iter::<kiln_ast::DeclId>(x.params).collect())
                .unwrap_or_default()
        };
        let (pp, np) = (params(self, p), params(self, n));
        let is_pack = |s: &Self, d: kiln_ast::DeclId| {
            s.sema
                .ast
                .decls
                .param(d)
                .is_some_and(|x| x.flags & kiln_ast::decl::ParamData::PACK != 0)
        };
        let lead = match pp.split_last() {
            Some((&last, lead))
                if is_pack(self, last) && !lead.iter().any(|&d| is_pack(self, d)) =>
            {
                if np.len() < lead.len() {
                    return;
                }
                let elems = self.sema.ast.lists.push(&np[lead.len()..]);
                self.locals.entry(last).or_insert(DeclOrPack::Pack(elems));
                lead.len()
            }
            _ if pp.len() == np.len() && !pp.iter().any(|&d| is_pack(self, d)) => pp.len(),
            _ => return,
        };
        for (&o, &x) in pp.iter().zip(&np).take(lead) {
            self.locals.entry(o).or_insert(DeclOrPack::Decl(x));
        }
    }

    /// The call operator's constraints stay the pattern's, checked with
    /// this instantiation's levels outside the operator's own (as a class
    /// template member's, `ConceptState::member_constraints`,
    /// spec/sema-templates.md).
    fn lambda_constraints(
        &mut self,
        pattern_template: Option<kiln_ast::DeclId>,
        call_op: kiln_ast::DeclId,
        new_op: kiln_ast::DeclId,
        loc: Loc,
    ) -> SubstResult<()> {
        let (pat, new) = match pattern_template {
            Some(t) => match self.sema.ast.described_template(new_op) {
                Some(x) if !self.sema.associated_constraints(t).is_empty() => (t, x),
                _ => return Ok(()),
            },
            None if self.sema.ast.templates.trailing_requires.get(call_op).is_some() => {
                (call_op, new_op)
            }
            None => return Ok(()),
        };
        let args = self.lambda_constraint_levels(pat, loc)?;
        self.record_member_constraints(new, pat, args, loc)
    }

    /// Records that `new`, rebuilt from `pat`, has `pat`'s constraints
    /// checked with `args` outside its own level
    /// (`ConceptState::member_constraints`). A pattern that is itself
    /// rebuilt from an enclosing template's (a lambda or a local class in
    /// a generic lambda in a template, `[]<class U>(U) { return []<class
    /// V>(V) requires C<T, U, V> {}; }`: the inner lambda was rebuilt when
    /// the template was instantiated, with `U` retained) keeps the
    /// original's constraints, so they are checked with the levels that
    /// pattern recorded, its retained levels this instantiation's
    /// ([temp.constr.decl]/3, [temp.constr.atomic]/3): else the
    /// parameters of the levels in between stay dependent.
    pub(crate) fn record_member_constraints(
        &mut self,
        new: kiln_ast::DeclId,
        pat: kiln_ast::DeclId,
        args: MultiLevelArgs,
        loc: Loc,
    ) -> SubstResult<()> {
        let (pat, args) = match self.sema.inst.concepts.member_constraints.get(&pat).cloned() {
            Some((p0, a0)) if p0 != pat => (p0, self.compose_levels(a0, args, loc)?),
            _ => (pat, args),
        };
        self.sema.inst.concepts.member_constraints.insert(new, (pat, args));
        Ok(())
    }

    /// `outer`, levels recorded against an original pattern whose
    /// retained levels (explicit, or implicit past its end) are the levels
    /// of the rebuilt pattern being substituted with `inner`: `outer`'s
    /// substituted levels (their arguments substituted with `inner` where
    /// they still name its parameters), each retained one replaced by
    /// `inner`'s next level, then `inner`'s remaining levels.
    fn compose_levels(
        &mut self,
        outer: MultiLevelArgs,
        inner: MultiLevelArgs,
        loc: Loc,
    ) -> SubstResult<MultiLevelArgs> {
        let mut rest = inner.levels.into_iter();
        let mut out = MultiLevelArgs::new();
        for l in outer.levels {
            match l.mode {
                LevelMode::Substitute => {
                    let args = if self.sema.levels_dependent(&MultiLevelArgs { levels: vec![l] }) {
                        self.args(l.args, loc)?
                    } else {
                        l.args
                    };
                    out.push(args, l.assoc, LevelMode::Substitute);
                }
                LevelMode::Retain => out.levels.push(rest.next().unwrap_or(l)),
            }
        }
        out.levels.extend(rest);
        Ok(out)
    }

    /// The levels the constraints of pattern `pat` (the call operator or
    /// its template) are checked with. A pack of an enclosing template
    /// that the constraints name unexpanded makes the lambda-expression
    /// contain it ([temp.variadic]/5), so an expansion around the lambda
    /// expands it even when nothing else in the lambda names it: it is
    /// reached here (a probe stops at it, [`Self::pack_reached`]), and
    /// its argument becomes the element being substituted, so
    /// `[]<class U>() requires C<U, Ts> {}...` checks `C<U, Ti>` for the
    /// `i`th closure.
    fn lambda_constraint_levels(
        &mut self,
        pat: kiln_ast::DeclId,
        loc: Loc,
    ) -> SubstResult<MultiLevelArgs> {
        let mut args = self.args.clone();
        if self.pack.keep && self.pack_index.is_none() {
            return Ok(args);
        }
        let exprs = if self.sema.ast.template_info(pat).is_some() {
            self.sema.associated_constraints(pat)
        } else {
            self.sema.ast.templates.trailing_requires.get(pat).copied().into_iter().collect()
        };
        let mut packs = Vec::new();
        for e in exprs {
            for p in self.sema.unexpanded_packs(PackOperand::Expr(e)) {
                if !packs.contains(&p) {
                    packs.push(p);
                }
            }
        }
        for p in packs {
            let Some(info) = self.sema.ast.template_parm(p).copied() else { continue };
            let Some(level) = args.levels.get(usize::from(info.depth)) else { continue };
            if level.mode != LevelMode::Substitute {
                continue;
            }
            let level_args = level.args;
            let mut list: Vec<TemplateArg> = self.sema.ast.types.args(level_args).to_vec();
            let Some(slot) = list.get_mut(usize::from(info.index)) else { continue };
            let ArgView::Pack(l) = slot.view() else { continue };
            let elems = self.sema.ast.types.args(l);
            let n = u32::try_from(elems.len()).unwrap_or(u32::MAX);
            let i = self.pack_reached(n, |s| s.parm_pack_name(Some(p)), loc)?;
            let Some(&elem) = self.sema.ast.types.args(l).get(i as usize) else {
                return Err(SubstFail);
            };
            *slot = elem;
            let new_args = self.sema.ast.types.arg_list(&list);
            if let Some(level) = args.levels.get_mut(usize::from(info.depth)) {
                level.args = new_args;
            }
        }
        Ok(args)
    }

    /// The default arguments of pattern call operator `pat` on `f`; the
    /// pattern's parameters stay mapped to `f`'s for the body.
    fn lambda_default_args(&mut self, pat: kiln_ast::DeclId, f: kiln_ast::DeclId) {
        let params = |s: &Self, x: kiln_ast::DeclId| -> Vec<kiln_ast::DeclId> {
            s.sema
                .ast
                .decls
                .function(x)
                .map(|fd| s.sema.ast.lists.iter::<kiln_ast::DeclId>(fd.params).collect())
                .unwrap_or_default()
        };
        let pp = params(self, pat);
        let fp = params(self, f);
        if pp.iter().any(|&p| self.sema.ast.decls.param(p).is_some_and(|x| x.default_arg.is_some()))
        {
            self.default_args(pat, f);
        }
        self.map_params(&pp, &fp);
    }

    /// Inside a lambda body being rebuilt, a shared (non-dependent)
    /// expression still uses `this` and the enclosing variables in the
    /// new closure: each potentially evaluated `this` in it is captured
    /// (clang `MarkThisReferenced` on an unchanged `CXXThisExpr`), and so
    /// is each variable it names, as a fresh reference would capture it
    /// (clang `MarkDeclRefReferenced` on an unchanged `DeclRefExpr`):
    /// at once if it cannot be usable in constant expressions, else at
    /// the end of the full-expression (the shared reference's
    /// `pending_odr` entry, or a potential capture while the rebuilt body
    /// is still dependent; WP-581).
    pub(crate) fn note_shared_captures(&mut self, e: ExprId) {
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            let n = *self.sema.ast.exprs.get(x);
            match n.kind() {
                ExprKind::CXXThisExpr => self.sema.note_this_use(n.loc()),
                ExprKind::DeclRefExpr => {
                    let d = self.sema.ast.exprs.decl(x);
                    if matches!(
                        self.sema.kind_of(d),
                        DeclKind::Var | DeclKind::ParmVar | DeclKind::Binding
                    ) {
                        let _ = self.sema.capture_at_decl_ref(d, n.loc());
                        if self.sema.in_template() && self.sema.unevaluated == 0 {
                            self.sema.note_potential_capture(x, d);
                        }
                    }
                }
                // Unevaluated operands and nested closures (their own
                // captures are complete) are not uses.
                ExprKind::UnaryExprOrTypeTraitExpr
                | ExprKind::CXXTypeidExpr
                | ExprKind::CXXNoexceptExpr
                | ExprKind::LambdaExpr
                | ExprKind::RequiresExpr => {}
                _ => self.sema.ast.exprs.children_into(x, &self.sema.ast.lists, &mut stack),
            }
        }
    }
}
