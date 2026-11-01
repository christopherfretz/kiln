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
        // without user-defined conversions ([over.best.ics]/4.2).
        let (kind, opts) = if style == InitStyle::Copy {
            (SetKind::CopyInit, IcsOptions::SUPPRESS_UDC)
        } else {
            (SetKind::Normal, IcsOptions::NONE)
        };
        let mut set = self.begin_set(kind, loc);
        for &a in args {
            let info = self.ctor_arg_info(a);
            set.args.push(info);
        }
        self.add_ctor_candidates(&mut set, c, opts, false);
        if let [arg] = args
            && !actual_ctor
        {
            self.add_elision_candidates(&mut set, c, *arg, style);
        }
        self.order_template_candidates(&mut set);
        let out = self.best_viable(&mut set);
        self.selected_of(set, out)
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
        for (found, f) in self.conversion_functions(from) {
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

    /// The constructors of class `c` and the base constructors its
    /// visible `ConstructorUsingShadow`s inherit ([namespace.udecl]/13:
    /// inherited constructors are candidates of [over.match.list] too).
    fn ctors_and_inherited(&self, c: DeclId) -> Vec<DeclId> {
        let mut out: Vec<DeclId> = self.ast.constructors(c).iter().collect();
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
