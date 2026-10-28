//! Attributes ([dcl.attr], GNU `__attribute__`; spec/parse.md §6).
//!
//! The parser collects the attribute's scope, name and arguments and hands
//! them to [`kiln_sema::Sema::act_on_attribute`], which decides what is
//! known. Arguments are parsed as expressions only for the attributes that
//! take them ([`takes_expr_args`]); the arguments of other attributes are
//! skipped as balanced tokens.

use kiln_ast::AttrId;
use kiln_ast::attr::AttrSyntax;
use kiln_diag::DiagId;
use kiln_lex::{Punct, TokKind};
use kiln_sema::decl::AttrArg;
use kiln_source::symbol::Symbol;

use crate::keywords::Kw;
use crate::parser::Parser;

/// True for the attributes whose arguments are expressions.
fn takes_expr_args(name: &str) -> bool {
    let n = name.trim_start_matches("__").trim_end_matches("__");
    matches!(n, "deprecated" | "nodiscard" | "aligned" | "warn_unused_result")
}

impl Parser<'_, '_> {
    /// True at `[[`.
    pub(crate) fn at_cxx11_attr(&self) -> bool {
        self.at(Punct::LSquare) && self.peek_is(1, Punct::LSquare)
    }

    /// True at any attribute-specifier.
    pub(crate) fn at_attr(&self) -> bool {
        self.at_cxx11_attr() || matches!(self.kw(), Some(Kw::Alignas | Kw::Attribute))
    }

    /// Parses a sequence of attribute-specifiers (`[[...]]`, `alignas(...)`,
    /// `__attribute__((...))`) into `out`.
    pub(crate) fn parse_attributes(&mut self, out: &mut Vec<AttrId>) {
        loop {
            if self.at_cxx11_attr() {
                self.parse_cxx11_attributes(out);
            } else if self.at_kw(Kw::Alignas) {
                self.parse_alignas(out);
            } else if self.at_kw(Kw::Attribute) {
                self.parse_gnu_attributes(out);
            } else if self.at_kw(Kw::Declspec) {
                let loc = self.bump().span.lo;
                self.unsupported("'__declspec'", loc);
                if let Some(l) = self.eat(Punct::LParen) {
                    self.skip_until(&[Punct::RParen], false, true);
                    let _ = l;
                }
            } else {
                return;
            }
        }
    }

    /// Parses and discards attributes (where kiln does not yet attach them).
    pub(crate) fn skip_attributes(&mut self) {
        let mut v = Vec::new();
        self.parse_attributes(&mut v);
    }

    /// The name of an attribute: an identifier or keyword.
    fn attr_name(&self) -> Option<Symbol> {
        match self.tok().kind {
            TokKind::Identifier(s) => Some(s),
            _ => None,
        }
    }

    /// `[[ attribute-list ]]` ([dcl.attr.grammar]).
    fn parse_cxx11_attributes(&mut self, out: &mut Vec<AttrId>) {
        let open = self.bump().span.lo;
        self.bump();
        let mut default_scope = None;
        if self.at_kw(Kw::Using) {
            self.bump();
            default_scope = self.attr_name();
            if default_scope.is_some() {
                self.bump();
            } else {
                self.error_here(DiagId::parse_expected_unqualified_id);
            }
            self.expect(Punct::Colon);
        }
        loop {
            if self.at(Punct::RSquare) || self.at_eof() {
                break;
            }
            if self.eat(Punct::Comma).is_some() {
                continue;
            }
            let Some(mut name) = self.attr_name() else {
                self.error_here(DiagId::parse_expected_unqualified_id);
                self.skip_until(&[Punct::RSquare], false, false);
                break;
            };
            let loc = self.bump().span.lo;
            let mut scope = default_scope;
            if self.at(Punct::ColonColon) {
                self.bump();
                match self.attr_name() {
                    Some(n) => {
                        scope = Some(name);
                        name = n;
                        self.bump();
                    }
                    None => {
                        self.error_here(DiagId::parse_expected_unqualified_id);
                        self.skip_until(&[Punct::RSquare], false, false);
                        break;
                    }
                }
            }
            let args = self.parse_attr_args(name);
            if let Some(l) = self.eat(Punct::Ellipsis) {
                self.unsupported("attribute pack expansions", l);
            }
            if let Some(args) = args
                && let Some(a) = self.s.act_on_attribute(AttrSyntax::Cxx11, scope, name, &args, loc)
            {
                out.push(a);
            }
            if !self.at(Punct::Comma) {
                break;
            }
        }
        if self.eat(Punct::RSquare).is_some() {
            self.expect(Punct::RSquare);
        } else {
            self.expect_closing(Punct::RSquare, Punct::LSquare, open);
            self.skip_until(&[Punct::RSquare], false, true);
            self.eat(Punct::RSquare);
        }
    }

    /// `__attribute__(( gnu-attribute-list ))`.
    fn parse_gnu_attributes(&mut self, out: &mut Vec<AttrId>) {
        self.bump();
        let Some(l1) = self.expect(Punct::LParen) else { return };
        let Some(l2) = self.expect(Punct::LParen) else {
            self.skip_until(&[Punct::RParen], false, true);
            return;
        };
        loop {
            if self.at(Punct::RParen) || self.at_eof() {
                break;
            }
            if self.eat(Punct::Comma).is_some() {
                continue;
            }
            let Some(name) = self.attr_name() else {
                self.error_here(DiagId::parse_expected_unqualified_id);
                self.skip_until(&[Punct::RParen], false, false);
                break;
            };
            let loc = self.bump().span.lo;
            if let Some(args) = self.parse_attr_args(name)
                && let Some(a) = self.s.act_on_attribute(AttrSyntax::Gnu, None, name, &args, loc)
            {
                out.push(a);
            }
            if !self.at(Punct::Comma) {
                break;
            }
        }
        self.expect_closing(Punct::RParen, Punct::LParen, l2);
        self.expect_closing(Punct::RParen, Punct::LParen, l1);
    }

    /// The arguments of attribute `name`, if any: `Some(args)`, or `None`
    /// after a syntax error.
    fn parse_attr_args(&mut self, name: Symbol) -> Option<Vec<AttrArg>> {
        let Some(open) = self.eat(Punct::LParen) else { return Some(Vec::new()) };
        let text = self.s.ast.symbols().as_str(name).to_owned();
        if !takes_expr_args(&text) {
            self.skip_until(&[Punct::RParen], false, true);
            return Some(Vec::new());
        }
        let mut args = Vec::new();
        let mut ok = true;
        while !self.at(Punct::RParen) && !self.at_eof() {
            let e = if self.at_string() {
                self.parse_string_literal(true)
            } else {
                self.parse_assignment_expr()
            };
            match e {
                Some(e) => args.push(AttrArg::Expr(e)),
                None => {
                    ok = false;
                    self.skip_until(&[Punct::RParen], false, false);
                    break;
                }
            }
            if self.eat(Punct::Comma).is_none() {
                break;
            }
        }
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        ok.then_some(args)
    }

    /// `alignas ( type-id )` / `alignas ( constant-expression )` ([dcl.align]).
    fn parse_alignas(&mut self, out: &mut Vec<AttrId>) {
        let loc = self.bump().span.lo;
        let Some(open) = self.expect(Punct::LParen) else { return };
        let arg = if self.is_type_id_start(self.pos()) {
            self.parse_type_id().map(AttrArg::Type)
        } else {
            self.parse_constant_expr().map(AttrArg::Expr)
        };
        if let Some(l) = self.eat(Punct::Ellipsis) {
            self.unsupported("'alignas' pack expansions", l);
        }
        let Some(arg) = arg else {
            self.skip_until(&[Punct::RParen], false, true);
            return;
        };
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        let name = self.s.intern("alignas");
        if let Some(a) = self.s.act_on_attribute(AttrSyntax::Keyword, None, name, &[arg], loc) {
            out.push(a);
        }
    }
}
