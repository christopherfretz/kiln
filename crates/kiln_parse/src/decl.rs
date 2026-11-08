//! Declarations ([dcl], [namespace], [class.mem]; spec/parse.md §4).

use kiln_ast::attr::{AttrKind, AttrSyntax};
use kiln_ast::decl::{EnumData, InitStyle};
use kiln_ast::expr::ExprKind;
use kiln_ast::names::DeclName;
use kiln_ast::strings::StringEncoding;
use kiln_ast::types::{ExcSpec, Quals, RefQual, TagKeyword, TypeAttrKind, UnaryTransformKind};
use kiln_ast::{AttrId, DeclId, ExprId, QualType};
use kiln_diag::{DiagId, FixIt};
use kiln_lex::{Punct, Std, TokFlags, TokKind};
use kiln_sema::declspec::SpecError;
use kiln_sema::templ::auto::AutoGroup;
use kiln_sema::{
    BindingId, Chunk, DeclSpec, Declarator, DeclaratorContext, Decomposition, DefaultArg,
    Designator, InitItem, InitList, InitValue, Initializer, NameClass, NullSpec, ParamInfo, Sign,
    SpecialName, StorageSpec, TagUse, TypeSpecKind, Width,
};
use kiln_sema::{ContractDeclKind, DeclSite};
use kiln_source::symbol::Symbol;
use kiln_source::{Loc, Span};

use crate::class::LateToks;
use crate::fp_pragma::FpAt;
use crate::keywords::Kw;
use crate::parser::{ParamTypename, Parser};
use crate::templ::TemplateDeclKind;

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
            | Kw::BitInt
            | Kw::ExtInt
            | Kw::Float128
            | Kw::Float16
            | Kw::Float32
            | Kw::Float64
            | Kw::StdFloat128
            | Kw::Float32x
            | Kw::Float64x
            | Kw::Bf16
            | Kw::Fp16
            | Kw::Complex
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
            | Kw::TypeofUnqual
            | Kw::Atomic
            | Kw::Nonnull
            | Kw::Nullable
            | Kw::NullUnspecified
            | Kw::NullableResult
    )
}

/// The clang nullability qualifier keyword `k` names (spec/parse.md).
pub(crate) fn nullability_kw(k: Kw) -> Option<TypeAttrKind> {
    Some(match k {
        Kw::Nonnull => TypeAttrKind::Nonnull,
        Kw::Nullable => TypeAttrKind::Nullable,
        Kw::NullUnspecified => TypeAttrKind::NullUnspecified,
        Kw::NullableResult => TypeAttrKind::NullableResult,
        _ => return None,
    })
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
                | Kw::Noreturn
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
            let tu = self.s.ast.tu;
            self.s.ast.decls.end_top_level_decl(tu);
        }
        self.take_fp_pragmas(FpAt::File);
        self.flush_fp_pragmas();
        self.module_flush();
        // [temp.point]/7: the end of the translation unit is a point of
        // instantiation of every pending definition.
        self.s.end_of_tu_instantiations();
        // Then clang's `ActOnEndOfTranslationUnit` checks (delegating
        // constructor cycles), after the pending instantiations as there.
        self.s.act_on_end_of_translation_unit();
    }

    /// A declaration at namespace scope ([dcl.pre] declaration).
    pub fn parse_external_declaration(&mut self) {
        if self.mods.is_some() {
            // Modules: directive events first, then the cross-unit
            // deduplication (spec/modules.md §4, §6).
            let crate::modules::BeforeDecl::Parse(start) = self.module_before_decl() else {
                return;
            };
            self.parse_external_declaration_inner();
            self.s.act_on_end_abbreviated();
            self.module_after_decl(start);
            return;
        }
        self.parse_external_declaration_inner();
        // An abbreviated function template's invented head ends with its
        // declaration ([dcl.fct]/22).
        self.s.act_on_end_abbreviated();
    }

    /// `__extension__` before an alias- or using-declaration (libc++
    /// `__extension__ using __calc_type = unsigned __int128;`) or a
    /// template declaration (libstdc++ `__extension__ template<> struct
    /// __is_integer_nonstrict<__int128>`, WP-237): the GNU
    /// keyword only silences extension diagnostics, so it is dropped and
    /// the declaration parsed as usual (clang skips it in
    /// `ParseExternalDeclaration`, `ParseCXXClassMemberDeclaration` and
    /// `ParseCompoundStatementBody`). Elsewhere it stays a decl-specifier
    /// or the unary operator.
    pub(crate) fn skip_extension_before_using(&mut self) {
        self.skip_extension_before(&[Kw::Using, Kw::Template]);
    }

    /// Drops the `__extension__` keywords before a declaration starting
    /// with one of `kws`.
    fn skip_extension_before(&mut self, kws: &[Kw]) {
        let mut n = 0;
        while self.kw_of(self.peek(n)) == Some(Kw::Extension) {
            n += 1;
        }
        if n > 0 && self.kw_of(self.peek(n)).is_some_and(|k| kws.contains(&k)) {
            for _ in 0..n {
                self.bump();
            }
        }
    }

    fn parse_external_declaration_inner(&mut self) {
        if !self.stack_ok() {
            return;
        }
        // At namespace scope also before an asm-declaration (libstdc++
        // `__extension__ asm(...)`); not in a class or block, as clang.
        self.skip_extension_before(&[Kw::Using, Kw::Template, Kw::Asm]);
        if self.take_fp_pragmas(FpAt::File) && (self.at(Punct::RBrace) || self.at_eof()) {
            return;
        }
        self.discard_attrs_before_unattributed_decl();
        let loc = self.loc();
        // A loop-hint pragma at namespace scope (clang's annotation token
        // fails `ParseExternalDeclaration` and the declaration after it is
        // skipped through its `;`).
        if let Some(h) = self.take_loop_hints() {
            self.emit(self.s.diag(DiagId::parse_expected_unqualified_id, h[0].start));
            self.skip_until(&[Punct::Semi], false, true);
            return;
        }
        if self.at_gnu_attrs_before_linkage_spec(0) {
            // `__attribute__((section("s"))) extern "C" int f(int);`: clang
            // parses the GNU attributes as decl-specifiers, then the
            // linkage specification with that `DeclSpec` (`ParseLinkage`).
            // A C++11 attribute-specifier-seq among them is reported and
            // dropped (`ProhibitAttributes`).
            let mut attrs = Vec::new();
            while !self.at_kw(Kw::Extern) {
                let before = self.pos();
                self.parse_gnu_attr_seq(&mut attrs);
                self.diagnose_and_skip_cxx11_attrs();
                if self.pos() == before {
                    break;
                }
            }
            if !self.at_kw(Kw::Extern) {
                self.parse_simple_declaration(DeclaratorContext::File, true);
                return;
            }
            self.parse_linkage_spec_with(attrs);
            return;
        }
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
            _ if self.at_consteval_block() => {
                self.parse_consteval_block();
            }
            (_, Some(Kw::Extern))
                if matches!(
                    self.peek(1).kind,
                    TokKind::StringLit(_) | TokKind::RawStringLit(_)
                ) =>
            {
                self.parse_linkage_spec_with(Vec::new());
            }
            // `template [: r :] < args > :: name x;` declares `x` (a
            // splice-scope-specifier, P2996; as GCC 16.1).
            (_, Some(Kw::Template)) if !self.peek_is(1, Punct::SpliceOpen) => {
                self.parse_template_declaration(DeclaratorContext::File);
            }
            (_, Some(Kw::Extern)) if self.kw_of(self.peek(1)) == Some(Kw::Template) => {
                self.parse_template_declaration(DeclaratorContext::File);
            }
            (_, Some(Kw::Export)) if self.std >= Std::Cxx20 => {
                self.parse_export_declaration();
            }
            (_, Some(Kw::Export)) => {
                self.unsupported("modules", loc);
                self.skip_unsupported_declaration();
            }
            (_, Some(Kw::Asm)) => self.parse_file_scope_asm(),
            _ if self.at_attributed_using() => {
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
        let mut attrs = Vec::new();
        self.parse_attributes(&mut attrs);
        if let Some(name) = self.ident()
            && self.peek_is(1, Punct::Equal)
        {
            let _ = ns_loc;
            self.s.reject_annotations_in(&attrs, Some("a namespace alias"));
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
        if self.s.clang_compat >= 22 && !names.is_empty() {
            // clang 22: only GNU attributes may follow the name; a C++11
            // specifier-seq there is reported and dropped ([namespace.def]
            // puts it before the name; spec/parse.md "Attributes").
            self.diagnose_and_skip_cxx11_attrs();
            self.parse_gnu_attr_seq(&mut attrs);
            self.diagnose_and_skip_cxx11_attrs();
        }
        self.parse_attributes(&mut attrs);
        if !self.at(Punct::LBrace) {
            let _ = self.expect(Punct::LBrace);
            self.skip_unsupported_declaration();
            return;
        }
        // The namespaces (and a visibility attribute's pragma-stack
        // marker) begin before `{` is consumed: consuming it lexes the
        // next token, firing a `#pragma GCC visibility` right after it.
        let open = self.loc();
        let mut opened = Vec::new();
        if names.is_empty() {
            let ns = self.s.act_on_start_namespace(None, inline, ns_loc, open);
            if self.mods.is_some() {
                self.s.module_note_anon_namespace(ns, open);
            }
            opened.push(ns);
        }
        for (n, l, inl) in names {
            opened.push(self.s.act_on_start_namespace(Some((n, l)), inl, ns_loc, open));
        }
        if let Some(&ns) = opened.last() {
            self.s.act_on_namespace_attrs(ns, &attrs);
        }
        self.bump();
        while !self.at(Punct::RBrace) && !self.at_eof() {
            self.parse_external_declaration();
        }
        self.take_fp_pragmas(FpAt::File);
        // The namespace's visibility scope ends at its `}`, before a
        // `#pragma GCC visibility` right after it is lexed.
        let end = self.loc();
        for &ns in opened.iter().rev() {
            self.s.pop_namespace_visibility(ns, end);
        }
        self.expect_closing(Punct::RBrace, Punct::LBrace, open);
        for ns in opened.into_iter().rev() {
            self.s.act_on_finish_namespace(ns);
        }
    }

    /// Whether the tokens from offset `n` are GNU `__attribute__((...))`
    /// specifiers (possibly mixed with C++11 attribute-specifiers after
    /// the first) followed by `extern "lang"` ([dcl.link]).
    pub(crate) fn at_gnu_attrs_before_linkage_spec(&self, n: usize) -> bool {
        if self.kw_of(self.peek(n)) != Some(Kw::Attribute) {
            return false;
        }
        let mut j = n;
        loop {
            let k = self.past_gnu_attrs(j);
            let Some(i) = self.after_cxx11_attrs_at(self.pos() + k) else { return false };
            if i - self.pos() == j {
                break;
            }
            j = i - self.pos();
        }
        self.kw_of(self.peek(j)) == Some(Kw::Extern)
            && matches!(self.peek(j + 1).kind, TokKind::StringLit(_) | TokKind::RawStringLit(_))
    }

    /// `extern "lang" declaration` / `extern "lang" { ... }` ([dcl.link]).
    /// `gnu` are the GNU attributes written before `extern`: clang's
    /// `ParseLinkage` continues the `DeclSpec` that holds them into the
    /// declaration without braces, so they appertain to it; before a
    /// braced body they are dropped without a diagnostic.
    fn parse_linkage_spec_with(&mut self, gnu: Vec<AttrId>) {
        let loc = self.bump().span.lo;
        let lang_tok = self.bump();
        let lang = self.unevaluated_text(&lang_tok);
        let ls = self.s.act_on_start_linkage_spec(&lang, loc, lang_tok.span.lo);
        if let Some(open) = self.eat(Punct::LBrace) {
            while !self.at(Punct::RBrace) && !self.at_eof() {
                self.parse_external_declaration();
            }
            self.take_fp_pragmas(FpAt::File);
            self.expect_closing(Punct::RBrace, Punct::LBrace, open);
        } else {
            self.s.act_on_linkage_spec_braceless(ls);
            let outer = std::mem::replace(&mut self.lead_decl_attrs, gnu);
            self.parse_external_declaration();
            // A declaration without decl-specifiers (a namespace, a
            // template) does not take them.
            self.lead_decl_attrs = outer;
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
            // `using namespace [: r :];` ([namespace.udir]/1 with a
            // splice-specifier).
            if self.at_splice() && self.decltype_scope_end(self.pos()).is_none() {
                let ns =
                    self.parse_splice_operand().and_then(|e| self.s.act_on_splice_namespace(e));
                let Some(ns) = ns else {
                    self.skip_to_semi();
                    return Vec::new();
                };
                let d = self.s.act_on_using_directive(ns, loc);
                self.s.act_on_decl_attrs(d, leading);
                self.expect_semi_after("namespace name");
                return vec![d];
            }
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
            // [namespace.udir]/1: only namespace names are considered, so
            // a function of the same name does not hide the namespace.
            let ns = match self.s.classify_name(n, qual) {
                NameClass::Namespace(ns) => Some(ns),
                _ => self.s.lookup_namespace_name(qual, n).map(|d| self.s.ast.resolve_alias(d)),
            };
            let Some(ns) = ns else {
                self.s.error(DiagId::parse_expected_namespace_name, nl);
                self.skip_to_semi();
                return Vec::new();
            };
            self.s.act_on_namespace_name_use(ns, nl);
            let d = self.s.act_on_using_directive(ns, loc);
            self.s.act_on_decl_attrs(d, leading);
            self.expect_semi_after("namespace name");
            return vec![d];
        }
        if let Some(name) = self.ident()
            && (self.peek_is(1, Punct::Equal)
                || self.peek_is(1, Punct::LSquare)
                || matches!(self.kw_of(self.peek(1)), Some(Kw::Attribute | Kw::Alignas)))
        {
            let nl = self.bump().span.lo;
            let mut attrs = Vec::new();
            self.parse_attributes(&mut attrs);
            if self.expect(Punct::Equal).is_none() {
                self.skip_to_semi();
                return Vec::new();
            }
            let tl = self.loc();
            self.typename_pack_index = None;
            let own = attrs.len();
            let Some(ty) = self.parse_type_id_into(Some(&mut attrs), DeclSite::Alias) else {
                self.skip_to_semi();
                return Vec::new();
            };
            let d = self.s.act_on_alias_decl(name, nl, ty, &attrs, own, tl);
            self.s.note_alias_decl_loc(d, loc);
            // clang reports a `typename T...[i]` type-id as unterminated
            // (spec/parse.md, typename-specifiers).
            if let Some((_, end)) = self.typename_pack_index.take()
                && end == self.expected_loc()
            {
                let e = self.s.diag(DiagId::parse_expected_after, end);
                self.emit(e.arg("alias declaration").arg("';'"));
            }
            self.expect_semi_after("alias declaration");
            return vec![d];
        }
        if self.eat_kw(Kw::Enum).is_some() {
            return self.parse_using_enum();
        }
        let if_exists = self.has_using_if_exists(leading);
        let ds = self.parse_using_declaration(loc, if_exists);
        if !leading.is_empty() {
            for &d in &ds {
                self.s.act_on_decl_attrs(d, leading);
            }
        }
        ds
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
            let typename = self.eat_kw(Kw::Typename).is_some();
            let Ok(qual) = self.parse_nested_name_nns() else {
                self.skip_to_semi();
                return out;
            };
            // [namespace.udecl]/1: any unqualified-id but a template-id;
            // an operator- or conversion-function-id names the base's
            // operators (`using B::operator=;`).
            let nl = self.loc();
            let name = if self.at_kw(Kw::Operator) {
                let Some(n) = self.parse_operator_function_name() else {
                    self.skip_to_semi();
                    return out;
                };
                n
            } else if let Some(name) = self.ident() {
                self.bump();
                DeclName::ident(name)
            } else {
                if self.at(Punct::Tilde) {
                    self.unsupported("a destructor name in a using-declarator", nl);
                } else {
                    self.error_here(DiagId::parse_expected_unqualified_id);
                }
                self.skip_to_semi();
                return out;
            };
            let ellipsis = self.eat(Punct::Ellipsis);
            let mut attrs = Vec::new();
            if self.at_cxx11_attr() {
                let at = self.peek(2).span.lo;
                self.s.error(DiagId::parse_cxx11_attr_placement, at);
            }
            self.parse_attributes(&mut attrs);
            let if_exists = leading_if_exists || self.has_using_if_exists(&attrs);
            self.s.check_using_declarator_access(qual, name, nl);
            let d = if typename {
                self.s.act_on_using_typename_declarator(
                    using_loc, qual, name, nl, if_exists, ellipsis,
                )
            } else {
                self.s.act_on_using_declarator(using_loc, qual, name, nl, if_exists, ellipsis)
            };
            if let Some(d) = d {
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

    /// `static_assert ( constant-expression [, static_assert-message] ) ;`
    /// ([dcl.pre]/10). The message is an unevaluated string literal when
    /// every token up to the `)` is a string literal without a
    /// ud-suffix, else a constant-expression (P2741; clang 22's
    /// lookahead, a C++26 extension before C++26).
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
            let mut as_expr = false;
            for i in 0.. {
                let t = self.peek(i);
                if t.kind == TokKind::Punct(Punct::RParen) {
                    break;
                }
                if !matches!(t.kind, TokKind::StringLit(_) | TokKind::RawStringLit(_))
                    || t.flags.contains(TokFlags::UD_SUFFIX)
                {
                    as_expr = true;
                    break;
                }
            }
            if as_expr {
                if self.std < Std::Cxx26 {
                    let d = self.s.diag(DiagId::parse_ext_static_assert_user_message, self.loc());
                    self.emit(d);
                }
                msg = self.parse_constant_expr();
                if msg.is_none() {
                    self.skip_until(&[Punct::RParen], false, true);
                    self.skip_to_semi();
                    return None;
                }
            } else if self.at_string() {
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
        self.parse_decl_specifiers_from(ctx, None)
    }

    /// [`Self::parse_decl_specifiers`] for a later friend-type-specifier
    /// of a friend-type-specifier-list ([class.friend]/3, P2893): the
    /// `friend` at `friend_loc` applies to it too (clang's `SetFriendSpec`
    /// before `ParseDeclarationSpecifiers`).
    #[allow(clippy::too_many_lines, reason = "one arm per decl-specifier")]
    pub(crate) fn parse_decl_specifiers_from(
        &mut self,
        ctx: DeclaratorContext,
        friend_loc: Option<Loc>,
    ) -> DeclSpec {
        let mut ds = DeclSpec::new(self.name_begin());
        ds.friend = friend_loc;
        ds.attrs = std::mem::take(&mut self.lead_decl_attrs);
        ds.lead_cxx11 = self.stmt_attrs_loc.take().filter(|_| !ds.attrs.is_empty());
        // The spelling of the thread storage class (`_Thread_local`).
        let mut thread_spelling: Option<String> = None;
        // The end of the leading attribute-specifier-seq: attributes there
        // appertain to the declaration ([dcl.pre]/5).
        let mut lead_end = self.pos();
        // A C++11 attribute-specifier-seq (`[[...]]`, `alignas`) after a
        // decl-specifier: (its position, the token index after it, its
        // attributes).
        let mut trailing: Option<(Loc, usize, Vec<AttrId>)> = None;
        loop {
            let loc = self.loc();
            if let Some((at, end, _)) = &trailing
                && *end != self.pos()
            {
                // More decl-specifiers follow (clang `ProhibitAttributes`
                // in `ParseDeclarationSpecifiers`).
                self.s.error(DiagId::parse_attributes_not_allowed, *at);
                trailing = None;
            }
            if self.at_attr() || self.at_kw(Kw::Declspec) {
                let lead = self.pos() == lead_end;
                let cxx11 = self.at_cxx11_attr() || self.at_kw(Kw::Alignas);
                let mut attrs = Vec::new();
                self.parse_attributes(&mut attrs);
                if lead {
                    if cxx11 && ds.lead_cxx11.is_none() {
                        ds.lead_cxx11 = Some(loc);
                    }
                    if cxx11 && ds.loc == loc {
                        // The decl-specifier-seq starts after its leading
                        // attributes (clang's `DeclSpec` range).
                        ds.loc = self.loc();
                    }
                    lead_end = self.pos();
                    ds.attrs.extend(attrs);
                } else if cxx11 {
                    trailing = Some((loc, self.pos(), attrs));
                } else {
                    ds.attrs.extend(attrs);
                }
                continue;
            }
            if !ds.has_type_specifier()
                && let Some(k) = self.transform_at(self.pos())
            {
                self.parse_unary_transform_into(&mut ds, k, loc);
                continue;
            }
            if !ds.has_type_specifier()
                && ds.auto_constraint.is_none()
                && (self.at(Punct::ColonColon) || self.ident().is_some())
                && self.at_constrained_placeholder()
            {
                ds.constraint_loc = self.loc();
                match self.parse_concept_name() {
                    Some(c) => ds.auto_constraint = Some(c),
                    None => ds.invalid = true,
                }
                continue;
            }
            if !ds.has_type_specifier()
                && self.template_scope_end(self.pos()).is_some()
                && !self.at_special_declarator(ctx)
            {
                // `X<A>::name` / `td::name` whose scope the lookahead
                // cannot look into: parse the scope first. Not
                // `X<T*>::X(`, a constructor of a partial specialization:
                // the declarator parses its scope, entering it.
                self.annotate_template_scope();
            }
            if self.decltype_scope_end(self.pos()).is_some() || self.at_decltype_scope() {
                // `decltype(e)::name` (WP-119); after a type specifier it
                // starts a member-pointer declarator.
                if ds.has_type_specifier() {
                    break;
                }
                if !self.annotate_decltype_scope() || !self.decltype_scope_ok() {
                    self.skip_failed_decltype_scope();
                    ds.invalid = true;
                    let _ = ds.set_base(TypeSpecKind::Named, Some(QualType::ERROR), loc);
                    continue;
                }
            }
            if self.at(Punct::ColonColon) || self.ident().is_some() {
                if ds.has_type_specifier()
                    || self.at_special_declarator(ctx)
                    || !self.parse_type_name_spec(&mut ds, ctx)
                {
                    break;
                }
                continue;
            }
            // A splice-type-specifier without `typename` in a type-only
            // context ([dcl.type.splice]/1, [temp.res.general]/4); a
            // splice followed by `::` was annotated as a scope above.
            if self.at_splice() {
                if ds.has_type_specifier() || self.typename_required(ctx) {
                    break;
                }
                let ty = self.parse_splice_type();
                ds.invalid |= ty.is_none();
                let _ = ds.set_base(TypeSpecKind::Named, Some(ty.unwrap_or(QualType::ERROR)), loc);
                continue;
            }
            let Some(k) = self.kw() else { break };
            let r: Result<(), SpecError> = match k {
                Kw::Typedef => ds.set_storage(StorageSpec::Typedef, loc),
                Kw::Static => ds.set_storage(StorageSpec::Static, loc),
                Kw::Extern => ds.set_storage(StorageSpec::Extern, loc),
                Kw::Register => ds.set_storage(StorageSpec::Register, loc),
                Kw::Mutable => ds.set_storage(StorageSpec::Mutable, loc),
                Kw::ThreadLocal => {
                    let sp = self.spelling_of(&self.tok());
                    let r = set_flag(&mut ds.thread_local, loc, "thread_local");
                    if r.is_ok() {
                        ds.thread_kind = if sp == "thread_local" {
                            kiln_ast::decl::TlsKind::Dynamic
                        } else {
                            kiln_ast::decl::TlsKind::Static
                        };
                    }
                    thread_spelling = Some(sp);
                    r
                }
                Kw::Constexpr | Kw::Consteval | Kw::Constinit => {
                    set_constexpr_kind(&mut ds, k, loc)
                }
                Kw::Inline => set_flag(&mut ds.inline, loc, "inline"),
                Kw::Virtual => set_flag(&mut ds.virtual_, loc, "virtual"),
                // [class.friend]/1: only in a class; elsewhere diagnosed
                // and dropped (clang's `err_friend_invalid_in_context`).
                Kw::Friend
                    if !matches!(ctx, DeclaratorContext::Member | DeclaratorContext::TypeName) =>
                {
                    let d = self.s.diag(DiagId::sema_friend_invalid_in_context, loc);
                    self.emit(d);
                    Ok(())
                }
                Kw::Friend => set_flag(&mut ds.friend, loc, "friend"),
                Kw::Noreturn => {
                    let d = self.s.diag(DiagId::parse_c11_noreturn, loc);
                    self.emit(d);
                    set_flag(&mut ds.noreturn, loc, "_Noreturn")
                }
                Kw::Explicit => {
                    let r = set_flag(&mut ds.explicit, loc, "explicit");
                    if self.peek_is(1, Punct::LParen) {
                        self.parse_explicit_specifier(&mut ds, loc, r);
                        continue;
                    }
                    r
                }
                Kw::Const => ds.add_cv(Quals::CONST, "const", loc),
                Kw::Volatile => ds.add_cv(Quals::VOLATILE, "volatile", loc),
                Kw::Restrict => ds.add_cv(Quals::RESTRICT, "restrict", loc),
                Kw::Nonnull | Kw::Nullable | Kw::NullUnspecified | Kw::NullableResult => {
                    if let Some(kind) = nullability_kw(k) {
                        ds.nullability.push(NullSpec { kind, loc });
                    }
                    Ok(())
                }
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
                Kw::BitInt | Kw::ExtInt => {
                    self.parse_bit_int_into(&mut ds, k, loc);
                    continue;
                }
                Kw::Float => ds.set_base(TypeSpecKind::Float, None, loc),
                Kw::Double => ds.set_base(TypeSpecKind::Double, None, loc),
                Kw::Float128 => ds.set_base(TypeSpecKind::Float128, None, loc),
                Kw::Float16 => ds.set_base(TypeSpecKind::Float16, None, loc),
                // glibc's <bits/floatn*.h> declare `typedef float _Float32;`
                // (and `_Float64`, `_Float128`, `_Float32x`, `_Float64x`)
                // for C++ compilers that are not GCC >= 13; kiln reports
                // itself as clang. In a system header such a typedef of a
                // P1467 keyword or GCC `_FloatNx` keyword declares nothing
                // (spec/types.md §5.1).
                Kw::Float32 | Kw::Float64 | Kw::StdFloat128 | Kw::Float32x | Kw::Float64x
                    if matches!(ds.storage, Some((StorageSpec::Typedef, _)))
                        && ds.has_type_specifier()
                        && self.peek_is(1, Punct::Semi)
                        && self.s.diags.sm().is_in_system_header(loc) =>
                {
                    self.bump();
                    break;
                }
                Kw::Float32 => ds.set_base(TypeSpecKind::Float32, None, loc),
                Kw::Float64 => ds.set_base(TypeSpecKind::Float64, None, loc),
                Kw::StdFloat128 => ds.set_base(TypeSpecKind::StdFloat128, None, loc),
                Kw::Float32x => ds.set_base(TypeSpecKind::Float32x, None, loc),
                Kw::Float64x => ds.set_base(TypeSpecKind::Float64x, None, loc),
                Kw::Bf16 => ds.set_base(TypeSpecKind::BFloat16, None, loc),
                Kw::Fp16 => ds.set_base(TypeSpecKind::Fp16, None, loc),
                Kw::Complex => {
                    // clang `ext_c99_feature` on the keyword.
                    self.s.error(DiagId::parse_c99_complex, loc);
                    ds.set_complex(loc)
                }
                Kw::Auto => ds.set_base(TypeSpecKind::Auto, None, loc),
                Kw::Struct | Kw::Class | Kw::Union | Kw::Enum => {
                    self.parse_tag_specifier(&mut ds, k, ctx);
                    continue;
                }
                Kw::Decltype => {
                    self.parse_decltype_into(&mut ds, loc);
                    continue;
                }
                Kw::Atomic => {
                    self.parse_atomic_specifier_into(&mut ds, loc);
                    continue;
                }
                Kw::Typename => {
                    self.typename_pack_index = None;
                    let ty = self.parse_typename_specifier_in(Some(ctx));
                    if std::mem::take(&mut self.typename_failed) {
                        if ctx == DeclaratorContext::Block
                            && let Some(n) = self.ident()
                        {
                            // At block scope the name is then taken for a
                            // type-specifier whatever it names ("unknown
                            // type name") when a declarator follows, else
                            // for the declarator-id of a declaration with
                            // no type-specifier (clang's `ParseImplicitInt`).
                            let at = self.loc();
                            if self.unknown_type_follows(self.pos() + 1, ctx) {
                                let text = self.s.ast.symbols().as_str(n).to_owned();
                                let d = self.s.diag(DiagId::sema_unknown_typename, at).arg(text);
                                self.emit(d);
                                self.bump();
                                ds.type_error = true;
                                let _ = ds.set_base(TypeSpecKind::Named, Some(QualType::INT), at);
                            } else {
                                self.s.error(DiagId::parse_missing_type_specifier, at);
                                let _ = ds.set_base(TypeSpecKind::Named, Some(QualType::ERROR), at);
                            }
                            ds.invalid = true;
                            break;
                        }
                        ds.invalid = true;
                        let _ = ds.set_base(TypeSpecKind::Named, Some(QualType::ERROR), loc);
                        break;
                    }
                    if ty.is_none() {
                        ds.invalid = true;
                    }
                    if let Err(e) =
                        ds.set_base(TypeSpecKind::Named, Some(ty.unwrap_or(QualType::ERROR)), loc)
                    {
                        ds.invalid = true;
                        self.spec_error(&e, loc);
                    }
                    if self.param_abort {
                        break;
                    }
                    // clang ends a namespace-scope or member declaration
                    // at `typename T...[i]` (spec/parse.md).
                    let id = match ctx {
                        DeclaratorContext::File => Some(DiagId::parse_expected_unqualified_id),
                        DeclaratorContext::Member => Some(DiagId::parse_expected_member_name),
                        _ => None,
                    };
                    if let Some(id) = id
                        && let Some((at, _)) = self.typename_pack_index.take()
                    {
                        self.s.error(id, at);
                        self.skip_until(&[Punct::Semi], false, false);
                    }
                    continue;
                }
                Kw::Typeof | Kw::TypeofUnqual => {
                    // GNU `typeof` (spec/sema.md §5.8). An erroneous one
                    // is an error type specifier that combines with
                    // nothing (clang `SetTypeSpecError`).
                    let Some(ty) = self.parse_typeof_specifier(k == Kw::TypeofUnqual) else {
                        ds.invalid = true;
                        if ds.base == TypeSpecKind::None {
                            let _ = ds.set_base(TypeSpecKind::Named, Some(QualType::ERROR), loc);
                        }
                        continue;
                    };
                    if let Err(e) = ds.set_base(TypeSpecKind::Named, Some(ty), loc) {
                        ds.invalid = true;
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
        if let Some((at, end, attrs)) = trailing {
            if end == self.pos() {
                // At the end of the decl-specifier-seq the attributes
                // appertain to the type ([dcl.spec.general]/1): an
                // alignment-specifier is no type attribute.
                let mut attrs = attrs;
                let all = attrs.clone();
                self.s.act_on_type_position_attrs(&mut attrs, false);
                if self.at(Punct::Semi)
                    && let Some(t) = ds.tag
                {
                    // A declaration of the tag alone: no declarator's
                    // type takes them (spec/sema.md §4.5).
                    self.s.warn_free_standing_tag_attrs(&ds, t, &all);
                }
                ds.attrs.extend(attrs);
            } else {
                self.s.error(DiagId::parse_attributes_not_allowed, at);
            }
        }
        self.reject_friend_storage(&mut ds, thread_spelling.as_deref().unwrap_or("thread_local"));
        if ctx == DeclaratorContext::TypeName {
            self.reject_type_name_storage(&mut ds);
        }
        ds
    }

    /// A type-specifier-seq ([dcl.type.general]/1: a type-id, new-type-id,
    /// conversion-type-id, condition; spec/sema.md [dcl.stc]) takes no
    /// storage class (clang `ParseSpecifierQualifierList`,
    /// `err_typename_invalid_storageclass`): reported at the storage class,
    /// else at `thread_local`, and dropped.
    pub(crate) fn reject_type_name_storage(&mut self, ds: &mut DeclSpec) {
        if let Some(l) = ds.storage.map(|(_, l)| l).or(ds.thread_local) {
            self.s.error(DiagId::parse_typename_storage_class, l);
            ds.storage = None;
            ds.thread_local = None;
        }
    }

    /// [class.friend]/6: no storage class in a friend declaration (clang
    /// `DeclSpec::Finish`, `err_friend_decl_spec`): reported at the
    /// storage class, or `thread_local` after it (`'static thread_local'
    /// is invalid in friend declarations`), and dropped, the declaration
    /// otherwise proceeding as written.
    fn reject_friend_storage(&mut self, ds: &mut DeclSpec, thread: &str) {
        if ds.friend.is_none() || ds.storage.is_none() && ds.thread_local.is_none() {
            return;
        }
        let mut name = ds.storage.map(|(s, _)| s.spelling().to_owned()).unwrap_or_default();
        let mut at = ds.storage.map_or(Loc::INVALID, |(_, l)| l);
        if let Some(l) = ds.thread_local {
            if !name.is_empty() {
                name.push(' ');
            }
            name.push_str(thread);
            at = l;
        }
        let d = self.s.diag(DiagId::parse_friend_decl_spec, at).arg(name);
        self.emit(d);
        ds.storage = None;
        ds.thread_local = None;
    }

    /// The C11 atomic-type-specifier `_Atomic ( type-id )` (a clang
    /// extension in C++) added to `ds` as a named type; without the
    /// parenthesis, the `_Atomic` type qualifier (applied by
    /// `type_from_declspec`).
    fn parse_atomic_specifier_into(&mut self, ds: &mut DeclSpec, loc: Loc) {
        self.bump();
        let ty = if let Some(open) = self.eat(Punct::LParen) {
            let ty = self.parse_type_id();
            if ty.is_none() {
                self.skip_until(&[Punct::RParen], false, true);
            } else {
                self.expect_closing(Punct::RParen, Punct::LParen, open);
            }
            ty.and_then(|t| self.s.act_on_atomic_type(t, loc))
        } else {
            ds.atomic_qual = Some(loc);
            return;
        };
        if ty.is_none() {
            ds.invalid = true;
        }
        if let Err(e) = ds.set_base(TypeSpecKind::Named, Some(ty.unwrap_or(QualType::ERROR)), loc) {
            ds.invalid = true;
            self.spec_error(&e, loc);
        }
    }

    /// `_BitInt ( constant-expression )` (or the deprecated `_ExtInt`), a
    /// C23 bit-precise integer type clang accepts in C++ as an extension
    /// (clang `DiagnoseBitIntUse`, `ParseExtIntegerArgument`; spec/types.md
    /// §5.3). The width is checked when the type is formed
    /// ([`kiln_sema::Sema::build_bit_int_type`]), where the sign is known.
    fn parse_bit_int_into(&mut self, ds: &mut DeclSpec, k: Kw, loc: Loc) {
        let id = if k == Kw::ExtInt {
            DiagId::parse_ext_int_deprecated
        } else {
            DiagId::parse_bit_int_extension
        };
        self.s.error(id, loc);
        self.bump();
        let Some(open) = self.eat(Punct::LParen) else {
            let d = self
                .s
                .diag(DiagId::parse_expected_after, self.expected_loc())
                .arg("'_BitInt'")
                .arg("'('");
            self.emit(d);
            return;
        };
        let Some(e) = self.with_greater_op(true, Self::parse_constant_expr) else {
            self.skip_until(&[Punct::RParen], false, true);
            return;
        };
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        match ds.set_base(TypeSpecKind::BitInt, None, loc) {
            Ok(()) => ds.bit_int_width = Some((e, loc)),
            Err(e) => {
                ds.invalid = true;
                self.spec_error(&e, loc);
            }
        }
    }

    /// A unary type transform `__transform ( type-id )` (spec/sema-templates.md
    /// §9.2) added to `ds` as a named type.
    fn parse_unary_transform_into(&mut self, ds: &mut DeclSpec, k: UnaryTransformKind, loc: Loc) {
        let ty = self.parse_unary_transform(k);
        if ty.is_none() {
            ds.invalid = true;
        }
        if let Err(e) = ds.set_base(TypeSpecKind::Named, Some(ty.unwrap_or(QualType::ERROR)), loc) {
            ds.invalid = true;
            self.spec_error(&e, loc);
        }
    }

    /// `__transform ( type-id )`, at the name: the `UnaryTransform` type,
    /// `None` after an error.
    pub(crate) fn parse_unary_transform(&mut self, k: UnaryTransformKind) -> Option<QualType> {
        let loc = self.bump().span.lo;
        let open = self.bump().span.lo;
        let Some(ty) = self.parse_type_id() else {
            self.skip_until(&[Punct::RParen], true, true);
            return None;
        };
        self.expect_closing(Punct::RParen, Punct::LParen, open);
        self.s.build_unary_transform(k, ty, loc)
    }

    /// A `decltype` type-specifier added to `ds`.
    fn parse_decltype_into(&mut self, ds: &mut DeclSpec, loc: Loc) {
        let (base, ty) = self.parse_decltype_specifier();
        if base == TypeSpecKind::Named && ty.is_none() {
            ds.invalid = true;
        }
        let ty =
            if base == TypeSpecKind::Named { Some(ty.unwrap_or(QualType::ERROR)) } else { None };
        if let Err(e) = ds.set_base(base, ty, loc) {
            ds.invalid = true;
            self.spec_error(&e, loc);
        }
    }

    /// `explicit ( constant-expression )` ([dcl.fct.spec]/3), at
    /// `explicit`; `r` is the result of recording the keyword.
    fn parse_explicit_specifier(&mut self, ds: &mut DeclSpec, loc: Loc, r: Result<(), SpecError>) {
        self.bump();
        if let Err(e) = r {
            self.spec_error(&e, loc);
        }
        let open = self.bump().span.lo;
        if let Some(e) = self.with_greater_op(true, Self::parse_constant_expr) {
            ds.explicit_expr = Some(e);
            self.expect_closing(Punct::RParen, Punct::LParen, open);
        } else {
            ds.invalid = true;
            self.skip_until(&[Punct::RParen], false, false);
            self.eat(Punct::RParen);
        }
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
        self.s.enter_decltype();
        // clang 19's `ParseDecltypeSpecifier` corrects the operand's
        // delayed typos as soon as it is parsed, even inside an enclosing
        // full-expression (WP-399, WP-408).
        let typos = self.begin_typo_correction();
        // [temp.names]/4: inside the parentheses `>` is an operator again.
        let e = self.with_greater_op(true, Self::parse_expression);
        self.correct_typos(typos, e.is_some());
        self.s.exit_decltype(e);
        let Some(e) = e else {
            self.skip_until(&[Punct::RParen], false, true);
            return (TypeSpecKind::Named, None);
        };
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
        (TypeSpecKind::Named, self.s.act_on_decltype(e, loc, rparen))
    }

    /// GNU `typeof` specifier (spec/sema.md §5.8): `__typeof ( type-id )`
    /// or `__typeof` and an unevaluated unary-expression, as clang;
    /// `unqual` for `__typeof_unqual`. At the keyword; `None` after an
    /// error.
    pub(crate) fn parse_typeof_specifier(&mut self, unqual: bool) -> Option<QualType> {
        let loc = self.bump().span.lo;
        match self.parse_trait_operand()? {
            crate::expr::TraitOperand::Type(ty, _) => Some(self.s.act_on_typeof_type(ty, unqual)),
            crate::expr::TraitOperand::Expr(e) => self.s.act_on_typeof_expr(e?, loc, unqual),
        }
    }

    fn spec_error(&self, e: &SpecError, loc: Loc) {
        let mut d = self.s.diag(e.id, loc);
        for a in &e.args {
            d = d.arg(a);
        }
        self.emit(d);
    }

    /// Class template `d`, written up to token `end` (`qualified`: after a
    /// nested-name-specifier), as the placeholder for a deduced class type
    /// ([dcl.type.class.deduct]) of `ds`.
    fn parse_ctad_placeholder(
        &mut self,
        ds: &mut DeclSpec,
        d: DeclId,
        qualified: bool,
        end: usize,
        loc: Loc,
    ) {
        let mut nns = None;
        let mut d = d;
        let name_loc = self.tok_at(end - 1).span.lo;
        if qualified {
            let name = self.ident_of(self.tok_at(end - 1));
            if let Ok(Some(q)) = self.parse_nested_name_nns() {
                nns = Some(q.nns);
                // `X<A>::M`: the lookahead found `M` in `X`'s primary
                // pattern; the placeholder names the member template of
                // the specialization the qualifier names (as `td::M`
                // with `typedef X<A> td` does), so its guides are those
                // of the instantiated member ([temp.mem]).
                if let Some(name) = name
                    && let NameClass::TypeTemplate(m) = self.s.classify_name(name, Some(q.ctx))
                    && self.s.is_ctad_template(m)
                {
                    d = m;
                }
            } else {
                ds.invalid = true;
            }
        }
        while self.pos() < end {
            self.bump();
        }
        let ty = self.s.ctad_placeholder(d, nns, (loc, name_loc));
        if let Err(e) = ds.set_base(TypeSpecKind::Named, Some(ty), loc) {
            ds.invalid = true;
            self.spec_error(&e, loc);
        }
    }

    /// A type-name in a decl-specifier-seq (`T`, `N::T`), or an unknown
    /// identifier that can only be one. Returns false (consuming nothing)
    /// if the tokens are not a type name.
    /// [`Parser::at_template_type_name`], diagnosing `X<int>` with `X`
    /// naming class templates of two namespaces (an inline namespace's and
    /// its parent's): ambiguous ([basic.lookup.general]/1), recovering
    /// with the first.
    fn at_template_type_name_checked(&mut self, ctx: DeclaratorContext, loc: Loc) -> bool {
        if !self.at_template_type_name(ctx) {
            return false;
        }
        if let Some(name) = self.ident()
            && self.peek_is(1, Punct::Less)
        {
            self.s.diagnose_ambiguous_template_name(name, loc);
        }
        true
    }

    /// A type-name spec that starts with a template-id (`X<A>` or
    /// `X<A>::type`).
    fn parse_template_type_name_spec(
        &mut self,
        ds: &mut DeclSpec,
        ctx: DeclaratorContext,
        start: usize,
        loc: Loc,
    ) {
        let scanned = self.scan_name_at(start).and_then(|(end, split, ..)| (!split).then_some(end));
        let ty = self.parse_template_type_name(ctx);
        if ty.is_none() {
            // An invalid qualifier (`X<A>::type` where the instantiation
            // of `X<A>` fails here) still spans the scanned name: the
            // declaration goes on with an invalid type (clang's invalid
            // `CXXScopeSpec`), not a syntax error.
            if let Some(end) = scanned {
                while self.pos() < end {
                    self.bump();
                }
            }
            ds.invalid = true;
        }
        if let Err(e) = ds.set_base(TypeSpecKind::Named, Some(ty.unwrap_or(QualType::ERROR)), loc) {
            ds.invalid = true;
            self.spec_error(&e, loc);
        }
    }

    /// A type-name decl-specifier at the current token: the name class
    /// picks the arm, and each arm is a named helper. False if the name
    /// is no type-name here (the caller tries the other specifiers).
    fn parse_type_name_spec(&mut self, ds: &mut DeclSpec, ctx: DeclaratorContext) -> bool {
        let start = self.pos();
        let loc = self.name_begin();
        if self.at_template_type_name_checked(ctx, loc) {
            self.parse_template_type_name_spec(ds, ctx, start, loc);
            return true;
        }
        match self.scan_name(start) {
            Some((end, NameClass::Type(..), _)) if end > start + 1 => {
                self.qualified_type_name_spec(ds, ctx, loc);
                true
            }
            Some((end, NameClass::Type(ty, d), name)) => {
                self.simple_type_name_spec(ds, end, (ty, d, name), loc);
                true
            }
            Some((end, NameClass::TypeTemplate(d), _))
                if end == start + 1 && self.s.names_injected_class(d) =>
            {
                self.injected_class_name_spec(ds, d, loc);
                true
            }
            // The injected-class-name of a base class template
            // specialization, `B` or `D::B` ([temp.local]/1-2).
            Some((end, NameClass::TypeTemplate(_), name))
                if !self.next_is_less(end) && self.inherited_injected_at(start).is_some() =>
            {
                if end > start + 1 {
                    self.qualified_type_name_spec(ds, ctx, loc);
                } else if let Some(d) = self.s.inherited_injected_class(name, None) {
                    self.bump();
                    self.s.mark_type_referenced(d);
                    let ty = self.s.elaborated_tag_type(d, TagKeyword::None);
                    self.set_named_base(ds, Some(ty), loc);
                }
                true
            }
            // The injected-class-names of two specializations of one
            // template found through different bases, not as a
            // template-name: ambiguous ([temp.local]/4), recovering with
            // the first.
            Some((end, NameClass::TypeTemplate(_), name))
                if end == start + 1
                    && !self.next_is_less(end)
                    && self.s.ambiguous_injected_class(name).is_some() =>
            {
                if let Some(d) = self.s.ambiguous_injected_class(name) {
                    self.bump();
                    self.s.diagnose_ambiguous_name(name, None, loc);
                    let ty = self.s.elaborated_tag_type(d, TagKeyword::None);
                    self.set_named_base(ds, Some(ty), loc);
                }
                true
            }
            // A placeholder for a deduced class type ([dcl.type.class.deduct]).
            Some((end, NameClass::TypeTemplate(d), _))
                if self.names_ctad_template(self.scan_name_qual_at(start).and_then(|x| x.4), d) =>
            {
                self.parse_ctad_placeholder(ds, d, end > start + 1, end, loc);
                true
            }
            // `X<A>::name` with an annotated scope ([`Parser::annotate_template_scope`])
            // in which `name` is not a type, or `S::name d` with `S` a
            // non-dependent class or namespace that has no `name`, where
            // only a type can be: the parse proper reports it ("no type
            // named 'name' in 'S'"; clang's `DiagnoseUnknownTypeName`).
            // At block scope the statement disambiguation chose a
            // declaration already (`S::name(e);`, `S::name *g;`). At
            // namespace scope a variable or function `N::f` is reported
            // the same way (`N::f(v);`, `N::f x;`).
            Some((end, NameClass::Undeclared, _))
                if (end == start + 2
                    && self.at_decltype_scope()
                    && self.decltype_scope_ok()
                    && self.unknown_type_follows(end, ctx))
                    || (end > start + 1
                        && self.unresolved_scope_name(start)
                        && self.qualified_unknown_type_follows(end, ctx))
                    || (end > start + 1
                        && self.incomplete_scope_name(start)
                        && (!matches!(
                            ctx,
                            DeclaratorContext::File | DeclaratorContext::Member
                        ) || self.qualified_unknown_type_follows(end, ctx))) =>
            {
                self.unknown_qualified_type_spec(ds, loc);
                true
            }
            Some((end, NameClass::Value(_), _))
                if matches!(ctx, DeclaratorContext::File | DeclaratorContext::Member)
                    && self.nontype_scope_name(start)
                    && self.qualified_unknown_type_follows(end, ctx) =>
            {
                self.unknown_qualified_type_spec(ds, loc);
                true
            }
            Some((end, NameClass::Undeclared, name))
                if end == start + 1 && self.undeclared_template_type(end, ctx) =>
            {
                self.undeclared_template_spec(ds, name, loc)
            }
            Some((end, NameClass::Undeclared, name)) if end == start + 1 => {
                self.unknown_type_name_spec(ds, end, ctx, name, loc)
            }
            // `Und::x y;`, `X::type z;` with `X` a class template: at
            // namespace or class scope a qualified name starts a
            // decl-specifier, and parsing its qualifier reports the
            // component that names no scope ("use of undeclared
            // identifier 'Und'", "use of class template 'X' requires
            // template arguments"); the declaration goes on with an
            // invalid type (clang's invalid `CXXScopeSpec` in
            // `ParseDeclarationSpecifiers`), one error in all.
            // Followed by what is no declarator, the name is the
            // declarator-id ([`Parser::at_qualified_implicit_int_declarator`]).
            None if matches!(ctx, DeclaratorContext::File | DeclaratorContext::Member)
                && self
                    .not_a_scope_name(start)
                    .is_some_and(|end| self.qualified_unknown_type_follows(end, ctx)) =>
            {
                self.unknown_qualified_type_spec(ds, loc);
                true
            }
            _ => false,
        }
    }

    /// Sets the decl-spec's named base type `ty` (`None` when the name
    /// names no type), reporting a conflicting specifier.
    fn set_named_base(&mut self, ds: &mut DeclSpec, ty: Option<QualType>, loc: Loc) {
        if let Err(e) = ds.set_base(TypeSpecKind::Named, ty, loc) {
            ds.invalid = true;
            self.spec_error(&e, loc);
        }
    }

    /// A qualified type-name (`N::T`, `X<A>::type`): the qualifier is
    /// formed (and kept as sugar) by the parse proper.
    fn qualified_type_name_spec(&mut self, ds: &mut DeclSpec, ctx: DeclaratorContext, loc: Loc) {
        let ty = match self.parse_nested_name_nns() {
            Ok(Some(q)) => {
                self.check_missing_typename_in_class(q, ctx, loc);
                self.parse_qualified_type_rest(q, TagKeyword::None)
            }
            _ => None,
        };
        if ty.is_none() {
            ds.invalid = true;
        }
        self.set_named_base(ds, Some(ty.unwrap_or(QualType::ERROR)), loc);
    }

    /// An unqualified type-name ending before token `end`, classified as
    /// the type `ty` declared by `d` under the name `name`, with an
    /// optional pack index (`T...[index]`, C++26 [dcl.type.pack.index]).
    fn simple_type_name_spec(
        &mut self,
        ds: &mut DeclSpec,
        end: usize,
        (ty, d, name): (QualType, DeclId, Symbol),
        loc: Loc,
    ) {
        while self.pos() < end {
            self.bump();
        }
        // An ambiguous name recovers as the type `d` classified
        // as ([basic.lookup.general]/1, clang's `getTypeName`).
        self.s.diagnose_ambiguous_name(name, None, loc);
        self.s.mark_type_referenced(d);
        self.s.check_type_name_access(name, None, d, loc);
        let kind = self.s.ast.decls.get(d).kind();
        let ty = if kiln_sema::lookup::is_tag_kind(kind) {
            self.s.elaborated_tag_type(d, TagKeyword::None)
        } else {
            ty
        };
        let ty = match self.parse_pack_index(loc) {
            Ok(Some((ellipsis, index, _))) => self.s.act_pack_indexing_type(ty, ellipsis, index),
            Ok(None) => ty,
            Err(()) => QualType::ERROR,
        };
        self.set_named_base(ds, Some(ty), loc);
    }

    /// Whether token `i` is `<`.
    fn next_is_less(&self, i: usize) -> bool {
        self.tok_at(i).kind == TokKind::Punct(Punct::Less)
    }

    /// The base class template specialization whose injected-class-name
    /// the (qualified) name at token `start` finds
    /// ([`kiln_sema::Sema::inherited_injected_class`]).
    fn inherited_injected_at(&self, start: usize) -> Option<DeclId> {
        let (_, _, class, name, qual) = self.scan_name_qual_at(start)?;
        if !matches!(class, NameClass::TypeTemplate(_)) {
            return None;
        }
        self.s.inherited_injected_class(name, qual)
    }

    /// The injected-class-name of the class template `d` ([temp.local]/1).
    fn injected_class_name_spec(&mut self, ds: &mut DeclSpec, d: DeclId, loc: Loc) {
        self.bump();
        let ty = self.s.injected_class_type(d);
        self.set_named_base(ds, ty, loc);
    }

    /// A qualified name that names no type where only a type can be
    /// ([`Parser::parse_type_name_spec`]): the parse proper reports it,
    /// and the decl-spec recovers to an invalid `int`.
    fn unknown_qualified_type_spec(&mut self, ds: &mut DeclSpec, loc: Loc) {
        match self.parse_nested_name_nns() {
            Ok(Some(q)) => {
                let _ = self.parse_qualified_type_rest(q, TagKeyword::None);
            }
            // The specifier named an incomplete class or a component
            // that names no scope (diagnosed): the rest of it and the
            // final name, with its template arguments, belong to the
            // invalid type (clang annotates the invalid `CXXScopeSpec`).
            Err(()) => {
                self.skip_nested_name_rest();
                if self.ident().is_some() {
                    self.bump();
                    if self.at(Punct::Less)
                        && let Some((close, false)) = self.skip_template_args(self.pos())
                    {
                        while self.pos() <= close {
                            self.bump();
                        }
                    }
                }
            }
            Ok(None) => {}
        }
        ds.invalid = true;
        ds.type_error = true;
        let _ = ds.set_base(TypeSpecKind::Named, Some(QualType::INT), loc);
    }

    /// An undeclared unqualified name ending before token `end`: "unknown
    /// type name" and an invalid `int` when a declarator follows
    /// ([`Parser::unknown_type_follows`]), else false.
    fn unknown_type_name_spec(
        &mut self,
        ds: &mut DeclSpec,
        end: usize,
        ctx: DeclaratorContext,
        name: Symbol,
        loc: Loc,
    ) -> bool {
        if !self.unknown_type_follows(end, ctx) {
            return false;
        }
        // A reachable type that is not visible ([module.reach]):
        // the missing-import error, then the type is used.
        if let Some(NameClass::Type(ty, d)) = self.s.module_hidden_type(name, None, loc) {
            self.bump();
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
            return true;
        }
        let text = self.s.ast.symbols().as_str(name).to_owned();
        let d = self.s.diag(DiagId::sema_unknown_typename, loc).arg(text);
        self.emit(d);
        self.bump();
        ds.invalid = true;
        ds.type_error = true;
        let _ = ds.set_base(TypeSpecKind::Named, Some(QualType::INT), loc);
        true
    }

    /// True if token `end`, after an unknown name in a decl-specifier,
    /// shows the name was meant as a type (a declarator follows).
    ///
    /// A ptr-operator counts in every declaration context (clang's
    /// `ParseImplicitInt`: `*`, `&` and `&&` are not valid after a
    /// declarator-id): at block scope the statement disambiguation chose
    /// a declaration for `u5 &r;` already
    /// ([`Scan::is_simple_declaration`](crate::tentative)), and after
    /// another decl-specifier (`const u7 &c;`) only a declaration is
    /// possible (WP-318).
    fn unknown_type_follows(&self, end: usize, ctx: DeclaratorContext) -> bool {
        let next = self.tok_at(end);
        self.ident_of(next).is_some()
            // A type-specifier keyword: `restrict int f();` (`restrict` is
            // no keyword in C++; clang's `ParseImplicitInt` default case).
            || (matches!(ctx, DeclaratorContext::File | DeclaratorContext::Member | DeclaratorContext::Block)
                && self.kw_of(next).is_some_and(is_type_specifier_kw))
            || (matches!(
                ctx,
                DeclaratorContext::File
                    | DeclaratorContext::Member
                    | DeclaratorContext::Block
                    | DeclaratorContext::Condition
                    | DeclaratorContext::ForInit
            ) && matches!(Self::punct_of(next), Some(Punct::Star | Punct::Amp | Punct::AmpAmp)))
            // A parameter: clang's `ParseImplicitInt` diagnoses any
            // undeclared name where a decl-specifier is required in C++
            // (`void a(u1);`, `int e(u4 *p);`); `(`, `::` and `<` are
            // reported elsewhere.
            || (ctx == DeclaratorContext::Prototype
                && !matches!(
                    Self::punct_of(next),
                    Some(Punct::LParen | Punct::ColonColon | Punct::Less)
                ))
            // A bit-field width ([class.bit]/1) or, at namespace scope,
            // what clang's `ParseImplicitInt` treats the same way: `y : 3;`
            // is "unknown type name 'y'", then an anonymous bit-field
            // (namespace scope: "expected unqualified-id" at the `:`).
            || (matches!(ctx, DeclaratorContext::File | DeclaratorContext::Member)
                && Self::punct_of(next) == Some(Punct::Colon))
            // A type-id (`new Zork`, `static_cast<Zork *>`, `using X =
            // Zork;`, a trailing return type or a default template
            // argument): only a type can start one, so clang's
            // `ParseImplicitInt` (a type-specifier context) diagnoses any
            // undeclared name (WP-186).
            || ctx == DeclaratorContext::TypeName
    }

    /// `foo<args> x` with `foo` undeclared ([`Parser::undeclared_template_type`]):
    /// "no template named", the arguments parsed and dropped, and an
    /// invalid `int` specifier (clang's `ParseImplicitInt` with
    /// `IsTemplateName`).
    fn undeclared_template_spec(&mut self, ds: &mut DeclSpec, name: Symbol, loc: Loc) -> bool {
        let text = self.s.ast.symbols().as_str(name).to_owned();
        let d = self.s.diag(DiagId::sema_no_template_named, loc).arg(text);
        self.emit(d);
        self.bump();
        let _ = self.parse_template_arg_list();
        ds.invalid = true;
        ds.type_error = true;
        let _ = ds.set_base(TypeSpecKind::Named, Some(QualType::INT), loc);
        true
    }

    /// True if the undeclared unqualified name before token `end` starts
    /// a template-id meant as a type ([temp.names]/3.3: lookup finding
    /// nothing before `<` makes it a template-name): at namespace or
    /// class scope only a declaration can follow, elsewhere a declarator
    /// follows the argument list ([`Parser::unknown_type_follows`]).
    fn undeclared_template_type(&self, end: usize, ctx: DeclaratorContext) -> bool {
        if Self::punct_of(self.tok_at(end)) != Some(Punct::Less) {
            return false;
        }
        let Some((close, false)) = self.skip_template_args(end) else { return false };
        matches!(ctx, DeclaratorContext::File | DeclaratorContext::Member)
            || self.unknown_type_follows(close + 1, ctx)
    }

    /// True if an unknown qualified name in a decl-specifier, ending
    /// before token `end`, was meant as a type: at block scope the
    /// statement disambiguation chose a declaration already, elsewhere a
    /// declarator follows ([`Parser::unknown_type_follows`]) or, at
    /// namespace or class scope, `<` (a class template:
    /// `std::codecvt_utf8<wchar_t> c;`, clang's `ParseImplicitInt` with
    /// `IsTemplateName`; at block scope `N::f<int> x;` is an expression).
    /// At namespace scope a `(` that starts a declarator counts too
    /// (`N::f(v);`, `N::f(*p)[2];`; clang's `ParseImplicitInt` runs
    /// `TryParseDeclarator` after the name), while `N::f(1);` and
    /// `N::f(int);` declare `N::f` with no type.
    fn qualified_unknown_type_follows(&self, end: usize, ctx: DeclaratorContext) -> bool {
        ctx == DeclaratorContext::Block
            || self.unknown_type_follows(end, ctx)
            || (matches!(ctx, DeclaratorContext::File | DeclaratorContext::Member)
                && Self::punct_of(self.tok_at(end)) == Some(Punct::Less))
            || (matches!(ctx, DeclaratorContext::File | DeclaratorContext::Member)
                && Self::punct_of(self.tok_at(end)) == Some(Punct::LParen)
                && self.declarator_may_start_at(end))
    }

    // -- class and enum specifiers ----------------------------------------------------

    /// The optional nested-name-specifier of a tag specifier: `N::C`, or
    /// `X<A>::C` (the out-of-line definition of a member class of a class
    /// template, [temp.mem.class]); the scope is entered as clang's
    /// `ParseClassSpecifier` does.
    fn parse_tag_qualifier(&mut self, ds: &mut DeclSpec) -> Option<kiln_sema::using::Qualifier> {
        let template_id_qual = || {
            self.ident().is_some()
                && self.peek_is(1, Punct::Less)
                && matches!(self.skip_template_args(self.pos() + 1),
                    Some((close, false)) if self.tok_at(close + 1).kind
                        == TokKind::Punct(Punct::ColonColon))
        };
        if !(self.at(Punct::ColonColon)
            || (self.ident().is_some() && self.peek_is(1, Punct::ColonColon))
            || template_id_qual()
            || (self.at_splice() && self.decltype_scope_end(self.pos()).is_some()))
        {
            return None;
        }
        let saved = self.s.set_entering_context(true);
        let q = self.parse_nested_name_nns();
        self.s.set_entering_context(saved);
        q.unwrap_or_else(|()| {
            ds.invalid = true;
            None
        })
    }

    /// `class-key M<args>` with `M` a template template parameter
    /// ([dcl.type.elab]/1, a dependent specialization; libc++'s
    /// `friend struct _MapOrSetSpecifics<...>;` in `<__node_handle>`): the
    /// type as written, a friend type in `friend class-key M<args>;`.
    /// False (nothing consumed) when the next tokens are not that form.
    fn parse_ttp_elaborated(
        &mut self,
        ds: &mut DeclSpec,
        kw_loc: Loc,
        ctx: DeclaratorContext,
    ) -> bool {
        let Some(n) = self.ident() else { return false };
        if Self::punct_of(self.tok_at(self.pos() + 1)) != Some(Punct::Less) {
            return false;
        }
        let NameClass::TypeTemplate(t) = self.s.classify_name(n, None) else { return false };
        if self.s.ast.decls.get(t).kind() != kiln_ast::decl::DeclKind::TemplateTemplateParm {
            return false;
        }
        let ty = self.parse_template_type_name(ctx);
        if ty.is_none() {
            ds.invalid = true;
        }
        if let Err(e) =
            ds.set_base(TypeSpecKind::Named, Some(ty.unwrap_or(QualType::ERROR)), kw_loc)
        {
            ds.invalid = true;
            self.spec_error(&e, kw_loc);
        }
        true
    }

    /// class-specifier, enum-specifier or elaborated-type-specifier
    /// ([class.pre], [dcl.enum], [dcl.type.elab]).
    /// `class-key T::X` with a dependent qualifier ([dcl.type.elab],
    /// [temp.dep.type]/6): a dependent name type with the class-key as its
    /// keyword, resolved (and its tag kind checked) at instantiation, as
    /// clang's `ActOnDependentTag`; with `friend`, a friend type
    /// ([class.friend]/3, libstdc++ generator:684
    /// `friend struct _Erased_promise::_Subyield_state;`).
    fn dependent_tag_reference(
        &mut self,
        ds: &mut DeclSpec,
        nns: kiln_ast::NnsId,
        (name, name_loc): (Symbol, Loc),
        keyword: TagKeyword,
        kw_loc: Loc,
    ) {
        let ty = self.s.act_dependent_name_at(nns, name, None, keyword, name_loc);
        if let Err(e) = ds.set_base(TypeSpecKind::Named, Some(ty), kw_loc) {
            ds.invalid = true;
            self.spec_error(&e, kw_loc);
        }
    }

    /// An optional enum-base `: type-specifier-seq` ([dcl.enum]/2); the
    /// fixed underlying type, or `None` if absent or not a type.
    fn parse_enum_base(&mut self) -> Option<QualType> {
        if !self.at(Punct::Colon) {
            return None;
        }
        self.bump();
        let base = self.parse_decl_specifiers(DeclaratorContext::TypeName);
        if !base.has_type_specifier() {
            self.error_here(DiagId::parse_expected_type);
            return None;
        }
        let ty = self.s.type_from_declspec(&base);
        // [dcl.enum]/2, [dcl.spec.auto.general]/5.
        let at = base.type_loc.unwrap_or(base.loc);
        let ty = self.s.type_specifier_placeholder_check(ty, at);
        // clang: at the type's location, a `_BitInt`'s keyword.
        let at = base.bit_int_width.map_or(at, |(_, kw)| kw);
        self.s.check_enum_fixed_type(Some(ty), at)
    }

    /// `parse_tag_qualifier`, and whether it failed (diagnosed it named
    /// nothing): the template headers it would have matched are
    /// discarded with it.
    fn parse_tag_qualifier_checked(
        &mut self,
        ds: &mut DeclSpec,
    ) -> (Option<kiln_sema::using::Qualifier>, bool) {
        let was_invalid = ds.invalid;
        let qual = self.parse_tag_qualifier(ds);
        let failed = !was_invalid && ds.invalid;
        if failed {
            self.s.discard_spec_headers();
        }
        (qual, failed)
    }

    /// `struct NT<int> {}` naming a non-template (or after a qualifier
    /// that named nothing): [temp.expl.spec]/1 specializes a template,
    /// so this is clang's `err_explicit_spec_non_template`; the arguments
    /// are skipped and `NT` itself declared.
    fn skip_non_template_args(
        &mut self,
        keyword: TagKeyword,
        (n, nloc): (Symbol, Loc),
        qualifier: Option<DeclId>,
        qual_failed: bool,
    ) {
        if !self.at(Punct::Less)
            || !qual_failed
                && matches!(self.s.classify_name(n, qualifier), NameClass::TypeTemplate(_))
        {
            return;
        }
        let Some((close, false)) = self.skip_template_args(self.pos()) else { return };
        let what = format!("{} '{}'", keyword.spelling(), self.s.ast.symbols().as_str(n));
        let d = self.s.diag(DiagId::sema_explicit_spec_non_template, nloc).arg(what);
        self.emit(d);
        while self.pos() <= close && self.tok().kind != TokKind::Eof {
            self.bump();
        }
    }

    /// `class F::H;` with `F` incomplete: clang's `ActOnTag` rejects
    /// the qualified standalone declaration before it enters the
    /// incomplete scope (diagnosed by the specifier parse).
    fn incomplete_standalone_tag_qualifier(
        &mut self,
        ds: &DeclSpec,
        keyword: TagKeyword,
        qual_loc: Loc,
    ) {
        if ds.friend.is_none()
            && ds.storage.is_none()
            && self.incomplete_scope_name(self.pos())
            && let Some((end, ..)) = self.scan_name_qual_at(self.pos())
            && Self::punct_of(self.tok_at(end)) == Some(Punct::Semi)
        {
            self.s.check_standalone_tag_qualifier(keyword, qual_loc);
        }
    }

    /// The class-key or `enum` (with its `class`/`struct`) at `k`: the tag
    /// keyword and the enum's `EnumData` scoped flags.
    fn parse_tag_key(&mut self, ds: &mut DeclSpec, k: Kw) -> (TagKeyword, u32) {
        let keyword = match k {
            Kw::Struct => TagKeyword::Struct,
            Kw::Class => TagKeyword::Class,
            Kw::Union => TagKeyword::Union,
            _ => TagKeyword::Enum,
        };
        let mut scoped = 0;
        if keyword == TagKeyword::Enum {
            if let Some(l) = self.eat_kw(Kw::Class) {
                scoped = EnumData::SCOPED;
                ds.scoped_key_end = Some(l.with_offset(5));
            } else if let Some(l) = self.eat_kw(Kw::Struct) {
                scoped = EnumData::SCOPED | EnumData::SCOPED_STRUCT;
                ds.scoped_key_end = Some(l.with_offset(6));
            }
        }
        (keyword, scoped)
    }

    /// The attributes after a class-key or enum-key. After a class-key,
    /// `_Nullable` (clang nullability on classes) may appear among them
    /// any number of times (clang `ParseClassSpecifier`); the other
    /// nullability keywords are not parsed here.
    fn parse_tag_key_attrs(&mut self, keyword: TagKeyword) -> Vec<AttrId> {
        let mut attrs = Vec::new();
        self.parse_attributes(&mut attrs);
        while keyword != TagKeyword::Enum && self.at_kw(Kw::Nullable) {
            let loc = self.bump().span.lo;
            attrs.push(self.s.act_on_class_nullable(loc));
            self.parse_attributes(&mut attrs);
        }
        attrs
    }

    fn parse_tag_specifier(&mut self, ds: &mut DeclSpec, k: Kw, ctx: DeclaratorContext) {
        let kw_loc = self.bump().span.lo;
        let (keyword, scoped) = self.parse_tag_key(ds, k);
        let mut attrs = self.parse_tag_key_attrs(keyword);
        let qual_loc = self.tok().span.lo;
        self.incomplete_standalone_tag_qualifier(ds, keyword, qual_loc);
        let splice_qual = self.at_splice().then_some(qual_loc);
        let (qual, mut qual_failed) = self.parse_tag_qualifier_checked(ds);
        let qualifier = qual.map(|q| q.ctx);
        if keyword != TagKeyword::Enum
            && qualifier.is_none()
            && self.parse_ttp_elaborated(ds, kw_loc, ctx)
        {
            return;
        }
        let name = self.ident().map(|n| (n, self.bump().span.lo));
        if name.is_none() && qualifier.is_some() {
            self.error_here(DiagId::parse_expected_unqualified_id);
            ds.invalid = true;
        }
        let fixed = if keyword == TagKeyword::Enum { self.parse_enum_base() } else { None };
        let is_class = keyword != TagKeyword::Enum;
        if is_class
            && !qual_failed
            && self.at(Punct::Less)
            && let Some((n, nloc)) = name
            && let NameClass::TypeTemplate(t) = self.s.classify_name(n, qualifier)
            && self.s.ast.decls.get(t).kind() == kiln_ast::decl::DeclKind::ClassTemplate
        {
            self.parse_class_template_id_tag(ds, keyword, t, qualifier, nloc, kw_loc, &attrs, ctx);
            return;
        }
        if is_class && let Some(name) = name {
            self.skip_non_template_args(keyword, name, qualifier, qual_failed);
        }
        let mut final_loc = None;
        if is_class && name.is_some() {
            self.parse_class_head_attrs(ds.storage.is_some(), &mut attrs);
            final_loc = self.parse_class_virt_specifiers();
        }
        let use_ = self.tag_use(ds, is_class, final_loc, ctx);
        // [expr.prim.id.qual]/3: the nested-name-specifier of a
        // class-head-name or enum-head-name is declarative and cannot
        // contain a splice-scope-specifier; the definition declares
        // nothing.
        if let Some(at) = splice_qual
            && use_ == TagUse::Definition
            && !qual_failed
        {
            self.s.error(DiagId::sema_splice_declarative_scope, at);
            self.s.discard_spec_headers();
            ds.invalid = true;
            qual_failed = true;
        }
        if name.is_none() && use_ != TagUse::Definition {
            if !ds.invalid {
                self.error_here(DiagId::parse_expected_unqualified_id);
            }
            ds.invalid = true;
            let _ = ds.set_base(TypeSpecKind::Named, Some(QualType::ERROR), kw_loc);
            return;
        }
        if use_ == TagUse::Declaration && qualifier.is_some() {
            self.s.check_standalone_tag_qualifier(keyword, qual_loc);
        }
        if let (Some(q), Some((n, nloc)), TagUse::Reference | TagUse::Friend) = (qual, name, use_)
            && self.s.is_dependent_ctx(q.ctx)
        {
            self.dependent_tag_reference(ds, q.nns, (n, nloc), keyword, kw_loc);
            return;
        }
        if qual_failed && use_ != TagUse::Definition {
            // The qualifier named nothing (diagnosed): no tag is declared
            // or found (clang's invalid `CXXScopeSpec` recovery).
            let _ = ds.set_base(TypeSpecKind::Named, Some(QualType::ERROR), kw_loc);
            return;
        }
        // A definition after a failed qualifier is parsed as an unnamed
        // class, declaring nothing.
        let name = if qual_failed { None } else { name };
        let d = self.s.act_on_tag(keyword, name, qualifier, use_, scoped, fixed, kw_loc);
        self.s.act_on_tag_attrs(d, &attrs, use_);
        if use_ == TagUse::Definition && self.s.tag_body_skipped(d) {
            self.skip_redefined_tag_body(ds, keyword, kw_loc);
            return;
        }
        if use_ == TagUse::Definition {
            // `struct A::B { ... }`: names in the body are looked up in
            // `A` too ([basic.lookup.unqual]/2, [class.nest]/3).
            let entered = qualifier.map_or(0, |q| self.s.act_on_enter_declarator_scope(q));
            if keyword == TagKeyword::Enum {
                self.parse_enum_body(d);
            } else {
                self.parse_record_body(d, final_loc, ctx == DeclaratorContext::TypeName);
            }
            self.s.act_on_exit_declarator_scope(entered);
        }
        ds.tag = Some(d);
        ds.tag_declares = use_ != TagUse::Reference;
        ds.keyword = keyword;
        let ty = self.s.elaborated_tag_type(d, keyword);
        if let Err(e) = ds.set_base(TypeSpecKind::Named, Some(ty), kw_loc) {
            ds.invalid = true;
            self.spec_error(&e, kw_loc);
        }
        if use_ == TagUse::Definition {
            // `keyword` is a class-key or `enum` here, never `None`.
            let could_be_bitfield = keyword == TagKeyword::Enum && ctx == DeclaratorContext::Member;
            self.check_after_tag_definition(keyword.spelling(), could_be_bitfield);
        }
    }

    /// How the tag specifier just parsed (up to its name and any
    /// class-virt-specifiers) is used, from the next token. In a type-id
    /// (`ctx`, an alias-declaration's `using Y = struct X<int>;`) a `;`
    /// ends the enclosing declaration: the specifier only refers (clang's
    /// `DSC_type_specifier` gives `TUK_Reference`).
    fn tag_use(
        &mut self,
        ds: &DeclSpec,
        is_class: bool,
        final_loc: Option<Loc>,
        ctx: DeclaratorContext,
    ) -> TagUse {
        let use_ = if self.at(Punct::LBrace)
            || (is_class && self.at(Punct::Colon))
            || final_loc.is_some()
        {
            TagUse::Definition
        } else if self.at(Punct::Semi) && ds.storage.is_none() && ctx != DeclaratorContext::TypeName
        {
            if ds.friend.is_some() { TagUse::Friend } else { TagUse::Declaration }
        } else if ds.friend.is_some()
            && ds.storage.is_none()
            && (self.at(Punct::Comma) || self.at(Punct::Ellipsis))
        {
            // An element of a friend-type-specifier-list
            // ([class.friend]/3, P2893): `friend struct A, struct B;`.
            TagUse::Friend
        } else {
            TagUse::Reference
        };
        if use_ == TagUse::Definition && ds.friend.is_some() {
            // [class.friend]/2; clang skips the body and treats the
            // specifier as a reference.
            self.error_here(DiagId::parse_friend_decl_defines_type);
            self.skip_until(&[Punct::Semi], false, false);
            return TagUse::Friend;
        }
        use_
    }

    /// `class-key X<args>` with `X` a class template: a partial or
    /// explicit specialization, an explicit instantiation, or a
    /// reference to a specialization ([class.pre]/1, [dcl.type.elab]/1,
    /// [temp.spec.partial], [temp.expl.spec], [temp.explicit]).
    #[allow(clippy::too_many_arguments)]
    fn parse_class_template_id_tag(
        &mut self,
        ds: &mut DeclSpec,
        keyword: TagKeyword,
        tmpl: DeclId,
        qualifier: Option<DeclId>,
        name_loc: Loc,
        kw_loc: Loc,
        attrs: &[AttrId],
        ctx: DeclaratorContext,
    ) {
        // The template arguments of a partial or explicit specialization's
        // class-head are not access checked ([temp.spec.partial.general]/10,
        // [temp.spec.general]/6); a reference's join the declaration's.
        let level = self.s.delayed_access_level();
        self.s.begin_delayed_access();
        let mut attrs = attrs.to_vec();
        let parsed = self.parse_template_arg_list().map(|a| {
            self.parse_class_head_attrs(ds.storage.is_some(), &mut attrs);
            let final_loc = self.parse_class_virt_specifiers();
            (a, final_loc, self.tag_use(ds, true, final_loc, ctx))
        });
        let declares = parsed
            .as_ref()
            .is_some_and(|p| matches!(p.2, TagUse::Definition | TagUse::Declaration));
        self.s.end_delayed_access_to_enclosing(level, declares);
        let Some(((args, l_angle, r_angle), final_loc, use_)) = parsed else {
            ds.invalid = true;
            let _ = ds.set_base(TypeSpecKind::Named, Some(QualType::ERROR), kw_loc);
            return;
        };
        let r = self.s.act_on_class_template_id_tag(
            keyword,
            tmpl,
            &args,
            qualifier,
            use_,
            name_loc,
            (l_angle, r_angle),
            kw_loc,
        );
        let ty = if let Ok((rec, ty)) = r {
            // A partial or explicit specialization's own attributes
            // ([class.pre]/1); a reference's appertain to nothing.
            if use_ != TagUse::Reference
                && let Some(d) = rec
            {
                self.s.act_on_tag_attrs(d, &attrs, use_);
            }
            if use_ == TagUse::Definition {
                match rec {
                    Some(d) => {
                        // `template<class T> template<class U> struct
                        // Outer<T>::Inner<U*> {...}`: names in the body
                        // are looked up in `Outer<T>` too ([class.nest]/3).
                        let entered =
                            qualifier.map_or(0, |q| self.s.act_on_enter_declarator_scope(q));
                        // Named, so never anonymous.
                        self.parse_record_body(d, final_loc, false);
                        self.s.act_on_exit_declarator_scope(entered);
                    }
                    None => self.skip_class_body(),
                }
            }
            if use_ != TagUse::Reference
                && let Some(d) = rec
            {
                ds.tag = Some(d);
                ds.tag_declares = true;
            }
            ty
        } else {
            if use_ == TagUse::Definition {
                self.skip_class_body();
            }
            ds.invalid = true;
            QualType::ERROR
        };
        ds.keyword = keyword;
        if let Err(e) = ds.set_base(TypeSpecKind::Named, Some(ty), kw_loc) {
            ds.invalid = true;
            self.spec_error(&e, kw_loc);
        }
        if use_ == TagUse::Definition {
            let spelling = match keyword {
                TagKeyword::Struct => "struct",
                TagKeyword::Class => "class",
                _ => "union",
            };
            self.check_after_tag_definition(spelling, false);
        }
    }

    /// A redefinition, diagnosed by sema: its body is skipped and the
    /// declarators it heads get an error type (clang's invalid anonymous
    /// tag; spec/sema.md §4.4).
    fn skip_redefined_tag_body(&mut self, ds: &mut DeclSpec, keyword: TagKeyword, kw_loc: Loc) {
        self.skip_class_body();
        ds.invalid = true;
        let _ = ds.set_base(TypeSpecKind::Named, Some(QualType::ERROR), kw_loc);
        let spelling = match keyword {
            TagKeyword::Struct => "struct",
            TagKeyword::Class => "class",
            TagKeyword::Union => "union",
            _ => "enum",
        };
        self.check_after_tag_definition(spelling, false);
    }

    /// Skips a class body that is not defined (`[: bases] { ... }`).
    fn skip_class_body(&mut self) {
        self.skip_until(&[Punct::LBrace, Punct::Semi], true, false);
        if self.at(Punct::LBrace) {
            self.bump();
            self.skip_until(&[Punct::RBrace], false, true);
        }
    }

    /// class-virt-specifier ([class.pre]/1): `final` after the class
    /// name, recognised only before `{` or `:` (clang's
    /// `isCXX11FinalKeyword` lookahead). Repeats are diagnosed and
    /// dropped. Returns the first `final`.
    fn parse_class_virt_specifiers(&mut self) -> Option<Loc> {
        let mut i = self.pos();
        while self.is_final_tok(self.tok_at(i)) {
            i += 1;
        }
        let j = self.after_cxx11_attrs_at(i)?;
        if i == self.pos()
            || !matches!(Self::punct_of(self.tok_at(j)), Some(Punct::LBrace | Punct::Colon))
        {
            return None;
        }
        let first = self.bump().span.lo;
        while self.pos() < i {
            let loc = self.bump().span.lo;
            let d = self.s.diag(DiagId::parse_duplicate_class_virt_specifier, loc).arg("final");
            self.emit(d);
        }
        if j > i {
            // `struct S final [[x]] { }`: no attribute appertains here
            // ([class.pre]/1); clang reports and drops them.
            let at = self.loc();
            self.prohibit_attrs_at(Some(at));
            let mut dropped = Vec::new();
            self.parse_attributes(&mut dropped);
        }
        Some(first)
    }

    /// `{ enumerator-list }` ([dcl.enum]).
    pub(crate) fn parse_enum_body(&mut self, e: DeclId) {
        let open = self.bump().span.lo;
        self.s.act_on_start_enum_definition(e);
        while !self.at(Punct::RBrace) && !self.at_eof() {
            let Some(name) = self.ident() else {
                // An enumerator cannot start with an attribute
                // ([dcl.enum]/1: they follow its name); like clang, resume
                // at the next enumerator.
                let d = self.s.diag(DiagId::parse_expected, self.loc()).arg("identifier");
                self.emit(d);
                self.skip_until(&[Punct::Comma, Punct::RBrace], false, false);
                if self.eat(Punct::Comma).is_some() {
                    continue;
                }
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
        // clang `ParseEnumBody`: GNU attributes after the `}` appertain to
        // the enumeration, ahead of its underlying type (`packed`).
        if self.at_kw(Kw::Attribute) {
            let mut attrs = Vec::new();
            self.parse_gnu_attr_seq(&mut attrs);
            self.s.act_on_record_trailing_attrs(e, &attrs);
        }
        self.s.act_on_finish_enum_definition(e);
    }

    // -- declarators ------------------------------------------------------------------

    /// declarator / abstract-declarator ([dcl.decl]). Chunks are recorded
    /// innermost first: pointer operators after the declarator they apply
    /// to, suffixes as written.
    pub(crate) fn parse_declarator(&mut self, d: &mut Declarator) {
        if !self.stack_ok() {
            return;
        }
        let loc = self.loc();
        match Self::punct_of(self.tok()) {
            Some(Punct::Star) => {
                self.bump();
                let (quals, nulls) = self.parse_ptr_quals();
                d.attrs.append(&mut self.chunk_decl_attrs);
                self.parse_declarator(d);
                d.push_chunk(Chunk::Pointer { quals, loc }, nulls);
            }
            Some(Punct::Amp) => {
                self.bump();
                let nulls = self.parse_ref_quals();
                self.parse_declarator(d);
                d.push_chunk(Chunk::LRef { loc }, nulls);
            }
            Some(Punct::AmpAmp) => {
                self.bump();
                let nulls = self.parse_ref_quals();
                self.parse_declarator(d);
                d.push_chunk(Chunk::RRef { loc }, nulls);
            }
            _ => {
                if let Some(star) = self.at_member_pointer() {
                    self.parse_member_pointer(d, star, Self::parse_declarator);
                    return;
                }
                self.reject_declarative_splice_scope(d);
                self.parse_direct_declarator(d);
            }
        }
    }

    /// True at `N::*` (a ptr-operator naming a member pointer): the index
    /// of the `*`. The components may be template-ids (`X<T>::*`).
    pub(crate) fn at_member_pointer(&self) -> Option<usize> {
        self.member_pointer_at(self.pos())
    }

    /// [`Parser::at_member_pointer`] at token `i`.
    pub(crate) fn member_pointer_at(&self, mut i: usize) -> Option<usize> {
        let mut any = false;
        if let Some(cc) = self.decltype_scope_end(i) {
            // `decltype(e)::*` (WP-119).
            i = cc + 1;
            any = true;
        } else if self.tok_at(i).kind == TokKind::Punct(Punct::ColonColon) {
            any = self.is_decltype_scope_at(i);
            i += 1;
        }
        loop {
            let k = usize::from(any && self.kw_of(self.tok_at(i)) == Some(Kw::Template));
            if self.ident_of(self.tok_at(i + k)).is_none() {
                break;
            }
            let next = match Self::punct_of(self.tok_at(i + k + 1)) {
                Some(Punct::ColonColon) => i + k + 1,
                Some(Punct::Less) => match self.skip_template_args(i + k + 1) {
                    Some((close, false)) => close + 1,
                    _ => return None,
                },
                _ => break,
            };
            if self.tok_at(next).kind != TokKind::Punct(Punct::ColonColon) {
                break;
            }
            i = next + 1;
            any = true;
        }
        (any && self.tok_at(i).kind == TokKind::Punct(Punct::Star)).then_some(i)
    }

    /// `nested-name-specifier * cv` ([dcl.mptr]) with the `*` at token
    /// `star`, then the rest of the declarator by `rest` (clang's
    /// `ParseDeclaratorInternal`). Sema checks that the qualifier names
    /// a class; after an error in the qualifier the chunk has no
    /// qualifier and the declarator is invalid.
    pub(crate) fn parse_member_pointer(
        &mut self,
        d: &mut Declarator,
        star: usize,
        rest: fn(&mut Self, &mut Declarator),
    ) {
        let loc = self.loc();
        let saved = self.s.set_entering_context(false);
        let qual = self.parse_nested_name_nns();
        self.s.set_entering_context(saved);
        let qual = match qual {
            Ok(Some(q)) if self.pos() == star => Some(q),
            _ => {
                d.invalid = true;
                while self.pos() < star && !self.at_eof() {
                    self.bump();
                }
                None
            }
        };
        if self.pos() == star {
            self.bump();
        }
        let (quals, nulls) = self.parse_ptr_quals();
        d.attrs.append(&mut self.chunk_decl_attrs);
        rest(self, d);
        d.push_chunk(Chunk::MemberPointer { qual, quals, loc }, nulls);
    }

    /// Attributes appertaining to a type, after a ptr-operator or an
    /// array declarator ([dcl.ptr]/1, [dcl.array]/1): kiln attaches no
    /// type attribute there, and reports an `alignas` among them (not a
    /// type attribute, clang `err_attribute_not_type_attr`). A GNU
    /// declaration attribute (`__attribute__((may_alias))`) is kept in
    /// [`Parser::chunk_decl_attrs`] for the declarator.
    fn skip_type_position_attrs(&mut self) {
        let mut attrs = Vec::new();
        self.parse_attributes(&mut attrs);
        self.s.act_on_type_position_attrs(&mut attrs, true);
        attrs.retain(|&a| {
            let n = self.s.ast.attrs.get(a);
            n.syntax == AttrSyntax::Gnu && !n.kind.is_vector_type()
        });
        self.chunk_decl_attrs.extend(attrs);
    }

    /// The attributes after a grouping `(` of a declarator
    /// (`void (__attribute__((noreturn)) *fp)(int)`; clang
    /// `ParseParenDeclarator`): like those after a ptr-operator
    /// ([`Parser::skip_type_position_attrs`]), a GNU declaration
    /// attribute is kept for the declarator, so GNU `noreturn` makes the
    /// declared function type `noreturn` (spec/types.md §5.4, WP-708).
    fn grouping_paren_attrs(&mut self, d: &mut Declarator) {
        let saved = std::mem::take(&mut self.chunk_decl_attrs);
        self.skip_type_position_attrs();
        d.attrs.append(&mut self.chunk_decl_attrs);
        self.chunk_decl_attrs = saved;
    }

    /// The lookahead offset past GNU `__attribute__((...))` specifiers
    /// starting at offset `n`: clang's `ParseParenDeclarator` eats them
    /// before deciding between a grouping paren and a parameter list,
    /// so `void g(void (__attribute__((noreturn)) *p)(int))` groups.
    fn past_gnu_attrs(&self, n: usize) -> usize {
        let base = self.pos();
        let mut i = base + n;
        while self.kw_of(self.tok_at(i)) == Some(Kw::Attribute) {
            let Some(j) = self.skip_balanced(i + 1) else { break };
            i = j;
        }
        i - base
    }

    /// The cv-qualifiers, nullability qualifiers and attributes after a
    /// `*` ([dcl.ptr]/1; clang's `ParseTypeQualifierListOpt`).
    pub(crate) fn parse_ptr_quals(&mut self) -> (Quals, Vec<NullSpec>) {
        self.chunk_decl_attrs.clear();
        let mut q = Quals::NONE;
        let mut nulls = Vec::new();
        loop {
            let (add, sp) = match self.kw() {
                Some(Kw::Const) => (Quals::CONST, "const"),
                Some(Kw::Volatile) => (Quals::VOLATILE, "volatile"),
                Some(Kw::Restrict) => (Quals::RESTRICT, "restrict"),
                Some(k) if nullability_kw(k).is_some() => {
                    let loc = self.bump().span.lo;
                    if let Some(kind) = nullability_kw(k) {
                        nulls.push(NullSpec { kind, loc });
                    }
                    continue;
                }
                _ if self.at_attr() => {
                    self.skip_type_position_attrs();
                    continue;
                }
                _ => return (q, nulls),
            };
            let loc = self.bump().span.lo;
            if q.contains(add) {
                let d = self.s.diag(DiagId::parse_duplicate_decl_spec, loc).arg(sp);
                self.emit(d);
            }
            q |= add;
        }
    }

    /// C99's `static` and type-qualifier-list in an array declarator's
    /// brackets (clang `ParseBracketDeclarator`): both are errors in C++
    /// ("... is a C99 feature" on a parameter's outermost array, else
    /// "... outside of function prototype", which invalidates the
    /// declarator), at the `[`. A nullability qualifier there is
    /// a clang extension: it applies to the array, which only the
    /// parameter's outermost one may carry (it moves to the decayed
    /// pointer; spec/sema.md §4.1.1).
    fn parse_bracket_quals(&mut self, d: &mut Declarator, open: Loc) {
        let mut stat = self.at_kw(Kw::Static);
        if stat {
            self.bump();
        }
        let (quals, nulls) = self.parse_ptr_quals();
        if !stat && !quals.is_empty() && self.at_kw(Kw::Static) {
            self.bump();
            stat = true;
        }
        let i = d.chunks.len();
        d.nullability.extend(nulls.into_iter().map(|n| (i, n)));
        if !stat && quals.is_empty() {
            return;
        }
        let param = d.ctx == DeclaratorContext::Prototype && i == 0;
        let id = match (param, stat) {
            (true, _) => DiagId::parse_c99_array_usage_cxx,
            (false, true) => DiagId::parse_array_static_outside_prototype,
            (false, false) => DiagId::parse_array_qual_outside_prototype,
        };
        let mut diag = self.s.diag(id, open);
        if param {
            diag = diag.arg(u8::from(stat));
        } else {
            d.invalid = true;
        }
        self.emit(diag);
    }

    /// The attributes, nullability qualifiers and GNU `__restrict`s after
    /// a `&` or `&&` ([dcl.ref]/1; a nullability qualifier there is
    /// diagnosed by sema). `__restrict` / `__restrict__` on a reference is
    /// a GNU extension clang accepts silently (`ParseTypeQualifierListOpt`
    /// then `DeclaratorChunk::getReference`); kiln drops it, as it is only
    /// an aliasing hint (spec/parse.md "GNU `__restrict` on references").
    pub(crate) fn parse_ref_quals(&mut self) -> Vec<NullSpec> {
        let mut nulls = Vec::new();
        loop {
            if self.at_kw(Kw::Restrict) {
                self.bump();
            } else if let Some(kind) = self.kw().and_then(nullability_kw) {
                let loc = self.bump().span.lo;
                nulls.push(NullSpec { kind, loc });
            } else if self.at_attr() {
                self.skip_attributes();
            } else {
                return nulls;
            }
        }
    }

    /// The `...` before a declarator-id ([`Self::parse_direct_declarator`]):
    /// whether it was before a ptr-operator, whose declarator it parsed.
    fn parse_leading_declarator_ellipsis(&mut self, d: &mut Declarator, in_catch: bool) -> bool {
        // `... declarator-id` / an abstract pack declarator
        // ([dcl.decl.general]/4, [dcl.fct]/14).
        if d.ctx == DeclaratorContext::Prototype && self.at(Punct::Ellipsis) {
            let named = self.ident_of(self.peek(1)).is_some()
                || matches!(Self::punct_of(self.peek(1)), Some(Punct::LSquare | Punct::LParen));
            if named || self.pack_hint {
                d.ellipsis = Some(self.bump().span.lo);
            }
        } else if in_catch && self.at(Punct::Ellipsis) {
            d.ellipsis = Some(self.bump().span.lo);
        } else if matches!(
            d.ctx,
            DeclaratorContext::File
                | DeclaratorContext::Member
                | DeclaratorContext::Block
                | DeclaratorContext::Condition
                | DeclaratorContext::ForInit
        ) && self.at(Punct::Ellipsis)
        {
            // [temp.variadic]/5: only a parameter declares a pack, but a
            // `...` before any declarator-id is taken (clang
            // `ParseDirectDeclarator`) and Sema drops it with "only
            // function and template parameters can be parameter packs".
            // Before a ptr-operator it is misplaced too (`int... *p`).
            let ellipsis = self.bump().span.lo;
            if matches!(Self::punct_of(self.tok()), Some(Punct::Star | Punct::Amp | Punct::AmpAmp))
                || self.at_member_pointer().is_some()
            {
                self.parse_declarator(d);
                self.misplaced_declarator_ellipsis(d, ellipsis);
                d.ellipsis.get_or_insert(ellipsis);
                return true;
            }
            d.ellipsis = Some(ellipsis);
        }
        false
    }

    /// [dcl.meaning]/1, [expr.prim.id.qual]/3: a splice-scope-specifier
    /// never begins a declarative nested-name-specifier. The declaration
    /// is dropped; the name after the qualifier is parsed for recovery.
    fn reject_declarative_splice_scope(&mut self, d: &mut Declarator) {
        if d.ctx != DeclaratorContext::TypeName
            && self.at_splice()
            && let Some(cc) = self.decltype_scope_end(self.pos())
        {
            self.s.error(DiagId::sema_splice_declarative_scope, self.loc());
            d.invalid = true;
            d.dropped = true;
            self.s.discard_spec_headers();
            while self.pos() <= cc {
                self.bump();
            }
            self.skip_nested_name_rest();
        }
    }

    /// noptr-declarator ([dcl.decl]): the declarator-id or `( declarator )`
    /// and the array and function suffixes.
    #[allow(clippy::too_many_lines, reason = "one arm per declarator-id form")]
    fn parse_direct_declarator(&mut self, d: &mut Declarator) {
        let abstract_ok =
            matches!(d.ctx, DeclaratorContext::Prototype | DeclaratorContext::TypeName);
        // An exception-declaration may be abstract ([except.pre]/1), so
        // `catch (int(...))` declares a function type (clang
        // `DeclaratorContext::CXXCatch`).
        let in_catch = std::mem::take(&mut self.catch_declarator);
        if self.parse_leading_declarator_ellipsis(d, in_catch) {
            return;
        }
        if self.at_decomposition_declarator(d) {
            self.parse_decomposition_declarator(d);
            return;
        }
        // The scopes a qualified declarator-id enters for the rest of the
        // declarator.
        let mut entered = 0;
        if (self.ident().is_some() || self.at(Punct::ColonColon))
            && d.ctx != DeclaratorContext::TypeName
        {
            let last_qual = self.last_qualifier_name();
            let global = self.at(Punct::ColonColon);
            let saved = self.s.set_entering_context(true);
            let qual = self.parse_nested_name();
            self.s.set_entering_context(saved);
            let qual = qual.unwrap_or_else(|()| {
                // An invalid qualifier declares nothing (clang's
                // `HandleDeclarator` returns no declaration).
                d.invalid = true;
                d.dropped = true;
                self.s.discard_spec_headers();
                self.skip_nested_name_rest();
                None
            });
            d.global_qualifier = global && qual.is_some();
            if self.at(Punct::Tilde) || self.at_kw(Kw::Operator) {
                d.qualifier = qual;
                entered = qual.map_or(0, |q| self.s.act_on_enter_declarator_scope(q));
                self.parse_special_declarator_id(d);
            } else if let Some(n) = self.ident() {
                let ctor = self.names_constructor(n, last_qual, d.ctx);
                let guide = if ctor
                    || d.typed
                    || qual.is_some()
                    || !matches!(d.ctx, DeclaratorContext::File | DeclaratorContext::Member)
                {
                    None
                } else {
                    self.deduction_guide_at(self.pos())
                };
                let l = self.bump().span.lo;
                if ctor {
                    d.special = Some((SpecialName::Constructor(n), l));
                } else if let Some(t) = guide {
                    d.special = Some((SpecialName::DeductionGuide(t), l));
                } else {
                    d.name = Some((n, l));
                }
                d.qualifier = qual;
                entered = qual.map_or(0, |q| self.s.act_on_enter_declarator_scope(q));
                if !ctor
                    && self.at(Punct::Less)
                    && (self.s.has_pending_head()
                        || self.s.has_explicit_spec_head()
                        || self.s.has_explicit_inst_head()
                        || d.friend)
                {
                    self.parse_specialization_declarator_id(d, n, l);
                }
            } else if qual.is_some() {
                self.error_here(DiagId::parse_expected_unqualified_id);
                d.invalid = true;
            }
            self.parse_declarator_id_attrs(d);
        } else if self.at(Punct::LParen)
            && !((abstract_ok || in_catch) && self.is_param_list_start(self.past_gnu_attrs(1)))
        {
            // Like clang's ParseParenDeclarator: grouping is in effect
            // only for the inner declarator, then restored.
            let had_grouping = d.grouping;
            d.grouping = true;
            // A `...` outside the parentheses is misplaced (`int ...(z)`;
            // clang `ParseParenDeclarator`).
            let ellipsis =
                if d.ctx == DeclaratorContext::Prototype { None } else { d.ellipsis.take() };
            let open = self.bump().span.lo;
            self.grouping_paren_attrs(d);
            self.parse_declarator(d);
            let close = self.expect_closing(Punct::RParen, Punct::LParen, open);
            d.grouping = had_grouping;
            if let Some(e) = ellipsis {
                self.misplaced_declarator_ellipsis(d, e);
                d.ellipsis.get_or_insert(e);
            }
            d.outer_paren = Some((d.chunks.len(), open));
            d.parens.push((d.chunks.len(), open, close));
            self.discard_prohibited_cxx11_attrs();
        } else if self.at_kw(Kw::Operator)
            && d.ctx != DeclaratorContext::TypeName
            && matches!(self.kw_of(self.peek(1)), Some(Kw::New | Kw::Delete))
        {
            // WP-061a's operator-function-ids for allocation functions.
            self.parse_new_delete_operator_id(d);
            self.parse_declarator_id_attrs(d);
        } else if (self.at(Punct::Tilde) || self.at_kw(Kw::Operator))
            && d.ctx != DeclaratorContext::TypeName
        {
            self.parse_special_declarator_id(d);
            self.parse_declarator_id_attrs(d);
        } else if abstract_ok && d.abstract_at.is_none() {
            d.abstract_at = Some(self.loc());
        }
        self.parse_declarator_suffixes(d, abstract_ok);
        self.s.act_on_exit_declarator_scope(entered);
    }

    /// `declarator-id attribute-specifier-seq(opt)` ([dcl.decl.general]/4,
    /// [class.mem.general]/1):
    /// the attributes appertain to the declared entity, for every kind of
    /// declarator-id (`f [[a]] (int)`, `operator() [[a]] (int)`,
    /// `~S [[a]] ()`).
    fn parse_declarator_id_attrs(&mut self, d: &mut Declarator) {
        // clang `ParseDirectDeclarator` (`MaybeParseCXX11Attributes`):
        // `alignas` too, so a bit-width may follow (`int b alignas(8) : 3`).
        if self.at_cxx11_attr() || self.at_kw(Kw::Alignas) {
            self.parse_attributes(&mut d.attrs);
        }
    }

    /// The array and function declarator suffixes after a declarator-id
    /// or parenthesized declarator ([dcl.array], [dcl.fct]).
    fn parse_declarator_suffixes(&mut self, d: &mut Declarator, abstract_ok: bool) {
        loop {
            if self.at(Punct::LSquare) && !self.at_cxx11_attr() {
                let open = self.bump().span.lo;
                self.parse_bracket_quals(d, open);
                let mut size = None;
                if !self.at(Punct::RSquare) {
                    // An array bound is manifestly constant-evaluated.
                    self.s.enter_constant_evaluated();
                    size = self.parse_assignment_expr();
                    self.s.exit_constant_evaluated(size);
                    if size.is_none() {
                        d.invalid = true;
                        self.skip_until(&[Punct::RSquare], false, false);
                    }
                }
                self.expect_closing(Punct::RSquare, Punct::LSquare, open);
                // clang `ParseBracketDeclarator` takes only C++11
                // attributes here; GNU ones after the declarator apply to
                // the whole declared type (`int b[2] __attribute__((..))`).
                if self.at_cxx11_attr() || self.at_kw(Kw::Alignas) {
                    self.skip_type_position_attrs();
                }
                d.chunks.push(Chunk::Array { size, loc: open });
            } else if self.at(Punct::LParen) {
                // [dcl.ambig.res]/1: after a name that could take a direct
                // initializer, the parentheses are a function declarator
                // only if they could be a parameter clause. A member
                // declarator has no direct-initializer (`direct_init` is
                // unset), so there `(` always starts its parameters.
                let mut ambiguous = false;
                if !abstract_ok && d.direct_init && !d.grouping && d.name.is_some() {
                    let (function, amb) = self.is_cxx_function_declarator(d.name.map(|n| n.0));
                    if !function {
                        break;
                    }
                    ambiguous = amb;
                }
                self.parse_function_suffix(d, ambiguous);
                // [dcl.fct]/1: an attribute-specifier-seq after the
                // parameter clause appertains to the function type.
                // GNU attributes after it are the declarator's (parsed
                // by the caller; clang `ParseFunctionDeclarator`).
                if self.at_cxx11_attr() || self.at_kw(Kw::Alignas) {
                    let mut attrs = Vec::new();
                    self.parse_cxx11_attr_seq(&mut attrs);
                    let lb = self.s.take_fn_type_lifetimebound(&mut attrs);
                    self.s.act_on_type_position_attrs(&mut attrs, true);
                    d.attrs.extend(attrs);
                    d.attrs.extend(lb);
                }
            } else {
                break;
            }
        }
    }

    /// True at the `[` of a structured binding declaration's
    /// sb-identifier-list ([dcl.pre]/6; clang's
    /// `mayHaveDecompositionDeclarator` and the lookahead of
    /// `ParseDecompositionDeclarator`): a declaration, condition or
    /// for-init context, an identifier followed by `,`, `]`, `alignas`,
    /// an identifier or `[`, or an empty list before `=` or `{`. A C++26
    /// binding pack's `...` (P1061) is accepted too.
    fn at_decomposition_declarator(&self, d: &Declarator) -> bool {
        if !matches!(
            d.ctx,
            DeclaratorContext::File
                | DeclaratorContext::Block
                | DeclaratorContext::ForInit
                | DeclaratorContext::Condition
        ) || d.name.is_some()
            || !self.at(Punct::LSquare)
            || self.at_cxx11_attr()
        {
            return false;
        }
        let mut i = 1;
        if Self::punct_of(self.peek(1)) == Some(Punct::Ellipsis) {
            i = 2;
        }
        if self.ident_of(self.peek(i)).is_some() {
            let next = self.peek(i + 1);
            return matches!(
                Self::punct_of(next),
                Some(Punct::Comma | Punct::RSquare | Punct::LSquare)
            ) || self.ident_of(next).is_some()
                || self.kw_of(next) == Some(Kw::Alignas);
        }
        i == 1
            && Self::punct_of(self.peek(1)) == Some(Punct::RSquare)
            && matches!(Self::punct_of(self.peek(2)), Some(Punct::Equal | Punct::LBrace))
    }

    /// `[ sb-identifier-list ]` (clang's `ParseDecompositionDeclarator`).
    fn parse_decomposition_declarator(&mut self, d: &mut Declarator) {
        let lbracket = self.bump().span.lo;
        let mut ids: Vec<BindingId> = Vec::new();
        let mut closed = true;
        while !self.at(Punct::RSquare) {
            if !ids.is_empty() && self.eat(Punct::Comma).is_none() {
                if self.ident().is_some() {
                    let d = self.s.diag(DiagId::parse_expected, self.prev_end()).arg("','");
                    self.emit(d);
                } else {
                    self.error_here(DiagId::parse_expected_comma_or_rsquare);
                }
                if self.ident().is_none() {
                    self.skip_until(&[Punct::RSquare, Punct::Comma], true, false);
                    if self.eat(Punct::Comma).is_none() {
                        closed = false;
                        break;
                    }
                }
            }
            if self.at_cxx11_attr() {
                self.skip_attributes();
            }
            let ellipsis = self.eat(Punct::Ellipsis);
            if let Some(e) = ellipsis
                && self.std < Std::Cxx26
            {
                // P1061: clang `ext_cxx_binding_pack`.
                let w = self.s.diag(DiagId::parse_ext_binding_pack, e);
                self.emit(w);
            }
            let Some(name) = self.ident() else {
                let d = self.s.diag(DiagId::parse_expected, self.loc()).arg("identifier");
                self.emit(d);
                closed = false;
                break;
            };
            let loc = self.bump().span.lo;
            let mut attrs = Vec::new();
            if self.at_cxx11_attr() {
                self.parse_attributes(&mut attrs);
            }
            ids.push(BindingId { name, loc, ellipsis, attrs });
        }
        let rbracket = if closed {
            self.bump().span.lo
        } else {
            self.skip_until(&[Punct::RSquare], true, false);
            self.eat(Punct::RSquare).unwrap_or(self.loc())
        };
        if closed && ids.is_empty() {
            self.s.error(DiagId::parse_decomp_decl_empty, rbracket);
        }
        d.decomp = Some(Decomposition { lbracket, rbracket, ids });
    }

    /// `name<args>` as the declarator-id after a template head, when
    /// `name` is a function or variable template: an explicit or partial
    /// specialization ([temp.expl.spec]/1, [temp.spec.partial]/1). The
    /// arguments are parsed and recorded on the declarator, or it is
    /// dropped ([`Sema::act_on_specialization_declarator_id`]).
    fn parse_specialization_declarator_id(&mut self, d: &mut Declarator, name: Symbol, loc: Loc) {
        let t = match self.s.classify_name(name, d.qualifier) {
            NameClass::FunctionTemplate(t) | NameClass::VarTemplate(t) => t,
            // `friend void f<>(int);` with no function template `f`
            // ([temp.friend]/1.3, [temp.names]/3.3).
            NameClass::Value(_) | NameClass::Undeclared if d.friend => {
                if self.parse_template_arg_list().is_none() {
                    d.invalid = true;
                    return;
                }
                self.s.act_on_friend_spec_without_template(DeclName::ident(name), d.qualifier, loc);
                d.dropped = true;
                return;
            }
            _ => return,
        };
        let Some((args, _, r)) = self.parse_template_arg_list() else {
            d.invalid = true;
            return;
        };
        // A template-id declarator-id takes no direct-initializer (clang
        // `Declarator::mayBeFollowedByCXXDirectInit` is false for a
        // non-identifier name): `template int v<char>(1);` is a function
        // declarator with a bad parameter clause, as in clang.
        d.direct_init = false;
        d.spec_id = self.s.act_on_specialization_declarator_id(t, &args, loc, r);
        if d.spec_id.is_none() {
            d.dropped = true;
        }
    }

    /// True if the token `n` ahead (after a `(` where an abstract
    /// declarator may start) begins a parameter-declaration-clause rather
    /// than grouping parentheses (clang's `ParseParenDeclarator`): `)`,
    /// `...)`, a decl-specifier or `[[`. An undeclared name followed by an
    /// identifier is taken as a misspelled type. A class template's
    /// injected-class-name is a type name ([temp.local]/1), so
    /// `void(CT)` in its class is a function type ([dcl.ambig.res]/3).
    pub(crate) fn is_param_list_start(&self, n: usize) -> bool {
        let t = self.peek(n);
        match t.kind {
            TokKind::Punct(Punct::RParen) => true,
            // `(...)`; `(...x)` groups a pack's declarator-id
            // ([dcl.decl.general]/4).
            TokKind::Punct(Punct::Ellipsis) => {
                self.peek(n + 1).kind == TokKind::Punct(Punct::RParen)
            }
            TokKind::Punct(Punct::LSquare) => {
                self.peek(n + 1).kind == TokKind::Punct(Punct::LSquare)
            }
            TokKind::Punct(Punct::ColonColon) => self.names_type_at(self.pos() + n),
            TokKind::Identifier(s) => match self.kw_of(t) {
                // An explicit object parameter ([dcl.fct]/6) starts with
                // `this` followed by its decl-specifiers.
                // `T *p(this);` in a member function body stays an initializer.
                Some(Kw::This) => {
                    self.peek(n + 1).kind != TokKind::Punct(Punct::RParen)
                        && self.is_param_list_start(n + 1)
                }
                Some(k) => is_decl_specifier_kw(k),
                None if self.names_type_at(self.pos() + n) => true,
                None => match self.scan_name(self.pos() + n) {
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

    /// Does the (possibly qualified) name at token `i` name a type: a
    /// type name, or a class template's injected-class-name in its own
    /// scope not followed by `<` ([temp.local]/1; clang annotates both
    /// as `annot_typename`)?
    pub(crate) fn names_type_at(&self, i: usize) -> bool {
        match self.scan_name(i) {
            Some((_, NameClass::Type(..), _)) => true,
            Some((end, NameClass::TypeTemplate(d), _)) => {
                end == i + 1
                    && Self::punct_of(self.tok_at(end)) != Some(Punct::Less)
                    && self.s.names_injected_class(d)
            }
            _ => false,
        }
    }

    /// `( parameter-declaration-clause )` ([dcl.fct]) and what may follow it.
    /// `ambiguous`: the parentheses could also have been a direct
    /// initializer ([dcl.ambig.res]/1).
    fn parse_function_suffix(&mut self, d: &mut Declarator, ambiguous: bool) {
        // Only a member declarator whose function suffix applies to the
        // name directly saves its default arguments and noexcept.
        let delayable = d.ctx == DeclaratorContext::Member && d.chunks.is_empty();
        let delay_args = delayable && self.late_fn.delay_args;
        let open = self.bump().span.lo;
        self.s.act_on_start_prototype();
        let (params, variadic) = self.parse_param_clause(d, delay_args, DeclSite::Plain);
        let rparen = self.expect_closing(Punct::RParen, Punct::LParen, open);
        let (exc, quals, ref_qual, trailing) = self.parse_function_qualifiers(d, delayable);
        self.s.act_on_end_prototype();
        let params = self.s.act_on_param_list(params);
        d.chunks.push(Chunk::Function {
            params,
            variadic,
            loc: open,
            rparen,
            exc,
            quals,
            ref_qual,
            trailing,
            ambiguous,
        });
    }

    /// The parameter-declaration-clause of a function declarator
    /// ([dcl.fct]/1) after the `(` (and `act_on_start_prototype`), up to
    /// the `)`: the parameters and whether `...` ends the list. With
    /// `delay_args`, default arguments are saved in `late_fn`.
    /// An abstract declarator's identifier location is where the name
    /// would be (clang `ParseDirectDeclarator`): the token at its
    /// innermost position (`(*)` gives the `)`), else the first array
    /// bound, else the token after the declarator. Named declarators keep
    /// theirs.
    pub(crate) fn set_abstract_loc(&self, d: &mut Declarator) {
        if d.name.is_none() && d.special.is_none() && d.decomp.is_none() {
            d.loc = d.abstract_at.unwrap_or_else(|| {
                d.chunks
                    .iter()
                    .find_map(|c| match c {
                        Chunk::Array { loc, .. } => Some(*loc),
                        _ => None,
                    })
                    .unwrap_or_else(|| self.loc())
            });
        }
    }

    pub(crate) fn parse_param_clause(
        &mut self,
        d: &mut Declarator,
        delay_args: bool,
        site: DeclSite,
    ) -> (Vec<ParamInfo>, bool) {
        let mut params = Vec::new();
        let mut variadic = false;
        let fn_decl = is_function_declaration_clause(d, site);
        loop {
            if self.at(Punct::RParen) || self.at_eof() {
                break;
            }
            if self.eat(Punct::Ellipsis).is_some() {
                variadic = true;
                break;
            }
            let start = self.loc();
            // explicit-object-parameter-declaration ([dcl.fct]/3, P0847).
            let this_loc = self.eat_kw(Kw::This);
            if let (Some(l), true) = (this_loc, self.std < Std::Cxx23) {
                // clang `err_cxx20_deducing_this`; parsing continues as C++23.
                let m = self.s.diag(DiagId::parse_explicit_object_pre_cxx23, l);
                self.emit(m);
            }
            // [temp.res.general]/4.2: only a member-declaration's, a
            // qualified declarator-id's, a lambda's or a requires-
            // expression's parameters take `X<T>::m` as a type. So does
            // the first parameter of a namespace-scope deduction guide, as
            // in clang (its constructor-declarator lookahead annotates
            // that one only).
            let needs_typename = site == DeclSite::Plain
                && matches!(d.ctx, DeclaratorContext::File | DeclaratorContext::Block)
                && d.qualifier.is_none()
                && !(matches!(d.special, Some((SpecialName::DeductionGuide(_), _)))
                    && params.is_empty());
            let mode =
                if needs_typename { ParamTypename::Required } else { ParamTypename::Implicit };
            let saved = std::mem::replace(&mut self.param_typename, mode);
            // clang disambiguates the first parameter of a non-member
            // declarator tentatively ([dcl.ambig.res]/1).
            let first = Some(needs_typename && params.is_empty());
            let saved_recovery = std::mem::replace(&mut self.param_recovery, first);
            let mut ds = self.parse_decl_specifiers(DeclaratorContext::Prototype);
            self.param_recovery = saved_recovery;
            if self.param_typename == ParamTypename::Missing {
                d.missing_typename = true;
            }
            self.param_typename = saved;
            if std::mem::take(&mut self.param_abort) {
                d.invalid = true;
                break;
            }
            if ds.is_empty() {
                // clang `err_missing_param`: the clause ends here and the
                // caller's missing `)` skips the rest; the declarator
                // stays valid (`template int v<char>(1);` then names a
                // function). Kept out of deduplication so that `)`, also
                // `parse_expected` at this token, is reported too.
                let e = self
                    .s
                    .diag(DiagId::parse_expected, self.loc())
                    .arg("parameter declarator")
                    .allow_duplicate();
                self.emit(e);
                break;
            }
            let mut pd = Declarator::new(DeclaratorContext::Prototype, start);
            pd.site = site;
            pd.explicit_object = this_loc;
            let placeholder = matches!(ds.base, TypeSpecKind::Auto);
            let hint = placeholder
                || (ds.base == TypeSpecKind::Named
                    && ds.named.is_some_and(|t| self.s.has_unexpanded_pack(t)));
            let saved = std::mem::replace(&mut self.pack_hint, hint);
            self.parse_declarator(&mut pd);
            self.pack_hint = saved;
            self.set_abstract_loc(&mut pd);
            self.param_placeholders(&mut ds, &mut pd, start, fn_decl, site, d.qualifier);
            if self.at_attr() {
                self.parse_attributes(&mut pd.attrs);
            }
            self.misplaced_param_pack_ellipsis(&ds, &mut pd, site);
            let mut p = self.s.act_on_param_declarator(&ds, &pd);
            // The point of declaration: before the default argument
            // ([basic.scope.pdecl]/1).
            self.s.act_on_prototype_param(&mut p);
            if self.at(Punct::Equal) {
                if delay_args {
                    // Complete-class context ([class.mem.general]/7):
                    // replayed after the class (spec/parse.md §2.4).
                    let eq = self.loc();
                    let toks = self.capture_default_arg();
                    let arg = toks.second_loc(eq);
                    p.default_arg = DefaultArg::Unparsed(eq, arg);
                    self.late_fn.default_args.push((params.len(), toks));
                } else {
                    self.parse_default_argument(&mut p);
                }
            }
            p.range = (start, self.prev_end());
            let pack_like = pd.ellipsis.is_some() || self.s.has_unexpanded_pack(p.ty);
            params.push(p);
            if let Some(ellipsis) = self.eat(Punct::Ellipsis) {
                // [dcl.fct]/3, [depr.ellipsis.comma] (C++26, P3176): a
                // varargs ellipsis without a preceding comma (clang
                // `warn_deprecated_missing_comma_before_ellipsis`).
                if self.std >= Std::Cxx26 {
                    let w = self
                        .s
                        .diag(DiagId::parse_deprecated_missing_comma_before_ellipsis, ellipsis);
                    self.emit(w);
                }
                if pack_like {
                    self.misplaced_ellipsis_vararg(&pd, ellipsis);
                }
                variadic = true;
                break;
            }
            if self.eat(Punct::Comma).is_none() {
                break;
            }
        }
        (params, variadic)
    }

    /// A `...` after parameter declarator `pd` that names an unexpanded
    /// pack is a misplaced pack ellipsis ([dcl.decl.general]/4: the `...`
    /// of a declarator-id or abstract-pack-declarator comes first):
    /// clang 22's "'...' must immediately precede declared identifier"
    /// (named) or "must be innermost component of anonymous pack
    /// declaration" (abstract), and the parameter becomes the pack (`Ts
    /// a...`, `Ts (*)()...`, `Ts......a`). As in clang
    /// (`DiagnoseMisplacedEllipsisInDeclarator` in the parser), this is
    /// decided syntactically and reported before the parameter's Sema
    /// action, so it precedes any type diagnostic of the parameter, and a
    /// parameter whose type became invalid is then "type 'int' of function
    /// parameter pack does not contain any unexpanded parameter packs".
    /// Not when `)` follows a declarator that already has its `...`
    /// (`Ts......`), or in a lambda one without (`[](Ts a...)`): those are
    /// the varargs `...` of -Wambiguous-ellipsis. A placeholder type names
    /// no pack yet.
    fn misplaced_param_pack_ellipsis(
        &mut self,
        ds: &DeclSpec,
        pd: &mut Declarator,
        site: DeclSite,
    ) {
        if !self.at(Punct::Ellipsis)
            || matches!(ds.base, TypeSpecKind::Auto | TypeSpecKind::DecltypeAuto)
            || !self.s.declarator_has_unexpanded_pack(ds, pd)
        {
            return;
        }
        let before_rparen = self.peek_is(1, Punct::RParen);
        if before_rparen && (pd.ellipsis.is_some() || site == DeclSite::LambdaParam) {
            return;
        }
        let ellipsis = self.loc();
        self.bump();
        self.misplaced_declarator_ellipsis(pd, ellipsis);
        if pd.ellipsis.is_none() {
            pd.ellipsis = Some(ellipsis);
        }
    }

    /// Diagnoses the misplaced `...` at `ellipsis` after declarator `pd`
    /// (clang `DiagnoseMisplacedEllipsis`).
    pub(crate) fn misplaced_declarator_ellipsis(&mut self, pd: &Declarator, ellipsis: Loc) {
        let anonymous = if pd.name.is_some() { "0" } else { "1" };
        let e =
            self.s.diag(DiagId::parse_misplaced_ellipsis_in_declaration, ellipsis).arg(anonymous);
        self.emit(e);
    }

    /// [dcl.fct]/3 (P3176): the varargs `...` at `ellipsis` follows a
    /// parameter that declares, or looks like it was meant to declare, a
    /// function parameter pack (`Ts......`): clang's
    /// `-Wambiguous-ellipsis` with a note at the pack's own `...` (or
    /// where it would go) and the insert-a-comma note.
    fn misplaced_ellipsis_vararg(&mut self, pd: &Declarator, ellipsis: Loc) {
        let mut w = self
            .s
            .diag(DiagId::parse_misplaced_ellipsis_vararg, ellipsis)
            .arg(if pd.ellipsis.is_some() { "1" } else { "0" });
        let note = if let Some(at) = pd.ellipsis {
            self.s.diag(DiagId::note_misplaced_ellipsis_vararg_existing_ellipsis, at)
        } else {
            let (sel, at) = pd.name.map_or(("1", pd.loc), |(_, at)| ("0", at));
            self.s.diag(DiagId::note_misplaced_ellipsis_vararg_add_ellipsis, at).arg(sel)
        };
        w = w.with_note(note);
        let w =
            w.with_note(self.s.diag(DiagId::note_misplaced_ellipsis_vararg_add_comma, ellipsis));
        self.emit(w);
    }

    /// The placeholders of parameter `ds pd` (which started at `start`):
    /// an abbreviated function template's invented parameter, or the
    /// misplaced-placeholder error ([dcl.fct]/22); `fn_decl` as
    /// [`is_function_declaration_clause`] says, `qualifier` the function
    /// declarator's.
    fn param_placeholders(
        &mut self,
        ds: &mut DeclSpec,
        pd: &mut Declarator,
        start: Loc,
        fn_decl: bool,
        site: DeclSite,
        qualifier: Option<DeclId>,
    ) {
        let decltype_auto = ds.base == TypeSpecKind::DecltypeAuto;
        let placeholder = ds.base == TypeSpecKind::Auto;
        // A decl-specifier `auto` of a declarator with a trailing
        // return type is that function type's `auto`, not a
        // placeholder for the parameter ([dcl.fct]/2, /22).
        let trailing =
            pd.chunks.iter().any(|c| matches!(c, Chunk::Function { trailing: Some(_), .. }));
        let at = ds.type_loc.unwrap_or(start);
        if (placeholder || decltype_auto) && !ds.invalid && !trailing {
            if let Some(why) = kiln_sema::Sema::param_placeholder_site(decltype_auto, fn_decl, site)
            {
                let int = self.s.act_on_misplaced_param_placeholder(at, decltype_auto, why);
                ds.auto_constraint = None;
                ds.base = TypeSpecKind::Named;
                ds.named = Some(int);
            } else {
                // An abbreviated function template's invented
                // parameter ([dcl.fct]/22).
                let constraint = ds.auto_constraint.take().map(|(c, a)| (c, a, ds.constraint_loc));
                let pname = pd.name.map(|(n, _)| n);
                let ty = self.s.act_on_invented_template_param(
                    at,
                    pd.ellipsis.is_some(),
                    constraint,
                    pname,
                    qualifier,
                );
                ds.base = TypeSpecKind::Named;
                ds.named = Some(ty);
            }
        }
        if trailing && !ds.invalid {
            self.trailing_return_placeholders(pd, at, fn_decl, site, qualifier);
        }
    }

    /// The placeholder in a trailing return type of parameter declarator
    /// `pd` (clang's `GetFullTypeForDeclarator`): `decltype(auto)` and an
    /// `auto` of a clause that declares no function are reported at the
    /// decl-specifier `at` and become `int`; any other `auto` is replaced
    /// by an invented template parameter ([dcl.fct]/22: `void f(auto (*p)()
    /// -> auto)` is an abbreviated function template). Only placeholders
    /// under pointer and reference declarators are rewritten.
    fn trailing_return_placeholders(
        &mut self,
        pd: &mut Declarator,
        at: Loc,
        fn_decl: bool,
        site: DeclSite,
        qualifier: Option<DeclId>,
    ) {
        let pname = pd.name.map(|(n, _)| n);
        let pack = pd.ellipsis.is_some();
        let cloc = pd.trailing_constraint;
        for c in &mut pd.chunks {
            let Chunk::Function { trailing: Some((t, tloc)), .. } = c else { continue };
            let Some((decltype_auto, constraint)) = self.s.trailing_return_placeholder(*t) else {
                continue;
            };
            let with = if let Some(why) =
                kiln_sema::Sema::param_placeholder_site(decltype_auto, fn_decl, site)
            {
                self.s.act_on_misplaced_param_placeholder(at, decltype_auto, why)
            } else {
                let constraint = constraint.map(|(c, a)| (c, a, cloc));
                let loc = if tloc.is_valid() { *tloc } else { at };
                self.s.act_on_invented_template_param(loc, pack, constraint, pname, qualifier)
            };
            *t = self.s.replace_trailing_return_placeholder(*t, with);
        }
    }

    /// `= initializer-clause` after a parameter-declaration
    /// ([dcl.fct.default]/1). At the `=`.
    fn parse_default_argument(&mut self, p: &mut ParamInfo) {
        p.default_loc = self.loc();
        self.bump();
        // clang rejects a default argument that starts with `({` outright
        // (`err_stmt_expr_in_default_arg`), skipping to the next `,`/`)`.
        if self.at(Punct::LParen) && self.peek_is(1, Punct::LBrace) {
            self.error_here(DiagId::parse_stmt_expr_in_default_arg);
            p.invalid = true;
            self.skip_until(&[Punct::Comma, Punct::RParen], true, false);
            return;
        }
        // clang 19 corrects a default argument's delayed typos before it
        // converts it to the parameter type (WP-408).
        let typos = self.begin_typo_correction();
        let init = if self.at(Punct::LBrace) {
            self.parse_init_list().ok().map(Initializer::List)
        } else {
            self.parse_assignment_expr().map(Initializer::Expr)
        };
        self.correct_typos(typos, init.is_some());
        if let Some(init) = init {
            self.s.act_on_param_default_init(p, init);
        } else {
            p.invalid = true;
            self.skip_until(&[Punct::Comma, Punct::RParen], false, false);
        }
    }

    /// cv- and ref-qualifiers, the exception specification and trailing
    /// return type after a parameter list ([dcl.fct]/1, [dcl.decl]/4).
    /// The exception specification is returned (`ExcSpec::None` when
    /// absent); for a delayable member declarator (see
    /// [`Self::parse_function_suffix`]) a parenthesized one is saved in
    /// `late_fn` and `d.late_noexcept` set instead (spec/parse.md §2.4).
    /// A second specification ends the declarator (clang then expects a
    /// function body).
    fn parse_function_qualifiers(
        &mut self,
        d: &mut Declarator,
        delayable: bool,
    ) -> (ExcSpec, Quals, RefQual, Option<(QualType, Loc)>) {
        let mut exc = None;
        let mut quals = Quals::NONE;
        let mut ref_qual = RefQual::None;
        let mut trailing = None;
        loop {
            let loc = self.loc();
            match self.kw() {
                Some(k @ (Kw::Const | Kw::Volatile))
                    if exc.is_none() && ref_qual == RefQual::None =>
                {
                    let (q, sp) = if k == Kw::Const {
                        (Quals::CONST, "const")
                    } else {
                        (Quals::VOLATILE, "volatile")
                    };
                    self.bump();
                    if d.chunks.is_empty() {
                        d.fn_qual_locs[usize::from(k == Kw::Volatile)].get_or_insert(loc);
                    }
                    if quals.contains(q) {
                        let diag = self.s.diag(DiagId::parse_duplicate_decl_spec, loc).arg(sp);
                        self.emit(diag);
                    }
                    quals |= q;
                }
                Some(Kw::Noexcept | Kw::Throw) if exc.is_none() => {
                    if delayable
                        && self.late_fn.delay_noexcept
                        && !(d.friend && d.qualifier.is_some())
                        && self.peek_is(1, Punct::LParen)
                    {
                        d.late_noexcept = Some(loc);
                        self.late_fn.noexcept = Some(self.capture_exception_spec());
                        exc = Some(ExcSpec::None);
                    } else {
                        let this = !d.friend
                            && self
                                .s
                                .act_on_start_this_scope(d.ctx == DeclaratorContext::Member, quals);
                        exc = Some(self.parse_exception_spec());
                        self.s.act_on_end_this_scope(this);
                    }
                    // [except.spec]/2 (C++14): no exception specification
                    // in a typedef or alias-declaration (clang
                    // `err_exception_spec_in_typedef`; the type keeps it).
                    let alias = d.site == DeclSite::Alias;
                    if self.std < Std::Cxx17 && (d.typedef || alias) {
                        let e = self
                            .s
                            .diag(DiagId::sema_exception_spec_in_typedef, loc)
                            .arg(u8::from(alias));
                        self.emit(e);
                    }
                }
                _ if exc.is_none()
                    && ref_qual == RefQual::None
                    && (self.at(Punct::Amp) || self.at(Punct::AmpAmp)) =>
                {
                    ref_qual = if self.at(Punct::Amp) { RefQual::LValue } else { RefQual::RValue };
                    if d.chunks.is_empty() {
                        d.fn_qual_locs[2] = Some(loc);
                    }
                    self.bump();
                }
                _ if self.at(Punct::Arrow) && trailing.is_none() => {
                    self.bump();
                    // A failed type-id was diagnosed: the declarator is
                    // invalid.
                    // [expr.prim.this]/2: `this` may appear in a member
                    // function's trailing return type; a friend is no
                    // member (clang's `InitCXXThisScopeForDeclaratorIfRelevant`).
                    let this = !d.friend
                        && self
                            .s
                            .act_on_start_this_scope(d.ctx == DeclaratorContext::Member, quals);
                    self.type_id_constraint = Loc::INVALID;
                    let ty = self.parse_type_id_no_ctad().unwrap_or(QualType::ERROR);
                    if !d.trailing_constraint.is_valid() {
                        d.trailing_constraint = self.type_id_constraint;
                    }
                    self.s.act_on_end_this_scope(this);
                    trailing = Some((ty, loc));
                    return (exc.unwrap_or_default(), quals, ref_qual, trailing);
                }
                _ => return (exc.unwrap_or_default(), quals, ref_qual, trailing),
            }
        }
    }

    /// noexcept-specifier ([except.spec]/1): `noexcept`,
    /// `noexcept ( constant-expression )`, or the dynamic `throw ( )` /
    /// `throw ( type-id-list )` (C++17 removed the latter; it is
    /// diagnosed by Sema). At the `noexcept` or `throw` keyword.
    pub(crate) fn parse_exception_spec(&mut self) -> ExcSpec {
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
                // `throw(Ts...)`: a pack expansion ([except.spec]/1 as of
                // C++14, [temp.variadic]/5), expanded on substitution.
                let t = match self.eat(Punct::Ellipsis) {
                    Some(l) => self.s.act_pack_expansion_type(t, l),
                    None => t,
                };
                types.push(t);
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
        self.parse_type_id_into(None, DeclSite::Plain)
    }

    /// A type-id where clang's parser forms no placeholder for a deduced
    /// class type (a trailing return type, a template argument;
    /// [dcl.type.class.deduct]/2): a class template name alone lacks its
    /// arguments.
    pub(crate) fn parse_type_id_no_ctad(&mut self) -> Option<QualType> {
        self.parse_type_id_into(None, DeclSite::NoDeduction)
    }

    /// A type-id whose vector attributes in the decl-specifiers
    /// (`int __attribute__((ext_vector_type(4)))`) move to `out`, as clang
    /// `ParseTypeName`'s `Attrs` hands them to an alias-declaration;
    /// without `out` they modify the decl-specifier type (a cast, `sizeof`,
    /// a template argument; clang `processTypeAttrs`).
    pub(crate) fn parse_type_id_into(
        &mut self,
        out: Option<&mut Vec<AttrId>>,
        site: DeclSite,
    ) -> Option<QualType> {
        let warn_mode = !std::mem::take(&mut self.named_cast_type_id);
        let mut ds = self.parse_decl_specifiers(DeclaratorContext::TypeName);
        let vec_out = out.is_some();
        if let Some(out) = out {
            let ast = &self.s.ast;
            let is_vec = |a: &AttrId| ast.attrs.get(*a).kind.is_vector_type();
            out.extend(ds.attrs.iter().copied().filter(is_vec));
            ds.attrs.retain(|a| !is_vec(a));
        }
        if !ds.has_type_specifier() {
            // `using A = S::nope;`, `template<class T = S::nope>`: where
            // only a type can be, a qualified name that names no type is
            // the lookup error ("no type named 'nope' in 'S'"; clang's
            // `DiagnoseUnknownTypeName` from `ParseTypeName`), not a syntax
            // error, and the type-id goes on with an invalid type.
            if matches!(site, DeclSite::Alias | DeclSite::TypeParamDefault)
                && self.qualified_non_type_here()
            {
                let loc = self.name_begin();
                self.unknown_qualified_type_spec(&mut ds, loc);
            } else {
                self.error_here(DiagId::parse_expected_type);
                return None;
            }
        }
        let mut d = Declarator::new(DeclaratorContext::TypeName, self.loc());
        d.site = site;
        self.parse_declarator(&mut d);
        let mut base = self.s.type_from_declspec(&ds);
        if !vec_out {
            base = self.s.apply_type_id_vector_attrs(base, &ds, warn_mode);
        }
        self.type_id_constraint = if ds.auto_constraint.is_some() && ds.constraint_loc.is_valid() {
            ds.constraint_loc
        } else {
            d.trailing_constraint
        };
        if ds.type_error {
            // "unknown type name" was reported: the type-id is clang's
            // invalid type, so a cast or trait of it says nothing more
            // (WP-186).
            return Some(QualType::ERROR);
        }
        let ty = self.s.type_for_declarator_at(base, ds.type_loc.unwrap_or(ds.loc), &d);
        let ty = self.s.act_on_type_id_decl_attrs(ty, &d.attrs);
        Some(self.s.check_qualified_fn_type(ty, &ds, &d))
    }

    /// Whether the current token starts a name qualified by a
    /// non-dependent class or namespace in which it names no type
    /// ([`Parser::unknown_qualified_type_spec`] reports it).
    fn qualified_non_type_here(&self) -> bool {
        let start = self.pos();
        match self.scan_name(start) {
            Some((end, NameClass::Undeclared, _)) => {
                end > start + 1
                    && (self.unresolved_scope_name(start) || self.incomplete_scope_name(start))
            }
            Some((end, NameClass::Value(_), _)) => {
                self.nontype_scope_name(start)
                    && Self::punct_of(self.tok_at(end)) != Some(Punct::Less)
            }
            _ => false,
        }
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
    /// A new declarator of a simple-declaration with decl-specifiers `ds`
    /// in context `ctx`, before its first token.
    fn group_declarator(&self, ctx: DeclaratorContext, ds: &DeclSpec) -> Declarator {
        let mut d = Declarator::new(ctx, self.loc());
        d.direct_init = Self::may_have_direct_init(ctx, ds.storage);
        d.typedef = ds.is_typedef();
        d.typed = ds.has_type_specifier();
        d
    }

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

    /// The initializer form the current token starts, if any.
    fn init_style_here(&self) -> Option<InitStyle> {
        match Self::punct_of(self.tok()) {
            Some(Punct::Equal) => Some(InitStyle::C),
            Some(Punct::LParen) => Some(InitStyle::Call),
            Some(Punct::LBrace) => Some(InitStyle::List),
            _ => None,
        }
    }

    /// The initializer of declarator `d` (declared as `v`) and its
    /// semantic action. `Err` when the declaration cannot continue.
    fn parse_declarator_initializer(
        &mut self,
        d: &Declarator,
        v: Option<DeclId>,
    ) -> Result<(), ()> {
        // The initializer of a qualified declarator-id is in the scope of
        // its qualifier ([basic.scope.class]/1).
        let entered = match (v, d.qualifier) {
            (Some(v), Some(_)) => self.s.act_on_enter_decl_initializer(v),
            _ => 0,
        };
        // The initializer, with its conversions, is potentially evaluated.
        let quiet = self.s.enter_var_initializer(v, d.init_style.is_some());
        // clang 19 corrects the initializer's delayed typos with the
        // initialization as the filter: a correction stands only if the
        // variable can be initialized from it (`int (*p)(int) = val<int>;`
        // is corrected to `::val`, `long (*q)(int) = val<int>;` is not,
        // WP-399).
        let typos = (self.s.clang_compat < 22).then(|| self.s.begin_delayed_typos());
        let outer_invalid = std::mem::replace(&mut self.expr_invalid, false);
        let init = self.parse_initializer(d.init_style, d.is_function_declarator());
        // clang's `ParseDeclarationAfterDeclaratorAndAttributes`: an
        // invalid `= expression` (`ExprError`, `expr_invalid`) skips to
        // the next declarator's `,` or the `;`, with no "expected ';'"
        // for what is left of it (`int q = 1 + v<S>{};`).
        if std::mem::replace(&mut self.expr_invalid, outer_invalid)
            && d.init_style == Some(InitStyle::C)
            && matches!(init, Ok(Some(Initializer::Expr(_))))
        {
            self.skip_until(&[Punct::Comma], true, false);
        }
        self.s.act_on_exit_declarator_scope(entered);
        // clang 19's `AddInitializerToDecl` corrects an undeduced
        // variable's initializer before deducing, with no filter
        // (`decltype(auto) z = val<int>;`, WP-408).
        let typos = match (typos, v) {
            (Some(m), Some(v)) if self.s.has_undeduced_type(v) => {
                self.s.flush_delayed_typos(m);
                None
            }
            (t, _) => t,
        };
        let r = self.finish_declarator_initializer(d, v, init);
        if let Some(m) = typos {
            self.s.end_delayed_typos(m);
        }
        self.s.exit_var_initializer(quiet);
        r
    }

    /// The semantic action of declarator `d`'s parsed initializer `init`.
    fn finish_declarator_initializer(
        &mut self,
        d: &Declarator,
        v: Option<DeclId>,
        init: Result<Option<Initializer>, ()>,
    ) -> Result<(), ()> {
        match init {
            Ok(init) => {
                match (v, init) {
                    (Some(v), Some(i)) => self.s.act_on_init(v, i),
                    (Some(v), None) => self.s.act_on_uninit(v),
                    _ => {}
                }
                Ok(())
            }
            // A malformed braced-init-list without `=` was skipped
            // through its `}` (clang's `ParseBraceInitializer`), a
            // malformed expression-list through its `)` (clang's
            // `ParseDeclarationAfterDeclaratorAndAttributes`: `void
            // f4(=) {}`); the declaration continues, so a missing `;` is
            // reported.
            Err(())
                if matches!(
                    d.init_style,
                    Some(InitStyle::List | InitStyle::Call | InitStyle::ParenList)
                ) =>
            {
                Ok(())
            }
            Err(()) => Err(()),
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
        // [class.access]/7: the names in the decl-specifiers and the first
        // declarator are checked in the context of the declared entity
        // (`A::I A::f()`, a befriended `A::I g(A::I)`; clang's
        // `ParsingDeclSpec`), together with those of a template head.
        let level = match self.template_delay_level.take() {
            Some(l) => l,
            None => self.s.delayed_access_level(),
        };
        self.s.begin_delayed_access();
        let decls = self.parse_simple_declaration_delayed(ctx, allow_fn_def, level);
        self.s.end_delayed_access_to(level, None);
        decls
    }

    /// The leading C++11 attribute-specifier-seq of a declaration
    /// without declarators (`[[nodiscard]] struct S { };`, `[[x]] int;`)
    /// would appertain to no entity ([dcl.pre]/5): at namespace scope,
    /// before a class or enum specifier, clang reports `misplaced
    /// attributes; expected attributes here` after the class-key, where
    /// they would appertain to the class ([class.pre]/1, [dcl.enum]/1;
    /// at compat 22 after the `class`/`struct` of a scoped enum),
    /// and otherwise `an attribute list cannot appear here` at their
    /// start; at block scope always the latter. They are dropped. (In
    /// a class they reach [`Sema::act_on_decl_spec_only`], which warns.)
    fn prohibit_free_standing_lead_attrs(&mut self, ds: &mut DeclSpec, ctx: DeclaratorContext) {
        let Some(at) = ds.lead_cxx11 else { return };
        if ds.is_empty() && ds.tag.is_none() {
            // An attribute-declaration ([dcl.pre]/10).
            return;
        }
        let key_len = match ds.keyword {
            TagKeyword::Struct => 6,
            TagKeyword::Class | TagKeyword::Union => 5,
            TagKeyword::Enum => 4,
            _ => 0,
        };
        match ctx {
            DeclaratorContext::File if ds.tag.is_some() && key_len > 0 => {
                // clang 22 inserts after `enum class` / `enum struct`.
                let fix = match ds.scoped_key_end {
                    Some(e) if self.s.clang_compat >= 22 => e,
                    _ => ds.type_loc.unwrap_or(at).with_offset(key_len),
                };
                self.s.error(DiagId::parse_attributes_misplaced, fix);
            }
            DeclaratorContext::File | DeclaratorContext::Block => {
                self.s.error(DiagId::parse_attributes_not_allowed, at);
            }
            _ => return,
        }
        Self::drop_lead_cxx11(&self.s.ast.attrs, ds);
    }

    /// Drops the C++11 attributes (`[[...]]`, `alignas`) of `ds`, whose
    /// leading attribute-specifier-seq was reported.
    pub(crate) fn drop_lead_cxx11(attrs: &kiln_ast::attr::AttrArena, ds: &mut DeclSpec) {
        ds.attrs
            .retain(|&a| !matches!(attrs.get(a).syntax, AttrSyntax::Cxx11 | AttrSyntax::Keyword));
        ds.lead_cxx11 = None;
    }

    /// True at a declarator that may follow an empty decl-specifier-seq
    /// in context `ctx`: a special member's, or at namespace scope `y;`,
    /// a declarator without a type declared as an invalid `int` (clang's
    /// `err_missing_type_specifier`).
    fn at_declarator_without_specifiers(&self, ctx: DeclaratorContext) -> bool {
        self.at_special_declarator(ctx)
            || (ctx == DeclaratorContext::File
                && (self.at_implicit_int_declarator()
                    || self.at_qualified_implicit_int_declarator()))
    }

    /// At namespace scope, a qualified name for which lookup finds a
    /// non-type or nothing (`R::f(1);`, `R::f;`, `R::h(int);`), followed
    /// by what clang's `ParseImplicitInt` takes as the rest of a
    /// declarator with no type: `;` `=` `,` `[` `{` `asm`, or a `(` that
    /// cannot start a declarator (else the name was meant as a type,
    /// [`Self::qualified_unknown_type_follows`]). The name is the
    /// declarator-id of an invalid `int` declaration, after "a type
    /// specifier is required" (WP-411).
    pub(crate) fn at_qualified_implicit_int_declarator(&self) -> bool {
        let i = self.pos();
        let Some((end, false, class, _, Some(q))) = self.scan_name_qual_at(i) else {
            // A qualifier component that names no scope (`Und::f();`):
            // the declarator's qualifier reports it.
            return self.not_a_scope_name(i).is_some_and(|end| self.implicit_int_follows(end));
        };
        if end <= i + 1
            || self.s.is_dependent_ctx(q)
            || !matches!(class, NameClass::Value(_) | NameClass::Undeclared)
        {
            return false;
        }
        self.implicit_int_follows(end)
    }

    /// Whether token `end`, after a qualified name, continues a declarator
    /// with no type ([`Self::at_qualified_implicit_int_declarator`]).
    pub(crate) fn implicit_int_follows(&self, end: usize) -> bool {
        match Self::punct_of(self.tok_at(end)) {
            Some(Punct::Semi | Punct::Equal | Punct::Comma | Punct::LSquare | Punct::LBrace) => {
                true
            }
            Some(Punct::LParen) => !self.declarator_may_start_at(end),
            _ => self.kw_of(self.tok_at(end)) == Some(Kw::Asm),
        }
    }

    /// [temp.explicit]/3: declarator `d` of an explicit instantiation
    /// (`function`: before a function body) is followed by a definition
    /// or initializer (`Sema::act_explicit_inst_with_definition`).
    fn explicit_inst_with_definition(
        &mut self,
        kind: Option<TemplateDeclKind>,
        d: &Declarator,
        function: bool,
    ) {
        if kind == Some(TemplateDeclKind::ExplicitInstantiation) {
            self.s.act_explicit_inst_with_definition(d, self.loc(), function);
        }
    }

    fn parse_simple_declaration_delayed(
        &mut self,
        ctx: DeclaratorContext,
        allow_fn_def: bool,
        level: usize,
    ) -> Vec<DeclId> {
        let tmpl_kind = self.template_decl_kind.take();
        let mut ds = self.parse_decl_specifiers(ctx);
        let mut tmpl_group = self.template_group(tmpl_kind);
        let mut decls: Vec<DeclId> = ds.tag.filter(|_| ds.tag_declares).into_iter().collect();
        if self.at(Punct::Semi) {
            self.prohibit_free_standing_lead_attrs(&mut ds, ctx);
            self.bump();
            decls.extend(self.s.act_on_decl_spec_only(&ds));
            self.s.end_delayed_access_to(level, ds.tag);
            return decls;
        }
        if ds.is_empty() && !self.at_declarator_without_specifiers(ctx) {
            self.error_here(DiagId::parse_expected_unqualified_id);
            self.skip_to_semi();
            return decls;
        }
        let mut base_ty: Option<QualType> = None;
        // [dcl.spec.auto.general]/3, /8: checked at the end of the group.
        let mut group = AutoGroup::default();
        // [dcl.pre]/6 (clang's `BuildDeclaratorGroup`): a structured
        // binding declaration is alone in its group.
        let mut first_decomp: Option<(Loc, Option<DeclId>)> = None;
        let (mut declarators, mut restrict) = (0usize, None);
        let _ = self.s.take_declarator_type_invalid();
        let mut type_invalid = false;
        let mut semi_at = None;
        loop {
            let mut d = self.group_declarator(ctx, &ds);
            self.parse_declarator(&mut d);
            let contracts = self.parse_requires_and_contract_specs(&mut d);
            self.set_declarator_end(&mut d);
            self.diagnose_virt_specifiers_outside_class(&d);
            let base = *base_ty.get_or_insert_with(|| self.group_base_type(&ds, &d));
            if self.parse_asm_label_and_gnu_attrs(&mut d, declarators, &mut restrict).is_err() {
                self.skip_until(&[], true, false);
                break;
            }
            if !d.has_name() {
                if !d.invalid {
                    self.error_here(DiagId::parse_expected_unqualified_id);
                }
                // clang's `SkipMalformedDecl`: a braced group ends the
                // declaration (`extern "C++" { ... }` at block scope).
                self.skip_malformed_decl();
                return decls;
            }
            // A function definition has no asm label: after one, `{` is
            // an initializer (clang's order of checks:
            // `isDeclarationAfterDeclarator`, `isStartOfFunctionDefinition`).
            if declarators == 0 && d.is_function_declarator() && d.asm_label.is_none() {
                if allow_fn_def && self.at_function_definition_start() {
                    self.explicit_inst_with_definition(tmpl_kind, &d, true);
                    self.parse_function_definition_after_declarator(
                        &ds, base, &mut d, &mut decls, level, contracts,
                    );
                    return decls;
                }
                if !allow_fn_def && self.at(Punct::LBrace) {
                    self.error_here(DiagId::parse_function_definition_not_allowed);
                    self.skip_unsupported_declaration();
                    return decls;
                }
                if self.missing_fn_body_before_cxx11_attrs(ctx) {
                    return decls;
                }
            }
            // clang's `ParseDeclGroup`: a later declarator of a
            // decl-specifier that named no type is parsed, not declared,
            // and takes no initializer (`u1 &a, &b;` reports `a` only).
            // So is one after a declarator whose type named an
            // unexpanded pack (`Ts a, b;` reports `a` only).
            if declarators > 0 && (ds.type_error || type_invalid) {
                if self.eat(Punct::Comma).is_some() {
                    continue;
                }
                // clang's tentative `isCXXFunctionDeclarator` took the `(`
                // before reverting, leaving `PrevTokLocation` on it: the
                // missing `;` is reported after the `(` (`R::f(v), g(v);`;
                // WP-411).
                semi_at = self.at(Punct::LParen).then(|| self.tok().span.hi);
                break;
            }
            d.after_comma = declarators > 0;
            d.init_style = self.init_style_here();
            if !self.at(Punct::Semi) {
                self.explicit_inst_with_definition(tmpl_kind, &d, false);
            }
            let v = self.s.act_on_declarator(&ds, base, &d);
            self.end_declaration_with_contracts(v, &d, contracts);
            // A declarator without a type is invalid too (`static y, z;`).
            type_invalid |= self.s.take_declarator_type_invalid()
                || (!ds.has_type_specifier() && !is_untyped_special(&d));
            self.s.end_delayed_access_to(level, v);
            if let Some(v) = v {
                self.s.act_on_group_declarator(&mut group, v);
            }
            declarators += 1;
            if first_decomp.is_none()
                && let Some(b) = &d.decomp
            {
                first_decomp = Some((b.lbracket, v));
            }
            if self.parse_declarator_initializer(&d, v).is_err() {
                decls.extend(v);
                self.skip_to_semi();
                return decls;
            }
            decls.extend(v);
            if !self.next_declarator(&mut tmpl_group) {
                break;
            }
        }
        self.s.act_on_end_group(group, &ds);
        if let Some((l, v)) = first_decomp.filter(|_| declarators > 1) {
            self.s.act_on_decomposition_not_alone(l, v);
        }
        self.expect_declaration_semi(ctx, semi_at);
        decls
    }

    /// clang forms each declarator's type from the decl-specifier-seq
    /// anew: its "restrict requires a pointer or reference" is reported
    /// once per declarator (`__restrict int a, b;`). The first declarator
    /// (`first`) takes the error of the group's type; later ones repeat it.
    pub(crate) fn repeat_declspec_errors(
        &mut self,
        restrict_error: &mut Option<(Loc, QualType)>,
        first: bool,
    ) {
        if first {
            *restrict_error = self.s.take_declspec_restrict_error();
        } else if let Some(r) = *restrict_error {
            self.s.report_restrict_not_pointer(r);
        }
    }

    /// Sets `d.end` right after `parse_declarator` (and a trailing
    /// requires-clause): the start of its last token (clang's
    /// `PrevTokLocation` once `ParseDeclarator` returns), not counting a
    /// trailing C++11 attribute-specifier-seq. An asm label, GNU
    /// attributes and virt-specifiers extend it later.
    pub(crate) fn set_declarator_end(&self, d: &mut Declarator) {
        let e = self.prev_lo();
        d.end = if e == self.cxx11_attr_run.1 { self.cxx11_attr_run.0 } else { e };
    }

    /// Records the end of bodiless function declaration `f`'s declarator
    /// `d`, where notes about it point (`ColdDeclData::declarator_end`).
    pub(crate) fn record_declarator_end(&mut self, f: Option<DeclId>, d: &Declarator) {
        self.record_fn_end(f, d.end);
    }

    /// Records `end` as the end of bodiless function declaration `f`
    /// (`ColdDeclData::declarator_end`).
    pub(crate) fn record_fn_end(&mut self, f: Option<DeclId>, end: Loc) {
        if let Some(f) = f
            && end != Loc::INVALID
            && self.s.ast.decls.function(f).is_some_and(|x| x.body.is_none())
        {
            let _ = self.s.ast.decls.cold.declarator_end.insert(f, end);
        }
    }

    /// The GNU asm label (clang's `ParseAsmAttributesAfterDeclarator`;
    /// `Err` when malformed, which drops the declarator) and GNU
    /// attributes after a non-member declarator; both extend `d.end`.
    /// Only GNU attributes follow (clang's `MaybeParseGNUAttributes` in
    /// `ParseDeclGroup`): a C++11 attribute-specifier-seq here ends the
    /// declarator, and the `;` (or function body) expected there is
    /// reported by the caller. First repeats the decl-specifiers' errors
    /// for this declarator ([`Parser::repeat_declspec_errors`]).
    /// `index` is the declarator's index in its group.
    fn parse_asm_label_and_gnu_attrs(
        &mut self,
        d: &mut Declarator,
        index: usize,
        restrict_error: &mut Option<(Loc, QualType)>,
    ) -> Result<(), ()> {
        let first = index == 0;
        self.repeat_declspec_errors(restrict_error, first);
        self.gnu_attr_end = Loc::INVALID;
        if first && d.is_function_declarator() {
            self.parse_declarator_gnu_attr_seq(d);
        }
        let label = self.parse_asm_label()?;
        if label.is_some() {
            d.end = self.prev_lo();
            self.gnu_attr_end = Loc::INVALID;
        }
        d.asm_label = label;
        self.parse_declarator_gnu_attr_seq(d);
        if self.gnu_attr_end != Loc::INVALID {
            d.end = self.gnu_attr_end;
        }
        Ok(())
    }

    /// clang `ParseDeclGroup`: at namespace scope, a first function
    /// declarator followed by neither a declaration's continuation nor a
    /// function body, here a C++11 attribute-specifier-seq after its GNU
    /// attributes (`void f() __attribute__((cold)) [[nodiscard]];`), is
    /// reported as a missing body; the declaration is skipped through its
    /// `;` and declares nothing (true).
    fn missing_fn_body_before_cxx11_attrs(&mut self, ctx: DeclaratorContext) -> bool {
        if ctx != DeclaratorContext::File || !(self.at_cxx11_attr() || self.at_kw(Kw::Alignas)) {
            return false;
        }
        self.error_here(DiagId::parse_expected_fn_body);
        self.skip_to_semi();
        true
    }

    /// The type of decl-specifier-seq `ds`, formed once the first
    /// declarator `d` of its group is known: clang reports a missing type
    /// specifier at its name. Constructors, destructors and conversion
    /// functions have none ([class.ctor.general]/1, [class.dtor]/1,
    /// [class.conv.fct]/1).
    fn group_base_type(&mut self, ds: &DeclSpec, d: &Declarator) -> QualType {
        let untyped = !ds.has_type_specifier() && is_untyped_special(d);
        if untyped && is_deduction_guide(d) {
            self.s.plain_auto_type()
        } else if untyped {
            QualType::VOID
        } else if (d.invalid || !d.has_name()) && !ds.has_type_specifier() {
            // A declarator with no name is reported as "expected
            // unqualified-id" alone: clang never builds its type
            // (`extern "C"` at block scope, [dcl.link]/4).
            QualType::ERROR
        } else {
            let t = self.s.type_from_declspec_at(ds, d.name_loc());
            self.s.rebuild_in_current_instantiation(t, d)
        }
    }

    /// A virt-specifier-seq after function declarator `d` outside a
    /// member-specification ([class.mem.general]/1 allows one only in a
    /// member-declarator): each `final`/`override` is diagnosed and
    /// skipped (clang's `err_virt_specifier_outside_class`).
    fn diagnose_virt_specifiers_outside_class(&mut self, d: &Declarator) {
        while d.is_function_declarator() {
            let t = self.tok();
            let sp = if self.is_final_tok(t) {
                "final"
            } else if self.is_override_tok(t) {
                "override"
            } else {
                return;
            };
            let loc = self.bump().span.lo;
            let diag = self.s.diag(DiagId::parse_virt_specifier_outside_class, loc).arg(sp);
            self.emit(diag);
        }
    }

    /// The `;` ending a simple-declaration.
    fn expect_declaration_semi(&mut self, ctx: DeclaratorContext, at: Option<Loc>) {
        if !self.eat_extraneous_before_semi() && self.eat(Punct::Semi).is_none() {
            let id = if ctx == DeclaratorContext::File {
                DiagId::parse_expected_semi_toplevel
            } else {
                DiagId::parse_expected_semi_declaration
            };
            // clang's `ExpectAndConsume`: the `;` is inserted where it is
            // reported, after the previous token.
            let at = at.unwrap_or_else(|| self.expected_loc());
            let msg = self.s.diag(id, at).fixit(FixIt::insert(at, ";"));
            self.s.emit(msg);
            // clang's `ParseDeclGroup`: `SkipMalformedDecl`, which stops
            // after a braced body (`void f4(=) {}`).
            if !self.is_decl_spec_start() {
                self.skip_malformed_decl();
            }
        }
    }

    /// True at the start of a function-body or `= default` / `= delete`
    /// after a function declarator ([dcl.fct.def.general]/1).
    fn at_function_definition_start(&self) -> bool {
        match Self::punct_of(self.tok()) {
            Some(Punct::LBrace | Punct::Colon) => true,
            Some(Punct::Equal) => {
                matches!(self.kw_of(self.peek(1)), Some(Kw::Default | Kw::Delete))
            }
            _ => self.at_kw(Kw::Try),
        }
    }

    /// The function-body or `= default ;` / `= delete ;` of a namespace-
    /// scope function definition whose declarator `d` was just parsed.
    fn parse_function_definition_after_declarator(
        &mut self,
        ds: &DeclSpec,
        base: QualType,
        d: &mut Declarator,
        decls: &mut Vec<DeclId>,
        level: usize,
        contracts: Option<LateToks>,
    ) {
        d.defaulted = self.at_eq_default();
        self.s.act_function_def_next(true);
        let f = self.s.act_on_declarator(ds, base, d);
        self.s.act_function_def_next(false);
        self.s.end_delayed_access_to(level, f);
        // An invalid declarator sema did not make a function (an unknown
        // return type, say): the error is reported; the body is skipped.
        let f = f.filter(|&f| self.s.ast.decls.function(f).is_some());
        decls.extend(f);
        if self.at(Punct::Equal) {
            // A defaulted or deleted definition ends at its declarator
            // too (clang `getEndLoc`; an implicit definition's body is
            // located there).
            self.record_declarator_end(f, d);
            let how = if self.kw_of(self.peek(1)) == Some(Kw::Delete) {
                ContractDeclKind::Deleted
            } else {
                ContractDeclKind::Defaulted
            };
            self.replay_contract_specs(contracts, d, f, how);
            self.parse_defaulted_or_deleted(f);
            return;
        }
        // [dcl.contract.res]/2: a result name of a deduced return type is
        // typed once the body has deduced it.
        let after = f.is_some_and(|f| self.s.contract_return_undeduced(f));
        let mut contracts = contracts;
        if !after {
            self.replay_contract_specs(contracts.take(), d, f, ContractDeclKind::Definition);
        }
        match f {
            Some(f) => {
                let ctor = matches!(d.special, Some((SpecialName::Constructor(_), _)));
                self.parse_function_definition(f, ctor);
            }
            None => self.skip_function_body(),
        }
        if after {
            self.replay_contract_specs(contracts, d, f, ContractDeclKind::Definition);
        }
    }

    /// The trailing requires-clause of `d`, then (a function declarator)
    /// its *function-contract-specifier-seq*, captured for replay
    /// ([dcl.contract.func]/1: after the requires-clause).
    fn parse_requires_and_contract_specs(&mut self, d: &mut Declarator) -> Option<LateToks> {
        self.parse_trailing_requires(d);
        if d.is_function_declarator() { self.capture_contract_specs() } else { None }
    }

    /// Records the end of `d`, which declared `v` without defining it,
    /// then replays its captured contract specifiers.
    fn end_declaration_with_contracts(
        &mut self,
        v: Option<DeclId>,
        d: &Declarator,
        contracts: Option<LateToks>,
    ) {
        self.record_declarator_end(v, d);
        if d.is_function_declarator() {
            let f = v.filter(|&f| self.s.ast.decls.function(f).is_some());
            self.replay_contract_specs(contracts, d, f, ContractDeclKind::Declaration);
        }
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
        is_function: bool,
    ) -> Result<Option<Initializer>, ()> {
        match style {
            None => Ok(None),
            Some(InitStyle::C) => {
                self.bump();
                // clang's `ParseDeclarationAfterDeclaratorAndAttributes`:
                // only the first declarator of a function definition can
                // be defaulted or deleted.
                if let Some(k @ (Kw::Delete | Kw::Default)) = self.kw() {
                    let l = self.bump().span.lo;
                    let id = match (k, is_function) {
                        (_, true) => DiagId::parse_default_delete_in_multiple_declaration,
                        (Kw::Delete, false) => DiagId::parse_deleted_non_function,
                        _ => DiagId::parse_default_special_members,
                    };
                    let mut diag = self.s.diag(id, l);
                    if is_function {
                        diag = diag.arg(if k == Kw::Delete { "delete" } else { "default" });
                    }
                    self.emit(diag);
                    if k == Kw::Delete {
                        self.skip_deleted_function_body();
                    }
                    return Ok(None);
                }
                if self.at(Punct::LBrace) {
                    return Ok(Some(Initializer::List(self.parse_init_list()?)));
                }
                Ok(Some(Initializer::Expr(self.parse_assignment_expr().ok_or(())?)))
            }
            Some(InitStyle::Call | InitStyle::ParenList) => {
                let l = self.bump().span.lo;
                // An expression-list of initializer-clauses: braced lists
                // and pack expansions ([dcl.init.general], [temp.variadic]/5).
                let v = if self.at(Punct::RParen) {
                    Vec::new()
                } else {
                    self.parse_expr_list(Punct::RParen).ok_or(())?
                };
                let r = self.expect_closing(Punct::RParen, Punct::LParen, l);
                Ok(Some(Initializer::Paren(v, l, r)))
            }
            Some(InitStyle::List) => Ok(Some(Initializer::List(self.parse_init_list()?))),
        }
    }

    /// Whether the `[` at the current token in a braced-init-list starts
    /// a (C99) array designator rather than a lambda (clang
    /// `MayBeDesignationStart`): `[=`, `[...` and `[]` begin a lambda;
    /// after `[&`, `[*` or `[name` it is a designator when the matching
    /// `]` is followed by `=`; anything else is a designator.
    fn array_designator_start(&self) -> bool {
        match self.peek(1).kind {
            TokKind::Punct(Punct::Equal | Punct::Ellipsis | Punct::RSquare) => false,
            TokKind::Punct(Punct::Amp | Punct::Star) | TokKind::Identifier(_) => {
                let mut depth = 0usize;
                let mut n = 0;
                loop {
                    match self.peek(n).kind {
                        TokKind::Punct(Punct::LSquare | Punct::LParen | Punct::LBrace) => {
                            depth += 1;
                        }
                        TokKind::Punct(Punct::RSquare | Punct::RParen | Punct::RBrace) => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 {
                                return self.peek_is(n + 1, Punct::Equal);
                            }
                        }
                        TokKind::Eof => return false,
                        _ => {}
                    }
                    n += 1;
                }
            }
            _ => true,
        }
    }

    /// braced-init-list ([dcl.init.general]), with C++20 designators.
    /// A list with an invalid designation (or containing such a list) is
    /// parsed through its `}` and then dropped, as clang's
    /// `ParseBraceInitializer` does: `Err` without further diagnostics.
    pub(crate) fn parse_init_list(&mut self) -> Result<InitList, ()> {
        match self.parse_init_list_checked()? {
            (l, true) => Ok(l),
            (_, false) => Err(()),
        }
    }

    /// [`Self::parse_init_list`], with whether the designations were
    /// valid instead of dropping the list.
    fn parse_init_list_checked(&mut self) -> Result<(InitList, bool), ()> {
        if !self.stack_ok() {
            return Err(());
        }
        let lbrace = self.bump().span.lo;
        let mut items = Vec::new();
        let mut valid = true;
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
                } else if self.at(Punct::LSquare)
                    && (!designator.is_empty() || self.array_designator_start())
                {
                    let l = self.bump().span.lo;
                    let Some(e) = self.parse_constant_expr() else {
                        self.skip_until(&[Punct::RBrace], false, true);
                        return Err(());
                    };
                    // A GNU array range designator `[first ... last]`.
                    let range = if let Some(ellipsis) = self.eat(Punct::Ellipsis) {
                        let d = self.s.diag(DiagId::sema_gnu_array_range, ellipsis);
                        self.emit(d);
                        let Some(last) = self.parse_constant_expr() else {
                            self.skip_until(&[Punct::RBrace], false, true);
                            return Err(());
                        };
                        Some((last, ellipsis))
                    } else {
                        None
                    };
                    self.expect_closing(Punct::RSquare, Punct::LSquare, l);
                    designator.push(match range {
                        Some((last, ellipsis)) => Designator::Range(e, last, l, ellipsis),
                        None => Designator::Index(e, l),
                    });
                } else {
                    break;
                }
            }
            if !designator.is_empty() && !self.s.act_on_designation(&designator) {
                valid = false;
            }
            if !designator.is_empty()
                && !self.at(Punct::LBrace)
                && self.expect(Punct::Equal).is_none()
            {
                self.skip_until(&[Punct::RBrace], false, true);
                return Err(());
            }
            let mut value = if self.at(Punct::LBrace) {
                let Ok((l, ok)) = self.parse_init_list_checked() else {
                    // The inner list skipped through its `}`; skip
                    // through ours too (clang's `ParseBraceInitializer`).
                    self.skip_until(&[Punct::RBrace], false, true);
                    return Err(());
                };
                valid &= ok;
                InitValue::List(l)
            } else {
                // clang 19's `ParseBraceInitializer` corrects each
                // element's delayed typos as it is parsed (WP-408).
                let typos = self.begin_typo_correction();
                let e = self.parse_assignment_expr();
                self.correct_typos(typos, e.is_some());
                let Some(e) = e else {
                    self.skip_until(&[Punct::RBrace], false, true);
                    return Err(());
                };
                InitValue::Expr(self.s.act_on_init_clause(e))
            };
            if let Some(l) = self.eat(Punct::Ellipsis) {
                // An initializer-clause pack expansion ([temp.variadic]/5.4).
                match value {
                    InitValue::Expr(e) => value = InitValue::Expr(self.s.act_pack_expansion(e, l)),
                    InitValue::List(n) => {
                        value = InitValue::Expr(self.s.act_pack_expansion_list(&n, l));
                    }
                }
            }
            items.push(InitItem { designator, value });
            if self.eat(Punct::Comma).is_none() {
                break;
            }
        }
        let rbrace = self.expect_closing(Punct::RBrace, Punct::LBrace, lbrace);
        let l = InitList { items, lbrace, rbrace };
        if valid {
            self.s.act_on_init_list_syntax(&l);
        }
        Ok((l, valid))
    }
}

/// True if `d` names a constructor, destructor or conversion function,
/// which take no type specifier.
pub(crate) fn is_untyped_special(d: &Declarator) -> bool {
    matches!(
        d.special,
        Some((
            SpecialName::Constructor(_)
                | SpecialName::Destructor(_)
                | SpecialName::Conversion(_)
                | SpecialName::DeductionGuide(_),
            _
        ))
    )
}

/// True for a deduction-guide declarator, whose declared return type is
/// the placeholder its trailing return type replaces
/// ([temp.deduct.guide]/1, [dcl.fct]/2).
pub(crate) fn is_deduction_guide(d: &Declarator) -> bool {
    matches!(d.special, Some((SpecialName::DeductionGuide(_), _)))
}

/// `constexpr`, `consteval` or `constinit`: at most one of them
/// ([dcl.spec]/2, clang's `SetConstexprSpec`).
fn set_constexpr_kind(ds: &mut DeclSpec, k: Kw, loc: Loc) -> Result<(), SpecError> {
    let prev =
        [(ds.constexpr, "constexpr"), (ds.consteval, "consteval"), (ds.constinit, "constinit")]
            .into_iter()
            .find_map(|(l, sp)| l.map(|_| sp));
    if let Some(prev) = prev {
        let id = if prev == k.spelling() {
            DiagId::parse_duplicate_decl_spec
        } else {
            DiagId::parse_invalid_decl_spec_combination
        };
        return Err(SpecError { id, args: vec![prev.into()] });
    }
    let slot = match k {
        Kw::Constexpr => &mut ds.constexpr,
        Kw::Consteval => &mut ds.consteval,
        _ => &mut ds.constinit,
    };
    *slot = Some(loc);
    Ok(())
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

/// clang's `FunctionDeclarationScope`: the parameter clause of the
/// function declarator being added to `d` belongs to the function a
/// declaration declares (`isFunctionDeclaratorAFunctionDeclaration`: not
/// a `typedef`, and only grouping parentheses lie between the name and
/// this function declarator) or to a lambda. `auto` is a placeholder for
/// an invented template parameter only there ([dcl.fct]/22,
/// [expr.prim.lambda.closure]/3).
fn is_function_declaration_clause(d: &Declarator, site: DeclSite) -> bool {
    site == DeclSite::LambdaParam
        || (matches!(
            d.ctx,
            DeclaratorContext::File
                | DeclaratorContext::Member
                | DeclaratorContext::Block
                | DeclaratorContext::ForInit
        ) && !d.typedef
            && d.chunks.is_empty())
}
