//! The `is_*` / `has_*` predicates of [meta.reflection.queries]. Each
//! answers `false` for a reflection it does not apply to (the IS makes
//! none of them throw); only `is_complete_type`, `is_enumerable_type`
//! and `is_noexcept` may instantiate, and they fail silently in a trial
//! evaluation rather than instantiate from one.

use kiln_ast::decl::{Access, DeclKind, Defaulted, DefinitionBits, EnumData};
use kiln_ast::names::{DeclNameView, OverloadedOperator};
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::RefQual;
use kiln_ast::{DeclId, QualType, ReflectionId};
use kiln_oven::{MetaCtx, MetaError, Oven, QueryOp};

use super::linkage::Linkage;
use crate::Sema;
use crate::class::SpecialKind;

impl Sema<'_, '_> {
    /// The answer of predicate `q` for `r`, or `None` when `q` is not a
    /// predicate.
    pub(super) fn query_pred(
        &mut self,
        q: QueryOp,
        r: ReflectionId,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<Option<bool>, MetaError> {
        use QueryOp as Q;
        let x = self.ast.reflections.get(r);
        let b = match q {
            Q::HasParent => self.parent_of(x).is_some(),
            Q::HasTemplateArguments => self.has_template_arguments(x),
            Q::IsFunctionParameter => matches!(x, Reflectee::FnParam(..)),
            Q::IsExplicitObjectParameter => self.is_explicit_object_parameter(x),
            Q::HasDefaultArgument => self.has_default_argument(x),
            Q::IsVarargFunction => self.is_vararg_function(x),
            Q::IsPublic => self.reflected_access(x) == Some(Access::Public),
            Q::IsProtected => self.reflected_access(x) == Some(Access::Protected),
            Q::IsPrivate => self.reflected_access(x) == Some(Access::Private),
            Q::IsVirtual => match x {
                Reflectee::BaseSpec(c, i) => self.base_spec(c, i).is_some_and(|b| b.is_virtual),
                _ => self.fn_decl(x).is_some_and(|f| crate::class::is_virtual(&self.ast, f)),
            },
            Q::IsPureVirtual => self.fn_decl(x).is_some_and(|f| {
                crate::class::is_virtual(&self.ast, f) && self.fn_spec(self.ast.first(f)).is_pure()
            }),
            Q::IsOverride => self.fn_decl(x).is_some_and(|f| {
                let first = self.ast.first(f);
                self.ast
                    .decls
                    .cold
                    .overrides
                    .get(first)
                    .is_some_and(|&l| self.ast.lists.iter::<DeclId>(l).next().is_some())
            }),
            Q::IsFinal => match x {
                Reflectee::Type(t) => self
                    .ast
                    .types
                    .as_record(t)
                    .and_then(|d| self.ast.definition(d))
                    .and_then(|d| self.ast.decls.record(d))
                    .is_some_and(|r| r.defn.contains(DefinitionBits::FINAL)),
                _ => self.fn_decl(x).is_some_and(|f| self.fn_spec(self.ast.first(f)).is_final()),
            },
            Q::IsDeleted => {
                self.fn_decl(x).is_some_and(|f| self.fn_spec(self.ast.first(f)).is_deleted())
            }
            // A later redeclaration can explicitly default the function.
            Q::IsDefaulted => self.fn_decl(x).is_some_and(|f| {
                self.ast.redecls_of(f).any(|d| self.fn_spec(d).defaulted() != Defaulted::No)
            }),
            Q::IsUserProvided => self.fn_decl(x).is_some_and(|f| self.is_user_provided(f)),
            Q::IsUserDeclared => {
                self.fn_decl(x).is_some_and(|f| self.is_user_declared(self.ast.first(f)))
            }
            Q::IsExplicit => self.fn_decl(x).is_some_and(|f| {
                matches!(self.kind_of(f), DeclKind::CXXConstructor | DeclKind::CXXConversion)
                    && self.fn_spec(self.ast.first(f)).is_explicit()
            }),
            Q::IsNoexcept => return self.is_noexcept(x, ctx, oven).map(Some),
            Q::IsBitField => self.field(x).is_some_and(|f| f.bit_width.is_some()),
            Q::IsEnumerator => self.decl_kind(x) == Some(DeclKind::EnumConstant),
            Q::IsConst => self.qualified_type(x).is_some_and(|t| self.type_quals_of(t).0),
            Q::IsVolatile => self.qualified_type(x).is_some_and(|t| self.type_quals_of(t).1),
            Q::IsMutableMember => self.field(x).is_some_and(kiln_ast::decl::FieldData::is_mutable),
            Q::IsLvalueReferenceQualified => self.ref_qual_of(x) == RefQual::LValue,
            Q::IsRvalueReferenceQualified => self.ref_qual_of(x) == RefQual::RValue,
            Q::HasStaticStorageDuration => self.has_static_storage_duration(x),
            Q::HasThreadStorageDuration => self.has_thread_storage_duration(x),
            Q::HasAutomaticStorageDuration => self.has_automatic_storage_duration(x),
            Q::HasInternalLinkage => self.linkage_of(x) == Linkage::Internal,
            Q::HasModuleLinkage => self.linkage_of(x) == Linkage::Module,
            Q::HasExternalLinkage => self.linkage_of(x) == Linkage::External,
            Q::HasCLanguageLinkage => self.has_c_language_linkage(x),
            Q::HasLinkage => self.linkage_of(x) != Linkage::None,
            Q::IsCompleteType => return self.is_complete_type(x, ctx, oven).map(Some),
            Q::IsEnumerableType => return self.is_enumerable_type(x, ctx, oven).map(Some),
            _ => return Ok(self.entity_pred(q, x)),
        };
        Ok(Some(b))
    }

    /// The kind, special-member, template and membership predicates:
    /// the answer of `q` for `x`, or `None` when `q` is not one of them.
    fn entity_pred(&self, q: QueryOp, x: Reflectee) -> Option<bool> {
        use QueryOp as Q;
        let b = match q {
            Q::IsVariable => matches!(
                self.decl_kind(x),
                Some(DeclKind::Var | DeclKind::VarTemplateSpecialization | DeclKind::ParmVar)
            ),
            Q::IsType => matches!(x, Reflectee::Type(_)),
            Q::IsNamespace => matches!(x, Reflectee::Namespace(_) | Reflectee::NamespaceAlias(_)),
            Q::IsTypeAlias => matches!(x, Reflectee::Type(t) if self.is_alias_type(t)),
            Q::IsNamespaceAlias => matches!(x, Reflectee::NamespaceAlias(_)),
            Q::IsFunction => self.fn_decl(x).is_some(),
            Q::IsConversionFunction => self.decl_kind(x) == Some(DeclKind::CXXConversion),
            Q::IsOperatorFunction => self
                .fn_decl(x)
                .is_some_and(|f| matches!(self.fn_name(f), DeclNameView::Operator(_))),
            Q::IsLiteralOperator => self
                .fn_decl(x)
                .is_some_and(|f| matches!(self.fn_name(f), DeclNameView::LiteralOperator(_))),
            Q::IsSpecialMemberFunction => self.special_kind(x).is_some(),
            Q::IsConstructor => self.decl_kind(x) == Some(DeclKind::CXXConstructor),
            Q::IsDefaultConstructor => self.special_kind(x) == Some(SpecialKind::DefaultCtor),
            Q::IsCopyConstructor => self.special_kind(x) == Some(SpecialKind::CopyCtor),
            Q::IsMoveConstructor => self.special_kind(x) == Some(SpecialKind::MoveCtor),
            Q::IsAssignment => self.fn_decl(x).is_some_and(|f| {
                self.kind_of(f) == DeclKind::CXXMethod
                    && matches!(self.fn_name(f), DeclNameView::Operator(OverloadedOperator::Equal))
            }),
            Q::IsCopyAssignment => self.special_kind(x) == Some(SpecialKind::CopyAssign),
            Q::IsMoveAssignment => self.special_kind(x) == Some(SpecialKind::MoveAssign),
            Q::IsDestructor => self.decl_kind(x) == Some(DeclKind::CXXDestructor),
            Q::IsTemplate => self.template_kind(x).is_some(),
            Q::IsFunctionTemplate => self.template_kind(x) == Some(DeclKind::FunctionTemplate),
            Q::IsVariableTemplate => self.template_kind(x) == Some(DeclKind::VarTemplate),
            Q::IsClassTemplate => self.template_kind(x) == Some(DeclKind::ClassTemplate),
            Q::IsAliasTemplate => self.template_kind(x) == Some(DeclKind::TypeAliasTemplate),
            Q::IsConcept => self.template_kind(x) == Some(DeclKind::Concept),
            Q::IsConversionFunctionTemplate => self
                .fn_template_pattern(x)
                .is_some_and(|p| self.kind_of(p) == DeclKind::CXXConversion),
            Q::IsOperatorFunctionTemplate => self
                .fn_template_pattern(x)
                .is_some_and(|p| matches!(self.fn_name(p), DeclNameView::Operator(_))),
            Q::IsLiteralOperatorTemplate => self
                .fn_template_pattern(x)
                .is_some_and(|p| matches!(self.fn_name(p), DeclNameView::LiteralOperator(_))),
            Q::IsConstructorTemplate => self
                .fn_template_pattern(x)
                .is_some_and(|p| self.kind_of(p) == DeclKind::CXXConstructor),
            Q::IsStructuredBinding => self.decl_kind(x) == Some(DeclKind::Binding),
            Q::IsValue => matches!(x, Reflectee::Value(_)),
            Q::IsObject => matches!(x, Reflectee::Object(_)),
            Q::IsClassMember => {
                self.member_parent(x).is_some_and(|c| is_class_kind(self.kind_of(c)))
            }
            Q::IsNamespaceMember => self.member_parent(x).is_some_and(|c| {
                matches!(self.kind_of(c), DeclKind::TranslationUnit | DeclKind::Namespace)
            }),
            Q::IsNonstaticDataMember => {
                matches!(self.decl_kind(x), Some(DeclKind::Field | DeclKind::IndirectField))
            }
            Q::IsStaticMember => self.is_static_member(x),
            Q::IsBase => matches!(x, Reflectee::BaseSpec(..)),
            Q::HasDefaultMemberInitializer => self.field(x).is_some_and(|f| f.init.is_some()),
            _ => return None,
        };
        Some(b)
    }

    /// The kind of declaration `x` reflects, if it is a `Decl`.
    fn decl_kind(&self, x: Reflectee) -> Option<DeclKind> {
        match x {
            Reflectee::Decl(d) => Some(self.kind_of(d)),
            _ => None,
        }
    }

    /// The function `x` reflects (not a deduction guide, not a template).
    pub(super) fn fn_decl(&self, x: Reflectee) -> Option<DeclId> {
        let Reflectee::Decl(d) = x else { return None };
        let k = self.kind_of(d);
        (k.is_function() && k != DeclKind::CXXDeductionGuide).then_some(d)
    }

    /// The name of function `f`.
    fn fn_name(&self, f: DeclId) -> DeclNameView {
        self.ast.names.view(self.ast.decls.get(f).name())
    }

    /// The field payload of the non-static data member `x` reflects.
    fn field(&self, x: Reflectee) -> Option<&kiln_ast::decl::FieldData> {
        match x {
            Reflectee::Decl(d) if self.kind_of(d) == DeclKind::Field => self.ast.decls.field(d),
            _ => None,
        }
    }

    /// Which special member function `x` reflects.
    fn special_kind(&self, x: Reflectee) -> Option<SpecialKind> {
        self.fn_decl(x).and_then(|f| self.special_kind_of(self.ast.first(f)))
    }

    /// The kind of the template declaration `x` reflects.
    fn template_kind(&self, x: Reflectee) -> Option<DeclKind> {
        let Reflectee::Template(n) = x else { return None };
        self.ast.types.template_decl(n).map(|d| self.kind_of(d))
    }

    /// The pattern of the function template `x` reflects.
    fn fn_template_pattern(&self, x: Reflectee) -> Option<DeclId> {
        let Reflectee::Template(n) = x else { return None };
        let td = self.ast.types.template_decl(n)?;
        if self.kind_of(td) != DeclKind::FunctionTemplate {
            return None;
        }
        self.ast.template_info(td)?.pattern
    }

    /// The class or namespace (or function, for a local entity) `x` is a
    /// member of: the semantic context of its declaration, through
    /// linkage specifications and `export`. An enumerator of an unscoped
    /// enumeration is a member of the enumeration's scope too
    /// ([dcl.enum]/11); one of a scoped enumeration is a member of
    /// neither (GCC 16.1 agrees). A type that is not a class,
    /// enumeration or alias is no member (GCC 16.1 makes `^^int` a
    /// namespace member).
    fn member_parent(&self, x: Reflectee) -> Option<DeclId> {
        let d = match x {
            Reflectee::BaseSpec(..) | Reflectee::FnParam(..) => return None,
            _ => self.reflectee_decl(x)?,
        };
        let d = match self.kind_of(d) {
            DeclKind::EnumConstant => {
                let e = self.ast.decls.get(d).ctx()?;
                if self.ast.decls.enumeration(e).is_some_and(|e| e.flags & EnumData::SCOPED != 0) {
                    return None;
                }
                e
            }
            _ => d,
        };
        self.semantic_parent(self.ast.first(d))
    }

    /// The access of the class member or base specifier `x` reflects
    /// (an unscoped member enumeration's enumerators have the
    /// enumeration's).
    fn reflected_access(&self, x: Reflectee) -> Option<Access> {
        if let Reflectee::BaseSpec(c, i) = x {
            let b = self.base_spec(c, i)?;
            return Some(self.base_access(self.ast.definition(c).unwrap_or(c), b.access));
        }
        let c = self.member_parent(x)?;
        if !is_class_kind(self.kind_of(c)) {
            return None;
        }
        let d = self.reflectee_decl(x)?;
        let d = match self.kind_of(d) {
            DeclKind::EnumConstant => self.ast.decls.get(d).ctx()?,
            // A member of an anonymous union has its access in the union.
            DeclKind::IndirectField => {
                let chain = self.ast.indirect_field_chain(d);
                self.ast.lists.iter::<DeclId>(chain).last()?
            }
            _ => d,
        };
        match self.ast.decls.get(self.ast.first(d)).misc().access() {
            Access::None => None,
            a => Some(a),
        }
    }

    /// `is_static_member`: a static data member, static member function,
    /// or a template of either.
    fn is_static_member(&self, x: Reflectee) -> bool {
        let in_class = |s: &Self| s.member_parent(x).is_some_and(|c| is_class_kind(s.kind_of(c)));
        match x {
            Reflectee::Decl(d) => match self.kind_of(d) {
                DeclKind::Var | DeclKind::VarTemplateSpecialization => in_class(self),
                DeclKind::CXXMethod => self.is_static_method(self.ast.first(d)),
                _ => false,
            },
            Reflectee::Template(n) => match self.ast.types.template_decl(n) {
                Some(td) => match self.kind_of(td) {
                    DeclKind::VarTemplate => in_class(self),
                    DeclKind::FunctionTemplate => self
                        .ast
                        .template_info(td)
                        .and_then(|i| i.pattern)
                        .is_some_and(|p| self.is_static_method(self.ast.first(p))),
                    _ => false,
                },
                None => false,
            },
            _ => false,
        }
    }

    /// The type `is_const`, `is_volatile` and the ref-qualifier queries
    /// inspect: `type_of(r)` if `r` has a type, else the type `r`
    /// reflects ([meta.reflection.queries]).
    fn qualified_type(&self, x: Reflectee) -> Option<QualType> {
        match x {
            Reflectee::Type(t) => Some(t),
            _ => self.type_of_reflectee(x),
        }
    }

    /// Whether type `t` is const / volatile, a function type by its
    /// cv-qualifier-seq.
    fn type_quals_of(&self, t: QualType) -> (bool, bool) {
        let c = self.ast.types.canonical(t);
        if let Some(p) = self.ast.types.proto(c) {
            let q = p.info().this_quals();
            return (q.is_const(), q.is_volatile());
        }
        let q = self.ast.types.effective_quals(c);
        (q.is_const(), q.is_volatile())
    }

    /// The ref-qualifier of the function or function type `x` reflects.
    fn ref_qual_of(&self, x: Reflectee) -> RefQual {
        let Some(t) = self.qualified_type(x) else { return RefQual::None };
        let c = self.ast.types.canonical(t);
        self.ast.types.proto(c).map_or(RefQual::None, |p| p.info().ref_qual())
    }

    /// Whether instantiation is allowed from this evaluation: not from a
    /// trial evaluation (the pattern of `reflected_complete_class`).
    fn may_instantiate(&self, ctx: &MetaCtx) -> bool {
        !ctx.trial && self.inst.eval_loc.is_some()
    }

    /// `is_noexcept`: a function with a non-throwing exception
    /// specification (resolved, instantiated if deferred) or a
    /// `noexcept` function type. A function template is not.
    fn is_noexcept(
        &mut self,
        x: Reflectee,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<bool, MetaError> {
        if let Some(f) = self.fn_decl(x) {
            return match self.known_nothrow(f) {
                Some(b) => Ok(b),
                None if self.may_instantiate(ctx) => {
                    let loc = ctx.loc;
                    Ok(self.lend_oven(oven, |s| s.resolve_nothrow(f, loc)))
                }
                None => Err(MetaError::silent()),
            };
        }
        let Reflectee::Type(t) = x else { return Ok(false) };
        let c = self.ast.types.canonical(t);
        Ok(self.ast.types.proto(c).is_some_and(|p| self.ast.types.exc_spec(p).is_nothrow()))
    }

    /// `is_complete_type` ([meta.reflection.queries]): `dealias(r)` is a
    /// type complete here, instantiating a class template specialization
    /// first (GCC 16.1 does: `is_complete_type(^^X<long>)` is true for a
    /// definable `X<long>` not yet instantiated).
    fn is_complete_type(
        &mut self,
        x: Reflectee,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<bool, MetaError> {
        let Reflectee::Type(t) = x else { return Ok(false) };
        self.complete_here(t, ctx, oven)
    }

    /// `is_enumerable_type`: a class type complete here, or an
    /// enumeration whose definition is reachable and does not enclose
    /// this point (an opaque enumeration declaration is not one).
    fn is_enumerable_type(
        &mut self,
        x: Reflectee,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<bool, MetaError> {
        let Reflectee::Type(t) = x else { return Ok(false) };
        if let Some(e) = self.ast.types.as_enum(t) {
            if self.undefined_member_enum(t).is_some() && self.may_instantiate(ctx) {
                let loc = ctx.loc;
                self.lend_oven(oven, |s| s.require_member_enum(e, loc));
            }
            return Ok(self
                .ast
                .definition(e)
                .and_then(|d| self.ast.decls.enumeration(d))
                .is_some_and(|d| {
                    d.flags & EnumData::COMPLETE != 0 && d.flags & EnumData::BEING_DEFINED == 0
                }));
        }
        if self.ast.types.as_record(t).is_none() {
            return Ok(false);
        }
        self.complete_here(t, ctx, oven)
    }

    /// Whether `t` is complete here, instantiating it first if needed and
    /// allowed (a trial fails silently instead).
    fn complete_here(
        &mut self,
        t: QualType,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<bool, MetaError> {
        if self.ast.types.is_dependent(t) {
            return Ok(false);
        }
        if self.undefined_spec(t).is_some() {
            if !self.may_instantiate(ctx) {
                return Err(MetaError::silent());
            }
            let loc = ctx.loc;
            return Ok(self.lend_oven(oven, |s| s.require_complete_silent(t, loc)));
        }
        Ok(self.is_complete(t))
    }
}

/// A class declaration kind.
pub(super) fn is_class_kind(k: DeclKind) -> bool {
    matches!(
        k,
        DeclKind::CXXRecord
            | DeclKind::ClassTemplateSpecialization
            | DeclKind::ClassTemplatePartialSpecialization
    )
}
