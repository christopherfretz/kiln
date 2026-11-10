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

use kiln_ast::decl::{DeclFlags, DeclKind, FriendTarget};
use kiln_ast::ids::{ArgListId, TemplateParamListId};
use kiln_ast::names::DeclName;
use kiln_ast::types::{Nns, TemplateArg, TypeFlags, TypeView};
use kiln_ast::{DeclId, ExprId, NnsId, QualType};
use kiln_diag::DiagId;
use kiln_source::Loc;
use kiln_source::fx::FxHashMap;
use kiln_source::symbol::Symbol;

use crate::Sema;
use crate::declspec::{DeclSpec, Declarator};
use crate::templ::args::{LevelMode, MultiLevelArgs};
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
    /// friend void A<T>::g(U);`), with this head: matched against the
    /// member template's head and type ([temp.over.link]; WP-1098).
    inner_head: bool,
    inner_list: Option<TemplateParamListId>,
    /// The befriended member's name, a function's declared type and its
    /// trailing requires-clause.
    name: DeclName,
    fn_ty: Option<QualType>,
    requires: Option<ExprId>,
    /// A function's parameter declarations, which `requires` names:
    /// mapped to the member's when the two trailing requires-clauses are
    /// compared ([temp.over.link]/5, WP-1146).
    fn_params: Vec<DeclId>,
    /// For a friend instantiated with its enclosing class template, that
    /// instantiation's arguments: the levels outside the friend's own
    /// head in `inner_list` and `requires`, which keep the pattern's
    /// parameters (WP-1130).
    enclosing: Option<MultiLevelArgs>,
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

    /// `template<class U> friend int B<T>::g(U);` in a class template:
    /// a dependent qualifier that names none of the innermost open head's
    /// parameters leaves that head to the befriended member template
    /// itself ([temp.friend]/1), not to the qualifier's class template
    /// (WP-1110). So does the invented head of an abbreviated friend
    /// with no written head (`friend int B<T>::g(auto);`, [dcl.fct]/22):
    /// no head is written for the qualifier's class template (WP-1145).
    pub(crate) fn friend_head_is_own(&self, d: &Declarator) -> bool {
        let Some(nns) = d.qualifier_nns else { return false };
        if !self.ast.types.nns_flags(nns).intersects(TypeFlags::DEPENDENT) {
            return false;
        }
        if self.open_written_heads().is_empty() {
            return self.open_invented_head_list().is_some();
        }
        self.member_friend_shape(nns)
            .is_some_and(|(_, args, _)| self.member_friend_head(&args).is_none())
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
        let inner_list = self.innermost_open_head_list().filter(|_| inner_head);
        self.take_open_heads();
        let fd =
            self.builder().friend(record, loc, FriendTarget::Type { ty: base, owned_tag: None });
        let name = DeclName::ident(name);
        let at = ds.dependent_tag_name.unwrap_or(loc);
        if path.is_empty() && self.member_friend_kind_mismatch(template, name, false, at) {
            self.ast.decls.set_flags(fd, DeclFlags::INVALID);
            return Some(fd);
        }
        let mf = MemberFriend {
            nns,
            depth,
            params,
            inner_head,
            inner_list,
            name,
            fn_ty: None,
            requires: None,
            fn_params: Vec::new(),
            enclosing: None,
            template,
            args,
            path,
        };
        self.warn_unsupported_friend(nns, record, at);
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
        let (depth, params, written) = self.member_friend_head(&args)?;
        // `template<class T> friend void A<T>::f(C auto);`: the invented
        // head of an abbreviated member template is the inner one.
        let invented = self.open_invented_head_list();
        let inner_head = written || invented.is_some();
        let inner_list = invented.or_else(|| self.innermost_open_head_list().filter(|_| written));
        let head_loc = self.outermost_open_head_loc();
        self.take_open_heads();
        let ty = self.type_for_declarator_at(base, ds.type_loc.unwrap_or(ds.loc), d);
        self.templ.cur = None;
        if !self.ast.types.is_function(ty) {
            return None;
        }
        let at = d.name_loc();
        let fd = self.builder().friend(record, at, FriendTarget::Type { ty, owned_tag: None });
        let name = d.fn_decl_name(name);
        if path.is_empty() && self.member_friend_kind_mismatch(template, name, true, at) {
            self.ast.decls.set_flags(fd, DeclFlags::INVALID);
            return Some(fd);
        }
        if !inner_head && self.non_template_member_friend_requires(d, depth, head_loc) {
            // clang drops the declaration in a templated class before it
            // warns.
            if depth == 0 {
                self.warn_unsupported_friend(nns, record, at);
            }
            self.ast.decls.set_flags(fd, DeclFlags::INVALID);
            return Some(fd);
        }
        let fn_ty = Some(ty);
        let fn_params =
            d.function_params().unwrap_or_default().iter().filter_map(|p| p.decl).collect();
        let mf = MemberFriend {
            nns,
            depth,
            params,
            inner_head,
            inner_list,
            name,
            fn_ty,
            requires: d.trailing_requires,
            fn_params,
            enclosing: None,
            template,
            args,
            path,
        };
        self.warn_unsupported_friend(nns, record, at);
        self.register_member_friend(fd, mf);
        Some(fd)
    }

    /// [temp.friend]/5, [dcl.type.elab]/5, [class.friend]/10: the member
    /// a friend of every specialization of `template` names is a class
    /// (`function` false) or a function. When the template's primary
    /// pattern is defined and declares `name` as an entity of another
    /// kind only (a function or variable for a class friend; a class,
    /// variable or enumerator for a function friend), the friend is
    /// ill-formed: diagnosed at `at` as clang does for a member of the
    /// current instantiation, "redefinition of 'Member' as different
    /// kind of symbol" (g++ also rejects; clang accepts it with
    /// `-Wunsupported-friend`). Explicit and partial specializations are
    /// not consulted (WP-1099).
    fn member_friend_kind_mismatch(
        &mut self,
        template: DeclId,
        name: DeclName,
        function: bool,
        at: Loc,
    ) -> bool {
        let Some(p) = self.pattern_of(template) else { return false };
        let Some(p) = self.ast.definition(p) else { return false };
        let found: Vec<DeclId> = self
            .ast
            .lookup_local(p, name)
            .iter_with_inline()
            .map(|(x, _)| self.ast.strip_shadow(x))
            .collect();
        let is_class = |s: &Self, x: DeclId| {
            matches!(s.kind_of(x), DeclKind::CXXRecord | DeclKind::ClassTemplate)
        };
        let is_fn = |s: &Self, x: DeclId| {
            s.kind_of(x).is_function() || s.kind_of(x) == DeclKind::FunctionTemplate
        };
        let Some(&first) = found.first() else { return false };
        let ok = found.iter().any(|&x| if function { is_fn(self, x) } else { is_class(self, x) });
        // A typedef member is diagnosed elsewhere for a class friend (or
        // names a class); an alias template likewise.
        let other_kind = found.iter().all(|&x| {
            let k = self.kind_of(x);
            if function {
                !is_fn(self, x) && k != DeclKind::UsingShadow
            } else {
                is_fn(self, x)
                    || matches!(k, DeclKind::Var | DeclKind::Field | DeclKind::VarTemplate)
            }
        });
        if ok || !other_kind {
            return false;
        }
        let msg =
            self.diag(DiagId::sema_redefinition_different_kind, at).arg(self.decl_name_str(name));
        let msg = self.note_prev(msg, first, true);
        self.emit(msg);
        true
    }

    /// The friend `pattern` of a class template, instantiated in the
    /// class being defined (`record`) with the substituted specifier
    /// `nns`, target type `ty` and the friend's parameters' new `depth`;
    /// `outer` the instantiation's arguments.
    pub(crate) fn instantiate_member_friend(
        &mut self,
        pattern: &MemberFriend,
        nns: NnsId,
        ty: QualType,
        (depth, outer): (u16, &MultiLevelArgs),
        loc: Loc,
    ) -> Option<DeclId> {
        let record = self.current_class()?.record;
        let (template, args, path) = self.member_friend_shape(nns)?;
        let fd = self.builder().friend(record, loc, FriendTarget::Type { ty, owned_tag: None });
        let fn_ty = pattern.fn_ty.map(|_| ty);
        // The head and requires-clause keep the pattern's levels: the
        // arguments of every level outside the friend's own head, when
        // this instantiation substitutes them all (else the constraints
        // are not compared, `member_friend_constraints_match`).
        let enclosing = (pattern.enclosing.is_none()
            && outer.depth() == usize::from(pattern.depth)
            && outer.substituted() == outer.depth())
        .then(|| outer.clone());
        let mf =
            MemberFriend { nns, depth, fn_ty, enclosing, template, args, path, ..pattern.clone() };
        self.register_member_friend(fd, mf);
        Some(fd)
    }

    /// [temp.friend]/9: `template<class T> friend int A<T>::w() requires
    /// C<T>;` befriends a non-template member, so it is a non-template
    /// friend declaration with a requires-clause, which must be a
    /// definition, and a qualified friend cannot be one ([class.friend]/6).
    /// As in clang 19 and 22: in a templated class (the friend's own head
    /// at `depth` > 0) `err_non_temp_friend_decl_with_requires_clause_
    /// must_be_def` at that head's `template` (`head_loc`); in a
    /// non-template class the friend's head does not make the function
    /// templated, so `err_constrained_non_templated_function` at the
    /// clause. Whether one was reported (WP-1146).
    fn non_template_member_friend_requires(
        &mut self,
        d: &Declarator,
        depth: u16,
        head_loc: Option<Loc>,
    ) -> bool {
        let Some(r) = d.trailing_requires.filter(|&r| !self.is_err_expr(r)) else { return false };
        if !self.inst.stack.is_empty() {
            return false;
        }
        let e = if depth == 0 {
            let at = self.begin_loc(r);
            self.diag(DiagId::sema_trailing_requires_non_template, at)
        } else {
            let at = head_loc.unwrap_or(d.loc);
            self.diag(DiagId::sema_non_temp_friend_decl_with_requires_clause_must_be_def, at)
        };
        self.emit(e);
        true
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
        // A member template is befriended only by a friend with a
        // template head of its own, and the other way round
        // ([temp.over.link]/4; WP-1098).
        let e_tmpl = self.member_friend_template_of(e);
        if mf.fn_ty.is_some() && e_tmpl.is_some() != mf.inner_head {
            return Grant::No;
        }
        // A member of the pattern or of a partial specialization: some
        // instantiation may be befriended.
        let Some((_, si)) = self.ast.spec_info(s) else { return Grant::Speculative };
        let as_ = self.ast.types.args(self.ast.types.canonical_args(si.args)).to_vec();
        let loc = self.ast.decls.get(fd).loc();
        let r = self.in_sfinae(FrameKind::DeducedArgSubstitution, mf.template, None, loc, |s| {
            let mut unknown = false;
            let mut d = DeducedArgs::new(mf.depth, usize::from(mf.params));
            if s.deduce_args(&mf.args, &as_, false, &mut d, loc) != Tdk::Success
                || d.first_undeduced().is_some()
            {
                return Grant::No;
            }
            let Some(fty) = mf.fn_ty else { return Grant::Yes };
            let deduced = s.ast.types.arg_list(&d.args);
            let mut levels = Self::outer_levels(mf.template, mf.depth);
            levels.push(deduced, mf.template, LevelMode::Substitute);
            // The levels outside the friend's member template head as
            // written (an instantiated friend's keep its class's).
            let mut head_levels =
                mf.enclosing.clone().unwrap_or_else(|| Self::outer_levels(mf.template, mf.depth));
            head_levels.push(deduced, mf.template, LevelMode::Substitute);
            let mut e_ty = e_ty;
            if let Some(t) = e_tmpl {
                // The friend's own head names the member template's
                // parameters: equivalent heads, then equivalent types
                // with the friend's parameters renamed to the member's.
                let Some((inner, own)) =
                    mf.inner_list.zip(s.ast.template_info(t).map(|i| i.params))
                else {
                    return Grant::No;
                };
                let Some(renamed) = s.member_friend_heads_match(inner, own, &head_levels, loc)
                else {
                    return Grant::No;
                };
                match s.member_friend_constraints_match(&mf, Some(t), e, &head_levels) {
                    Some(true) => {}
                    Some(false) => return Grant::No,
                    None => unknown = true,
                }
                levels.push(renamed, mf.template, LevelMode::Substitute);
                e_ty = s.pattern_of(t).and_then(|p| s.ast.decls.get(p).ty());
            } else {
                match s.member_friend_constraints_match(&mf, None, e, &head_levels) {
                    Some(true) => {}
                    Some(false) => return Grant::No,
                    None => unknown = true,
                }
            }
            let Ok(t) = Substituter::new(s, &levels).ty(fty, loc) else { return Grant::No };
            if !e_ty.is_some_and(|et| s.ast.types.canonical(et) == s.ast.types.canonical(t)) {
                return Grant::No;
            }
            // The constraints of a friend still in a class template's
            // pattern are decided per instantiation (WP-1146).
            if unknown { Grant::Speculative } else { Grant::Yes }
        });
        match r {
            Some((g, Ok(()))) => g,
            _ => Grant::No,
        }
    }
    /// The member function template `e` (its pattern, the template or a
    /// specialization of it) belongs to.
    fn member_friend_template_of(&self, e: DeclId) -> Option<DeclId> {
        if self.kind_of(e) == DeclKind::FunctionTemplate {
            return Some(e);
        }
        if let Some(t) = self.ast.described_template(self.ast.first(e)) {
            return Some(t);
        }
        self.ast
            .spec_info(e)
            .map(|(_, si)| si.template)
            .filter(|&t| self.kind_of(t) == DeclKind::FunctionTemplate)
    }

    /// [temp.over.link]/6: the friend's member template head `inner`
    /// (its outer levels substituted by `levels`) and the member
    /// template's head `own` are equivalent in their length, kinds,
    /// packs, non-type parameter types and template template parameter
    /// heads. The member's parameters as arguments for `inner`'s level
    /// when they are. Type-constraints and requires-clauses are compared
    /// by [`Sema::member_friend_constraints_match`].
    fn member_friend_heads_match(
        &mut self,
        inner: TemplateParamListId,
        own: TemplateParamListId,
        levels: &MultiLevelArgs,
        loc: Loc,
    ) -> Option<ArgListId> {
        let (li, lo) = (*self.ast.templates.param_list(inner), *self.ast.templates.param_list(own));
        let pi: Vec<DeclId> = self.ast.lists.iter::<DeclId>(li.params).collect();
        let po: Vec<DeclId> = self.ast.lists.iter::<DeclId>(lo.params).collect();
        if pi.len() != po.len() {
            return None;
        }
        let mut args = Vec::with_capacity(po.len());
        for (&x, &y) in pi.iter().zip(&po) {
            let (kx, ky) = (self.kind_of(x), self.kind_of(y));
            let (ix, iy) = (self.ast.template_parm(x).copied(), self.ast.template_parm(y).copied());
            if kx != ky || ix.map(|i| i.pack) != iy.map(|i| i.pack) {
                return None;
            }
            match kx {
                DeclKind::NonTypeTemplateParm => {
                    let tx = self.ast.decls.get(x).ty()?;
                    let ty = self.ast.decls.get(y).ty()?;
                    let tx = Substituter::new(self, levels).ty(tx, loc).ok()?;
                    if self.ast.types.canonical(tx) != self.ast.types.canonical(ty) {
                        return None;
                    }
                }
                DeclKind::TemplateTemplateParm => {
                    let (a, b) = (ix.and_then(|i| i.params), iy.and_then(|i| i.params));
                    match (a, b) {
                        (Some(a), Some(b)) if self.heads_equivalent(a, b) => {}
                        (None, None) => {}
                        _ => return None,
                    }
                }
                _ => {}
            }
            let a = self.template_param_arg(y);
            let pack = iy.is_some_and(|i| i.pack);
            args.push(if pack { TemplateArg::pack(self.ast.types.arg_list(&[a])) } else { a });
        }
        Some(self.ast.types.arg_list(&args))
    }
    /// [temp.over.link]/6-7, [basic.scope.scope]/4: the friend `mf`
    /// names member function `e` (of member template `t`, whose head
    /// matched the friend's inner one) only with equivalent constraints:
    /// each template parameter's type-constraint, the head's
    /// requires-clause and the trailing requires-clause (an overload set
    /// `f(U)`, `f(U) requires C<U>` has two members, WP-1130). Each side
    /// is compared substituted, as clang's `AreConstraintExpressionsEqual`
    /// does: the friend's with `levels` (the arguments the friend's head
    /// deduced, its member template level kept), the member's with its
    /// class's (`member_constraints`), so both name the member template's
    /// parameters at the same depth. A friend whose levels outside its
    /// head are not all known is still in a class template's pattern
    /// (`enclosing` is `None`): `None`, the friend only might befriend
    /// `e` ([`Grant::Speculative`], as clang leaves a dependent friend to
    /// its instantiations; WP-1146). Each instantiation of that class
    /// registers it again with every level known
    /// (`instantiate_member_friend`), and that friend is compared, so a
    /// friend of a member class template instantiated for two arguments
    /// is compared in each (WP-1145).
    fn member_friend_constraints_match(
        &mut self,
        mf: &MemberFriend,
        t: Option<DeclId>,
        e: DeclId,
        levels: &MultiLevelArgs,
    ) -> Option<bool> {
        if levels.substituted() != levels.depth() {
            return None;
        }
        Some(self.member_friend_constraints_equivalent(mf, t, e, levels))
    }

    fn member_friend_constraints_equivalent(
        &mut self,
        mf: &MemberFriend,
        t: Option<DeclId>,
        e: DeclId,
        levels: &MultiLevelArgs,
    ) -> bool {
        // The friend's: type-constraints, requires-clause, trailing one.
        let mut ours: Vec<Option<ExprId>> = Vec::new();
        if t.is_some() {
            let Some(inner) = mf.inner_list else { return false };
            let l = *self.ast.templates.param_list(inner);
            let ps: Vec<DeclId> = self.ast.lists.iter::<DeclId>(l.params).collect();
            ours.extend(ps.iter().map(|&p| self.ast.template_parm(p).and_then(|i| i.constraint)));
            ours.push(l.requires);
        }
        ours.push(mf.requires);
        // The member's, from its pattern's declaration when its class is
        // a specialization (they stay the pattern's). Each side's
        // function parameters, which a trailing requires-clause may name,
        // become parameters with the substituted types
        // ([temp.over.link]/5: positions and types compare; WP-1146).
        let params = |s: &Self, f: Option<DeclId>| -> Vec<DeclId> {
            f.and_then(|f| s.ast.decls.function(f))
                .map(|x| s.ast.lists.iter::<DeclId>(x.params).collect())
                .unwrap_or_default()
        };
        let (theirs, outer) = if let Some(t) = t {
            let (src, map) = match self.inst.concepts.member_constraints.get(&t) {
                Some(&(pt, _)) if pt != t => (pt, self.member_cmp_constraints(t)),
                _ => (t, FxHashMap::default()),
            };
            let Some(info) = self.ast.template_info(src).copied() else { return false };
            let l = *self.ast.templates.param_list(info.params);
            let ps: Vec<DeclId> = self.ast.lists.iter::<DeclId>(l.params).collect();
            let mut v: Vec<Option<ExprId>> =
                ps.iter().map(|&p| self.ast.template_parm(p).and_then(|i| i.constraint)).collect();
            v.push(l.requires);
            let f = self.templated_function(src);
            v.push(f.and_then(|f| self.ast.templates.trailing_requires.get(f).copied()));
            let v = v.into_iter().map(|c| c.map(|c| map.get(&c).copied().unwrap_or(c))).collect();
            (v, None)
        } else {
            let r = self.ast.templates.trailing_requires.get(e).copied();
            let mc = self.inst.concepts.member_constraints.get(&e).cloned();
            let outer = mc.map(|(p, o)| (o, params(self, Some(p))));
            (vec![r], outer)
        };
        if ours.len() != theirs.len()
            || ours.iter().zip(&theirs).any(|(a, b)| a.is_some() != b.is_some())
        {
            return false;
        }
        let mut inner = levels.clone();
        if let Some(t) = t {
            inner.push(ArgListId::EMPTY, t, LevelMode::Retain);
        }
        let loc = self.ast.decls.get(e).loc();
        for (a, b) in ours.into_iter().zip(theirs) {
            let (Some(a), Some(mut b)) = (a, b) else { continue };
            let mut sub = Substituter::new(self, &inner);
            for &p in &mf.fn_params {
                let _ = sub.nested_prototype_param(p, loc);
            }
            let Ok(a) = sub.expr(a) else { return false };
            if let Some((o, ps)) = &outer {
                let mut sub = Substituter::new(self, o);
                for &p in ps {
                    let _ = sub.nested_prototype_param(p, loc);
                }
                let Ok(x) = sub.expr(b) else { return false };
                b = x;
            }
            if !self.exprs_equivalent(a, b) {
                return false;
            }
        }
        true
    }
}
