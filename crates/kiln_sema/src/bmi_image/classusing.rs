//! Non-dependent using-declarations of base members in a class
//! (spec/modules.md §11.11, "Forms carried by WP-1353").
//!
//! `using Base::f;` in a class that is not dependent
//! ([namespace.udecl]/3, `using _RangeAdaptor<_Drop>::operator();` in
//! libstdc++'s `<ranges>`) is a `Using` member record (its qualifier's
//! steps), followed by one `Shadow` member record per member it
//! introduces, in the class's member order, as at namespace scope. A
//! shadow's target is the base member's record: an ordinary member
//! record of a carried class, or a member reference record (`Member`)
//! of a class template specialization, written just before the shadow.
//! The importer declares them with the builders sema uses
//! (`using_decl`, `using_shadow`), with the record's access.
//!
//! Not carried: an inheriting declarator (`using B::B;`, its
//! `ConstructorUsingShadow`s), a using-declaration in a dependent class
//! (`depusing.rs` has the dependent qualifier's).

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclFlags, DeclKind};
use kiln_ast::names::DeclNameView;

use super::{DEP, DeclRec, Image, Payload, W, Writer};

/// The census reason of the forms this module leaves out.
pub(super) const CLASS_USING: &str = "a using-declaration in a class";

impl Writer<'_, '_, '_> {
    /// Whether class-scope using-declaration (or shadow) `d` is one this
    /// module carries: not dependent, not inheriting constructors.
    pub(super) fn class_using_ok(&self, d: DeclId) -> bool {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        !n.has(DeclFlags::DEPENDENT_CTX)
            && matches!(n.kind(), DeclKind::Using | DeclKind::UsingShadow)
            && !matches!(ast.names.view(n.name()), DeclNameView::Constructor(_))
    }

    /// Writes the member reference record class-scope shadow `d`'s
    /// target needs before `d`'s own (errors are left for the record).
    pub(super) fn prescan_class_shadow(&mut self, d: DeclId) {
        let ast = &self.s.ast;
        let n = ast.decls.get(d);
        let in_class = n.ctx().is_some_and(|c| ast.decls.get(c).kind() == DeclKind::CXXRecord);
        if in_class && n.kind() == DeclKind::UsingShadow && self.class_using_ok(d) {
            let _ = self.class_shadow_target(d);
        }
    }

    /// The record of class-scope shadow `d`'s target: its own, or a
    /// member reference record of a class specialization's member.
    pub(super) fn class_shadow_target(&mut self, d: DeclId) -> W<Option<u32>> {
        let Some(t) = self.s.ast.using_target(d) else { return Ok(None) };
        if let Some(&r) = self.recs.get(&t) {
            return Ok(Some(r));
        }
        if self.spec_member(t) {
            return self.member_rec(t);
        }
        Ok(None)
    }
}

/// Whether member record `d` (record `i`) of class record `class` is a
/// non-dependent class-scope using-declaration or shadow this module
/// carries.
pub(super) fn class_using_rec_ok(img: &Image, class: &DeclRec, d: &DeclRec, i: usize) -> bool {
    if class.flags & DEP != 0 || d.flags & DEP != 0 {
        return false;
    }
    match &d.payload {
        Payload::Using { steps } => !steps.is_empty() && super::steps_ok(img, steps, i),
        Payload::Shadow { target } => {
            (*target as usize) < i
                && img.decls.get(*target as usize).is_some_and(|t| {
                    t.flags & DEP == 0
                        && match &t.payload {
                            Payload::Member { .. } => true,
                            // A constructor or destructor is never named
                            // so (an inheriting declarator is refused).
                            Payload::Function { kind, .. } => matches!(kind, 1 | 4),
                            Payload::Field { .. }
                            | Payload::Var { .. }
                            | Payload::Typedef { .. }
                            | Payload::Record { .. }
                            | Payload::Enum { .. }
                            | Payload::Enumerator { .. }
                            | Payload::Template { .. }
                            | Payload::Shadow { .. } => t.name == d.name,
                            _ => false,
                        }
                })
        }
        _ => false,
    }
}
