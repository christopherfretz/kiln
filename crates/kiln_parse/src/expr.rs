//! Expressions ([expr]; spec/parse.md §5).
//!
//! Binary operators use operator-precedence parsing with clang's
//! precedence levels; everything below a cast-expression is recursive
//! descent. A `None` result ([`PExpr`]) means a syntax error was reported
//! and the caller must recover; semantic errors yield an error expression.

use kiln_ast::expr::{BinaryOp, ExprKind, TraitKind, TypeTrait, UnaryOp};
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;
use kiln_lex::literal::{
    CodeUnits, NumberClass, StringLiteral, classify_number, decode_char, decode_string,
    decode_unevaluated_string,
};
use kiln_lex::{Punct, TokKind, Token};
use kiln_sema::expr::CastStyle;
use kiln_sema::{DeclaratorContext, NameClass, OffsetofComponent};
use kiln_source::Loc;
use kiln_source::symbol::Symbol;

use crate::decl::is_type_specifier_kw;
use crate::keywords::Kw;
use crate::parser::{PExpr, Parser, is_template_class};
use crate::tentative::TypeIdContext;
use kiln_sema::templ::actions::TemplateIdRef;

/// clang's `prec::Level`.
pub(crate) mod prec {
    pub const COMMA: u8 = 1;
    pub const ASSIGN: u8 = 2;
    pub const COND: u8 = 3;
    pub const LOR: u8 = 4;
    pub const LAND: u8 = 5;
    pub const OR: u8 = 6;
    pub const XOR: u8 = 7;
    pub const AND: u8 = 8;
    pub const EQ: u8 = 9;
    pub const REL: u8 = 10;
    pub const SPACESHIP: u8 = 11;
    pub const SHIFT: u8 = 12;
    pub const ADD: u8 = 13;
    pub const MUL: u8 = 14;
    pub const PTRMEM: u8 = 15;
}

/// A binary operator token: its precedence and operator (`None` for `?`).
fn binop(p: Punct) -> Option<(u8, Option<BinaryOp>)> {
    use BinaryOp as B;
    let r = match p {
        Punct::Comma => (prec::COMMA, B::Comma),
        Punct::Equal => (prec::ASSIGN, B::Assign),
        Punct::StarEqual => (prec::ASSIGN, B::MulAssign),
        Punct::SlashEqual => (prec::ASSIGN, B::DivAssign),
        Punct::PercentEqual => (prec::ASSIGN, B::RemAssign),
        Punct::PlusEqual => (prec::ASSIGN, B::AddAssign),
        Punct::MinusEqual => (prec::ASSIGN, B::SubAssign),
        Punct::LessLessEqual => (prec::ASSIGN, B::ShlAssign),
        Punct::GreaterGreaterEqual => (prec::ASSIGN, B::ShrAssign),
        Punct::AmpEqual => (prec::ASSIGN, B::AndAssign),
        Punct::CaretEqual => (prec::ASSIGN, B::XorAssign),
        Punct::PipeEqual => (prec::ASSIGN, B::OrAssign),
        Punct::Question => return Some((prec::COND, None)),
        Punct::PipePipe => (prec::LOR, B::LOr),
        Punct::AmpAmp => (prec::LAND, B::LAnd),
        Punct::Pipe => (prec::OR, B::Or),
        Punct::Caret => (prec::XOR, B::Xor),
        Punct::Amp => (prec::AND, B::And),
        Punct::EqualEqual => (prec::EQ, B::EQ),
        Punct::ExclaimEqual => (prec::EQ, B::NE),
        Punct::Less => (prec::REL, B::LT),
        Punct::Greater => (prec::REL, B::GT),
        Punct::LessEqual => (prec::REL, B::LE),
        Punct::GreaterEqual => (prec::REL, B::GE),
        Punct::Spaceship => (prec::SPACESHIP, B::Cmp),
        Punct::LessLess => (prec::SHIFT, B::Shl),
        Punct::GreaterGreater => (prec::SHIFT, B::Shr),
        Punct::Plus => (prec::ADD, B::Add),
        Punct::Minus => (prec::ADD, B::Sub),
        Punct::Star => (prec::MUL, B::Mul),
        Punct::Slash => (prec::MUL, B::Div),
        Punct::Percent => (prec::MUL, B::Rem),
        Punct::PeriodStar => (prec::PTRMEM, B::PtrMemD),
        Punct::ArrowStar => (prec::PTRMEM, B::PtrMemI),
        _ => return None,
    };
    Some((r.0, Some(r.1)))
}

/// True for a simple-type-specifier keyword that can begin a functional
/// cast (`int(x)`).
fn is_simple_type_kw(k: Kw) -> bool {
    is_type_specifier_kw(k)
        && !matches!(
            k,
            Kw::Const
                | Kw::Volatile
                | Kw::Restrict
                | Kw::Struct
                | Kw::Class
                | Kw::Union
                | Kw::Enum
                | Kw::Typename
        )
}

impl Parser<'_, '_> {
    /// expression ([expr.comma]).
    pub fn parse_expression(&mut self) -> PExpr {
        let lhs = self.parse_assignment_expr()?;
        self.parse_rhs(lhs, prec::COMMA)
    }

    /// assignment-expression ([expr.assign]).
    pub fn parse_assignment_expr(&mut self) -> PExpr {
        if self.at_kw(Kw::Throw) {
            return self.parse_throw_expr();
        }
        let lhs = self.parse_cast_expr()?;
        self.parse_rhs(lhs, prec::ASSIGN)
    }

    /// throw-expression ([expr.throw]): `throw assignment-expression?`.
    /// The operand is absent before a token that cannot start one (clang's
    /// `ParseThrowExpression`).
    fn parse_throw_expr(&mut self) -> PExpr {
        let loc = self.bump().span.lo;
        let none = matches!(self.tok().kind, TokKind::Eof)
            || [
                Punct::Semi,
                Punct::RParen,
                Punct::RSquare,
                Punct::RBrace,
                Punct::Colon,
                Punct::Comma,
            ]
            .iter()
            .any(|&p| self.at(p));
        let operand = if none { None } else { Some(self.parse_assignment_expr()?) };
        Some(self.s.act_on_throw(operand, loc))
    }

    /// constant-expression ([expr.const]): a conditional-expression.
    pub fn parse_constant_expr(&mut self) -> PExpr {
        let lhs = self.parse_cast_expr()?;
        self.parse_rhs(lhs, prec::COND)
    }

    /// An expression whose operators all have precedence `min` or higher
    /// (`min` = 1 is a full expression).
    pub fn parse_expr_prec(&mut self, min: u8) -> PExpr {
        let lhs = self.parse_cast_expr()?;
        self.parse_rhs(lhs, min.max(1))
    }

    /// The precedence of the binary operator at the current token.
    pub(crate) fn cur_binop_prec(&self) -> Option<u8> {
        self.cur_binop().map(|b| b.0)
    }

    /// The operators of precedence `min` or higher after `lhs`.
    pub(crate) fn parse_rhs_from(&mut self, lhs: ExprId, min: u8) -> PExpr {
        self.parse_rhs(lhs, min)
    }

    fn cur_binop(&self) -> Option<(u8, Option<BinaryOp>)> {
        let p = Self::punct_of(self.tok())?;
        if !self.greater_is_op && matches!(p, Punct::Greater | Punct::GreaterGreater) {
            return None;
        }
        let r = binop(p)?;
        // `op ...` belongs to the enclosing fold-expression (clang's
        // `ParseRHSOfBinaryExpression` leaves it alone).
        if r.1.is_some() && self.peek_is(1, Punct::Ellipsis) {
            return None;
        }
        Some(r)
    }

    /// The fold-operator at index `i` when an ellipsis follows it
    /// ([expr.prim.fold]/1: any binary operator but `?:`).
    fn fold_op_at(&self, i: usize) -> Option<BinaryOp> {
        let p = Self::punct_of(self.tok_at(i))?;
        if !self.greater_is_op && matches!(p, Punct::Greater | Punct::GreaterGreater) {
            return None;
        }
        let op = binop(p)?.1?;
        (Self::punct_of(self.tok_at(i + 1)) == Some(Punct::Ellipsis)).then_some(op)
    }

    /// Runs `f` with `>` treated as an operator or not (clang's
    /// `GreaterThanIsOperatorScope`).
    pub(crate) fn with_greater_op<T>(&mut self, on: bool, f: impl FnOnce(&mut Self) -> T) -> T {
        let saved = std::mem::replace(&mut self.greater_is_op, on);
        let r = f(self);
        self.greater_is_op = saved;
        r
    }

    /// clang's `ParseRHSOfBinaryExpression`.
    fn parse_rhs(&mut self, mut lhs: ExprId, min: u8) -> PExpr {
        loop {
            let Some((p, op)) = self.cur_binop() else { return Some(lhs) };
            if p < min {
                return Some(lhs);
            }
            if op == Some(BinaryOp::LT) && self.diagnose_missing_template_kw(lhs) {
                return None;
            }
            let op_tok = self.bump();
            let op_loc = op_tok.span.lo;
            let mut middle = None;
            let mut colon = None;
            if op.is_none() {
                if let Some(c) = self.eat(Punct::Colon) {
                    // GNU `a ?: b`.
                    let rhs = self.parse_assignment_expr()?;
                    lhs = self.s.act_on_binary_conditional(lhs, rhs, op_loc, c);
                    continue;
                }
                middle = Some(self.parse_expression()?);
                colon = self.eat(Punct::Colon);
                if colon.is_none() {
                    let note = self.s.diag(DiagId::note_matching, op_loc).arg("'?'");
                    let d = self
                        .s
                        .diag(DiagId::parse_expected, self.expected_loc())
                        .arg("':'")
                        .with_note(note);
                    self.emit(d);
                    return None;
                }
            }
            // [expr.assign], [expr.cond]: the right operand of an assignment
            // and the third operand of `?:` are assignment-expressions.
            if self.at(Punct::LBrace) {
                lhs = self.parse_braced_rhs(lhs, p, op, &op_tok, colon)?;
                continue;
            }
            let mut rhs = if p <= prec::COND {
                self.parse_assignment_expr()?
            } else {
                self.parse_cast_expr()?
            };
            if p > prec::COND {
                while let Some((np, _)) = self.cur_binop() {
                    if np <= p {
                        break;
                    }
                    rhs = self.parse_rhs(rhs, p + 1)?;
                }
            }
            lhs = match (op, middle) {
                (Some(op), _) => self.s.act_on_binary(op, lhs, rhs, op_loc),
                (None, Some(m)) => self.s.act_on_conditional(lhs, m, rhs, op_loc),
                (None, None) => unreachable!("'?' always has a middle operand"),
            };
        }
    }

    /// [temp.names]/3: a dependent name (`T::g`, `t.g`) followed by a `<`
    /// that must start a template argument list -- `<>`, an unambiguous
    /// type-id (`err_missing_dependent_template_keyword`), or a
    /// list closing on `>` followed by `()` (`err_template_kw_missing`) --
    /// is diagnosed as missing the `template` keyword at the name (clang's
    /// `checkPotentialAngleBracket` and
    /// `checkPotentialAngleBracketDelimiter`). Returns whether it was
    /// diagnosed; the expression is then erroneous.
    fn diagnose_missing_template_kw(&mut self, lhs: ExprId) -> bool {
        let n = *self.s.ast.exprs.get(lhs);
        if !matches!(
            n.kind(),
            ExprKind::DependentScopeDeclRefExpr | ExprKind::CXXDependentScopeMemberExpr
        ) || self.s.ast.templates.expr_targs.contains(lhs)
        {
            return false;
        }
        let less = self.pos();
        let next = Self::punct_of(self.tok_at(less + 1));
        let id = if matches!(next, Some(Punct::Greater | Punct::GreaterGreater))
            || self.is_cxx_type_id_at(less + 1, TypeIdContext::Unambiguous).0
        {
            DiagId::parse_missing_dependent_template_kw
        } else if self.skip_template_args(less).is_some_and(|(close, split)| {
            !split
                && Self::punct_of(self.tok_at(close + 1)) == Some(Punct::LParen)
                && Self::punct_of(self.tok_at(close + 2)) == Some(Punct::RParen)
        }) {
            DiagId::parse_template_kw_missing
        } else {
            return false;
        };
        let name = Symbol::from_index(n.b());
        let text = self.s.ast.symbols().as_str(name).to_owned();
        let d = self.s.diag(id, n.loc()).arg(text);
        self.emit(d);
        true
    }

    /// A braced-init-list right operand of the binary operator `op_tok`
    /// (precedence `p`; `op` is `None` for `?:`, whose `:` is at `colon`).
    /// clang parses one after any operator and accepts it only as the
    /// right operand of an assignment ([expr.assign]/8): elsewhere it
    /// reports `err_init_list_bin_op` and the operation is erroneous.
    fn parse_braced_rhs(
        &mut self,
        lhs: ExprId,
        p: u8,
        op: Option<BinaryOp>,
        op_tok: &Token,
        colon: Option<Loc>,
    ) -> PExpr {
        let l = self.parse_init_list().ok()?;
        // An operator binding tighter than this one (or a right-associative
        // one of the same precedence) would take the list as its left
        // operand.
        let next = self.cur_binop().map(|(np, _)| np);
        if let Some(np) = next
            && (np > p || (np == p && p <= prec::COND))
        {
            let t = self.tok();
            let sp = self.spelling_of(&t);
            let mut rhs = self.s.act_on_misplaced_init_list(lhs, &l, false, &sp, t.span.lo);
            while let Some((np, _)) = self.cur_binop() {
                if np < p || (np == p && p > prec::COND) {
                    break;
                }
                rhs = self.parse_rhs(rhs, if p <= prec::COND { p } else { p + 1 })?;
            }
            return Some(self.s.act_on_error_expr(&[lhs, rhs], op_tok.span.lo));
        }
        Some(match (op, colon) {
            (Some(op), _) if p == prec::ASSIGN => {
                self.s.act_on_assign_list(op, lhs, &l, op_tok.span.lo)
            }
            (None, Some(c)) => self.s.act_on_misplaced_init_list(lhs, &l, true, ":", c),
            _ => {
                let sp = self.spelling_of(op_tok);
                self.s.act_on_misplaced_init_list(lhs, &l, true, &sp, op_tok.span.lo)
            }
        })
    }

    // -- unary and cast expressions ----------------------------------------------------

    /// cast-expression ([expr.cast]) and unary-expression ([expr.unary]).
    pub(crate) fn parse_cast_expr(&mut self) -> PExpr {
        if !self.stack_ok() {
            return None;
        }
        let loc = self.loc();
        if self.at_global_new_delete() {
            return self.parse_new_delete_expr();
        }
        if let Some(p) = Self::punct_of(self.tok()) {
            let op = match p {
                Punct::PlusPlus => Some(UnaryOp::PreInc),
                Punct::MinusMinus => Some(UnaryOp::PreDec),
                Punct::Amp => Some(UnaryOp::AddrOf),
                Punct::Star => Some(UnaryOp::Deref),
                Punct::Plus => Some(UnaryOp::Plus),
                Punct::Minus => Some(UnaryOp::Minus),
                Punct::Tilde => Some(UnaryOp::Not),
                Punct::Exclaim => Some(UnaryOp::LNot),
                _ => None,
            };
            if let Some(op) = op {
                self.bump();
                let e = self.parse_cast_expr()?;
                return Some(self.s.act_on_unary(op, e, loc));
            }
            if p == Punct::AmpAmp {
                self.unsupported("the GNU address-of-label operator", loc);
                return None;
            }
            if p == Punct::LParen
                && self.is_cxx_type_id_at(self.pos() + 1, TypeIdContext::InParens).0
            {
                return self.parse_c_style_cast();
            }
        }
        if let Some(crate::keywords::BuiltinKw::Trait(t)) = self.builtin_kw_at(self.pos()) {
            // clang's `ParseTypeTrait` / `ParseArrayTypeTrait` /
            // `ParseExpressionTrait`: no postfix suffix follows.
            return Some(self.parse_type_trait(t));
        }
        match self.kw() {
            Some(Kw::Sizeof) => return self.parse_sizeof_alignof(TraitKind::SizeOf),
            Some(Kw::Alignof) => return self.parse_sizeof_alignof(TraitKind::AlignOf),
            Some(Kw::GnuAlignof) => {
                return self.parse_sizeof_alignof(TraitKind::PreferredAlignOf);
            }
            Some(Kw::Extension) => {
                self.bump();
                return self.parse_cast_expr();
            }
            Some(Kw::Noexcept) => return self.parse_noexcept_expr(),
            Some(Kw::New | Kw::Delete) => return self.parse_new_delete_expr(),
            Some(k @ Kw::CoAwait) => {
                self.unsupported(&format!("'{}' expressions", k.spelling()), loc);
                return None;
            }
            _ => {}
        }
        self.parse_postfix_expr()
    }

    /// The rest of a GNU statement expression `( compound-statement )`
    /// after the `(` at `lparen` (clang's `ParseParenExpression`,
    /// `CompoundStmt` case). Only allowed in a function body; elsewhere
    /// it is skipped up to and including the `)`.
    fn parse_stmt_expr(&mut self, lparen: Loc) -> PExpr {
        if !self.s.check_stmt_expr_allowed(lparen) {
            self.skip_until(&[Punct::RParen], true, true);
            return None;
        }
        let scope = self.s.act_on_start_stmt_expr();
        let (body, _) = self.parse_compound_in(true, true);
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, lparen);
        Some(self.s.act_on_stmt_expr(scope, body, lparen, rparen))
    }

    /// `( type-id ) cast-expression`.
    fn parse_c_style_cast(&mut self) -> PExpr {
        let open = self.bump().span.lo;
        let ty = self.parse_type_id();
        let Some(ty) = ty else {
            self.skip_until(&[Punct::RParen], false, true);
            return None;
        };
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
        if self.at(Punct::LBrace) {
            // GNU/C99 compound literal `( type-id ) braced-init-list`, a
            // postfix-expression (clang's `ParseCompoundLiteralExpression`).
            let l = self.parse_init_list().ok()?;
            let e = self.s.act_on_compound_literal(ty, &l, open);
            return self.parse_postfix_suffix(e);
        }
        let e = self.parse_cast_expr()?;
        Some(self.s.act_on_cast(CastStyle::CStyle, ty, e, open, rparen))
    }

    /// `sizeof...(name)` ([expr.sizeof]/5): Sema looks the pack up; the
    /// unparenthesized form is unsupported.
    fn parse_sizeof_pack(&mut self, loc: Loc) -> PExpr {
        let dots = self.bump().span.lo;
        if !self.at(Punct::LParen) {
            self.unsupported("'sizeof...' without parentheses", dots);
            return None;
        }
        let open = self.bump().span.lo;
        let Some(name) = self.ident() else {
            self.error_here(DiagId::parse_expected_name_of_pack);
            self.skip_until(&[Punct::RParen], true, true);
            return Some(self.s.act_on_error_expr(&[], loc));
        };
        let name_loc = self.bump().span.lo;
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
        Some(self.s.act_on_sizeof_pack_name(name, name_loc, loc, rparen))
    }

    /// `sizeof`/`alignof`/`__alignof` ([expr.sizeof], [expr.alignof]).
    fn parse_sizeof_alignof(&mut self, kind: TraitKind) -> PExpr {
        let loc = self.bump().span.lo;
        if kind == TraitKind::SizeOf && self.at(Punct::Ellipsis) {
            return self.parse_sizeof_pack(loc);
        }
        if self.at(Punct::LParen)
            && self.is_cxx_type_id_at(self.pos() + 1, TypeIdContext::InParens).0
        {
            let open = self.bump().span.lo;
            let Some(ty) = self.parse_type_id() else {
                self.skip_until(&[Punct::RParen], false, true);
                return None;
            };
            let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
            return Some(self.s.act_on_trait_type(kind, ty, loc, rparen));
        }
        self.s.enter_unevaluated();
        let e = self.parse_cast_expr();
        self.s.exit_unevaluated();
        Some(self.s.act_on_trait_expr(kind, e?, loc))
    }

    /// A builtin type trait `__trait ( ... )` (spec/sema-templates.md
    /// §9.1, §9.4): a type-id list (each possibly a pack expansion), the
    /// type and dimension of `__array_extent`, or the unevaluated operand
    /// of `__is_lvalue_expr` / `__is_rvalue_expr`.
    fn parse_type_trait(&mut self, t: TypeTrait) -> ExprId {
        let loc = self.bump().span.lo;
        let open = self.bump().span.lo;
        match t {
            TypeTrait::IsLvalueExpr | TypeTrait::IsRvalueExpr => {
                self.s.enter_unevaluated();
                let e = self.parse_expression();
                self.s.exit_unevaluated();
                let Some(e) = e else {
                    self.skip_until(&[Punct::RParen], true, true);
                    return self.s.act_on_error_expr(&[], loc);
                };
                let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
                self.s.build_expression_trait(t, e, loc, rparen)
            }
            TypeTrait::ArrayRank | TypeTrait::ArrayExtent => {
                let Some(ty) = self.parse_type_id() else {
                    self.skip_until(&[Punct::RParen], true, true);
                    return self.s.act_on_error_expr(&[], loc);
                };
                let mut dim = None;
                if t == TypeTrait::ArrayExtent {
                    if self.eat(Punct::Comma).is_none() {
                        let d = self.s.diag(DiagId::parse_expected, self.expected_loc()).arg("','");
                        self.emit(d);
                        self.skip_until(&[Punct::RParen], true, true);
                        return self.s.act_on_error_expr(&[], loc);
                    }
                    let Some(e) = self.parse_expression() else {
                        self.skip_until(&[Punct::RParen], true, true);
                        return self.s.act_on_error_expr(&[], loc);
                    };
                    dim = Some(e);
                }
                let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
                self.s.build_array_type_trait(t, ty, dim, loc, rparen)
            }
            _ => {
                let mut args = Vec::new();
                loop {
                    let at = self.loc();
                    let Some(mut ty) = self.parse_type_id() else {
                        self.skip_until(&[Punct::RParen], true, true);
                        return self.s.act_on_error_expr(&[], loc);
                    };
                    if let Some(l) = self.eat(Punct::Ellipsis) {
                        ty = self.s.act_pack_expansion_type(ty, l);
                    }
                    args.push((ty, at));
                    if self.eat(Punct::Comma).is_none() {
                        break;
                    }
                }
                let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
                self.s.act_type_trait(t, &args, loc, rparen)
            }
        }
    }

    /// `noexcept ( expression )` ([expr.unary.noexcept]); the operand is
    /// unevaluated.
    fn parse_noexcept_expr(&mut self) -> PExpr {
        let loc = self.bump().span.lo;
        let Some(open) = self.eat(Punct::LParen) else {
            let d =
                self.s.diag(DiagId::parse_expected_after, self.loc()).arg("'noexcept'").arg("'('");
            self.emit(d);
            return None;
        };
        self.s.enter_unevaluated();
        let e = self.parse_expression();
        self.s.exit_unevaluated();
        let Some(e) = e else {
            self.skip_until(&[Punct::RParen], false, true);
            return Some(self.s.act_on_error_expr(&[], loc));
        };
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
        Some(self.s.act_on_noexcept_expr(e, loc, rparen))
    }

    /// `typeid ( type-id )` / `typeid ( expression )` ([expr.typeid]);
    /// a parenthesized type-id wins ([dcl.ambig.res]/2). The expression
    /// operand is unevaluated (no polymorphic classes exist in M2).
    fn parse_typeid(&mut self) -> PExpr {
        let loc = self.bump().span.lo;
        let Some(open) = self.eat(Punct::LParen) else {
            let d =
                self.s.diag(DiagId::parse_expected_after, self.loc()).arg("'typeid'").arg("'('");
            self.emit(d);
            return None;
        };
        if self.is_cxx_type_id(TypeIdContext::InParens).0 {
            // clang matches the `)` before looking at the type.
            let ty = self.parse_type_id();
            let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
            return Some(self.s.act_on_typeid(Err(ty?), loc, rparen));
        }
        self.s.enter_unevaluated();
        let e = self.parse_expression();
        self.s.exit_unevaluated();
        let Some(e) = e else {
            self.skip_until(&[Punct::RParen], true, true);
            return None;
        };
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
        Some(self.s.act_on_typeid(Ok(e), loc, rparen))
    }

    /// `__builtin_bit_cast ( type-id , expression )` (clang
    /// `ParseBuiltinBitCast`; [bit.cast]).
    fn parse_builtin_bit_cast(&mut self) -> PExpr {
        let loc = self.bump().span.lo;
        let Some(open) = self.eat(Punct::LParen) else {
            let d = self
                .s
                .diag(DiagId::parse_expected_after, self.expected_loc())
                .arg("'__builtin_bit_cast'")
                .arg("'('");
            self.emit(d);
            return None;
        };
        let ty = self.parse_type_id();
        if self.eat(Punct::Comma).is_none() {
            // clang reports the missing `,` twice (`ExpectAndConsume`,
            // then its own `err_expected`).
            for _ in 0..2 {
                let d = self.s.diag(DiagId::parse_expected, self.expected_loc()).arg("','");
                self.emit(d.allow_duplicate());
            }
            self.skip_until(&[Punct::RParen], true, true);
            return None;
        }
        let e = self.parse_expression();
        let ok = self.at(Punct::RParen);
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
        let (ty, e) = (ty?, e?);
        ok.then(|| self.s.act_on_builtin_bit_cast(ty, e, loc, rparen))
    }

    /// `__builtin_offsetof ( type-id , offsetof-member-designator )`
    /// (clang `ParseBuiltinPrimaryExpression`): the designator is an
    /// identifier followed by `.identifier` and `[expression]` parts.
    fn parse_builtin_offsetof(&mut self) -> PExpr {
        let loc = self.bump().span.lo;
        let Some(open) = self.eat(Punct::LParen) else {
            let d = self
                .s
                .diag(DiagId::parse_expected_after, self.expected_loc())
                .arg("'__builtin_offsetof'")
                .arg("'('");
            self.emit(d);
            return None;
        };
        let Some(ty) = self.parse_type_id() else {
            self.skip_until(&[Punct::RParen], true, true);
            return None;
        };
        if self.expect(Punct::Comma).is_none() {
            self.skip_until(&[Punct::RParen], true, true);
            return None;
        }
        let mut comps = Vec::new();
        let Some(name) = self.ident() else {
            return self.offsetof_expected_identifier();
        };
        let at = self.bump().span.lo;
        comps.push(OffsetofComponent::Field { name, start: at, end: at });
        loop {
            if let Some(dot) = self.eat(Punct::Period) {
                let Some(name) = self.ident() else {
                    return self.offsetof_expected_identifier();
                };
                let end = self.bump().span.lo;
                comps.push(OffsetofComponent::Field { name, start: dot, end });
            } else if self.at(Punct::LSquare) {
                let start = self.bump().span.lo;
                let Some(index) = self.parse_expression() else {
                    self.skip_until(&[Punct::RParen], true, true);
                    return None;
                };
                let end = self.expect_closing(Punct::RSquare, Punct::LSquare, start);
                comps.push(OffsetofComponent::Index { index, start, end });
            } else {
                let ok = self.at(Punct::RParen);
                let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
                return ok.then(|| self.s.act_on_builtin_offsetof(loc, ty, &comps, rparen));
            }
        }
    }

    /// `expected identifier` in an offsetof designator, then skip past `)`.
    fn offsetof_expected_identifier(&mut self) -> PExpr {
        let d = self.s.diag(DiagId::parse_expected, self.loc()).arg("identifier");
        self.emit(d);
        self.skip_until(&[Punct::RParen], true, true);
        None
    }

    // -- postfix expressions --------------------------------------------------------------

    /// postfix-expression ([expr.post]).
    fn parse_postfix_expr(&mut self) -> PExpr {
        let e = self.parse_primary_expr()?;
        self.parse_postfix_suffix(e)
    }

    /// The postfix operators applied to `e` (clang's
    /// `ParsePostfixExpressionSuffix`).
    pub(crate) fn parse_postfix_suffix(&mut self, mut e: ExprId) -> PExpr {
        loop {
            let loc = self.loc();
            match Self::punct_of(self.tok()) {
                Some(Punct::LSquare) => {
                    if self.peek_is(1, Punct::LSquare) {
                        return Some(e);
                    }
                    self.bump();
                    if self.at(Punct::LBrace) {
                        self.unsupported("braced-init-list subscripts", self.loc());
                        return None;
                    }
                    let Some(idx) = self.with_greater_op(true, Self::parse_expression) else {
                        self.skip_until(&[Punct::RSquare], false, true);
                        return None;
                    };
                    self.expect_closing(Punct::RSquare, Punct::LSquare, loc);
                    e = self.s.act_on_subscript(e, idx, loc);
                }
                Some(Punct::LParen) => {
                    self.bump();
                    let args = self.with_greater_op(true, Self::parse_call_args)?;
                    let rparen = self.expect_closing(Punct::RParen, Punct::LParen, loc);
                    e = self.s.act_on_call(e, &args, loc, rparen);
                }
                Some(p @ (Punct::Period | Punct::Arrow)) => {
                    self.bump();
                    let template_kw = self.eat_kw(Kw::Template).is_some();
                    if self.at(Punct::Tilde) {
                        self.unsupported("this member access", self.loc());
                        return None;
                    }
                    // `e.N::m` ([expr.ref]/1 with a qualified-id): sema's
                    // `act_on_member` takes no nested-name-specifier yet.
                    if self.at(Punct::ColonColon)
                        || (self.ident().is_some() && self.peek_is(1, Punct::ColonColon))
                    {
                        self.unsupported("qualified member access", self.loc());
                        return None;
                    }
                    let Some(name) = self.ident() else {
                        self.error_here(DiagId::parse_expected_unqualified_id);
                        return None;
                    };
                    let nl = self.bump().span.lo;
                    let arrow = p == Punct::Arrow;
                    if template_kw {
                        // `e.template f<args>` ([temp.names]/5).
                        let targs = if self.at(Punct::Less) {
                            let (args, _, _) = self.parse_template_arg_list()?;
                            Some(self.s.ast.types.arg_list(&args))
                        } else {
                            None
                        };
                        let Some(m) = self.s.try_dependent_member(e, name, targs, true, nl, arrow)
                        else {
                            self.unsupported(
                                "'template' member access on a non-dependent object",
                                nl,
                            );
                            return None;
                        };
                        e = m;
                        continue;
                    }
                    e = self.s.act_on_member(e, name, nl, loc, arrow);
                }
                Some(Punct::PlusPlus) => {
                    self.bump();
                    e = self.s.act_on_unary(UnaryOp::PostInc, e, loc);
                }
                Some(Punct::MinusMinus) => {
                    self.bump();
                    e = self.s.act_on_unary(UnaryOp::PostDec, e, loc);
                }
                _ => return Some(e),
            }
        }
    }

    /// The arguments of a call after its `(` ([expr.call]): an
    /// expression-list whose initializer-clauses may be braced lists or
    /// pack expansions ([temp.variadic]/5).
    fn parse_call_args(&mut self) -> Option<Vec<ExprId>> {
        let mut args = Vec::new();
        if self.at(Punct::RParen) {
            return Some(args);
        }
        loop {
            let a = if self.at(Punct::LBrace) {
                let Ok(l) = self.parse_init_list() else {
                    self.skip_until(&[Punct::RParen], false, true);
                    return None;
                };
                self.s.act_on_braced_arg(l)
            } else {
                let Some(a) = self.parse_assignment_expr() else {
                    self.skip_until(&[Punct::RParen], false, true);
                    return None;
                };
                a
            };
            let a = match self.eat(Punct::Ellipsis) {
                Some(l) => self.s.act_pack_expansion(a, l),
                None => a,
            };
            args.push(a);
            if self.eat(Punct::Comma).is_none() {
                return Some(args);
            }
        }
    }

    // -- primary expressions ----------------------------------------------------------------

    /// primary-expression ([expr.prim]), named casts and functional casts.
    pub(crate) fn parse_primary_expr(&mut self) -> PExpr {
        let t = self.tok();
        let loc = t.span.lo;
        match t.kind {
            TokKind::PpNumber => return Some(self.parse_number()),
            TokKind::CharLit(_) => return Some(self.parse_char()),
            TokKind::StringLit(_) | TokKind::RawStringLit(_) => {
                return self.parse_string_literal(false);
            }
            TokKind::Punct(Punct::LParen) => {
                self.bump();
                if self.at(Punct::LBrace) {
                    return self.parse_stmt_expr(loc);
                }
                // [expr.prim.paren]: `>` is an operator inside parentheses.
                return self.with_greater_op(true, |p| p.parse_paren_rest(loc));
            }
            TokKind::Punct(Punct::LSquare) => {
                self.unsupported("lambda expressions", loc);
                return None;
            }
            TokKind::Punct(Punct::CaretCaret | Punct::SpliceOpen) => {
                self.unsupported("reflection", loc);
                return None;
            }
            _ => {}
        }
        if let Some(k) = self.kw() {
            return match k {
                Kw::True | Kw::False => {
                    self.bump();
                    Some(self.s.act_on_bool_literal(k == Kw::True, loc))
                }
                Kw::Nullptr => {
                    self.bump();
                    Some(self.s.act_on_nullptr(loc))
                }
                Kw::StaticCast => self.parse_named_cast(CastStyle::Static),
                Kw::ConstCast => self.parse_named_cast(CastStyle::Const),
                Kw::ReinterpretCast => self.parse_named_cast(CastStyle::Reinterpret),
                Kw::DynamicCast => self.parse_named_cast(CastStyle::Dynamic),
                Kw::Operator => {
                    self.unsupported("operator function names", loc);
                    None
                }
                _ if is_simple_type_kw(k) => self.parse_functional_cast(),
                Kw::This => {
                    self.bump();
                    Some(self.s.act_on_this(loc))
                }
                Kw::Typeid => self.parse_typeid(),
                Kw::BuiltinOffsetof => self.parse_builtin_offsetof(),
                Kw::BuiltinBitCast => self.parse_builtin_bit_cast(),
                Kw::Requires => Some(self.parse_requires_expression()),
                // `typename T::type(e)` ([expr.type.conv]).
                Kw::Typename => self.parse_functional_cast(),
                Kw::CoYield | Kw::Template => {
                    self.unsupported(&format!("'{}' expressions", k.spelling()), loc);
                    None
                }
                _ => {
                    self.error_here(DiagId::parse_expected_expression);
                    None
                }
            };
        }
        if self.ident().is_some() || self.at(Punct::ColonColon) {
            if let Some((end, NameClass::Type(..), name)) = self.scan_name(self.pos()) {
                // An unqualified type name not followed by `(` or `{` is
                // not looked at as a type (clang only annotates it before
                // those), so the id-expression names a type.
                if end == self.pos() + 1
                    && !matches!(
                        Self::punct_of(self.tok_at(end)),
                        Some(Punct::LParen | Punct::LBrace)
                    )
                {
                    let text = self.s.ast.symbols().as_str(name).to_owned();
                    let d = self.s.diag(DiagId::sema_unexpected_typedef_ident, loc).arg(text);
                    self.emit(d);
                    self.bump();
                    return Some(self.s.act_on_error_expr(&[], loc));
                }
                return self.parse_functional_cast();
            }
            return self.parse_id_expression_rest();
        }
        self.error_here(DiagId::parse_expected_expression);
        None
    }

    /// A (possibly qualified) id-expression that does not name a type
    /// ([expr.prim.id]): a template-id names a variable template, a
    /// function template or a concept ([temp.names]/1); a name after a
    /// dependent qualifier is a `DependentScopeDeclRefExpr`
    /// ([temp.dep.expr]/3); an undeclared name before `(` in a template
    /// is found by argument-dependent lookup at instantiation
    /// ([temp.dep.general]/2).
    fn parse_id_expression_rest(&mut self) -> PExpr {
        let begin = self.loc();
        let qual = self.parse_nested_name_nns().ok()?;
        let template_kw = qual.is_some() && self.eat_kw(Kw::Template).is_some();
        let Some(name) = self.ident() else {
            self.error_here(DiagId::parse_expected_unqualified_id);
            return None;
        };
        let nl = self.bump().span.lo;
        if let Some(q) = qual
            && self.s.is_dependent_ctx(q.ctx)
        {
            let targs = if template_kw && self.at(Punct::Less) {
                let (args, _, _) = self.parse_template_arg_list()?;
                Some(self.s.ast.types.arg_list(&args))
            } else {
                None
            };
            return Some(self.s.act_dependent_scope_ref(q.nns, name, targs, begin, nl));
        }
        let class = self.s.classify_name(name, qual.map(|q| q.ctx));
        if self.at(Punct::Less) && (template_kw || is_template_class(class)) {
            return match self.parse_template_id_rest(class, name, nl) {
                Ok(TemplateIdRef::Overload(e)) => {
                    if qual.is_none() && self.at(Punct::LParen) {
                        self.s.act_on_template_id_call_name(e);
                    }
                    Some(e)
                }
                Ok(TemplateIdRef::Value(e)) => Some(e),
                Ok(TemplateIdRef::Type(_)) => {
                    let t = self.s.ast.symbols().as_str(name).to_owned();
                    let d = self.s.diag(DiagId::sema_unexpected_typedef_ident, nl).arg(t);
                    self.emit(d);
                    Some(self.s.act_on_error_expr(&[], nl))
                }
                Err(()) => Some(self.s.act_on_error_expr(&[], nl)),
            };
        }
        if qual.is_none()
            && class == NameClass::Undeclared
            && self.at(Punct::LParen)
            && self.s.in_template()
        {
            return Some(self.s.act_on_undeclared_call_name(name, nl));
        }
        if qual.is_none() && self.at(Punct::LParen) && self.s.in_template() {
            return Some(self.s.act_on_call_name(name, nl));
        }
        Some(self.s.act_on_id_expression(name, nl, qual))
    }

    /// The rest of a parenthesized expression or fold-expression after
    /// `(` at `lparen` ([expr.prim.paren], [expr.prim.fold]).
    fn parse_paren_rest(&mut self, lparen: Loc) -> PExpr {
        // `( ... op e )`.
        if self.at(Punct::Ellipsis)
            && let Some(op) =
                Self::punct_of(self.tok_at(self.pos() + 1)).and_then(binop).and_then(|b| b.1)
        {
            let ellipsis = self.bump().span.lo;
            self.bump();
            let Some(rhs) = self.parse_fold_operand(true) else {
                self.skip_until(&[Punct::RParen], false, true);
                return None;
            };
            let r = self.expect_closing(Punct::RParen, Punct::LParen, lparen);
            return Some(self.s.act_fold(None, op, Some(rhs), lparen, ellipsis, r));
        }
        let e = if self.at_kw(Kw::Throw) {
            self.parse_expression()
        } else {
            self.parse_fold_operand(false)
        };
        let Some(e) = e else {
            self.skip_until(&[Punct::RParen], false, true);
            return None;
        };
        let Some(op) = self.fold_op_at(self.pos()) else {
            let r = self.expect_closing(Punct::RParen, Punct::LParen, lparen);
            return Some(self.s.act_on_paren(e, lparen, r));
        };
        // `( e op ... )` or `( e op ... op e )`.
        self.bump();
        let ellipsis = self.bump().span.lo;
        let mut rhs = None;
        if !self.at(Punct::RParen) {
            let p = Self::punct_of(self.tok()).and_then(binop).and_then(|b| b.1);
            let Some(op2) = p else {
                self.error_here(DiagId::parse_expected_fold_operator);
                self.skip_until(&[Punct::RParen], false, true);
                return Some(self.s.act_on_error_expr(&[e], lparen));
            };
            if op2 != op {
                self.error_here(DiagId::parse_fold_operator_mismatch);
            }
            self.bump();
            let Some(r) = self.parse_fold_operand(true) else {
                self.skip_until(&[Punct::RParen], false, true);
                return None;
            };
            rhs = Some(r);
        }
        let r = self.expect_closing(Punct::RParen, Punct::LParen, lparen);
        Some(self.s.act_fold(Some(e), op, rhs, lparen, ellipsis, r))
    }

    /// An expression that may be an operand of a fold-expression: a
    /// cast-expression, or any expression (diagnosed when a fold operator
    /// follows, [expr.prim.fold]/1, clang's `CheckFoldOperand`).
    fn parse_fold_operand(&mut self, in_fold: bool) -> PExpr {
        let start = self.loc();
        let cast = self.parse_cast_expr()?;
        let e = self.parse_rhs(cast, prec::COMMA)?;
        if e != cast && (in_fold || self.fold_op_at(self.pos()).is_some()) {
            // clang's `getExprLoc`: a binary operator's operator, else the
            // start.
            let n = self.s.ast.exprs.get(e);
            let at = match n.kind() {
                ExprKind::BinaryOperator | ExprKind::CompoundAssignOperator => n.loc(),
                _ => start,
            };
            let d = self.s.diag(DiagId::parse_fold_operand_not_cast, at);
            self.emit(d);
        }
        Some(e)
    }

    /// `static_cast<T>(e)` and friends ([expr.static.cast] etc.).
    fn parse_named_cast(&mut self, style: CastStyle) -> PExpr {
        let loc = self.bump().span.lo;
        self.expect(Punct::Less)?;
        let ty = self.parse_type_id()?;
        self.expect(Punct::Greater)?;
        let open = self.expect(Punct::LParen)?;
        let Some(e) = self.parse_expression() else {
            self.skip_until(&[Punct::RParen], false, true);
            return None;
        };
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
        Some(self.s.act_on_cast(style, ty, e, loc, rparen))
    }

    /// `T(e)` ([expr.type.conv]) with one operand.
    fn parse_functional_cast(&mut self) -> PExpr {
        let loc = self.loc();
        let ds = self.parse_decl_specifiers(DeclaratorContext::TypeName);
        let ty: QualType = self.s.type_from_declspec(&ds);
        if self.at(Punct::LBrace) {
            let l = self.parse_init_list().ok()?;
            return Some(self.s.act_on_list_cast(ty, &l, loc));
        }
        let Some(open) = self.eat(Punct::LParen) else {
            let d = self.s.diag(DiagId::parse_expected_lparen_after_type, self.loc());
            self.emit(d);
            return None;
        };
        if self.at(Punct::RParen) {
            let rparen = self.bump().span.lo;
            return Some(self.s.act_on_value_init_cast(ty, loc, rparen));
        }
        let mut args = Vec::new();
        loop {
            if self.at(Punct::LBrace) {
                self.unsupported("braced-init-list operands of a functional cast", self.loc());
                self.skip_until(&[Punct::RParen], false, true);
                return None;
            }
            let Some(e) = self.parse_assignment_expr() else {
                self.skip_until(&[Punct::RParen], false, true);
                return None;
            };
            args.push(e);
            if self.eat(Punct::Comma).is_none() {
                break;
            }
        }
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
        if let [e] = args[..] {
            return Some(self.s.act_on_cast(CastStyle::Functional, ty, e, loc, rparen));
        }
        Some(self.s.act_on_paren_list_cast(ty, &args, loc, rparen))
    }

    // -- literals ------------------------------------------------------------------------------

    /// The spelling of token `t` (any token: from a file, a macro
    /// definition or a scratch buffer of `#` / `##`).
    pub(crate) fn spelling_of(&self, t: &Token) -> String {
        kiln_lex::spelling(self.sm, t, &self.lex_opts).into_owned()
    }

    /// Reports the literal diagnostics `ds` of token `t` with spelling
    /// `sp` (`enc`: a character literal's encoding).
    fn literal_diags(
        &self,
        t: &Token,
        sp: &str,
        ds: &[kiln_lex::literal::LitDiag],
        enc: Option<kiln_lex::Encoding>,
    ) {
        for d in kiln_pp::literal_diags(self.sm, t, sp, ds, enc, &self.lex_opts) {
            self.emit(d);
        }
    }

    fn parse_number(&mut self) -> ExprId {
        let t = self.bump();
        let sp = self.spelling_of(&t);
        let n = classify_number(&sp, self.std);
        self.literal_diags(&t, &sp, &n.diags, None);
        if n.has_error {
            return self.s.act_on_error_expr(&[], t.span.lo);
        }
        match &n.class {
            NumberClass::Integer(l) => {
                let ud = suffix_loc(t.span.hi, l.ud_suffix);
                self.s.act_on_integer_literal(l, t.span.lo, ud)
            }
            NumberClass::Floating(l) => {
                let ud = suffix_loc(t.span.hi, l.ud_suffix);
                self.s.act_on_float_literal(l, t.span.lo, ud)
            }
            NumberClass::Invalid => self.s.act_on_error_expr(&[], t.span.lo),
        }
    }

    fn parse_char(&mut self) -> ExprId {
        let t = self.bump();
        let sp = self.spelling_of(&t);
        let c = decode_char(&sp, self.lit_opts);
        self.literal_diags(&t, &sp, &c.diags, Some(c.encoding));
        if c.has_error {
            return self.s.act_on_error_expr(&[], t.span.lo);
        }
        let ud = suffix_loc(t.span.hi, c.ud_suffix);
        self.s.act_on_char_literal(&c, t.span.lo, ud)
    }

    /// True at a string literal.
    pub(crate) fn at_string(&self) -> bool {
        matches!(self.tok().kind, TokKind::StringLit(_) | TokKind::RawStringLit(_))
    }

    /// A sequence of adjacent string literals ([lex.string]/7), as an
    /// unevaluated operand ([lex.string.uneval]) when `unevaluated`.
    pub(crate) fn parse_string_literal(&mut self, unevaluated: bool) -> PExpr {
        let mut toks = Vec::new();
        let mut spellings = Vec::new();
        while self.at_string() {
            let t = self.bump();
            spellings.push(self.spelling_of(&t));
            toks.push(t);
        }
        if toks.is_empty() {
            self.error_here(DiagId::parse_expected_expression);
            return None;
        }
        let mut pieces: Vec<(StringLiteral<'_>, Loc)> = Vec::new();
        let mut error = false;
        for (t, sp) in toks.iter().zip(&spellings) {
            let s = if unevaluated {
                decode_unevaluated_string(sp, self.lit_opts)
            } else {
                decode_string(sp, self.lit_opts)
            };
            self.literal_diags(t, sp, &s.diags, None);
            error |= s.has_error;
            pieces.push((s, t.span.lo));
        }
        if error {
            return Some(self.s.act_on_error_expr(&[], toks[0].span.lo));
        }
        let ud = toks
            .iter()
            .zip(&pieces)
            .find(|(_, p)| p.0.ud_suffix.is_some())
            .map_or(Loc::INVALID, |(t, p)| suffix_loc(t.span.hi, p.0.ud_suffix));
        Some(self.s.act_on_string_literal(&pieces, unevaluated, ud))
    }

    /// The value of the unevaluated string literal token `t` (for a
    /// linkage specification), lossily as UTF-8.
    pub(crate) fn unevaluated_text(&self, t: &Token) -> String {
        let sp = self.spelling_of(t);
        let s = decode_unevaluated_string(&sp, self.lit_opts);
        self.literal_diags(t, &sp, &s.diags, None);
        match &s.units {
            CodeUnits::U8(b) => String::from_utf8_lossy(b).into_owned(),
            _ => String::new(),
        }
    }
}

/// The location of a literal's ud-suffix: its last `suffix.len()` bytes
/// before the token end `hi` ([lex.ext]).
fn suffix_loc(hi: Loc, suffix: Option<&str>) -> Loc {
    let n = suffix.map_or(0, str::len);
    u32::try_from(n).ok().and_then(|n| hi.to_raw().checked_sub(n)).map_or(hi, Loc::from_raw)
}
