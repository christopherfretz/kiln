//! Eligible special members ([special]/6) and the class properties the
//! standard defines by them: trivially copyable ([class.prop]/1) and
//! trivial ([class.prop]/2) classes (spec/sema-class.md §4.7.3).
//!
//! The class bits (`TRIVIAL_*`, `NONTRIVIAL_*`, `TRIVIALLY_COPYABLE`)
//! follow clang, which counts a deleted member as eligible; they keep
//! driving clang's own predicates (debug info, `isValidVarArgType`, the
//! constant evaluator's `memcpy`). The traits ask the functions here.

use kiln_ast::DeclId;
use kiln_ast::decl::{DeclKind, DefinitionBits};
use kiln_ast::names::{DeclName, OverloadedOperator};

use super::SpecialKind;
use crate::Sema;

/// The eligible special members of one kind of a class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Eligible {
    /// The class has at least one.
    pub(crate) any: bool,
    /// Every one is trivial (true when there is none).
    pub(crate) all_trivial: bool,
    /// Some one is trivial.
    pub(crate) any_trivial: bool,
}

impl Eligible {
    const NONE: Eligible = Eligible { any: false, all_trivial: true, any_trivial: false };

    fn add(&mut self, trivial: bool) {
        self.any = true;
        self.all_trivial &= trivial;
        self.any_trivial |= trivial;
    }
}

impl Sema<'_, '_> {
    /// [special]/6: whether the declared special member `f` is eligible:
    /// not deleted, and not marked ineligible by
    /// `select_eligible_special_members` at the class's `}` (its
    /// constraints unsatisfied, or another member of the kind more
    /// constrained), nor an unselected prospective destructor. The one
    /// test every trait reading eligibility uses.
    pub(crate) fn is_eligible_special(&self, f: DeclId) -> bool {
        !self.is_deleted_fn(f) && !self.fn_spec(f).not_selected()
    }

    /// The eligible special members of kind `k` (not the destructor) of
    /// complete class `r`: its declared ones (user-declared or implicit),
    /// else the implicit one it would declare, which is deleted only when
    /// the class bits already say so (an implicit copy or move member
    /// whose deletion needs overload resolution is declared at `}`).
    /// A constructor template whose parameters all have default
    /// arguments counts as a non-trivial default constructor, as for the
    /// class bits (spec/sema-class.md §4.3).
    pub(crate) fn eligible_specials(&self, r: DeclId, k: SpecialKind) -> Eligible {
        let d = self.class_def(r);
        let mut out = Eligible::NONE;
        let mut declared = false;
        let cands: Vec<DeclId> = match k {
            SpecialKind::DefaultCtor | SpecialKind::CopyCtor | SpecialKind::MoveCtor => {
                self.ast.constructors(d).iter().collect()
            }
            SpecialKind::CopyAssign | SpecialKind::MoveAssign => self
                .ast
                .lookup_local(d, DeclName::operator(OverloadedOperator::Equal))
                .iter()
                .collect(),
            SpecialKind::Dtor => self.ast.destructor(d).into_iter().collect(),
        };
        for m in cands {
            if self.kind_of(m) == DeclKind::FunctionTemplate {
                if k == SpecialKind::DefaultCtor
                    && let Some(p) = self.pattern_of(m)
                    && self.kind_of(p) == DeclKind::CXXConstructor
                    && self.special_params(p).iter().all(|&x| self.param_has_default(x))
                    && !self.is_deleted_fn(p)
                {
                    out.add(false);
                }
                continue;
            }
            if self.special_kind_of(m) != Some(k) {
                continue;
            }
            declared = true;
            if self.is_eligible_special(m) {
                out.add(self.fn_spec(m).is_trivial());
            }
        }
        if !declared && self.needs_implicit(d, k) && !self.known_deleted(d, k) {
            out.add(self.has_trivial(d, k));
        }
        out
    }

    /// Whether complete class `r` has a trivial, non-deleted destructor
    /// ([class.prop]/1, [class.dtor]/8; access does not matter).
    pub(crate) fn has_trivial_nondeleted_dtor(&self, r: DeclId) -> bool {
        let d = self.class_def(r);
        if !self.defn_bits(d).contains(DefinitionBits::TRIVIAL_DTOR) {
            return false;
        }
        match self.ast.destructor(d) {
            Some(dt) => !self.is_deleted_fn(dt),
            None => !self.known_deleted(d, SpecialKind::Dtor),
        }
    }

    /// [class.prop]/1: complete class `r` is trivially copyable: it has
    /// at least one eligible copy constructor, move constructor, copy
    /// assignment operator or move assignment operator, each eligible
    /// one is trivial, and its destructor is trivial and not deleted.
    /// Unlike clang, a class whose copy and move members are all deleted
    /// is not trivially copyable, and a deleted non-trivial member does
    /// not make it non-trivially copyable.
    pub(crate) fn is_trivially_copyable_class(&self, r: DeclId) -> bool {
        use SpecialKind as K;
        let mut any = false;
        for k in [K::CopyCtor, K::MoveCtor, K::CopyAssign, K::MoveAssign] {
            let e = self.eligible_specials(r, k);
            if !e.all_trivial {
                return false;
            }
            any |= e.any;
        }
        any && self.has_trivial_nondeleted_dtor(r)
    }

    /// [class.prop]/2: complete class `r` is trivial: trivially copyable,
    /// with one or more eligible default constructors, all trivial. The
    /// implicit default constructor is declared first when it decides the
    /// answer (whether it is deleted, [class.default.ctor]/2, needs
    /// overload resolution on the subobjects).
    pub(crate) fn is_trivial_class(&mut self, r: DeclId) -> bool {
        if !self.is_trivially_copyable_class(r) {
            return false;
        }
        let d = self.class_def(r);
        if self.needs_implicit(d, SpecialKind::DefaultCtor)
            && self.has_trivial(d, SpecialKind::DefaultCtor)
        {
            self.ensure_implicit(d, SpecialKind::DefaultCtor);
        }
        let e = self.eligible_specials(d, SpecialKind::DefaultCtor);
        e.any && e.all_trivial
    }
}
