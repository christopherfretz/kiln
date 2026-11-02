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

use crate::expr::prec;
use crate::keywords::Kw;
use crate::parser::Parser;

/// True for the attributes whose arguments are expressions.
fn takes_expr_args(name: &str) -> bool {
    let n = name.trim_start_matches("__").trim_end_matches("__");
    matches!(
        n,
        "deprecated"
            | "nodiscard"
            | "aligned"
            | "warn_unused_result"
            | "ext_vector_type"
            | "vector_size"
            | "alloc_size"
            | "alloc_align"
            | "abi_tag"
            | "flag_enum"
            | "enum_extensibility"
    )
}

/// True for the attributes whose first argument is an identifier when
/// one is written (clang `attributeHasIdentifierArg`).
fn takes_ident_arg(name: &str) -> bool {
    name.trim_start_matches("__").trim_end_matches("__") == "enum_extensibility"
}

/// True for the standard attributes that take no argument list
/// (clang `IsBuiltInOrStandardCXX11Attribute` with no arguments).
fn forbids_args(name: &str) -> bool {
    matches!(
        name,
        "carries_dependency"
            | "fallthrough"
            | "noreturn"
            | "no_unique_address"
            | "likely"
            | "unlikely"
            | "maybe_unused"
    )
}

impl Parser<'_, '_> {
    /// True at `[[`.
    pub(crate) fn at_cxx11_attr(&self) -> bool {
        self.at(Punct::LSquare) && self.peek_is(1, Punct::LSquare)
    }

    /// True at `[[...]]` attribute-specifiers followed by `using` (an
    /// attribute-specifier-seq written before a using-declaration).
    pub(crate) fn at_attributed_using(&self) -> bool {
        let mut i = self.pos();
        loop {
            let t = self.tok_at(i);
            let two = Self::punct_of(t) == Some(Punct::LSquare)
                && Self::punct_of(self.tok_at(i + 1)) == Some(Punct::LSquare);
            if !two {
                return self.kw_of(t) == Some(Kw::Using);
            }
            let mut depth = 0usize;
            loop {
                let t = self.tok_at(i);
                match Self::punct_of(t) {
                    Some(Punct::LSquare) => depth += 1,
                    Some(Punct::RSquare) => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            i += 1;
                            break;
                        }
                    }
                    _ if t.kind == TokKind::Eof => return false,
                    _ => {}
                }
                i += 1;
            }
        }
    }

    /// `[[...]] using ...`: the attributes (an extension clang warns
    /// about), then the using-declaration they apply to.
    pub(crate) fn parse_attributed_using(&mut self) -> Vec<kiln_ast::DeclId> {
        let at = self.peek(2).span.lo;
        self.s.error(DiagId::parse_cxx11_attr_placement, at);
        let mut attrs = Vec::new();
        self.parse_attributes(&mut attrs);
        self.parse_using_attrs(&attrs)
    }

    /// The token index after the `[[...]]` / `alignas(...)` specifiers at
    /// the current token (the current index if there are none).
    fn after_cxx11_attrs(&self) -> Option<usize> {
        let mut i = self.pos();
        loop {
            let t = self.tok_at(i);
            if Self::punct_of(t) == Some(Punct::LSquare)
                && Self::punct_of(self.tok_at(i + 1)) == Some(Punct::LSquare)
            {
                i = self.skip_balanced(i)?;
            } else if self.kw_of(t) == Some(Kw::Alignas) {
                i = self.skip_balanced(i + 1)?;
            } else {
                return Some(i);
            }
        }
    }

    /// A C++11 attribute-specifier-seq before a declaration that takes
    /// none: a namespace-definition, a static_assert-declaration, or (for
    /// `alignas`, `[[...]] using` being an extension) a using-declaration
    /// or alias-declaration (clang `ProhibitAttributes`: "an attribute
    /// list cannot appear here"). Reports and drops it.
    pub(crate) fn discard_attrs_before_unattributed_decl(&mut self) {
        let alignas = self.at_kw(Kw::Alignas);
        if !alignas && !self.at_cxx11_attr() {
            return;
        }
        let Some(j) = self.after_cxx11_attrs() else { return };
        let next = self.kw_of(self.tok_at(j));
        let bad = match next {
            Some(Kw::Namespace | Kw::StaticAssert) => true,
            Some(Kw::Inline) => self.kw_of(self.tok_at(j + 1)) == Some(Kw::Namespace),
            Some(Kw::Using) => alignas,
            _ => false,
        };
        if bad {
            let at = self.loc();
            let mut attrs = Vec::new();
            self.parse_attributes(&mut attrs);
            self.s.error(DiagId::parse_attributes_not_allowed, at);
        }
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

    /// `[[...]]` where no attribute-specifier may appear, such as after
    /// a parenthesized declarator (`int (x) [[a]];`; clang's
    /// `CheckProhibitedCXX11Attribute` in `ParseBracketDeclarator`):
    /// each specifier is reported at its `[[` and skipped unparsed.
    pub(crate) fn discard_prohibited_cxx11_attrs(&mut self) {
        while self.at_cxx11_attr() {
            let at = self.bump().span.lo;
            let mut depth = 1usize;
            while depth > 0 {
                let t = self.tok();
                if t.kind == TokKind::Eof {
                    break;
                }
                match Self::punct_of(t) {
                    Some(Punct::LSquare) => depth += 1,
                    Some(Punct::RSquare) => depth -= 1,
                    _ => {}
                }
                self.bump();
            }
            self.s.error(DiagId::parse_attributes_not_allowed, at);
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
                let Some(n) = self.attr_name() else {
                    self.error_here(DiagId::parse_expected_unqualified_id);
                    self.skip_until(&[Punct::RSquare], false, false);
                    break;
                };
                scope = Some(name);
                name = n;
                self.bump();
            }
            let text = self.s.ast.symbols().as_str(name).to_owned();
            let args = if scope.is_none() && text == "assume" {
                self.parse_assume_arg(name)
            } else if scope.is_none() && self.at(Punct::LParen) && forbids_args(&text) {
                // clang `ParseCXX11AttributeArgs`: a standard attribute
                // without arguments takes no argument list, even `()`.
                let d =
                    self.s.diag(DiagId::parse_attribute_forbids_arguments, self.loc()).arg(text);
                self.emit(d);
                self.parse_attr_args(name);
                None
            } else {
                self.parse_attr_args(name)
            };
            if let Some(l) = self.eat(Punct::Ellipsis) {
                self.unsupported("attribute pack expansions", l);
            }
            if let Some(args) = args
                && let Some(a) = self.s.act_on_attribute(AttrSyntax::Cxx11, scope, name, &args, loc)
            {
                out.push(a);
            }
            // clang `ParseCXX11AttributeSpecifierInternal`: anything but
            // `]` after an attribute is "expected ','" (after the previous
            // token), then skip to the `]` without consuming it.
            if self.at(Punct::RSquare) || self.at_eof() {
                break;
            }
            if self.expect(Punct::Comma).is_none() {
                self.skip_until(&[Punct::RSquare], true, false);
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
        if takes_ident_arg(&text)
            && let Some(id) = self.ident()
        {
            let at = self.bump().span.lo;
            args.push(AttrArg::Ident(id, at));
            if self.eat(Punct::Comma).is_none() {
                self.expect_closing(Punct::RParen, Punct::LParen, open);
                return Some(args);
            }
        }
        while !self.at(Punct::RParen) && !self.at_eof() {
            let e = if self.at_string() {
                self.parse_string_literal(true)
            } else {
                self.parse_assignment_expr()
            };
            let Some(e) = e else {
                ok = false;
                self.skip_until(&[Punct::RParen], false, false);
                break;
            };
            args.push(AttrArg::Expr(e));
            if self.eat(Punct::Comma).is_none() {
                break;
            }
        }
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        ok.then_some(args)
    }

    /// The argument of `[[assume(conditional-expression)]]`
    /// ([dcl.attr.assume]/1; clang's `ParseCXXAssumeAttributeArg`). An
    /// expression that is not a conditional-expression (a comma or an
    /// assignment at the top) is diagnosed at its operator and the
    /// attribute is dropped (`None`). The expression is converted to
    /// `bool` by `Sema::act_on_attributed_stmt` once the statement is known.
    fn parse_assume_arg(&mut self, name: Symbol) -> Option<Vec<AttrArg>> {
        let Some(open) = self.eat(Punct::LParen) else { return Some(Vec::new()) };
        let Some(c) = self.parse_cast_expr().and_then(|l| self.parse_rhs_from(l, prec::COND))
        else {
            self.skip_until(&[Punct::RParen], false, true);
            return None;
        };
        if !self.at(Punct::RParen) && !self.at(Punct::RSquare) {
            if let Some(full) = self.parse_rhs_from(c, prec::COMMA) {
                let loc = self.s.expr_loc(full);
                let text = self.s.ast.symbols().as_str(name).to_owned();
                let d = self.s.diag(DiagId::parse_assume_attr_expects_cond_expr, loc).arg(text);
                self.emit(d);
            }
            self.skip_until(&[Punct::RParen], false, true);
            return None;
        }
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        Some(vec![AttrArg::Expr(c)])
    }

    /// `alignas ( type-id )` / `alignas ( constant-expression )` ([dcl.align]).
    fn parse_alignas(&mut self, out: &mut Vec<AttrId>) {
        let loc = self.bump().span.lo;
        let Some(open) = self.expect(Punct::LParen) else { return };
        let begin = self.tok().span.lo;
        let arg = if self.is_type_id_start(self.pos()) {
            self.parse_type_id().map(AttrArg::Type)
        } else {
            self.parse_constant_expr().map(AttrArg::Expr)
        };
        // `alignas(Ts...)` ([dcl.align]/1, [temp.variadic]/5).
        let ellipsis = self.eat(Punct::Ellipsis);
        let arg = arg.map(|a| self.s.act_on_alignas_operand(a, ellipsis, begin));
        if let Some(None) = arg {
            self.expect_closing(Punct::RParen, Punct::LParen, open);
            return;
        }
        let Some(arg) = arg.flatten() else {
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
