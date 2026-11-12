//! Dependent using-declarations in class templates (spec/modules.md
//! §11.11, "Forms carried by WP-1345", WP-1345).
//!
//! `using B<T>::name;` and `using typename B<T>::type;` whose qualifier is
//! dependent ([namespace.udecl]/1, [temp.dep.type]) are clang's
//! `UnresolvedUsingValueDecl` and `UnresolvedUsingTypenameDecl`: a member
//! of the pattern that names nothing until each instantiation of the
//! class redoes the declarator with the substituted qualifier. One
//! record, [`Payload::DepUsing`] (kind 29): whether `typename` was
//! written, and the qualifier's steps; the name is the header's (an
//! identifier or an operator). The importer declares it with the
//! builder sema uses (`unresolved_using_value`, registered as a member
//! name; `unresolved_using_typename`, whose type is its own
//! `UnresolvedUsing` type, [`super::deptypes::DepTy::Using`]).
//!
//! An inheriting declarator (`using B<T>::B;`, [class.inhctor.init]) is
//! named by the constructor name of its qualifier's canonical type, not
//! entered in lookup, and marks its class `HAS_INHERITED_CTOR` (kept in
//! the class record's bits): its record's name word is `N_CTOR`, and the
//! importer names it again from the qualifier it rebuilt.
//!
//! Not carried: a pack (`using Ts::f...;`), a conversion function name.

use kiln_ast::decl::DeclKind;
use kiln_ast::names::{DeclName, DeclNameView, OverloadedOperator};
use kiln_ast::types::Nns;
use kiln_ast::{DeclHead, DeclId, QualType};

use super::{DEP, Image, N_CTOR, N_OP, NnsRec, Payload, QRef, W, Writer};
use crate::Sema;

/// The census reason of the forms this module leaves out.
const INHERITING: &str = "an inherited constructor or assignment operator";

impl Writer<'_, '_, '_> {
    /// The payload of dependent using-declarator `d`.
    pub(super) fn dep_using(&mut self, d: DeclId) -> W<Payload> {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        if !self.dep_member(d) {
            return Err("a dependent using-declaration outside a class template");
        }
        let q = ast.using_qualifier(d).ok_or("a malformed using-declaration")?;
        match ast.names.view(n.name()) {
            DeclNameView::Identifier(_) | DeclNameView::Operator(_) => {}
            // An inheriting declarator: the constructor name of the
            // qualifier's canonical type.
            DeclNameView::Constructor(t)
                if n.kind() == DeclKind::UnresolvedUsingValue
                    && matches!(ast.types.nns_view(q), Nns::Type { ty, .. }
                        if ast.types.canonical(ty).ty() == t) => {}
            DeclNameView::Constructor(_) => return Err(INHERITING),
            _ => return Err("a dependent using-declaration not named by an identifier"),
        }
        let typename = n.kind() == DeclKind::UnresolvedUsingTypename;
        Ok(Payload::DepUsing { typename, steps: self.nns_steps(q)? })
    }
}

/// Whether class `d`'s inherited constructors all come from dependent
/// inheriting declarators: a class template pattern or partial
/// specialization with no constructor using-shadow member (the
/// instantiations make those).
pub(super) fn dep_inheriting(ast: &kiln_ast::Ast, d: DeclId) -> bool {
    use kiln_ast::decl::TemplLinkKind as L;
    ast.decls
        .record(d)
        .is_some_and(|r| matches!(r.templ.get(), Some((L::Pattern | L::PartialSpecialization, _))))
        && ast.members(d).all(|m| ast.decls.get(m).kind() != DeclKind::ConstructorUsingShadow)
}

/// Whether dependent using record `d` is well formed: a member of a
/// dependent class, named by an identifier (or an operator, without
/// `typename`).
pub(super) fn dep_using_ok(img: &Image, d: &super::DeclRec) -> bool {
    let Payload::DepUsing { typename, steps } = &d.payload else { return false };
    let class = img
        .decls
        .get(d.ctx as usize)
        .is_some_and(|c| c.flags & DEP != 0 && matches!(c.payload, Payload::Record { .. }));
    let name = match d.name {
        // An inheriting declarator.
        N_CTOR => !typename,
        n if n >= N_OP => {
            !typename
                && n < N_CTOR
                && u8::try_from(n - N_OP).ok().and_then(OverloadedOperator::from_u8).is_some()
        }
        n => (n as usize) < img.names.len(),
    };
    class
        && name
        && d.flags & DEP != 0
        && !steps.is_empty()
        && !matches!(steps.first(), Some(NnsRec::Global) if steps.len() == 1)
}

/// The importer's side.
impl Sema<'_, '_> {
    /// Declares dependent using record `r` (head `h`, qualifier steps).
    pub(super) fn image_dep_using(
        &mut self,
        img: &Image,
        r: &super::DeclRec,
        mut h: DeclHead,
        ids: &[DeclId],
        ty: &mut dyn FnMut(&mut Self, QRef) -> QualType,
    ) -> DeclId {
        let Payload::DepUsing { typename, steps } = &r.payload else { return self.ast.tu };
        if let Some(op) = r
            .name
            .checked_sub(N_OP)
            .and_then(|o| u8::try_from(o).ok())
            .and_then(OverloadedOperator::from_u8)
        {
            h.name = DeclName::operator(op);
        }
        let nns = match self.image_nns(img, steps, ids, ty) {
            Some(n) => n,
            None => self.ast.types.nns(Nns::Global),
        };
        if *typename {
            self.builder().unresolved_using_typename(h, nns)
        } else if r.name == N_CTOR {
            if let Nns::Type { ty, .. } = self.ast.types.nns_view(nns) {
                let canon = self.ast.types.canonical(ty).ty();
                h.name = self.ast.names.constructor(canon);
            }
            self.builder().unresolved_using_value(h, nns, false)
        } else {
            self.builder().unresolved_using_value(h, nns, true)
        }
    }
}
