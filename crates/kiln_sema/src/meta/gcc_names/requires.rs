//! GCC 16.1's spelling of requires-clauses in `display_string_of`
//! (spec/reflection.md §3.9, WP-1243).
//!
//! GCC prints a template's associated constraints after its template
//! header (`dump_template_decl`, `CI_TEMPLATE_REQS`) and a function's
//! trailing requires-clause after its declarator (`dump_function_decl`,
//! `CI_DECLARATOR_REQS`), both through `pp_cxx_requires_clause`: padding
//! set, `requires`, a space, then the constraint expression
//! (gcc/cp/cxx-pretty-print.cc). The expression printer is the C one
//! (gcc/c-family/c-pretty-print.cc): each binary operator prints its left
//! operand at its own precedence and its right operand one level up, and
//! a node below the level its position needs, or one the C printer does
//! not know (a template-id, a concept-id, `T::value`, a fold), goes
//! through `pp_c_primary_expression`'s default, in parentheses. Parens
//! written in the source are not kept: GCC derives them again. So
//! `requires  is_integral_v<T>`, `requires (integral<T>) && (is_signed_v<T>)`,
//! `requires  sizeof (T) > 4`, `requires !(N > 0)`, `requires - N < 0`.
//!
//! The template's constraints are its type parameters' type-constraints
//! in order, a pack's as a left fold (`(... && integral<Ts>)`), then the
//! requires-clause, joined left to right by `&&`
//! (`requires (integral<T>) && N > 1`).

use kiln_ast::expr::{BinaryOp, ExprKind, UnaryOp};
use kiln_ast::ids::RawId as _;
use kiln_ast::types::ArgView;
use kiln_ast::{ArgListId, DeclId, ExprId, TemplateParamListId};

use super::Pp;

/// `expression` (the top: the operand of `requires`, a template argument).
const EXPR: u8 = 0;
/// `assignment-expression`.
const ASSIGN: u8 = 1;
/// `conditional-expression`.
const COND: u8 = 2;
/// `logical-or-expression`.
const LOR: u8 = 3;
/// `logical-and-expression`.
const LAND: u8 = 4;
/// `inclusive-or-expression`.
const IOR: u8 = 5;
/// `cast-expression`.
const CAST: u8 = 14;
/// `unary-expression`.
const UNARY: u8 = 15;

/// One conjunct of a template's associated constraints.
#[derive(Clone, Copy)]
enum Conjunct {
    /// A type-constraint or the requires-clause.
    Expr(ExprId),
    /// A parameter pack's type-constraint, a fold over its pattern.
    Pack(ExprId),
}

/// The C printer's precedence level of binary operator `op`; `None` for
/// the operators it does not print as binary expressions here.
fn binary_level(op: BinaryOp) -> Option<u8> {
    use BinaryOp as B;
    Some(match op {
        B::LOr => LOR,
        B::LAnd => LAND,
        B::Or => IOR,
        B::Xor => 6,
        B::And => 7,
        B::EQ | B::NE => 8,
        B::LT | B::GT | B::LE | B::GE => 9,
        B::Shl | B::Shr => 10,
        B::Add | B::Sub => 11,
        B::Mul | B::Div | B::Rem => 12,
        B::PtrMemD | B::PtrMemI => 13,
        _ => return None,
    })
}

impl Pp<'_, '_, '_> {
    /// ` requires <constraints> ` after the template header of a template
    /// with parameter list `params`; nothing when it has none.
    pub(super) fn template_requires(&mut self, params: TemplateParamListId) {
        let ast = self.ast();
        let pl = ast.templates.param_list(params);
        let mut cs = Vec::new();
        for p in ast.lists.iter::<DeclId>(pl.params) {
            if let Some(info) = ast.template_parm(p)
                && let Some(c) = info.constraint
            {
                cs.push(if info.pack { Conjunct::Pack(c) } else { Conjunct::Expr(c) });
            }
        }
        if let Some(r) = pl.requires {
            cs.push(Conjunct::Expr(r));
        }
        if cs.is_empty() {
            return;
        }
        self.requires_clause(&cs);
        self.whitespace();
    }

    /// The trailing requires-clause of function `f`, if it has one.
    pub(super) fn trailing_requires(&mut self, f: DeclId) {
        if let Some(&r) = self.ast().templates.trailing_requires.get(f) {
            self.requires_clause(&[Conjunct::Expr(r)]);
        }
    }

    /// `pp_cxx_requires_clause`.
    fn requires_clause(&mut self, cs: &[Conjunct]) {
        self.pad = true;
        self.ws("requires");
        // `pp_space`: the padding stays.
        self.out.push(' ');
        self.conjunction(cs, EXPR);
    }

    /// The conjunction of `cs`, left-associated, at precedence `level`.
    fn conjunction(&mut self, cs: &[Conjunct], level: u8) {
        match cs {
            [] => {}
            [c] => self.conjunct(*c, level),
            [rest @ .., last] => {
                if level > LAND {
                    self.parenthesized(|p| p.conjunction(cs, EXPR));
                    return;
                }
                self.conjunction(rest, LAND);
                self.binary_operator("&&");
                self.conjunct(*last, IOR);
            }
        }
    }

    fn conjunct(&mut self, c: Conjunct, level: u8) {
        match c {
            Conjunct::Expr(e) => self.constraint_expr(e, level),
            Conjunct::Pack(e) => {
                // GCC folds a pack's type-constraint from the left.
                let ast = self.ast();
                let n = ast.exprs.get(e);
                let pattern = if n.kind() == ExprKind::CXXFoldExpr {
                    ExprId::from_raw(n.a()).or(ExprId::from_raw(n.b())).unwrap_or(e)
                } else {
                    e
                };
                self.wrap(level, EXPR, |p| {
                    p.punct("(");
                    p.ws("...");
                    p.ws("&&");
                    p.constraint_expr(pattern, EXPR);
                    p.punct(")");
                });
            }
        }
    }

    /// `pp_c_left_paren`, `f`, `pp_c_right_paren`.
    fn parenthesized(&mut self, f: impl FnOnce(&mut Self)) {
        self.punct("(");
        f(self);
        self.punct(")");
    }

    /// `f` at precedence `own` where `level` is needed: in parentheses
    /// when `own` is below it (the C printer's primary-expression default).
    fn wrap(&mut self, level: u8, own: u8, f: impl FnOnce(&mut Self)) {
        if level > own {
            self.parenthesized(|p| f(p));
        } else {
            f(self);
        }
    }

    /// A binary operator between its operands: ` op `.
    fn binary_operator(&mut self, op: &str) {
        self.whitespace();
        self.out.push_str(op);
        self.whitespace();
    }

    /// A fold-expression: `(... && C<Ts>)`, `(N + ... +0)`.
    fn fold_expr(&mut self, n: kiln_ast::expr::ExprNode, level: u8) {
        let op = BinaryOp::from_u8(n.aux()).map_or("?", BinaryOp::spelling);
        let (l, r) = (ExprId::from_raw(n.a()), ExprId::from_raw(n.b()));
        self.wrap(level, EXPR, |p| {
            p.punct("(");
            match (l, r) {
                (Some(l), Some(r)) => {
                    p.constraint_expr(l, EXPR);
                    p.ws(op);
                    p.ws("...");
                    p.ws(op);
                    p.constraint_expr(r, EXPR);
                }
                (Some(x), None) => {
                    p.constraint_expr(x, EXPR);
                    // `pp_space`: the padding stays.
                    p.out.push(' ');
                    p.ws(op);
                    p.ws("...");
                }
                (None, Some(x)) => {
                    p.ws("...");
                    p.ws(op);
                    p.constraint_expr(x, EXPR);
                }
                (None, None) => p.ws("..."),
            }
            p.punct(")");
        });
    }

    /// `sizeof (T)`, `alignof (T)`, `sizeof x`.
    fn trait_expr(&mut self, e: ExprId, level: u8) {
        let exprs = &self.ast().exprs;
        let n = *exprs.get(e);
        let kw = match exprs.trait_kind(e) {
            kiln_ast::expr::TraitKind::SizeOf => "sizeof",
            kiln_ast::expr::TraitKind::AlignOf => "alignof",
            kiln_ast::expr::TraitKind::PreferredAlignOf => "__alignof__",
            k => k.spelling(),
        };
        let ty = exprs.trait_type_arg(e);
        let x = ExprId::from_raw(n.a());
        self.wrap(level, UNARY, |p| {
            p.ws(kw);
            p.whitespace();
            match (ty, x) {
                (Some(t), _) => {
                    p.punct("(");
                    p.dump_type(t, 0);
                    p.punct(")");
                }
                (None, Some(x)) => p.constraint_expr(x, UNARY),
                (None, None) => {}
            }
        });
    }

    /// `cxx_pretty_printer::conditional_expression`, which prints no `:`
    /// (`sizeof (T) > 1 ? true  false`).
    fn conditional(&mut self, e: ExprId, level: u8) {
        let exprs = &self.ast().exprs;
        let n = *exprs.get(e);
        let c = ExprId::new(n.a());
        let [t, f] = exprs.cond_arms(e);
        self.wrap(level, COND, |p| {
            p.constraint_expr(c, LOR);
            p.out.push_str(" ? ");
            p.constraint_expr(t, EXPR);
            p.out.push(' ');
            p.constraint_expr(f, ASSIGN);
        });
    }

    /// `sizeof ... (Ts ...)`.
    fn sizeof_pack(&mut self, n: kiln_ast::expr::ExprNode, level: u8) {
        let parm = DeclId::from_raw(n.a());
        self.wrap(level, UNARY, |p| {
            p.ws("sizeof");
            p.ws("...");
            p.whitespace();
            p.punct("(");
            // The operand is a pack expansion: `sizeof ... (Ts ...)`.
            if let Some(d) = parm {
                p.parm_name(d);
            }
            p.ws("...");
            p.punct(")");
        });
    }

    /// Constraint expression `e` printed at precedence `level`.
    fn constraint_expr(&mut self, e: ExprId, level: u8) {
        let ast = self.ast();
        let e = self.sema.ignore_paren_imp_casts(e);
        let exprs = &ast.exprs;
        let n = *exprs.get(e);
        match n.kind() {
            ExprKind::IntegerLiteral if exprs.int_value(e).is_some() => {
                let v = exprs.int_value(e).unwrap_or_default();
                self.out.push_str(&v.to_string());
            }
            ExprKind::CXXBoolLiteralExpr => {
                self.ws(if exprs.bool_value(e) { "true" } else { "false" });
            }
            ExprKind::CXXNullPtrLiteralExpr => self.out.push_str("nullptr"),
            ExprKind::DeclRefExpr => {
                let d = exprs.decl(e);
                if ast.template_parm(d).is_some() {
                    self.parm_name(d);
                } else {
                    self.dump_scope_of(d, 0);
                    self.decl_identifier(d);
                }
            }
            ExprKind::UnaryOperator
                if matches!(
                    exprs.unary_op(e),
                    UnaryOp::LNot | UnaryOp::Minus | UnaryOp::Plus | UnaryOp::Not
                ) =>
            {
                let op = exprs.unary_op(e).spelling();
                let x = exprs.operand(e);
                self.wrap(level, UNARY, |p| {
                    // The operator is not padded: `! X`, `!(integral<T>)`.
                    p.out.push_str(op);
                    p.constraint_expr(x, CAST);
                });
            }
            ExprKind::BinaryOperator if binary_level(exprs.binary_op(e)).is_some() => {
                let op = exprs.binary_op(e);
                let own = binary_level(op).unwrap_or(EXPR);
                let [l, r] = exprs.operands(e);
                self.wrap(level, own, |p| {
                    p.constraint_expr(l, own);
                    p.binary_operator(op.spelling());
                    p.constraint_expr(r, own + 1);
                });
            }
            ExprKind::ConditionalOperator => self.conditional(e, level),
            ExprKind::UnaryExprOrTypeTraitExpr => self.trait_expr(e, level),
            ExprKind::SizeOfPackExpr => self.sizeof_pack(n, level),
            // A template-id (`is_integral_v<T>`): the template's name, not
            // its qualifier.
            ExprKind::UnresolvedLookupExpr => {
                let name = ast.unresolved_name(e);
                let targs = ast.templates.expr_targs.get(e).copied();
                self.wrap(level, EXPR, |p| {
                    let s = ast.names.simple_spelling(name, ast.symbols()).unwrap_or_default();
                    p.ws(&s);
                    if let Some(l) = targs {
                        p.constraint_args(l, 0);
                    }
                });
            }
            ExprKind::ConceptSpecializationExpr if DeclId::from_raw(n.a()).is_some() => {
                let concept = DeclId::new(n.a());
                let args = ArgListId::from_raw(n.b());
                self.wrap(level, EXPR, |p| {
                    p.decl_identifier(concept);
                    if let Some(l) = args {
                        p.constraint_args(l, 0);
                    }
                });
            }
            // `T::value`.
            ExprKind::DependentScopeDeclRefExpr => {
                let nns = kiln_ast::NnsId::new(n.a());
                let name = ast.dependent_scope_ref_name(e);
                let targs = ast.templates.expr_targs.get(e).copied();
                self.wrap(level, EXPR, |p| {
                    p.nns(nns, 0);
                    let s = ast.names.simple_spelling(name, ast.symbols()).unwrap_or_default();
                    p.ws(&s);
                    if let Some(l) = targs {
                        p.constraint_args(l, 0);
                    }
                });
            }
            ExprKind::CXXFoldExpr => self.fold_expr(n, level),
            ExprKind::RequiresExpr => self.requires_expr(e),
            _ => {
                let mut s = String::new();
                kiln_ast::types::TypeNames::write_expr(ast, e, &mut s);
                self.ws(&s);
            }
        }
    }

    /// `decltype(e)`, qualified as `t`.
    pub(super) fn decltype(&mut self, t: kiln_ast::QualType, e: ExprId) {
        self.cv(t.quals_bits());
        self.ws("decltype");
        self.parenthesized(|p| p.constraint_expr(e, EXPR));
    }

    /// A template parameter by the name it has in scope.
    fn parm_name(&mut self, d: DeclId) {
        let named = self.ast().template_parm(d).and_then(|info| {
            self.parms.get(info.depth as usize).and_then(|l| l.get(info.index as usize)).copied()
        });
        self.decl_identifier(named.unwrap_or(d));
    }

    /// The arguments of a template-id or concept-id, skipping the first
    /// `skip`: `<T, const int&>`.
    fn constraint_args(&mut self, l: ArgListId, skip: usize) {
        let args: Vec<_> = self.ast().types.args(l).iter().skip(skip).copied().collect();
        self.nonconsecutive('<');
        self.constraint_arg_list(&args);
        self.nonconsecutive('>');
    }

    fn constraint_arg_list(&mut self, args: &[kiln_ast::types::TemplateArg]) {
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                self.comma();
            }
            match a.view() {
                ArgView::Expr { expr, .. } => self.constraint_expr(expr, EXPR),
                ArgView::Pack(p) => {
                    let inner: Vec<_> = self.ast().types.args(p).to_vec();
                    self.constraint_arg_list(&inner);
                }
                v => self.template_arg(v, 0),
            }
        }
    }

    /// `pp_cxx_requires_expr`: `requires(T t) {t + t;}`.
    fn requires_expr(&mut self, e: ExprId) {
        let ast = self.ast();
        let Some(rq) = ast.requires_of(e) else {
            self.out.push_str("requires");
            return;
        };
        // `pp_string`: neither padded nor padding.
        self.out.push_str("requires");
        let parms: Vec<DeclId> = ast.lists.iter::<DeclId>(rq.params).collect();
        if !parms.is_empty() {
            self.punct("(");
            for (i, &p) in parms.iter().enumerate() {
                if i > 0 {
                    self.comma();
                }
                if let Some(t) = ast.decls.get(p).ty() {
                    self.dump_type(t, 0);
                }
                self.decl_identifier(p);
            }
            self.punct(")");
            self.whitespace();
        }
        self.punct("{");
        for r in ast.templates.requirements(rq).to_vec() {
            match r {
                kiln_ast::templates::Requirement::Simple(x) => self.constraint_expr(x, EXPR),
                kiln_ast::templates::Requirement::Type(t, _) => self.dump_type(t, 0),
                kiln_ast::templates::Requirement::Compound { expr, noexcept, constraint } => {
                    self.punct("{");
                    self.constraint_expr(expr, EXPR);
                    self.punct("}");
                    if noexcept.is_some() {
                        self.ws("noexcept");
                    }
                    if let Some(c) = constraint {
                        self.whitespace();
                        self.ws("->");
                        self.type_constraint(c);
                    }
                }
                kiln_ast::templates::Requirement::Nested(x) => {
                    self.ws("requires");
                    self.constraint_expr(x, EXPR);
                }
            }
            self.punct(";");
        }
        self.punct("}");
    }

    /// A return-type-requirement's type-constraint, as GCC prints the
    /// constrained placeholder it stands for: `decltype(auto) [requires
    /// std::same_as<<placeholder>, T>]`, `std::integral<<placeholder>, >`.
    fn type_constraint(&mut self, c: ExprId) {
        let ast = self.ast();
        let n = *ast.exprs.get(c);
        let Some(concept) =
            DeclId::from_raw(n.a()).filter(|_| n.kind() == ExprKind::ConceptSpecializationExpr)
        else {
            return;
        };
        self.ws("decltype(auto)");
        self.punct(" [requires ");
        self.dump_scope_of(concept, 0);
        self.decl_identifier(concept);
        self.punct("<<placeholder>, ");
        if let Some(l) = ArgListId::from_raw(n.b()) {
            let args: Vec<_> = ast.types.args(l).iter().skip(1).copied().collect();
            self.constraint_arg_list(&args);
        }
        self.punct(">]");
    }
}
