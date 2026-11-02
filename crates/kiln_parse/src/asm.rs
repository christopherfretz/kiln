//! GNU asm statements and file-scope asm declarations ([dcl.asm]; clang's
//! `ParseAsmStatement`, `ParseAsmOperandsOpt`, `ParseSimpleAsm`).

use kiln_ast::expr::ExprKind;
use kiln_ast::stmt::{AsmOperand, asm_bits};
use kiln_ast::strings::StringEncoding;
use kiln_ast::{DeclId, StmtId, StringLitId};
use kiln_diag::DiagId;
use kiln_lex::{Punct, Std};
use kiln_source::{Loc, Span};

use crate::keywords::Kw;
use crate::parser::Parser;

/// A parsed asm string literal: the pooled string and its first token.
type AsmStr = (StringLitId, Loc);

impl Parser<'_, '_> {
    /// An asm string literal (clang's `ParseAsmStringLiteral(false)`):
    /// adjacent literals concatenate; it must be an ordinary literal.
    /// `None` (reported) when missing or malformed.
    fn parse_asm_string(&mut self) -> Option<AsmStr> {
        self.parse_asm_string_expr().map(|(s, _)| s)
    }

    /// [`Self::parse_asm_string`], also returning the literal expression.
    fn parse_asm_string_expr(&mut self) -> Option<(AsmStr, kiln_ast::ExprId)> {
        if !self.at_string() {
            // clang 22 accepts a parenthesized constant expression in an
            // asm statement since C++11 and says so (kiln does not parse
            // one yet); asm labels and C++98 keep the plain wording.
            let d = if self.std >= Std::Cxx11 {
                self.s.diag(DiagId::parse_asm_expected_string, self.loc())
            } else {
                self.s.diag(DiagId::parse_expected_string_literal_in, self.loc()).arg("'asm'")
            };
            self.emit(d);
            return None;
        }
        let start = self.loc();
        let e = self.parse_string_literal(false)?;
        let ast = &self.s.ast;
        if ast.exprs.get(e).kind() != ExprKind::StringLiteral {
            return None;
        }
        let lit = ast.exprs.string_lit(e);
        let which = match ast.strings.encoding(lit) {
            StringEncoding::Ordinary => return Some(((lit, start), e)),
            StringEncoding::Wide16 | StringEncoding::Wide32 => "1",
            _ => "0",
        };
        let span = Span::new(start, self.prev_end());
        // clang reports at the token after the literal.
        let d = self.s.diag(DiagId::parse_asm_string_kind, self.loc()).arg(which).range(span);
        self.emit(d);
        None
    }

    /// `asm-qualifier-list asm ( ... ) ;` at the `asm` keyword
    /// (GNU extended asm; clang's `ParseAsmStatement`). The `;` is
    /// consumed.
    pub(crate) fn parse_asm_statement(&mut self) -> Option<StmtId> {
        let loc = self.bump().span.lo;
        let mut bits = 0u16;
        loop {
            let (bit, name) = match self.kw() {
                Some(Kw::Volatile) => (asm_bits::VOLATILE, "volatile"),
                Some(Kw::Inline) => (asm_bits::INLINE, "inline"),
                Some(Kw::Goto) => (asm_bits::GOTO, "goto"),
                _ => break,
            };
            if bits & bit != 0 {
                let d = self.s.diag(DiagId::parse_asm_duplicate_qual, self.loc()).arg(name);
                self.emit(d);
            }
            bits |= bit;
            self.bump();
        }
        let Some(lparen) = self.eat(Punct::LParen) else {
            self.error_here(DiagId::parse_asm_qualifier_ignored);
            self.skip_until(&[Punct::RParen], true, true);
            self.eat(Punct::Semi);
            return None;
        };
        let Some(string) = self.parse_asm_string() else {
            self.skip_asm_rest();
            return None;
        };
        let mut outputs = Vec::new();
        let mut inputs = Vec::new();
        let mut clobbers = Vec::new();
        let mut labels = Vec::new();
        if self.at(Punct::RParen) {
            bits |= asm_bits::BASIC;
        } else {
            // In C++ `::` parses as `: :`.
            let mut extra = false;
            let section = |p: &mut Self, extra: &mut bool| -> bool {
                if *extra {
                    *extra = false;
                    return true;
                }
                if p.at(Punct::Colon) {
                    p.bump();
                    true
                } else if p.at(Punct::ColonColon) {
                    p.bump();
                    *extra = true;
                    true
                } else {
                    false
                }
            };
            if section(self, &mut extra)
                && !extra
                && self.parse_asm_operands(&mut outputs).is_none()
            {
                self.skip_asm_rest();
                return None;
            }
            if section(self, &mut extra) && !extra && self.parse_asm_operands(&mut inputs).is_none()
            {
                self.skip_asm_rest();
                return None;
            }
            if section(self, &mut extra) && !extra && self.at_string() {
                while let Some(c) = self.parse_asm_string() {
                    clobbers.push(c);
                    if self.eat(Punct::Comma).is_none() {
                        break;
                    }
                }
            }
            let goto = bits & asm_bits::GOTO != 0;
            if !goto && (!self.at(Punct::RParen) || extra) {
                return self.asm_expected("')'");
            }
            // `asm goto` labels (clang `ParseAsmStatement`: each is looked
            // up or created as a label of the function).
            if extra || self.at(Punct::Colon) {
                if !extra {
                    self.bump();
                }
                labels = self.parse_asm_goto_labels()?;
            } else if goto {
                return self.asm_expected("':'");
            }
        }
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, lparen);
        let s = self.s.act_on_gcc_asm_goto_stmt(
            loc, bits, string, &outputs, &inputs, &clobbers, &labels, rparen,
        );
        if self.eat(Punct::Semi).is_none() {
            let d =
                self.s.diag(DiagId::parse_expected_semi_after_stmt, self.expected_loc()).arg("asm");
            self.emit(d);
            self.skip_until(&[Punct::RBrace], true, false);
            self.eat(Punct::Semi);
        }
        s
    }

    /// The `asm goto` label list after its `:`: identifiers, each looked
    /// up or created as a label of the function (clang
    /// `ParseAsmStatement`).
    fn parse_asm_goto_labels(&mut self) -> Option<Vec<(DeclId, Loc)>> {
        let mut labels = Vec::new();
        loop {
            let Some(name) = self.ident() else {
                let d = self.s.diag(DiagId::parse_expected, self.loc()).arg("identifier");
                self.emit(d);
                self.skip_asm_rest();
                return None;
            };
            let at = self.bump().span.lo;
            labels.push((self.s.act_on_asm_goto_label(name, at), at));
            if self.eat(Punct::Comma).is_none() {
                return Some(labels);
            }
        }
    }

    /// Reports "expected {what}" and skips the rest of the statement.
    fn asm_expected<T>(&mut self, what: &str) -> Option<T> {
        let d = self.s.diag(DiagId::parse_expected, self.loc()).arg(what);
        self.emit(d);
        self.skip_asm_rest();
        None
    }

    /// Skips to (and over) the closing `)` and a following `;`.
    fn skip_asm_rest(&mut self) {
        if self.skip_until(&[Punct::RParen], true, false) {
            self.bump();
        }
        self.eat(Punct::Semi);
    }

    /// `asm-operands`: `[name] "constraint" (expr), ...`; `None`
    /// (reported) on error.
    fn parse_asm_operands(&mut self, out: &mut Vec<AsmOperand>) -> Option<()> {
        if !self.at_string() && !self.at(Punct::LSquare) {
            return Some(());
        }
        loop {
            let name = if let Some(open) = self.eat(Punct::LSquare) {
                let Some(id) = self.ident() else {
                    let d = self.s.diag(DiagId::parse_expected, self.loc()).arg("identifier");
                    self.emit(d);
                    return None;
                };
                self.bump();
                self.expect_closing(Punct::RSquare, Punct::LSquare, open);
                Some(id)
            } else {
                None
            };
            let (constraint, constraint_loc) = self.parse_asm_string()?;
            let Some(open) = self.eat(Punct::LParen) else {
                let d = self
                    .s
                    .diag(DiagId::parse_expected_after, self.loc())
                    .arg("asm operand")
                    .arg("'('");
                self.emit(d);
                return None;
            };
            let e = self.parse_expression();
            self.expect_closing(Punct::RParen, Punct::LParen, open);
            let expr = e?;
            out.push(AsmOperand { name, constraint, constraint_loc, expr });
            if self.eat(Punct::Comma).is_none() {
                return Some(());
            }
        }
    }

    /// A file-scope `asm ( string-literal ) ;` (clang's `ParseSimpleAsm`
    /// for a top-level asm block).
    pub(crate) fn parse_file_scope_asm(&mut self) {
        let loc = self.bump().span.lo;
        let Some(lparen) = self.eat(Punct::LParen) else {
            let d = self.s.diag(DiagId::parse_expected_after, self.loc()).arg("'asm'").arg("'('");
            self.emit(d);
            self.skip_until(&[Punct::Semi], false, true);
            return;
        };
        let Some((_, string)) = self.parse_asm_string_expr() else {
            self.skip_asm_rest();
            return;
        };
        self.expect_closing(Punct::RParen, Punct::LParen, lparen);
        self.s.act_on_file_scope_asm(string, loc);
        if self.eat(Punct::Semi).is_none() {
            let d = self
                .s
                .diag(DiagId::parse_expected_after, self.expected_loc())
                .arg("top-level asm block")
                .arg("';'");
            self.emit(d);
        }
    }
}
