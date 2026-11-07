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
            let opts = if same_or_derived { IcsOptions::NONE } else { IcsOptions::SUPPRESS_UDC };
            (SetKind::CopyInit, opts, same_or_derived)
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
            self.add_ctor_candidates(&mut set, c, opts, false);
        }
        if let [arg] = args
            && !actual_ctor
        {
            self.add_elision_candidates(&mut set, c, *arg, style);
        }
        let out = self.resolve_set(&mut set);
        self.selected_of(set, out)
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
        let found = self.converting_ctors(c, true);
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

    /// clang's workaround for C++17 guaranteed copy elision (CWG2327):
    /// when class `c` is initialized by constructor from one expression
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
