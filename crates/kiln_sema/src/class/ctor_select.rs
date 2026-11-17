//! Constructor selection for a call with arbitrary arguments
//! ([over.match.ctor], [over.match.list]; spec/sema-overload.md §10.3):
//! a candidate set of the class's constructors resolved by the overload
//! engine. Function templates (WP-071) are skipped; a selection that a
//! skipped template or an unchecked *requires-clause* (WP-073) could
//! change answers [`Selected::Unsupported`].

use kiln_ast::decl::DeclKind;
use kiln_ast::names::DeclName;
use kiln_ast::types::TypeView;
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_source::Loc;

use super::special::Selected;
use crate::Sema;
use crate::overload::{IcsOptions, OverloadCandidateSet, OvlOutcome, SetKind};

/// The form of an initialization by constructor (clang's
/// `InitializationKind`), which decides the candidates
/// ([over.match.ctor]/1, [over.match.list]/1) and whether an argument
/// may need a user-defined conversion ([over.best.ics]/4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum InitStyle {
    /// `T x = e;`, argument passing, `return e;`: converting
    /// constructors only, arguments without user-defined conversions.
    Copy,
    /// `T x(a, ...)`, `T(a, ...)`, `static_cast<T>(e)`.
    Direct,
    /// `T x = {a, ...}`: an `explicit` constructor may be chosen but
    /// makes the initialization ill-formed ([over.match.list]/1).
    CopyList,
    /// `T x{a, ...}`, `T{a, ...}`.
    DirectList,
}

impl InitStyle {
    pub(crate) fn is_list(self) -> bool {
        matches!(self, InitStyle::CopyList | InitStyle::DirectList)
    }
}

impl Sema<'_, '_> {
    /// Selects the constructor of complete class `class` for `args` in an
    /// initialization of form `style` ([over.match.ctor]; for a list
    /// form, [over.match.list] phase 2 over the list's elements).
    /// `Missing` when no constructor is viable; a failed set is kept in
    /// `ovl.failed` for [`Sema::ctor_failure_notes`].
    pub(crate) fn resolve_ctor(
        &mut self,
        class: DeclId,
        args: &[ExprId],
        style: InitStyle,
    ) -> Selected {
        self.resolve_ctor_ex(class, args, style, false)
    }

    /// [`Sema::resolve_ctor`]; `actual_ctor` drops the CWG2327
    /// conversion-function candidates, as for a base subobject or a
    /// delegating constructor's target (clang `RequireActualConstructor`).
    pub(crate) fn resolve_ctor_ex(
        &mut self,
        class: DeclId,
        args: &[ExprId],
        style: InitStyle,
        actual_ctor: bool,
    ) -> Selected {
        self.resolve_ctor_opts(class, args, style, actual_ctor, false)
    }

    /// [`Sema::resolve_ctor_ex`]; `no_udc` gives the argument no
    /// user-defined conversion, for the copy of the temporary in the
    /// second step of a C++11/14 copy-initialization by user-defined
    /// conversion ([over.best.ics]/4 there: "13.3.1.3 when the argument is
    /// the temporary in the second step of a class copy-initialization";
    /// clang's `SuppressUserConversions` for the final copy).
    pub(crate) fn resolve_ctor_opts(
        &mut self,
        class: DeclId,
        args: &[ExprId],
        style: InitStyle,
        actual_ctor: bool,
        no_udc: bool,
    ) -> Selected {
        let (set, out) = self.ctor_overload(class, args, style, actual_ctor, no_udc);
        self.selected_of(set, out)
    }

    /// The resolved candidate set of [`Sema::resolve_ctor_opts`], kept for
    /// its notes (the caller ends it).
    pub(crate) fn ctor_overload(
        &mut self,
        class: DeclId,
        args: &[ExprId],
        style: InitStyle,
        actual_ctor: bool,
        no_udc: bool,
    ) -> (OverloadCandidateSet, OvlOutcome) {
        let c = self.class_def(class);
        let loc = args.first().map_or(Loc::INVALID, |&a| self.begin_loc(a));
        // [over.match.copy]: converting constructors only, the argument
        // without user-defined conversions ([over.best.ics]/4.2). A
        // copy-initialization from the class itself or a derived class is
        // [over.match.ctor] (/16.6.2.1), whose argument may need one
        // (clang `TryConstructorInitialization`: `A a = b;` through
        // `A(X)` and `B::operator X()`); its explicit constructors are
        // still not viable.
        let (kind, opts, by_ctor) = if style == InitStyle::Copy {
            let same_or_derived = match args {
                [a] => self.ast.types.as_record(self.ety(*a)).is_some_and(|r| {
                    crate::conv::bases::same_class(self, r, c)
                        || crate::conv::bases::complete_class(self, r)
                            .is_some_and(|r| crate::conv::bases::is_derived_from(self, r, c))
                }),
                _ => false,
            };
            let opts = if same_or_derived && !no_udc {
                IcsOptions::NONE
            } else {
                IcsOptions::SUPPRESS_UDC
            };
            (SetKind::CopyInit, opts, same_or_derived)
        } else if no_udc {
            (SetKind::Normal, IcsOptions::SUPPRESS_UDC, true)
        } else {
            (SetKind::Normal, IcsOptions::NONE, true)
        };
        let mut set = self.begin_set(kind, loc);
        // An initialization by constructor (clang `CSK_InitByConstructor`;
        // not one by user-defined conversion) defers its constructor
        // templates (clang 21+).
        if by_ctor {
            self.defer_templates(&mut set);
        }
        for &a in args {
            let info = self.ctor_arg_info(a);
            set.args.push(info);
        }
        // [over.match.copy]/1.1: from a non-class expression, converting
        // constructors only (clang `TryUserDefinedConversion` keeps
        // `isConvertingConstructor` ones, so a default constructor is
        // not a noted candidate).
        if let [arg] = args
            && style == InitStyle::Copy
            && self.ast.types.as_record(self.ety(*arg)).is_none()
        {
            self.add_converting_ctor_candidates(&mut set, c, opts);
        } else {
            // [over.match.copy]/1.2, [over.match.ref]/1.1: one argument
            // of a direct-initialization may reach a copy or move
            // constructor through an explicit conversion function.
            let opts =
                if args.len() == 1 && matches!(style, InitStyle::Direct | InitStyle::DirectList) {
                    opts.with(IcsOptions::DIRECT_COPY_CTOR)
                } else {
                    opts
                };
            // [over.best.ics.general]/4.5: a list whose only element is a
            // braced-init-list reaches no copy or move constructor through
            // a user-defined conversion (clang `ResolveConstructorOverload`
            // `SuppressUserConversions`).
            let opts = match args {
                [a] if style.is_list() && self.braced_args.iter().any(|(x, _)| x == a) => {
                    opts.with(IcsOptions::LIST_COPY_CTOR)
                }
                _ => opts,
            };
            // A copy-initialization from the class itself or a derived
            // class deduces no non-dependently explicit constructor
            // template ([over.match.ctor]/1: converting constructors only;
            // clang `AddTemplateOverloadCandidate` without `AllowExplicit`).
            self.add_ctor_candidates_ex(&mut set, c, opts, false, style == InitStyle::Copy);
        }
        // C++17 only, as clang (`ResolveConstructorOverload`'s
        // `CPlusPlus17`): before it, the copy or move constructor is
        // called on the converted value.
        if let [arg] = args
            && !actual_ctor
            && self.std >= kiln_lex::Std::Cxx17
        {
            self.add_elision_candidates(&mut set, c, *arg, style);
        }
        let out = self.resolve_set(&mut set);
        let out = self.elision_against_ctors(&mut set, out, c);
        (set, out)
    }

    /// [over.match.ctor]/1 makes only the constructors candidates
    /// (CWG2327 is open): a winning conversion-function candidate of
    /// [`Self::add_elision_candidates`] does not stand when the
    /// constructors alone are ambiguous between copy and move
    /// constructors only, unless its implicit object conversion is better
    /// than that of every other conversion function they are reached
    /// through. An ambiguity involving another constructor (`P(string&&)`
    /// beside `P(P&&)`, both through a conversion function template)
    /// leaves the conversion function winning, as in GCC 16 and clang 22,
    /// although the text makes it ambiguous too. A copy and a move
    /// constructor reached through two different conversion functions
    /// are indistinguishable ([over.ics.rank]/3.3), and a third
    /// conversion function no better than those cannot resolve that (GCC
    /// 16; clang 22 lets the conversion function win). When the
    /// constructors alone select one, the conversion function still wins
    /// as in clang (libc++ tuple tests rely on it; GCC 16 picks the
    /// constructor there).
    fn elision_against_ctors(
        &mut self,
        set: &mut OverloadCandidateSet,
        out: OvlOutcome,
        class: DeclId,
    ) -> OvlOutcome {
        use crate::conv::IcsCompare;
        use crate::overload::{CandidateKind, Ics};
        let (OvlOutcome::Success(b) | OvlOutcome::Deleted(b)) = out else { return out };
        if set.cand(b).kind != CandidateKind::ConvFn {
            return out;
        }
        let Some(Ics::Standard(fobj)) = set.ics_of(b).first().copied() else { return out };
        let f = set.cand(b).func();
        let hidden: Vec<usize> = (0..set.cands.len())
            .filter(|&i| set.cands[i].viable && set.cands[i].kind == CandidateKind::ConvFn)
            .collect();
        for &i in &hidden {
            set.cands[i].viable = false;
        }
        let marks: Vec<bool> = set.cands.iter().map(|c| c.ambiguous_mark).collect();
        for c in &mut set.cands {
            c.ambiguous_mark = false;
        }
        let r = self.best_viable(set);
        let marked: Vec<u32> = (0..set.cands.len())
            .filter(|&i| set.cands[i].ambiguous_mark)
            .map(|i| u32::try_from(i).unwrap_or(u32::MAX))
            .collect();
        let ambiguous = r == OvlOutcome::Ambiguous
            && marked.iter().all(|&c| self.copy_or_move_ctor_param(set.cand(c).func(), class))
            && marked.iter().any(|&c| match set.ics_of(c).first() {
                Some(Ics::UserDefined(u)) if u.func == Some(f) => false,
                Some(Ics::UserDefined(u)) => {
                    self.compare_ics(&Ics::Standard(fobj), &Ics::Standard(u.before))
                        != IcsCompare::Better
                }
                _ => false,
            })
            && !marked.iter().any(|&c| match set.ics_of(c).first().copied() {
                Some(Ics::UserDefined(u)) if u.func != Some(f) => {
                    !self.is_deleted_fn(set.cand(c).func())
                        && u.func
                            .is_some_and(|g| self.lvalue_conversion_best(set, g, u.before, class))
                }
                _ => false,
            });
        for &i in &hidden {
            set.cands[i].viable = true;
        }
        if ambiguous {
            return r;
        }
        for (c, m) in set.cands.iter_mut().zip(marks) {
            c.ambiguous_mark = m;
        }
        out
    }

    /// Whether conversion function `g`, through which a copy constructor
    /// of `class` is reached, yields an lvalue reference and its implicit
    /// object conversion `gobj` is better than that of every other
    /// conversion function of the argument's class yielding `class` or a
    /// class derived from it, by value or by reference. GCC 16 then binds
    /// the move constructor's `T&&` through `g` too (it does not set the
    /// lvalue-yielding functions aside first, as [over.match.ref]/1.1
    /// does), so the move constructor is not viable and the copy
    /// constructor alone is selected: clang 22 lets the conversion
    /// function yielding the prvalue win, and kiln follows clang where
    /// both accept (WP-1592; docs/status/deviations.md). The caller
    /// requires that copy constructor not to be deleted (GCC selects it
    /// and rejects). A rival yielding a derived class by value or by
    /// lvalue reference with an object conversion as good loses the tie
    /// when `g` yields the class itself; a conversion function template
    /// with an object conversion no better than `gobj` loses to the
    /// non-template `g`, one with a better one leaves the answer open:
    /// not best.
    fn lvalue_conversion_best(
        &mut self,
        set: &OverloadCandidateSet,
        g: DeclId,
        gobj: crate::conv::StandardConversion,
        class: DeclId,
    ) -> bool {
        use crate::conv::IcsCompare;
        use crate::overload::Ics;
        let ret = self.conversion_result_type(g);
        if !matches!(self.ast.types.canonical_view(ret), TypeView::LValueRef(_)) {
            return false;
        }
        let Some(&a) = set.args.first() else { return false };
        let target = self.class_def(class);
        let class_of = |s: &mut Self, t: QualType| {
            let t = s.non_reference(t);
            s.ast.types.as_record(t).map(|c| s.class_def(c))
        };
        let g_exact = class_of(self, ret) == Some(target);
        for (_, h) in self.conversion_functions_all(a.ty) {
            if h == g {
                continue;
            }
            let hret = match self.kind_of(h) {
                DeclKind::CXXConversion => self.conversion_result_type(h),
                // A template that may yield the class leaves it open; one
                // yielding a pointer, say, is no rival.
                DeclKind::FunctionTemplate => {
                    let Some(p) = self.templated_function(h) else { return false };
                    let t = self.non_reference(self.conversion_result_type(p));
                    let never_class = matches!(
                        self.ast.types.canonical_view(t),
                        TypeView::Pointer(_)
                            | TypeView::MemberPointer { .. }
                            | TypeView::Builtin(_)
                    );
                    if never_class {
                        continue;
                    }
                    // One whose object conversion is no better than
                    // `g`'s loses to the non-template `g`
                    // ([over.match.best]/2.4) in GCC's binding of `T&&`
                    // too (WP-1625; clang accepts through its CWG2327
                    // candidates).
                    if self.has_explicit_object(p) {
                        return false;
                    }
                    match self.conv_object_ics(&a, p) {
                        Ics::Standard(hobj) => {
                            if self.compare_ics(&Ics::Standard(gobj), &Ics::Standard(hobj))
                                == IcsCompare::Worse
                            {
                                return false;
                            }
                        }
                        ics if ics.is_bad() => {}
                        _ => return false,
                    }
                    continue;
                }
                _ => return false,
            };
            let Some(rc) = class_of(self, hret) else { continue };
            if rc != target && !crate::conv::bases::is_derived_from(self, rc, target) {
                continue;
            }
            match self.conv_object_ics(&a, h) {
                Ics::Standard(hobj) => {
                    match self.compare_ics(&Ics::Standard(gobj), &Ics::Standard(hobj)) {
                        IcsCompare::Better => {}
                        // GCC's tie-break on the result: an lvalue of the
                        // class binds better than an lvalue or a prvalue
                        // of a derived class (WP-1625: clang accepts too,
                        // through its CWG2327 candidates).
                        IcsCompare::Indistinguishable
                            if g_exact
                                && rc != target
                                && !matches!(
                                    self.ast.types.canonical_view(hret),
                                    TypeView::RValueRef(_)
                                ) => {}
                        _ => return false,
                    }
                }
                ics if ics.is_bad() => {}
                _ => return false,
            }
        }
        true
    }

    /// The converting constructors of class `c` ([class.conv.ctor]; clang
    /// `isConvertingConstructor`: callable with one argument, or a
    /// variadic one without parameters), and its inherited ones.
    fn add_converting_ctor_candidates(
        &mut self,
        set: &mut OverloadCandidateSet,
        c: DeclId,
        opts: IcsOptions,
    ) {
        let mut found = self.converting_ctors(c, true);
        self.drop_explicit_ctor_templates(set, &mut found);
        self.add_found_candidates(set, &found, crate::overload::CandidateKind::Ctor, opts, false);
    }

    /// The constructors of class `c` with the `ConstructorUsingShadow`s
    /// of its inherited ones ([namespace.udecl]/13), declaring the
    /// implicit ones first; with `one_arg`, only those callable with one
    /// argument.
    pub(crate) fn converting_ctors(&mut self, c: DeclId, one_arg: bool) -> Vec<DeclId> {
        let c = self.class_def(c);
        self.ensure_implicit(c, crate::class::SpecialKind::DefaultCtor);
        self.ensure_implicit(c, crate::class::SpecialKind::CopyCtor);
        self.ensure_implicit(c, crate::class::SpecialKind::MoveCtor);
        let mut found: Vec<DeclId> = self.usable_ctors(c);
        found.extend(self.ctor_shadows(c));
        if !one_arg {
            return found;
        }
        let mut params = Vec::new();
        found.retain(|&fd| {
            // Through an inherited constructor's shadow, and to a
            // constructor template's pattern (clang checks the templated
            // declaration).
            let mut f = fd;
            if self.kind_of(f) == DeclKind::ConstructorUsingShadow {
                let Some(t) = self.ast.using_target(f) else { return true };
                f = t;
            }
            if self.kind_of(f) == DeclKind::FunctionTemplate {
                let Some(p) = self.pattern_of(f) else { return true };
                f = p;
            }
            if self.kind_of(f) != DeclKind::CXXConstructor {
                return true;
            }
            params.clear();
            let variadic = self.fn_param_types(f, &mut params);
            if params.is_empty() { variadic } else { self.fn_min_args(f) <= 1 }
        });
        found
    }

    /// clang's workaround for C++17 guaranteed copy elision (CWG2327),
    /// from C++17 on: when class `c` is initialized by constructor from one expression
    /// of class type, that class's conversion functions yielding the
    /// prvalue `cv T` are candidates too (explicit ones only in a
    /// direct-initialization), and one that wins initializes the object
    /// directly.
    fn add_elision_candidates(
        &mut self,
        set: &mut OverloadCandidateSet,
        c: DeclId,
        arg: ExprId,
        style: InitStyle,
    ) {
        let from = self.ety(arg);
        if self.ast.types.as_record(from).is_none()
            || !self.require_complete_silent(from, Loc::INVALID)
        {
            return;
        }
        let to = self.ast.types.canonical(self.class_type(c));
        let direct = matches!(style, InitStyle::Direct | InitStyle::DirectList);
        let opts = if direct { IcsOptions::ALLOW_EXPLICIT } else { IcsOptions::NONE };
        for (found, f) in self.conversion_functions_all(from) {
            if self.kind_of(f) == DeclKind::FunctionTemplate {
                // Deduced at once, and no perfect match is picked before
                // the constructor templates are deduced (clang 21+
                // `DisableResolutionByPerfectCandidate`).
                set.no_perfect = true;
                // clang `AddTemplateConversionCandidate` with
                // `AllowResultConversion` false: the specialization
                // deduced for `cv T` must yield exactly `cv T`.
                let explicit =
                    self.templated_function(f).is_some_and(|p| self.nondependently_explicit(p));
                if !direct && explicit {
                    continue;
                }
                let n = set.cands.len();
                self.add_conversion_template_candidate(set, f, found, to, opts);
                if let Some(c) = set.cands.get(n)
                    && c.viable
                {
                    let ret = self.conversion_result_type(c.func());
                    if self.ast.types.is_reference(ret) || !self.ast.types.same_unqualified(ret, to)
                    {
                        let start = c.ics.start as usize;
                        set.cands.truncate(n);
                        set.ics.truncate(start);
                    }
                }
                continue;
            }
            if self.kind_of(f) != DeclKind::CXXConversion
                || (!direct && self.fn_spec(f).is_explicit())
            {
                continue;
            }
            let ret = self.conversion_result_type(f);
            if self.ast.types.is_reference(ret) || !self.ast.types.same_unqualified(ret, to) {
                continue;
            }
            self.add_conversion_candidate(set, f, found, to, opts);
        }
    }

    /// The function candidate `b` of `set` calls: for an inherited
    /// constructor, the derived class's inheriting constructor
    /// ([namespace.udecl]/13, clang `findInheritingConstructor`).
    fn selected_ctor(&mut self, set: &OverloadCandidateSet, b: u32) -> DeclId {
        let c = set.cand(b);
        let f = c.func();
        if c.found != f && self.kind_of(c.found) == DeclKind::ConstructorUsingShadow {
            self.find_inheriting_ctor(c.found, f)
        } else {
            f
        }
    }

    /// Maps the outcome of a constructor or special-member set to
    /// [`Selected`], keeping a failed set for its notes.
    pub(crate) fn selected_of(&mut self, set: OverloadCandidateSet, out: OvlOutcome) -> Selected {
        let unsure = set.uncertain;
        let skipped = set.skipped_templates;
        let r = match out {
            _ if unsure => Selected::Unsupported,
            OvlOutcome::Success(b) | OvlOutcome::Deleted(b)
                if skipped && !self.template_cannot_win(&set, b) =>
            {
                Selected::Unsupported
            }
            OvlOutcome::Success(b) => {
                // The inheriting constructor of a usable base constructor
                // can itself be deleted ([class.inhctor.init]/1; clang
                // diagnoses it on use, `err_deleted_inherited_ctor_use`).
                let m = self.selected_ctor(&set, b);
                if self.is_deleted_fn(m) { Selected::Deleted(m) } else { Selected::Found(m) }
            }
            OvlOutcome::Deleted(b) => Selected::Deleted(self.selected_ctor(&set, b)),
            OvlOutcome::NoViable | OvlOutcome::Ambiguous if skipped => Selected::Unsupported,
            OvlOutcome::NoViable => Selected::Missing,
            OvlOutcome::Ambiguous => Selected::Ambiguous,
        };
        if matches!(r, Selected::Missing | Selected::Ambiguous) {
            if let Some(old) = self.ovl.failed.replace(set) {
                self.end_set(old);
            }
        } else {
            self.end_set(set);
        }
        r
    }

    /// Whether declaration `d` is invalid.
    pub(crate) fn is_invalid_decl(&self, d: DeclId) -> bool {
        self.ast.decls.get(d).has(kiln_ast::decl::DeclFlags::INVALID)
    }

    /// The constructors (and constructor templates) of class `c` that take
    /// part in overload resolution: an invalid one, such as one with a
    /// parameter naming no type, does not (clang's
    /// `ResolveConstructorOverload` and `LookupSpecialMember` skip an
    /// invalid constructor; WP-318).
    pub(crate) fn usable_ctors(&self, c: DeclId) -> Vec<DeclId> {
        self.ast
            .constructors(c)
            .iter()
            .filter(|&f| {
                !self.is_invalid_decl(f)
                    && (self.kind_of(f) != DeclKind::FunctionTemplate
                        || self.pattern_of(f).is_none_or(|p| !self.is_invalid_decl(p)))
            })
            .collect()
    }

    /// The constructors of class `c` and the base constructors its
    /// visible `ConstructorUsingShadow`s inherit ([namespace.udecl]/13:
    /// inherited constructors are candidates of [over.match.list] too).
    fn ctors_and_inherited(&self, c: DeclId) -> Vec<DeclId> {
        let mut out: Vec<DeclId> = self.usable_ctors(c);
        out.extend(self.ctor_shadows(c).into_iter().filter_map(|s| self.ast.using_target(s)));
        out
    }

    /// Whether some constructor of `class` is an initializer-list
    /// constructor ([dcl.init.list]/2): its first parameter is
    /// `std::initializer_list<E>` (or a reference to it) and the others
    /// have default arguments.
    pub(crate) fn has_init_list_ctor(&mut self, class: DeclId) -> bool {
        let c = self.class_def(class);
        let name = DeclName::ident(self.intern("initializer_list"));
        for f in self.ctors_and_inherited(c) {
            let f = match self.kind_of(f) {
                DeclKind::CXXConstructor => f,
                DeclKind::FunctionTemplate => {
                    match self.ast.template_info(f).and_then(|t| t.pattern) {
                        Some(x) => x,
                        None => continue,
                    }
                }
                _ => continue,
            };
            let params = self.param_decls(f);
            let Some(&p) = params.first() else { continue };
            if params[1..].iter().any(|&q| !self.param_has_default(q)) {
                continue;
            }
            let pty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
            let t = self.non_reference(pty);
            let named = match self.ast.types.canonical_view(t) {
                TypeView::Record(d) => self.ast.decls.get(d).name() == name,
                TypeView::TemplateSpecialization { .. } => self
                    .ast
                    .types
                    .as_record(t)
                    .is_some_and(|d| self.ast.decls.get(d).name() == name),
                _ => false,
            };
            // A constructor template's dependent `initializer_list<U>`.
            if named || self.initializer_list_param(t).is_some() {
                return true;
            }
        }
        false
    }
}
