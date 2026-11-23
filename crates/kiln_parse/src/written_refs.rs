//! The parser side of the written-reference table (spec/ast.md §18,
//! WP-1930): an id-expression or member access notes the span of the
//! name it consumed ([`Parser::note_wname`]) and records it once its
//! expression is built ([`Parser::with_written_name`]). Callers test
//! [`kiln_sema::Sema::records_written_refs`] first, so recording off
//! costs one branch per site.

use kiln_ast::decl::{DeclKind, FriendTarget};
use kiln_ast::written_refs::{RefMark, RefQualifier, RefRole, WrittenRefId};
use kiln_ast::{DeclId, QualType};
use kiln_sema::declspec::{Declarator, SpecialName};
use kiln_source::{Loc, Span};

use crate::parser::{PExpr, Parser};

impl Parser<'_, '_> {
    /// The name being parsed began at `lo` and ended at the last token
    /// consumed, after qualifier `q`.
    #[inline]
    pub(crate) fn note_wname(&mut self, lo: Loc, q: RefQualifier) {
        if self.s.records_written_refs() {
            self.wname = Some((Span::new(lo, self.prev_end()), q, Loc::INVALID));
        }
    }

    /// [`note_wname`](Self::note_wname) for a destructor name `~T`
    /// whose `~` is at `tilde` and class name at `lo` (spec/ast.md §18.11.2).
    pub(crate) fn note_wname_dtor(&mut self, tilde: Loc, lo: Loc, q: RefQualifier) {
        if self.s.records_written_refs() {
            self.wname = Some((Span::new(lo, self.prev_end()), q, tilde));
        }
    }

    /// Runs `f` (an id-expression or member access parse) and records the
    /// name it noted with the expression it built; the site of an
    /// enclosing parse is kept across it.
    pub(crate) fn with_written_name(
        &mut self,
        role: RefRole,
        f: impl FnOnce(&mut Self) -> PExpr,
    ) -> PExpr {
        let outer = self.wname.take();
        let outer_role = self.wname_role.take();
        let r = f(self);
        let member = role == RefRole::Member;
        let role = self.wname_role.take().unwrap_or(role);
        if let Some((range, q, tilde)) = self.wname.take()
            && let Some(id) = self.s.note_written_name(r, range, role, q, member)
            && tilde.is_valid()
        {
            self.s.ast.written_refs.set_tilde(id, tilde);
        }
        self.wname = outer;
        self.wname_role = outer_role;
        r
    }

    /// Records the type or template name written at `name` (one token,
    /// or the tokens of the name) with qualifier `q`, naming `ty` (the
    /// template `tmpl` for a template name; spec/ast.md §18.4). Callers
    /// test [`kiln_sema::Sema::records_written_refs`] first.
    pub(crate) fn note_type_name(
        &mut self,
        name: Span,
        q: RefQualifier,
        ty: Option<QualType>,
        tmpl: Option<DeclId>,
        role: RefRole,
    ) {
        if let Some(id) = self.s.note_written_type(name, ty, tmpl, role, q) {
            self.last_tname = Some(id);
        }
    }

    /// Records the unqualified type name at `name` (role `type`).
    pub(crate) fn note_unqualified_type_name(&mut self, name: Span, ty: QualType) {
        if self.s.records_written_refs() {
            self.note_type_name(name, RefQualifier::NONE, Some(ty), None, RefRole::Type);
        }
    }

    /// The outermost type name recorded since `m` becomes the name of a
    /// base-specifier or mem-initializer-id (`role`).
    pub(crate) fn rerole_type_name(&mut self, m: Option<RefMark>, role: RefRole) -> bool {
        if let (Some(m), Some(id)) = (m, self.last_tname)
            && id.index() >= m.entries as usize
            && self.s.records_written_refs()
        {
            self.s.ast.written_refs.set_role(id, role);
            return true;
        }
        false
    }

    /// The one token starting at `lo`, as a span.
    pub(crate) fn token_span_at(&self, lo: Loc) -> Span {
        Span::new(lo, self.s.token_end_of(lo).unwrap_or(lo))
    }

    /// Records the declarator-id of declarator `d`, which declared `decl`
    /// (stage 3, spec/ast.md §18.4), when it names something declared
    /// elsewhere: a deduction guide's template name (`template`), the
    /// template of an explicit specialization or instantiation, or the
    /// member of a class template specialization one instantiates
    /// (`spec-target`), the earlier declaration a friend function
    /// redeclares (`elaborated`) or a qualified declarator-id redeclares
    /// (`redecl`). A first declaration's own name is not recorded. Callers
    /// run it before the declaration claims its names.
    pub(crate) fn note_declarator_ref(&mut self, decl: Option<DeclId>, d: &Declarator) {
        if !self.s.records_written_refs() {
            return;
        }
        if d.id_recorded {
            // A template-id's template name, recorded with lookup's first
            // template: an explicit specialization or instantiation names
            // the one it matched (a friend's is patched by sema, which
            // matches it).
            let Some(id) = d.id_ref.filter(|_| !d.friend) else { return };
            let t = self.s.ast.written_refs.take_inst_target(d.name_loc()).or_else(|| {
                let x = decl?;
                self.s.ast.spec_info(x).map(|(_, i)| i.template)
            });
            if let Some(t) = t {
                self.s.note_written_spec_template(id, t);
            }
            return;
        }
        let range = match d.special {
            Some((SpecialName::DeductionGuide(t), l)) => {
                let r = self.token_span_at(l);
                self.s.note_written_decl_name(
                    r,
                    Some(t),
                    RefRole::Template,
                    RefQualifier::NONE,
                    false,
                );
                return;
            }
            Some((SpecialName::Constructor(_), l)) => self.token_span_at(l),
            Some((SpecialName::Destructor(_), _)) if d.dtor_name.is_valid() => {
                self.token_span_at(d.dtor_name)
            }
            Some(_) if d.id_span.lo.is_valid() => d.id_span,
            Some(_) => return,
            None => match d.name {
                Some((_, l)) => self.token_span_at(l),
                None => return,
            },
        };
        // The `~` of a destructor's declarator-id (§18.11.2).
        let tilde = match d.special {
            Some((SpecialName::Destructor(_), l)) if d.dtor_name.is_valid() => Some(l),
            _ => None,
        };
        let qualified = d.id_link != RefQualifier::NONE;
        let friend = d.friend;
        // An explicit instantiation declares nothing; sema says what it
        // named ([temp.explicit]/3).
        if let Some(t) = self.s.ast.written_refs.take_inst_target(d.name_loc()) {
            let id = self.s.note_written_decl_name(
                range,
                Some(t),
                RefRole::SpecTarget,
                d.id_link,
                false,
            );
            self.note_decl_tilde(id, tilde);
            return;
        }
        let Some(decl) = decl else {
            // A qualified declarator-id that named nothing (diagnosed).
            if qualified && !d.dropped {
                let dep = d.qualifier.is_some_and(|q| self.s.is_dependent_ctx(q));
                let id =
                    self.s.note_written_decl_name(range, None, RefRole::Redecl, d.id_link, dep);
                self.note_decl_tilde(id, tilde);
            }
            return;
        };
        let ast = &self.s.ast;
        let target = if ast.decls.get(decl).kind() == DeclKind::Friend {
            let Some(t) = self.friend_declarator_target(decl, d, range, tilde) else { return };
            t
        } else {
            decl
        };
        let ast = &self.s.ast;
        let spec = ast.spec_info(target).map(|(_, i)| i.template);
        let first = ast.first(target);
        let (entity, role) = if let Some(t) = spec {
            (Some(t), RefRole::SpecTarget)
        } else if qualified {
            let role = if friend { RefRole::Elaborated } else { RefRole::Redecl };
            (Some(first).filter(|&f| f != target), role)
        } else if friend && first != target {
            (Some(first), RefRole::Elaborated)
        } else {
            return;
        };
        let dep = entity.is_none() && d.qualifier.is_some_and(|q| self.s.is_dependent_ctx(q));
        let id = self.s.note_written_decl_name(range, entity, role, d.id_link, dep);
        self.note_decl_tilde(id, tilde);
    }

    /// The declaration the declarator-id of friend `decl` (declarator
    /// `d`, written at `range`) redeclares, for `note_declarator_ref`;
    /// `None` when its entry is recorded here (a friend matched per
    /// specialization or kept with its type only) or there is none.
    fn friend_declarator_target(
        &mut self,
        decl: DeclId,
        d: &Declarator,
        range: Span,
        tilde: Option<Loc>,
    ) -> Option<DeclId> {
        let qualified = d.id_link != RefQualifier::NONE;
        let ast = &self.s.ast;
        // A friend matched per specialization names its row's candidate
        // (spec/ast.md §23.3): one source of truth.
        let (entity, dep) = if let Some(r) = ast.decls.friend_form_row(decl)
            && r.flags.contains(kiln_ast::friend_form::FriendFormFlags::DEPENDENT)
            && qualified
        {
            (r.entity, true)
        } else {
            match ast.decls.friend_target(decl) {
                FriendTarget::Decl(t) => return Some(t),
                // A qualified friend kept with its type only: a member of
                // every specialization of a class template
                // ([temp.friend]/5) or of a dependent class.
                FriendTarget::Type { .. } if qualified => {
                    self.s.written_friend_type_target(decl, d)
                }
                FriendTarget::Type { .. } => return None,
            }
        };
        let id = self.s.note_written_decl_name(range, entity, RefRole::Elaborated, d.id_link, dep);
        self.note_decl_tilde(id, tilde);
        None
    }

    fn note_decl_tilde(&mut self, id: Option<WrittenRefId>, tilde: Option<Loc>) {
        if let (Some(id), Some(t)) = (id, tilde) {
            self.s.ast.written_refs.set_tilde(id, t);
        }
    }
}
