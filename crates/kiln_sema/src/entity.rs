//! Entity identity (spec/sema.md §4 "Entity identity"; WP-471).
//!
//! Two declarations denote the same entity when they are in one
//! redeclaration chain ([basic.link], [basic.def]/1): a redeclaration, an
//! out-of-line definition of a member, and a friend declaration of a
//! function all name the entity their first declaration introduced. An
//! instantiated specialization is a distinct entity from the template it
//! was instantiated from ([temp.spec.general]): its own chain. Code that
//! asks "is this the same entity" (reflection equality above all) uses
//! [`Sema::same_entity`], never `DeclId` `==` (an `xtask` lint enforces
//! it for the reflection code).

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclKind, FriendTarget};

use crate::Sema;

impl Sema<'_, '_> {
    /// The declaration that stands for `d`'s entity: the befriended
    /// function of a friend declaration (`friend void f();` declares
    /// `f`), then the first declaration of its redeclaration chain (a
    /// template's follows its pattern's, [`kiln_ast::Ast::first`]).
    pub fn entity_decl(&self, d: DeclId) -> DeclId {
        let d = if self.ast.decls.get(d).kind() == DeclKind::Friend {
            match self.ast.decls.friend_target(d) {
                FriendTarget::Decl(f) => f,
                FriendTarget::Type { .. } => d,
            }
        } else {
            d
        };
        self.ast.first(d)
    }

    /// Whether `a` and `b` declare the same entity ([basic.link]): one
    /// redeclaration chain, friend declarations looked through.
    pub fn same_entity(&self, a: DeclId, b: DeclId) -> bool {
        a == b || self.entity_decl(a) == self.entity_decl(b)
    }
}
