//! The query family ([meta.reflection.queries]; spec/reflection.md §5).

use kiln_ast::decl::DeclKind;
use kiln_ast::reflect::Reflectee;
use kiln_ast::{QualType, ReflectionId};
use kiln_oven::{MetaCtx, MetaError, MetaOp, MetaOut, QueryOp, info_arg};
use kiln_source::Loc;

use super::fail;
use crate::Sema;

impl Sema<'_, '_> {
    pub(super) fn meta_query(
        &mut self,
        q: QueryOp,
        args: &[u128],
        out: &mut MetaOut,
        ctx: &MetaCtx,
    ) -> Result<(), MetaError> {
        match q {
            QueryOp::TypeOf => {
                *out = MetaOut::Info(self.reflect_type_of(info_arg(args, 0)?, ctx.loc)?);
                Ok(())
            }
            _ => Err(MetaError::not_implemented(MetaOp::Query(q), ctx.loc)),
        }
    }

    /// `type_of(r)` ([meta.reflection.queries]): the type of a variable,
    /// data member or function; the spike reflects it canonically (the
    /// IS keeps aliases: WP-476).
    fn reflect_type_of(&mut self, r: ReflectionId, loc: Loc) -> Result<ReflectionId, MetaError> {
        let ty = match self.ast.reflections.get(r) {
            Reflectee::Decl(d)
                if matches!(
                    self.kind_of(d),
                    DeclKind::Var
                        | DeclKind::Field
                        | DeclKind::Function
                        | DeclKind::CXXMethod
                        | DeclKind::EnumConstant
                ) =>
            {
                self.ast.decls.get(d).ty()
            }
            _ => None,
        };
        match ty {
            Some(t) if t != QualType::ERROR => {
                let t = self.ast.types.canonical(t);
                Ok(self.intern_reflection(Reflectee::Type(t), loc)?)
            }
            _ => fail(loc, format!("'{}' has no type", self.reflection_str(r))),
        }
    }
}
