//! Tentative parsing ([stmt.ambig], [dcl.ambig.res]; spec/parse.md §2.1).
//!
//! A side-effect-free scanner over token indices that decides whether
//! what follows is a declaration (or type-id) or an expression, ported
//! from clang's `ParseTentative.cpp`. Nothing is consumed, diagnosed or
//! declared: the scanner only classifies names, and identifiers its
//! declarators would declare are tracked locally (clang's
//! `TentativelyDeclaredIdentifiers`).

use kiln_lex::{Punct, TokKind};
use kiln_sema::NameClass;
use kiln_source::symbol::Symbol;

use crate::decl::is_decl_specifier_kw;
use crate::keywords::Kw;
use crate::parser::Parser;

/// The result of a tentative parse step (clang's `TPResult`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Tpr {
    /// Definitely a declaration (or type-id).
    True,
    /// Definitely not.
    False,
    /// Could be either; keep looking.
    Ambiguous,
    /// A syntax error: let the declaration parser report it.
    Error,
}

/// Where a type-id is being disambiguated from an expression
/// ([dcl.ambig.res]/2-3, clang's `TentativeCXXTypeIdContext`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)] // template arguments and _Generic arrive later
pub enum TypeIdContext {
    /// Inside parentheses: `sizeof(T(x))`, `(T())x`; a `)` after an
    /// ambiguous abstract declarator makes it a type-id.
    InParens,
    /// Nothing follows that could tip an ambiguity: an ambiguous parse is
    /// an expression.
    Unambiguous,
    /// A template argument: `>`, `,`, `>>` or `...` before one of them
    /// make it a type-id.
    AsTemplateArgument,
    /// After `->` in a trailing return type: always a type-id.
    InTrailingReturnType,
    /// The controlling operand of `_Generic`: a `,` makes it a type-id.
    AsGenericSelectionArgument,
}

/// The scanner state: a token index and the identifiers tentatively
/// declared so far.
struct Scan<'x, 'a, 'sm> {
    p: &'x Parser<'a, 'sm>,
    i: usize,
    declared: Vec<Symbol>,
}

impl<'x, 'a, 'sm> Scan<'x, 'a, 'sm> {
    fn new(p: &'x Parser<'a, 'sm>, i: usize) -> Self {
        Scan { p, i, declared: Vec::new() }
    }

    fn punct_at(&self, i: usize) -> Option<Punct> {
        Parser::punct_of(self.p.tok_at(i))
    }

    fn at(&self, p: Punct) -> bool {
        self.punct_at(self.i) == Some(p)
    }

    fn next_is(&self, p: Punct) -> bool {
        self.punct_at(self.i + 1) == Some(p)
    }

    fn kw(&self) -> Option<Kw> {
        self.p.kw_of(self.p.tok_at(self.i))
    }

    fn kw_at(&self, i: usize) -> Option<Kw> {
        self.p.kw_of(self.p.tok_at(i))
    }

    fn ident_at(&self, i: usize) -> Option<Symbol> {
        self.p.ident_of(self.p.tok_at(i))
    }

    fn eat(&mut self, p: Punct) -> bool {
        let r = self.at(p);
        if r {
            self.i += 1;
        }
        r
    }

    fn at_cxx11_attr(&self) -> bool {
        self.at(Punct::LSquare) && self.next_is(Punct::LSquare)
    }

    /// Skips a balanced bracket group starting at the opener at `i`;
    /// returns the index after its closer. `None` at a `;` outside nested
    /// braces, an unbalanced closer or end of file.
    fn skip_group(&self, mut i: usize) -> Option<usize> {
        let mut stack: Vec<Punct> = Vec::new();
        loop {
            let t = self.p.tok_at(i);
            if t.kind == TokKind::Eof {
                return None;
            }
            match Parser::punct_of(t) {
                Some(Punct::LParen) => stack.push(Punct::RParen),
                Some(Punct::LSquare) => stack.push(Punct::RSquare),
                Some(Punct::LBrace) => stack.push(Punct::RBrace),
                Some(c @ (Punct::RParen | Punct::RSquare | Punct::RBrace)) => {
                    if stack.pop() != Some(c) {
                        return None;
                    }
                    if stack.is_empty() {
                        return Some(i + 1);
                    }
                }
                Some(Punct::Semi) if !stack.contains(&Punct::RBrace) => return None,
                _ => {}
            }
            i += 1;
        }
    }

    /// Consumes a balanced group at the current opener.
    fn skip_group_here(&mut self) -> bool {
        match self.skip_group(self.i) {
            Some(j) => {
                self.i = j;
                true
            }
            None => false,
        }
    }

    /// Skips to (not past) the first `,` or `)` at depth 0 (a default
    /// argument). False at a `;` or end of file.
    fn skip_to_comma_or_rparen(&mut self) -> bool {
        loop {
            match self.punct_at(self.i) {
                Some(Punct::Comma | Punct::RParen) => return true,
                Some(Punct::Semi | Punct::RSquare | Punct::RBrace) => return false,
                Some(Punct::LParen | Punct::LSquare | Punct::LBrace) => {
                    if !self.skip_group_here() {
                        return false;
                    }
                }
                _ => {
                    if self.p.tok_at(self.i).kind == TokKind::Eof {
                        return false;
                    }
                    self.i += 1;
                }
            }
        }
    }

    /// Skips `[[...]]`, `alignas(...)` and `__attribute__((...))`.
    /// False on an unbalanced group.
    fn skip_attributes(&mut self) -> bool {
        loop {
            if self.at_cxx11_attr() || matches!(self.kw(), Some(Kw::Alignas | Kw::Attribute)) {
                if !self.at(Punct::LSquare) {
                    self.i += 1;
                }
                if !self.at(Punct::LParen) && !self.at(Punct::LSquare) {
                    return false;
                }
                if !self.skip_group_here() {
                    return false;
                }
            } else {
                return true;
            }
        }
    }

    /// The end of a (possibly qualified) name at the current token and
    /// its classification, honoring identifiers declared earlier in this
    /// scan (clang's `ANK_TentativeDecl`).
    fn name_here(&self) -> Option<(usize, NameClass)> {
        let (end, class, sym) = self.p.scan_name(self.i)?;
        let unqualified = end == self.i + 1;
        if unqualified && self.declared.contains(&sym) {
            return Some((end, NameClass::Undeclared));
        }
        Some((end, class))
    }

    /// The token after a type-specifier that was just named: `(` leaves
    /// a functional cast possible, `{` is `braced`.
    fn after_simple_type(&self, next: usize, braced: Tpr) -> Tpr {
        match self.punct_at(next) {
            Some(Punct::LParen) => Tpr::Ambiguous,
            Some(Punct::LBrace) => braced,
            _ => Tpr::True,
        }
    }

    /// clang's `isCXXDeclarationSpecifier`: can a decl-specifier start at
    /// the current token? `True` if it must be one, `Ambiguous` for a
    /// simple-type-specifier followed by `(` (a functional cast is
    /// possible), `braced` for one followed by `{`.
    fn decl_specifier(&self, braced: Tpr) -> Tpr {
        if let Some(k) = self.kw() {
            return match k {
                Kw::Typedef
                | Kw::Static
                | Kw::Extern
                | Kw::Register
                | Kw::Mutable
                | Kw::ThreadLocal
                | Kw::Constexpr
                | Kw::Constinit
                | Kw::Consteval
                | Kw::Inline
                | Kw::Virtual
                | Kw::Explicit
                | Kw::Friend
                | Kw::Struct
                | Kw::Class
                | Kw::Union
                | Kw::Enum
                | Kw::Const
                | Kw::Volatile
                | Kw::Restrict
                | Kw::Alignas
                | Kw::Attribute
                | Kw::Declspec
                | Kw::Extension => Tpr::True,
                Kw::Void
                | Kw::Bool
                | Kw::Char
                | Kw::WChar
                | Kw::Char8
                | Kw::Char16
                | Kw::Char32
                | Kw::Short
                | Kw::Int
                | Kw::Long
                | Kw::Signed
                | Kw::Unsigned
                | Kw::Float
                | Kw::Double
                | Kw::Int128
                | Kw::Float128 => self.after_simple_type(self.i + 1, braced),
                // [dcl.type.auto.deduct]: `auto(x)` and `auto{x}` are
                // expressions in C++23.
                Kw::Auto => match self.punct_at(self.i + 1) {
                    Some(Punct::LBrace) => Tpr::False,
                    Some(Punct::LParen) => Tpr::Ambiguous,
                    _ => Tpr::True,
                },
                Kw::Decltype | Kw::Typeof => {
                    if self.punct_at(self.i + 1) != Some(Punct::LParen) {
                        return Tpr::Error;
                    }
                    match self.skip_group(self.i + 1) {
                        Some(j) => self.after_simple_type(j, braced),
                        None => Tpr::Error,
                    }
                }
                Kw::Typename => {
                    let Some((end, _, _)) = self.p.scan_name(self.i + 1) else {
                        return Tpr::Error;
                    };
                    self.after_simple_type(end, braced)
                }
                _ => Tpr::False,
            };
        }
        if self.ident_at(self.i).is_none() && !self.at(Punct::ColonColon) {
            return Tpr::False;
        }
        let Some((end, class)) = self.name_here() else {
            return Tpr::False;
        };
        // `foo bar` with `foo` undeclared: `foo` is (meant as) a type
        // name, so the declaration parser reports it. A declared non-type
        // `foo` is an expression (clang's `ClassifyName`).
        if matches!(class, NameClass::Undeclared) && self.ident_at(end).is_some() {
            return Tpr::True;
        }
        match class {
            NameClass::Type(..) => self.after_simple_type(end, braced),
            _ => Tpr::False,
        }
    }

    /// clang's `isCXXDeclarationSpecifierAType`.
    fn decl_specifier_is_type(&self) -> bool {
        match self.kw() {
            Some(Kw::Const | Kw::Volatile | Kw::Restrict) => false,
            Some(k) => crate::decl::is_type_specifier_kw(k),
            None => matches!(self.name_here(), Some((_, NameClass::Type(..)))),
        }
    }

    /// clang's `TryConsumeDeclarationSpecifier`: skips one
    /// simple-type-specifier (or other decl-specifier).
    fn consume_decl_specifier(&mut self) -> Tpr {
        match self.kw() {
            Some(Kw::Decltype | Kw::Typeof) => {
                self.i += 1;
                if !self.at(Punct::LParen) || !self.skip_group_here() {
                    return Tpr::Error;
                }
            }
            Some(Kw::Alignas | Kw::Attribute) => {
                if !self.skip_attributes() {
                    return Tpr::Error;
                }
            }
            Some(Kw::Typename | Kw::Struct | Kw::Class | Kw::Union | Kw::Enum) => {
                self.i += 1;
                match self.p.scan_name(self.i) {
                    Some((end, _, _)) => self.i = end,
                    None => return Tpr::Error,
                }
            }
            Some(_) => self.i += 1,
            None => match self.p.scan_name(self.i) {
                Some((end, _, _)) => self.i = end,
                None => return Tpr::Error,
            },
        }
        Tpr::Ambiguous
    }

    /// clang's non-tentative `isDeclarationSpecifier`: a decl-specifier
    /// keyword or a type name at the current token.
    fn is_decl_specifier_start(&self) -> bool {
        match self.kw() {
            Some(k) => is_decl_specifier_kw(k),
            None => {
                (self.ident_at(self.i).is_some() || self.at(Punct::ColonColon))
                    && matches!(self.name_here(), Some((_, NameClass::Type(..))))
            }
        }
    }

    /// clang's `TryParsePtrOperatorSeq`.
    fn ptr_operator_seq(&mut self) -> Tpr {
        loop {
            let member = self.member_pointer_end();
            if let Some(j) = member {
                self.i = j;
            } else if matches!(
                self.punct_at(self.i),
                Some(Punct::Star | Punct::Amp | Punct::AmpAmp | Punct::Caret)
            ) {
                self.i += 1;
            } else {
                return Tpr::Ambiguous;
            }
            loop {
                if matches!(self.kw(), Some(Kw::Const | Kw::Volatile | Kw::Restrict)) {
                    self.i += 1;
                } else if self.at_cxx11_attr() || self.kw() == Some(Kw::Attribute) {
                    if !self.skip_attributes() {
                        return Tpr::Error;
                    }
                } else {
                    break;
                }
            }
        }
    }

    /// The index after `N::*` (a pointer-to-member operator) at the
    /// current token.
    fn member_pointer_end(&self) -> Option<usize> {
        let mut i = self.i;
        if self.punct_at(i) == Some(Punct::ColonColon) {
            i += 1;
        }
        let mut any = false;
        while self.ident_at(i).is_some() && self.punct_at(i + 1) == Some(Punct::ColonColon) {
            i += 2;
            any = true;
        }
        (any && self.punct_at(i) == Some(Punct::Star)).then_some(i + 1)
    }

    /// clang's `TryParseDeclarator`.
    #[allow(clippy::fn_params_excessive_bools, reason = "clang's TryParseDeclarator flags")]
    fn declarator(
        &mut self,
        may_be_abstract: bool,
        may_have_identifier: bool,
        may_have_direct_init: bool,
        may_have_trailing_return: bool,
    ) -> Tpr {
        if self.ptr_operator_seq() == Tpr::Error {
            return Tpr::Error;
        }
        self.eat(Punct::Ellipsis);
        let qualified_id = {
            let mut i = self.i;
            if self.punct_at(i) == Some(Punct::ColonColon) {
                i += 1;
            }
            while self.ident_at(i).is_some() && self.punct_at(i + 1) == Some(Punct::ColonColon) {
                i += 2;
            }
            (i != self.i && (self.ident_at(i).is_some() || self.kw_at(i) == Some(Kw::Operator)))
                .then_some(i)
        };
        let id_here = self.ident_at(self.i).is_some() || self.kw() == Some(Kw::Operator);
        if may_have_identifier && (id_here || qualified_id.is_some()) {
            if let Some(i) = qualified_id {
                self.i = i;
            } else if let Some(s) = self.ident_at(self.i) {
                self.declared.push(s);
            }
            if self.kw() == Some(Kw::Operator) {
                if !self.operator_id() {
                    return Tpr::Error;
                }
            } else {
                self.i += 1;
            }
        } else if self.at(Punct::LParen) {
            self.i += 1;
            let function = may_be_abstract
                && (self.at(Punct::RParen)
                    || (self.at(Punct::Ellipsis) && self.next_is(Punct::RParen))
                    || self.is_decl_specifier_start());
            if function {
                let r = self.function_declarator(may_have_trailing_return);
                if r != Tpr::Ambiguous {
                    return r;
                }
            } else {
                if matches!(self.kw(), Some(Kw::Attribute | Kw::Declspec)) {
                    return Tpr::True;
                }
                let r = self.declarator(may_be_abstract, may_have_identifier, false, false);
                if r != Tpr::Ambiguous {
                    return r;
                }
                if !self.eat(Punct::RParen) {
                    return Tpr::False;
                }
            }
        } else if !may_be_abstract {
            return Tpr::False;
        }
        if may_have_direct_init {
            return Tpr::Ambiguous;
        }
        loop {
            let r = if self.at(Punct::LParen) {
                if !may_be_abstract && !self.function_declarator_at(self.i, &mut false) {
                    break;
                }
                self.i += 1;
                self.function_declarator(may_have_trailing_return)
            } else if self.at(Punct::LSquare) && !self.at_cxx11_attr() {
                if self.skip_group_here() { Tpr::Ambiguous } else { Tpr::Error }
            } else if self.kw() == Some(Kw::Requires) {
                Tpr::True
            } else {
                break;
            };
            if r != Tpr::Ambiguous {
                return r;
            }
        }
        Tpr::Ambiguous
    }

    /// Skips `operator @` (an operator-function-id or conversion name).
    fn operator_id(&mut self) -> bool {
        self.i += 1;
        match self.punct_at(self.i) {
            Some(Punct::LParen) if self.punct_at(self.i + 1) == Some(Punct::RParen) => {
                self.i += 2;
                true
            }
            Some(Punct::LSquare) if self.punct_at(self.i + 1) == Some(Punct::RSquare) => {
                self.i += 2;
                true
            }
            Some(_) => {
                self.i += 1;
                true
            }
            None => {
                if matches!(self.kw(), Some(Kw::New | Kw::Delete)) {
                    self.i += 1;
                    if self.at(Punct::LSquare) && self.next_is(Punct::RSquare) {
                        self.i += 2;
                    }
                    return true;
                }
                // A conversion-function-id: skip the type-specifiers.
                let start = self.i;
                while self.decl_specifier(Tpr::True) != Tpr::False {
                    if self.consume_decl_specifier() == Tpr::Error {
                        return false;
                    }
                }
                self.ptr_operator_seq();
                self.i != start
            }
        }
    }

    /// clang's `TryParseFunctionDeclarator`, after the `(`.
    fn function_declarator(&mut self, may_have_trailing_return: bool) -> Tpr {
        let mut r = self.parameter_declaration_clause(&mut false);
        if r == Tpr::Ambiguous && !self.at(Punct::RParen) {
            r = Tpr::False;
        }
        if matches!(r, Tpr::False | Tpr::Error) {
            return r;
        }
        // Through the `)`.
        if !self.skip_past_rparen() {
            return Tpr::Error;
        }
        while matches!(self.kw(), Some(Kw::Const | Kw::Volatile | Kw::Restrict)) {
            self.i += 1;
        }
        if matches!(self.punct_at(self.i), Some(Punct::Amp | Punct::AmpAmp)) {
            self.i += 1;
        }
        if self.kw() == Some(Kw::Throw) {
            self.i += 1;
            if !self.at(Punct::LParen) || !self.skip_group_here() {
                return Tpr::Error;
            }
        }
        if self.kw() == Some(Kw::Noexcept) {
            self.i += 1;
            if self.at(Punct::LParen) && !self.skip_group_here() {
                return Tpr::Error;
            }
        }
        if !self.skip_attributes() {
            return Tpr::Ambiguous;
        }
        if self.at(Punct::Arrow) && may_have_trailing_return {
            if r == Tpr::True {
                return r;
            }
            self.i += 1;
            if self.ident_at(self.i).is_some()
                && matches!(
                    self.name_here(),
                    Some((_, NameClass::Value(_) | NameClass::Undeclared))
                )
            {
                return Tpr::False;
            }
            if self.type_id(TypeIdContext::InTrailingReturnType).0 {
                return Tpr::True;
            }
        }
        Tpr::Ambiguous
    }

    /// Skips through the `)` closing a parameter clause (clang's
    /// `SkipUntil(tok::r_paren, StopAtSemi)`).
    fn skip_past_rparen(&mut self) -> bool {
        loop {
            match self.punct_at(self.i) {
                Some(Punct::RParen) => {
                    self.i += 1;
                    return true;
                }
                Some(Punct::Semi | Punct::RSquare | Punct::RBrace) => return false,
                Some(Punct::LParen | Punct::LSquare | Punct::LBrace) => {
                    if !self.skip_group_here() {
                        return false;
                    }
                }
                _ => {
                    if self.p.tok_at(self.i).kind == TokKind::Eof {
                        return false;
                    }
                    self.i += 1;
                }
            }
        }
    }

    /// clang's `TryParseParameterDeclarationClause`, after the `(`.
    fn parameter_declaration_clause(&mut self, _invalid: &mut bool) -> Tpr {
        if self.at(Punct::RParen) {
            return Tpr::Ambiguous;
        }
        loop {
            if self.eat(Punct::Ellipsis) {
                return if self.at(Punct::RParen) { Tpr::True } else { Tpr::False };
            }
            if self.at_cxx11_attr() {
                return Tpr::True;
            }
            // A parameter's initializer needs `=`: `T{` is not one.
            let mut r = self.decl_specifier(Tpr::False);
            if r != Tpr::Ambiguous {
                return r;
            }
            let mut seen_type = false;
            let is_auto = self.kw() == Some(Kw::Auto);
            loop {
                seen_type |= self.decl_specifier_is_type();
                if self.consume_decl_specifier() == Tpr::Error {
                    return Tpr::Error;
                }
                if seen_type && self.ident_at(self.i).is_some() {
                    return Tpr::True;
                }
                r = self.decl_specifier(Tpr::False);
                if r == Tpr::Error || r == Tpr::True {
                    return r;
                }
                if r == Tpr::False {
                    break;
                }
            }
            let r = self.declarator(true, true, false, is_auto);
            if r != Tpr::Ambiguous {
                return r;
            }
            if self.kw() == Some(Kw::Attribute) {
                return Tpr::True;
            }
            if self.at(Punct::Equal) {
                self.i += 1;
                if !self.skip_to_comma_or_rparen() {
                    return Tpr::Error;
                }
            }
            if self.eat(Punct::Ellipsis) {
                return if self.at(Punct::RParen) { Tpr::True } else { Tpr::False };
            }
            if !self.eat(Punct::Comma) {
                break;
            }
        }
        Tpr::Ambiguous
    }

    /// clang's `isCXXFunctionDeclarator` at the `(` at index `open`: true
    /// unless the parentheses are certainly an initializer; `ambiguous`
    /// is set when they could be either.
    fn function_declarator_at(&self, open: usize, ambiguous: &mut bool) -> bool {
        let mut s = Scan { p: self.p, i: open + 1, declared: self.declared.clone() };
        let mut invalid = false;
        let mut r = s.parameter_declaration_clause(&mut invalid);
        if r == Tpr::Ambiguous {
            if s.at(Punct::RParen) {
                let n = s.i + 1;
                let virt = s.ident_at(n).is_some_and(|sym| {
                    let t = self.p.s.ast.symbols().as_str(sym);
                    t == "override" || t == "final"
                });
                let decisive = matches!(
                    s.punct_at(n),
                    Some(
                        Punct::Amp
                            | Punct::AmpAmp
                            | Punct::LSquare
                            | Punct::LBrace
                            | Punct::Equal
                            | Punct::Arrow
                    )
                ) || matches!(
                    s.kw_at(n),
                    Some(Kw::Const | Kw::Volatile | Kw::Throw | Kw::Noexcept | Kw::Try)
                ) || virt;
                if decisive {
                    r = Tpr::True;
                }
            } else {
                r = Tpr::False;
            }
        }
        if r == Tpr::Ambiguous {
            *ambiguous = true;
        }
        r != Tpr::False
    }

    /// clang's `isCXXTypeId`: (is a type-id, the parse was ambiguous).
    fn type_id(&mut self, ctx: TypeIdContext) -> (bool, bool) {
        let r = self.decl_specifier(Tpr::False);
        if r != Tpr::Ambiguous {
            return (r != Tpr::False, false);
        }
        let trailing = self.kw() == Some(Kw::Auto);
        self.consume_decl_specifier();
        let mut r = self.declarator(true, false, false, trailing);
        if r == Tpr::Error {
            r = Tpr::True;
        }
        if r == Tpr::Ambiguous {
            let tips = match ctx {
                TypeIdContext::InParens => self.at(Punct::RParen),
                TypeIdContext::AsGenericSelectionArgument => self.at(Punct::Comma),
                TypeIdContext::AsTemplateArgument => {
                    let close = |p: Option<Punct>| {
                        matches!(p, Some(Punct::Greater | Punct::Comma | Punct::GreaterGreater))
                    };
                    close(self.punct_at(self.i))
                        || (self.at(Punct::Ellipsis) && close(self.punct_at(self.i + 1)))
                }
                TypeIdContext::InTrailingReturnType => true,
                TypeIdContext::Unambiguous => false,
            };
            return (tips, tips);
        }
        (r == Tpr::True, false)
    }

    /// clang's `TryParseInitDeclaratorList`.
    fn init_declarator_list(&mut self, may_have_trailing_return: bool) -> Tpr {
        loop {
            let r = self.declarator(false, true, false, may_have_trailing_return);
            if r != Tpr::Ambiguous {
                return r;
            }
            if matches!(self.kw(), Some(Kw::Asm | Kw::Attribute)) {
                return Tpr::True;
            }
            if self.at(Punct::LParen) {
                if !self.skip_group_here() {
                    return Tpr::Error;
                }
            } else if self.at(Punct::LBrace) || self.at(Punct::Equal) {
                // An expression is never directly followed by a
                // braced-init-list; g++ and MSVC stop at `=` too.
                return Tpr::True;
            }
            if !self.eat(Punct::Comma) {
                break;
            }
        }
        Tpr::Ambiguous
    }

    /// clang's `TryParseSimpleDeclaration`, at a decl-specifier that was
    /// `Ambiguous`.
    fn simple_declaration(&mut self, allow_for_range: bool) -> Tpr {
        let trailing = self.kw() == Some(Kw::Auto);
        if self.consume_decl_specifier() == Tpr::Error {
            return Tpr::Error;
        }
        // Two decl-specifiers in a row make it a declaration.
        if !self.at(Punct::LParen) {
            match self.decl_specifier(Tpr::False) {
                Tpr::Ambiguous => return Tpr::True,
                r @ (Tpr::True | Tpr::Error) => return r,
                Tpr::False => {}
            }
        }
        let r = self.init_declarator_list(trailing);
        if r != Tpr::Ambiguous {
            return r;
        }
        if !self.at(Punct::Semi) && (!allow_for_range || !self.at(Punct::Colon)) {
            return Tpr::False;
        }
        Tpr::Ambiguous
    }

    /// clang's `isCXXSimpleDeclaration`.
    fn is_simple_declaration(&mut self, allow_for_range: bool) -> bool {
        let r = self.decl_specifier(Tpr::False);
        if r != Tpr::Ambiguous {
            return r != Tpr::False;
        }
        // Declarations take precedence over expressions; errors are left
        // to the declaration parser.
        self.simple_declaration(allow_for_range) != Tpr::False
    }
}

impl Parser<'_, '_> {
    /// [stmt.ambig]: whether the statement at the current token (after
    /// any attributes) is a declaration-statement (clang's
    /// `isCXXDeclarationStatement`).
    pub(crate) fn is_cxx_declaration_statement(&self, allow_for_range: bool) -> bool {
        let mut s = Scan::new(self, self.pos());
        if !s.skip_attributes() {
            return true;
        }
        while s.kw() == Some(Kw::Extension) {
            s.i += 1;
        }
        match s.kw() {
            Some(Kw::Asm | Kw::Namespace | Kw::Using | Kw::StaticAssert) => true,
            _ => s.is_simple_declaration(allow_for_range),
        }
    }

    /// Whether the condition at the current token declares a variable
    /// (clang's `isCXXConditionDeclarationOrInitStatement`, condition
    /// half): it must have a brace-or-equal-initializer.
    pub(crate) fn is_cxx_condition_declaration(&self) -> bool {
        let mut s = Scan::new(self, self.pos());
        if !s.skip_attributes() {
            return true;
        }
        match s.decl_specifier(Tpr::False) {
            Tpr::True | Tpr::Error => return true,
            Tpr::False => return false,
            Tpr::Ambiguous => {}
        }
        let trailing = s.kw() == Some(Kw::Auto);
        if s.consume_decl_specifier() == Tpr::Error {
            return true;
        }
        match s.declarator(true, true, false, trailing) {
            Tpr::True | Tpr::Error => return true,
            Tpr::False => return false,
            Tpr::Ambiguous => {}
        }
        s.at(Punct::Equal) || s.at_cxx11_attr() || matches!(s.kw(), Some(Kw::Asm | Kw::Attribute))
    }

    /// [dcl.ambig.res]/2-3: whether a type-id starts at the current token
    /// (clang's `isCXXTypeId`). Returns (is a type-id, the parse was
    /// ambiguous -- it could also have been an expression).
    #[allow(dead_code)] // the entry point for WP-069 (template arguments)
    pub(crate) fn is_cxx_type_id(&self, ctx: TypeIdContext) -> (bool, bool) {
        self.is_cxx_type_id_at(self.pos(), ctx)
    }

    /// [`Parser::is_cxx_type_id`] at token index `i`.
    pub(crate) fn is_cxx_type_id_at(&self, i: usize, ctx: TypeIdContext) -> (bool, bool) {
        Scan::new(self, i).type_id(ctx)
    }

    /// [dcl.ambig.res]/1: whether the `(` at the current token, after a
    /// declarator-id that could take a direct initializer, starts a
    /// function declarator (clang's `isCXXFunctionDeclarator`). `name`
    /// is the declarator-id, tentatively declared inside the parentheses.
    /// Returns (function declarator, the parse was ambiguous).
    pub(crate) fn is_cxx_function_declarator(&self, name: Option<Symbol>) -> (bool, bool) {
        let mut s = Scan::new(self, self.pos());
        s.declared.extend(name);
        let mut ambiguous = false;
        let f = s.function_declarator_at(self.pos(), &mut ambiguous);
        (f, ambiguous)
    }
}
