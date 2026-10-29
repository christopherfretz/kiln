//! Declarations ([dcl], [namespace], [class.mem]; spec/parse.md §4).

use kiln_ast::attr::AttrKind;
use kiln_ast::decl::{EnumData, InitStyle};
use kiln_ast::expr::ExprKind;
use kiln_ast::strings::StringEncoding;
use kiln_ast::types::{ExcSpec, Quals, TagKeyword};
use kiln_ast::{AttrId, DeclId, ExprId, QualType};
use kiln_diag::DiagId;
use kiln_lex::{Punct, TokKind};
use kiln_sema::declspec::SpecError;
use kiln_sema::{
    Chunk, DeclSpec, Declarator, DeclaratorContext, Designator, InitItem, InitList, InitValue,
    Initializer, NameClass, ParamInfo, Sign, StorageSpec, TagUse, TypeSpecKind, Width,
};
use kiln_source::symbol::Symbol;
use kiln_source::{Loc, Span};

use crate::keywords::Kw;
use crate::parser::Parser;

/// True for a keyword that can begin a type-specifier-seq ([dcl.type]).
pub(crate) fn is_type_specifier_kw(k: Kw) -> bool {
    matches!(
        k,
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
            | Kw::Auto
            | Kw::Const
            | Kw::Volatile
            | Kw::Restrict
            | Kw::Struct
            | Kw::Class
            | Kw::Union
            | Kw::Enum
            | Kw::Typename
            | Kw::Decltype
            | Kw::Typeof
    )
}

/// True for a keyword that can begin a decl-specifier-seq ([dcl.spec]).
pub(crate) fn is_decl_specifier_kw(k: Kw) -> bool {
    is_type_specifier_kw(k)
        || matches!(
            k,
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
                | Kw::Alignas
                | Kw::Attribute
                | Kw::Declspec
                | Kw::Extension
        )
}

impl Parser<'_, '_> {
    // -- translation unit -----------------------------------------------------------

    /// translation-unit ([basic.link]): declarations to end of file.
    pub fn parse_translation_unit(&mut self) {
        while !self.at_eof() {
            self.parse_external_declaration();
        }
    }

    /// A declaration at namespace scope ([dcl.pre] declaration).
    pub fn parse_external_declaration(&mut self) {
        let loc = self.loc();
        match (Self::punct_of(self.tok()), self.kw()) {
            (Some(Punct::Semi), _) => {
                self.bump();
            }
            (Some(Punct::RBrace), _) => {
                self.error_here(DiagId::parse_extraneous_closing_brace);
                self.bump();
            }
            (_, Some(Kw::Namespace)) => self.parse_namespace(),
            (_, Some(Kw::Inline)) if self.kw_of(self.peek(1)) == Some(Kw::Namespace) => {
                self.parse_namespace();
            }
            (_, Some(Kw::Using)) => {
                self.parse_using();
            }
            (_, Some(Kw::StaticAssert)) => {
                self.parse_static_assert();
            }
            (_, Some(Kw::Extern))
                if matches!(
                    self.peek(1).kind,
                    TokKind::StringLit(_) | TokKind::RawStringLit(_)
                ) =>
            {
                self.parse_linkage_spec();
            }
            (_, Some(Kw::Template)) => {
                self.unsupported("templates", loc);
                self.skip_unsupported_declaration();
            }
            (_, Some(Kw::Export)) => {
                self.unsupported("modules", loc);
                self.skip_unsupported_declaration();
            }
            (_, Some(Kw::Asm)) => {
                self.unsupported("asm declarations", loc);
                self.skip_unsupported_declaration();
            }
            (Some(Punct::LSquare), _) if self.at_attributed_using() => {
                self.parse_attributed_using();
            }
            _ => {
                self.parse_simple_declaration(DeclaratorContext::File, true);
            }
        }
    }

    /// Skips a declaration kiln does not parse: to a `;`, or past a
    /// top-level `{ ... }` (and a `;` right after it).
    pub(crate) fn skip_unsupported_declaration(&mut self) {
        loop {
            match Self::punct_of(self.tok()) {
                _ if self.at_eof() => return,
                Some(Punct::Semi) => {
                    self.bump();
                    return;
                }
                Some(Punct::LBrace) => {
                    self.bump();
                    self.skip_until(&[Punct::RBrace], false, true);
                    self.eat(Punct::Semi);
                    return;
                }
                Some(Punct::RBrace) => return,
                Some(Punct::LParen) => {
                    self.bump();
                    self.skip_until(&[Punct::RParen], false, true);
                }
                Some(Punct::LSquare) => {
                    self.bump();
                    self.skip_until(&[Punct::RSquare], false, true);
                }
                _ => {
                    self.bump();
                }
            }
        }
    }

    // -- namespaces, linkage, using, static_assert ----------------------------------------

    /// `[inline] namespace [A::B::]N { ... }` ([namespace.def]).
    fn parse_namespace(&mut self) {
        let inline = self.eat_kw(Kw::Inline).is_some();
        let ns_loc = self.bump().span.lo;
        self.skip_attributes();
        if let Some(name) = self.ident()
            && self.peek_is(1, Punct::Equal)
        {
            let _ = ns_loc;
            let nl = self.bump().span.lo;
            self.bump();
            let _ = self.parse_namespace_alias(name, nl);
            return;
        }
        let mut names = Vec::new();
        if let Some(n) = self.ident() {
            let l = self.bump().span.lo;
            names.push((n, l, inline));
            while self.at(Punct::ColonColon) {
                self.bump();
                let inl = self.eat_kw(Kw::Inline).is_some();
                let Some(n) = self.ident() else {
                    self.error_here(DiagId::parse_expected_namespace_name);
                    self.skip_unsupported_declaration();
                    return;
                };
                let l = self.bump().span.lo;
                names.push((n, l, inl));
            }
        }
        self.skip_attributes();
        let Some(open) = self.expect(Punct::LBrace) else {
            self.skip_unsupported_declaration();
            return;
        };
        let mut opened = Vec::new();
        if names.is_empty() {
            opened.push(self.s.act_on_start_namespace(None, inline, ns_loc));
        }
        for (n, l, inl) in names {
            opened.push(self.s.act_on_start_namespace(Some((n, l)), inl, ns_loc));
        }
        while !self.at(Punct::RBrace) && !self.at_eof() {
            self.parse_external_declaration();
        }
        self.expect_closing(Punct::RBrace, Punct::LBrace, open);
        for ns in opened.into_iter().rev() {
            self.s.act_on_finish_namespace(ns);
        }
    }

    /// `extern "lang" declaration` / `extern "lang" { ... }` ([dcl.link]).
    fn parse_linkage_spec(&mut self) {
        let loc = self.bump().span.lo;
        let lang_tok = self.bump();
        let lang = self.unevaluated_text(&lang_tok);
        let ls = self.s.act_on_start_linkage_spec(&lang, loc, lang_tok.span.lo);
        if let Some(open) = self.eat(Punct::LBrace) {
            while !self.at(Punct::RBrace) && !self.at_eof() {
                self.parse_external_declaration();
            }
            self.expect_closing(Punct::RBrace, Punct::LBrace, open);
        } else {
            self.parse_external_declaration();
        }
        self.s.act_on_finish_linkage_spec(ls);
    }

    /// The rest of `namespace name = [qual] target;` after the `=`
    /// ([namespace.alias]).
    pub(crate) fn parse_namespace_alias(&mut self, name: Symbol, name_loc: Loc) -> Option<DeclId> {
        let Ok(qual) = self.parse_nested_name_nns() else {
            self.skip_to_semi();
            return None;
        };
        let Some(target) = self.ident() else {
            self.error_here(DiagId::parse_expected_namespace_name);
            self.skip_to_semi();
            return None;
        };
        let tl = self.bump().span.lo;
        let d = self.s.act_on_namespace_alias(name, name_loc, qual, target, tl);
        self.expect_semi_after("namespace name");
        d
    }

    /// `using namespace N;`, `using X = T;`, `using enum E;` and
    /// using-declarations ([namespace.udir], [dcl.typedef],
    /// [enum.udecl], [namespace.udecl]). Returns the declarations.
    pub(crate) fn parse_using(&mut self) -> Vec<DeclId> {
        self.parse_using_attrs(&[])
    }

    /// [`Parser::parse_using`] with the attribute-specifier-seq written
    /// before `using` (`leading`).
    pub(crate) fn parse_using_attrs(&mut self, leading: &[AttrId]) -> Vec<DeclId> {
        let loc = self.bump().span.lo;
        if self.eat_kw(Kw::Namespace).is_some() {
            let Ok(qual) = self.parse_nested_name() else {
                self.skip_to_semi();
                return Vec::new();
            };
            let Some(n) = self.ident() else {
                self.error_here(DiagId::parse_expected_namespace_name);
                self.skip_to_semi();
                return Vec::new();
            };
            let nl = self.bump().span.lo;
            let NameClass::Namespace(ns) = self.s.classify_name(n, qual) else {
                self.s.error(DiagId::parse_expected_namespace_name, nl);
                self.skip_to_semi();
                return Vec::new();
            };
            let d = self.s.act_on_using_directive(ns, loc);
            self.expect_semi_after("namespace name");
            return vec![d];
        }
        if let Some(name) = self.ident()
            && (self.peek_is(1, Punct::Equal) || self.peek_is(1, Punct::LSquare))
        {
            let nl = self.bump().span.lo;
            let mut attrs = Vec::new();
            self.parse_attributes(&mut attrs);
            if self.expect(Punct::Equal).is_none() {
                self.skip_to_semi();
                return Vec::new();
            }
            let Some(ty) = self.parse_type_id() else {
                self.skip_to_semi();
                return Vec::new();
            };
            let d = self.s.act_on_alias_decl(name, nl, ty, &attrs);
            self.expect_semi_after("alias declaration");
            return vec![d];
        }
        if self.eat_kw(Kw::Enum).is_some() {
            return self.parse_using_enum();
        }
        let if_exists = self.has_using_if_exists(leading);
        self.parse_using_declaration(loc, if_exists)
    }

    fn has_using_if_exists(&self, attrs: &[AttrId]) -> bool {
        attrs.iter().any(|&a| self.s.ast.attrs.get(a).kind == AttrKind::USING_IF_EXISTS)
    }

    /// `using enum [qual] name;` after `enum` ([enum.udecl]).
    fn parse_using_enum(&mut self) -> Vec<DeclId> {
        let Ok(qual) = self.parse_nested_name_nns() else {
            self.skip_to_semi();
            return Vec::new();
        };
        let Some(name) = self.ident() else {
            self.error_here(DiagId::parse_using_enum_requires_name);
            self.skip_to_semi();
            return Vec::new();
        };
        let nl = self.bump().span.lo;
        let class = self.s.classify_name(name, qual.map(|q| q.ctx));
        let d = self.s.act_on_using_enum(qual, class, name, nl);
        self.expect_semi_after("using-enum declaration");
        d.into_iter().collect()
    }

    /// `using-declarator-list ;` ([namespace.udecl]): each declarator is
    /// `[typename] nested-name-specifier unqualified-id [...]`, here
    /// optionally followed by an attribute-specifier-seq (a clang
    /// extension for `using_if_exists`).
    fn parse_using_declaration(&mut self, using_loc: Loc, leading_if_exists: bool) -> Vec<DeclId> {
        let mut out = Vec::new();
        loop {
            if let Some(l) = self.eat_kw(Kw::Typename) {
                self.unsupported("'typename' using-declarators", l);
                self.skip_to_semi();
                return out;
            }
            let Ok(qual) = self.parse_nested_name_nns() else {
                self.skip_to_semi();
                return out;
            };
            let Some(name) = self.ident() else {
                if self.at_kw(Kw::Operator) || self.at(Punct::Tilde) {
                    let l = self.loc();
                    self.unsupported("this kind of name in a using-declarator", l);
                } else {
                    self.error_here(DiagId::parse_expected_unqualified_id);
                }
                self.skip_to_semi();
                return out;
            };
            let nl = self.bump().span.lo;
            if let Some(l) = self.eat(Punct::Ellipsis) {
                self.unsupported("pack expansions in using-declarations", l);
                self.skip_to_semi();
                return out;
            }
            let mut attrs = Vec::new();
            if self.at_cxx11_attr() {
                let at = self.peek(2).span.lo;
                self.s.error(DiagId::parse_cxx11_attr_placement, at);
            }
            self.parse_attributes(&mut attrs);
            let if_exists = leading_if_exists || self.has_using_if_exists(&attrs);
            if let Some(d) = self.s.act_on_using_declarator(using_loc, qual, name, nl, if_exists) {
                out.push(d);
            }
            if self.eat(Punct::Comma).is_none() {
                break;
            }
        }
        self.expect_semi_after("using declaration");
        out
    }

    /// Expects `;` after `what` (`expected ';' after {what}`).
    pub(crate) fn expect_semi_after(&mut self, what: &str) {
        if self.eat(Punct::Semi).is_none() {
            let d =
                self.s.diag(DiagId::parse_expected_after, self.expected_loc()).arg(what).arg("';'");
            self.emit(d);
            self.skip_to_semi();
        }
    }

    /// `static_assert ( constant-expression [, string-literal] ) ;`
    /// ([dcl.pre]/10).
    pub(crate) fn parse_static_assert(&mut self) -> Option<DeclId> {
        let loc = self.bump().span.lo;
        let Some(open) = self.expect(Punct::LParen) else {
            self.skip_to_semi();
            return None;
        };
        let Some(cond) = self.parse_constant_expr() else {
            self.skip_until(&[Punct::RParen], false, true);
            self.skip_to_semi();
            return None;
        };
        let mut msg = None;
        if self.eat(Punct::Comma).is_some() {
            if self.at_string() {
                msg = self.parse_string_literal(true);
            } else {
                self.error_here(DiagId::parse_expected_string_literal);
                self.skip_until(&[Punct::RParen], false, true);
                self.skip_to_semi();
                return None;
            }
        }
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        let d = self.s.act_on_static_assert(cond, msg, loc);
        self.expect_semi_after("static_assert");
        Some(d)
    }

    // -- decl-specifiers ------------------------------------------------------------

    /// decl-specifier-seq ([dcl.spec]).
    pub(crate) fn parse_decl_specifiers(&mut self, ctx: DeclaratorContext) -> DeclSpec {
        let mut ds = DeclSpec::new(self.loc());
        loop {
            let loc = self.loc();
            if self.at_attr() || self.at_kw(Kw::Declspec) {
                let mut attrs = Vec::new();
                self.parse_attributes(&mut attrs);
                ds.attrs.extend(attrs);
                continue;
            }
            if self.at(Punct::ColonColon) || self.ident().is_some() {
                if ds.has_type_specifier() || !self.parse_type_name_spec(&mut ds, ctx) {
                    break;
                }
                continue;
            }
            let Some(k) = self.kw() else { break };
            let r: Result<(), SpecError> = match k {
                Kw::Typedef => ds.set_storage(StorageSpec::Typedef, loc),
                Kw::Static => ds.set_storage(StorageSpec::Static, loc),
                Kw::Extern => ds.set_storage(StorageSpec::Extern, loc),
                Kw::Register => ds.set_storage(StorageSpec::Register, loc),
                Kw::Mutable => ds.set_storage(StorageSpec::Mutable, loc),
                Kw::ThreadLocal => set_flag(&mut ds.thread_local, loc, "thread_local"),
                Kw::Constexpr => set_flag(&mut ds.constexpr, loc, "constexpr"),
                Kw::Constinit => set_flag(&mut ds.constinit, loc, "constinit"),
                Kw::Inline => set_flag(&mut ds.inline, loc, "inline"),
                Kw::Consteval | Kw::Virtual | Kw::Explicit | Kw::Friend => {
                    self.unsupported(&format!("'{}'", k.spelling()), loc);
                    ds.invalid = true;
                    Ok(())
                }
                Kw::Const => ds.add_cv(Quals::CONST, "const"),
                Kw::Volatile => ds.add_cv(Quals::VOLATILE, "volatile"),
                Kw::Restrict => ds.add_cv(Quals::RESTRICT, "restrict"),
                Kw::Short => ds.set_width(Width::Short, loc),
                Kw::Long => ds.set_width(Width::Long, loc),
                Kw::Signed => ds.set_sign(Sign::Signed, loc),
                Kw::Unsigned => ds.set_sign(Sign::Unsigned, loc),
                Kw::Void => ds.set_base(TypeSpecKind::Void, None, loc),
                Kw::Bool => ds.set_base(TypeSpecKind::Bool, None, loc),
                Kw::Char => ds.set_base(TypeSpecKind::Char, None, loc),
                Kw::WChar => ds.set_base(TypeSpecKind::WChar, None, loc),
                Kw::Char8 => ds.set_base(TypeSpecKind::Char8, None, loc),
                Kw::Char16 => ds.set_base(TypeSpecKind::Char16, None, loc),
                Kw::Char32 => ds.set_base(TypeSpecKind::Char32, None, loc),
                Kw::Int => ds.set_base(TypeSpecKind::Int, None, loc),
                Kw::Int128 => ds.set_base(TypeSpecKind::Int128, None, loc),
                Kw::Float => ds.set_base(TypeSpecKind::Float, None, loc),
                Kw::Double => ds.set_base(TypeSpecKind::Double, None, loc),
                Kw::Float128 => ds.set_base(TypeSpecKind::Float128, None, loc),
                Kw::Auto => ds.set_base(TypeSpecKind::Auto, None, loc),
                Kw::Struct | Kw::Class | Kw::Union | Kw::Enum => {
                    self.parse_tag_specifier(&mut ds, k);
                    continue;
                }
                Kw::Decltype => {
                    let (base, ty) = self.parse_decltype_specifier();
                    if base == TypeSpecKind::Named && ty.is_none() {
                        ds.invalid = true;
                    }
                    let ty = if base == TypeSpecKind::Named {
                        Some(ty.unwrap_or(QualType::ERROR))
                    } else {
                        None
                    };
                    if let Err(e) = ds.set_base(base, ty, loc) {
                        ds.invalid = true;
                        self.spec_error(&e, loc);
                    }
                    continue;
                }
                Kw::Typename | Kw::Typeof => {
                    self.unsupported(&format!("'{}'", k.spelling()), loc);
                    self.bump();
                    if self.eat(Punct::LParen).is_some() {
                        self.skip_until(&[Punct::RParen], false, true);
                    } else if k == Kw::Typename {
                        let _ = self.scan_name(self.pos()).map(|(end, ..)| end);
                        while self.ident().is_some() || self.at(Punct::ColonColon) {
                            self.bump();
                        }
                    }
                    ds.invalid = true;
                    if let Err(e) = ds.set_base(TypeSpecKind::Named, Some(QualType::ERROR), loc) {
                        self.spec_error(&e, loc);
                    }
                    continue;
                }
                Kw::Extension => Ok(()),
                _ => break,
            };
            self.bump();
            if let Err(e) = r {
                if e.id != DiagId::parse_duplicate_decl_spec {
                    ds.invalid = true;
                }
                self.spec_error(&e, loc);
            }
        }
        ds
    }

    /// decltype-specifier ([dcl.type.decltype]): `decltype ( expression )`
    /// or `decltype ( auto )` ([dcl.type.auto.deduct]). At `decltype`.
    /// Returns the base specifier and, for `decltype(e)`, its type (`None`
    /// after an error).
    pub(crate) fn parse_decltype_specifier(&mut self) -> (TypeSpecKind, Option<QualType>) {
        let loc = self.bump().span.lo;
        let Some(open) = self.eat(Punct::LParen) else {
            let d = self
                .s
                .diag(DiagId::parse_expected_after, self.expected_loc())
                .arg("'decltype'")
                .arg("'('");
            self.emit(d);
            return (TypeSpecKind::Named, None);
        };
        if self.at_kw(Kw::Auto) && self.peek_is(1, Punct::RParen) {
            self.bump();
            self.bump();
            return (TypeSpecKind::DecltypeAuto, None);
        }
        self.s.enter_unevaluated();
        let e = self.parse_expression();
        self.s.exit_unevaluated();
        let Some(e) = e else {
            self.skip_until(&[Punct::RParen], false, true);
            return (TypeSpecKind::Named, None);
        };
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
        (TypeSpecKind::Named, self.s.act_on_decltype(e, loc, rparen))
    }

    fn spec_error(&self, e: &SpecError, loc: Loc) {
        let mut d = self.s.diag(e.id, loc);
        for a in &e.args {
            d = d.arg(a);
        }
        self.emit(d);
    }

    /// A type-name in a decl-specifier-seq (`T`, `N::T`), or an unknown
    /// identifier that can only be one. Returns false (consuming nothing)
    /// if the tokens are not a type name.
    fn parse_type_name_spec(&mut self, ds: &mut DeclSpec, ctx: DeclaratorContext) -> bool {
        let start = self.pos();
        let loc = self.loc();
        match self.scan_name(start) {
            Some((end, NameClass::Type(ty, d), _)) => {
                while self.pos() < end {
                    self.bump();
                }
                self.s.mark_type_referenced(d);
                let kind = self.s.ast.decls.get(d).kind();
                let ty = if kiln_sema::lookup::is_tag_kind(kind) {
                    self.s.elaborated_tag_type(d, TagKeyword::None)
                } else {
                    ty
                };
                if let Err(e) = ds.set_base(TypeSpecKind::Named, Some(ty), loc) {
                    ds.invalid = true;
                    self.spec_error(&e, loc);
                }
                true
            }
            Some((end, NameClass::Undeclared, name)) if end == start + 1 => {
                let next = self.tok_at(end);
                let unknown_type = self.ident_of(next).is_some()
                    || (matches!(ctx, DeclaratorContext::File | DeclaratorContext::Member)
                        && matches!(
                            Self::punct_of(next),
                            Some(Punct::Star | Punct::Amp | Punct::AmpAmp)
                        ))
                    || (ctx == DeclaratorContext::Prototype
                        && matches!(Self::punct_of(next), Some(Punct::RParen | Punct::Comma)));
                if !unknown_type {
                    return false;
                }
                let text = self.s.ast.symbols().as_str(name).to_owned();
                let d = self.s.diag(DiagId::sema_unknown_typename, loc).arg(text);
                self.emit(d);
                self.bump();
                ds.invalid = true;
                let _ = ds.set_base(TypeSpecKind::Named, Some(QualType::ERROR), loc);
                true
            }
            _ => false,
        }
    }

    // -- class and enum specifiers ----------------------------------------------------

    /// class-specifier, enum-specifier or elaborated-type-specifier
    /// ([class.pre], [dcl.enum], [dcl.type.elab]).
    fn parse_tag_specifier(&mut self, ds: &mut DeclSpec, k: Kw) {
        let kw_loc = self.bump().span.lo;
        let keyword = match k {
            Kw::Struct => TagKeyword::Struct,
            Kw::Class => TagKeyword::Class,
            Kw::Union => TagKeyword::Union,
            _ => TagKeyword::Enum,
        };
        let mut scoped = 0;
        if keyword == TagKeyword::Enum {
            if self.eat_kw(Kw::Class).is_some() {
                scoped = EnumData::SCOPED;
            } else if self.eat_kw(Kw::Struct).is_some() {
                scoped = EnumData::SCOPED | EnumData::SCOPED_STRUCT;
            }
        }
        let mut attrs = Vec::new();
        self.parse_attributes(&mut attrs);
        let mut qualifier = None;
        if self.at(Punct::ColonColon)
            || (self.ident().is_some() && self.peek_is(1, Punct::ColonColon))
        {
            match self.parse_nested_name() {
                Ok(q) => qualifier = q,
                Err(()) => ds.invalid = true,
            }
        }
        let name = self.ident().map(|n| (n, self.bump().span.lo));
        if name.is_none() && qualifier.is_some() {
            self.error_here(DiagId::parse_expected_unqualified_id);
            ds.invalid = true;
        }
        let mut fixed = None;
        if keyword == TagKeyword::Enum && self.at(Punct::Colon) {
            self.bump();
            let base = self.parse_decl_specifiers(DeclaratorContext::TypeName);
            if base.has_type_specifier() {
                fixed = Some(self.s.type_from_declspec(&base));
            } else {
                self.error_here(DiagId::parse_expected_type);
            }
        }
        if keyword != TagKeyword::Enum && self.at(Punct::Colon) {
            self.unsupported("base classes", self.loc());
            self.skip_until(&[Punct::LBrace, Punct::Semi], false, false);
        }
        let use_ = if self.at(Punct::LBrace) {
            TagUse::Definition
        } else if self.at(Punct::Semi) && ds.storage.is_none() {
            TagUse::Declaration
        } else {
            TagUse::Reference
        };
        if name.is_none() && use_ != TagUse::Definition {
            if !ds.invalid {
                self.error_here(DiagId::parse_expected_unqualified_id);
            }
            ds.invalid = true;
            let _ = ds.set_base(TypeSpecKind::Named, Some(QualType::ERROR), kw_loc);
            return;
        }
        let d = self.s.act_on_tag(keyword, name, qualifier, use_, scoped, fixed, kw_loc);
        self.s.act_on_tag_attrs(d, &attrs);
        if use_ == TagUse::Definition {
            if keyword == TagKeyword::Enum {
                self.parse_enum_body(d);
            } else {
                self.parse_record_body(d);
            }
        }
        ds.tag = Some(d);
        ds.tag_declares = use_ != TagUse::Reference;
        ds.keyword = keyword;
        let ty = self.s.elaborated_tag_type(d, keyword);
        if let Err(e) = ds.set_base(TypeSpecKind::Named, Some(ty), kw_loc) {
            ds.invalid = true;
            self.spec_error(&e, kw_loc);
        }
    }

    /// `{ member-specification }` ([class.mem]).
    fn parse_record_body(&mut self, r: DeclId) {
        let open = self.bump().span.lo;
        self.s.act_on_start_record_definition(r);
        while !self.at(Punct::RBrace) && !self.at_eof() {
            self.parse_member_declaration(r);
        }
        self.expect_closing(Punct::RBrace, Punct::LBrace, open);
        let then_semi = self.at(Punct::Semi);
        self.s.act_on_finish_record_definition(r, then_semi);
    }

    /// True if a constructor declaration of the class `r` starts here
    /// ([class.ctor.general]/1: optional specifiers and attributes, then
    /// the class's injected-class-name and a parameter list).
    fn at_constructor_declaration(&self, r: DeclId) -> bool {
        let Some(class) = self.s.ast.decls.get(r).name().as_ident() else {
            return false;
        };
        let mut i = self.pos();
        loop {
            let t = self.tok_at(i);
            match self.kw_of(t) {
                Some(Kw::Explicit) if self.tok_at(i + 1).kind == TokKind::Punct(Punct::LParen) => {
                    i = self.skip_balanced_at(i + 1);
                }
                Some(Kw::Explicit | Kw::Constexpr | Kw::Consteval | Kw::Inline) => i += 1,
                Some(Kw::Attribute | Kw::Alignas | Kw::Declspec) => {
                    i = self.skip_balanced_at(i + 1);
                }
                _ if t.kind == TokKind::Punct(Punct::LSquare)
                    && self.tok_at(i + 1).kind == TokKind::Punct(Punct::LSquare) =>
                {
                    i = self.skip_balanced_at(i);
                }
                _ => break,
            }
        }
        self.ident_of(self.tok_at(i)) == Some(class)
            && self.tok_at(i + 1).kind == TokKind::Punct(Punct::LParen)
    }

    /// The index after the bracketed group opening at `i` (`i` itself if
    /// no bracket opens there).
    fn skip_balanced_at(&self, mut i: usize) -> usize {
        let mut depth = 0u32;
        loop {
            match Self::punct_of(self.tok_at(i)) {
                Some(Punct::LParen | Punct::LSquare | Punct::LBrace) => depth += 1,
                Some(Punct::RParen | Punct::RSquare | Punct::RBrace) => {
                    depth = depth.saturating_sub(1);
                }
                _ if self.tok_at(i).kind == TokKind::Eof => return i,
                _ => {}
            }
            i += 1;
            if depth == 0 {
                return i;
            }
        }
    }

    /// The member-declarations of class `r` that a leading token decides
    /// (empty declarations, access specifiers, `static_assert`, `using`,
    /// and the unsupported constructor, destructor, operator and template
    /// members): parses one and returns true, or returns false.
    fn parse_member_declaration_leader(&mut self, r: DeclId) -> bool {
        let loc = self.loc();
        if self.at_constructor_declaration(r) {
            // Not a field of the class's own type ([class.ctor.general]).
            self.unsupported("special member functions", loc);
            self.skip_unsupported_declaration();
            return true;
        }
        match (Self::punct_of(self.tok()), self.kw()) {
            (Some(Punct::Semi), _) => {
                self.bump();
                true
            }
            (_, Some(Kw::Public | Kw::Private | Kw::Protected)) => {
                self.unsupported("access specifiers", loc);
                self.bump();
                self.expect(Punct::Colon);
                true
            }
            (_, Some(Kw::StaticAssert)) => {
                self.parse_static_assert();
                true
            }
            (_, Some(Kw::Using)) => {
                self.parse_using();
                true
            }
            (Some(Punct::LSquare), _) if self.at_attributed_using() => {
                self.parse_attributed_using();
                true
            }
            (_, Some(Kw::Template)) => {
                self.unsupported("templates", loc);
                self.skip_unsupported_declaration();
                true
            }
            (Some(Punct::Tilde), _) | (_, Some(Kw::Operator)) => {
                self.unsupported("special member functions", loc);
                self.skip_unsupported_declaration();
                true
            }
            _ => false,
        }
    }

    /// member-declaration ([class.mem]) of the class `r`.
    fn parse_member_declaration(&mut self, r: DeclId) {
        if self.parse_member_declaration_leader(r) {
            return;
        }
        let ds = self.parse_decl_specifiers(DeclaratorContext::Member);
        if self.eat(Punct::Semi).is_some() {
            let _ = self.s.act_on_decl_spec_only(&ds);
            return;
        }
        if ds.is_empty() {
            self.error_here(DiagId::parse_expected_member_name);
            self.skip_to_semi();
            return;
        }
        let mut base_ty: Option<QualType> = None;
        loop {
            let mut d = Declarator::new(DeclaratorContext::Member, self.loc());
            if !self.at(Punct::Colon) {
                self.parse_declarator(&mut d);
            }
            // The declspec's type is formed once the first declarator is
            // known: clang reports a missing type specifier at its name.
            let base =
                *base_ty.get_or_insert_with(|| self.s.type_from_declspec_at(&ds, d.name_loc()));
            if self.eat(Punct::Colon).is_some() {
                let Some(w) = self.parse_constant_expr() else {
                    self.skip_to_semi();
                    return;
                };
                d.bit_width = Some(w);
            }
            // clang's `ParseCXXMemberDeclaratorBeforeInitializer`: a
            // malformed asm label skips to the next declarator.
            if let Ok(l) = self.parse_asm_label() {
                d.asm_label = l;
            } else {
                self.skip_until(&[Punct::Comma], true, false);
            }
            if self.at_attr() {
                self.parse_attributes(&mut d.attrs);
            }
            if d.name.is_none() && d.bit_width.is_none() {
                if !d.invalid {
                    self.error_here(DiagId::parse_expected_member_name);
                }
                self.skip_to_semi();
                return;
            }
            if d.is_function_declarator() && self.at(Punct::LBrace) {
                self.unsupported("member functions", d.name_loc());
                self.skip_unsupported_declaration();
                return;
            }
            let field = self.s.act_on_declarator(&ds, base, &d);
            if self.at(Punct::Equal) || self.at(Punct::LBrace) {
                self.unsupported("default member initializers", self.loc());
                self.skip_until(&[Punct::Comma, Punct::Semi], false, false);
            }
            let _ = field;
            if self.eat(Punct::Comma).is_none() {
                break;
            }
        }
        if self.eat(Punct::Semi).is_none() {
            self.s.error(DiagId::parse_expected_semi_decl_list, self.expected_loc());
            self.skip_until(&[Punct::RBrace], true, false);
            self.eat(Punct::Semi);
        }
    }

    /// `{ enumerator-list }` ([dcl.enum]).
    fn parse_enum_body(&mut self, e: DeclId) {
        let open = self.bump().span.lo;
        self.s.act_on_start_enum_definition(e);
        while !self.at(Punct::RBrace) && !self.at_eof() {
            let Some(name) = self.ident() else {
                let d = self.s.diag(DiagId::parse_expected, self.loc()).arg("identifier");
                self.emit(d);
                self.skip_until(&[Punct::RBrace], false, false);
                break;
            };
            let loc = self.bump().span.lo;
            let mut attrs = Vec::new();
            self.parse_attributes(&mut attrs);
            let mut init = None;
            if self.eat(Punct::Equal).is_some() {
                init = self.parse_constant_expr();
                if init.is_none() {
                    self.skip_until(&[Punct::Comma, Punct::RBrace], false, false);
                }
            }
            self.s.act_on_enumerator(name, loc, init, &attrs);
            if self.eat(Punct::Comma).is_none() {
                if !self.at(Punct::RBrace) {
                    let d = self.s.diag(DiagId::parse_expected, self.loc()).arg("',' or '}'");
                    self.emit(d);
                    self.skip_until(&[Punct::RBrace], false, false);
                }
                break;
            }
        }
        self.expect_closing(Punct::RBrace, Punct::LBrace, open);
        self.s.act_on_finish_enum_definition(e);
    }

    // -- declarators ------------------------------------------------------------------

    /// declarator / abstract-declarator ([dcl.decl]). Chunks are recorded
    /// innermost first: pointer operators after the declarator they apply
    /// to, suffixes as written.
    pub(crate) fn parse_declarator(&mut self, d: &mut Declarator) {
        let loc = self.loc();
        match Self::punct_of(self.tok()) {
            Some(Punct::Star) => {
                self.bump();
                let quals = self.parse_ptr_cv();
                self.parse_declarator(d);
                d.chunks.push(Chunk::Pointer { quals, loc });
            }
            Some(Punct::Amp) => {
                self.bump();
                self.skip_attributes();
                self.parse_declarator(d);
                d.chunks.push(Chunk::LRef { loc });
            }
            Some(Punct::AmpAmp) => {
                self.bump();
                self.skip_attributes();
                self.parse_declarator(d);
                d.chunks.push(Chunk::RRef { loc });
            }
            _ => {
                if self.at_member_pointer() {
                    self.unsupported("pointers to members", loc);
                    while !self.at(Punct::Star) {
                        self.bump();
                    }
                    self.bump();
                    d.invalid = true;
                    self.parse_declarator(d);
                    return;
                }
                self.parse_direct_declarator(d);
            }
        }
    }

    /// True at `N::*` (a ptr-operator naming a member pointer).
    fn at_member_pointer(&self) -> bool {
        let mut i = self.pos();
        if self.tok_at(i).kind == TokKind::Punct(Punct::ColonColon) {
            i += 1;
        }
        let mut any = false;
        while self.ident_of(self.tok_at(i)).is_some()
            && self.tok_at(i + 1).kind == TokKind::Punct(Punct::ColonColon)
        {
            i += 2;
            any = true;
        }
        any && self.tok_at(i).kind == TokKind::Punct(Punct::Star)
    }

    /// The cv-qualifiers (and attributes) after a `*`.
    fn parse_ptr_cv(&mut self) -> Quals {
        let mut q = Quals::NONE;
        loop {
            let (add, sp) = match self.kw() {
                Some(Kw::Const) => (Quals::CONST, "const"),
                Some(Kw::Volatile) => (Quals::VOLATILE, "volatile"),
                Some(Kw::Restrict) => (Quals::RESTRICT, "restrict"),
                _ if self.at_attr() => {
                    self.skip_attributes();
                    continue;
                }
                _ => return q,
            };
            let loc = self.bump().span.lo;
            if q.contains(add) {
                let d = self.s.diag(DiagId::parse_duplicate_decl_spec, loc).arg(sp);
                self.emit(d);
            }
            q |= add;
        }
    }

    /// noptr-declarator ([dcl.decl]): the declarator-id or `( declarator )`
    /// and the array and function suffixes.
    fn parse_direct_declarator(&mut self, d: &mut Declarator) {
        let abstract_ok =
            matches!(d.ctx, DeclaratorContext::Prototype | DeclaratorContext::TypeName);
        if (self.ident().is_some() || self.at(Punct::ColonColon))
            && d.ctx != DeclaratorContext::TypeName
        {
            let qual = self.parse_nested_name().unwrap_or_else(|()| {
                d.invalid = true;
                None
            });
            if let Some(n) = self.ident() {
                let l = self.bump().span.lo;
                d.name = Some((n, l));
                d.qualifier = qual;
            } else if qual.is_some() {
                self.error_here(DiagId::parse_expected_unqualified_id);
                d.invalid = true;
            }
            if self.at_cxx11_attr() {
                self.parse_attributes(&mut d.attrs);
            }
        } else if self.at(Punct::LParen) && !(abstract_ok && self.is_param_list_start(1)) {
            // Like clang's ParseParenDeclarator: grouping is in effect
            // only for the inner declarator, then restored.
            let had_grouping = d.grouping;
            d.grouping = true;
            let open = self.bump().span.lo;
            self.skip_attributes();
            self.parse_declarator(d);
            self.expect_closing(Punct::RParen, Punct::LParen, open);
            d.grouping = had_grouping;
        } else if self.at(Punct::Tilde) || self.at_kw(Kw::Operator) {
            self.unsupported("special member functions", self.loc());
            d.invalid = true;
            self.bump();
            if self.ident().is_some() {
                self.bump();
            }
        }
        loop {
            if self.at(Punct::LSquare) && !self.at_cxx11_attr() {
                let open = self.bump().span.lo;
                let mut size = None;
                if !self.at(Punct::RSquare) {
                    size = self.parse_assignment_expr();
                    if size.is_none() {
                        d.invalid = true;
                        self.skip_until(&[Punct::RSquare], false, false);
                    }
                }
                self.expect_closing(Punct::RSquare, Punct::LSquare, open);
                self.skip_attributes();
                d.chunks.push(Chunk::Array { size, loc: open });
            } else if self.at(Punct::LParen) {
                // [dcl.ambig.res]/1: after a name that could take a direct
                // initializer, the parentheses are a function declarator
                // only if they could be a parameter clause.
                let mut ambiguous = false;
                if !abstract_ok && d.direct_init && !d.grouping && d.name.is_some() {
                    let (function, amb) = self.is_cxx_function_declarator(d.name.map(|n| n.0));
                    if !function {
                        break;
                    }
                    ambiguous = amb;
                }
                self.parse_function_suffix(d, ambiguous);
            } else {
                break;
            }
        }
    }

    /// True if the token `n` ahead (after a `(` where an abstract
    /// declarator may start) begins a parameter-declaration-clause rather
    /// than grouping parentheses (clang's `ParseParenDeclarator`): `)`,
    /// `...)`, a decl-specifier or `[[`. An undeclared name followed by an
    /// identifier is taken as a misspelled type.
    pub(crate) fn is_param_list_start(&self, n: usize) -> bool {
        let t = self.peek(n);
        match t.kind {
            TokKind::Punct(Punct::RParen | Punct::Ellipsis) => true,
            TokKind::Punct(Punct::LSquare) => {
                self.peek(n + 1).kind == TokKind::Punct(Punct::LSquare)
            }
            TokKind::Punct(Punct::ColonColon) => {
                matches!(self.scan_name(self.pos() + n), Some((_, NameClass::Type(..), _)))
            }
            TokKind::Identifier(s) => match self.kw_of(t) {
                Some(k) => is_decl_specifier_kw(k),
                None => match self.scan_name(self.pos() + n) {
                    Some((_, NameClass::Type(..), _)) => true,
                    Some((end, NameClass::Undeclared, _)) => {
                        let _ = s;
                        self.ident_of(self.tok_at(end)).is_some()
                    }
                    _ => false,
                },
            },
            _ => false,
        }
    }

    /// `( parameter-declaration-clause )` ([dcl.fct]) and what may follow it.
    /// `ambiguous`: the parentheses could also have been a direct
    /// initializer ([dcl.ambig.res]/1).
    fn parse_function_suffix(&mut self, d: &mut Declarator, ambiguous: bool) {
        let open = self.bump().span.lo;
        self.s.act_on_start_prototype();
        let mut params = Vec::new();
        let mut variadic = false;
        loop {
            if self.at(Punct::RParen) || self.at_eof() {
                break;
            }
            if self.eat(Punct::Ellipsis).is_some() {
                variadic = true;
                break;
            }
            let start = self.loc();
            let ds = self.parse_decl_specifiers(DeclaratorContext::Prototype);
            if ds.is_empty() {
                let e = self.s.diag(DiagId::parse_expected, self.loc()).arg("parameter declarator");
                self.emit(e);
                self.skip_until(&[Punct::RParen], false, false);
                d.invalid = true;
                break;
            }
            let mut pd = Declarator::new(DeclaratorContext::Prototype, start);
            self.parse_declarator(&mut pd);
            if self.at_attr() {
                self.parse_attributes(&mut pd.attrs);
            }
            let mut p = self.s.act_on_param_declarator(&ds, &pd);
            if self.at(Punct::Equal) {
                self.parse_default_argument(&mut p);
            }
            p.range = (start, self.prev_end());
            self.s.act_on_prototype_param(&p);
            params.push(p);
            if self.eat(Punct::Ellipsis).is_some() {
                variadic = true;
                break;
            }
            if self.eat(Punct::Comma).is_none() {
                break;
            }
        }
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
        let (exc, trailing) = self.parse_function_qualifiers();
        self.s.act_on_end_prototype();
        let params = self.s.act_on_param_list(params);
        d.chunks.push(Chunk::Function {
            params,
            variadic,
            loc: open,
            rparen,
            exc,
            trailing,
            ambiguous,
        });
    }

    /// `= initializer-clause` after a parameter-declaration
    /// ([dcl.fct.default]/1). At the `=`.
    fn parse_default_argument(&mut self, p: &mut ParamInfo) {
        self.bump();
        // clang rejects a default argument that starts with `({` outright
        // (`err_stmt_expr_in_default_arg`), skipping to the next `,`/`)`.
        if self.at(Punct::LParen) && self.peek_is(1, Punct::LBrace) {
            self.error_here(DiagId::parse_stmt_expr_in_default_arg);
            p.invalid = true;
            self.skip_until(&[Punct::Comma, Punct::RParen], true, false);
            return;
        }
        let init = if self.at(Punct::LBrace) {
            self.parse_init_list().ok().map(Initializer::List)
        } else {
            self.parse_assignment_expr().map(Initializer::Expr)
        };
        if let Some(init) = init {
            self.s.act_on_param_default_init(p, init);
        } else {
            p.invalid = true;
            self.skip_until(&[Punct::Comma, Punct::RParen], false, false);
        }
    }

    /// cv- and ref-qualifiers, the exception specification and trailing
    /// return types after a parameter list. Only the exception
    /// specification ([except.spec]) is supported in M2; it is returned
    /// (`ExcSpec::None` when absent). A second specification ends the
    /// declarator (clang then expects a function body).
    fn parse_function_qualifiers(&mut self) -> (ExcSpec, Option<(QualType, Loc)>) {
        let mut exc = None;
        let mut trailing = None;
        loop {
            let loc = self.loc();
            match self.kw() {
                Some(Kw::Const | Kw::Volatile) => {
                    self.unsupported("member function qualifiers", loc);
                    self.bump();
                }
                Some(Kw::Noexcept | Kw::Throw) if exc.is_none() => {
                    exc = Some(self.parse_exception_spec());
                }
                _ if self.at(Punct::Arrow) && trailing.is_none() => {
                    self.bump();
                    // A failed type-id was diagnosed: the declarator is
                    // invalid.
                    let ty = self.parse_type_id().unwrap_or(QualType::ERROR);
                    trailing = Some((ty, loc));
                    return (exc.unwrap_or_default(), trailing);
                }
                _ => return (exc.unwrap_or_default(), trailing),
            }
        }
    }

    /// noexcept-specifier ([except.spec]/1): `noexcept`,
    /// `noexcept ( constant-expression )`, or the dynamic `throw ( )` /
    /// `throw ( type-id-list )` (C++17 removed the latter; it is
    /// diagnosed by Sema). At the `noexcept` or `throw` keyword.
    fn parse_exception_spec(&mut self) -> ExcSpec {
        let is_throw = self.at_kw(Kw::Throw);
        let loc = self.bump().span.lo;
        if is_throw {
            let Some(open) = self.eat(Punct::LParen) else {
                let d = self
                    .s
                    .diag(DiagId::parse_expected_after, self.expected_loc())
                    .arg("'throw'")
                    .arg("'('");
                self.emit(d);
                return ExcSpec::None;
            };
            let mut types = Vec::new();
            let mut ok = true;
            while !self.at(Punct::RParen) && !self.at_eof() {
                let Some(t) = self.parse_type_id() else {
                    ok = false;
                    self.skip_until(&[Punct::RParen], false, false);
                    break;
                };
                types.push(t);
                if self.at(Punct::Ellipsis) {
                    self.unsupported("pack expansions", self.loc());
                    self.bump();
                }
                if self.eat(Punct::Comma).is_none() {
                    break;
                }
            }
            let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
            return self.s.act_on_dynamic_exception_spec(&types, ok, loc, rparen);
        }
        let Some(open) = self.eat(Punct::LParen) else {
            return ExcSpec::BasicNoexcept;
        };
        let start = self.loc();
        let Some(e) = self.parse_constant_expr() else {
            self.skip_until(&[Punct::RParen], false, true);
            return ExcSpec::None;
        };
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        self.s.act_on_noexcept_spec(e, loc, start)
    }

    /// type-id ([dcl.name]).
    pub(crate) fn parse_type_id(&mut self) -> Option<QualType> {
        let ds = self.parse_decl_specifiers(DeclaratorContext::TypeName);
        if !ds.has_type_specifier() {
            self.error_here(DiagId::parse_expected_type);
            return None;
        }
        let mut d = Declarator::new(DeclaratorContext::TypeName, self.loc());
        self.parse_declarator(&mut d);
        let base = self.s.type_from_declspec(&ds);
        Some(self.s.type_for_declarator_at(base, ds.type_loc.unwrap_or(ds.loc), &d))
    }

    /// True if a type-id starts at token index `i`.
    pub(crate) fn is_type_id_start(&self, i: usize) -> bool {
        let t = self.tok_at(i);
        match self.kw_of(t) {
            Some(k) => is_type_specifier_kw(k),
            None => {
                (self.ident_of(t).is_some() || t.kind == TokKind::Punct(Punct::ColonColon))
                    && matches!(self.scan_name(i), Some((_, NameClass::Type(..), _)))
            }
        }
    }

    // -- simple declarations ------------------------------------------------------------

    /// Whether a declarator in `ctx` with storage class `storage` may take
    /// a parenthesized initializer, so `T x(...)` is tentatively parsed
    /// as a possible variable (clang's `mayHaveDirectInit`).
    fn may_have_direct_init(ctx: DeclaratorContext, storage: Option<(StorageSpec, Loc)>) -> bool {
        match storage {
            Some((StorageSpec::Typedef, _)) => false,
            Some((StorageSpec::Extern, _)) => ctx == DeclaratorContext::File,
            _ => matches!(
                ctx,
                DeclaratorContext::File | DeclaratorContext::Block | DeclaratorContext::ForInit
            ),
        }
    }

    /// simple-declaration or function-definition ([dcl.pre], [dcl.fct.def]):
    /// returns the declarations made (including a tag the decl-specifiers
    /// declare), for a declaration statement.
    pub(crate) fn parse_simple_declaration(
        &mut self,
        ctx: DeclaratorContext,
        allow_fn_def: bool,
    ) -> Vec<DeclId> {
        let ds = self.parse_decl_specifiers(ctx);
        let mut decls = Vec::new();
        if ds.tag_declares
            && let Some(t) = ds.tag
        {
            decls.push(t);
        }
        if self.eat(Punct::Semi).is_some() {
            decls.extend(self.s.act_on_decl_spec_only(&ds));
            return decls;
        }
        if ds.is_empty() {
            self.error_here(DiagId::parse_expected_unqualified_id);
            self.skip_to_semi();
            return decls;
        }
        let mut base_ty: Option<QualType> = None;
        let direct_init = Self::may_have_direct_init(ctx, ds.storage);
        loop {
            let mut d = Declarator::new(ctx, self.loc());
            d.direct_init = direct_init;
            self.parse_declarator(&mut d);
            // The declspec's type is formed once the first declarator is
            // known: clang reports a missing type specifier at its name.
            let base =
                *base_ty.get_or_insert_with(|| self.s.type_from_declspec_at(&ds, d.name_loc()));
            // GNU asm label (clang's `ParseAsmAttributesAfterDeclarator`):
            // a malformed one drops the declarator.
            let Ok(label) = self.parse_asm_label() else {
                self.skip_until(&[], true, false);
                break;
            };
            d.asm_label = label;
            if self.at_attr() {
                self.parse_attributes(&mut d.attrs);
            }
            if d.name.is_none() {
                if !d.invalid {
                    self.error_here(DiagId::parse_expected_unqualified_id);
                }
                self.skip_to_semi();
                return decls;
            }
            // A function definition has no asm label: after one, `{` is
            // an initializer (clang's order of checks).
            if d.is_function_declarator() && d.asm_label.is_none() && self.at(Punct::LBrace) {
                if !allow_fn_def {
                    self.error_here(DiagId::parse_function_definition_not_allowed);
                    self.skip_unsupported_declaration();
                    return decls;
                }
                match self.s.act_on_declarator(&ds, base, &d) {
                    Some(f) if self.s.ast.decls.function(f).is_some() => {
                        self.parse_function_body(f);
                        decls.push(f);
                    }
                    // An invalid declarator sema did not make a function
                    // (an unknown return type, say): the error is
                    // reported; skip the body.
                    Some(_) => {
                        self.bump();
                        self.skip_until(&[Punct::RBrace], false, true);
                    }
                    None => self.skip_unsupported_declaration(),
                }
                return decls;
            }
            d.init_style = match Self::punct_of(self.tok()) {
                Some(Punct::Equal) => Some(InitStyle::C),
                Some(Punct::LParen) => Some(InitStyle::Call),
                Some(Punct::LBrace) => Some(InitStyle::List),
                _ => None,
            };
            let v = self.s.act_on_declarator(&ds, base, &d);
            match self.parse_initializer(d.init_style) {
                Ok(init) => match (v, init) {
                    (Some(v), Some(i)) => self.s.act_on_init(v, i),
                    (Some(v), None) => self.s.act_on_uninit(v),
                    _ => {}
                },
                // A malformed braced-init-list without `=` was skipped
                // through its `}` (clang's `ParseBraceInitializer`); the
                // declaration continues.
                Err(()) if d.init_style == Some(InitStyle::List) => {}
                Err(()) => {
                    decls.extend(v);
                    self.skip_to_semi();
                    return decls;
                }
            }
            decls.extend(v);
            if self.eat(Punct::Comma).is_none() {
                break;
            }
        }
        if self.eat(Punct::Semi).is_none() {
            let id = if ctx == DeclaratorContext::File {
                DiagId::parse_expected_semi_toplevel
            } else {
                DiagId::parse_expected_semi_declaration
            };
            self.s.error(id, self.expected_loc());
            if !self.is_decl_spec_start() {
                self.skip_to_semi();
            }
        }
        decls
    }

    /// A GNU asm label after a declarator (`asm("name")`, also spelled
    /// `__asm`/`__asm__`; clang's `ParseSimpleAsm(ForAsmLabel)`): `Ok(None)`
    /// when there is none, `Err` (reported) when it is malformed. The
    /// adjacent string literals concatenate ([lex.string]/7); the result
    /// must be an ordinary, non-empty literal.
    pub(crate) fn parse_asm_label(&mut self) -> Result<Option<(ExprId, Span)>, ()> {
        if !self.at_kw(Kw::Asm) {
            return Ok(None);
        }
        self.bump();
        let Some(lparen) = self.eat(Punct::LParen) else {
            let d = self.s.diag(DiagId::parse_expected_after, self.loc()).arg("'asm'").arg("'('");
            self.emit(d);
            return Err(());
        };
        let start = self.loc();
        let label = if self.at_string() {
            self.parse_string_literal(false).and_then(|e| {
                let span = Span::new(start, self.prev_end());
                let ast = &self.s.ast;
                if ast.exprs.get(e).kind() != ExprKind::StringLiteral {
                    return None;
                }
                let lit = ast.exprs.string_lit(e);
                let which = match ast.strings.encoding(lit) {
                    StringEncoding::Ordinary if ast.strings.unit_count(lit) == 0 => "2",
                    StringEncoding::Ordinary => return Some((e, span)),
                    StringEncoding::Wide16 | StringEncoding::Wide32 => "1",
                    _ => "0",
                };
                let d =
                    self.s.diag(DiagId::parse_asm_string_kind, self.loc()).arg(which).range(span);
                self.emit(d);
                None
            })
        } else {
            let d = self.s.diag(DiagId::parse_expected_string_literal_in, self.loc()).arg("'asm'");
            self.emit(d);
            None
        };
        let Some(l) = label else {
            if self.skip_until(&[Punct::RParen], true, false) {
                self.bump();
            }
            return Err(());
        };
        self.expect_closing(Punct::RParen, Punct::LParen, lparen);
        Ok(Some(l))
    }

    /// True if the current token can start a decl-specifier-seq.
    pub(crate) fn is_decl_spec_start(&self) -> bool {
        match self.kw() {
            Some(k) => is_decl_specifier_kw(k),
            None => self.is_type_id_start(self.pos()),
        }
    }

    /// The initializer after a declarator, per the style peeked before the
    /// declaration was created. `Err` after a syntax error.
    pub(crate) fn parse_initializer(
        &mut self,
        style: Option<InitStyle>,
    ) -> Result<Option<Initializer>, ()> {
        match style {
            None => Ok(None),
            Some(InitStyle::C) => {
                self.bump();
                if self.at_kw(Kw::Delete) || self.at_kw(Kw::Default) {
                    self.unsupported("deleted and defaulted functions", self.loc());
                    self.bump();
                    return Ok(None);
                }
                if self.at(Punct::LBrace) {
                    return Ok(Some(Initializer::List(self.parse_init_list()?)));
                }
                Ok(Some(Initializer::Expr(self.parse_assignment_expr().ok_or(())?)))
            }
            Some(InitStyle::Call | InitStyle::ParenList) => {
                let l = self.bump().span.lo;
                let mut v = Vec::new();
                if !self.at(Punct::RParen) {
                    loop {
                        let Some(e) = self.parse_assignment_expr() else {
                            self.skip_until(&[Punct::RParen], false, true);
                            return Err(());
                        };
                        v.push(e);
                        if self.eat(Punct::Comma).is_none() {
                            break;
                        }
                    }
                }
                let r = self.expect_closing(Punct::RParen, Punct::LParen, l);
                Ok(Some(Initializer::Paren(v, l, r)))
            }
            Some(InitStyle::List) => Ok(Some(Initializer::List(self.parse_init_list()?))),
        }
    }

    /// braced-init-list ([dcl.init.general]), with C++20 designators.
    pub(crate) fn parse_init_list(&mut self) -> Result<InitList, ()> {
        let lbrace = self.bump().span.lo;
        let mut items = Vec::new();
        while !self.at(Punct::RBrace) && !self.at_eof() {
            let mut designator = Vec::new();
            loop {
                if self.at(Punct::Period) {
                    self.bump();
                    let Some(n) = self.ident() else {
                        self.error_here(DiagId::parse_expected_unqualified_id);
                        self.skip_until(&[Punct::RBrace], false, true);
                        return Err(());
                    };
                    let l = self.bump().span.lo;
                    designator.push(Designator::Field(n, l));
                } else if self.at(Punct::LSquare) && !designator.is_empty() {
                    let l = self.bump().span.lo;
                    let Some(e) = self.parse_constant_expr() else {
                        self.skip_until(&[Punct::RBrace], false, true);
                        return Err(());
                    };
                    self.expect_closing(Punct::RSquare, Punct::LSquare, l);
                    designator.push(Designator::Index(e, l));
                } else {
                    break;
                }
            }
            if !designator.is_empty()
                && !self.at(Punct::LBrace)
                && self.expect(Punct::Equal).is_none()
            {
                self.skip_until(&[Punct::RBrace], false, true);
                return Err(());
            }
            let value = if self.at(Punct::LBrace) {
                let Ok(l) = self.parse_init_list() else {
                    // The inner list skipped through its `}`; skip
                    // through ours too (clang's `ParseBraceInitializer`).
                    self.skip_until(&[Punct::RBrace], false, true);
                    return Err(());
                };
                InitValue::List(l)
            } else {
                let Some(e) = self.parse_assignment_expr() else {
                    self.skip_until(&[Punct::RBrace], false, true);
                    return Err(());
                };
                InitValue::Expr(e)
            };
            if let Some(l) = self.eat(Punct::Ellipsis) {
                self.unsupported("pack expansions", l);
            }
            items.push(InitItem { designator, value });
            if self.eat(Punct::Comma).is_none() {
                break;
            }
        }
        let rbrace = self.expect_closing(Punct::RBrace, Punct::LBrace, lbrace);
        Ok(InitList { items, lbrace, rbrace })
    }

    /// function-body ([dcl.fct.def.general]): the compound statement shares
    /// the function scope sema opens.
    fn parse_function_body(&mut self, f: DeclId) {
        self.s.act_on_start_function_def(f);
        let (body, rbrace) = self.parse_compound(false);
        self.s.act_on_finish_function_def(f, body, rbrace);
    }
}

/// Records a function or variable specifier written at most once.
fn set_flag(slot: &mut Option<Loc>, loc: Loc, spelling: &str) -> Result<(), SpecError> {
    if slot.is_some() {
        return Err(SpecError {
            id: DiagId::parse_duplicate_decl_spec,
            args: vec![spelling.into()],
        });
    }
    *slot = Some(loc);
    Ok(())
}
