//! Constrained templates (spec/abi.md §8.16, WP-204): the
//! `<template-param-decl>` forms `Ty`/`Tk`/`Tt` (clang
//! `mangleTemplateParamDecl`, cxx-abi issues 24 and 47), `Dk`/`DK`
//! placeholder types, requires-clauses (`Q <expression>`) after a
//! function template's arguments and after a function's bare type, and
//! the constraint expressions they contain: concept-ids as unresolved
//! names, fold-expressions, requires-expressions and references to
//! function and requires-expression parameters (`fp`/`fL`).
//!
//! Template parameters inside a constraint keep their declared depth
//! (constraints are not substituted when a member is instantiated), so
//! they mangle with it: `TL0__` for the first parameter at depth 1, as
//! clang's `mangleTemplateParameter`.

use super::{AnonIds, Mangler, Structor};
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::{BinaryOp, ExprDeps, ExprKind};
use kiln_ast::ids::{ArgListId, DeclId, ExprId, TemplateParamListId};
use kiln_ast::templates::Requirement;
use kiln_ast::types::{ArgView, TemplateArg, TemplateName};

impl Mangler<'_, '_> {
    /// The concept and written arguments of type parameter `p`'s
    /// immediately-declared constraint (a `ConceptSpecializationExpr`,
    /// or a fold over one for a pack), without the implicit first
    /// argument.
    fn parm_constraint(&self, p: DeclId) -> Option<(DeclId, Vec<ArgView>)> {
        let ast = self.ast;
        let mut c = ast.template_parm(p)?.constraint?;
        if ast.exprs.get(c).kind() == ExprKind::CXXFoldExpr {
            c = ExprId::try_new(ast.exprs.get(c).a())?;
        }
        let x = ast.exprs.get(c);
        if x.kind() != ExprKind::ConceptSpecializationExpr {
            return None;
        }
        let concept = DeclId::try_new(x.a())?;
        let args = ArgListId::try_new(x.b())
            .map(|l| ast.types.args(l).iter().skip(1).map(TemplateArg::view).collect())
            .unwrap_or_default();
        Some((concept, args))
    }

    /// Whether type parameter `p` has a type-constraint.
    pub(super) fn is_constrained_type_parm(&self, p: DeclId) -> bool {
        self.ast.template_parm(p).is_some_and(|x| x.constraint.is_some())
    }

    /// <type-constraint> (clang `mangleTypeConstraint`): the concept's
    /// template name and arguments, or, without arguments, its unscoped
    /// (not substitutable) or nested name.
    pub(super) fn type_constraint(&mut self, concept: DeclId, args: &[ArgView]) {
        let saved = std::mem::replace(&mut self.real_depth, true);
        let at_top = self.parent(concept).is_none_or(|p| {
            self.ast.decls.get(p).kind() == DeclKind::TranslationUnit || self.is_std(p)
        });
        let std = self.parent(concept).is_some_and(|p| self.is_std(p));
        if !args.is_empty() {
            if at_top {
                self.unscoped_template(concept, std, None);
            } else {
                self.out.push('N');
                self.template_prefix(concept, None, Structor::Complete, None);
            }
            self.arg_views(args);
            if !at_top {
                self.out.push('E');
            }
        } else if at_top {
            if std {
                self.out.push_str("St");
            }
            self.unqualified(concept, Structor::Complete);
        } else {
            self.out.push('N');
            if let Some(p) = self.parent(concept) {
                self.prefix(p, None);
            }
            self.unqualified(concept, Structor::Complete);
            self.out.push('E');
        }
        self.real_depth = saved;
    }

    /// `I <template-arg>* E` from argument views.
    fn arg_views(&mut self, args: &[ArgView]) {
        self.out.push('I');
        for a in args {
            self.template_arg(*a);
        }
        self.out.push('E');
    }

    /// <template-param-decl> of type or template template parameter `p`
    /// (non-type parameters are [`Mangler::template_param_decl`]'s).
    pub(super) fn type_or_template_param_decl(&mut self, p: DeclId) {
        let ast = self.ast;
        let pack = ast.template_parm(p).is_some_and(|x| x.pack);
        match ast.decls.get(p).kind() {
            DeclKind::TemplateTypeParm => {
                if pack {
                    self.out.push_str("Tp");
                }
                if let Some((c, args)) = self.parm_constraint(p) {
                    self.out.push_str("Tk");
                    self.type_constraint(c, &args);
                } else {
                    self.out.push_str("Ty");
                }
            }
            DeclKind::TemplateTemplateParm => {
                if pack {
                    self.out.push_str("Tp");
                }
                match ast.template_parm(p).and_then(|x| x.params) {
                    Some(l) => self.template_param_list(l),
                    None => self.out.push_str("TtE"),
                }
            }
            _ => self.template_param_decl(p),
        }
    }

    /// `Tt <template-param-decl>* [Q <requires-clause>] E` (clang
    /// `mangleTemplateParameterList`).
    fn template_param_list(&mut self, l: TemplateParamListId) {
        let ast = self.ast;
        let list = *ast.templates.param_list(l);
        self.out.push_str("Tt");
        let params: Vec<DeclId> = ast.lists.iter::<DeclId>(list.params).collect();
        for p in params {
            self.type_or_template_param_decl(p);
        }
        if let Some(r) = list.requires {
            self.requires_clause(r, false);
        }
        self.out.push('E');
    }

    /// Whether template template parameter `parm`'s template-head differs
    /// from that of the template argument `arg` (clang
    /// `needToMangleTemplateParam`; heads are compared by their mangling,
    /// ignoring depth).
    pub(super) fn template_head_differs(&mut self, parm: DeclId, arg: ArgView) -> bool {
        let ast = self.ast;
        let name = match arg {
            ArgView::Pack(l) => match ast.types.args(l).first() {
                None => return true,
                Some(x) => return self.template_head_differs(parm, x.view()),
            },
            ArgView::Template(n) | ArgView::TemplateExpansion { name: n, .. } => n,
            _ => return true,
        };
        let name = ast.types.canonical_template_name(name);
        let TemplateName::Template(t) = ast.types.template_name_view(name) else { return true };
        let (Some(pl), Some(info)) =
            (ast.template_parm(parm).and_then(|x| x.params), ast.template_info(t))
        else {
            return true;
        };
        let head = |anon: &mut AnonIds, list: TemplateParamListId| {
            let mut m = Mangler::new(ast, anon, "");
            m.head_only = true;
            m.template_param_list(list);
            m.out
        };
        let ph = head(self.anon, pl);
        let ah = head(self.anon, info.params);
        ph != ah
    }

    /// `Q <expression>`: a requires-clause.
    /// A trailing clause (after the bare function type) is mangled one
    /// function type deeper, as its result type; a template-head's at
    /// the depth of its context, the function encoding's name (clang's
    /// `FunctionTypeDepth`, for [`Mangler::function_param_ref`]).
    pub(super) fn requires_clause(&mut self, r: ExprId, trailing: bool) {
        let saved_fn = self.fn_depth;
        if trailing {
            self.fn_depth = (saved_fn.0 + 1, true);
        }
        let saved_depth = std::mem::replace(&mut self.real_depth, true);
        let saved_failed = self.failed.take();
        self.out.push('Q');
        if !self.expression(r) {
            // No encoding: a hard error at the innermost unsupported
            // subexpression (clang's "cannot yet mangle expression
            // type"), never a silently wrong name.
            let at = self.failed.unwrap_or(r);
            self.anon.unmangleable.push(at);
        }
        self.failed = saved_failed;
        self.real_depth = saved_depth;
        self.fn_depth = saved_fn;
    }

    /// The requires-clause of function template `t`'s template-head, when
    /// one exists.
    pub(super) fn head_requires(&self, t: DeclId) -> Option<ExprId> {
        let info = self.ast.template_info(t)?;
        self.ast.templates.param_list(info.params).requires
    }

    /// A `ConceptSpecializationExpr` in an expression: an unresolved name
    /// `<source-name> <template-args>` with the arguments as written
    /// (clang 18+; a converted pack is spelled out).
    pub(super) fn concept_id_expr(&mut self, e: ExprId) -> bool {
        let ast = self.ast;
        let x = ast.exprs.get(e);
        let Some(concept) = DeclId::try_new(x.a()) else { return false };
        if let Some(&nns) = ast.templates.expr_nns.get(e)
            && !self.unresolved_prefix(nns, false)
        {
            return false;
        }
        let name = super::decl_name(ast, concept);
        self.source_name(&name);
        let Some(l) = ArgListId::try_new(x.b()) else { return true };
        let mut flat = Vec::new();
        for a in ast.types.args(l) {
            match a.view() {
                ArgView::Pack(p) => flat.extend(ast.types.args(p).iter().map(TemplateArg::view)),
                v => flat.push(v),
            }
        }
        self.arg_views(&flat);
        true
    }

    /// A fold-expression: `fl`/`fr` (unary) or `fL`/`fR` (binary), the
    /// operator, then the operands as written (clang `CXXFoldExprClass`).
    pub(super) fn fold_expr(&mut self, e: ExprId) -> bool {
        let ast = self.ast;
        let x = ast.exprs.get(e);
        let (lhs, rhs) = (ExprId::try_new(x.a()), ExprId::try_new(x.b()));
        let Some(op) = BinaryOp::from_u8(x.aux()) else { return false };
        let right =
            lhs.is_some_and(|l| ast.exprs.get(l).deps().contains(ExprDeps::UNEXPANDED_PACK));
        let init = lhs.is_some() && rhs.is_some();
        self.out.push_str(match (right, init) {
            (false, false) => "fl",
            (false, true) => "fL",
            (true, false) => "fr",
            (true, true) => "fR",
        });
        let code = super::dep::binary_code(op);
        self.out.push_str(code);
        lhs.is_none_or(|l| self.expression(l)) && rhs.is_none_or(|r| self.expression(r))
    }

    /// A requires-expression: `rQ <parameter types> _ <requirement>+ E`
    /// with a parameter list, else `rq <requirement>+ E` (clang
    /// `RequiresExprClass`, `mangleRequirement`).
    pub(super) fn requires_expr(&mut self, e: ExprId) -> bool {
        let ast = self.ast;
        let Some(data) = ast.requires_of(e) else { return false };
        let params: Vec<DeclId> = ast.lists.iter::<DeclId>(data.params).collect();
        let reqs: Vec<Requirement> = ast.templates.requirements(data).to_vec();
        let scoped = !params.is_empty();
        // A parameter list is a function prototype scope: its types are
        // mangled one function type deeper, the requirements as its
        // result type (clang `RequiresExprClass`).
        let saved_fn = self.fn_depth;
        if scoped {
            self.out.push_str("rQ");
            self.fn_depth = (saved_fn.0 + 1, false);
            for p in params {
                let q = ast.decls.get(p).ty().unwrap_or(kiln_ast::ids::QualType::INT);
                self.ty(self.param_type(q));
            }
            self.out.push('_');
            self.fn_depth.1 = true;
        } else {
            self.out.push_str("rq");
        }
        let mut ok = true;
        for r in reqs {
            ok &= match r {
                Requirement::Simple(x) => {
                    self.out.push('X');
                    self.expression(x)
                }
                Requirement::Compound { expr, noexcept, constraint } => {
                    self.out.push('X');
                    let mut ok = self.expression(expr);
                    if noexcept.is_some() {
                        self.out.push('N');
                    }
                    if let Some(c) = constraint {
                        self.out.push('R');
                        ok &= self.compound_constraint(c);
                    }
                    ok
                }
                Requirement::Type(t, _) => {
                    self.out.push('T');
                    self.ty(t);
                    true
                }
                Requirement::Nested(x) => {
                    self.out.push('Q');
                    self.expression(x)
                }
            };
            if !ok {
                break;
            }
        }
        self.fn_depth = saved_fn;
        self.out.push('E');
        ok
    }

    /// The type-constraint of a return-type-requirement (its
    /// immediately-declared constraint, without the implicit argument).
    fn compound_constraint(&mut self, c: ExprId) -> bool {
        let ast = self.ast;
        let x = ast.exprs.get(c);
        if x.kind() != ExprKind::ConceptSpecializationExpr {
            return false;
        }
        let Some(concept) = DeclId::try_new(x.a()) else { return false };
        let args: Vec<ArgView> = ArgListId::try_new(x.b())
            .map(|l| ast.types.args(l).iter().skip(1).map(TemplateArg::view).collect())
            .unwrap_or_default();
        self.type_constraint(concept, &args);
        true
    }

    /// A reference to a function or requires-expression parameter in a
    /// requires-clause or in a `decltype` of a signature: `fp [<cv>]
    /// [<index - 1>] _` at nesting 0, else `fL <nesting - 1> p ...`
    /// (clang `mangleFunctionParam`). The nesting is the number of
    /// function types (and requires-expression parameter lists) being
    /// mangled around the reference less the parameter's function-scope
    /// depth (`ParamData::scope_depth`), less one in a result type: a
    /// parameter of a function type nested in the signature
    /// (`void (*)(T a, decltype(a))`) is one prototype scope closer.
    pub(super) fn function_param_ref(&mut self, d: DeclId) -> bool {
        let ast = self.ast;
        let Some(pd) = ast.decls.param(d) else { return false };
        let index = u32::from(pd.index);
        let (depth, in_result) = self.fn_depth;
        let own = pd.scope_depth();
        if depth <= own {
            return false;
        }
        let nesting = depth - own - u32::from(in_result);
        if nesting == 0 {
            self.out.push_str("fp");
        } else {
            self.out.push_str("fL");
            self.out.push_str(&(nesting - 1).to_string());
            self.out.push('p');
        }
        if let Some(t) = ast.decls.get(d).ty() {
            let q = ast.types.effective_quals(t);
            if q.is_restrict() {
                self.out.push('r');
            }
            if q.is_volatile() {
                self.out.push('V');
            }
            if q.is_const() {
                self.out.push('K');
            }
        }
        if index != 0 {
            self.out.push_str(&(index - 1).to_string());
        }
        self.out.push('_');
        true
    }
}
