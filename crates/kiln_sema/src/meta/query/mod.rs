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

use kiln_ast::consts::{PtrBase, Scalar};
use kiln_ast::decl::DeclKind;
use kiln_ast::reflect::Reflectee;
use kiln_ast::types::{Quals, TypeView};
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
            let (cv, len) = self.reflect_symbol(op, q == QueryOp::U8SymbolOf, loc)?;
            // `std::[u8]string_view` (the standard's, through <meta>) or
            // the NTBS of tests/support/kiln_meta_queries.h.
            let view = ctx
                .callee
                .and_then(|f| self.ast.decls.get(f).ty())
                .and_then(|t| self.ast.types.proto(t))
                .is_some_and(|p| {
                    self.ast.types.as_record(self.ast.types.canonical(p.ret())).is_some()
                });
            *out = if view { MetaOut::Str(cv, len) } else { MetaOut::Frozen(cv) };
            return Ok(());
        }
        let r = info_arg(args, 0)?;
        if let Some(b) = self.query_pred(q, r, ctx, oven)? {
            *out = MetaOut::Scalar(u128::from(b));
            return Ok(());
        }
        *out = match q {
            QueryOp::TypeOf => MetaOut::Info(self.reflect_type_of(r, loc, oven)?),
            QueryOp::ObjectOf => {
                self.use_reflected_var(r, loc, oven);
                MetaOut::Info(self.reflect_object_of(r, loc)?)
            }
            QueryOp::ConstantOf => {
                self.use_reflected_var(r, loc, oven);
                MetaOut::Info(self.reflect_constant_of(r, loc, oven)?)
            }
            QueryOp::ParentOf => match self.parent_of(self.ast.reflections.get(r)) {
                Some(p) => MetaOut::Info(self.intern_reflection(p, loc)?),
                None => return fail(loc, "reflection does not represent an entity with parent"),
            },
            QueryOp::Dealias => MetaOut::Info(self.dealias(r, loc)?),
            QueryOp::OperatorOf => MetaOut::Scalar(u128::from(self.reflect_operator_of(r, loc)?)),
            QueryOp::VariableOf => MetaOut::Info(self.reflect_variable_of(r, loc)?),
            QueryOp::ReturnTypeOf => MetaOut::Info(self.reflect_return_type_of(r, loc)?),
            _ => return Err(MetaError::not_implemented(MetaOp::Query(q), loc)),
        };
        Ok(())
    }

    /// `is_annotation` / `is_data_member_spec`: the reflection is an
    /// annotation / a data member description. Nothing builds either yet
    /// (WP-481 / WP-480), so both are false for every reflection today.
    pub(in crate::meta) fn meta_kind_pred(
        &mut self,
        op: MetaOp,
        args: &[u128],
        out: &mut MetaOut,
    ) -> Result<(), MetaError> {
        let x = self.ast.reflections.get(info_arg(args, 0)?);
        let b = match op {
            MetaOp::Annot(_) => matches!(x, Reflectee::Annotation(_)),
            _ => matches!(x, Reflectee::DataMemberSpec(_)),
        };
        *out = MetaOut::Scalar(u128::from(b));
        Ok(())
    }

    /// The type `type_of(r)` yields, as the declaration or value carries
    /// it (not canonicalized), if `r` has one ([meta.reflection.queries]
    /// *has-type*): a value, object, variable, function other than a
    /// constructor or destructor, data member, enumerator, function
    /// parameter or direct base.
    pub(in crate::meta) fn type_of_reflectee(&self, x: Reflectee) -> Option<QualType> {
        let ty = match x {
            Reflectee::Decl(d) => match self.kind_of(d) {
                DeclKind::Var | DeclKind::VarTemplateSpecialization => self.reflected_var_type(d),
                DeclKind::ParmVar
                | DeclKind::Binding
                | DeclKind::Field
                | DeclKind::IndirectField
                | DeclKind::Function
                | DeclKind::CXXMethod
                | DeclKind::CXXConversion
                | DeclKind::EnumConstant => self.ast.decls.get(d).ty(),
                _ => None,
            },
            Reflectee::Value(v) => Some(self.ast.consts.ty(v)),
            Reflectee::Object(v) => self.ast.types.pointee(self.ast.consts.ty(v)),
            // The parameter's type in the function's type ([dcl.fct]/5:
            // adjusted, top-level cv-qualifiers dropped), as GCC 16.1 gives
            // it: `void f(const int)` has `type_of(parameters_of(^^f)[0])
            // == ^^int`, whatever any declaration wrote.
            Reflectee::FnParam(f, i) => {
                let fty = self.ast.decls.get(f).ty();
                let adjusted = fty.and_then(|t| self.ast.types.proto(t)).and_then(|p| {
                    self.ast.types.params(p).get(i as usize).map(|q| q.without_quals())
                });
                match adjusted {
                    Some(q) => Some(q),
                    None => self.fn_param(f, i).and_then(|p| self.ast.decls.get(p).ty()),
                }
            }
            Reflectee::BaseSpec(c, i) => self.base_spec(c, i).map(|b| b.ty),
            // An annotation has the type of its underlying constant
            // (P3394R4 has-type; `type_of(constant_of(a))`): `T` as
            // `reflect_constant` deduces it. A class `T` is a const
            // template parameter object; `const_object` adds the `const`.
            // None while the operand is dependent.
            Reflectee::Annotation(a) => {
                let n = self.ast.annots.get(a);
                n.value.map(|_| n.written_type)
            }
            _ => None,
        };
        ty.filter(|&t| t != QualType::ERROR)
    }

    /// The type of variable `d` as its latest declaration so far declares
    /// it. A reflection designates the first declaration, but a later one
    /// can complete an array of unknown bound ([basic.types.general]/6,
    /// [dcl.array]/8): after `struct S { static const char k[]; };` and
    /// `const char S::k[] = "ab";` the type is `const char[3]`, as an
    /// expression naming `S::k` sees it (`decltype`, `sizeof`) and as
    /// GCC 16.1's `type_of` answers (WP-795).
    pub(in crate::meta) fn reflected_var_type(&self, d: DeclId) -> Option<QualType> {
        self.ast.decls.get(self.ast.latest(d)).ty()
    }

    /// `type_of(r)` ([meta.reflection.queries]): the type of what `r`
    /// represents, canonical (GCC 16.1 drops a type alias there too:
    /// `using A = int; A v;` gives `type_of(^^v) == ^^int`). A variable's
    /// type still an array of unknown bound can be completed by a later
    /// declaration, so that answer makes the evaluation impure (not
    /// memoized; spec/reflection.md §3.3); a complete one never changes.
    fn reflect_type_of(
        &mut self,
        r: ReflectionId,
        loc: Loc,
        oven: &mut Oven,
    ) -> Result<ReflectionId, MetaError> {
        let x = self.ast.reflections.get(r);
        match self.type_of_reflectee(x) {
            Some(t) if !self.ast.types.is_dependent(t) => {
                let mut t = self.ast.types.canonical(t);
                if matches!(x, Reflectee::Decl(_))
                    && matches!(self.ast.types.canonical_view(t), TypeView::IncompleteArray(_))
                {
                    oven.mark_impure();
                }
                if self.const_object(x) {
                    t = self.ast.types.qualify(t, Quals::CONST);
                }
                Ok(self.intern_reflection(Reflectee::Type(t), loc)?)
            }
            _ => fail(loc, format!("'{}' has no type", self.reflection_str(r))),
        }
    }

    /// Whether object reflection `x` designates a const object: a
    /// template parameter object ([temp.param]/8; `reflect_constant*`),
    /// or a subobject of a variable declared const. Object reflections are
    /// interned cv-unqualified (one per object), so `type_of` restores
    /// the `const` GCC 16.1 reports from here. An annotation of class
    /// type counts too: its underlying constant is a template parameter
    /// object (`annotation_constant`), so `type_of(a)` is `const T`
    /// exactly as `type_of(constant_of(a))` is.
    pub(in crate::meta) fn const_object(&self, x: Reflectee) -> bool {
        let p = match x {
            Reflectee::Object(p) => p,
            Reflectee::Annotation(a) => {
                let n = self.ast.annots.get(a);
                return n.value.is_some()
                    && self
                        .ast
                        .types
                        .as_record(self.ast.types.canonical(n.written_type))
                        .is_some();
            }
            _ => return false,
        };
        match self.ast.consts.scalar(p) {
            Some(Scalar::Pointer { base: PtrBase::Const(_), .. }) => true,
            Some(Scalar::Pointer { base: PtrBase::Decl(d), .. }) => {
                let vt = self.ast.decls.get(d).ty();
                vt.is_some_and(|t| self.ast.types.effective_quals(t).is_const())
            }
            _ => false,
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
