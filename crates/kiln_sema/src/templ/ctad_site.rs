//! Where a placeholder may appear: for a deduced class type
//! ([dcl.type.class.deduct]/2) clang's `err_auto_not_allowed` (with a
//! deduced class template), `err_template_missing_args` where its parser
//! forms no placeholder, and `err_deduced_class_template_compound_type`;
//! for `auto` and `decltype(auto)` ([dcl.spec.auto.general]/2-5,
//! [dcl.type.auto.deduct]) clang's `err_auto_not_allowed` with its
//! position wording (`GetDeclSpecTypeForDeclarator` and
//! `GetFullTypeForDeclarator`). Spec: sema-templates.md §6.3.

use kiln_ast::QualType;
use kiln_ast::decl::{DeclKind, TagKind};
use kiln_ast::ids::TemplateNameId;
use kiln_ast::types::{AutoKeyword, TypeView};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::declspec::{
    Chunk, DeclSite, DeclSpec, Declarator, DeclaratorContext, SpecialName, StorageSpec,
};

/// clang's `err_auto_not_allowed` context selectors used here.
pub(crate) mod site {
    pub(crate) const PROTOTYPE: u8 = 0;
    pub(crate) const STRUCT_MEMBER: u8 = 1;
    pub(crate) const UNION_MEMBER: u8 = 3;
    pub(crate) const CLASS_MEMBER: u8 = 5;
    pub(crate) const CATCH: u8 = 7;
    pub(crate) const TEMPLATE_PARAM_17: u8 = 8;
    pub(crate) const TEMPLATE_ARG: u8 = 10;
    pub(crate) const TYPEDEF: u8 = 11;
    pub(crate) const ALIAS: u8 = 12;
    pub(crate) const RETURN_TYPE: u8 = 13;
    pub(crate) const CONVERSION_ID: u8 = 14;
    pub(crate) const HERE: u8 = 15;
    pub(crate) const LAMBDA_PARAM: u8 = 16;
    pub(crate) const NEW: u8 = 17;
    pub(crate) const FRIEND: u8 = 20;
    pub(crate) const NOT_FUNCTION_DECLARATION: u8 = 21;
    pub(crate) const REQUIRES_PARAM: u8 = 22;
    pub(crate) const CONVERSION_TEMPLATE: u8 = 24;
    pub(crate) const LAMBDA_PARAM_14: u8 = 25;
}

/// The verdict on a placeholder in a declarator's context.
#[derive(Clone, Copy)]
enum Misplaced {
    /// `err_auto_not_allowed` with this context selector.
    NotAllowed(u8),
    /// `err_template_missing_args`.
    MissingArgs,
}

impl Sema<'_, '_> {
    /// The placeholder for a deduced class type `ty` where none may
    /// appear, reported at `loc` with the template note; the error type.
    fn ctad_misplaced(&mut self, ty: QualType, loc: Loc, why: Misplaced) -> QualType {
        let Some((_, tmpl)) = self.undeduced_ctad(ty) else { return ty };
        let what = if self.kind_of(tmpl) == DeclKind::ClassTemplate {
            "class template"
        } else {
            "alias template"
        };
        let name = self.ty_arg(ty.without_quals());
        let note = self.diag(DiagId::note_template_decl_here, self.ast.decls.get(tmpl).loc());
        let msg = match why {
            Misplaced::NotAllowed(s) => {
                self.diag(DiagId::sema_ctad_auto_not_allowed, loc).arg(what).arg(name).arg(s)
            }
            Misplaced::MissingArgs => {
                self.diag(DiagId::sema_template_requires_args, loc).arg(what).arg(name)
            }
        };
        self.emit(msg.with_note(note));
        QualType::ERROR
    }

    /// Whether a placeholder may be the decl-specifier type of `d`, from
    /// its context alone (clang's `DeclaratorContext` switch; the
    /// decl-specifier cases are [`Sema::ctad_decl_spec_check`]'s).
    fn ctad_declarator_site(d: &Declarator) -> Option<Misplaced> {
        match d.site {
            DeclSite::Subst | DeclSite::New | DeclSite::TemplateParam => return None,
            DeclSite::NoDeduction | DeclSite::Alias | DeclSite::TypeParamDefault => {
                return Some(Misplaced::MissingArgs);
            }
            _ => {}
        }
        // A function declarator: the placeholder is its return type
        // (clang checks `D.isFunctionDeclarator()` last, so it wins).
        if matches!(d.chunks.first(), Some(Chunk::Function { .. })) {
            return Some(Misplaced::NotAllowed(site::RETURN_TYPE));
        }
        let s = match d.site {
            DeclSite::LambdaParam => site::LAMBDA_PARAM,
            DeclSite::RequiresParam => site::REQUIRES_PARAM,
            DeclSite::ConversionId => site::CONVERSION_ID,
            DeclSite::TemplateArg => site::TEMPLATE_ARG,
            _ => match d.ctx {
                DeclaratorContext::Prototype => site::PROTOTYPE,
                DeclaratorContext::TypeName => site::HERE,
                _ => return None,
            },
        };
        Some(Misplaced::NotAllowed(s))
    }

    /// The outermost declarator operator over a placeholder, which clang
    /// reports (`err_deduced_class_template_compound_type`'s selector
    /// text and where): grouping parentheses outside every chunk, else
    /// the last chunk. clang 22 no longer rejects the parentheses
    /// (`parens` false; spec/sema-templates.md §6.6).
    fn ctad_compound(d: &Declarator, parens: bool) -> Option<(&'static str, Loc)> {
        if parens
            && let Some((n, loc)) = d.outer_paren
            && n == d.chunks.len()
        {
            return Some(("use parentheses when declaring variable with", loc));
        }
        Some(match d.chunks.last()? {
            Chunk::Pointer { loc, .. } | Chunk::MemberPointer { loc, .. } => {
                ("form pointer to", *loc)
            }
            Chunk::LRef { loc } | Chunk::RRef { loc } => ("form reference to", *loc),
            Chunk::Array { loc, .. } => ("form array of", *loc),
            Chunk::Function { loc, .. } => ("form function returning", *loc),
        })
    }

    /// [dcl.type.class.deduct]/2 for the decl-specifier type `base` of
    /// declarator `d` whose type-specifier is at `spec_loc`: the error
    /// type after a diagnostic, else `base`. Called first by
    /// [`Sema::type_for_declarator_at`].
    pub(crate) fn ctad_declarator_check(
        &mut self,
        base: QualType,
        spec_loc: Loc,
        d: &Declarator,
    ) -> QualType {
        if !self.is_ctad_placeholder(base)
            || matches!(d.special, Some((SpecialName::DeductionGuide(_), _)))
        {
            return base;
        }
        if let Some(why) = Self::ctad_declarator_site(d) {
            return self.ctad_misplaced(base, spec_loc, why);
        }
        if d.site == DeclSite::Subst {
            return base;
        }
        if let Some((what, loc)) = Self::ctad_compound(d, self.clang_compat < 22) {
            let msg = self.diag(DiagId::sema_ctad_compound_type, loc).arg(what);
            self.emit(msg);
            return QualType::ERROR;
        }
        base
    }

    /// The decl-specifier cases of clang's `err_auto_not_allowed` for a
    /// simple-declaration or member-declaration `d` whose decl-specifier
    /// type is the placeholder `base` (`typedef`, a non-static data
    /// member, a friend): the error type after a diagnostic, else `base`.
    /// A function declarator is left to [`Sema::ctad_declarator_check`].
    pub(crate) fn ctad_decl_spec_check(
        &mut self,
        ds: &DeclSpec,
        base: QualType,
        d: &Declarator,
    ) -> QualType {
        if !self.is_ctad_placeholder(base)
            || matches!(d.chunks.first(), Some(Chunk::Function { .. }))
            || matches!(d.special, Some((SpecialName::DeductionGuide(_), _)))
        {
            return base;
        }
        let s = if ds.is_typedef() {
            site::TYPEDEF
        } else if d.ctx != DeclaratorContext::Member
            || matches!(ds.storage, Some((StorageSpec::Static, _)))
        {
            return base;
        } else if ds.friend.is_some() {
            site::FRIEND
        } else {
            let tag = self
                .current_class()
                .and_then(|c| self.ast.decls.record(c.record))
                .map_or(TagKind::Struct, |r| r.tag);
            match tag {
                TagKind::Union => site::UNION_MEMBER,
                TagKind::Class => site::CLASS_MEMBER,
                TagKind::Struct => site::STRUCT_MEMBER,
            }
        };
        self.ctad_misplaced(base, ds.type_loc.unwrap_or(ds.loc), Misplaced::NotAllowed(s))
    }

    /// A friend declaration whose decl-specifier type is the placeholder
    /// `base` and that declares no function (clang's
    /// `err_auto_not_allowed`, "in friend declaration").
    pub(crate) fn ctad_friend_check(
        &mut self,
        ds: &DeclSpec,
        base: QualType,
        d: Option<&Declarator>,
    ) -> QualType {
        if d.is_some_and(|d| matches!(d.chunks.first(), Some(Chunk::Function { .. }))) {
            return base;
        }
        self.ctad_misplaced(
            base,
            ds.type_loc.unwrap_or(ds.loc),
            Misplaced::NotAllowed(site::FRIEND),
        )
    }

    /// A class or alias template `name` written without arguments for a
    /// template type parameter (clang's `CheckTemplateTypeArgument` ->
    /// `diagnoseMissingTemplateArguments`): reported at `loc`; false (and
    /// nothing reported) for any other template.
    pub(crate) fn template_arg_missing_args(&mut self, name: TemplateNameId, loc: Loc) -> bool {
        let ty = self.ast.types.deduced_template_specialization(name, None);
        if self.undeduced_ctad(ty).is_none() {
            return false;
        }
        self.ctad_misplaced(ty, loc, Misplaced::MissingArgs);
        true
    }

    /// The exception-declaration `d` of a handler naming a placeholder
    /// (clang's `err_auto_not_allowed`, "in exception declaration"): the
    /// error type after a diagnostic, else `base`.
    pub(crate) fn catch_placeholder_check(
        &mut self,
        base: QualType,
        spec_loc: Loc,
        d: &Declarator,
    ) -> QualType {
        if let Some(kw) = self.declarator_placeholder(base, d) {
            self.auto_not_allowed(spec_loc, kw, site::CATCH);
            return QualType::ERROR;
        }
        self.ctad_misplaced(base, spec_loc, Misplaced::NotAllowed(site::CATCH))
    }

    // -- `auto` and `decltype(auto)` ---------------------------------------------------

    /// "'auto' not allowed <position>" at `loc` (clang's
    /// `err_auto_not_allowed`; `site` is its position selector).
    pub(crate) fn auto_not_allowed(&mut self, loc: Loc, kw: AutoKeyword, site: u8) {
        let msg = self.diag(DiagId::sema_auto_not_allowed, loc).arg(kw as u8).arg(site);
        self.emit(msg);
    }

    /// GNU `__auto_type` as the return type `ret` of a function
    /// declarator, never a deduced return type (clang
    /// `GetFullTypeForDeclarator`): "'`__auto_type`' not allowed in
    /// function return type" at `loc`, and true.
    pub fn gnu_auto_type_return_check(&mut self, ret: QualType, loc: Loc) -> bool {
        if self.placeholder_keyword(ret) != Some(AutoKeyword::GnuAutoType) {
            return false;
        }
        self.auto_not_allowed(loc, AutoKeyword::GnuAutoType, site::RETURN_TYPE);
        true
    }

    /// The `auto`/`decltype(auto)` whose position declarator `d` with
    /// decl-specifier type `base` decides (clang
    /// `GetDeclSpecTypeForDeclarator`): `base`'s own, or, when `d` has a
    /// trailing return type, the one contained in the trailing return
    /// type of its outermost function declarator (`using F =
    /// auto (*)(int) -> int;` names none). GNU `__auto_type` too (clang's
    /// selector 2).
    pub(crate) fn declarator_placeholder(
        &self,
        base: QualType,
        d: &Declarator,
    ) -> Option<AutoKeyword> {
        let TypeView::Auto { keyword, deduced: None, .. } = self.ast.types.canonical_view(base)
        else {
            return None;
        };
        let trailing = d.chunks.iter().find_map(|c| match c {
            Chunk::Function { trailing: Some((t, _)), .. } => Some(*t),
            _ => None,
        });
        match trailing {
            Some(t) => self.placeholder_keyword(t),
            None => Some(keyword),
        }
    }

    /// The position selector of a placeholder `auto` that declarator `d`
    /// does not allow, from its context alone (clang's `DeclaratorContext`
    /// switch, with `typedef` overriding it). A parameter of a function
    /// or lambda declarator is decided by the parser as it invents
    /// template parameters ([`Sema::param_placeholder_site`]); a
    /// non-static data member, a catch handler and an enum-base by their
    /// own checks.
    fn auto_declarator_site(d: &Declarator) -> Option<u8> {
        // A structured binding declaration (also a `typedef` one, which
        // gets only the decomposition error, as in clang) is checked by its own rules.
        if d.decomp.is_some() {
            return None;
        }
        if d.typedef {
            return Some(site::TYPEDEF);
        }
        Some(match d.site {
            DeclSite::Alias => site::ALIAS,
            DeclSite::TemplateArg | DeclSite::TypeParamDefault => site::TEMPLATE_ARG,
            DeclSite::RequiresParam => site::REQUIRES_PARAM,
            DeclSite::Plain | DeclSite::Reflect if d.ctx == DeclaratorContext::TypeName => {
                site::HERE
            }
            _ => return None,
        })
    }

    /// [dcl.spec.auto.general]/2-5 for the decl-specifier type `base` of
    /// declarator `d` whose type-specifier is at `spec_loc`: a placeholder
    /// where `d`'s position allows none is reported there, and the
    /// declarator's type is then the error type. Called by
    /// [`Sema::type_for_declarator_at`] after the deduced class type
    /// check.
    pub(crate) fn auto_declarator_check(
        &mut self,
        base: QualType,
        spec_loc: Loc,
        d: &Declarator,
    ) -> QualType {
        let Some(kw) = self.declarator_placeholder(base, d) else { return base };
        // GNU `__auto_type` is no deduced return type and no allocated
        // type; a conversion-type-id's `auto` is a deduced return type,
        // so C++14 on ([class.conv.fct]/9; clang
        // `GetDeclSpecTypeForDeclarator`'s `IsCXXAutoType`).
        let gnu = kw == AutoKeyword::GnuAutoType;
        let Some(s) = Self::auto_declarator_site(d).or(match d.site {
            DeclSite::ConversionId if gnu || self.std < kiln_lex::Std::Cxx14 => {
                Some(site::CONVERSION_ID)
            }
            DeclSite::New if gnu => Some(site::NEW),
            // A trailing return type's `auto` is deduced from C++14.
            DeclSite::NoDeduction if self.std < kiln_lex::Std::Cxx14 => Some(site::RETURN_TYPE),
            // [temp.param]/4: a placeholder parameter type is C++17.
            DeclSite::TemplateParam if self.std < kiln_lex::Std::Cxx17 => {
                Some(site::TEMPLATE_PARAM_17)
            }
            _ => None,
        }) else {
            return base;
        };
        self.auto_not_allowed(spec_loc, kw, s);
        QualType::ERROR
    }

    /// A type-id's type `ty` formed without a declarator (an enum-base)
    /// that is a placeholder: "'auto' not allowed here" at `loc`, and the
    /// error type.
    pub fn type_specifier_placeholder_check(&mut self, ty: QualType, loc: Loc) -> QualType {
        match self.ast.types.canonical_view(ty) {
            TypeView::Auto { keyword, deduced: None, .. } => {
                self.auto_not_allowed(loc, keyword, site::HERE);
                QualType::ERROR
            }
            _ => ty,
        }
    }

    /// Why a parameter's placeholder `auto` (`decltype(auto)` or
    /// `__auto_type` when `decltype_auto`) is ill-formed, as clang's
    /// position selector (`GetDeclSpecTypeForDeclarator`):
    /// `decltype(auto)` is never a parameter's type
    /// ([dcl.spec.auto.general]/2-3: "in function prototype", "in lambda
    /// parameter"); `auto` invents a template parameter of a lambda from
    /// C++14 (else "in lambda parameter before C++14") and of a function
    /// from C++20 (else "in function prototype"), and only in the clause
    /// of a function declaration or a lambda (`fn_decl`, [dcl.fct]/22:
    /// else "in function prototype that is not a function declaration").
    /// `None` when it invents one.
    pub fn param_placeholder_site(
        &self,
        decltype_auto: bool,
        fn_decl: bool,
        s: DeclSite,
    ) -> Option<u8> {
        use kiln_lex::Std;
        let lambda = s == DeclSite::LambdaParam;
        if decltype_auto {
            Some(if lambda { site::LAMBDA_PARAM } else { site::PROTOTYPE })
        } else if lambda && self.std < Std::Cxx14 {
            Some(site::LAMBDA_PARAM_14)
        } else if !lambda && self.std < Std::Cxx20 {
            Some(site::PROTOTYPE)
        } else if !fn_decl {
            Some(site::NOT_FUNCTION_DECLARATION)
        } else {
            None
        }
    }
}
