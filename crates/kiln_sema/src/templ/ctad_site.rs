//! Where a placeholder for a deduced class type may appear
//! ([dcl.type.class.deduct]/2): clang's `err_auto_not_allowed` (with a
//! deduced class template), `err_template_missing_args` where its parser
//! forms no placeholder, and `err_deduced_class_template_compound_type`
//! (`GetDeclSpecTypeForDeclarator` and `GetFullTypeForDeclarator`).

use kiln_ast::QualType;
use kiln_ast::decl::{DeclKind, TagKind};
use kiln_ast::ids::TemplateNameId;
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::declspec::{
    Chunk, DeclSite, DeclSpec, Declarator, DeclaratorContext, SpecialName, StorageSpec,
};

/// clang's `err_auto_not_allowed` context selectors used here.
mod site {
    pub(super) const PROTOTYPE: u8 = 0;
    pub(super) const STRUCT_MEMBER: u8 = 1;
    pub(super) const UNION_MEMBER: u8 = 3;
    pub(super) const CLASS_MEMBER: u8 = 5;
    pub(super) const CATCH: u8 = 7;
    pub(super) const TEMPLATE_ARG: u8 = 10;
    pub(super) const TYPEDEF: u8 = 11;
    pub(super) const RETURN_TYPE: u8 = 13;
    pub(super) const CONVERSION_ID: u8 = 14;
    pub(super) const HERE: u8 = 15;
    pub(super) const LAMBDA_PARAM: u8 = 16;
    pub(super) const FRIEND: u8 = 20;
    pub(super) const REQUIRES_PARAM: u8 = 22;
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
            DeclSite::NoDeduction => return Some(Misplaced::MissingArgs),
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
    /// the last chunk.
    fn ctad_compound(d: &Declarator) -> Option<(&'static str, Loc)> {
        if let Some((n, loc)) = d.outer_paren
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
        if let Some((what, loc)) = Self::ctad_compound(d) {
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

    /// The exception-declaration of a handler naming a placeholder
    /// (clang's `err_auto_not_allowed`, "in exception declaration").
    pub(crate) fn ctad_catch_check(&mut self, base: QualType, spec_loc: Loc) -> QualType {
        self.ctad_misplaced(base, spec_loc, Misplaced::NotAllowed(site::CATCH))
    }
}
