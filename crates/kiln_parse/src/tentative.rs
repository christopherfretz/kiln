//! Tentative parsing ([stmt.ambig], [dcl.ambig.res]; spec/parse.md §2.1).
//!
//! A side-effect-free scanner over token indices that decides whether
//! what follows is a declaration (or type-id) or an expression, ported
//! from clang's `ParseTentative.cpp`. Nothing is consumed, diagnosed or
//! declared: the scanner only classifies names, and identifiers its
//! declarators would declare are tracked locally (clang's
//! `TentativelyDeclaredIdentifiers`).

use kiln_ast::DeclId;
use kiln_ast::decl::DeclKind;
use kiln_lex::{Punct, Std, TokKind};
use kiln_sema::NameClass;
use kiln_source::symbol::Symbol;

use crate::decl::is_decl_specifier_kw;
use crate::keywords::Kw;
use crate::parser::{Parser, is_template_class};

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

    /// Does a placeholder type specifier (`auto` or `decltype(auto)`)
    /// start at token `i`? Used after a type-constraint.
    fn placeholder_at(&self, i: usize) -> bool {
        match self.p.kw_of(self.p.tok_at(i)) {
            Some(Kw::Auto) => true,
            Some(Kw::Decltype) => {
                self.p.tok_at(i + 1).kind == TokKind::Punct(Punct::LParen)
                    && self.p.kw_of(self.p.tok_at(i + 2)) == Some(Kw::Auto)
            }
            _ => false,
        }
    }

    /// clang's `isCXXDeclarationSpecifier`: can a decl-specifier start at
    /// the current token? `True` if it must be one, `Ambiguous` for a
    /// simple-type-specifier followed by `(` (a functional cast is
    /// possible), `braced` for one followed by `{`.
    /// The token index past the name after `typename` at `self.i`: a
    /// splice `[: r :]` (-freflection) or a (qualified) name.
    fn typename_end(&self) -> Option<usize> {
        self.p.splice_spec_end(self.i + 1).or_else(|| self.p.scan_name(self.i + 1).map(|n| n.0))
    }

    #[allow(clippy::too_many_lines, reason = "one arm per decl-specifier keyword")]
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
                | Kw::Nonnull
                | Kw::Nullable
                | Kw::NullUnspecified
                | Kw::NullableResult
                | Kw::Complex
                | Kw::Imaginary
                | Kw::Alignas
                | Kw::Attribute
                // `__auto_type(x)` is never an expression (clang: "expected
                // expression").
                | Kw::GnuAutoType
                | Kw::Declspec => Tpr::True,
                // GNU `__extension__` is a unary operator as well as a
                // decl-specifier prefix (clang's `ParseCastExpression`):
                // it starts a decl-specifier only when one follows it, so
                // `(__extension__ 1 + 2)` stays a parenthesized expression.
                Kw::Extension => {
                    let mut s = Scan { p: self.p, i: self.i, declared: self.declared.clone() };
                    while s.kw() == Some(Kw::Extension) {
                        s.i += 1;
                    }
                    if s.decl_specifier(braced) == Tpr::False { Tpr::False } else { Tpr::True }
                }
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
                | Kw::Float128
                | Kw::Float16
                | Kw::Float32
                | Kw::Float64
                | Kw::StdFloat128
                | Kw::Float32x
                | Kw::Float64x
                | Kw::Bf16
                | Kw::Fp16
                | Kw::Decimal32
                | Kw::Decimal64
                | Kw::Decimal128 => self.after_simple_type(self.i + 1, braced),
                // [dcl.type.auto.deduct]: `auto(x)` and `auto{x}` are
                // expressions in C++23.
                Kw::Auto => match self.punct_at(self.i + 1) {
                    Some(Punct::LBrace) => Tpr::False,
                    Some(Punct::LParen) => Tpr::Ambiguous,
                    _ => Tpr::True,
                },
                Kw::Decltype | Kw::Typeof | Kw::TypeofUnqual | Kw::Atomic | Kw::BitInt | Kw::ExtInt => {
                    // Without `(`, `_Atomic` is the type qualifier: as `const`.
                    if self.punct_at(self.i + 1) != Some(Punct::LParen) {
                        return if k == Kw::Atomic { Tpr::True } else { Tpr::Error };
                    }
                    match self.skip_group(self.i + 1) {
                        Some(j)
                            if k == Kw::Decltype && self.punct_at(j) == Some(Punct::ColonColon) =>
                        {
                            self.unparsed_decltype_scope(self.i, j, braced)
                        }
                        Some(j) => self.after_simple_type(j, braced),
                        None => Tpr::Error,
                    }
                }
                Kw::Typename => match self.typename_end() {
                    Some(end) => self.after_simple_type(end, braced),
                    None => Tpr::Error,
                },
                _ => Tpr::False,
            };
        }
        // spec/sema-templates.md §9: `__remove_cv(T)` is a type
        // specifier, `__is_same(T, U)` an expression.
        match self.p.builtin_kw_at(self.i) {
            Some(crate::keywords::BuiltinKw::Transform(_)) => {
                return match self.skip_group(self.i + 1) {
                    Some(j) => self.after_simple_type(j, braced),
                    None => Tpr::Error,
                };
            }
            Some(crate::keywords::BuiltinKw::Trait(_)) => return Tpr::False,
            None => {}
        }
        if self.ident_at(self.i).is_none() && !self.at(Punct::ColonColon) {
            return Tpr::False;
        }
        let Some((end, class)) = self.name_here() else {
            return Tpr::False;
        };
        if matches!(class, NameClass::Undeclared) && self.undeclared_type_start(end) {
            return Tpr::True;
        }
        match class {
            NameClass::Type(..) => self.after_simple_type(end, braced),
            NameClass::TypeTemplate(d) if self.p.s.names_class_type(d) => {
                self.after_simple_type(end, braced)
            }
            // `C auto`, `C decltype(auto)` ([dcl.spec.auto.general]/1).
            NameClass::Concept(_) if self.placeholder_at(end) => Tpr::True,
            // `T::type x`, `T::type const` (a missing `typename`,
            // diagnosed by the declaration parser): no expression has a
            // name or a decl-specifier there (clang's
            // `isCXXDeclarationSpecifier` on a dependent `annot_cxxscope`).
            NameClass::Dependent if end > self.i + 1 => self.after_dependent_name(end),
            _ => Tpr::False,
        }
    }

    /// `foo bar` with `foo` undeclared (the name ends at `end`): `foo` is
    /// (meant as) a type name, so the declaration parser reports it. A
    /// declared non-type `foo` is an expression (clang's `ClassifyName`).
    /// A qualified name not found in its non-dependent scope (`S::nom x`)
    /// is not: clang's `ANK_Unresolved` on an `annot_cxxscope` is `False`
    /// unless the caller tracks `InvalidAsDeclSpec` (WP-318), so
    /// `(S::nom x)` is a parenthesized expression.
    fn undeclared_type_before_ident(&self, end: usize) -> bool {
        self.ident_at(end).is_some() && !(end > self.i + 1 && self.p.unresolved_scope_name(self.i))
    }

    /// [`Scan::undeclared_type_before_ident`] or
    /// [`Scan::undeclared_template_id_before_ident`].
    fn undeclared_type_start(&self, end: usize) -> bool {
        self.undeclared_type_before_ident(end) || self.undeclared_template_id_before_ident(end)
    }

    /// `foo<args> bar` with `foo` unqualified and undeclared (the name
    /// ends at `end`): lookup finding nothing before `<` makes `foo` a
    /// template-name ([temp.names]/3.3, assumed to name functions), and a
    /// declarator after the list shows it was meant as a type, so the
    /// declaration parser reports "no template named" (clang's
    /// `TNK_Undeclared_template`, `Ambiguous` in
    /// `isCXXDeclarationSpecifier` until the declarator decides).
    fn undeclared_template_id_before_ident(&self, end: usize) -> bool {
        end == self.i + 1
            && self.punct_at(end) == Some(Punct::Less)
            && matches!(self.p.skip_template_args(end),
                Some((close, false)) if self.ident_at(close + 1).is_some())
    }

    /// The `NameClass::Dependent` arm of [`Scan::decl_specifier`]: the
    /// qualified name ends at `end`; an identifier or a decl-specifier
    /// after it makes it a (typename-less) type.
    fn after_dependent_name(&self, end: usize) -> Tpr {
        if self.ident_at(end).is_some() {
            return Tpr::True;
        }
        let next = Scan { p: self.p, i: end, declared: self.declared.clone() };
        match next.decl_specifier(Tpr::False) {
            Tpr::True | Tpr::Error => Tpr::True,
            _ => Tpr::False,
        }
    }

    /// [`Scan::decl_specifier`] in a parameter-declaration-clause that
    /// is weighed against an initializer: a name after a dependent
    /// qualifier that is neither a type nor an expression for certain
    /// (`T::m`, `X<T>::m` followed by `)` or `,`) is `Ambiguous` and sets
    /// `invalid` -- it is a declaration only with a missing `typename`
    /// (clang's `InvalidAsDeclSpec`, the tie-break in
    /// [`Scan::function_declarator_at`]).
    ///
    /// An undeclared unqualified name not followed by `(`, `::` or `<`
    /// (`void a(u1);`) is `Ambiguous` without setting `invalid`: it is
    /// neither a valid expression nor a type, so the declaration parser
    /// takes it and reports "unknown type name" (clang's `ANK_Unresolved`
    /// in `isCXXDeclarationSpecifier`, which returns `Ambiguous` when
    /// `InvalidAsDeclaration` is tracked).
    fn param_decl_specifier(&self, invalid: Option<&mut bool>) -> Tpr {
        let r = self.decl_specifier(Tpr::False);
        if r != Tpr::False {
            return r;
        }
        let Some(invalid) = invalid else { return r };
        match self.p.scan_name(self.i) {
            Some((end, NameClass::Dependent, _)) if end > self.i + 1 => {
                *invalid = true;
                Tpr::Ambiguous
            }
            Some((end, NameClass::Undeclared, _))
                if end > self.i + 1 && self.p.unresolved_scope_name(self.i) =>
            {
                Tpr::Ambiguous
            }
            Some((end, NameClass::Undeclared, sym))
                if end == self.i + 1
                    && !self.declared.contains(&sym)
                    && !matches!(
                        self.punct_at(end),
                        Some(Punct::LParen | Punct::ColonColon | Punct::Less)
                    ) =>
            {
                Tpr::Ambiguous
            }
            _ => r,
        }
    }

    /// A name after a dependent qualifier that contains a template-id
    /// (`X<T>::m`, `T::template U<int>::m`): at the start of a statement
    /// or condition it is an expression whatever follows it (clang
    /// annotates it as a dependent non-type name before disambiguating);
    /// a missing `typename` there is not assumed.
    fn dependent_template_id_name(&self) -> bool {
        let Some((end, NameClass::Dependent)) = self.name_here() else { return false };
        // A parsed-ahead `X<T>::` (an unknown specialization stays the
        // class template).
        if let Some(Ok(q)) = self.p.decltype_scope_qual_at(self.i)
            && self.p.s.ast.decls.get(q).kind() == DeclKind::ClassTemplate
        {
            return true;
        }
        (self.i..end)
            .any(|j| self.punct_at(j) == Some(Punct::Less) || self.kw_at(j) == Some(Kw::Template))
    }

    /// A `decltype(e)::name` whose qualifier was not parsed ahead (a
    /// position other than the current token, e.g. after the `(` of a
    /// possible cast): its scope is unknown, so the name is classified
    /// by what follows it (WP-119, spec/parse.md §2.1), unless the
    /// operand is a variable whose class is known without parsing it
    /// ([`Parser::peek_decltype_scope_name`], WP-843). `kw` is the index
    /// of `decltype`, `cc` that of the `::`.
    fn unparsed_decltype_scope(&self, kw: usize, cc: usize, braced: Tpr) -> Tpr {
        match self.p.peek_decltype_scope_name(kw, cc) {
            Some((end, NameClass::Type(..))) => return self.after_simple_type(end, braced),
            Some((_, NameClass::Value(_))) => return Tpr::False,
            _ => {}
        }
        let Some(end) = self.p.skip_qualified_rest(cc) else { return Tpr::Error };
        let after = |n: usize| self.punct_at(end + n);
        match self.punct_at(end) {
            None if self.ident_at(end).is_some() => Tpr::True,
            Some(Punct::Star | Punct::Amp | Punct::AmpAmp)
                if matches!(after(1), Some(Punct::RParen | Punct::Comma | Punct::Greater)) =>
            {
                Tpr::True
            }
            Some(Punct::RParen)
                if self.ident_at(end + 1).is_some()
                    || matches!(after(1), Some(Punct::LParen))
                    || matches!(
                        self.p.tok_at(end + 1).kind,
                        TokKind::PpNumber | TokKind::CharLit(_) | TokKind::StringLit(_)
                    ) =>
            {
                Tpr::True
            }
            _ => Tpr::False,
        }
    }

    /// clang's `isCXXDeclarationSpecifierAType`.
    fn decl_specifier_is_type(&self) -> bool {
        match self.kw() {
            Some(
                Kw::Const
                | Kw::Volatile
                | Kw::Restrict
                | Kw::Nonnull
                | Kw::Nullable
                | Kw::NullUnspecified
                | Kw::NullableResult,
            ) => false,
            Some(Kw::Atomic) if self.punct_at(self.i + 1) != Some(Punct::LParen) => false,
            Some(k) => crate::decl::is_type_specifier_kw(k),
            None => {
                self.p.transform_at(self.i).is_some()
                    || matches!(self.name_here(), Some((_, NameClass::Type(..))))
            }
        }
    }

    /// clang's `TryConsumeDeclarationSpecifier`: skips one
    /// simple-type-specifier (or other decl-specifier).
    fn consume_decl_specifier(&mut self) -> Tpr {
        if self.p.transform_at(self.i).is_some() {
            self.i += 1;
            if !self.skip_group_here() {
                return Tpr::Error;
            }
            return Tpr::Ambiguous;
        }
        match self.kw() {
            Some(Kw::Atomic) if self.punct_at(self.i + 1) != Some(Punct::LParen) => self.i += 1,
            Some(
                k @ (Kw::Decltype
                | Kw::Typeof
                | Kw::TypeofUnqual
                | Kw::Atomic
                | Kw::BitInt
                | Kw::ExtInt),
            ) => {
                self.i += 1;
                if !self.at(Punct::LParen) || !self.skip_group_here() {
                    return Tpr::Error;
                }
                if k == Kw::Decltype && self.at(Punct::ColonColon) {
                    // `decltype(e)::name` (WP-119).
                    match self.p.skip_qualified_rest(self.i) {
                        Some(end) => self.i = end,
                        None => return Tpr::Error,
                    }
                }
            }
            Some(Kw::Alignas | Kw::Attribute) => {
                if !self.skip_attributes() {
                    return Tpr::Error;
                }
            }
            Some(Kw::Typename) if self.p.splice_end(self.i + 1).is_some() => {
                self.i = self.p.splice_spec_end(self.i + 1).unwrap_or(self.i + 1);
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
                // An undeclared template-id taken as a type
                // ([`Scan::undeclared_template_id_before_ptr_operator`]).
                Some((end, NameClass::Undeclared, _))
                    if end == self.i + 1 && self.punct_at(end) == Some(Punct::Less) =>
                {
                    match self.p.skip_template_args(end) {
                        Some((close, false)) => self.i = close + 1,
                        _ => return Tpr::Error,
                    }
                }
                Some((end, _, _)) => self.i = end,
                None => return Tpr::Error,
            },
        }
        Tpr::Ambiguous
    }

    /// clang's non-tentative `isDeclarationSpecifier`: a decl-specifier
    /// keyword or a type name at the current token. A class template's
    /// injected-class-name in its own scope is a type name there
    /// ([temp.local]/1), so `void(CT)` is a function type, not `void`
    /// with a parenthesized declarator `CT` ([dcl.ambig.res]/3).
    fn is_decl_specifier_start(&self) -> bool {
        match self.kw() {
            Some(k) => is_decl_specifier_kw(k),
            None => {
                self.p.transform_at(self.i).is_some()
                    || (self.ident_at(self.i).is_some() || self.at(Punct::ColonColon))
                        && !matches!(self.name_here(), Some((_, NameClass::Undeclared)))
                        && self.p.names_type_at(self.i)
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
                if matches!(
                    self.kw(),
                    Some(
                        Kw::Const
                            | Kw::Volatile
                            | Kw::Restrict
                            | Kw::Nonnull
                            | Kw::Nullable
                            | Kw::NullUnspecified
                            | Kw::NullableResult
                    )
                ) {
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

    /// The index after `nested-name-specifier *` (a pointer-to-member
    /// operator, [dcl.mptr]/1) at the current token: `N::*`, `::N::C::*`,
    /// and template-id components `X<T>::*`, `X<T>::Y::*`, `::X<int>::*`,
    /// `T::template Y<U>::*` (clang annotates the scope first). A `<`
    /// opens template arguments only after a name that classifies as a
    /// template in its qualifier's scope or after `template`
    /// ([temp.names]/3), so `x<1>::*` with a variable `x` is not one.
    fn member_pointer_end(&self) -> Option<usize> {
        let mut i = self.i;
        // The lookup context of the next component: `Some(q)` known
        // (`None` = unqualified), `None` once it can no longer be
        // classified (a component that names no scope).
        let mut ctx: Option<Option<DeclId>> = Some(None);
        let mut any = false;
        if let Some(q) = self.p.decltype_scope_qual_at(i) {
            // `decltype(e)::` parsed ahead (WP-119).
            ctx = q.ok().map(Some);
            any = true;
            i += 1;
        } else if let Some(cc) = self.p.decltype_scope_end(i) {
            // `decltype(e)::` not yet parsed: its scope is unknown.
            ctx = None;
            any = true;
            i = cc + 1;
        } else if self.punct_at(i) == Some(Punct::ColonColon) {
            ctx = Some(Some(self.p.s.ast.tu));
            i += 1;
        }
        loop {
            if any && self.punct_at(i) == Some(Punct::Star) {
                return Some(i + 1);
            }
            let template_kw = i > self.i && self.kw_at(i) == Some(Kw::Template);
            if template_kw {
                i += 1;
            }
            let name = self.ident_at(i)?;
            if !matches!(self.punct_at(i + 1), Some(Punct::Less | Punct::ColonColon)) {
                return None;
            }
            let class = ctx.map(|q| {
                if self.punct_at(i + 1) == Some(Punct::ColonColon) {
                    self.p.s.classify_nested_name(name, q)
                } else {
                    self.p.s.classify_name(name, q)
                }
            });
            let mut end = i + 1;
            if self.punct_at(end) == Some(Punct::Less)
                && (template_kw || class.is_some_and(is_template_class))
            {
                let (close, split) = self.p.skip_template_args(end)?;
                if split {
                    return None;
                }
                end = close + 1;
            }
            if self.punct_at(end) != Some(Punct::ColonColon) {
                return None;
            }
            ctx = match class {
                Some(
                    NameClass::Namespace(d) | NameClass::Type(_, d) | NameClass::TypeTemplate(d),
                ) => Some(Some(d)),
                // `T::x::`: still dependent.
                Some(NameClass::Dependent) => ctx,
                _ => None,
            };
            i = end + 1;
            any = true;
        }
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
        if !self.p.stack_ok() {
            return Tpr::Error;
        }
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
            } else if self.at(Punct::LSquare) {
                self.bracket_declarator()
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

    /// `[ constant-expression? ]` after a declarator (clang's
    /// `TryParseBracketDeclarator`), also at a `[[`: a constant-expression
    /// cannot begin with `{` and holds no top-level `,`, while a
    /// subscript's operand can (`T(a)[{1}]`, `T(a)[1, 2]` are
    /// expressions). So `T(n) [[maybe_unused]];` is a declaration, whose
    /// misplaced attribute the declaration parser reports.
    fn bracket_declarator(&mut self) -> Tpr {
        let mut i = self.i + 1;
        if self.punct_at(i) == Some(Punct::LBrace) {
            return Tpr::False;
        }
        loop {
            match self.punct_at(i) {
                Some(Punct::RSquare) => {
                    self.i = i + 1;
                    return Tpr::Ambiguous;
                }
                Some(Punct::Comma) => return Tpr::False,
                Some(Punct::LParen | Punct::LSquare | Punct::LBrace) => match self.skip_group(i) {
                    Some(j) => i = j,
                    None => return Tpr::Error,
                },
                Some(Punct::Semi | Punct::RParen | Punct::RBrace) => return Tpr::Error,
                _ if self.p.tok_at(i).kind == TokKind::Eof => return Tpr::Error,
                _ => i += 1,
            }
        }
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
        let mut r = self.parameter_declaration_clause(None, false);
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
    /// `versus_template_arg`: disambiguating the rest of a saved default
    /// argument after a `,` inside `<...>` from more template arguments;
    /// a decl-specifier is then no proof, and a parameter must have `=`.
    fn parameter_declaration_clause(
        &mut self,
        mut invalid: Option<&mut bool>,
        versus_template_arg: bool,
    ) -> Tpr {
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
            // An explicit object parameter ([dcl.fct]/3): `this` followed
            // by a decl-specifier is never an expression.
            if self.kw() == Some(Kw::This) {
                let t = Scan { p: self.p, i: self.i + 1, declared: self.declared.clone() };
                if matches!(t.decl_specifier(Tpr::False), Tpr::True | Tpr::Ambiguous) {
                    return Tpr::True;
                }
            }
            // A parameter's initializer needs `=`: `T{` is not one.
            let mut r = self.param_decl_specifier(invalid.as_deref_mut());
            if r != Tpr::Ambiguous && !(versus_template_arg && r == Tpr::True) {
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
                r = self.param_decl_specifier(invalid.as_deref_mut());
                if r == Tpr::Error || (r == Tpr::True && !versus_template_arg) {
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
            // A later parameter of a first declaration needs a default
            // argument too, so `=` decides it.
            if versus_template_arg {
                return if self.at(Punct::Equal) { Tpr::True } else { Tpr::False };
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
        let mut r = s.parameter_declaration_clause(Some(&mut invalid), false);
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
                    Some(
                        Kw::Const
                            | Kw::Volatile
                            | Kw::Restrict
                            | Kw::Throw
                            | Kw::Noexcept
                            | Kw::Try
                    )
                ) || virt;
                if decisive {
                    r = Tpr::True;
                } else if invalid {
                    // The absence of `typename` breaks the tie: an
                    // initializer (clang's `InvalidAsDeclaration`).
                    r = Tpr::False;
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

    /// An undeclared unqualified name followed by `*`, `&` or `&&`.
    fn undeclared_before_ptr_operator(&self) -> bool {
        self.ident_at(self.i).is_some()
            && matches!(self.name_here(), Some((end, NameClass::Undeclared)) if end == self.i + 1)
            && matches!(self.punct_at(self.i + 1), Some(Punct::Star | Punct::Amp | Punct::AmpAmp))
    }

    /// An undeclared unqualified template-id followed by a ptr-operator or
    /// a cv-qualifier (`w<int> *p;`, `w<int> const *q;`): clang's
    /// `TNK_Undeclared_template`, `Ambiguous` in `isCXXDeclarationSpecifier`
    /// until the declarator decides (`w<int> & i + 1;` is an expression).
    /// WP-372.
    fn undeclared_template_id_before_ptr_operator(&self) -> bool {
        if self.ident_at(self.i).is_none()
            || !matches!(self.name_here(), Some((end, NameClass::Undeclared)) if end == self.i + 1)
            || self.punct_at(self.i + 1) != Some(Punct::Less)
        {
            return false;
        }
        let Some((close, false)) = self.p.skip_template_args(self.i + 1) else { return false };
        matches!(self.punct_at(close + 1), Some(Punct::Star | Punct::Amp | Punct::AmpAmp))
            || matches!(self.kw_at(close + 1), Some(Kw::Const | Kw::Volatile))
    }

    /// An undeclared unqualified name followed by a type-specifier keyword.
    fn undeclared_before_type_keyword(&self) -> bool {
        self.ident_at(self.i).is_some()
            && matches!(self.name_here(), Some((end, NameClass::Undeclared)) if end == self.i + 1)
            && self.kw_at(self.i + 1).is_some_and(crate::decl::is_type_specifier_kw)
    }

    /// clang's `isCXXSimpleDeclaration`.
    fn is_simple_declaration(&mut self, allow_for_range: bool) -> bool {
        if self.dependent_template_id_name() {
            return false;
        }
        // `S::name(e);`, `S::name *g;` with `name` not found in the
        // non-dependent scope `S`: clang's `ANK_Unresolved`, ambiguous
        // because `isCXXSimpleDeclaration` tracks `InvalidAsDeclaration`;
        // the declarator decides.
        let r = if self.p.unresolved_scope_name(self.i) {
            // clang's `TryParseSimpleDeclaration` annotates a name after
            // it that is a non-type (or undeclared before `(`,
            // `NC_UndeclaredNonType`), which no declarator accepts:
            // `S::pn y(e);` and `S::pn yv;` are expressions.
            let end = self.p.scan_name(self.i).map_or(self.i, |n| n.0);
            let non_type = match self.p.scan_name(end) {
                Some((e2, NameClass::Undeclared, sym)) => {
                    e2 == end + 1
                        && !self.declared.contains(&sym)
                        && self.punct_at(e2) == Some(Punct::LParen)
                }
                Some((e2, NameClass::Value(_) | NameClass::FunctionTemplate(_), _)) => {
                    e2 == end + 1
                }
                _ => false,
            };
            if non_type {
                return false;
            }
            Tpr::Ambiguous
        } else if self.undeclared_before_type_keyword() {
            // `restrict int z;`: no expression continues a name with a
            // type-specifier keyword; the declaration parser reports
            // "unknown type name" (clang's `ParseImplicitInt`, WP-371).
            return true;
        } else if self.undeclared_before_ptr_operator() {
            // An undeclared unqualified name followed by a ptr-operator
            // (`u5 &r;`, `u5 *p;`): clang's `ANK_Unresolved`, ambiguous
            // here; the declarator decides (`u5 & r + 1;` is an
            // expression), and the declaration parser reports "unknown
            // type name" (WP-318).
            Tpr::Ambiguous
        } else if self.undeclared_template_id_before_ptr_operator() {
            // The same for an undeclared template-id; the declaration
            // parser reports "no template named" (WP-372).
            Tpr::Ambiguous
        } else {
            self.decl_specifier(Tpr::False)
        };
        if r != Tpr::Ambiguous {
            return r != Tpr::False;
        }
        // Declarations take precedence over expressions; errors are left
        // to the declaration parser.
        self.simple_declaration(allow_for_range) != Tpr::False
    }
}

impl Parser<'_, '_> {
    /// `Q::name ...` at the start of a statement, where the qualifier
    /// `Q` failed and was diagnosed (a class template specialization
    /// that is undefined or failed to instantiate, [temp.inst]/2): the
    /// name is an error name (clang's `ClassifyName` `NC_Error`, so
    /// `isCXXDeclarationSpecifier` is `Error` whatever follows:
    /// `Q::name *g;` declares nothing more), and the statement is
    /// skipped without further diagnostics.
    pub(crate) fn at_failed_scope_declaration(&mut self) -> bool {
        self.annotate_decltype_scope_here();
        let i = self.pos();
        self.decltype_scope_qual_at(i) == Some(Err(()))
            && self.ident_of(self.tok_at(i + 1)).is_some()
    }

    /// [stmt.ambig]: whether the statement at the current token (after
    /// any attributes) is a declaration-statement (clang's
    /// `isCXXDeclarationStatement`).
    pub(crate) fn is_cxx_declaration_statement(&mut self, allow_for_range: bool) -> bool {
        self.annotate_decltype_scope_here();
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
    pub(crate) fn is_cxx_condition_declaration(&mut self) -> bool {
        self.annotate_decltype_scope_here();
        let mut s = Scan::new(self, self.pos());
        if !s.skip_attributes() {
            return true;
        }
        if s.dependent_template_id_name() {
            return false;
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
        // C++11 [stmt.pre]/1: a braced-init-list initializes it too.
        s.at(Punct::Equal)
            || (s.p.std >= Std::Cxx11 && s.at(Punct::LBrace))
            || s.at_cxx11_attr()
            || matches!(s.kw(), Some(Kw::Asm | Kw::Attribute))
    }

    /// [dcl.ambig.res]/2-3: whether a type-id starts at the current token
    /// (clang's `isCXXTypeId`). Returns (is a type-id, the parse was
    /// ambiguous -- it could also have been an expression).
    #[allow(dead_code)] // the entry point for WP-069 (template arguments)
    pub(crate) fn is_cxx_type_id(&mut self, ctx: TypeIdContext) -> (bool, bool) {
        self.annotate_decltype_scope_here();
        self.is_cxx_type_id_at(self.pos(), ctx)
    }

    /// Parses a `decltype(e)::` at the current token ahead, as clang
    /// annotates the scope before disambiguating (WP-119): the scanner
    /// then classifies the name after it.
    /// Also parses ahead a class template specialization qualifier in
    /// which the lookahead cannot find the name
    /// ([`Parser::annotate_template_scope`]).
    pub(crate) fn annotate_decltype_scope_here(&mut self) {
        if self.decltype_scope_end(self.pos()).is_some() {
            self.annotate_decltype_scope();
        } else if self.template_scope_end(self.pos()).is_some() {
            self.annotate_template_scope();
        }
    }

    /// clang's `ConsumeAndStoreInitializer` at a `,` inside what may be a
    /// template argument list of a saved default argument (`nsdmi` false)
    /// or default member initializer: whether the tokens after the comma
    /// form a parameter-declaration-clause whose parameters all have
    /// initializers, or an init-declarator-list followed by `;` -- then
    /// the comma ends the initializer.
    pub(crate) fn comma_ends_saved_initializer(&self, nsdmi: bool) -> bool {
        let mut s = Scan::new(self, self.pos() + 1);
        let r = if nsdmi {
            let r = s.init_declarator_list(false);
            if r == Tpr::Ambiguous && !s.at(Punct::Semi) { Tpr::False } else { r }
        } else {
            s.parameter_declaration_clause(None, true)
        };
        !matches!(r, Tpr::False | Tpr::Error)
    }

    /// [`Parser::is_cxx_type_id`] at token index `i`.
    pub(crate) fn is_cxx_type_id_at(&self, i: usize, ctx: TypeIdContext) -> (bool, bool) {
        Scan::new(self, i).type_id(ctx)
    }

    /// Whether a declarator may start at token `i` (clang's
    /// `TryParseDeclarator` without an abstract declarator not giving
    /// `False`): `(v)`, `(*p)[2]`, `(N::x)` may, `(1)` and `(int)` may
    /// not.
    pub(crate) fn declarator_may_start_at(&self, i: usize) -> bool {
        Scan::new(self, i).declarator(false, true, false, false) != Tpr::False
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
