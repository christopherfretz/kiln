//! Friend declarations ([class.friend], spec/sema-class.md §2.4; WP-066b).

use kiln_ast::DeclId;
use kiln_ast::QualType;
use kiln_ast::decl::{ConstexprKind, DeclKind, FriendTarget};
use kiln_ast::ids::TemplateParamListId;
use kiln_ast::names::DeclName;
use kiln_ast::types::{TagKeyword, TemplateArg, TypeView};
use kiln_diag::DiagId;
use kiln_source::Loc;

use crate::Sema;
use crate::declspec::{DeclSpec, Declarator, DefaultArg, SpecialName};
use crate::local_friend::LocalFriendMember;
use crate::lookup::LookupKind;
use crate::templ::pack::PackOperand;

/// A friend function template specialization in a dependent context
/// (`friend bool operator==<>(const X&, const X&);` in a class
/// template, [temp.friend]/1.3; clang's
/// `DependentFunctionTemplateSpecializationInfo`): matched when its class
/// is instantiated.
#[derive(Clone, Debug, kiln_snap::Snap)]
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

/// Which special member a friend of a class template's pattern names
/// (`friend Z<T>::Z(typename T::t);`, `friend Z<T>::~Z();`, `friend
/// Z<T>::operator int();`; [class.friend]/4, [class.qual]/2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, kiln_snap::Snap)]
pub(crate) enum SpecialFriendKind {
    /// A constructor.
    Ctor,
    /// The destructor.
    Dtor,
    /// A conversion function (its type: the function type's return type).
    Conversion,
}

/// A special member friend of a templated class whose class or type is
/// dependent, matched per specialization ([`Sema::befriend_special_member`]):
/// side data of its `FriendDecl` (`Sema::special_friends`), so the class
/// record does not grow.
#[derive(Clone, Copy, Debug, kiln_snap::Snap)]
pub(crate) struct SpecialFriend {
    /// The special member named.
    pub(crate) kind: SpecialFriendKind,
    /// The declared function type (a constructor's or destructor's
    /// returning `void`).
    pub(crate) ty: QualType,
    /// The class the qualifier names in the definition (a pattern, or a
    /// class of the current instantiation).
    pub(crate) class: DeclId,
    /// The written nested-name-specifier, substituted per specialization
    /// (none: `class`, instantiated with the enclosing template when it
    /// is templated).
    pub(crate) nns: Option<kiln_ast::ids::NnsId>,
    /// The friend's own template head (`template<class U> friend
    /// Z<T>::Z(U);`): it names a member template ([temp.friend]/1).
    pub(crate) head: Option<TemplateParamListId>,
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

    /// [temp.param]/12: a friend template declaration that is not a
    /// function template definition specifies no default template
    /// argument. Reported at each defaulted parameter of `head` (its
    /// name, or the `=` when unnamed; clang
    /// `err_template_parameter_default_friend_template`). Checked as
    /// written, not on instantiation. The written default stays in the
    /// AST (`TemplateTables::defaults` is write-once).
    pub(crate) fn check_friend_template_defaults(&mut self, head: Option<TemplateParamListId>) {
        let Some(list) = head.filter(|_| self.inst.stack.is_empty()) else { return };
        let l = *self.ast.templates.param_list(list);
        let params: Vec<DeclId> = self.ast.lists.iter::<DeclId>(l.params).collect();
        for p in params {
            // Recovery: the default no longer counts for the
            // redeclaration rules (clang drops it).
            if self.templ.default_locs.remove(&p).is_some() {
                let loc = self.ast.decls.get(p).loc();
                let m = self.diag(DiagId::sema_template_param_default_friend_template, loc);
                self.emit(m.allow_duplicate());
            }
        }
    }

    /// [`Sema::check_friend_template_defaults`] for `template<...> friend
    /// class X;`: not in a dependent context, where clang's
    /// `CheckClassTemplate` does not check the head of a friend.
    pub(crate) fn check_friend_class_template_defaults(
        &mut self,
        head: Option<TemplateParamListId>,
    ) {
        if head.is_some_and(|l| self.ast.templates.param_list(l).depth == 0) {
            self.check_friend_template_defaults(head);
        }
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

    /// [class.friend]/10: befriending a member function of another class
    /// names it, so it must be accessible from the befriending class
    /// (clang `CheckFriendAccess`, only for a qualified declaration that
    /// redeclares a member of a class). A member of a dependent class is
    /// checked in each instantiation (`TemplateDeclInstantiator`), as is
    /// one only a befriended specialization may name.
    fn friend_needs_access_check(&self, target: DeclId) -> bool {
        self.ast.first(target) != target
            && !self.ast.decls.get(target).has(kiln_ast::decl::DeclFlags::INVALID)
            && self.inst.stack.is_empty()
            && self.declaring_class(target).is_some_and(|c| !self.in_dependent_class(c))
    }

    /// [temp.friend]/9: a non-template friend declaration with a
    /// requires-clause, and a friend function template with a constraint
    /// that depends on a template parameter of an enclosing template,
    /// must be definitions (clang `ActOnFunctionDeclarator`:
    /// `err_non_temp_friend_decl_with_requires_clause_must_be_def`,
    /// `err_friend_decl_with_enclosing_temp_constraint_must_be_def`, at
    /// the start of the declaration after any template head). Checked as
    /// written, for a friend `f` that is not a definition; `f` is marked
    /// invalid as in clang, which then reports nothing else about its
    /// default arguments. A requires-clause outside a templated entity
    /// is reported by `act_on_trailing_requires` instead.
    fn check_constrained_friend_def(
        &mut self,
        f: DeclId,
        head: Option<TemplateParamListId>,
        ds: &DeclSpec,
        d: &Declarator,
    ) {
        if !self.in_template() {
            return;
        }
        let id = match head {
            None => {
                if d.trailing_requires.is_none_or(|r| self.is_err_expr(r)) {
                    return;
                }
                DiagId::sema_non_temp_friend_decl_with_requires_clause_must_be_def
            }
            // An invalid template (`template<C<T> U>` for a unary `C`)
            // was reported already.
            Some(list)
                if !self.ast.decls.get(f).has(kiln_ast::decl::DeclFlags::INVALID)
                    && self.constraints_name_enclosing(list, d.trailing_requires) =>
            {
                DiagId::sema_friend_decl_with_enclosing_temp_constraint_must_be_def
            }
            Some(_) => return,
        };
        // Each declarator of `friend void f() requires C<T>, g() requires
        // C<T>;` is reported at the same place.
        let e = self.diag(id, ds.lead_cxx11.unwrap_or(ds.loc)).allow_duplicate();
        self.emit(e);
        self.ast.decls.set_flags(f, kiln_ast::decl::DeclFlags::INVALID);
    }

    /// Whether a constraint of a friend function template with head
    /// `list` (a parameter's type-constraint, the head's requires-clause
    /// or the trailing requires-clause `trailing`) names a template
    /// parameter of an enclosing template, directly or through the
    /// injected-class-name or a member typedef of the current
    /// instantiation (clang `FriendConstraintsDependOnEnclosingTemplate`,
    /// [temp.friend]/9). As in clang, a static data member or member
    /// class of the enclosing template does not count.
    fn constraints_name_enclosing(
        &self,
        list: TemplateParamListId,
        trailing: Option<kiln_ast::ExprId>,
    ) -> bool {
        let l = *self.ast.templates.param_list(list);
        if l.depth == 0 {
            return false;
        }
        let mut cs: Vec<kiln_ast::ExprId> = self
            .ast
            .lists
            .iter::<DeclId>(l.params)
            .filter_map(|p| self.ast.template_parm(p).and_then(|i| i.constraint))
            .collect();
        cs.extend(l.requires);
        cs.extend(trailing);
        cs.retain(|&c| !self.is_err_expr(c));
        if cs.is_empty() {
            return false;
        }
        // A slot for every parameter index (`TemplateParmInfo::index` is
        // a `u16`); only non-defining constrained friend templates get
        // here.
        let width = usize::from(u16::MAX) + 1;
        let mut used = vec![false; width];
        (0..u32::from(l.depth)).any(|depth| {
            used.fill(false);
            for &c in &cs {
                self.mark_used_expr(c, depth, &mut used, 0);
            }
            used.contains(&true)
        })
    }

    /// [`Sema::declare_friend`]: true (after reporting it) if a friend
    /// declarator's type is not a function type, or contains an
    /// unexpanded pack ([temp.variadic]/12: `friend void f(Ts);`, which
    /// clang reports at the declarator-id).
    fn friend_fn_type_invalid(&mut self, ty: QualType, d: &Declarator) -> bool {
        if !self.ast.types.is_function(ty) {
            if !self.is_error_type(ty) {
                self.error(DiagId::sema_friend_not_class_or_fn, d.name_loc());
            }
            return true;
        }
        self.diagnose_unexpanded_pack(PackOperand::Type(ty), "friend declaration", d.name_loc())
    }

    /// [class.friend]/6: a function defined in a friend declaration is
    /// named by an unqualified name. `Some(dependent)` when `d` is a
    /// qualified friend function definition being parsed, `dependent`
    /// when its nested-name-specifier is (clang reports a dependent one
    /// at once and a non-dependent one once the friend matched).
    fn qualified_friend_def(&self, d: &Declarator) -> Option<bool> {
        let n = d.qualifier_nns.filter(|_| self.templ.fn_def_next && self.inst.stack.is_empty())?;
        Some(self.ast.types.nns_flags(n).intersects(kiln_ast::types::TypeFlags::DEPENDENT))
    }

    /// clang's `err_qualified_friend_def` at the start of the
    /// nested-name-specifier (clang 22 prints it without its trailing
    /// `::`, except the global `::`). The declaration goes on as written
    /// (WP-3140).
    fn report_qualified_friend_def(&mut self, d: &Declarator) {
        let Some(n) = d.qualifier_nns else { return };
        let mut spec = self.nns_str(n);
        if self.clang_compat >= 22 && spec.len() > 2 && spec.ends_with("::") {
            spec.truncate(spec.len() - 2);
        }
        let at = if d.id_qual.is_valid() { d.id_qual } else { d.name_loc() };
        let e = self.diag(DiagId::sema_qualified_friend_def, at).arg(spec);
        self.emit(e);
    }

    /// [`Sema::act_on_friend`]: the befriended function or type, recorded
    /// in the class being defined (`RecordCold.friends` at `}`).
    #[allow(
        clippy::too_many_lines,
        reason = "one step per friend rule, as clang's ActOnFriendFunctionDecl"
    )]
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
        // [temp.expl.spec]/1: an explicit specialization is not declared
        // by a friend (`template<> friend void f<0>();`, clang
        // `err_template_spec_decl_friend` at the name; nothing declared).
        if self.templ.explicit_spec.is_some() && self.at_explicit_head() {
            self.templ.explicit_spec = None;
            self.error(DiagId::sema_template_spec_decl_friend, d.name_loc());
            return None;
        }
        // `friend void f(auto);` is an abbreviated friend function
        // template ([dcl.fct]/22): its invented head is the friend's.
        self.finish_invented_head();
        let qualified_def = self.qualified_friend_def(d);
        if qualified_def == Some(true) {
            self.report_qualified_friend_def(d);
        }
        let concrete_qualified_def = qualified_def == Some(false);
        // `template<class T> friend void A<T>::f();` ([temp.friend]/5).
        if let Some(fd) = self.declare_member_friend_fn(ds, base, d, record) {
            return Some(fd);
        }
        let own_head = self.friend_head_is_own(d);
        let head = self.take_template_head_of(if own_head { None } else { d.qualifier }, true);
        // [temp.expl.spec]/15 (clang `err_template_param_list_matches_nontemplate`,
        // diagnosed by the call): a head matching an implicitly
        // instantiated class of the qualifier; nothing is declared.
        if self.templ.head_mismatch {
            return None;
        }
        if self.check_template_in_local_class(true, ds.lead_cxx11.unwrap_or(ds.loc)) {
            return None;
        }
        if let Some((sp, sloc)) = d.special
            && matches!(
                sp,
                SpecialName::Constructor(_)
                    | SpecialName::Destructor(_)
                    | SpecialName::Conversion(_)
            )
            && let Some(class) = d.qualifier
        {
            let fd = self.declare_special_member_friend(ds, base, d, record, class, head, sloc);
            if concrete_qualified_def && fd.is_some() {
                self.report_qualified_friend_def(d);
            }
            return fd;
        }
        // [class.friend]/1, [class.mem.general]/3: a constructor,
        // destructor or conversion function is a member, so a friend names
        // it qualified (clang's "must use a qualified name" error, after
        // the declarator's type checks; nothing declared).
        if let Some((sp, sloc)) = d.special
            && d.qualifier.is_none()
            && let Some(k) = match sp {
                SpecialName::Constructor(_) => Some(0u8),
                SpecialName::Destructor(_) => Some(1),
                SpecialName::Conversion(_) => Some(2),
                _ => None,
            }
        {
            if matches!(sp, SpecialName::Conversion(_)) {
                let ty = self.type_for_declarator_at(base, ds.type_loc.unwrap_or(ds.loc), d);
                let _ = self.drop_invalid_fn_quals(ty, d, false, ds.loc);
            }
            self.templ.cur = None;
            let msg = self.diag(DiagId::sema_friend_special_unqualified, sloc).arg(k);
            self.emit(msg);
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
        if self.friend_fn_type_invalid(ty, d) {
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
        if head.is_none()
            && let Some(class) = d.qualifier
            && let Some(fd) =
                self.declare_member_template_spec_friend(record, d, ty, d.fn_decl_name(name), class)
        {
            return Some(fd);
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
        if concrete_qualified_def && !self.ast.decls.get(f).has(kiln_ast::decl::DeclFlags::INVALID)
        {
            self.report_qualified_friend_def(d);
        }
        // [class.friend]/6: a function may be defined in a friend
        // declaration only of a non-local class (clang
        // `err_friend_def_in_local_class`, for a valid declaration).
        if local && def && !self.ast.decls.get(f).has(kiln_ast::decl::DeclFlags::INVALID) {
            self.error(DiagId::sema_friend_def_in_local_class, d.name_loc());
        }
        if ds.invalid || d.invalid {
            self.ast.decls.set_flags(f, kiln_ast::decl::DeclFlags::INVALID);
        }
        if !def && self.inst.stack.is_empty() {
            self.check_constrained_friend_def(f, head, ds, d);
        }
        if self.inst.stack.is_empty() {
            self.check_friend_default_args(f, d, def);
        }
        if let Some(r) = d.trailing_requires {
            self.act_on_trailing_requires(f, r);
        }
        let mut attrs = ds.attrs.clone();
        attrs.extend(d.attrs.iter().copied());
        // clang `ProcessDeclAttributes` -> `AddPragmaAttributes`: a friend
        // function declaration (a hidden friend included) takes the pushed
        // `#pragma clang attribute` attributes as any function does.
        self.add_pragma_attrs(f, &mut attrs);
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
        // Read by the attribute checks above only; the redeclaration
        // errors held for them follow (WP-1848).
        self.after_def_prev = None;
        self.release_after_def_held();
        if !def {
            self.check_friend_template_defaults(head);
        }
        let target = self.finish_template_pattern(f).unwrap_or(f);
        if d.qualifier.is_some() && !local && self.friend_needs_access_check(target) {
            // clang notes a member template at the friend redeclaring it.
            let note_at = head.is_some().then(|| d.name_loc());
            self.check_friend_access_noted(target, d.name_loc(), note_at);
        }
        let fd = self.builder().friend(record, d.name_loc(), FriendTarget::Decl(target));
        let first = self.ast.first(target);
        self.push_friend(first);
        Some(fd)
    }

    /// `friend A::A(int);`, `friend A::~A();`, `friend A::operator int();`
    /// ([class.friend]/4, [temp.friend]/1): a constructor, destructor or
    /// conversion function of class `class` befriended by `record`. The
    /// member is found by its name in `class` and matched by type, as an
    /// out-of-line declaration is ([class.mfct]/3): a non-template member
    /// first, else the specialization of a member template deduced from
    /// the type ([temp.friend]/1.2, [temp.deduct.decl]); no match is
    /// "friend declaration of 'A' does not match any declaration in 'A'".
    /// The befriended member must be accessible ([class.friend]/9).
    ///
    /// A templated or dependent class, or a dependent type, cannot be
    /// matched in the template definition: the friend keeps its type and
    /// a [`SpecialFriend`] entry, matched in each specialization
    /// (WP-1507). A friend template (`head`, `template<class U> friend
    /// Z::Z(U);`) befriends the member template whose head and type are
    /// equivalent ([temp.friend]/1, [temp.over.link]/6; WP-1513).
    #[allow(clippy::too_many_arguments, reason = "the friend's parts as declare_friend has them")]
    fn declare_special_member_friend(
        &mut self,
        ds: &DeclSpec,
        base: QualType,
        d: &Declarator,
        record: DeclId,
        class: DeclId,
        head: Option<TemplateParamListId>,
        sloc: Loc,
    ) -> Option<DeclId> {
        let ty = self.type_for_declarator_at(base, ds.type_loc.unwrap_or(ds.loc), d);
        self.templ.cur = None;
        let (sp, _) = d.special?;
        // [class.dtor]/1: a destructor is not a template, a friend one
        // either (clang `err_destructor_template`).
        if head.is_some() && matches!(sp, SpecialName::Destructor(_)) {
            self.error(DiagId::sema_destructor_template, sloc);
            return None;
        }
        if !self.ast.types.is_function(ty) {
            self.error(DiagId::parse_expected_unqualified_id, sloc);
            return None;
        }
        let ty = match sp {
            SpecialName::Constructor(_) => self.rebuild_fn_type(ty, QualType::VOID, None, true),
            SpecialName::Destructor(n) => {
                if !self.is_dependent_ctx(class)
                    && !self.dtor_name_matches(class, n, Some(class), sloc)
                {
                    return None;
                }
                self.rebuild_fn_type(ty, QualType::VOID, Some(&[]), true)
            }
            SpecialName::Conversion(t) => self.rebuild_fn_type(ty, t, None, false),
            _ => return None,
        };
        let at = d.name_loc();
        let constexpr = if ds.consteval.is_some() {
            ConstexprKind::Consteval
        } else if ds.constexpr.is_some() {
            ConstexprKind::Constexpr
        } else {
            ConstexprKind::None
        };
        let kind = match sp {
            SpecialName::Constructor(_) => SpecialFriendKind::Ctor,
            SpecialName::Destructor(_) => SpecialFriendKind::Dtor,
            _ => SpecialFriendKind::Conversion,
        };
        // A class of a template (`Z<T>::` outside `Z` names `Z`'s pattern
        // when the parameters line up) is matched in each specialization.
        let templated_class =
            self.kind_of(class) == DeclKind::CXXRecord && self.in_dependent_class(class);
        let concrete = !templated_class && !self.is_dependent_ctx(class);
        if let Some(h) = head
            && concrete
        {
            // [class.friend]/4: a friend template naming a member template
            // that does not exist (clang `err_qualified_friend_no_match`,
            // without notes). Not in a templated class, whose friend's
            // parameters are one level deeper than the member's: clang
            // matches those by depth (rejecting a valid conversion
            // function template), kiln befriends nothing.
            let t = self.match_special_template(class, kind, ty, h);
            if self.in_dependent_class(record) {
                return Some(self.befriend_matched_template(record, t, ty, at));
            }
            let Some(t) = t else {
                let def = self.ast.definition(class).unwrap_or(class);
                let name = self.special_friend_name(def, kind, ty);
                let fname = self.decl_name_str(name);
                let msg = self
                    .diag(DiagId::sema_friend_def_no_match, at)
                    .arg(fname)
                    .arg(self.qualifier_desc(def));
                self.emit(msg);
                return None;
            };
            // [class.friend]/9: it must be accessible.
            if self.inst.stack.is_empty()
                && !self.ast.decls.get(t).has(kiln_ast::decl::DeclFlags::INVALID)
            {
                self.check_friend_access_noted(t, at, Some(at));
            }
            return Some(self.befriend_matched_template(record, Some(t), ty, at));
        }
        if head.is_some() || !concrete || self.ast.types.is_dependent(ty) {
            // A member of the current instantiation (`friend M::M(T);`
            // for a member class `M`) also befriends the pattern's member,
            // for the access checks in the template definition.
            let pattern = if self.kind_of(class) != DeclKind::CXXRecord {
                None
            } else if let Some(h) = head {
                self.match_special_template(class, kind, ty, h)
            } else {
                self.match_special_member(class, kind, ty, at, false).ok()
            };
            let target =
                pattern.map_or(FriendTarget::Type { ty, owned_tag: None }, FriendTarget::Decl);
            let fd = self.builder().friend(record, at, target);
            if let Some(p) = pattern {
                let first = self.ast.first(p);
                self.push_friend(first);
            }
            let nns = d.qualifier_nns;
            self.special_friends.insert(fd, SpecialFriend { kind, ty, class, nns, head });
            return Some(fd);
        }
        self.befriend_special_member(record, class, (kind, Some(constexpr)), ty, at)
    }

    /// The special member `kind` of `class` whose type is `ty` befriended
    /// by `record` at `at`: a non-template member first, else the
    /// specialization of a member template deduced from the type
    /// ([temp.friend]/1.2, [temp.deduct.decl]); no match is "friend
    /// declaration of 'A' does not match any declaration in 'A'", with
    /// clang's candidate notes. The befriended member must be accessible
    /// ([class.friend]/9), and the friend is a redeclaration: its
    /// exception specification ([except.spec]/5) and then its `constexpr`
    /// or `consteval` specifier `cx` ([dcl.constexpr]/1) match the
    /// member's, an implicitly-declared one's included (clang
    /// `MergeFunctionDecl`). `cx` is None per specialization, where clang
    /// compares a dependent friend with the member inconsistently. Also each specialization's match of a
    /// [`SpecialFriend`] (`templ/subst/decls.rs` `friend_decl`).
    pub(crate) fn befriend_special_member(
        &mut self,
        record: DeclId,
        class: DeclId,
        (kind, cx): (SpecialFriendKind, Option<ConstexprKind>),
        ty: QualType,
        at: Loc,
    ) -> Option<DeclId> {
        let (found, name) = match self.match_special_member(class, kind, ty, at, true) {
            Ok(target) => {
                // [class.friend]/9: the member itself, not a redeclaration,
                // so checked in a class template's definition too (clang);
                // a deduced specialization is noted at the friend.
                if self.inst.stack.is_empty()
                    && !self.ast.decls.get(target).has(kiln_ast::decl::DeclFlags::INVALID)
                    && self.declaring_class(target).is_some_and(|c| !self.in_dependent_class(c))
                {
                    let note_at = self.ast.spec_info(target).is_some().then_some(at);
                    self.check_friend_access_noted(target, at, note_at);
                }
                let missing_exc = self.check_friend_exc_spec(target, kind, ty, at);
                if let Some(cx) = cx
                    && !missing_exc
                    && !self.ast.decls.get(target).has(kiln_ast::decl::DeclFlags::INVALID)
                {
                    self.check_friend_constexpr_redecl(target, cx, at);
                }
                let fd = self.builder().friend(record, at, FriendTarget::Decl(target));
                let first = self.ast.first(target);
                self.push_friend(first);
                return Some(fd);
            }
            Err(x) => x,
        };
        let def = self.ast.definition(class).unwrap_or(class);
        let fname = self.decl_name_str(name);
        let msg = self
            .diag(DiagId::sema_friend_def_no_match, at)
            .arg(fname)
            .arg(self.qualifier_desc(def));
        let msg = self.with_friend_candidate_notes(msg, &found, ty);
        self.emit(msg);
        None
    }

    /// [except.spec]/5: a friend declaration `ty` of constructor,
    /// destructor or conversion function `target` redeclares it, so its
    /// exception specification is checked as an out-of-line
    /// redeclaration's ([`Sema::check_special_redecl_exc`], clang
    /// `CheckEquivalentExceptionSpec` through `MergeFunctionDecl`): one
    /// written without a specification is potentially throwing (a
    /// destructor's is the implicit one, [except.spec]/8), a written one
    /// must match, and `-Wimplicit-exception-spec-mismatch` notes an
    /// implicit specification on one side only. Whether the "missing
    /// exception specification" error was given, after which clang does
    /// not check `constexpr` agreement (it does after a mismatch).
    fn check_friend_exc_spec(
        &mut self,
        target: DeclId,
        kind: SpecialFriendKind,
        ty: QualType,
        at: Loc,
    ) -> bool {
        if self.ast.decls.get(target).has(kiln_ast::decl::DeclFlags::INVALID) {
            return false;
        }
        self.resolve_exc_for_type(target, at);
        let Some(pty) = self.declared_fn_type(target) else { return false };
        let name = self.decl_name(target);
        let missing = kind != SpecialFriendKind::Dtor
            && self.written_exc_spec(ty) == kiln_ast::types::ExcSpec::None;
        let error = self
            .check_special_redecl_exc(
                (target, pty, ty),
                kind == SpecialFriendKind::Dtor,
                (at, &name, None),
            )
            .1;
        missing && error
    }

    /// [dcl.constexpr]/1: a friend redeclaring special member `target`
    /// with the `constexpr` or `consteval` specifier `cx` agrees with it;
    /// an implicitly-declared or defaulted member that is constexpr
    /// counts as declared so (clang `err_constexpr_redecl_mismatch`; GCC
    /// does not check friends).
    fn check_friend_constexpr_redecl(&mut self, target: DeclId, cx: ConstexprKind, at: Loc) {
        let ok = self.fn_spec(target).constexpr_kind();
        if ok == cx {
            return;
        }
        let name = self.decl_name(target);
        let msg = self
            .diag(DiagId::sema_constexpr_redecl_mismatch, at)
            .arg(cx as u8)
            .arg(name)
            .arg(ok as u8);
        let msg = self.note_prev(msg, target, false);
        self.emit(msg);
    }

    /// `template<class U> friend Z::Z(U);` in `record`: befriends the
    /// member template of `class` of kind `kind` whose head is
    /// equivalent to `head` and whose type is `ty` ([temp.friend]/1),
    /// so every specialization of it. None matching befriends nothing.
    /// Also each specialization's match of a [`SpecialFriend`] with a
    /// head.
    pub(crate) fn befriend_special_template(
        &mut self,
        record: DeclId,
        class: DeclId,
        kind: SpecialFriendKind,
        ty: QualType,
        head: TemplateParamListId,
        at: Loc,
    ) -> DeclId {
        let t = self.match_special_template(class, kind, ty, head);
        self.befriend_matched_template(record, t, ty, at)
    }

    /// The friend of [`Sema::befriend_special_template`] once matched.
    fn befriend_matched_template(
        &mut self,
        record: DeclId,
        t: Option<DeclId>,
        ty: QualType,
        at: Loc,
    ) -> DeclId {
        let Some(t) = t else {
            return self.builder().friend(record, at, FriendTarget::Type { ty, owned_tag: None });
        };
        let fd = self.builder().friend(record, at, FriendTarget::Decl(t));
        let first = self.ast.first(t);
        self.push_friend(first);
        fd
    }

    /// The name a special member friend of kind `kind` and type `ty` of
    /// class `def` declares.
    fn special_friend_name(
        &mut self,
        def: DeclId,
        kind: SpecialFriendKind,
        ty: QualType,
    ) -> DeclName {
        let canon = self.ast.types.canonical(self.class_type(def)).ty();
        match kind {
            SpecialFriendKind::Ctor => self.ast.names.constructor(canon),
            SpecialFriendKind::Dtor => self.ast.names.destructor(canon),
            SpecialFriendKind::Conversion => {
                let ret = self
                    .ast
                    .types
                    .proto(ty)
                    .map_or(QualType::ERROR, kiln_ast::types::FuncProto::ret);
                self.ast.names.conversion(self.ast.types.canonical(ret))
            }
        }
    }

    /// The member template of `class` of kind `kind` with a head
    /// equivalent to `head` and type `ty` ([temp.over.link]/6).
    fn match_special_template(
        &mut self,
        class: DeclId,
        kind: SpecialFriendKind,
        ty: QualType,
        head: TemplateParamListId,
    ) -> Option<DeclId> {
        let def = self.ast.definition(class).unwrap_or(class);
        let canon = self.ast.types.canonical(self.class_type(def)).ty();
        let candidates = match kind {
            SpecialFriendKind::Conversion => self.conversion_templates_of(def),
            SpecialFriendKind::Ctor | SpecialFriendKind::Dtor => {
                let name = if kind == SpecialFriendKind::Ctor {
                    self.ast.names.constructor(canon)
                } else {
                    self.ast.names.destructor(canon)
                };
                let found: Vec<DeclId> = self.ast.lookup_local(def, name).iter().collect();
                self.function_templates_in(&found)
            }
        };
        candidates.into_iter().find(|&t| {
            let own = self.ast.template_info(t).map(|i| i.params);
            let pty = self.pattern_of(t).and_then(|p| self.ast.decls.get(p).ty());
            match (own, pty) {
                (Some(own), Some(pty)) => {
                    self.heads_equivalent(own, head) && self.same_ignoring_exc_spec(pty, ty)
                }
                _ => false,
            }
        })
    }

    /// The special member `kind` of `class` of type `ty`
    /// ([`Sema::befriend_special_member`]), else what lookup found and the
    /// name looked up. `implicit`: declare the implicit members first
    /// (not in a template's pattern).
    fn match_special_member(
        &mut self,
        class: DeclId,
        kind: SpecialFriendKind,
        ty: QualType,
        at: Loc,
        implicit: bool,
    ) -> Result<DeclId, (Vec<DeclId>, DeclName)> {
        let def = self.ast.definition(class).unwrap_or(class);
        let canon = self.ast.types.canonical(self.class_type(def)).ty();
        let name = match kind {
            SpecialFriendKind::Ctor => {
                if implicit {
                    for k in [
                        crate::class::special::SpecialKind::DefaultCtor,
                        crate::class::special::SpecialKind::CopyCtor,
                        crate::class::special::SpecialKind::MoveCtor,
                    ] {
                        self.ensure_implicit(def, k);
                    }
                }
                self.ast.names.constructor(canon)
            }
            SpecialFriendKind::Dtor => {
                if implicit {
                    self.ensure_implicit(def, crate::class::special::SpecialKind::Dtor);
                }
                self.ast.names.destructor(canon)
            }
            SpecialFriendKind::Conversion => {
                let ret = self
                    .ast
                    .types
                    .proto(ty)
                    .map_or(QualType::ERROR, kiln_ast::types::FuncProto::ret);
                let c = self.ast.types.canonical(ret);
                self.ast.names.conversion(c)
            }
        };
        let found: Vec<DeclId> = self.ast.lookup_local(def, name).iter().collect();
        let mut target = found.iter().copied().find(|&c| {
            self.ast.described_template(c).is_none()
                && self.kind_of(c) != DeclKind::FunctionTemplate
                && self
                    .ast
                    .decls
                    .get(c)
                    .ty()
                    .is_some_and(|cty| self.same_ignoring_exc_spec(cty, ty))
        });
        if target.is_none() {
            let candidates = match kind {
                SpecialFriendKind::Conversion => self.conversion_templates_of(def),
                _ => self.function_templates_in(&found),
            };
            if !candidates.is_empty() {
                target = self.match_function_spec_quiet(&candidates, name, ty, at).map(|m| m.spec);
            }
        }
        target.ok_or((found, name))
    }

    /// clang's notes on "friend declaration of 'X' does not match any
    /// declaration in 'C'" ([class.friend]/4): one per function `found`
    /// by the name in the class, why its type differs from `want`
    /// (`HandleFunctionTypeMismatch`), then one per function template
    /// ("candidate template ignored: failed template argument
    /// deduction").
    pub(crate) fn with_friend_candidate_notes(
        &self,
        mut msg: kiln_diag::Diag,
        found: &[DeclId],
        want: QualType,
    ) -> kiln_diag::Diag {
        use crate::class::special::SpecialKind;
        let mut templates = Vec::new();
        for &c in found {
            // Not an invalid declaration, nor a friend redeclaration.
            let decl = self.ast.decls.get(c);
            if decl.has(kiln_ast::decl::DeclFlags::INVALID)
                || self.ast.decls.lexical_ctx(c).is_some_and(|l| Some(l) != decl.ctx())
            {
                continue;
            }
            let kind = self.kind_of(c);
            if kind == DeclKind::FunctionTemplate {
                templates.push(self.templated_function(c).unwrap_or(c));
                continue;
            }
            if !kind.is_function() {
                continue;
            }
            if self.ast.described_template(c).is_some() {
                templates.push(c);
                continue;
            }
            let implicit = self.ast.decls.get(c).has(kiln_ast::decl::DeclFlags::IMPLICIT);
            let what = match (kind, implicit) {
                (DeclKind::CXXConstructor, true) => match self.special_kind_of(c) {
                    Some(SpecialKind::DefaultCtor) => {
                        "constructor (the implicit default constructor)"
                    }
                    Some(SpecialKind::MoveCtor) => "constructor (the implicit move constructor)",
                    _ => "constructor (the implicit copy constructor)",
                },
                (DeclKind::CXXConstructor, false) => "constructor",
                _ => "function",
            };
            msg = msg.with_note(self.mismatch_note_as(c, want, what));
        }
        for f in templates {
            let at = self.ast.decls.get(f).loc();
            msg = msg.with_note(self.diag(DiagId::note_ovl_candidate_bad_deduction, at));
        }
        msg
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
        // A friend class declaration is a free-standing one first (clang's
        // `ParsedFreeStandingDeclSpec` before `ActOnFriendTypeDecl`).
        if let Some(t) = ds.tag.or(owned) {
            if !self.free_standing_tag_specifiers(ds, t) {
                self.friend_class_template = None;
                return None;
            }
        } else if let Some(l) = ds.inline {
            self.inline_non_function(l);
        }
        // [class.friend]/3: a friend type declaration is `friend` followed
        // by the type specifier (clang `err_friend_not_first_in_declaration`,
        // C++11 on).
        if ds.friend_not_first && self.std >= kiln_lex::Std::Cxx11 {
            self.error(DiagId::sema_friend_not_first, ds.friend.unwrap_or(friend_loc));
        }
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
        // [temp.friend]/1: a friend class template is declared by an
        // elaborated-type-specifier (`template<class U> friend S<U>;` and
        // `template<int U> friend typename S<U>::Y;` are clang
        // `err_tagless_friend_type_template` at `friend`; nothing declared).
        if self.has_pending_head()
            && owned.is_none()
            && ds.tag.is_none()
            && ds.dependent_tag_name.is_none()
            && !self.is_error_type(base)
        {
            self.take_open_heads();
            self.error(DiagId::sema_tagless_friend_type_template, ds.loc);
            return None;
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
        self.check_cxx98_friend_type(ds, base, owned);
        let loc = ds.type_loc.unwrap_or(friend_loc);
        // `template<class T> friend class A<T>::Member;` ([temp.friend]/5).
        if owned.is_none()
            && let Some(fd) = self.declare_member_friend_class(ds, base, record, loc)
        {
            return Some(fd);
        }
        if owned.is_none() && self.ignore_dependent_friend_class_template(ds, base, loc) {
            return None;
        }
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

    /// C++11 [class.friend]/3 allows a friend type declaration without a
    /// class-key; before it clang's `ext_unelaborated_friend_type` (a
    /// class) or `ext_nonclass_type_friend` (anything else) at the start of
    /// the declaration. Checked in the pattern only.
    fn check_cxx98_friend_type(&mut self, ds: &DeclSpec, base: QualType, owned: Option<DeclId>) {
        if self.std >= kiln_lex::Std::Cxx11
            || owned.is_some()
            || ds.tag.is_some()
            || ds.dependent_tag_name.is_some()
            || matches!(
                ds.keyword,
                TagKeyword::Struct | TagKeyword::Class | TagKeyword::Union | TagKeyword::Enum
            )
            || self.inst.stack.in_instantiation_record()
            || self.is_error_type(base)
            || matches!(
                self.ast.types.view(base.ty()),
                TypeView::Elaborated { keyword, .. }
                    if !matches!(keyword, TagKeyword::None | TagKeyword::Typename)
            )
        {
            return;
        }
        let d = if let Some(r) = self.ast.types.as_record(base) {
            let key = self.tag_keyword(r);
            self.diag(DiagId::sema_ext_unelaborated_friend_type, ds.loc).arg(key)
        } else {
            self.diag(DiagId::sema_ext_nonclass_type_friend, ds.loc)
        };
        let d = d.arg(self.ty_arg(base));
        self.emit(d);
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
            // A specialization that depends on nothing of the template is
            // matched where written ([temp.res.general]/6.4; clang
            // `CheckFunctionTemplateSpecialization` at the definition):
            // when no template matches it is an error now, and the
            // friend is dropped (`friend void A::f<0>(int);`).
            let matched = if self.friend_spec_dependent(d, ty, &sid.args) {
                None
            } else {
                let explicit = Some(self.ast.types.arg_list(&sid.args));
                Some(self.match_function_spec_in(&templates, explicit, name, ty, false, loc)?)
            };
            let target = FriendTarget::Type { ty, owned_tag: None };
            let fd = self.builder().friend(record, loc, target);
            // Its written name: the template matched, else the
            // candidates, decided per specialization (spec/ast.md §18.4).
            if let Some(id) = d.id_ref {
                match matched {
                    Some(m) => self.note_written_spec_template(id, m.template),
                    None => self.note_written_friend_spec(id, fd, &templates, d.qualifier),
                }
            }
            let args = sid.args.clone();
            let late_exc = d.late_noexcept.is_some();
            self.friend_specs.insert(fd, FriendSpec { name, ty, args, templates, loc, late_exc });
            return Some(fd);
        }
        let explicit = Some(self.ast.types.arg_list(&sid.args));
        let m = self.match_function_spec_in(&templates, explicit, name, ty, false, loc)?;
        if let Some(id) = d.id_ref {
            self.note_written_spec_template(id, m.template);
        }
        // A noexcept-specifier delayed to the end of the class is not
        // compared (clang's delayed equivalence check is a follow-up).
        let fix_at = crate::except::missing_exc_spec_fixit_loc(d);
        if d.late_noexcept.is_none() && !self.check_spec_exc_spec(m.spec, ty, loc, fix_at) {
            return None;
        }
        // [class.friend]/9: the specialization befriended must be
        // accessible (clang `CheckFriendAccess`).
        if let Some(class) = d.qualifier
            && self.kind_of(class) == DeclKind::CXXRecord
        {
            self.check_friend_access_noted(m.spec, loc, Some(loc));
        }
        Some(self.befriend_spec(record, m.spec, loc))
    }

    /// Whether the friend function template specialization `d` (of type
    /// `ty`, explicit arguments `args`) depends on a template parameter:
    /// its qualifier, type or explicit arguments.
    fn friend_spec_dependent(&self, d: &Declarator, ty: QualType, args: &[TemplateArg]) -> bool {
        use kiln_ast::types::{ArgView, TypeFlags};
        let t = &self.ast.types;
        let dep = TypeFlags::DEPENDENT | TypeFlags::UNEXPANDED_PACK;
        d.qualifier_nns.is_some_and(|n| t.nns_flags(n).intersects(dep))
            || t.qflags(ty).intersects(dep)
            || args.iter().any(|&a| match a.view() {
                ArgView::Expr { expr, .. } => {
                    let n = self.ast.exprs.get(expr);
                    n.is_value_dependent() || n.is_type_dependent()
                }
                _ => t.arg_flags(a).intersects(dep),
            })
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

    /// `friend void A::f(int);` where `A::f` declares no non-template
    /// member of that type but a member function template whose
    /// specialization the type deduces ([temp.friend]/1.2,
    /// [temp.deduct.decl]): only that specialization becomes the friend.
    /// A matching non-template member wins (the ordinary path, `None`),
    /// as does a dependent class or type (matched per instantiation) and
    /// a template-id (`declare_friend_spec`). No match: `None`, and the
    /// ordinary path reports that the friend matches nothing.
    fn declare_member_template_spec_friend(
        &mut self,
        record: DeclId,
        d: &Declarator,
        ty: QualType,
        name: DeclName,
        class: DeclId,
    ) -> Option<DeclId> {
        if d.spec_id.is_some()
            || self.kind_of(class) != DeclKind::CXXRecord
            || self.is_dependent_ctx(class)
            || self.ast.types.is_dependent(ty)
            || !self.ast.types.is_function(ty)
        {
            return None;
        }
        let def = self.ast.definition(class)?;
        let found: Vec<DeclId> = self.ast.lookup_local(def, name).iter().collect();
        let templates = self.function_templates_in(&found);
        if templates.is_empty()
            || found.iter().any(|&c| {
                self.kind_of(c) != DeclKind::FunctionTemplate
                    && self.ast.described_template(c).is_none()
                    && self
                        .declared_fn_type(c)
                        .is_some_and(|cty| self.same_ignoring_exc_spec(cty, ty))
            })
        {
            return None;
        }
        let loc = d.name_loc();
        let spec = self.match_function_spec_quiet(&templates, name, ty, loc)?.spec;
        if self.inst.stack.is_empty() && !self.in_dependent_class(def) {
            self.check_friend_access_noted(spec, loc, Some(loc));
        }
        self.templ.cur = None;
        Some(self.befriend_spec(record, spec, loc))
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
