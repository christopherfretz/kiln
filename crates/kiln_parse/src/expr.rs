//! Expressions ([expr]; spec/parse.md §5).
//!
//! Binary operators use operator-precedence parsing with clang's
//! precedence levels; everything below a cast-expression is recursive
//! descent. A `None` result ([`PExpr`]) means a syntax error was reported
//! and the caller must recover; semantic errors yield an error expression.

use kiln_ast::expr::{
    BinaryOp, ExprKind, GenericAssoc, GenericControl, PredefinedKind, SourceLocKind, TraitKind,
    TypeTrait, UnaryOp,
};
use kiln_ast::names::DeclName;
use kiln_ast::{ExprId, QualType};
use kiln_diag::DiagId;
use kiln_lex::literal::{
    CodeUnits, NumberClass, StringLiteral, classify_number, decode_char, decode_string,
    decode_unevaluated_string,
};
use kiln_lex::{Punct, Std, TokKind, Token};
use kiln_sema::expr::CastStyle;
use kiln_sema::{
    DeclSite, DeclaratorContext, NameClass, OffsetofComponent, SpecialName, TypeSpecKind,
};
use kiln_source::{Loc, Span};

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

/// The operand of `sizeof`/`alignof`/`typeof` ([`Parser::parse_trait_operand`]).
pub(crate) enum TraitOperand {
    /// `( type-id )`, with the `)`.
    Type(QualType, Loc),
    /// A unary-expression; `None` when it failed to parse (an `alignof`
    /// still warns, as clang).
    Expr(Option<ExprId>),
}

/// True for a simple-type-specifier keyword that can begin a functional
/// cast (`int(x)`; not `__typeof_unqual` or `__auto_type`, as clang).
fn is_simple_type_kw(k: Kw) -> bool {
    is_type_specifier_kw(k)
        && !matches!(
            k,
            Kw::Const
                | Kw::GnuAutoType
                | Kw::TypeofUnqual
                | Kw::Volatile
                | Kw::Restrict
                | Kw::Atomic
                | Kw::Nonnull
                | Kw::Nullable
                | Kw::NullUnspecified
                | Kw::NullableResult
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
        if self.at_kw(Kw::CoYield) {
            return self.parse_coyield_expr();
        }
        let start = self.pos();
        let Some(lhs) = self.parse_cast_expr() else {
            // clang's `ParseAssignmentExpression` goes on to the operators
            // after an operand that is missing (`(= x)`: "expected
            // expression" at the `=`, then at what follows it); the
            // expression is erroneous.
            if self.pos() == start && self.cur_binop_prec().is_some_and(|p| p >= prec::ASSIGN) {
                let err = self.s.act_on_error_expr(&[], self.loc());
                let _ = self.parse_rhs(err, prec::ASSIGN);
            }
            return None;
        };
        self.parse_rhs(lhs, prec::ASSIGN)
    }

    /// yield-expression ([expr.yield]): `co_yield assignment-expression`
    /// or `co_yield braced-init-list` (clang's `ParseCoyieldExpression`).
    fn parse_coyield_expr(&mut self) -> PExpr {
        let loc = self.bump().span.lo;
        let e = if self.at(Punct::LBrace) {
            let l = self.parse_init_list().ok()?;
            self.s.act_on_braced_arg(l)
        } else {
            self.parse_assignment_expr()?
        };
        Some(self.s.act_on_coyield(e, loc))
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

    /// constant-expression ([expr.const]): a conditional-expression,
    /// manifestly constant-evaluated (an immediate function context,
    /// [expr.const]/16).
    pub fn parse_constant_expr(&mut self) -> PExpr {
        self.s.enter_constant_evaluated();
        // clang 19's `ActOnConstantExpression` corrects the delayed typos
        // of a constant-expression (a template argument) at once, with no
        // filter (WP-408).
        let typos = self.begin_typo_correction();
        let r = self.parse_cast_expr().and_then(|lhs| self.parse_rhs(lhs, prec::COND));
        self.correct_typos(typos, r.is_some());
        self.s.exit_constant_evaluated(r);
        r
    }

    /// Opens a clang 19 delayed-typo correction scope (none at a later
    /// compat level) for an expression clang 19 passes to
    /// `CorrectDelayedTyposInExpr` with no initialization filter as soon
    /// as it is parsed (a constant-expression, a default argument or
    /// member initializer, an element of a braced list; spec/parse.md
    /// "Compat-19 correction scopes", WP-408).
    pub(crate) fn begin_typo_correction(&mut self) -> Option<usize> {
        (self.s.clang_compat < 22).then(|| self.s.begin_delayed_typos())
    }

    /// Closes the scope [`Self::begin_typo_correction`] opened: a parsed
    /// expression (`parsed`) has its delayed typos corrected now, even
    /// inside an enclosing scope; a failed one leaves them to the
    /// enclosing scope (clang corrects nothing in an `ExprError`).
    pub(crate) fn correct_typos(&mut self, typos: Option<usize>, parsed: bool) {
        match typos {
            Some(m) if parsed => self.s.flush_delayed_typos(m),
            Some(m) => self.s.end_delayed_typos(m),
            None => {}
        }
    }

    /// An expression whose operators all have precedence `min` or higher
    /// (`min` = 1 is a full expression).
    pub fn parse_expr_prec(&mut self, min: u8) -> PExpr {
        let lhs = self.parse_cast_expr()?;
        self.parse_rhs(lhs, min.max(1))
    }

    /// clang's `ParseSimpleExpressionList` for a C-style cast's
    /// parenthesized operand after its first element `first` and a comma:
    /// the assignment-expressions are all parsed before their comma
    /// operators are built, left to right at the `(` `at`.
    fn parse_paren_list_rest(&mut self, first: ExprId, at: Loc) -> PExpr {
        let mut elems = vec![self.parse_assignment_expr()?];
        while self.cur_binop().is_some_and(|(_, op)| op == Some(BinaryOp::Comma)) {
            self.bump();
            elems.push(self.parse_assignment_expr()?);
        }
        let mut lhs = first;
        for e in elems {
            lhs = self.s.act_on_binary(BinaryOp::Comma, lhs, e, at);
        }
        Some(lhs)
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
        if self.greater_ends_arg() && matches!(p, Punct::Greater | Punct::GreaterGreater) {
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
        if self.greater_ends_arg() && matches!(p, Punct::Greater | Punct::GreaterGreater) {
            return None;
        }
        let op = binop(p)?.1?;
        (Self::punct_of(self.tok_at(i + 1)) == Some(Punct::Ellipsis)).then_some(op)
    }

    /// Runs `f` with `>` treated as an operator or not (clang's
    /// `GreaterThanIsOperatorScope`).
    pub(crate) fn with_greater_op<T>(&mut self, on: bool, f: impl FnOnce(&mut Self) -> T) -> T {
        let saved = std::mem::replace(&mut self.greater_is_op, on);
        let depth = self.total_depth();
        let saved_depth = std::mem::replace(&mut self.greater_off_depth, depth);
        let r = f(self);
        self.greater_is_op = saved;
        self.greater_off_depth = saved_depth;
        r
    }

    /// clang's `ParseRHSOfBinaryExpression`.
    fn parse_rhs(&mut self, mut lhs: ExprId, min: u8) -> PExpr {
        let comma_at = self.comma_loc.take();
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
            if let Some(at) = comma_at
                && op == Some(BinaryOp::Comma)
            {
                return self.parse_paren_list_rest(lhs, at);
            }
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
        let name = if n.kind() == ExprKind::CXXDependentScopeMemberExpr {
            self.s.ast.dependent_member_name(lhs)
        } else {
            self.s.ast.dependent_scope_ref_name(lhs)
        };
        let text = self.s.decl_name_str(name);
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

    /// GNU `&& identifier` (labels as values; clang `ParseCastExpression`,
    /// `tok::ampamp`): "use of GNU address-of-label extension" at the
    /// `&&`, then the label is looked up or created and marked used.
    /// No postfix suffix follows. Outside a function body the operand is
    /// rejected at the identifier (spec/parse.md §6.1).
    fn parse_addr_label(&mut self, amp: Loc) -> Option<ExprId> {
        self.bump();
        let Some(name) = self.ident() else {
            let d = self.s.diag(DiagId::parse_expected, self.loc()).arg("identifier");
            self.emit(d);
            return None;
        };
        let nl = self.bump().span.lo;
        self.s.act_on_addr_label(name, nl, amp)
    }

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
                if op == UnaryOp::AddrOf {
                    self.addr_operand_at = Some(self.pos());
                }
                let e = self.parse_cast_expr()?;
                return Some(self.s.act_on_unary(op, e, loc));
            }
            if p == Punct::AmpAmp {
                return self.parse_addr_label(loc);
            }
            if let Some((open, is_type, ambiguous)) = self.open_paren_template_scope() {
                if is_type && (!ambiguous || self.ambiguous_paren_is_cast(self.pos())) {
                    return self.parse_c_style_cast_rest(open);
                }
                let e = self.with_greater_op(true, |p| p.parse_paren_rest(open))?;
                return self.parse_postfix_suffix(e);
            }
            if p == Punct::LParen {
                let (is_type, ambiguous) =
                    self.is_cxx_type_id_at(self.pos() + 1, TypeIdContext::InParens);
                if is_type && (!ambiguous || self.ambiguous_paren_is_cast(self.pos() + 1)) {
                    return self.parse_c_style_cast();
                }
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
            Some(Kw::DataSizeof) => return self.parse_sizeof_alignof(TraitKind::DataSizeOf),
            Some(Kw::Extension) => {
                // GNU `__extension__ e`: clang keeps a `UO_Extension`
                // node, which `decltype` and the mangler see
                // (spec/abi.md, WP-1029); sema looks through it.
                self.bump();
                let e = self.parse_cast_expr()?;
                return Some(self.s.act_on_unary(UnaryOp::Extension, e, loc));
            }
            Some(k @ (Kw::Real | Kw::Imag)) => {
                // GNU `__real__ e` / `__imag__ e` (clang's `ParseCastExpression`).
                self.bump();
                let e = self.parse_cast_expr()?;
                let op = if k == Kw::Real { UnaryOp::Real } else { UnaryOp::Imag };
                return Some(self.s.act_on_unary(op, e, loc));
            }
            Some(Kw::Noexcept) => return self.parse_noexcept_expr(),
            Some(Kw::New | Kw::Delete) => return self.parse_new_delete_expr(),
            Some(Kw::CoAwait) => {
                // [expr.await]: `co_await cast-expression` (clang's
                // `ParseCastExpression`, `kw_co_await`).
                self.bump();
                let e = self.parse_cast_expr()?;
                return Some(self.s.act_on_coawait(e, loc));
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
        // As a lambda body: its statements end their own full-expressions
        // (WP-330).
        let typos = self.s.suspend_delayed_typos();
        let (body, _) = self.parse_compound_in(true, true);
        self.s.resume_delayed_typos(typos);
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, lparen);
        Some(self.s.act_on_stmt_expr(scope, body, lparen, rparen))
    }

    /// [expr.cast], [dcl.ambig.res]/2: the parentheses whose contents
    /// start at token `inner` hold an ambiguous type-id (`(T())`, `(T(*)())`); clang's
    /// `ParseCXXAmbiguousParenExpression` decides on the tokens after the
    /// `)`. A `{` makes it a compound literal and a token that starts a
    /// cast-expression makes it a cast (`(T())x`, `(T())*x`); anything
    /// else (`(T());`, `(T())/x`, `(T())()`) makes it a parenthesized
    /// expression.
    fn ambiguous_paren_is_cast(&self, inner: usize) -> bool {
        let mut i = inner;
        let after = loop {
            match self.tok_at(i).kind {
                TokKind::Eof => return true,
                TokKind::Punct(Punct::RParen) => break i + 1,
                TokKind::Punct(Punct::LParen | Punct::LSquare | Punct::LBrace) => {
                    let Some(j) = self.skip_balanced(i) else { return true };
                    i = j;
                }
                _ => i += 1,
            }
        };
        let t = self.tok_at(after);
        match t.kind {
            TokKind::Punct(Punct::LBrace) => true,
            TokKind::Punct(Punct::LParen) => {
                self.tok_at(after + 1).kind != TokKind::Punct(Punct::RParen)
            }
            _ => self.starts_cast_expr(t),
        }
    }

    /// Whether `t` can begin a cast-expression: the tokens clang's
    /// `ParseCastExpression` accepts without setting `NotCastExpr`.
    fn starts_cast_expr(&self, t: Token) -> bool {
        match t.kind {
            TokKind::PpNumber
            | TokKind::CharLit(_)
            | TokKind::StringLit(_)
            | TokKind::RawStringLit(_) => true,
            TokKind::Punct(p) => matches!(
                p,
                Punct::LParen
                    | Punct::LSquare
                    | Punct::PlusPlus
                    | Punct::MinusMinus
                    | Punct::Amp
                    | Punct::AmpAmp
                    | Punct::Star
                    | Punct::Plus
                    | Punct::Minus
                    | Punct::Exclaim
                    | Punct::Tilde
                    | Punct::ColonColon
                    | Punct::CaretCaret
                    | Punct::SpliceOpen
            ),
            TokKind::Identifier(_) => match self.kw_of(t) {
                None => true,
                Some(k) => matches!(
                    k,
                    Kw::Alignof
                        | Kw::GnuAlignof
                        | Kw::DataSizeof
                        | Kw::Auto
                        | Kw::Bool
                        | Kw::Char
                        | Kw::Char8
                        | Kw::Char16
                        | Kw::Char32
                        | Kw::ConstCast
                        | Kw::CoAwait
                        | Kw::Decltype
                        | Kw::Delete
                        | Kw::Double
                        | Kw::DynamicCast
                        | Kw::False
                        | Kw::Float
                        | Kw::Float128
                        | Kw::Float16
                        | Kw::Float32
                        | Kw::Float64
                        | Kw::StdFloat128
                        | Kw::Float32x
                        | Kw::Float64x
                        | Kw::Bf16
                        | Kw::Fp16
                        | Kw::Int
                        | Kw::Int128
                        | Kw::Long
                        | Kw::New
                        | Kw::Noexcept
                        | Kw::Nullptr
                        | Kw::GnuNull
                        | Kw::Operator
                        | Kw::ReinterpretCast
                        | Kw::Requires
                        | Kw::Short
                        | Kw::Signed
                        | Kw::Sizeof
                        | Kw::StaticCast
                        | Kw::This
                        | Kw::True
                        | Kw::Typeid
                        | Kw::Typename
                        | Kw::Typeof
                        | Kw::Unsigned
                        | Kw::Void
                        | Kw::WChar
                        | Kw::Extension
                        | Kw::BuiltinOffsetof
                        | Kw::BuiltinChooseExpr
                        | Kw::Generic
                        | Kw::BuiltinBitCast
                        | Kw::BuiltinVaArg
                        | Kw::BuiltinConvertVector
                        | Kw::BuiltinFile
                        | Kw::BuiltinFileName
                        | Kw::BuiltinFunction
                        | Kw::Func
                        | Kw::FunctionName
                        | Kw::PrettyFunction
                        | Kw::BuiltinLine
                        | Kw::BuiltinColumn
                        | Kw::BuiltinSourceLocation
                ),
            },
            _ => false,
        }
    }

    /// `( type-id ) cast-expression`.
    fn parse_c_style_cast(&mut self) -> PExpr {
        let open = self.bump().span.lo;
        self.parse_c_style_cast_rest(open)
    }

    /// At a `(` followed by a qualified name whose class template
    /// specialization qualifier ([`Parser::template_scope_end`]) or
    /// `decltype(e)::` qualifier ([`Parser::decltype_scope_end`]) the
    /// lookahead cannot resolve: consumes the `(`, parses the qualifier
    /// ahead, and says whether a type-id follows (and whether that was
    /// ambiguous, [`Parser::is_cxx_type_id_at`]). `None` (and nothing
    /// consumed) at any other token. With the decltype qualifier parsed,
    /// `sizeof(decltype(m)::key_type)` is a type operand whatever follows
    /// the `)` (WP-811).
    fn open_paren_template_scope(&mut self) -> Option<(Loc, bool, bool)> {
        if !self.at(Punct::LParen)
            || (self.template_scope_end(self.pos() + 1).is_none()
                && self.decltype_scope_end(self.pos() + 1).is_none())
        {
            return None;
        }
        let open = self.bump().span.lo;
        self.annotate_decltype_scope_here();
        let (is_type, ambiguous) = self.is_cxx_type_id_at(self.pos(), TypeIdContext::InParens);
        Some((open, is_type, ambiguous))
    }

    /// [`Parser::parse_c_style_cast`] after the `(` at `open`.
    fn parse_c_style_cast_rest(&mut self, open: Loc) -> PExpr {
        let ty = self.parse_type_id_checked();
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
        if self.at(Punct::LParen) {
            self.cast_paren_list = Some(self.loc());
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

    /// `sizeof`/`alignof`/`__alignof`/`__datasizeof` ([expr.sizeof],
    /// [expr.alignof]; `__datasizeof` is clang's size without tail padding).
    fn parse_sizeof_alignof(&mut self, kind: TraitKind) -> PExpr {
        let spelling = (kind == TraitKind::AlignOf).then(|| self.spelling_of(&self.tok()));
        let loc = self.bump().span.lo;
        if kind == TraitKind::SizeOf && self.at(Punct::Ellipsis) {
            return self.parse_sizeof_pack(loc);
        }
        match self.parse_trait_operand()? {
            TraitOperand::Type(ty, rparen) => Some(self.s.act_on_trait_type(kind, ty, loc, rparen)),
            TraitOperand::Expr(e) => {
                // [expr.alignof]/1 takes a type-id; `alignof e` is a GNU
                // extension (clang `ext_alignof_expr`, at the keyword,
                // before the operand is checked, even an invalid one:
                // `alignof(F::x)` with `F` incomplete). Not `__alignof`.
                if let Some(sp) = spelling {
                    let d =
                        self.s.diag(DiagId::sema_alignof_expression, loc).arg(format!("'{sp}'"));
                    self.emit(d);
                }
                Some(self.s.act_on_trait_expr(kind, e?, loc))
            }
        }
    }

    /// The operand of `sizeof`/`alignof` or GNU `typeof`, after the
    /// keyword (clang `ParseExprAfterUnaryExprOrTypeTrait`): `( type-id )`,
    /// or an unevaluated unary-expression (a parenthesized one takes its
    /// postfix suffix). `None` after an error.
    pub(crate) fn parse_trait_operand(&mut self) -> Option<TraitOperand> {
        self.s.enter_unevaluated();
        let scoped = self.open_paren_template_scope();
        self.s.exit_unevaluated();
        if let Some((open, is_type, _)) = scoped {
            if is_type {
                let Some(ty) = self.parse_type_id_checked() else {
                    self.skip_until(&[Punct::RParen], false, true);
                    return None;
                };
                let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
                return Some(TraitOperand::Type(ty, rparen));
            }
            self.s.enter_unevaluated();
            let e = self
                .with_greater_op(true, |p| p.parse_paren_rest(open))
                .and_then(|e| self.parse_postfix_suffix(e));
            self.s.exit_unevaluated();
            return Some(TraitOperand::Expr(e));
        }
        if self.at(Punct::LParen)
            && self.is_cxx_type_id_at(self.pos() + 1, TypeIdContext::InParens).0
        {
            let open = self.bump().span.lo;
            let Some(ty) = self.parse_type_id_checked() else {
                self.skip_until(&[Punct::RParen], false, true);
                return None;
            };
            let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
            return Some(TraitOperand::Type(ty, rparen));
        }
        self.s.enter_unevaluated();
        let e = self.parse_cast_expr();
        self.s.exit_unevaluated();
        Some(TraitOperand::Expr(e))
    }

    /// A builtin type trait `__trait ( ... )` (spec/sema-templates.md
    /// §9.1, §9.4): a type-id list (each possibly a pack expansion), the
    /// type and dimension of `__array_extent`, or the unevaluated operand
    /// of `__is_lvalue_expr` / `__is_rvalue_expr`. Each type-id is parsed
    /// as a template type argument (clang `ParseTypeTrait` /
    /// `ParseArrayTypeTrait` use `DeclaratorContext::TemplateTypeArg`): a
    /// qualified function type is allowed ([dcl.fct]/6) and a placeholder
    /// is "not allowed in template argument".
    pub(crate) fn parse_type_trait(&mut self, t: TypeTrait) -> ExprId {
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
                let Some(ty) = self.parse_type_id_into(None, DeclSite::TemplateArg) else {
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
                    let Some(mut ty) = self.parse_type_id_into(None, DeclSite::TemplateArg) else {
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
            // An invalid type-id (`typeid(auto)`) ends it (clang
            // `ParseCXXTypeid`), before the `<typeinfo>` check.
            let ty = ty.filter(|&t| !self.s.is_error_type(t))?;
            return Some(self.s.act_on_typeid(Err(ty), loc, rparen));
        }
        self.s.enter_typeid_operand();
        let e = self.parse_expression();
        self.s.exit_typeid_operand();
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

    /// `__builtin_va_arg ( assignment-expression , type-id )` ([cstdarg.syn],
    /// C 7.16.1.1; clang `ParseBuiltinPrimaryExpression`).
    fn parse_builtin_va_arg(&mut self) -> PExpr {
        let loc = self.bump().span.lo;
        let Some(open) = self.eat(Punct::LParen) else {
            let d = self
                .s
                .diag(DiagId::parse_expected_after, self.expected_loc())
                .arg("'__builtin_va_arg'")
                .arg("'('");
            self.emit(d);
            return None;
        };
        let e = self.parse_assignment_expr();
        if self.expect(Punct::Comma).is_none() {
            self.skip_until(&[Punct::RParen], true, true);
            return None;
        }
        // The type's checks report where clang's `TypeLoc` begins: past
        // leading cv-qualifiers (a `QualifiedTypeLoc` has no location of
        // its own).
        let mut n = 0;
        while matches!(self.kw_of(self.peek(n)), Some(Kw::Const | Kw::Volatile)) {
            n += 1;
        }
        let ty_loc = self.peek(n).span.lo;
        let ty = self.parse_type_id();
        let ok = self.at(Punct::RParen);
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
        let (e, ty) = (e?, ty?);
        ok.then(|| self.s.act_on_va_arg(e, ty, ty_loc, loc, rparen))
    }

    /// `__builtin_convertvector ( assignment-expression , type-id )`
    /// (clang `ParseBuiltinPrimaryExpression`).
    fn parse_builtin_convert_vector(&mut self) -> PExpr {
        let loc = self.bump().span.lo;
        let Some(open) = self.eat(Punct::LParen) else {
            let d = self
                .s
                .diag(DiagId::parse_expected_after, self.expected_loc())
                .arg("'__builtin_convertvector'")
                .arg("'('");
            self.emit(d);
            return None;
        };
        let Some(e) = self.parse_assignment_expr() else {
            self.skip_until(&[Punct::RParen], true, true);
            return None;
        };
        if self.eat(Punct::Comma).is_none() {
            let d = self.s.diag(DiagId::parse_expected, self.expected_loc()).arg("','");
            self.emit(d);
            self.skip_until(&[Punct::RParen], true, true);
            return None;
        }
        let ty = self.parse_type_id()?;
        if !self.at(Punct::RParen) {
            let d = self.s.diag(DiagId::parse_expected, self.expected_loc()).arg("')'");
            self.emit(d);
            self.skip_until(&[Punct::RParen], true, true);
            return None;
        }
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
        Some(self.s.act_on_convert_vector(e, ty, loc, rparen))
    }

    /// `__builtin_FILE ( )` and the other source-location builtins (clang
    /// `ParseBuiltinPrimaryExpression`; [support.srcloc.cons]).
    fn parse_source_loc_expr(&mut self, kind: SourceLocKind) -> PExpr {
        let loc = self.bump().span.lo;
        let Some(open) = self.eat(Punct::LParen) else {
            let d = self
                .s
                .diag(DiagId::parse_expected_after, self.expected_loc())
                .arg(format!("'{}'", kind.spelling()))
                .arg("'('");
            self.emit(d);
            return None;
        };
        if !self.at(Punct::RParen) {
            let d = self.s.diag(DiagId::parse_expected, self.expected_loc()).arg("')'");
            self.emit(d);
            self.skip_until(&[Punct::RParen], true, true);
            return None;
        }
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
        Some(self.s.act_on_source_loc_expr(kind, loc, rparen))
    }

    /// `__builtin_choose_expr ( assignment-expression ,
    /// assignment-expression , assignment-expression )` (clang
    /// `ParseBuiltinPrimaryExpression`).
    fn parse_builtin_choose_expr(&mut self) -> PExpr {
        let loc = self.bump().span.lo;
        let Some(open) = self.eat(Punct::LParen) else {
            let d = self
                .s
                .diag(DiagId::parse_expected_after, self.expected_loc())
                .arg("'__builtin_choose_expr'")
                .arg("'('");
            self.emit(d);
            return None;
        };
        let mut ops = [None; 3];
        for (i, slot) in ops.iter_mut().enumerate() {
            if i > 0 && self.eat(Punct::Comma).is_none() {
                let d = self.s.diag(DiagId::parse_expected, self.expected_loc()).arg("','");
                self.emit(d);
                self.skip_until(&[Punct::RParen], true, true);
                return None;
            }
            let Some(e) = self.parse_assignment_expr() else {
                self.skip_until(&[Punct::RParen], true, true);
                return None;
            };
            *slot = Some(e);
        }
        if !self.at(Punct::RParen) {
            let d = self.s.diag(DiagId::parse_expected, self.expected_loc()).arg("')'");
            self.emit(d);
            self.skip_until(&[Punct::RParen], true, true);
            return None;
        }
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
        let [Some(c), Some(l), Some(r)] = ops else { return None };
        Some(self.s.act_on_choose_expr(loc, c, l, r, rparen))
    }

    /// `_Generic ( assignment-expression , generic-assoc-list )`, or with a
    /// type-name operand (clang's C2y extension), in C++ as a C11
    /// extension (clang `ParseGenericSelectionExpression`). Each
    /// association is `type-name : assignment-expression` or
    /// `default : assignment-expression`. The controlling expression is
    /// parsed unevaluated; the associations are not.
    fn parse_generic_selection(&mut self) -> PExpr {
        let loc = self.bump().span.lo;
        let d = self.s.diag(DiagId::parse_c11_generic, loc);
        self.emit(d);
        let Some(open) = self.eat(Punct::LParen) else {
            let d = self
                .s
                .diag(DiagId::parse_expected_after, self.expected_loc())
                .arg("'_Generic'")
                .arg("'('");
            self.emit(d);
            return None;
        };
        let ctrl = if self.is_cxx_type_id(TypeIdContext::AsGenericSelectionArgument).0 {
            let at = self.type_loc_begin();
            let Some(t) = self.parse_type_id() else {
                self.skip_until(&[Punct::RParen], true, true);
                return None;
            };
            let d = self.s.diag(DiagId::parse_c2y_generic_with_type_arg, at);
            self.emit(d);
            GenericControl::Type(t, at)
        } else {
            self.s.enter_unevaluated();
            let e = self.parse_assignment_expr();
            self.s.exit_unevaluated();
            let Some(e) = e else {
                self.skip_until(&[Punct::RParen], true, true);
                return None;
            };
            GenericControl::Expr(e)
        };
        let mut assocs: Vec<GenericAssoc> = Vec::new();
        let mut default_loc: Option<Loc> = None;
        while self.eat(Punct::Comma).is_some() {
            let at = if self.at_kw(Kw::Default) { self.loc() } else { self.type_loc_begin() };
            let ty = if self.at_kw(Kw::Default) {
                if let Some(prev) = default_loc {
                    let note = self.s.diag(DiagId::note_previous_default_assoc, prev);
                    let d = self.s.diag(DiagId::parse_duplicate_default_assoc, at).with_note(note);
                    self.emit(d);
                    self.skip_until(&[Punct::RParen], true, true);
                    return None;
                }
                self.bump();
                default_loc = Some(at);
                None
            } else {
                let Some(t) = self.parse_type_id() else {
                    self.skip_until(&[Punct::RParen], true, true);
                    return None;
                };
                Some(t)
            };
            if self.eat(Punct::Colon).is_none() {
                let d = self.s.diag(DiagId::parse_expected, self.expected_loc()).arg("':'");
                self.emit(d);
                self.skip_until(&[Punct::RParen], true, true);
                return None;
            }
            let Some(e) = self.parse_assignment_expr() else {
                self.skip_until(&[Punct::RParen], true, true);
                return None;
            };
            assocs.push(GenericAssoc { ty, ty_loc: at, expr: e });
        }
        if assocs.is_empty() {
            // clang `ExpectAndConsume(tok::comma)` after the operand.
            let d = self.s.diag(DiagId::parse_expected, self.expected_loc()).arg("','");
            self.emit(d);
            self.skip_until(&[Punct::RParen], true, true);
            return None;
        }
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
        Some(self.s.act_on_generic_selection(loc, ctrl, &assocs, rparen))
    }

    /// Where clang's `TypeLoc` of the type-id starting here begins: a
    /// `QualifiedTypeLoc` carries no location for its leading
    /// cv-qualifiers, so `const int` begins at `int`.
    fn type_loc_begin(&self) -> Loc {
        let mut i = 0;
        while i < 8
            && matches!(self.kw_of(self.peek(i)), Some(Kw::Const | Kw::Volatile | Kw::Restrict))
        {
            i += 1;
        }
        self.peek(i).span.lo
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

    /// Whether a postfix-expression suffix starts here (clang's
    /// `isPostfixExpressionSuffixStart`).
    fn at_postfix_suffix_start(&self) -> bool {
        matches!(
            Self::punct_of(self.tok()),
            Some(
                Punct::LSquare
                    | Punct::LParen
                    | Punct::Period
                    | Punct::Arrow
                    | Punct::PlusPlus
                    | Punct::MinusMinus
            )
        )
    }

    /// Whether the id-expression that began at `start` is the whole
    /// operand of unary `&`: right after it, with no postfix suffix after
    /// it (clang's `tryParseCXXIdExpression`). A qualified name there
    /// forms no implicit member access.
    fn whole_addr_operand(&self, start: usize) -> bool {
        self.addr_operand_at == Some(start) && !self.at_postfix_suffix_start()
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
                    // C++23 [expr.sub]: an expression-list (possibly
                    // empty, possibly braced-init-lists). Its delayed
                    // typos are held (WP-330): clang's
                    // `ParsePostfixExpressionSuffix` corrects the base's at
                    // the `]` and drops the index of a subscript missing
                    // its `]`, whose typos then wait for the end of the
                    // translation unit.
                    let mark = self.s.begin_delayed_typos();
                    let args = if self.at(Punct::RSquare) {
                        Some(Vec::new())
                    } else if self.std < Std::Cxx23 && !self.at(Punct::LBrace) {
                        // Before C++23 the index is one expression, so a
                        // top-level comma is the comma operator
                        // ([expr.sub]/1 in C++20, [depr.comma.subscript];
                        // clang `ParsePostfixExpressionSuffix`).
                        let outer = std::mem::replace(&mut self.expr_invalid, false);
                        let a = self.with_greater_op(true, Self::parse_expression);
                        let a = a.filter(|_| !std::mem::replace(&mut self.expr_invalid, outer));
                        a.map(|a| vec![a])
                    } else {
                        self.with_greater_op(true, |p| p.parse_expr_list(Punct::RSquare))
                    };
                    let Some(args) = args else {
                        // `ParseExpressionList` corrects what it parsed,
                        // then the base. The subscript is `ExprError`
                        // and its `]` is matched (WP-399).
                        self.s.flush_delayed_typos(mark);
                        self.s.correct_delayed_typos_in(e);
                        let _ = self.expect_closing(Punct::RSquare, Punct::LSquare, loc);
                        self.expr_invalid = true;
                        e = self.s.act_on_error_expr(&[e], loc);
                        continue;
                    };
                    self.s.correct_delayed_typos_in(e);
                    if !self.at(Punct::RSquare) {
                        // clang's `ParsePostfixExpressionSuffix`: no `]`
                        // after the list makes the subscript `ExprError`
                        // with no Sema action (WP-399).
                        self.s.drop_delayed_typos(mark);
                        let _ = self.expect_closing(Punct::RSquare, Punct::LSquare, loc);
                        self.expr_invalid = true;
                        e = self.s.act_on_error_expr(&[e], loc);
                        continue;
                    }
                    self.s.end_delayed_typos(mark);
                    let rsquare = self.expect_closing(Punct::RSquare, Punct::LSquare, loc);
                    // A subscript whose Sema action fails is `ExprError`
                    // (`i[1]` on an `int`): an expression-list skips to
                    // its `,` (WP-399).
                    let errors = self.s.diags.error_count();
                    e = self.s.act_on_subscript_args(e, &args, loc, rsquare);
                    if self.s.diags.error_count() > errors && self.s.is_error_expr(e) {
                        self.expr_invalid = true;
                    }
                }
                Some(Punct::LParen) => {
                    self.bump();
                    let mark = self.s.typo_mark();
                    let Some(args) = self.with_greater_op(true, Self::parse_call_args) else {
                        // clang's `ParseExpressionList` corrects the
                        // arguments it parsed after an error, then
                        // `ParsePostfixExpressionSuffix` the callee
                        // (WP-330). clang 22 goes on with a recovery
                        // expression for the call (its `)` consumed).
                        self.s.flush_typos_from(mark);
                        self.s.correct_delayed_typos_in(e);
                        if self.s.clang_compat >= 22 {
                            e = self.s.act_on_error_expr(&[e], loc);
                            continue;
                        }
                        return None;
                    };
                    let rparen = if self.at(Punct::RParen) {
                        self.expect_closing(Punct::RParen, Punct::LParen, loc)
                    } else {
                        // clang's `ParsePostfixExpressionSuffix` corrects
                        // the delayed typos of the callee and the
                        // arguments now, before what follows (WP-330); if
                        // there were any (`f(S::nom x)`, `S::nom(1`), it
                        // skips to the `)` (stopping at a `;`) with no
                        // "expected ')'" for the unmatched `(` (WP-318).
                        // clang 22 has no delayed typos: it skips so when
                        // the callee or an argument contains errors
                        // (`f(u<int>(1) 5)`, WP-391).
                        let mut had_typo = self.s.correct_delayed_typos_in(e);
                        for &a in &args {
                            had_typo |= self.s.correct_delayed_typos_in(a);
                        }
                        if self.s.clang_compat >= 22 {
                            had_typo = self.s.contains_errors(e)
                                || args.iter().any(|&a| self.s.contains_errors(a));
                        }
                        self.expr_invalid = true;
                        if had_typo {
                            let at = self.loc();
                            self.skip_until(&[Punct::RParen], true, false);
                            let _ = self.eat(Punct::RParen).unwrap_or(at);
                        } else {
                            // "expected ')'", and the call is `ExprError`
                            // (no overload resolution: `f(1 2)` reports
                            // nothing more).
                            let _ = self.expect_closing(Punct::RParen, Punct::LParen, loc);
                        }
                        e = self.s.act_on_error_expr(&[], loc);
                        continue;
                    };
                    e = self.s.act_on_call(e, &args, loc, rparen);
                }
                Some(p @ (Punct::Period | Punct::Arrow)) => {
                    self.bump();
                    e = self.parse_member_access_suffix(e, loc, p == Punct::Arrow)?;
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

    /// `p->~T()` / `x.~T()` / `p->~X<T>()` after `.`/`->`, at the `~`
    /// ([expr.prim.id.dtor]).
    fn parse_member_destructor_name(&mut self, e: ExprId, loc: Loc, arrow: bool) -> PExpr {
        let tl = self.bump().span.lo;
        if self.at_kw(Kw::Decltype) {
            // `p->~decltype(*p)()`: the decltype-specifier denotes the
            // destroyed type ([expr.prim.id.dtor]/1, [expr.prim.id.unqual]/1).
            let dl = self.loc();
            let (kind, ty) = self.parse_decltype_specifier();
            if kind == TypeSpecKind::DecltypeAuto {
                self.s.error(DiagId::parse_decltype_auto_invalid, dl);
                return None;
            }
            let ty = ty?;
            return Some(self.s.rebuild_member_destructor(e, ty, false, dl, loc, arrow));
        }
        let Some(name) = self.ident() else {
            self.error_here(DiagId::parse_destructor_tilde_identifier);
            return None;
        };
        let nl = self.bump().span.lo;
        if self.at(Punct::Less) {
            // `p->~X<T>()`: the simple-template-id names the destroyed
            // class type.
            let class = self.s.classify_name(name, None);
            return match self.parse_template_id_rest(class, name, nl) {
                Ok(TemplateIdRef::Type(t)) => {
                    Some(self.s.rebuild_member_destructor(e, t, false, nl, loc, arrow))
                }
                _ => None,
            };
        }
        Some(self.s.act_on_member_destructor(e, name, tl, nl, loc, arrow))
    }

    /// The member designator after `.`/`->` (the operator token at `loc`
    /// already consumed): `[template] id-expression` or a
    /// pseudo-destructor-name ([expr.ref]/1, [expr.prim.id.dtor]).
    #[allow(clippy::too_many_lines, reason = "one path per member designator form")]
    fn parse_member_access_suffix(&mut self, e: ExprId, loc: Loc, arrow: bool) -> PExpr {
        // `e.[: r :]`, `p->[: r :]` ([expr.ref]/1 with a splice-expression).
        if self.at_splice() && self.decltype_scope_end(self.pos()).is_none() {
            let open = self.loc();
            let (r, close) = self.parse_splice_specifier()?;
            return Some(self.s.act_on_member_splice(e, r, open, close, loc, arrow));
        }
        // `e.template [: r :] < args >`: a member template named by a
        // splice is not implemented (spec/reflection.md §5).
        if self.kw() == Some(Kw::Template) && self.peek_is(1, Punct::SpliceOpen) {
            self.bump();
            let open = self.loc();
            let (r, _) = self.parse_splice_specifier()?;
            if self.at(Punct::Less) {
                let _ = self.parse_template_arg_list();
            }
            self.unsupported("a member access naming a template by a splice", open);
            return Some(self.s.act_on_error_expr(&[e, r], open));
        }
        let mut template_kw = self.eat_kw(Kw::Template).is_some();
        // `p->T::~T()`, `p->N::T::~T()` on a scalar or dependent object: a
        // pseudo-destructor-name whose last `type-name ::` is not a
        // nested-name-specifier ([expr.prim.id.dtor]/2; clang's
        // `MayBePseudoDestructor`).
        if !template_kw
            && let Some(cc) = self.pseudo_dtor_colons()
            && self.s.may_be_pseudo_destructor(e, arrow)
        {
            return self.parse_pseudo_destructor(e, loc, arrow, cc);
        }
        // `e.N::m`, `p->B::f`, `e.::N::m` ([expr.ref]/1 with a
        // qualified-id; [basic.lookup.qual.general]/2 looks the first
        // component up in the object's class, then here).
        let qual = if self.at_member_qualifier() {
            self.s.begin_member_qualifier(e, arrow);
            let q = self.parse_nested_name_nns();
            self.s.end_member_qualifier();
            let Ok(q) = q else { return None };
            // A leading `template` belonged to the qualifier's first
            // component (`e.template B<T>::m`, CWG1835); only one after
            // the qualifier applies to the member's name.
            if q.is_some() {
                template_kw = self.eat_kw(Kw::Template).is_some();
            }
            q
        } else {
            None
        };
        if let Some(q) = qual
            && self.at(Punct::Tilde)
        {
            // `x.N::~T()`, `p->B::~B()` on a class object
            // ([expr.prim.id.dtor], [basic.lookup.qual.general]/4).
            let tl = self.bump().span.lo;
            let Some(name) = self.ident() else {
                self.error_here(DiagId::parse_destructor_tilde_identifier);
                return None;
            };
            let nl = self.bump().span.lo;
            if self.at(Punct::Less) {
                self.unsupported("a qualified destructor name with a template-id", nl);
                return None;
            }
            return Some(self.s.act_on_member_destructor_q(
                e,
                Some(q),
                None,
                name,
                tl,
                nl,
                loc,
                arrow,
            ));
        }
        if self.at(Punct::Tilde) {
            return self.parse_member_destructor_name(e, loc, arrow);
        }
        let nl = self.loc();
        let name = if self.at_kw(Kw::Operator) {
            // `x.operator->()`, `x.operator T()`: an operator- or
            // conversion-function-id.
            self.parse_operator_function_name()?
        } else {
            let Some(name) = self.ident() else {
                self.error_here(DiagId::parse_expected_unqualified_id);
                return None;
            };
            self.bump();
            DeclName::ident(name)
        };
        if template_kw {
            // `e.template f<args>` ([temp.names]/5).
            let targs = if self.at(Punct::Less) {
                let (args, _, _) = self.parse_template_arg_list()?;
                Some(self.s.ast.types.arg_list(&args))
            } else {
                None
            };
            return Some(self.s.act_on_template_kw_member_q(e, qual, name, targs, nl, loc, arrow));
        }
        let mut e = self.s.act_on_member_q(e, qual, name, nl, loc, arrow);
        // `e.f<args>` naming a member function template ([temp.names]/3).
        if self.at(Punct::Less) && self.s.is_member_template_ref(e) {
            let (args, _, _) = self.parse_template_arg_list()?;
            let l = self.s.ast.types.arg_list(&args);
            e = self.s.act_on_member_template_args(e, l);
        } else if self.at(Punct::Less) && self.s.dependent_member_names_template(e) {
            // `this->f<args>` with `f` a member template of the current
            // instantiation that stays a dependent access
            // ([temp.names]/3.2).
            let (args, _, _) = self.parse_template_arg_list()?;
            let l = self.s.ast.types.arg_list(&args);
            e = self.s.act_on_dependent_member_template_args(e, l);
        } else if self.at(Punct::Less)
            && let Some(found) = self.s.member_named_non_template(e)
            && let Some(close) = self.potential_angle_close()
        {
            // `a.g<int>(1)` (`this->operator+<int>(b)`) with `g` no
            // template: the template argument list is skipped and the
            // access is an error (clang's `checkPotentialAngleBracket`
            // after a member access).
            let angles = self.skip_potential_angles(close);
            self.s.diagnose_expr_intended_as_template_name(name, found, nl, angles);
            self.expr_invalid = true;
            return Some(self.s.act_on_error_expr(&[e], nl));
        } else if !self.at(Punct::Less) {
            // `x.v` naming a member variable template needs its arguments.
            e = self.s.act_on_member_without_template_args(e);
        }
        Some(e)
    }

    /// The index of the `::` before the `~` when the member designator at
    /// the current token is `[nested-name-specifier] identifier :: ~`
    /// (a possible pseudo-destructor-name, [expr.prim.id.dtor]).
    fn pseudo_dtor_colons(&self) -> Option<usize> {
        let colons = |p: &Self, i: usize| p.tok_at(i).kind == TokKind::Punct(Punct::ColonColon);
        let mut i = self.pos();
        if colons(self, i) {
            i += 1;
        }
        loop {
            if self.kw_of(self.tok_at(i)) == Some(Kw::Template) {
                i += 1;
            }
            self.ident_of(self.tok_at(i))?;
            let mut j = i + 1;
            let template_id = self.tok_at(j).kind == TokKind::Punct(Punct::Less);
            if template_id {
                match self.skip_template_args(j) {
                    Some((close, false)) => j = close + 1,
                    _ => return None,
                }
            }
            if !colons(self, j) {
                return None;
            }
            if self.tok_at(j + 1).kind == TokKind::Punct(Punct::Tilde) {
                return (!template_id).then_some(j);
            }
            i = j + 1;
        }
    }

    /// `p->[S::] F :: ~ Q` with the `::` before the `~` at token `cc`
    /// ([expr.prim.id.dtor]/2, [basic.lookup.qual.general]/4): `S` is
    /// parsed as a nested-name-specifier, `F` and `Q` are type-names.
    fn parse_pseudo_destructor(&mut self, e: ExprId, loc: Loc, arrow: bool, cc: usize) -> PExpr {
        let first_at = cc - 1;
        let qual = if self.pos() + 1 == first_at {
            // `::F::~Q`.
            let start = self.bump().span.lo;
            Some(kiln_sema::using::Qualifier {
                ctx: self.s.ast.tu,
                nns: self.s.nns_global(),
                loc: start,
            })
        } else if self.pos() < first_at {
            self.s.begin_member_qualifier(e, arrow);
            let q = self.parse_nested_name_upto(Some(first_at - 1));
            self.s.end_member_qualifier();
            let q = q.ok()?;
            if self.pos() + 1 != first_at {
                return None;
            }
            self.bump();
            q
        } else {
            None
        };
        let first = self.ident()?;
        let fl = self.bump().span.lo;
        self.bump();
        let tl = self.bump().span.lo;
        if self.at_kw(Kw::Decltype) {
            // `p->~decltype(*p)()`: the decltype-specifier denotes the
            // destroyed type ([expr.prim.id.dtor]/1, [expr.prim.id.unqual]/1).
            let dl = self.loc();
            let (kind, ty) = self.parse_decltype_specifier();
            if kind == TypeSpecKind::DecltypeAuto {
                self.s.error(DiagId::parse_decltype_auto_invalid, dl);
                return None;
            }
            let ty = ty?;
            return Some(self.s.rebuild_member_destructor(e, ty, false, dl, loc, arrow));
        }
        let Some(name) = self.ident() else {
            self.error_here(DiagId::parse_destructor_tilde_identifier);
            return None;
        };
        let nl = self.bump().span.lo;
        if self.at(Punct::Less) {
            self.unsupported("a qualified destructor name with a template-id", nl);
            return None;
        }
        Some(self.s.act_on_member_destructor_q(
            e,
            qual,
            Some((first, fl)),
            name,
            tl,
            nl,
            loc,
            arrow,
        ))
    }

    /// True at a nested-name-specifier after `.`/`->`: `::`, `N::`,
    /// `X<args>::` or `decltype(e)::`.
    fn at_member_qualifier(&mut self) -> bool {
        if self.at(Punct::ColonColon) || self.decltype_scope_end(self.pos()).is_some() {
            return true;
        }
        if self.ident().is_none() {
            return false;
        }
        if self.peek_is(1, Punct::ColonColon) {
            return true;
        }
        self.peek_is(1, Punct::Less)
            && matches!(
                self.skip_template_args(self.pos() + 1),
                Some((close, false))
                    if self.tok_at(close + 1).kind == TokKind::Punct(Punct::ColonColon)
            )
    }

    /// An unqualified operator- or conversion-function-id as a primary
    /// expression ([expr.prim.id.unqual]/1): `operator()(x)` in a member
    /// function is `this->operator()(x)`.
    fn parse_operator_id_expression(&mut self, loc: Loc) -> PExpr {
        let name = self.parse_operator_function_name()?;
        if self.at_operator_template_args(name, None, None) {
            return self.parse_operator_template_id(name, loc, None, None);
        }
        if self.at(Punct::LParen) {
            Some(self.s.act_on_call_name_dn(name, loc))
        } else {
            Some(self.s.act_on_id_expression_dn(name, loc, None))
        }
    }

    /// An operator-function-id or conversion-function-id at `operator`
    /// ([over.oper.general]/1, [class.conv.fct]/1), as a declaration
    /// name. `None` after diagnosing (literal operators are not yet
    /// supported in expressions).
    pub(crate) fn parse_operator_function_name(&mut self) -> Option<DeclName> {
        let mut d = kiln_sema::Declarator::new(DeclaratorContext::TypeName, self.loc());
        self.parse_special_declarator_id(&mut d);
        if d.invalid {
            return None;
        }
        match d.special {
            Some((SpecialName::Operator(op), _)) => Some(DeclName::operator(op)),
            Some((SpecialName::LiteralOperator(s), _)) => Some(DeclName::literal_operator(s)),
            Some((SpecialName::Conversion(t), _)) => {
                let c = self.s.ast.types.canonical(t);
                Some(self.s.ast.names.conversion(c))
            }
            _ => None,
        }
    }

    /// The arguments of a call after its `(` ([expr.call]): an
    /// expression-list whose initializer-clauses may be braced lists or
    /// pack expansions ([temp.variadic]/5).
    fn parse_call_args(&mut self) -> Option<Vec<ExprId>> {
        if self.at(Punct::RParen) {
            return Some(Vec::new());
        }
        self.parse_expr_list(Punct::RParen)
    }

    /// A non-empty expression-list of initializer-clauses closed by
    /// `close` ([expr.post.general]). clang's `ParseExpressionList`
    /// recovery (spec/parse.md "Expression-list recovery"): an invalid
    /// initializer-clause skips to the next `,` or `)` (brackets balanced,
    /// before an unmatched closer, stopping at `;` under clang 22 and past
    /// it under clang 19) and the list goes
    /// on, so a later clause's errors are reported too; after an invalid
    /// clause the list is `None`, its `)` taken (or skipped to, stopping
    /// at `;`: the callers' `SkipUntil(close, StopAtSemi)`) but its `]`
    /// left to the subscript. A clause is invalid when it fails to parse
    /// or leaves `expr_invalid` set (clang's `ExprError`).
    pub(crate) fn parse_expr_list(&mut self, close: Punct) -> Option<Vec<ExprId>> {
        let mut args = Vec::new();
        let mut saw_error = false;
        let outer_invalid = std::mem::replace(&mut self.expr_invalid, false);
        loop {
            let a = if self.at(Punct::LBrace) {
                self.parse_init_list().ok().map(|l| self.s.act_on_braced_arg(l))
            } else {
                self.parse_assignment_expr()
            };
            // An argument the parser went on past as clang's `ExprError`
            // (`expr_invalid`: `i[1] 5`, `f(1 2)`) failed too.
            let a = a.filter(|_| !std::mem::replace(&mut self.expr_invalid, false));
            let ellipsis = self.eat(Punct::Ellipsis);
            if let Some(a) = a {
                let a = match ellipsis {
                    Some(l) => self.s.act_pack_expansion(a, l),
                    None => a,
                };
                args.push(a);
            } else {
                saw_error = true;
                // clang 22 stops at a `;` (`f(u<int> 7; x)`), clang 19
                // skips past it.
                let semi = self.s.clang_compat >= 22;
                self.skip_until(&[Punct::Comma, Punct::RParen], semi, false);
            }
            if self.eat(Punct::Comma).is_none() {
                break;
            }
        }
        self.expr_invalid = outer_invalid;
        if saw_error {
            // A subscript's `]` is the caller's (clang's `consumeClose`,
            // which reports a missing one).
            if close != Punct::RSquare && self.eat(close).is_none() {
                self.skip_until(&[close], true, true);
            }
            return None;
        }
        Some(args)
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
            TokKind::Punct(Punct::LSquare) => return self.parse_lambda_expr(),
            TokKind::Punct(Punct::CaretCaret) => return self.parse_reflect_expr(),
            // A splice-expression; `[: r :]::` is a qualifier (below).
            TokKind::Punct(Punct::SpliceOpen) if self.decltype_scope_end(self.pos()).is_none() => {
                return self.parse_splice_expr();
            }
            _ => {}
        }
        // `decltype(e)::name` (WP-119): the qualifier is parsed first.
        if self.decltype_scope_end(self.pos()).is_some() && !self.annotate_decltype_scope() {
            return None;
        }
        // `X<A>::name` with `name` from a specialization of `X`: the
        // qualifier is parsed first too.
        if self.template_scope_end(self.pos()).is_some() {
            self.annotate_template_scope();
        }
        if self.at_decltype_scope() && !self.decltype_scope_ok() {
            self.skip_failed_decltype_scope();
            return Some(self.s.act_on_error_expr(&[], loc));
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
                Kw::GnuNull => {
                    self.bump();
                    Some(self.s.act_on_gnu_null(loc))
                }
                Kw::StaticCast => self.parse_named_cast(CastStyle::Static),
                Kw::ConstCast => self.parse_named_cast(CastStyle::Const),
                Kw::ReinterpretCast => self.parse_named_cast(CastStyle::Reinterpret),
                Kw::DynamicCast => self.parse_named_cast(CastStyle::Dynamic),
                Kw::Operator => self.parse_operator_id_expression(loc),
                _ if is_simple_type_kw(k) => self.parse_functional_cast(),
                Kw::This => {
                    self.bump();
                    Some(self.s.act_on_this(loc))
                }
                Kw::Typeid => self.parse_typeid(),
                Kw::BuiltinOffsetof => self.parse_builtin_offsetof(),
                Kw::BuiltinChooseExpr => self.parse_builtin_choose_expr(),
                Kw::Generic => self.parse_generic_selection(),
                Kw::BuiltinBitCast => self.parse_builtin_bit_cast(),
                Kw::BuiltinVaArg => self.parse_builtin_va_arg(),
                Kw::BuiltinConvertVector => self.parse_builtin_convert_vector(),
                Kw::Func | Kw::FunctionName | Kw::PrettyFunction => {
                    self.bump();
                    let kind = match k {
                        Kw::Func => PredefinedKind::Func,
                        Kw::FunctionName => PredefinedKind::Function,
                        _ => PredefinedKind::PrettyFunction,
                    };
                    Some(self.s.act_on_predefined_expr(kind, loc))
                }
                Kw::BuiltinFile => self.parse_source_loc_expr(SourceLocKind::File),
                Kw::BuiltinFileName => self.parse_source_loc_expr(SourceLocKind::FileName),
                Kw::BuiltinFunction => self.parse_source_loc_expr(SourceLocKind::Function),
                Kw::BuiltinLine => self.parse_source_loc_expr(SourceLocKind::Line),
                Kw::BuiltinColumn => self.parse_source_loc_expr(SourceLocKind::Column),
                Kw::BuiltinSourceLocation => {
                    self.parse_source_loc_expr(SourceLocKind::SourceLocStruct)
                }
                Kw::Requires => Some(self.parse_requires_expression()),
                // `typename T::type(e)` ([expr.type.conv]).
                Kw::Typename => self.parse_functional_cast(),
                // `template [: r :]` / `template [: r :] < args >`
                // ([expr.prim.splice]); `template [: r :] < args > ::` was
                // annotated as a scope above.
                Kw::Template if self.peek_is(1, Punct::SpliceOpen) => {
                    self.parse_template_splice_expr()
                }
                Kw::Template => {
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
            return self.parse_name_primary(loc);
        }
        self.error_here(DiagId::parse_expected_expression);
        None
    }

    /// A primary expression starting with a name or `::`: a functional
    /// cast of a type name, or an id-expression.
    fn parse_name_primary(&mut self, loc: Loc) -> PExpr {
        if let Some((end, NameClass::Type(..), name)) = self.scan_name(self.pos()) {
            // An unqualified type name not followed by `(` or `{` is
            // not looked at as a type (clang only annotates it before
            // those), so the id-expression names a type.
            // A pack-index-specifier `T...[i]` ([dcl.type.pack.index]) is
            // part of the type: `T...[0]{}` is a functional cast.
            let after = if end == self.pos() + 1
                && self.peek_is(1, Punct::Ellipsis)
                && self.peek_is(2, Punct::LSquare)
            {
                self.skip_balanced(end + 1).unwrap_or(end)
            } else {
                end
            };
            if end == self.pos() + 1
                && !matches!(
                    Self::punct_of(self.tok_at(after)),
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
        // A class template's injected-class-name used as a type
        // ([temp.local]/1): `X(args)` / `X{args}` in its definition.
        if let Some((end, _, NameClass::TypeTemplate(d), _, q)) = self.scan_name_qual_at(self.pos())
            && matches!(Self::punct_of(self.tok_at(end)), Some(Punct::LParen | Punct::LBrace))
            && (end == self.pos() + 1 && self.s.names_injected_class(d)
                || self.names_ctad_template(q, d))
        {
            // Or a class template named for class template argument
            // deduction ([dcl.type.class.deduct]/2): `C(args)`.
            return self.parse_functional_cast();
        }
        self.parse_id_expression_rest()
    }

    /// The template-id of an id-expression whose name is a template (or
    /// follows `template`): a function template's overload set, a variable
    /// template or concept's value, or an error for a type template
    /// ([temp.names]/1).
    fn template_id_expr(
        &mut self,
        qual: Option<kiln_sema::using::Qualifier>,
        (class, name, nl): (NameClass, kiln_source::symbol::Symbol, Loc),
        begin: Loc,
        template_loc: Option<Loc>,
    ) -> ExprId {
        let res = self.parse_template_id_rest(class, name, nl);
        let r_angle = self.prev_lo();
        match res {
            Ok(TemplateIdRef::Overload(e)) => {
                let e = self.s.widen_function_template_id(e, name, qual.map(|q| q.ctx));
                self.s.set_template_id_end(e, r_angle);
                if let Some(q) = qual {
                    self.s.set_template_id_begin(e, begin, nl);
                    self.s.set_template_id_qualifier(e, q.nns);
                    self.s.set_template_kw(e, template_loc);
                }
                if !self.template_id_is_callee(begin) {
                    let r = self.s.resolve_single_template_id(e);
                    self.s.set_template_id_end(r, r_angle);
                    return r;
                }
                if qual.is_none() && self.at(Punct::LParen) {
                    self.s.act_on_template_id_call_name(e);
                }
                e
            }
            Ok(TemplateIdRef::Value(e)) => {
                self.s.set_template_id_end(e, r_angle);
                if let Some(q) = qual {
                    self.s.set_template_id_begin(e, begin, nl);
                    self.s.set_template_id_qualifier(e, q.nns);
                    self.s.set_template_kw(e, template_loc);
                }
                e
            }
            Ok(TemplateIdRef::Type(_)) => {
                let t = self.s.ast.symbols().as_str(name).to_owned();
                let d = self.s.diag(DiagId::sema_unexpected_typedef_ident, nl).arg(t);
                self.emit(d);
                self.s.act_on_error_expr(&[], nl)
            }
            Err(()) => self.s.act_on_error_expr(&[], nl),
        }
    }

    /// Whether the `<` here opens a template argument list that closes
    /// on a whole `>`, or -- inside an enclosing template argument list
    /// (`T<val<int>>`, [temp.names]/4) -- on the first `>` of a `>>`.
    fn closes_assumed_template_args(&self) -> bool {
        matches!(self.skip_template_args(self.pos()), Some((_, split)) if !split || !self.greater_is_op)
    }

    /// An undeclared template-id that is not called (`(undeclared<int> * 2)`),
    /// at its `<` ([temp.names]/3.3): the name is still a template-name, so
    /// the list is its arguments (not `<`/`>` operators), and the id names
    /// nothing: "use of undeclared identifier" and an invalid expression
    /// (clang 22's `ExprError` from the annotated template-id). Before an
    /// operator or a closing token the enclosing expression goes on
    /// parsing around an error operand, which diagnoses nothing more;
    /// before anything else the enclosing construct skips past it. WP-372.
    fn undeclared_template_id_expr(
        &mut self,
        name: kiln_source::symbol::Symbol,
        nl: Loc,
        functions: bool,
    ) -> PExpr {
        let args = self.parse_template_arg_list().map(|(a, _, _)| a).unwrap_or_default();
        if self.at(Punct::ColonColon) {
            // `undeclared<int>::x`: a nested-name-specifier naming no
            // template (clang's `ActOnCXXNestedNameSpecifier` on a
            // `TNK_Undeclared_template`).
            let text = self.s.ast.symbols().as_str(name).to_owned();
            let d = self.s.diag(DiagId::sema_no_template_named, nl).arg(text);
            self.emit(d);
            return None;
        }
        if self.s.clang_compat < 22
            && !functions
            && let Some(kw) = self.s.template_id_keyword_typo(name, nl)
        {
            let len = u32::try_from(self.s.ast.symbols().as_str(name).len()).unwrap_or(0);
            return self.template_id_keyword(kw, Span::new(nl, nl.with_offset(len)));
        }
        if self.s.clang_compat < 22 {
            // clang 19: a delayed typo on an expression parsing goes on
            // around ("expected ')'" / "expected ';'" may follow).
            return Some(self.s.act_on_undeclared_template_id_typo(name, nl));
        }
        match self.s.correct_template_id_typo(name, &args, nl) {
            kiln_sema::TemplateIdTypo::Recovered(e) => return Some(e),
            kiln_sema::TemplateIdTypo::Suggested => {}
            kiln_sema::TemplateIdTypo::Uncorrected if functions => {
                let text = self.s.ast.symbols().as_str(name).to_owned();
                let d = self.s.diag(DiagId::sema_undeclared_var_use, nl).arg(text);
                self.emit(d);
            }
            kiln_sema::TemplateIdTypo::Uncorrected => {
                let _ = self.s.act_on_id_expression(name, nl, None);
            }
        }
        let goes_on = self.error_operand_goes_on();
        // The template-id is clang's `ExprError`: an expression-list
        // skips past it (`a[u<int>], x` skips to the `,`; WP-399).
        self.expr_invalid |= goes_on;
        goes_on.then(|| self.s.act_on_error_expr(&[], nl))
    }

    /// Whether the enclosing expression goes on parsing around an error
    /// operand (clang's `ExprError`) that ends here: before a binary
    /// operator, a postfix suffix (`[`, `.`, `->`, `++`, `--`) or a
    /// closing token.
    fn error_operand_goes_on(&self) -> bool {
        self.cur_binop().is_some()
            || [
                Punct::LSquare,
                Punct::Period,
                Punct::Arrow,
                Punct::PlusPlus,
                Punct::MinusMinus,
                Punct::RParen,
                Punct::RSquare,
                Punct::RBrace,
                Punct::Comma,
                Punct::Semi,
                Punct::Colon,
            ]
            .iter()
            .any(|&p| self.at(p))
    }

    /// clang 19's re-parse of the template-id at `nl` corrected to the
    /// keyword `kw` ([`kiln_sema::Sema::template_id_keyword_typo`]): the
    /// keyword stands where the template-id was and is no
    /// unqualified-id. `template` is consumed ("'template' keyword not
    /// permitted here") and the enclosing expression goes on around an
    /// error operand before an operator; any other keyword is left for
    /// the enclosing construct to skip.
    fn template_id_keyword(&mut self, kw: &str, name: Span) -> PExpr {
        let nl = name.lo;
        if kw == "template" {
            // The fix-it removes it (clang's `FixItHint::CreateRemoval`).
            let d = self
                .s
                .diag(DiagId::parse_unexpected_template_in_unqualified_id, nl)
                .fixit(kiln_diag::FixIt::remove(name));
            self.emit(d);
            self.error_here(DiagId::parse_expected_unqualified_id);
            let goes_on = self.cur_binop().is_some();
            self.expr_invalid |= goes_on;
            return goes_on.then(|| self.s.act_on_error_expr(&[], nl));
        }
        let d = self.s.diag(DiagId::parse_expected_unqualified_id, nl);
        self.emit(d);
        let sym = self.s.ast.symbols().intern(kw);
        self.inject(TokKind::Identifier(sym), nl);
        None
    }

    /// A (possibly qualified) id-expression that does not name a type
    /// ([expr.prim.id]): a template-id names a variable template, a
    /// function template or a concept ([temp.names]/1); a name after a
    /// dependent qualifier is a `DependentScopeDeclRefExpr`
    /// ([temp.dep.expr]/3); an undeclared name before `(` in a template
    /// is found by argument-dependent lookup at instantiation
    /// ([temp.dep.general]/2).
    #[allow(clippy::too_many_lines, reason = "the id-expression forms in one place")]
    fn parse_id_expression_rest(&mut self) -> PExpr {
        let start = self.pos();
        let begin = self.name_begin();
        let qual = self.parse_nested_name_nns().ok()?;
        let template_loc = if qual.is_some() { self.eat_kw(Kw::Template) } else { None };
        let template_kw = template_loc.is_some();
        if let Some(q) = qual
            && self.at_kw(Kw::Operator)
        {
            return self.parse_qualified_operator_id(q, begin, template_loc);
        }
        let Some(name) = self.ident() else {
            self.error_here(DiagId::parse_expected_unqualified_id);
            return None;
        };
        let nl = self.bump().span.lo;
        if let Some(q) = qual
            && let Some(nns) = self.s.dep_member_nns(q, name, template_kw && self.at(Punct::Less))
        {
            // `T::template f` with no template argument list
            // ([temp.names]/6; clang's `TNK_Dependent_template_name`).
            if template_kw && !self.at(Punct::Less) {
                self.s.diag_missing_template_arg_list(nl);
            }
            let targs = if template_kw && self.at(Punct::Less) {
                let (args, _, _) = self.parse_template_arg_list()?;
                Some(self.s.ast.types.arg_list(&args))
            } else {
                None
            };
            let e = self.s.act_dependent_scope_ref(nns, DeclName::ident(name), targs, begin, nl);
            self.s.set_template_kw(e, template_loc);
            return Some(e);
        }
        let class = self.s.classify_name(name, qual.map(|q| q.ctx));
        // `N::template f` naming a function or variable template must be
        // followed by a template argument list ([temp.names]/6).
        if template_kw
            && !self.at(Punct::Less)
            && matches!(
                class,
                NameClass::FunctionTemplate(..) | NameClass::VarTemplate(..) | NameClass::Dependent
            )
        {
            self.s.diag_missing_template_arg_list(nl);
        }
        if self.at(Punct::Less) && (template_kw || is_template_class(class)) {
            return Some(self.template_id_expr(qual, (class, name, nl), begin, template_loc));
        }
        // [temp.names]/3 (P0846): an unqualified name that lookup finds
        // nothing or only functions for, followed by `<`, is a
        // template-name; ADL finds the template at the call
        // (`get<0>(c)` for `std::complex`). Only taken when a
        // template-argument-list closed by `>(` follows. Before C++20
        // only a name lookup finds nothing for (clang's
        // `TNK_Undeclared_template`).
        if qual.is_none()
            && self.at(Punct::Less)
            && (class == NameClass::Undeclared
                || (self.std >= Std::Cxx20 && matches!(class, NameClass::Value(_))))
            && let Some((close, false)) = self.skip_template_args(self.pos())
            && self.tok_at(close + 1).kind == TokKind::Punct(Punct::LParen)
            && self.s.is_assumed_template_name(name)
        {
            let (args, _, _) = self.parse_template_arg_list()?;
            return Some(self.s.act_on_assumed_template_id(name, &args, nl));
        }
        // A name lookup found a single non-template for, followed by what
        // can only be a template argument list: `<>`, or `<` type-id ...
        // `>` (clang's `checkPotentialAngleBracket`; before C++20 also
        // for functions, which [temp.names]/3 does not yet assume to be
        // templates). The list is skipped and the name is an error.
        if self.at(Punct::Less)
            && !template_kw
            && let NameClass::Value(_) = class
            && (qual.is_some()
                || !(self.std >= Std::Cxx20 && self.s.is_assumed_template_name(name)))
            && let Some(found) =
                self.s.potential_template_name_in(qual.map(|q| q.ctx), DeclName::ident(name))
            && let Some(close) = self.potential_angle_close()
        {
            let angles = self.skip_potential_angles(close);
            self.s.diagnose_expr_intended_as_template_name(
                DeclName::ident(name),
                found,
                nl,
                angles,
            );
            // clang's `checkPotentialAngleBracket` makes the name
            // `ExprError()`: the enclosing expression is invalid.
            self.expr_invalid = true;
            return Some(self.s.act_on_error_expr(&[], nl));
        }
        if qual.is_none()
            && self.at(Punct::Less)
            && class == NameClass::Undeclared
            && self.closes_assumed_template_args()
            && self.s.is_assumed_template_name(name)
        {
            return self.undeclared_template_id_expr(name, nl, false);
        }
        // The same for a name lookup found only functions for (C++20):
        // clang 22 typo-corrects it to the qualified function name.
        if qual.is_none()
            && self.at(Punct::Less)
            && self.std >= Std::Cxx20
            && matches!(class, NameClass::Value(_))
            && self.closes_assumed_template_args()
            && self.s.is_assumed_template_name(name)
        {
            return self.undeclared_template_id_expr(name, nl, true);
        }
        if qual.is_none()
            && class == NameClass::Undeclared
            && self.at(Punct::LParen)
            && !self.s.is_builtin_name(name)
        {
            return Some(self.s.act_on_undeclared_call_name(name, nl));
        }
        if qual.is_none() && self.at(Punct::LParen) {
            return Some(self.s.act_on_call_name(name, nl));
        }
        let e = self.s.act_on_id_expression_in(name, nl, qual, self.whole_addr_operand(start));
        if self.s.clang_compat >= 22
            && qual.is_none()
            && class == NameClass::Undeclared
            && self.s.is_expr_error(e)
        {
            // clang 22's undeclared identifier is `ExprError` (no delayed
            // typo): the enclosing expression goes on around it before an
            // operator, a postfix suffix or a closing token, and anything
            // else (`f(u 8, x)`, `a[u 8]`) is skipped by the enclosing
            // construct, as for an undeclared template-id (WP-420).
            let goes_on = self.error_operand_goes_on();
            self.expr_invalid |= goes_on;
            return goes_on.then_some(e);
        }
        self.parse_pack_index_suffix(e, nl)
    }

    /// The index of the `>` closing what follows the current `<` as a
    /// template argument list intended for a non-template name (clang's
    /// `checkPotentialAngleBracket`): `<>` (or `<>>` from C++11), or `<`
    /// followed by an unambiguous type-id and a later `>` -- brackets
    /// balanced, not past a `;` or an unmatched closer.
    /// Skips the potential template argument list at its `<` through its
    /// `>` at token index `close` ([`Self::potential_angle_close`]),
    /// splitting a `>>`; the locations of the `<` and the `>`.
    fn skip_potential_angles(&mut self, close: usize) -> (Loc, Loc) {
        let less = self.loc();
        while self.pos() < close {
            let _ = self.bump();
        }
        (less, self.split_greater())
    }

    fn potential_angle_close(&self) -> Option<usize> {
        let less = self.pos();
        let next = self.tok_at(less + 1).kind;
        if next == TokKind::Punct(Punct::Greater)
            || (self.std >= Std::Cxx11 && next == TokKind::Punct(Punct::GreaterGreater))
        {
            return Some(less + 1);
        }
        if !self.is_cxx_type_id_at(less + 1, TypeIdContext::Unambiguous).0 {
            return None;
        }
        let mut j = less + 1;
        loop {
            match self.tok_at(j).kind {
                // The end of the statement, or an unmatched closer.
                TokKind::Eof
                | TokKind::Punct(Punct::Semi | Punct::RParen | Punct::RSquare | Punct::RBrace) => {
                    return None;
                }
                TokKind::Punct(Punct::Greater | Punct::GreaterGreater) => return Some(j),
                TokKind::Punct(Punct::LParen | Punct::LSquare | Punct::LBrace) => {
                    j = self.skip_balanced(j)?;
                }
                _ => j += 1,
            }
        }
    }

    /// `N::operator@` / `X<T>::operator T` after the nested-name-specifier
    /// `q` ([expr.prim.id.qual]/1: a qualified-id's unqualified-id may be
    /// an operator- or conversion-function-id): `B<float>::operator()(v)`
    /// in libc++ <__functional/hash.h>.
    fn parse_qualified_operator_id(
        &mut self,
        q: kiln_sema::using::Qualifier,
        begin: Loc,
        template_kw: Option<Loc>,
    ) -> PExpr {
        let nl = self.loc();
        let name = self.parse_operator_function_name()?;
        if self.at_operator_template_args(name, Some(q), template_kw) {
            return self.parse_operator_template_id(name, nl, Some((q, begin)), template_kw);
        }
        if self.s.is_dependent_ctx(q.ctx) {
            return Some(self.s.act_dependent_scope_ref(q.nns, name, None, begin, nl));
        }
        // `::operator+<int>(a, b)` with `operator+` no template: as for a
        // qualified identifier, clang's `checkPotentialAngleBracket`.
        if self.at(Punct::Less)
            && template_kw.is_none()
            && let Some(found) = self.s.potential_template_name_in(Some(q.ctx), name)
            && let Some(close) = self.potential_angle_close()
        {
            let angles = self.skip_potential_angles(close);
            self.s.diagnose_expr_intended_as_template_name(name, found, nl, angles);
            self.expr_invalid = true;
            return Some(self.s.act_on_error_expr(&[], nl));
        }
        Some(self.s.act_on_id_expression_dn(name, nl, Some(q)))
    }

    /// Whether the `<` after the operator-function-id `name` (qualified
    /// by `q`, after `template` at `template_kw`) starts its template
    /// argument list ([temp.names]/3): after `template`, or when the name
    /// is a template-name (otherwise `<` is the less-than operator).
    fn at_operator_template_args(
        &mut self,
        name: DeclName,
        q: Option<kiln_sema::using::Qualifier>,
        template_kw: Option<Loc>,
    ) -> bool {
        if !self.at(Punct::Less) {
            return false;
        }
        if template_kw.is_some() {
            return true;
        }
        if q.is_some_and(|q| self.s.is_dependent_ctx(q.ctx)) {
            return false;
        }
        self.s.operator_names_template(name, q.map(|q| q.ctx))
    }

    /// An operator-function-id template-id at its `<` ([temp.names]/1):
    /// `operator()<T>`, `Q::operator()<T>`, `Q<T>::template
    /// operator()<U>`; `name` was named at `nl`, after the qualifier `q`
    /// (which begins at its `Loc`) and the `template` keyword at
    /// `template_kw`, if any. As for an identifier template-id, a
    /// dependent qualifier makes a `DependentScopeDeclRefExpr`, and one
    /// not called is resolved to its single specialization.
    fn parse_operator_template_id(
        &mut self,
        name: DeclName,
        nl: Loc,
        q: Option<(kiln_sema::using::Qualifier, Loc)>,
        template_kw: Option<Loc>,
    ) -> PExpr {
        let (args, _, _) = self.parse_template_arg_list()?;
        if let Some((q, begin)) = q
            && self.s.is_dependent_ctx(q.ctx)
        {
            let targs = self.s.ast.types.arg_list(&args);
            let e = self.s.act_dependent_scope_ref(q.nns, name, Some(targs), begin, nl);
            self.s.set_template_kw(e, template_kw);
            return Some(e);
        }
        let call = self.at(Punct::LParen);
        let e = self.s.act_on_operator_template_id(name, &args, nl, q.map(|(q, _)| q.ctx), call);
        if let Some((q, begin)) = q {
            self.s.set_template_id_begin(e, begin, nl);
            self.s.set_template_id_qualifier(e, q.nns);
            self.s.set_template_kw(e, template_kw);
        }
        // `(operator()<A>)(x)` is called as `operator()<A>(x)` is
        // ([`Parser::template_id_is_callee`]).
        if !self.template_id_is_callee(q.map_or(nl, |(_, begin)| begin)) {
            return Some(self.s.resolve_single_template_id(e));
        }
        Some(e)
    }

    /// `pack...[index]` after an id-expression (C++26
    /// [expr.prim.pack.index]) whose name begins at `begin`; `e`
    /// unchanged without `...[`.
    fn parse_pack_index_suffix(&mut self, e: ExprId, begin: Loc) -> PExpr {
        let Some((ellipsis, index, rsquare)) = self.parse_pack_index(begin).ok()? else {
            return Some(e);
        };
        Some(self.s.act_pack_indexing(e, ellipsis, index, rsquare))
    }

    /// `...[ constant-expression ]` (shared by the expression and type
    /// forms) after a pack name beginning at `begin`: `Err` on a parse
    /// error, `Ok(None)` without `...[`.
    pub(crate) fn parse_pack_index(
        &mut self,
        begin: Loc,
    ) -> Result<Option<(Loc, ExprId, Loc)>, ()> {
        if !(self.at(Punct::Ellipsis) && self.peek_is(1, Punct::LSquare)) {
            return Ok(None);
        }
        let ellipsis = self.bump().span.lo;
        if self.std < Std::Cxx26 {
            // P2662: clang `ext_pack_indexing`, at the pack name.
            let w = self.s.diag(DiagId::parse_ext_pack_indexing, begin);
            self.emit(w);
        }
        let lsquare = self.bump().span.lo;
        let index = self.with_greater_op(true, Self::parse_constant_expr).ok_or(())?;
        let rsquare = self.expect_closing(Punct::RSquare, Punct::LSquare, lsquare);
        Ok(Some((ellipsis, index, rsquare)))
    }

    /// The rest of a parenthesized expression or fold-expression after
    /// `(` at `lparen` ([expr.prim.paren], [expr.prim.fold]).
    fn parse_paren_rest(&mut self, lparen: Loc) -> PExpr {
        // `(e)(args)`, `((e))(args)`: the parenthesized expression is a
        // callee ([`Parser::template_id_is_callee`]).
        let start = self.pos();
        let callee = start.checked_sub(1).and_then(|open| {
            let after = self.skip_balanced(open)?;
            let called = self.tok_at(after).kind == TokKind::Punct(Punct::LParen)
                || self.paren_callee == Some((open, after));
            called.then_some((start, after - 1))
        });
        let outer = std::mem::replace(&mut self.paren_callee, callee);
        let r = self.parse_paren_rest_inner(lparen);
        self.paren_callee = outer;
        r
    }

    /// True when the function template-id beginning at `begin`, whose
    /// template argument list has just been parsed, is called: followed
    /// by `(`, or the whole of a parenthesized callee, `(f<A>)(x)`. A
    /// called template-id stays an overload set: the call deduces the
    /// remaining template arguments ([temp.arg.explicit]/4,
    /// [over.call.func]); one not called names its single
    /// specialization ([over.over]/3).
    pub(crate) fn template_id_is_callee(&self, begin: Loc) -> bool {
        self.at(Punct::LParen)
            || self.paren_callee.is_some_and(|(first, close)| {
                close == self.pos() && self.tok_at(first).span.lo == begin
            })
    }

    fn parse_paren_rest_inner(&mut self, lparen: Loc) -> PExpr {
        // `( ... op e )`.
        if self.at(Punct::Ellipsis)
            && let Some(op) =
                Self::punct_of(self.tok_at(self.pos() + 1)).and_then(binop).and_then(|b| b.1)
        {
            let ellipsis = self.bump().span.lo;
            self.bump();
            let Some(rhs) = self.parse_fold_operand(true, None) else {
                self.skip_until(&[Punct::RParen], false, true);
                return None;
            };
            let r = self.expect_closing(Punct::RParen, Punct::LParen, lparen);
            return Some(self.s.act_fold(None, op, Some(rhs), lparen, ellipsis, r));
        }
        // clang 19 classifies a leading identifier not followed by `(`,
        // `<` or `::` (`ClassifyName`): an undeclared one is reported
        // there and the parenthesized expression is `ExprError`, skipped
        // to its `)` with no "expected ')'" (`(u 8)`; `(u) + x` goes on
        // around it, WP-420).
        let undeclared_first = self.s.clang_compat < 22
            && matches!(self.tok().kind, TokKind::Identifier(sym)
                if self.s.classify_name(sym, None) == NameClass::Undeclared)
            && !matches!(
                self.tok_at(self.pos() + 1).kind,
                TokKind::Punct(Punct::LParen | Punct::Less | Punct::ColonColon)
            );
        let list = self.cast_paren_list == Some(lparen);
        if list {
            self.cast_paren_list = None;
        }
        let outer_invalid = std::mem::replace(&mut self.expr_invalid, false);
        // A throw- or yield-expression is an assignment-expression, not
        // a fold operand: `(co_yield i) + i` ([expr.yield]/1).
        let e = if self.at_kw(Kw::Throw) || self.at_kw(Kw::CoYield) {
            self.parse_expression()
        } else {
            self.parse_fold_operand(false, list.then_some(lparen))
        };
        let invalid = std::mem::replace(&mut self.expr_invalid, outer_invalid);
        // clang's `ParseParenExpression`: an `ExprError` inside skips to
        // the `)` with no "expected ')'" and the parenthesized expression
        // is `ExprError`, around which the enclosing expression goes on
        // (`(1 + ) + x`, `((u) 8)`; WP-420).
        let Some(e) = e else {
            self.skip_until(&[Punct::RParen], false, true);
            self.expr_invalid = true;
            return Some(self.s.act_on_error_expr(&[], lparen));
        };
        if invalid || (undeclared_first && self.s.correct_delayed_typos_in(e)) {
            self.skip_until(&[Punct::RParen], true, true);
            self.expr_invalid = true;
            return Some(self.s.act_on_error_expr(&[e], lparen));
        }
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
            let Some(r) = self.parse_fold_operand(true, None) else {
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
    /// follows, [expr.prim.fold]/1, clang's `CheckFoldOperand`). Its
    /// top-level commas are at `comma_at` when given (a C-style cast's
    /// parenthesized operand, [`Parser::cast_paren_list`]).
    fn parse_fold_operand(&mut self, in_fold: bool, comma_at: Option<Loc>) -> PExpr {
        let start = self.loc();
        let cast = self.parse_cast_expr()?;
        self.comma_loc = comma_at;
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
        self.named_cast_type_id = true;
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
        let loc = self.name_begin();
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
        // An expression-list of initializer-clauses ([expr.type.conv]/1):
        // a braced-init-list is a placeholder argument that the
        // construction list-initializes its parameter from.
        let args = self.parse_expr_list(Punct::RParen)?;
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
        if let [e] = args[..]
            && self.s.ast.exprs.get(e).kind() != ExprKind::PackExpansionExpr
            && !self.s.is_braced_arg(e)
        {
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
                if let Some(u) = l.ud_suffix {
                    // [lex.ext]/3: the raw form is the spelling without
                    // the ud-suffix.
                    let raw = &sp[..sp.len() - u.len()];
                    return self.s.act_on_integer_udl(l, raw, t.span.lo, ud);
                }
                self.s.act_on_integer_literal(l, t.span.lo, ud)
            }
            NumberClass::Floating(l) => {
                let ud = suffix_loc(t.span.hi, l.ud_suffix);
                if let Some(u) = l.ud_suffix {
                    let raw = &sp[..sp.len() - u.len()];
                    return self.s.act_on_float_udl(l, raw, t.span.lo, ud);
                }
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
        // [lex.string.concat]/2: every ud-suffix of the concatenation is
        // the first one (clang `err_string_concat_mixed_suffix`, at each
        // differing token).
        let mut first_suffix: Option<&str> = None;
        for (p, loc) in &pieces {
            let Some(sfx) = p.ud_suffix else { continue };
            match first_suffix {
                None => first_suffix = Some(sfx),
                Some(f) if f != sfx => {
                    let m = self
                        .s
                        .diag(DiagId::lex_string_concat_mixed_suffix, *loc)
                        .arg(f.to_owned())
                        .arg(sfx.to_owned());
                    self.s.emit(m);
                    error = true;
                }
                Some(_) => {}
            }
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
