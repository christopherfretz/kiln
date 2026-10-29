//! Constructor selection for a call with arbitrary arguments
//! ([over.match.ctor], [over.match.list]; spec/sema-overload.md §10.3):
//! a candidate set of the class's constructors resolved by the overload
//! engine. Function templates (WP-071) are skipped; a selection that a
//! skipped template or an unchecked *requires-clause* (WP-073) could
//! change answers [`Selected::Unsupported`].

use kiln_ast::decl::{DeclKind, DefinitionBits};
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
        let c = self.class_def(class);
        if self.defn_bits(c).contains(DefinitionBits::HAS_INHERITED_CTOR) {
            // todo(WP-068a follow-up): inherited constructors
            // ([over.match.funcs]/8) are not candidates yet.
            return Selected::Unsupported;
        }
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
            let info = self.arg_info(a);
            set.args.push(info);
        }
        self.add_ctor_candidates(&mut set, c, opts, false);
        let out = self.best_viable(&mut set);
        self.selected_of(set, out)
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
            OvlOutcome::Success(b) => Selected::Found(set.cand(b).func()),
            OvlOutcome::Deleted(b) => Selected::Deleted(set.cand(b).func()),
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

    /// Whether some constructor of `class` is an initializer-list
    /// constructor ([dcl.init.list]/2): its first parameter is
    /// `std::initializer_list<E>` (or a reference to it) and the others
    /// have default arguments.
    pub(crate) fn has_init_list_ctor(&mut self, class: DeclId) -> bool {
        let c = self.class_def(class);
        let name = DeclName::ident(self.intern("initializer_list"));
        for f in self.ast.constructors(c).iter().collect::<Vec<_>>() {
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
            if named {
                return true;
            }
        }
        false
    }
}
