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
            // `template<class U> friend class X;` ([temp.friend]/1): the
            // befriended class template (declared, hidden, by
            // `act_on_friend_tag`).
            if let Some(t) = self.friend_class_template.take() {
                let loc = ds.type_loc.unwrap_or(friend_loc);
                let fd = self.builder().friend(record, loc, FriendTarget::Decl(t));
                let first = self.ast.first(t);
                self.push_friend(first);
                return Some(fd);
            }
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
        // `template<class U> friend void f(U);` ([temp.friend]/1): a
        // friend function template.
        let head = self.take_template_head(d.qualifier);
        let op_d = self.operator_declarator(d);
        let d = op_d.as_ref().unwrap_or(d);
        if self.unsupported_declarator(ds, d) {
            self.templ.cur = None;
            return None;
        }
        let ty = self.type_for_declarator(base, d);
        let Some((name, _)) = d.name else {
            self.templ.cur = None;
            self.error(DiagId::parse_expected_unqualified_id, d.loc);
            return None;
        };
        if !self.ast.types.is_function(ty) {
            self.templ.cur = None;
            if !self.is_error_type(ty) {
                self.error(DiagId::sema_friend_not_class_or_fn, d.name_loc());
            }
            return None;
        }
        if head.is_some() {
            self.mark_pattern_next();
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
        let target = self.finish_template_pattern(f).unwrap_or(f);
        let fd = self.builder().friend(record, d.name_loc(), FriendTarget::Decl(target));
        let first = self.ast.first(target);
        self.push_friend(first);
        Some(fd)
    }

    pub(crate) fn push_friend(&mut self, d: DeclId) {
        if let Some(c) = self.classes.last_mut()
            && !c.friends.contains(&d)
        {
            c.friends.push(d);
        }
    }
}
