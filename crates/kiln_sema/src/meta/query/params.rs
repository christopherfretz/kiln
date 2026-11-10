//! P3096's function-parameter queries: `variable_of`, `return_type_of`,
//! `is_explicit_object_parameter`, `has_default_argument` and
//! `has_ellipsis_parameter` (GCC 16.1's `is_vararg_function`). A
//! parameter reflection is `Reflectee::FnParam(function, index)`, which
//! `parameters_of` (WP-478) builds; `^^p` of a parameter name reflects
//! the parameter *variable* (a `Decl` of the `ParmVar`) instead.

use kiln_ast::decl::{DeclKind, ParamData};
use kiln_ast::reflect::Reflectee;
use kiln_ast::{DeclId, ReflectionId};
use kiln_oven::MetaError;
use kiln_source::Loc;

use super::super::fail;
use crate::Sema;

impl Sema<'_, '_> {
    /// `is_explicit_object_parameter`: the parameter is an explicit
    /// object parameter ([dcl.fct]/6).
    pub(super) fn is_explicit_object_parameter(&self, x: Reflectee) -> bool {
        let Reflectee::FnParam(f, i) = x else { return false };
        self.fn_param(f, i)
            .and_then(|p| self.ast.decls.param(p))
            .is_some_and(|p| p.flags & ParamData::EXPLICIT_OBJECT != 0)
    }

    /// `has_default_argument`: some reachable declaration of the function
    /// gives the parameter a default argument ([dcl.fct.default]/4: they
    /// accumulate over redeclarations).
    pub(super) fn has_default_argument(&self, x: Reflectee) -> bool {
        let Reflectee::FnParam(f, i) = x else { return false };
        self.ast.redecls_of(f).any(|d| {
            self.ast
                .decls
                .function(d)
                .and_then(|data| self.ast.lists.iter::<DeclId>(data.params).nth(i as usize))
                .is_some_and(|p| self.param_has_default(p))
        })
    }

    /// `has_ellipsis_parameter` (`is_vararg_function`): a function or
    /// function type whose parameter list ends in `...`.
    pub(super) fn is_vararg_function(&self, x: Reflectee) -> bool {
        let t = match x {
            Reflectee::Type(t) => t,
            _ => match self.fn_decl(x).and_then(|f| self.ast.decls.get(f).ty()) {
                Some(t) => t,
                None => return false,
            },
        };
        let c = self.ast.types.canonical(t);
        self.ast.types.proto(c).is_some_and(kiln_ast::types::FuncProto::variadic)
    }

    /// `return_type_of(r)`: the return type of the function (not a
    /// constructor or destructor) or function type `r` reflects.
    pub(super) fn reflect_return_type_of(
        &mut self,
        r: ReflectionId,
        loc: Loc,
    ) -> Result<ReflectionId, MetaError> {
        let x = self.ast.reflections.get(r);
        let t = match x {
            Reflectee::Type(t) => Some(t),
            _ => self
                .fn_decl(x)
                .filter(|&f| {
                    !matches!(self.kind_of(f), DeclKind::CXXConstructor | DeclKind::CXXDestructor)
                })
                // *has-type* ([meta.reflection.queries]): not a function
                // whose type contains an undeduced placeholder type.
                .filter(|&f| !self.undeduced_decl(f))
                .and_then(|f| self.ast.decls.get(f).ty()),
        };
        let ret = t.and_then(|t| {
            let c = self.ast.types.canonical(t);
            self.ast.types.proto(c).map(kiln_ast::types::FuncProto::ret)
        });
        match ret {
            Some(ret) if !self.ast.types.is_dependent(ret) => {
                let ret = self.ast.types.canonical(ret);
                Ok(self.intern_reflection(Reflectee::Type(ret), loc)?)
            }
            _ => fail(
                loc,
                "reflection does not represent a function or function type with a return type",
            ),
        }
    }

    /// `variable_of(r)` (P3096): for a parameter of the function whose
    /// definition encloses this evaluation, that definition's parameter
    /// variable.
    pub(super) fn reflect_variable_of(
        &mut self,
        r: ReflectionId,
        loc: Loc,
    ) -> Result<ReflectionId, MetaError> {
        let not_param =
            |loc| fail(loc, "reflection does not represent parameter of current function");
        let Reflectee::FnParam(f, i) = self.ast.reflections.get(r) else { return not_param(loc) };
        let Some(cur) = self.enclosing_function(self.ctx) else { return not_param(loc) };
        if !self.same_entity(cur, f) {
            return not_param(loc);
        }
        let p = self
            .ast
            .decls
            .function(cur)
            .and_then(|data| self.ast.lists.iter::<DeclId>(data.params).nth(i as usize));
        match p {
            Some(p) => {
                let key = self.entity_decl(p);
                Ok(self.intern_reflection(Reflectee::Decl(key), loc)?)
            }
            None => not_param(loc),
        }
    }
}
