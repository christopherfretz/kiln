//! A friend's written form (spec/ast.md §23; WP-3164): the rows Sema
//! writes when it declares a friend, and the parts of its friend
//! matching it reads back from them.

use kiln_ast::decl::{DeclKind, FriendTarget};
use kiln_ast::friend_form::{FriendForm, FriendFormFlags, FriendFormKind};
use kiln_ast::ids::{ListId, TemplateParamListId};
use kiln_ast::names::DeclName;
use kiln_ast::types::{Nns, TypeFlags};
use kiln_ast::{DeclId, NnsId, QualType};
use kiln_source::Loc;

use crate::Sema;
use crate::declspec::Declarator;

impl Sema<'_, '_> {
    /// The row of a friend of kind `kind` written with declarator `d`
    /// naming `name`: its qualifier as written, its template-id's
    /// arguments as written and its trailing requires-clause.
    pub(crate) fn friend_form_of(
        &mut self,
        kind: FriendFormKind,
        d: &Declarator,
        name: DeclName,
    ) -> FriendForm {
        let mut r = FriendForm::new(kind, name, d.name_loc());
        if let Some(n) = d.qualifier_nns {
            r.qualifier = Some(n);
            r.qualifier_loc = if d.id_qual.is_valid() { d.id_qual } else { d.loc };
            if d.global_qualifier || matches!(self.nns_first(n), Nns::Global) {
                r.flags |= FriendFormFlags::GLOBAL;
            }
        }
        if d.template_kw {
            r.flags |= FriendFormFlags::TEMPLATE_KW;
        }
        if let Some(sid) = &d.spec_id {
            r.args = Some(self.ast.types.arg_list(&sid.args));
        }
        r.requires = d.trailing_requires;
        r
    }

    /// Whether nested-name-specifier `n` begins with `::`.
    pub(crate) fn nns_is_global_first(&self, n: NnsId) -> bool {
        matches!(self.nns_first(n), Nns::Global)
    }

    /// The first (outermost) component of nested-name-specifier `n`.
    fn nns_first(&self, n: NnsId) -> Nns {
        let mut cur = n;
        for _ in 0..64 {
            let v = self.ast.types.nns_view(cur);
            let prefix = match v {
                Nns::Identifier { prefix, .. }
                | Nns::Namespace { prefix, .. }
                | Nns::NamespaceAlias { prefix, .. }
                | Nns::Type { prefix, .. } => prefix,
                _ => None,
            };
            match prefix {
                Some(p) => cur = p,
                None => return v,
            }
        }
        self.ast.types.nns_view(cur)
    }

    /// The friend's own template-heads, outermost first, as a row list.
    pub(crate) fn friend_heads_list(&mut self, heads: &[TemplateParamListId]) -> ListId {
        self.ast.lists.push(heads)
    }

    /// Records `row` for `FriendDecl` `fd` (spec/ast.md §23.1): the
    /// friend's `-Wunsupported-friend` flag is added. A friend an
    /// instantiation declares in a class that is not a pattern gets one
    /// only while it is still matched per specialization (a fully
    /// instantiated friend's target is the entity).
    pub(crate) fn record_friend_form(&mut self, fd: DeclId, mut row: FriendForm) {
        if std::mem::take(&mut self.pending_unsupported_friend) {
            row.flags |= FriendFormFlags::UNSUPPORTED;
        }
        // A friend of a class an instantiation made (not itself a
        // pattern) is fully instantiated unless still dependent.
        if !self.inst.stack.is_empty()
            && !row.flags.contains(FriendFormFlags::DEPENDENT)
            && !self.ast.decls.get(fd).has(kiln_ast::decl::DeclFlags::DEPENDENT_CTX)
        {
            return;
        }
        let _ = self.ast.decls.set_friend_form(fd, row);
    }

    /// A copy of the row of pattern friend `pattern` for its instantiation
    /// `fd`, which is still matched per specialization, with the
    /// substituted qualifier `nns` (spec/ast.md §23.1).
    pub(crate) fn record_instantiated_friend_form(
        &mut self,
        pattern: DeclId,
        fd: DeclId,
        nns: Option<NnsId>,
    ) {
        let Some(mut row) = self.ast.decls.friend_form_row(pattern).copied() else { return };
        if nns.is_some() {
            row.qualifier = nns;
        }
        row.flags |= FriendFormFlags::DEPENDENT;
        let _ = self.ast.decls.set_friend_form(fd, row);
    }

    /// The candidate a qualified friend function matched per
    /// specialization names (spec/ast.md §23.3): the function (a
    /// function template when `template`) named `name` in the class or
    /// namespace its non-dependent qualifier names, or in the primary
    /// pattern of the class template a dependent qualifier names a
    /// specialization of; among several, the one whose type is `ty`.
    pub(crate) fn friend_fn_candidate(
        &mut self,
        d: &Declarator,
        name: DeclName,
        ty: QualType,
        template: bool,
    ) -> Option<DeclId> {
        let dependent = d
            .qualifier_nns
            .is_some_and(|n| self.ast.types.nns_flags(n).intersects(TypeFlags::DEPENDENT));
        let found: Vec<DeclId> = if dependent {
            let (tmpl, _, path) = self.member_friend_shape(d.qualifier_nns?)?;
            let mut class = self.ast.definition(self.pattern_of(tmpl)?)?;
            for sym in path {
                let found = self.lookup_in(class, sym, crate::lookup::LookupKind::All).decls;
                let inner = found.into_iter().find(|&x| self.kind_of(x) == DeclKind::CXXRecord)?;
                class = self.ast.definition(inner)?;
            }
            self.ast.lookup_local(class, name).iter().collect()
        } else {
            let q = d.qualifier?;
            self.lookup_qualified_dn(q, name, crate::lookup::LookupKind::Ordinary).decls
        };
        let cands: Vec<DeclId> = found
            .into_iter()
            .map(|x| self.ast.strip_shadow(x))
            .filter(|&x| {
                let k = self.kind_of(x);
                if template { k == DeclKind::FunctionTemplate } else { k.is_function() }
            })
            .collect();
        let pick = match cands.as_slice() {
            [] => return None,
            [one] => *one,
            _ => {
                let c = self.ast.types.canonical(ty);
                let mut same = cands.iter().copied().filter(|&x| {
                    let x = if template { self.pattern_of(x).unwrap_or(x) } else { x };
                    self.ast.decls.get(x).ty().is_some_and(|t| self.ast.types.canonical(t) == c)
                });
                match (same.next(), same.next()) {
                    (Some(x), None) => x,
                    _ => return None,
                }
            }
        };
        Some(self.ast.first(pick))
    }

    /// The `FriendDecl` whose row describes the friend function or
    /// template pattern `f` declared with a qualified name in a templated
    /// class (`Sema::friend_fn_decls`), with its row's qualifier and where
    /// it begins, and whether its match waits for instantiation.
    pub(crate) fn qualified_friend_form(&self, f: DeclId) -> Option<(NnsId, Loc, bool)> {
        let fd = *self.friend_fn_decls.get(&f)?;
        let r = self.ast.decls.friend_form_row(fd)?;
        Some((r.qualifier?, r.qualifier_loc, r.flags.contains(FriendFormFlags::DEPENDENT)))
    }

    /// The context the last component of the non-dependent qualifier `n`
    /// names (a qualified friend's namespace or class).
    pub(crate) fn nns_last_context(&self, n: NnsId) -> Option<DeclId> {
        let t = &self.ast.types;
        match t.nns_view(n) {
            Nns::Global => Some(self.ast.tu),
            Nns::Namespace { ns, .. } | Nns::NamespaceAlias { ns, .. } => Some(ns),
            Nns::Type { ty, .. } => {
                let r = t.as_record(ty).or_else(|| t.as_enum(ty))?;
                Some(self.ast.definition(r).unwrap_or(r))
            }
            _ => None,
        }
    }

    /// The elaborated type `ty` (`struct Y`) of a friend class written
    /// with qualifier `nns` beginning at `at`: the same type with that
    /// qualifier (spec/ast.md §23.2).
    pub fn friend_elaborated_type(&mut self, ty: QualType, nns: NnsId, at: Loc) -> QualType {
        self.pending_friend_class_qual = Some((nns, at));
        let t = &mut self.ast.types;
        if let kiln_ast::types::TypeView::Elaborated { keyword, named, .. } = t.view(ty.ty()) {
            let e = t.elaborated(keyword, Some(nns), named);
            t.qualify(e, ty.quals())
        } else {
            ty
        }
    }

    /// The declared function type of a friend kept with its type only.
    pub(crate) fn friend_fn_ty(&self, fd: DeclId) -> Option<QualType> {
        match self.ast.decls.friend_target(fd) {
            FriendTarget::Type { ty, .. } if self.ast.types.is_function(ty) => Some(ty),
            _ => None,
        }
    }
}
