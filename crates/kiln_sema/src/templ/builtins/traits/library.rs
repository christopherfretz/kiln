//! The traits the IS defines only through a library template
//! ([meta.reflection.traits]/1 names them by their `<type_traits>`,
//! `<tuple>`, `<variant>` and `<functional>` counterparts) and that no
//! `__trait` builtin evaluates: swappability, invocability,
//! `common_type`/`common_reference`, `unwrap_reference`, and the tuple
//! and variant queries. The engine answers them by instantiating the
//! translation unit's own `std::NAME<args>` and reading its `value` or
//! `type` member (spec/reflection-traits.md §1.2), so a reflection query
//! and the library trait it mirrors cannot diverge.

use kiln_ast::decl::DeclKind;
use kiln_ast::types::TemplateArg;
use kiln_ast::{DeclId, QualType};
use kiln_oven::{EvalContext, EvalKind};
use kiln_source::Loc;

use crate::Sema;
use crate::lookup::LookupKind;
use crate::templ::actions::TemplateIdRef;

/// Why a library trait gave no answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LibFailure {
    /// `std::NAME` is not a class template declared in the TU (its header
    /// is not included), or the template-id is invalid.
    NotDeclared,
    /// `std::NAME<args>` is incomplete ([tuple.helper]/1: `tuple_size<T>`
    /// is undefined for a type that is not tuple-like).
    Incomplete,
    /// The specialization has no usable `value` / `type` member
    /// (`common_type<int, S>` with no common type).
    NoMember,
}

impl Sema<'_, '_> {
    /// The definition of the complete class `std::NAME<args>`, instantiated
    /// at `loc` without diagnosing an undefined template.
    pub(crate) fn library_trait_class(
        &mut self,
        name: &str,
        args: &[TemplateArg],
        loc: Loc,
    ) -> Result<DeclId, LibFailure> {
        let (_, tmpl) = self.std_class_template(name).ok_or(LibFailure::NotDeclared)?;
        let tn = self.template_name_for(tmpl);
        let Ok(TemplateIdRef::Type(t)) = self.act_template_id(tn, args, loc, loc) else {
            return Err(LibFailure::NotDeclared);
        };
        if !self.require_complete_silent(t, loc) {
            return Err(LibFailure::Incomplete);
        }
        let r = self.ast.types.as_record(t).ok_or(LibFailure::NotDeclared)?;
        self.ast.definition(r).ok_or(LibFailure::Incomplete)
    }

    /// `std::NAME<args>::value` as an integer (a `bool` is 0 or 1).
    pub(crate) fn library_trait_value(
        &mut self,
        name: &str,
        args: &[TemplateArg],
        loc: Loc,
    ) -> Result<i128, LibFailure> {
        let c = self.library_trait_class(name, args, loc)?;
        let value = self.intern("value");
        let found = self.lookup_in_class(c, value, LookupKind::Ordinary);
        let d = found
            .decls
            .first()
            .copied()
            .filter(|&d| self.kind_of(d) == DeclKind::Var)
            .ok_or(LibFailure::NoMember)?;
        let e = self.build_decl_ref(d, loc);
        if self.is_err_expr(e) {
            return Err(LibFailure::NoMember);
        }
        self.eval_int(e, EvalContext::new(EvalKind::Other, loc)).map_err(|_| LibFailure::NoMember)
    }

    /// `std::NAME<args>::type`, canonical.
    pub(crate) fn library_trait_type(
        &mut self,
        name: &str,
        args: &[TemplateArg],
        loc: Loc,
    ) -> Result<QualType, LibFailure> {
        let c = self.library_trait_class(name, args, loc)?;
        let ty = self.intern("type");
        let found = self.lookup_in_class(c, ty, LookupKind::Ordinary);
        let td = found
            .decls
            .first()
            .copied()
            .filter(|&d| crate::lookup::is_type_kind(self.kind_of(d)))
            .ok_or(LibFailure::NoMember)?;
        let named = match self.kind_of(td) {
            DeclKind::Typedef | DeclKind::TypeAlias | DeclKind::TemplateTypeParm => {
                self.ast.decls.get(td).ty().ok_or(LibFailure::NoMember)?
            }
            _ => self.tag_type(td),
        };
        if named == QualType::ERROR {
            return Err(LibFailure::NoMember);
        }
        Ok(self.ast.types.canonical(named))
    }

    /// `std::is_structural<T>` ([meta.unary.prop]; no `__trait` builtin):
    /// a scalar type, an lvalue reference type, or a literal class type
    /// whose bases and non-static data members are public, non-mutable
    /// and structural ([temp.param]/7). `None` for an incomplete class,
    /// which must be complete.
    pub(crate) fn is_structural_type(&mut self, q: QualType, loc: Loc) -> Option<bool> {
        let tys = &self.ast.types;
        if matches!(tys.canonical_view(q), kiln_ast::types::TypeView::LValueRef(_)) {
            return Some(true);
        }
        if tys.as_record(q).is_some() {
            if !self.trait_complete(q, loc) {
                self.trait_incomplete(kiln_diag::DiagId::sema_incomplete_type_trait, loc, q);
                return None;
            }
            return Some(self.is_structural(q));
        }
        Some(self.is_scalar_ext(q))
    }
}
