//! Substitution into a lambda-expression ([temp.inst]/2,
//! [expr.prim.lambda.closure]/2; clang `TreeTransform::TransformLambdaExpr`,
//! spec/sema-lambda.md §9): each instantiation of the enclosing template
//! gets a closure type of its own. The lambda is replayed through the
//! actions the parser calls: the explicit captures (init-capture
//! initializers substituted), the call operator's signature with
//! substituted parameter and trailing return types, then the body, whose
//! rebuilt uses capture implicitly again.

use kiln_ast::ExprId;
use kiln_ast::decl::{ConstexprKind, FnStorage, InitStyle};
use kiln_ast::expr::ExprKind;
use kiln_ast::names::OverloadedOperator;
use kiln_ast::types::{Quals, RefQual, TypeView};
use kiln_source::Loc;

use super::{DeclOrPack, SubstFail, SubstResult, Substituter};
use crate::declspec::{Chunk, Declarator, DeclaratorContext};
use crate::init::Initializer;
use crate::lambda::{LambdaCaptureSpec, LambdaIntroducer, LambdaSpecifiers};

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
        let mut init_vars = Vec::new();
        for c in data.captures.iter().filter(|c| !c.implicit) {
            if c.ellipsis.is_valid() {
                // A simple-capture pack `ts...` / `&ts...`
                // ([expr.prim.lambda.capture]/18): one capture per
                // parameter the function parameter pack expanded to.
                let params = match c.var.filter(|_| !c.init).and_then(|v| self.locals.get(&v)) {
                    Some(&DeclOrPack::Pack(l)) => {
                        self.sema.ast.lists.iter::<kiln_ast::DeclId>(l).collect::<Vec<_>>()
                    }
                    _ => return self.unimplemented("capture packs in templates", c.loc),
                };
                for v in params {
                    let name = self.sema.ast.decls.get(v).name().as_ident().map(|s| (s, c.loc));
                    intro.captures.push(LambdaCaptureSpec {
                        kind: c.kind,
                        name,
                        loc: c.loc,
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
                    let i = self.sema.ast.decls.var(v).and_then(|x| x.init).ok_or(SubstFail)?;
                    init_vars.push(v);
                    match self.initializer(i, InitStyle::C)? {
                        Some(Initializer::Expr(x)) => Some(x),
                        _ => return self.unimplemented("this init-capture in a template", c.loc),
                    }
                }
                None => None,
            };
            // A simple capture names its variable again directly: the
            // name may not be in scope (a nested lambda instantiated with
            // its enclosing generic lambda's call operator).
            let var = match c.var.filter(|_| !c.init) {
                Some(v) => Some(self.find_instantiated_decl(v, c.loc)?),
                None => None,
            };
            intro.captures.push(LambdaCaptureSpec {
                kind: c.kind,
                name,
                loc: c.loc,
                ellipsis: None,
                init,
                var,
            });
        }
        let lambda = self.sema.act_on_lambda_start(&intro);
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
        // A generic lambda's dependent access checks are replayed by the
        // instantiations of the new call operator.
        self.sema.copy_dependent_access(call_op, new_op);
        self.lambda_constraints(head.map(|h| h.0), call_op, new_op);
        self.lambda_default_args(call_op, new_op);
        // The init-capture variables, by name.
        for v in init_vars {
            let name = self.sema.ast.decls.get(v).name();
            let caps = self.sema.ast.exprs.lambdas.get(lambda).captures.clone();
            if let Some(x) = caps
                .iter()
                .filter(|c| c.init)
                .filter_map(|c| c.var)
                .find(|&x| self.sema.ast.decls.get(x).name() == name)
            {
                self.locals.insert(v, DeclOrPack::Decl(x));
            }
        }
        let rbrace = self.sema.ast.stmts.rbrace(body);
        self.lambda_bodies += 1;
        let b = self.body(body);
        self.lambda_bodies -= 1;
        self.forget_pattern_params(call_op);
        if self.probe_done() {
            // The body reached the pack a probe looks for (the statement
            // failed and was recovered): the probe fails here instead of
            // completing a lambda with a truncated body.
            self.sema.act_on_lambda_error(lambda);
            return Err(SubstFail);
        }
        Ok(self.sema.act_on_lambda_end(lambda, b, rbrace))
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
        let exc = self.exc_spec(&p, loc).ok()?;
        let mut d = Declarator::new(DeclaratorContext::Member, data.lbracket);
        d.op_name = Some(OverloadedOperator::Call);
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

    /// The default arguments of pattern call operator `pat` on `f`; the
    /// pattern's parameters stay mapped to `f`'s for the body.
    /// The call operator's constraints stay the pattern's, checked with
    /// this instantiation's levels outside the operator's own (as a class
    /// template member's, `ConceptState::member_constraints`,
    /// spec/sema-templates.md).
    fn lambda_constraints(
        &mut self,
        pattern_template: Option<kiln_ast::DeclId>,
        call_op: kiln_ast::DeclId,
        new_op: kiln_ast::DeclId,
    ) {
        let (pat, new) = match pattern_template {
            Some(t) => match self.sema.ast.described_template(new_op) {
                Some(x) if !self.sema.associated_constraints(t).is_empty() => (t, x),
                _ => return,
            },
            None if self.sema.ast.templates.trailing_requires.get(call_op).is_some() => {
                (call_op, new_op)
            }
            None => return,
        };
        let args = self.args.clone();
        self.sema.inst.concepts.member_constraints.insert(new, (pat, args));
    }

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
    /// expression still uses `this` in the new closure: each potentially
    /// evaluated `this` in it is captured (clang `MarkThisReferenced` on
    /// an unchanged `CXXThisExpr`).
    pub(crate) fn note_shared_this(&mut self, e: ExprId) {
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            let n = *self.sema.ast.exprs.get(x);
            match n.kind() {
                ExprKind::CXXThisExpr => self.sema.note_this_use(n.loc()),
                // Unevaluated operands and nested closures (their own
                // captures are complete) are not uses.
                ExprKind::UnaryExprOrTypeTraitExpr
                | ExprKind::CXXTypeidExpr
                | ExprKind::CXXNoexceptExpr
                | ExprKind::LambdaExpr
                | ExprKind::RequiresExpr => {}
                _ => stack.extend(self.sema.ast.exprs.children(x, &self.sema.ast.lists)),
            }
        }
    }
}
