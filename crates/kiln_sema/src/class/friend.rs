//! Friend declarations ([class.friend], spec/sema-class.md §2.4; WP-066b).

use kiln_ast::DeclId;
use kiln_ast::QualType;
use kiln_ast::decl::FriendTarget;
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::declspec::{DeclSpec, Declarator};

impl Sema<'_, '_> {
    /// [`Sema::act_on_friend`]: the befriended function or type, recorded
    /// in the class being defined (`RecordCold.friends` at `}`).
    pub(crate) fn declare_friend(
        &mut self,
        ds: &DeclSpec,
        base: QualType,
        d: Option<&Declarator>,
        friend_loc: Loc,
    ) -> Option<DeclId> {
        let record = self.current_class()?.record;
        let owned = self.friend_tag.take();
        let Some(d) = d else {
            // `friend T;` / `friend class X;` ([class.friend]/3).
            let loc = ds.type_loc.unwrap_or(friend_loc);
            let target = FriendTarget::Type { ty: base, owned_tag: owned };
            let fd = self.builder().friend(record, loc, target);
            if let Some(c) = self.ast.types.as_record(base) {
                let c = self.ast.first(c);
                self.push_friend(c);
            }
            return Some(fd);
        };
        // Constructor, destructor, conversion and operator names
        // (WP-067a/WP-068b) are still stubs; report them by name.
        let op_d = self.operator_declarator(d);
        let d = op_d.as_ref().unwrap_or(d);
        if self.unsupported_declarator(ds, d) {
            return None;
        }
        let ty = self.type_for_declarator(base, d);
        let Some((name, _)) = d.name else {
            self.error(DiagId::parse_expected_unqualified_id, d.loc);
            return None;
        };
        if !self.ast.types.is_function(ty) {
            if !self.is_error_type(ty) {
                self.error(DiagId::sema_friend_not_class_or_fn, d.name_loc());
            }
            return None;
        }
        let f = self.act_on_function(ds, ty, d, name, true);
        if ds.invalid || d.invalid {
            self.ast.decls.set_flags(f, kiln_ast::decl::DeclFlags::INVALID);
        }
        if let Some(r) = d.trailing_requires {
            self.act_on_trailing_requires(f, r);
        }
        let mut attrs = ds.attrs.clone();
        attrs.extend(d.attrs.iter().copied());
        if !attrs.is_empty() {
            self.check_decl_attrs(f, &attrs);
            let _ = self.builder().attach_attrs(f, &attrs);
        }
        let fd = self.builder().friend(record, d.name_loc(), FriendTarget::Decl(f));
        let first = self.ast.first(f);
        self.push_friend(first);
        Some(fd)
    }

    fn push_friend(&mut self, d: DeclId) {
        if let Some(c) = self.classes.last_mut()
            && !c.friends.contains(&d)
        {
            c.friends.push(d);
        }
    }
}
