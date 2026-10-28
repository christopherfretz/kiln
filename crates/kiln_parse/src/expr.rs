//! Expressions ([expr]; spec/parse.md §5).
//!
//! Binary operators use operator-precedence parsing with clang's
//! precedence levels; everything below a cast-expression is recursive
//! descent. A `None` result ([`PExpr`]) means a syntax error was reported
//! and the caller must recover; semantic errors yield an error expression.

use kiln_ast::QualType;
use kiln_ast::expr::{BinaryOp, TraitKind, UnaryOp};
use kiln_diag::DiagId;
use kiln_lex::literal::{
    CodeUnits, NumberClass, StringLiteral, classify_number, decode_char, decode_string,
    decode_unevaluated_string,
};
use kiln_lex::{Punct, TokKind, Token};
use kiln_sema::expr::CastStyle;
use kiln_sema::{DeclaratorContext, NameClass};
use kiln_source::Loc;

use crate::decl::is_type_specifier_kw;
use crate::keywords::Kw;
use crate::parser::{PExpr, Parser};

/// clang's `prec::Level`.
mod prec {
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
        let lhs = self.parse_cast_expr()?;
        self.parse_rhs(lhs, prec::COMMA)
    }

    /// assignment-expression ([expr.assign]).
    pub fn parse_assignment_expr(&mut self) -> PExpr {
        let lhs = self.parse_cast_expr()?;
        self.parse_rhs(lhs, prec::ASSIGN)
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

    fn cur_binop(&self) -> Option<(u8, Option<BinaryOp>)> {
        Self::punct_of(self.tok()).and_then(binop)
    }

    /// clang's `ParseRHSOfBinaryExpression`.
    fn parse_rhs(&mut self, mut lhs: kiln_ast::ExprId, min: u8) -> PExpr {
        loop {
            let Some((p, op)) = self.cur_binop() else { return Some(lhs) };
            if p < min {
                return Some(lhs);
            }
            let op_tok = self.bump();
            let op_loc = op_tok.span.lo;
            let mut middle = None;
            if op.is_none() {
                if self.at(Punct::Colon) {
                    self.unsupported("the GNU '?:' operator", self.loc());
                    return None;
                }
                middle = Some(self.parse_expression()?);
                if self.eat(Punct::Colon).is_none() {
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
            let mut rhs = if p <= prec::COND {
                if self.at(Punct::LBrace) {
                    self.unsupported("braced-init-list operands", self.loc());
                    return None;
                }
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

    // -- unary and cast expressions ----------------------------------------------------

    /// cast-expression ([expr.cast]) and unary-expression ([expr.unary]).
    pub(crate) fn parse_cast_expr(&mut self) -> PExpr {
        let loc = self.loc();
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
            if p == Punct::LParen && self.is_type_id_start(self.pos() + 1) {
                return self.parse_c_style_cast();
            }
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
            Some(k @ (Kw::New | Kw::Delete | Kw::CoAwait | Kw::Noexcept)) => {
                self.unsupported(&format!("'{}' expressions", k.spelling()), loc);
                return None;
            }
            _ => {}
        }
        self.parse_postfix_expr()
    }

    /// `( type-id ) cast-expression`.
    fn parse_c_style_cast(&mut self) -> PExpr {
        let open = self.bump().span.lo;
        let ty = self.parse_type_id();
        let Some(ty) = ty else {
            self.skip_until(&[Punct::RParen], false, true);
            return None;
        };
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        if self.at(Punct::LBrace) {
            self.unsupported("compound literals", open);
            return None;
        }
        let e = self.parse_cast_expr()?;
        Some(self.s.act_on_cast(CastStyle::CStyle, ty, e, open))
    }

    /// `sizeof...(name)` ([expr.sizeof]/5). M2 has no packs, so Sema
    /// rejects every name; the unparenthesized form is TODO(M3).
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
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        Some(self.s.act_on_sizeof_pack(name, name_loc, loc))
    }

    /// `sizeof`/`alignof`/`__alignof` ([expr.sizeof], [expr.alignof]).
    fn parse_sizeof_alignof(&mut self, kind: TraitKind) -> PExpr {
        let loc = self.bump().span.lo;
        if kind == TraitKind::SizeOf && self.at(Punct::Ellipsis) {
            return self.parse_sizeof_pack(loc);
        }
        if self.at(Punct::LParen) && self.is_type_id_start(self.pos() + 1) {
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

    // -- postfix expressions --------------------------------------------------------------

    /// postfix-expression ([expr.post]).
    fn parse_postfix_expr(&mut self) -> PExpr {
        let mut e = self.parse_primary_expr()?;
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
                    let Some(idx) = self.parse_expression() else {
                        self.skip_until(&[Punct::RSquare], false, true);
                        return None;
                    };
                    self.expect_closing(Punct::RSquare, Punct::LSquare, loc);
                    e = self.s.act_on_subscript(e, idx, loc);
                }
                Some(Punct::LParen) => {
                    self.bump();
                    let mut args = Vec::new();
                    if !self.at(Punct::RParen) {
                        loop {
                            if self.at(Punct::LBrace) {
                                self.unsupported("braced-init-list arguments", self.loc());
                                self.skip_until(&[Punct::RParen], false, true);
                                return None;
                            }
                            let Some(a) = self.parse_assignment_expr() else {
                                self.skip_until(&[Punct::RParen], false, true);
                                return None;
                            };
                            args.push(a);
                            if self.eat(Punct::Comma).is_none() {
                                break;
                            }
                        }
                    }
                    let rparen = self.expect_closing(Punct::RParen, Punct::LParen, loc);
                    e = self.s.act_on_call(e, &args, loc, rparen);
                }
                Some(p @ (Punct::Period | Punct::Arrow)) => {
                    self.bump();
                    if self.at_kw(Kw::Template) || self.at(Punct::Tilde) {
                        self.unsupported("this member access", self.loc());
                        return None;
                    }
                    let Some(name) = self.ident() else {
                        self.error_here(DiagId::parse_expected_unqualified_id);
                        return None;
                    };
                    let nl = self.bump().span.lo;
                    e = self.s.act_on_member(e, name, nl, loc, p == Punct::Arrow);
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

    // -- primary expressions ----------------------------------------------------------------

    /// primary-expression ([expr.prim]), named casts and functional casts.
    fn parse_primary_expr(&mut self) -> PExpr {
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
                    self.unsupported("statement expressions", loc);
                    self.skip_until(&[Punct::RParen], false, true);
                    return None;
                }
                let Some(e) = self.parse_expression() else {
                    self.skip_until(&[Punct::RParen], false, true);
                    return None;
                };
                let r = self.expect_closing(Punct::RParen, Punct::LParen, loc);
                return Some(self.s.act_on_paren(e, loc, r));
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
                Kw::This
                | Kw::Throw
                | Kw::Typeid
                | Kw::Requires
                | Kw::CoYield
                | Kw::Typename
                | Kw::Template => {
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
            if let Some((_, NameClass::Type(..), _)) = self.scan_name(self.pos()) {
                return self.parse_functional_cast();
            }
            let qual = self.parse_nested_name().ok()?;
            let Some(name) = self.ident() else {
                self.error_here(DiagId::parse_expected_unqualified_id);
                return None;
            };
            let nl = self.bump().span.lo;
            return Some(self.s.act_on_id_expression(name, nl, qual));
        }
        self.error_here(DiagId::parse_expected_expression);
        None
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
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        Some(self.s.act_on_cast(style, ty, e, loc))
    }

    /// `T(e)` ([expr.type.conv]) with one operand.
    fn parse_functional_cast(&mut self) -> PExpr {
        let loc = self.loc();
        let ds = self.parse_decl_specifiers(DeclaratorContext::TypeName);
        let ty: QualType = self.s.type_from_declspec(&ds);
        if self.at(Punct::LBrace) {
            self.unsupported("list-initialization of a temporary", self.loc());
            return None;
        }
        let Some(open) = self.eat(Punct::LParen) else {
            let d = self.s.diag(DiagId::parse_expected, self.expected_loc()).arg("'('");
            self.emit(d);
            return None;
        };
        if self.at(Punct::RParen) {
            self.unsupported("value-initialization of a temporary", loc);
            self.bump();
            return None;
        }
        let Some(e) = self.parse_assignment_expr() else {
            self.skip_until(&[Punct::RParen], false, true);
            return None;
        };
        if self.at(Punct::Comma) {
            self.unsupported("functional casts with several operands", self.loc());
            self.skip_until(&[Punct::RParen], false, true);
            return None;
        }
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        Some(self.s.act_on_cast(CastStyle::Functional, ty, e, loc))
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

    fn parse_number(&mut self) -> kiln_ast::ExprId {
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

    fn parse_char(&mut self) -> kiln_ast::ExprId {
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
