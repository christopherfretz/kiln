//! The query family ([meta.reflection.queries] and P3096's parameter
//! queries; spec/reflection.md §3.3, spec/reflection-queries.md).
//!
//! Every op here is O(1) in the reflection, apart from the walks over a
//! declaration's contexts or redeclarations, and allocates nothing but
//! its result. A `Stable` op leaves the call's purity untouched, so the
//! Oven's consteval-call memo caches the calls built from them. A
//! `PointDependent` op (`is_complete_type`, `is_enumerable_type`,
//! `is_defaulted`, `has_default_argument`) makes its caller impure, so a
//! call built from one is never memoized (`kiln_oven::vm::meta`).
//! No result is cached on the sema side.

mod entity;
mod linkage;
mod params;
mod pred;

use kiln_ast::decl::DeclKind;
use kiln_ast::reflect::Reflectee;
use kiln_ast::{DeclId, QualType, ReflectionId};
use kiln_oven::{MetaCtx, MetaError, MetaOp, MetaOut, Oven, QueryOp, info_arg};
use kiln_source::Loc;

use super::fail;
use crate::Sema;

impl Sema<'_, '_> {
    pub(in crate::meta) fn meta_query(
        &mut self,
        q: QueryOp,
        args: &[u128],
        out: &mut MetaOut,
        ctx: &MetaCtx,
        oven: &mut Oven,
    ) -> Result<(), MetaError> {
        let loc = ctx.loc;
        if let QueryOp::SymbolOf | QueryOp::U8SymbolOf = q {
            let op = *args.first().ok_or_else(MetaError::invalid)?;
            *out = MetaOut::Frozen(self.reflect_symbol(op, q == QueryOp::U8SymbolOf, loc)?);
            return Ok(());
        }
        let r = info_arg(args, 0)?;
        if let Some(b) = self.query_pred(q, r, ctx, oven)? {
            *out = MetaOut::Scalar(u128::from(b));
            return Ok(());
        }
        *out = match q {
            QueryOp::TypeOf => MetaOut::Info(self.reflect_type_of(r, loc)?),
            QueryOp::ObjectOf => MetaOut::Info(self.reflect_object_of(r, loc)?),
            QueryOp::ConstantOf => MetaOut::Info(self.reflect_constant_of(r, loc, oven)?),
            QueryOp::ParentOf => match self.parent_of(self.ast.reflections.get(r)) {
                Some(p) => MetaOut::Info(self.ast.reflections.intern(p)),
                None => return fail(loc, "reflection does not represent an entity with parent"),
            },
            QueryOp::Dealias => MetaOut::Info(self.dealias(r)),
            QueryOp::OperatorOf => MetaOut::Scalar(u128::from(self.reflect_operator_of(r, loc)?)),
            QueryOp::VariableOf => MetaOut::Info(self.reflect_variable_of(r, loc)?),
            QueryOp::ReturnTypeOf => MetaOut::Info(self.reflect_return_type_of(r, loc)?),
            _ => return Err(MetaError::not_implemented(MetaOp::Query(q), loc)),
        };
        Ok(())
    }

    /// The type `type_of(r)` yields, as the declaration or value carries
    /// it (not canonicalized), if `r` has one ([meta.reflection.queries]
    /// *has-type*): a value, object, variable, function, data member,
    /// enumerator, function parameter or direct base.
    fn type_of_reflectee(&self, x: Reflectee) -> Option<QualType> {
        let ty = match x {
            Reflectee::Decl(d) => match self.kind_of(d) {
                DeclKind::Var
                | DeclKind::ParmVar
                | DeclKind::Binding
                | DeclKind::VarTemplateSpecialization
                | DeclKind::Field
                | DeclKind::IndirectField
                | DeclKind::Function
                | DeclKind::CXXMethod
                | DeclKind::CXXConstructor
                | DeclKind::CXXDestructor
                | DeclKind::CXXConversion
                | DeclKind::EnumConstant => self.ast.decls.get(d).ty(),
                _ => None,
            },
            Reflectee::Value(v) => Some(self.ast.consts.ty(v)),
            Reflectee::Object(v) => self.ast.types.pointee(self.ast.consts.ty(v)),
            Reflectee::FnParam(f, i) => {
                let p = self.fn_param(f, i)?;
                self.ast.decls.get(p).ty()
            }
            Reflectee::BaseSpec(c, i) => self.base_spec(c, i).map(|b| b.ty),
            _ => None,
        };
        ty.filter(|&t| t != QualType::ERROR)
    }

    /// `type_of(r)` ([meta.reflection.queries]): the type of what `r`
    /// represents, canonical (GCC 16.1 drops a type alias there too:
    /// `using A = int; A v;` gives `type_of(^^v) == ^^int`).
    fn reflect_type_of(&mut self, r: ReflectionId, loc: Loc) -> Result<ReflectionId, MetaError> {
        match self.type_of_reflectee(self.ast.reflections.get(r)) {
            Some(t) if !self.ast.types.is_dependent(t) => {
                let t = self.ast.types.canonical(t);
                Ok(self.ast.reflections.intern(Reflectee::Type(t)))
            }
            _ => fail(loc, format!("'{}' has no type", self.reflection_str(r))),
        }
    }

    /// The `i`-th parameter declaration of function `f`'s first
    /// declaration.
    fn fn_param(&self, f: DeclId, i: u32) -> Option<DeclId> {
        let data = self.ast.decls.function(self.ast.first(f))?;
        self.ast.lists.iter::<DeclId>(data.params).nth(i as usize)
    }

    /// The `i`-th direct base specifier of class `c`.
    fn base_spec(&self, c: DeclId, i: u32) -> Option<kiln_ast::decl::BaseSpec> {
        let def = self.ast.definition(c)?;
        self.ast.decls.bases(def, &self.ast.lists).nth(i as usize).copied()
    }
}
