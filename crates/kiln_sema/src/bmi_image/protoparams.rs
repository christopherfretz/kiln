//! Parameters before their function's type (spec/modules.md §11.11,
//! "Forms carried by WP-1343").
//!
//! A function's type may name its own parameters: a trailing return type
//! (`auto f(T x) -> decltype(x + 1)`), a `noexcept` operand, a later
//! parameter's type (`decltype(a) b`), all in the parameter's scope
//! ([basic.scope.param]). The parser declares each parameter as the
//! declarator reaches it, in the declarator's context, and the function
//! adopts them once it exists. The importer does the same: it builds
//! every parameter of a function record before the record's type, and
//! the expression sections of the types ([`super::exprs`]) may then name
//! them.

use kiln_ast::names::DeclName;
use kiln_ast::{DeclId, QualType};

use super::{Image, Locs, NONE, ParamRec};
use crate::Sema;

impl Sema<'_, '_> {
    /// Declares parameters `params` of a function record in context `ctx`
    /// (the function adopts them, `set_param_owner`), each after its
    /// type; they join the parameters the expression sections may name.
    pub(super) fn image_prototype_params(
        &mut self,
        img: &Image,
        tys: &mut Vec<QualType>,
        ids: &[DeclId],
        ctx: DeclId,
        params: &[ParamRec],
        main: &Locs,
    ) -> Vec<DeclId> {
        let mut ps = Vec::with_capacity(params.len());
        for (k, p) in params.iter().enumerate() {
            let pty = self.image_ty(img, tys, ids, p.ty);
            let name = match img.pnames.get(p.name as usize) {
                Some(n) if p.name != NONE => DeclName::ident(self.intern(n)),
                _ => DeclName::EMPTY,
            };
            let index = u16::try_from(k).unwrap_or(u16::MAX);
            let pd =
                self.builder().prototype_param(ctx, name, main.get(p.loc), pty, index, p.pflags);
            self.image_texprs.params.push(pd);
            ps.push(pd);
        }
        ps
    }
}
