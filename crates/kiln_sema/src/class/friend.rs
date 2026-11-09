//! Friend declarations ([class.friend], spec/sema-class.md §2.4; WP-066b).

use kiln_ast::DeclId;
use kiln_ast::QualType;
use kiln_ast::decl::{DeclKind, FriendTarget};
use kiln_ast::names::DeclName;
use kiln_ast::types::{TemplateArg, TypeView};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::declspec::{DeclSpec, Declarator, DefaultArg};
use crate::local_friend::LocalFriendMember;
use crate::lookup::LookupKind;
use crate::templ::pack::PackOperand;

/// A friend function template specialization in a dependent context
/// (`friend bool operator==<>(const X&, const X&);` in a class
/// template, [temp.friend]/1.3; clang's
/// `DependentFunctionTemplateSpecializationInfo`): matched when its class
/// is instantiated.
#[derive(Clone, Debug)]
pub(crate) struct FriendSpec {
    /// The name the declarator-id's template-id names.
    pub(crate) name: DeclName,
    /// The declared (dependent) function type.
    pub(crate) ty: QualType,
    /// The explicit template arguments written (possibly none, `f<>`).
    pub(crate) args: Vec<TemplateArg>,
    /// The function templates lookup found at the declaration.
    pub(crate) templates: Vec<DeclId>,
    /// The declarator-id.
    pub(crate) loc: Loc,
    /// Its noexcept-specifier waits for the end of the class (not in
    /// `ty`), so it is not compared with the specialization's.
    pub(crate) late_exc: bool,
}

impl Sema<'_, '_> {
    /// GNU `noreturn` and `nothrow` are function type attributes on a
    /// friend as on any declaration (spec/types.md §5.4, WP-752): a later
    /// redeclaration inherits them from the friend's type.
    fn friend_fn_type_attrs(&mut self, ty: QualType, ds: &DeclSpec, d: &Declarator) -> QualType {
        if !self.ast.types.is_function(ty) {
            return ty;
        }
        let mut attrs = self.non_vector_attrs(&ds.attrs);
        attrs.extend(d.attrs.iter().copied());
        let ty = self.apply_nothrow_attrs(ty, &mut attrs);
        let ty = self.apply_noreturn_attrs(ty, &attrs);
        self.apply_cc_attrs(ty, &mut attrs, true)
    }

    /// [dcl.fct.default]/4: a friend declaration `f` that specifies a
    /// default argument is a definition and the only declaration of its
    /// function (clang `ActOnFriendFunctionDecl`; a later redeclaration
    /// is reported by `act_on_function`). Checked as written, not on
    /// instantiation; an invalid redeclaration was already reported.
    fn check_friend_default_args(&mut self, f: DeclId, d: &Declarator, def: bool) {
        let has_default = d
            .function_params()
            .is_some_and(|ps| ps.iter().any(|p| !matches!(p.default_arg, DefaultArg::None)));
        if !has_default || self.ast.decls.get(f).has(kiln_ast::decl::DeclFlags::INVALID) {
            return;
        }
        if let Some(p) = self.ast.prev(f) {
            let msg = self.diag(DiagId::sema_friend_decl_with_def_arg_redeclared, d.name_loc());
            let msg = self.note_prev(msg, p, false);
            self.emit(msg);
        } else if !def {
            self.error(DiagId::sema_friend_decl_with_def_arg_must_be_def, d.name_loc());
        }
    }

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
            return self.declare_friend_type(ds, base, record, owned, friend_loc);
        };
        // Constructor, destructor, conversion and operator names
        // (WP-067a/WP-068b) are still stubs; report them by name.
        // `template<class U> friend void f(U);` ([temp.friend]/1): a
        // friend function template.
        if d.dropped {
            return None;
        }
        // `friend void f(auto);` is an abbreviated friend function
        // template ([dcl.fct]/22): its invented head is the friend's.
        self.finish_invented_head();
        let head = self.take_template_head(d.qualifier);
        if self.check_template_in_local_class(true, ds.lead_cxx11.unwrap_or(ds.loc)) {
            return None;
        }
        let op_d = self.operator_declarator(d);
        let d = op_d.as_ref().unwrap_or(d);
        if self.unsupported_declarator(ds, d) {
            self.templ.cur = None;
            return None;
        }
        let ty = self.type_for_declarator_at(base, ds.type_loc.unwrap_or(ds.loc), d);
        let ty = self.check_qualified_fn_type(ty, ds, d);
        let ty = self.friend_fn_type_attrs(ty, ds, d);
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
        // [temp.variadic]/12: `friend void f(Ts);` (clang reports it at
        // the declarator-id).
        if self.diagnose_unexpanded_pack(PackOperand::Type(ty), "friend declaration", d.name_loc())
        {
            self.templ.cur = None;
            return None;
        }
        if head.is_none()
            && let Some(sid) = &d.spec_id
            && self.kind_of(sid.template) == DeclKind::FunctionTemplate
        {
            let dname = d.fn_decl_name(name);
            return self.declare_friend_spec(record, d, ty, dname);
        }
        if head.is_some() {
            self.mark_pattern_next();
        }
        // An unqualified friend of a local class may redeclare a member
        // of an enclosing local class ([class.friend]/11): declared as
        // if qualified by that class.
        let local = d.qualifier.is_none() && self.is_local_class(record);
        // A definition being parsed (not one instantiated meanwhile).
        let def = self.templ.fn_def_next && self.inst.stack.is_empty();
        let member_d;
        let d = if local && head.is_none() {
            let dname = d.fn_decl_name(name);
            match self.local_friend_member(dname, ty, d.trailing_requires, d.name_loc()) {
                LocalFriendMember::None => d,
                LocalFriendMember::Member(class) => {
                    let mut q = d.clone();
                    q.qualifier = Some(class);
                    member_d = q;
                    &member_d
                }
                LocalFriendMember::Invalid => {
                    self.templ.cur = None;
                    return None;
                }
            }
        } else {
            d
        };
        let f = self.act_on_function(ds, ty, d, name, true);
        // [class.friend]/6: a function may be defined in a friend
        // declaration only of a non-local class (clang
        // `err_friend_def_in_local_class`, for a valid declaration).
        if local && def && !self.ast.decls.get(f).has(kiln_ast::decl::DeclFlags::INVALID) {
            self.error(DiagId::sema_friend_def_in_local_class, d.name_loc());
        }
        if ds.invalid || d.invalid {
            self.ast.decls.set_flags(f, kiln_ast::decl::DeclFlags::INVALID);
        }
        if self.inst.stack.is_empty() {
            self.check_friend_default_args(f, d, def);
        }
        if let Some(r) = d.trailing_requires {
            self.act_on_trailing_requires(f, r);
        }
        let mut attrs = ds.attrs.clone();
        attrs.extend(d.attrs.iter().copied());
        if !attrs.is_empty() {
            if !def {
                // [dcl.attr.annotation]/1.
                self.reject_annotations(
                    &mut attrs,
                    Some("a friend declaration that is not a definition"),
                );
            }
            self.check_decl_attrs(f, &mut attrs);
            let _ = self.builder().attach_attrs(f, &attrs);
        }
        let target = self.finish_template_pattern(f).unwrap_or(f);
        // [class.friend]/10: befriending a member function of another
        // class names it, so it must be accessible here (clang
        // `CheckFriendAccess`, only for a declaration that redeclares a
        // member of a class it found through its qualifier).
        if d.qualifier.is_some()
            && !local
            && self.ast.first(target) != target
            && self.declaring_class(target).is_some()
            && !self.ast.decls.get(target).has(kiln_ast::decl::DeclFlags::INVALID)
            && self.inst.stack.is_empty()
            && self.declaring_class(target).is_some_and(|c| !self.in_dependent_class(c))
        {
            // A member of a dependent class is checked in each
            // instantiation (`TemplateDeclInstantiator`), as is one only
            // a befriended specialization may name.
            self.check_friend_access(target, d.name_loc());
        }
        let fd = self.builder().friend(record, d.name_loc(), FriendTarget::Decl(target));
        let first = self.ast.first(target);
        self.push_friend(first);
        Some(fd)
    }

    /// `friend T;`, `friend class X;` and `template<class U> friend class
    /// X;`: [`Sema::declare_friend`] without a declarator.
    fn declare_friend_type(
        &mut self,
        ds: &DeclSpec,
        base: QualType,
        record: DeclId,
        owned: Option<DeclId>,
        friend_loc: Loc,
    ) -> Option<DeclId> {
        self.reject_friend_type_quals(ds);
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
        // `friend T;` / `friend class X;` ([class.friend]/3). A pack
        // outside an expansion is ill-formed ([temp.variadic]/12).
        if self.diagnose_unexpanded_pack(PackOperand::Type(base), "friend declaration", friend_loc)
        {
            return None;
        }
        // [dcl.fct]/6: a qualified function typedef (`typedef int G()
        // const; friend G;`) is not the type of a non-static member
        // function; clang `err_invalid_qualified_function_type` at the
        // start of the declaration (the friend is ignored either way,
        // [class.friend]/3). A dependent type is not checked when it
        // substitutes (clang is silent for `friend T;` with `T = G`), and
        // a non-dependent one was reported in the pattern.
        let base = if self.inst.stack.in_instantiation_record() {
            base
        } else {
            self.reject_qualified_fn_type(base, ds.loc, false)
        };
        let loc = ds.type_loc.unwrap_or(friend_loc);
        let target = FriendTarget::Type { ty: base, owned_tag: owned };
        let fd = self.builder().friend(record, loc, target);
        // `friend class X<T>;` naming the current instantiation
        // befriends the pattern too ([temp.dep.type]/1): its own
        // members are checked against it before instantiation.
        let rec =
            self.ast.types.as_record(base).or_else(|| self.friend_current_instantiation(base));
        if let Some(c) = rec {
            let c = self.ast.first(c);
            self.push_friend(c);
        }
        Some(fd)
    }

    /// [class.friend]/3: a friend-type-specifier is a simple-, elaborated-
    /// or typename-specifier, so no cv-qualifier may be written in a
    /// friend type declaration (`friend const T;`): clang reports each
    /// (`err_friend_decl_spec`, in the order const, volatile, restrict,
    /// _Atomic, at the qualifier) and declares the friend without it. A
    /// cv-qualified typedef is fine.
    fn reject_friend_type_quals(&mut self, ds: &DeclSpec) {
        let [c, v, r] = ds.cv_locs;
        let quals = [(c, "const"), (v, "volatile"), (r, "restrict"), (ds.atomic_qual, "_Atomic")];
        for (at, name) in quals {
            if let Some(at) = at {
                let d = self.diag(DiagId::parse_friend_decl_spec, at).arg(name);
                self.emit(d);
            }
        }
    }

    /// `friend R f<args>(params);` ([temp.friend]/1.3): befriends the
    /// specialization of a function template found by lookup of `name`
    /// that the declared type deduces ([temp.deduct.decl]); in a
    /// dependent context the match waits for each specialization of the
    /// class ([`FriendSpec`]).
    fn declare_friend_spec(
        &mut self,
        record: DeclId,
        d: &Declarator,
        ty: QualType,
        name: DeclName,
    ) -> Option<DeclId> {
        let sid = d.spec_id.as_ref()?;
        let loc = d.name_loc();
        let found = self.friend_spec_lookup(name, d.qualifier);
        let templates = self.function_templates_in(&found);
        if self.in_template() {
            if templates.is_empty() {
                self.dependent_spec_no_candidate(
                    DiagId::sema_dependent_friend_spec_no_candidate,
                    &found,
                    loc,
                );
                return None;
            }
            let target = FriendTarget::Type { ty, owned_tag: None };
            let fd = self.builder().friend(record, loc, target);
            let args = sid.args.clone();
            let late_exc = d.late_noexcept.is_some();
            self.friend_specs.insert(fd, FriendSpec { name, ty, args, templates, loc, late_exc });
            return Some(fd);
        }
        let explicit = Some(self.ast.types.arg_list(&sid.args));
        let m = self.match_function_spec_in(&templates, explicit, name, ty, false, loc)?;
        // A noexcept-specifier delayed to the end of the class is not
        // compared (clang's delayed equivalence check is a follow-up).
        let fix_at = crate::except::missing_exc_spec_fixit_loc(d);
        if d.late_noexcept.is_none() && !self.check_spec_exc_spec(m.spec, ty, loc, fix_at) {
            return None;
        }
        Some(self.befriend_spec(record, m.spec, loc))
    }

    /// The [`FriendSpec`] of a class template's pattern for the
    /// specialization being instantiated (`record`), its type and
    /// arguments substituted.
    pub(crate) fn instantiate_friend_spec(
        &mut self,
        record: DeclId,
        fs: &FriendSpec,
        ty: QualType,
        args: &[TemplateArg],
    ) -> Option<DeclId> {
        let explicit = Some(self.ast.types.arg_list(args));
        let m = self.match_function_spec_in(&fs.templates, explicit, fs.name, ty, false, fs.loc)?;
        if !fs.late_exc && !self.check_spec_exc_spec(m.spec, ty, fs.loc, None) {
            return None;
        }
        Some(self.befriend_spec(record, m.spec, fs.loc))
    }

    fn befriend_spec(&mut self, record: DeclId, spec: DeclId, loc: Loc) -> DeclId {
        let fd = self.builder().friend(record, loc, FriendTarget::Decl(spec));
        let first = self.ast.first(spec);
        self.push_friend(first);
        fd
    }

    /// What lookup of a friend's template-id name finds ([temp.friend]/1.3:
    /// ordinary lookup, or qualified lookup in its nested-name-specifier).
    fn friend_spec_lookup(&self, name: DeclName, qualifier: Option<DeclId>) -> Vec<DeclId> {
        match qualifier {
            Some(q) => self.lookup_qualified_dn(q, name, LookupKind::Ordinary).decls,
            None => self.lookup_name_dn(name, LookupKind::Ordinary).decls,
        }
    }

    /// The function template a friend declarator-id `name<...>` names
    /// (the first found), for the parser.
    pub fn friend_spec_template(
        &self,
        name: DeclName,
        qualifier: Option<DeclId>,
    ) -> Option<DeclId> {
        let found = self.friend_spec_lookup(name, qualifier);
        self.function_templates_in(&found).first().copied()
    }

    /// `friend void f<>(int);` where lookup of `f` finds no function
    /// template: clang's `err_dependent_function_template_spec_no_match`
    /// in a dependent context (with a note per declaration found), else
    /// `err_function_template_spec_no_match`. The declaration is dropped.
    pub fn act_on_friend_spec_without_template(
        &mut self,
        name: DeclName,
        qualifier: Option<DeclId>,
        loc: Loc,
    ) {
        if self.in_template() {
            let found = self.friend_spec_lookup(name, qualifier);
            self.dependent_spec_no_candidate(
                DiagId::sema_dependent_friend_spec_no_candidate,
                &found,
                loc,
            );
        } else {
            let n = self.decl_name_str(name);
            let e = self.diag(DiagId::sema_fn_template_spec_no_match, loc).arg(n);
            self.emit(e);
        }
    }

    /// clang's `err_dependent_function_template_spec_no_match` (`id`: the
    /// friend or member form) at `loc`, with "candidate ignored: not a
    /// function template" at each declaration `found`.
    pub(crate) fn dependent_spec_no_candidate(&mut self, id: DiagId, found: &[DeclId], loc: Loc) {
        let mut e = self.diag(id, loc);
        for &f in found {
            let n =
                self.diag(DiagId::note_dependent_spec_not_template, self.ast.decls.get(f).loc());
            e = e.with_note(n);
        }
        self.emit(e);
    }

    /// The pattern a dependent `X<P...>` (befriended in a class, or the
    /// type of a member access's object) names when it is the current
    /// instantiation ([temp.dep.type]/1.2).
    pub(crate) fn friend_current_instantiation(&self, ty: QualType) -> Option<DeclId> {
        let mut q = ty;
        for _ in 0..16 {
            if let TypeView::TemplateSpecialization { name, .. } = self.ast.types.view(q.ty()) {
                let tmpl = self.ast.types.template_decl(name)?;
                return self.current_instantiation(tmpl, q);
            }
            let n = self.ast.types.desugar_once(q);
            if n == q {
                return None;
            }
            q = n;
        }
        None
    }

    pub(crate) fn push_friend(&mut self, d: DeclId) {
        if let Some(c) = self.classes.last_mut()
            && !c.friends.contains(&d)
        {
            c.friends.push(d);
        }
    }
}
