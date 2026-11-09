//! Friends that are members of every specialization of a class template
//! ([temp.friend]/5; spec/sema-templates.md §4.8, WP-1072):
//! `template<class T> friend class A<T>::Member;` and
//! `template<class T> friend void A<T>::f();`.
//!
//! A member `Member` of a specialization `S` of `A` is a friend when the
//! friend's template parameters deduce from `A<T>` against `S`
//! ([temp.deduct.type]) and, for a function, the substituted type is
//! the member's. clang does not model the rule: it warns
//! (`-Wunsupported-friend`, emitted here at the same place with the same
//! text) and turns off access checking for the befriending class. kiln
//! grants only the members the rule names.

use kiln_ast::decl::{DeclKind, FriendTarget};
use kiln_ast::names::DeclName;
use kiln_ast::types::{Nns, TemplateArg, TypeView};
use kiln_ast::{DeclId, NnsId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::fx::FxHashMap;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::declspec::{DeclSpec, Declarator};
use crate::templ::args::LevelMode;
use crate::templ::deduce::{DeducedArgs, Tdk};
use crate::templ::stack::FrameKind;
use crate::templ::subst::Substituter;

/// One [temp.friend]/5 friend declaration.
#[derive(Clone, Debug)]
pub(crate) struct MemberFriend {
    /// The written nested-name-specifier (`A<T>::`, `A<T>::X::`).
    pub(crate) nns: NnsId,
    /// The friend's own template parameters' depth and count (the
    /// outermost head; parameters retained by substitution).
    depth: u16,
    params: u16,
    /// A member template is named (`template<class T> template<class U>
    /// friend void A<T>::g(U);`): its type is not compared.
    inner_head: bool,
    /// The befriended member's name, and a function's declared type.
    name: DeclName,
    fn_ty: Option<QualType>,
    /// The class template the specifier names a specialization of (first
    /// declaration), that template-id's arguments, and the member class
    /// names after it (`X` of `A<T>::X::Y`), outermost first.
    template: DeclId,
    args: Vec<TemplateArg>,
    path: Vec<Symbol>,
}

/// Whether a member is befriended by a [`MemberFriend`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Grant {
    No,
    Yes,
    /// The member of a class template's pattern or partial
    /// specialization: decided per instantiation (the access check is
    /// replayed, [`Sema::replay_dependent_access`]).
    Speculative,
}

/// The [temp.friend]/5 friends of the translation unit.
#[derive(Default, Debug)]
pub(crate) struct MemberFriends {
    /// Keyed by their `FriendDecl`.
    pub(crate) by_friend: FxHashMap<DeclId, MemberFriend>,
    /// Their `FriendDecl`s, keyed by [`MemberFriend::template`].
    by_template: FxHashMap<DeclId, Vec<DeclId>>,
    /// The answers so far, keyed by `FriendDecl` and member.
    matches: FxHashMap<(DeclId, DeclId), Grant>,
}

impl MemberFriend {
    /// The depth of the friend's own template parameters.
    pub(crate) fn depth(&self) -> u16 {
        self.depth
    }
}

impl Sema<'_, '_> {
    /// The class template `nns` names a specialization of, that
    /// template-id's arguments, and the member class names written after
    /// it, outermost first: `A`, `[T]`, `[X]` for `A<T>::X::`.
    fn member_friend_shape(&self, nns: NnsId) -> Option<(DeclId, Vec<TemplateArg>, Vec<Symbol>)> {
        let t = &self.ast.types;
        let mut path = Vec::new();
        let mut cur = nns;
        for _ in 0..64 {
            let mut q = match t.nns_view(cur) {
                Nns::Identifier { prefix, name } => {
                    path.push(name);
                    cur = prefix?;
                    continue;
                }
                // `A<T>::X::` names the member `X` of the pattern.
                Nns::Type { prefix: Some(p), ty, .. }
                    if let TypeView::Record(r) = t.view(ty.ty())
                        && let Some(sym) = self.ast.decls.get(r).name().as_ident() =>
                {
                    path.push(sym);
                    cur = p;
                    continue;
                }
                Nns::Type { ty, .. } => ty,
                _ => return None,
            };
            let mut next = None;
            for _ in 0..16 {
                match t.view(q.ty()) {
                    TypeView::TemplateSpecialization { name, args, alias: false, .. } => {
                        let tmpl = t.template_decl(name)?;
                        if self.kind_of(tmpl) != DeclKind::ClassTemplate {
                            return None;
                        }
                        path.reverse();
                        return Some((self.ast.first(tmpl), t.args(args).to_vec(), path));
                    }
                    TypeView::DependentName { nns, name, .. } => {
                        path.push(name);
                        next = Some(nns);
                        break;
                    }
                    _ => {
                        let n = t.desugar_once(q);
                        if n == q {
                            return None;
                        }
                        q = n;
                    }
                }
            }
            cur = next?;
        }
        None
    }

    /// The friend's own head (its depth and parameter count) and whether
    /// a member template's head follows it, when the template-id's
    /// arguments `args` name the head's parameters: the declaration is a
    /// [temp.friend]/5 friend.
    fn member_friend_head(&self, args: &[TemplateArg]) -> Option<(u16, u16, bool)> {
        let heads = self.open_written_heads();
        let &(depth, n) = heads.first()?;
        let mut used = vec![false; usize::from(n)];
        for &a in args {
            self.mark_used_arg(a, u32::from(depth), &mut used, 0);
        }
        used.contains(&true).then_some((depth, n, heads.len() > 1))
    }

    /// `template<class T> friend class A<T>::Member;` in class `record`
    /// (`base` the dependent elaborated type): `None` when the
    /// declaration is not of that form.
    pub(crate) fn declare_member_friend_class(
        &mut self,
        ds: &DeclSpec,
        base: QualType,
        record: DeclId,
        loc: Loc,
    ) -> Option<DeclId> {
        if !self.has_pending_head() {
            return None;
        }
        let TypeView::DependentName { nns, name, .. } = self.ast.types.view(base.ty()) else {
            return None;
        };
        let (template, args, path) = self.member_friend_shape(nns)?;
        let (depth, params, inner_head) = self.member_friend_head(&args)?;
        self.take_open_heads();
        let fd =
            self.builder().friend(record, loc, FriendTarget::Type { ty: base, owned_tag: None });
        let name = DeclName::ident(name);
        let mf = MemberFriend {
            nns,
            depth,
            params,
            inner_head,
            name,
            fn_ty: None,
            template,
            args,
            path,
        };
        self.warn_unsupported_friend(nns, record, ds.dependent_tag_name.unwrap_or(loc));
        self.register_member_friend(fd, mf);
        Some(fd)
    }

    /// `template<class T> friend void A<T>::f();` in class `record`:
    /// `None` when the declaration is not of that form.
    pub(crate) fn declare_member_friend_fn(
        &mut self,
        ds: &DeclSpec,
        base: QualType,
        d: &Declarator,
        record: DeclId,
    ) -> Option<DeclId> {
        let nns = d.qualifier_nns?;
        let (name, _) = d.name?;
        // A definition (ill-formed, [class.friend]/6) and the special
        // names take the ordinary path.
        if d.special.is_some() || self.templ.fn_def_next {
            return None;
        }
        let (template, args, path) = self.member_friend_shape(nns)?;
        let (depth, params, inner_head) = self.member_friend_head(&args)?;
        self.take_open_heads();
        let ty = self.type_for_declarator_at(base, ds.type_loc.unwrap_or(ds.loc), d);
        self.templ.cur = None;
        if !self.ast.types.is_function(ty) {
            return None;
        }
        let at = d.name_loc();
        let fd = self.builder().friend(record, at, FriendTarget::Type { ty, owned_tag: None });
        let name = d.fn_decl_name(name);
        let fn_ty = Some(ty);
        let mf = MemberFriend { nns, depth, params, inner_head, name, fn_ty, template, args, path };
        self.warn_unsupported_friend(nns, record, at);
        self.register_member_friend(fd, mf);
        Some(fd)
    }

    /// The friend `pattern` of a class template, instantiated in the
    /// class being defined (`record`) with the substituted specifier
    /// `nns`, target type `ty` and the friend's parameters' new `depth`.
    pub(crate) fn instantiate_member_friend(
        &mut self,
        pattern: &MemberFriend,
        nns: NnsId,
        ty: QualType,
        depth: u16,
        loc: Loc,
    ) -> Option<DeclId> {
        let record = self.current_class()?.record;
        let (template, args, path) = self.member_friend_shape(nns)?;
        let fd = self.builder().friend(record, loc, FriendTarget::Type { ty, owned_tag: None });
        let fn_ty = pattern.fn_ty.map(|_| ty);
        let mf = MemberFriend { nns, depth, fn_ty, template, args, path, ..pattern.clone() };
        self.register_member_friend(fd, mf);
        Some(fd)
    }

    fn register_member_friend(&mut self, fd: DeclId, mf: MemberFriend) {
        let m = &mut self.member_friends;
        m.by_template.entry(mf.template).or_default().push(fd);
        m.by_friend.insert(fd, mf);
        self.push_friend(fd);
    }

    /// clang's `warn_template_qualified_friend_unsupported`, at the
    /// befriended name: clang 22 prints the specifier without its
    /// trailing `::`.
    fn warn_unsupported_friend(&mut self, nns: NnsId, record: DeclId, at: Loc) {
        let mut spec = self.nns_str(nns);
        if self.clang_compat >= 22 && spec.ends_with("::") {
            spec.truncate(spec.len() - 2);
        }
        let class = self.decl_name(record);
        let e = self.diag(DiagId::sema_unsupported_friend, at).arg(spec).arg(class);
        self.emit(e);
    }

    /// The [temp.friend]/5 friends (`FriendDecl`s) that befriend one of
    /// `members` (first declarations of classes and functions of an
    /// effective context), and whether each only might
    /// ([`Grant::Speculative`]).
    pub(crate) fn member_friend_grants(&mut self, members: &[DeclId]) -> Vec<(DeclId, bool)> {
        let mut out = Vec::new();
        if self.member_friends.by_template.is_empty() {
            return out;
        }
        for &e in members {
            self.member_friend_grants_for(e, &mut out);
        }
        out
    }

    fn member_friend_grants_for(&mut self, e: DeclId, out: &mut Vec<(DeclId, bool)>) {
        let mut path: Vec<Symbol> = Vec::new();
        let mut cur = e;
        for _ in 0..16 {
            let Some(p) = self.ast.decls.get(cur).ctx() else { return };
            if !matches!(
                self.kind_of(p),
                DeclKind::CXXRecord
                    | DeclKind::ClassTemplateSpecialization
                    | DeclKind::ClassTemplatePartialSpecialization
            ) {
                return;
            }
            let fds = self
                .friend_template_of(p)
                .and_then(|t| self.member_friends.by_template.get(&t))
                .cloned()
                .unwrap_or_default();
            for fd in fds {
                match self.member_friend_match(fd, e, p, &path) {
                    Grant::Yes => out.push((fd, false)),
                    Grant::Speculative => out.push((fd, true)),
                    Grant::No => {}
                }
            }
            let Some(sym) = self.ast.decls.get(p).name().as_ident() else { return };
            path.push(sym);
            cur = p;
        }
    }

    /// Whether friend `fd` befriends `e`, a member of class `s` (through
    /// the member classes `path`, innermost first).
    fn member_friend_match(&mut self, fd: DeclId, e: DeclId, s: DeclId, path: &[Symbol]) -> Grant {
        if let Some(&g) = self.member_friends.matches.get(&(fd, e)) {
            return g;
        }
        let g = self.member_friend_match_uncached(fd, e, s, path);
        self.member_friends.matches.insert((fd, e), g);
        g
    }

    fn member_friend_match_uncached(
        &mut self,
        fd: DeclId,
        e: DeclId,
        s: DeclId,
        path: &[Symbol],
    ) -> Grant {
        let Some(mf) = self.member_friends.by_friend.get(&fd).cloned() else { return Grant::No };
        let n = self.ast.decls.get(e);
        if mf.fn_ty.is_some() != n.kind().is_function()
            || n.name() != mf.name
            || !mf.path.iter().rev().eq(path.iter())
        {
            return Grant::No;
        }
        let e_ty = n.ty();
        // A member of the pattern or of a partial specialization: some
        // instantiation may be befriended.
        let Some((_, si)) = self.ast.spec_info(s) else { return Grant::Speculative };
        let as_ = self.ast.types.args(self.ast.types.canonical_args(si.args)).to_vec();
        let loc = self.ast.decls.get(fd).loc();
        let r = self.in_sfinae(FrameKind::DeducedArgSubstitution, mf.template, None, loc, |s| {
            let mut d = DeducedArgs::new(mf.depth, usize::from(mf.params));
            if s.deduce_args(&mf.args, &as_, false, &mut d, loc) != Tdk::Success
                || d.first_undeduced().is_some()
            {
                return false;
            }
            let Some(fty) = mf.fn_ty.filter(|_| !mf.inner_head) else { return true };
            let deduced = s.ast.types.arg_list(&d.args);
            let mut levels = Self::outer_levels(mf.template, mf.depth);
            levels.push(deduced, mf.template, LevelMode::Substitute);
            let Ok(t) = Substituter::new(s, &levels).ty(fty, loc) else { return false };
            e_ty.is_some_and(|et| s.ast.types.canonical(et) == s.ast.types.canonical(t))
        });
        match r {
            Some((true, Ok(()))) => Grant::Yes,
            _ => Grant::No,
        }
    }
}
