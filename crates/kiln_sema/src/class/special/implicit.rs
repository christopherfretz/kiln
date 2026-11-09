//! Which special members a class has, their triviality and `constexpr`,
//! and their implicit declarations (spec/sema-class.md §4.1, §4.3, §4.6;
//! clang `CXXRecordDecl::addedMember` / `addedClassSubobject`,
//! `AddImplicitlyDeclaredMembersToClass`, `DeclareImplicit*`).

use kiln_ast::decl::{
    BaseSpec, ConstexprKind, DeclFlags, DeclKind, Defaulted, DefinitionBits, FieldData, FnSpec,
    TagKind,
};
use kiln_ast::names::{DeclName, OverloadedOperator};
use kiln_ast::types::{ExcSpec, FuncInfo, Quals, TypeView};
use kiln_ast::{DeclHead, DeclId, QualType};
use kiln_source::fx::FxHashSet;

use super::SpecialKind;
use super::deletion::Selected;
use crate::Sema;
use crate::class::r#virtual::is_virtual;

/// How deep the walks over bases and member classes go before giving up
/// (a cycle is impossible for complete classes; this bounds the stack).
const MAX_DEPTH: usize = 256;

/// Whether `k` is declared, given the definition bits and the
/// implicitly-declared set (clang `DeclaredSpecialMembers`).
fn declared(bits: DefinitionBits, implicit: u8, k: SpecialKind) -> bool {
    implicit & k.bit() != 0 || bits.contains(k.user_declared_bit())
}

/// clang `needsImplicit*` ([class.default.ctor]/1, [class.copy.ctor]/6
/// and /8, [class.copy.assign]/2 and /4, [class.dtor]/2): `k` would be
/// implicitly declared and has not been yet.
pub(crate) fn needs_from(bits: DefinitionBits, implicit: u8, k: SpecialKind) -> bool {
    if declared(bits, implicit, k) {
        return false;
    }
    let ud = |k: SpecialKind| bits.contains(k.user_declared_bit());
    match k {
        SpecialKind::MoveCtor => {
            !ud(SpecialKind::CopyCtor)
                && !ud(SpecialKind::CopyAssign)
                && !ud(SpecialKind::MoveAssign)
                && !ud(SpecialKind::Dtor)
        }
        SpecialKind::MoveAssign => {
            !bits.contains(DefinitionBits::LAMBDA_CAPTURES)
                && !ud(SpecialKind::CopyCtor)
                && !ud(SpecialKind::CopyAssign)
                && !ud(SpecialKind::MoveCtor)
                && !ud(SpecialKind::Dtor)
        }
        _ => true,
    }
}

/// clang `needsOverloadResolutionFor*`: whether the implicit `k` of a
/// class with `bits` must be declared to know its properties.
pub(crate) fn needs_ovl_from(bits: DefinitionBits, k: SpecialKind) -> bool {
    let moves = bits.contains(DefinitionBits::USER_DECLARED_MOVE_CTOR)
        || bits.contains(DefinitionBits::USER_DECLARED_MOVE_ASSIGN);
    match k {
        SpecialKind::CopyCtor | SpecialKind::CopyAssign if moves => true,
        _ => k.needs_ovl_bit().is_some_and(|b| bits.contains(b)),
    }
}

/// What the scan of a class's declared special members found.
#[derive(Default)]
#[allow(clippy::struct_excessive_bools, reason = "independent facts of the scan")]
struct Declared {
    /// Kinds with a user-declared (non-implicit) member.
    user: u8,
    /// Kinds with a declared member that is not trivial.
    non_trivial: u8,
    /// Some user-declared constructor (template included) is `constexpr`
    /// and not a copy/move constructor.
    constexpr_ctor: bool,
    /// An in-class defaulted default constructor.
    defaulted_default_ctor: bool,
    /// A user-provided default constructor.
    provided_default_ctor: bool,
    /// Any user-declared constructor (template included).
    any_ctor: bool,
    /// A user-declared constructor (template included) that is
    /// user-provided or `explicit`: before C++20 only these make the class
    /// a non-aggregate ([dcl.init.aggr]/1 in C++11-17, with DR1518).
    provided_or_explicit_ctor: bool,
    /// The user-declared destructor is `constexpr` (or defaulted in
    /// class, decided by [`Sema::defaulted_is_constexpr`]).
    dtor_constexpr: Option<bool>,
}

impl Sema<'_, '_> {
    /// The definition of class `r` (or `r`).
    pub(crate) fn class_def(&self, r: DeclId) -> DeclId {
        self.ast.definition(r).unwrap_or(r)
    }

    /// The definition bits of class `r`: those being computed at its `}`
    /// while it is finishing, else the stored ones.
    pub(crate) fn defn_bits(&self, r: DeclId) -> DefinitionBits {
        let d = self.class_def(r);
        if let Some(&(_, b)) = self.special.finishing.iter().rev().find(|x| x.0 == d) {
            return b;
        }
        self.ast.decls.record(d).map(|x| x.defn).unwrap_or_default()
    }

    /// Updates the bits of finishing class `d` (no-op otherwise).
    fn set_finishing_bits(&mut self, d: DeclId, f: impl FnOnce(DefinitionBits) -> DefinitionBits) {
        if let Some(e) = self.special.finishing.iter_mut().rev().find(|x| x.0 == d) {
            e.1 = f(e.1);
        }
    }

    /// The implicitly declared special members of class `r`.
    pub(crate) fn implicit_bits(&self, r: DeclId) -> u8 {
        self.implicit_declared.get(&self.class_def(r)).copied().unwrap_or(0)
    }

    /// Whether the special member `k` of class `r` would be implicitly
    /// declared and has not been yet.
    pub(crate) fn needs_implicit(&self, r: DeclId, k: SpecialKind) -> bool {
        needs_from(self.defn_bits(r), self.implicit_bits(r), k)
    }

    /// Whether class `r` has a special member `k` (declared, or to be
    /// implicitly declared; clang `hasDefaultConstructor`,
    /// `hasMoveConstructor`, `hasMoveAssignment`).
    pub(crate) fn has_special(&self, r: DeclId, k: SpecialKind) -> bool {
        let bits = self.defn_bits(r);
        let implicit = self.implicit_bits(r);
        (declared(bits, implicit, k)
            && (k != SpecialKind::DefaultCtor || self.has_default_ctor_decl(r)))
            || needs_from(bits, implicit, k)
    }

    /// Whether class `r` declares a default constructor.
    fn has_default_ctor_decl(&self, r: DeclId) -> bool {
        let d = self.class_def(r);
        self.ast
            .constructors(d)
            .iter()
            .any(|c| self.special_kind_of(c) == Some(SpecialKind::DefaultCtor))
    }

    /// Whether the defaulted `k` of class `r` is known deleted (clang
    /// `Defaulted*IsDeleted`).
    pub(crate) fn known_deleted(&self, r: DeclId, k: SpecialKind) -> bool {
        let d = self.class_def(r);
        k.defaulted_deleted_bit().is_some_and(|b| self.defn_bits(d).contains(b))
            || self.special.deleted_implicit.get(&d).is_some_and(|m| m & k.bit() != 0)
    }

    /// clang `hasSimple*`: whether the `k` of class `r` is known to be the
    /// non-deleted implicit (or defaulted) one without overload
    /// resolution.
    pub(crate) fn has_simple(&self, r: DeclId, k: SpecialKind) -> bool {
        let bits = self.defn_bits(r);
        if bits.contains(k.user_declared_bit()) || self.known_deleted(r, k) {
            return false;
        }
        match k {
            SpecialKind::MoveCtor | SpecialKind::MoveAssign => self.has_special(r, k),
            _ => true,
        }
    }

    /// clang `hasTrivial*`: whether class `r` has a trivial `k`.
    pub(crate) fn has_trivial(&self, r: DeclId, k: SpecialKind) -> bool {
        let bits = self.defn_bits(r);
        match k {
            SpecialKind::MoveCtor | SpecialKind::MoveAssign => {
                self.has_special(r, k) && bits.contains(k.trivial_bit())
            }
            _ => bits.contains(k.trivial_bit()),
        }
    }

    /// clang `hasCopyConstructorWithConstParam` /
    /// `hasCopyAssignmentWithConstParam` of class `r`.
    fn copy_has_const_param(&self, r: DeclId, k: SpecialKind) -> bool {
        let d = self.class_def(r);
        let bits = self.defn_bits(d);
        let implicit_bit = match k {
            SpecialKind::CopyCtor => DefinitionBits::IMPLICIT_COPY_CTOR_CONST_PARAM,
            _ => DefinitionBits::IMPLICIT_COPY_ASSIGN_CONST_PARAM,
        };
        if self.needs_implicit(d, k) && bits.contains(implicit_bit) {
            return true;
        }
        let cands: Vec<DeclId> = if k == SpecialKind::CopyCtor {
            self.ast.constructors(d).iter().collect()
        } else {
            self.ast.lookup_local(d, DeclName::operator(OverloadedOperator::Equal)).iter().collect()
        };
        cands.into_iter().any(|m| self.special_kind_of(m) == Some(k) && self.copy_param_is_const(m))
    }

    /// Whether the first parameter of copy constructor / assignment `m`
    /// accepts a `const` argument (a reference to `const`, or by value).
    pub(crate) fn copy_param_is_const(&self, m: DeclId) -> bool {
        let Some(&p) = self.special_params(m).first() else { return false };
        let ty = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
        match self.ast.types.canonical_view(ty) {
            TypeView::LValueRef(t) | TypeView::RValueRef(t) => {
                self.ast.types.effective_quals(t).is_const()
            }
            _ => true,
        }
    }

    /// Whether class `r` can be const-default-initialized (clang
    /// `allowConstDefaultInit`, [dcl.init.general]/8).
    fn can_const_default_init(&self, r: DeclId) -> bool {
        self.defn_bits(r).contains(DefinitionBits::CAN_CONST_DEFAULT_INIT)
    }

    /// The array element type of `q` (arrays of arrays stripped).
    pub(crate) fn base_element(&self, q: QualType) -> QualType {
        let mut t = q;
        for _ in 0..MAX_DEPTH {
            match self.ast.types.element(t) {
                Some(e) if self.ast.types.is_array(t) => t = e,
                _ => break,
            }
        }
        t
    }

    /// The complete class of the base-element type of `q`, if any.
    pub(crate) fn element_class(&self, q: QualType) -> Option<DeclId> {
        let d = self.ast.types.as_record(self.base_element(q))?;
        let d = self.ast.definition(d)?;
        self.defn_bits(d).contains(DefinitionBits::COMPLETE).then_some(d)
    }

    /// The class of base specifier `b`, if complete.
    pub(crate) fn base_class(&self, b: &BaseSpec) -> Option<DeclId> {
        let d = self.ast.types.as_record(b.ty)?;
        let d = self.ast.definition(d)?;
        self.defn_bits(d).contains(DefinitionBits::COMPLETE).then_some(d)
    }

    /// The virtual base classes of class `r`, in clang's `vbases()` order:
    /// per direct base, its virtual bases, then itself if virtual; each
    /// class once. The specifier is the one naming it (in `r` or in the
    /// base that introduced it).
    pub(crate) fn virtual_bases(&self, r: DeclId) -> Vec<(DeclId, BaseSpec)> {
        let mut out: Vec<(DeclId, BaseSpec)> = Vec::new();
        let mut seen: FxHashSet<DeclId> = FxHashSet::default();
        let mut walked: FxHashSet<DeclId> = FxHashSet::default();
        self.collect_vbases(self.class_def(r), &mut out, &mut seen, &mut walked, 0);
        out
    }

    /// Appends the virtual bases of `r` not yet in `out` in inheritance
    /// graph order. A class already `walked` has all its virtual bases in
    /// `out`, so each class is walked once (linear in a virtual lattice,
    /// not exponential; WP-400).
    fn collect_vbases(
        &self,
        r: DeclId,
        out: &mut Vec<(DeclId, BaseSpec)>,
        seen: &mut FxHashSet<DeclId>,
        walked: &mut FxHashSet<DeclId>,
        depth: usize,
    ) {
        if depth > MAX_DEPTH {
            return;
        }
        let bases: Vec<BaseSpec> = self.ast.decls.bases(r, &self.ast.lists).copied().collect();
        for b in bases {
            let Some(bd) = self.base_class(&b) else { continue };
            let first = self.ast.first(bd);
            if self.defn_bits(bd).contains(DefinitionBits::HAS_VIRTUAL_BASES)
                && walked.insert(first)
            {
                self.collect_vbases(bd, out, seen, walked, depth + 1);
            }
            if b.is_virtual && seen.insert(first) {
                out.push((bd, b));
            }
        }
    }

    /// Whether field `f` has a default member initializer.
    pub(crate) fn field_has_nsdmi(&self, f: DeclId) -> bool {
        self.special.nsdmi.contains(&f) || self.ast.decls.field(f).is_some_and(|x| x.init.is_some())
    }

    /// Whether field `f` is `mutable`.
    pub(crate) fn field_is_mutable(&self, f: DeclId) -> bool {
        self.ast.decls.field(f).is_some_and(|x| x.flags & FieldData::MUTABLE != 0)
    }

    /// Whether field `f` is an unnamed bit-field (not a member for the
    /// special-member rules, [class.bit]/2).
    pub(crate) fn is_unnamed_bit_field(&self, f: DeclId) -> bool {
        self.ast.decls.field(f).is_some_and(|x| x.bit_width.is_some())
            && self.ast.decls.get(f).name().is_empty()
    }

    /// Whether field `f` is an anonymous struct or union member.
    pub(crate) fn is_anonymous_member(&self, f: DeclId) -> bool {
        let Some(t) = self.ast.decls.get(f).ty() else { return false };
        self.ast
            .types
            .as_record(t)
            .and_then(|d| self.ast.decls.record(self.class_def(d)))
            .is_some_and(|x| x.defn.contains(DefinitionBits::ANONYMOUS))
    }

    /// The fields of class `r` the special-member rules consider (no
    /// unnamed bit-fields).
    pub(crate) fn special_fields(&self, r: DeclId) -> Vec<DeclId> {
        self.special_fields_iter(r).collect()
    }

    /// [`Sema::special_fields`] without collecting them (WP-645).
    pub(crate) fn special_fields_iter(&self, r: DeclId) -> impl Iterator<Item = DeclId> + '_ {
        self.ast
            .decls
            .members(self.class_def(r))
            .filter(|&f| self.kind_of(f) == DeclKind::Field && !self.is_unnamed_bit_field(f))
    }

    /// Whether class `r` is a union.
    pub(crate) fn is_union(&self, r: DeclId) -> bool {
        self.ast.decls.record(self.class_def(r)).is_some_and(|x| x.tag == TagKind::Union)
    }

    /// Whether the P3074R7 trivial-union rules apply (C++26,
    /// spec/sema-class.md §4.2): a variant member's non-trivial default
    /// constructor or destructor no longer deletes or makes non-trivial
    /// its union's.
    pub(crate) fn trivial_unions(&self) -> bool {
        self.std >= kiln_lex::Std::Cxx26
    }

    /// Whether field `f` is a variant member (a direct member of a union,
    /// named or anonymous; [class.union.general]/1).
    pub(crate) fn is_variant_member(&self, f: DeclId) -> bool {
        self.ast.decls.get(f).ctx().is_some_and(|p| self.is_union(p))
    }

    /// Whether an implicit or defaulted `k` of class `class` (copying from
    /// a `const` argument when `const_arg`) is `constexpr`
    /// (spec/sema-class.md §4.6; clang `defaultedSpecialMemberIsConstexpr`):
    /// never a constructor of a class with a virtual base before C++26
    /// (P3533R2); before C++23 (P2448R2) only when the member each base and
    /// field subobject uses is constexpr, and an assignment of a literal
    /// class from C++14 on, a destructor from C++20 on.
    pub(crate) fn defaulted_is_constexpr(
        &mut self,
        class: DeclId,
        k: SpecialKind,
        const_arg: bool,
    ) -> bool {
        use SpecialKind as K;
        let c = self.class_def(class);
        let bits = self.defn_bits(c);
        match k {
            K::DefaultCtor => {
                return bits.contains(DefinitionBits::DEFAULTED_DEFAULT_CTOR_CONSTEXPR);
            }
            K::Dtor => {
                if self.vbases_forbid_constexpr(bits)
                    && !self.special.vbase_dtor_constexpr.contains(&c)
                {
                    return false;
                }
                // [class.dtor]/9 before C++23 (P2448R2): constexpr only
                // when every subobject's destructor is (clang
                // `defaultedDestructorIsConstexpr`, C++20 on).
                if self.std != kiln_lex::Std::Cxx20 {
                    return self.std > kiln_lex::Std::Cxx20;
                }
                let bases: Vec<DeclId> = self
                    .ast
                    .decls
                    .bases(c, &self.ast.lists)
                    .copied()
                    .collect::<Vec<BaseSpec>>()
                    .iter()
                    .filter_map(|b| self.base_class(b))
                    .collect();
                let fields: Vec<DeclId> = self
                    .special_fields(c)
                    .into_iter()
                    .filter(|&f| !self.is_invalid_decl(f))
                    .filter_map(|f| self.ast.decls.get(f).ty())
                    .filter(|&t| !self.ast.types.is_reference(t))
                    .filter_map(|t| self.element_class(t))
                    .collect();
                return bases.into_iter().chain(fields).all(|s| self.has_constexpr_dtor(s));
            }
            K::CopyCtor | K::MoveCtor => {
                if self.is_union(c) {
                    return true;
                }
                if self.vbases_forbid_constexpr(bits) {
                    return false;
                }
            }
            K::CopyAssign | K::MoveAssign => {
                if self.std < kiln_lex::Std::Cxx14 {
                    return false;
                }
                if self.std < kiln_lex::Std::Cxx23 && !bits.contains(DefinitionBits::LITERAL) {
                    return false;
                }
            }
        }
        if self.std >= kiln_lex::Std::Cxx23 {
            return true;
        }
        let bases: Vec<DeclId> = self
            .ast
            .decls
            .bases(c, &self.ast.lists)
            .copied()
            .collect::<Vec<BaseSpec>>()
            .iter()
            .filter_map(|b| self.base_class(b))
            .collect();
        let rhs = if const_arg { Quals::CONST } else { Quals::NONE };
        for b in bases {
            if !self.special_member_is_constexpr(b, k, rhs, Quals::NONE) {
                return false;
            }
        }
        for f in self.special_fields(c) {
            if self.is_invalid_decl(f) {
                continue;
            }
            let Some(t) = self.ast.decls.get(f).ty() else { continue };
            if self.ast.types.is_reference(t) {
                continue;
            }
            let Some(fc) = self.element_class(t) else { continue };
            let fq = self.ast.types.effective_quals(self.base_element(t))
                & (Quals::CONST | Quals::VOLATILE);
            let mut rq = fq;
            if const_arg && !self.field_is_mutable(f) {
                rq |= Quals::CONST;
            }
            let lq = if k.is_assign() { fq } else { Quals::NONE };
            if !self.special_member_is_constexpr(fc, k, rq, lq) {
                return false;
            }
        }
        true
    }

    /// Whether the `k` that copies or moves a subobject of class `c`
    /// (argument qualifiers `rhs`, object qualifiers `lhs`) is constexpr;
    /// a member that would not be selected involves nothing (clang
    /// `specialMemberIsConstexpr`).
    fn special_member_is_constexpr(
        &mut self,
        c: DeclId,
        k: SpecialKind,
        rhs: Quals,
        lhs: Quals,
    ) -> bool {
        match self.select_special(c, k, rhs, lhs) {
            Selected::Found(m) | Selected::Deleted(m) => {
                if self.kind_of(m) == DeclKind::ConstructorUsingShadow {
                    return self.inherited_constexpr_ctor(m);
                }
                let f = if self.kind_of(m) == DeclKind::FunctionTemplate {
                    self.pattern_of(m).unwrap_or(m)
                } else {
                    m
                };
                self.fn_spec(f).constexpr_kind() != ConstexprKind::None
            }
            Selected::Missing | Selected::Ambiguous | Selected::Unsupported => true,
        }
    }

    /// Whether complete class `c` has a constexpr default constructor,
    /// declared or still to be implicitly declared (clang
    /// `hasConstexprDefaultConstructor`). As clang's `addedMember` sets
    /// it, a constexpr constructor template callable without arguments
    /// (no parameters, or a first parameter with a default argument or
    /// that is a pack) counts too ([class.default.ctor]/1: a default
    /// constructor is any constructor callable with no arguments).
    pub(crate) fn has_constexpr_default_ctor(&self, c: DeclId) -> bool {
        let d = self.class_def(c);
        let declared = self.ast.constructors(d).iter().any(|f| {
            if self.kind_of(f) == DeclKind::FunctionTemplate {
                return self.pattern_of(f).is_some_and(|p| {
                    self.kind_of(p) == DeclKind::CXXConstructor
                        && self.fn_spec(p).constexpr_kind() != ConstexprKind::None
                        && self.param_decls(p).first().is_none_or(|&q| {
                            self.param_has_default(q)
                                || self.ast.decls.get(q).ty().is_some_and(|t| {
                                    matches!(
                                        self.ast.types.view(t.ty()),
                                        TypeView::PackExpansion { .. }
                                    )
                                })
                        })
                });
            }
            self.special_kind_of(f) == Some(SpecialKind::DefaultCtor)
                && self.fn_spec(f).constexpr_kind() != ConstexprKind::None
        });
        declared
            || self.needs_implicit(d, SpecialKind::DefaultCtor)
                && self.defn_bits(d).contains(DefinitionBits::DEFAULTED_DEFAULT_CTOR_CONSTEXPR)
    }

    /// Whether a defaulted default constructor of class `r` (a union when
    /// `union`) with direct bases `bases` and fields `fields` is
    /// constexpr ([dcl.constexpr]/3 before C++23 with [class.default.ctor];
    /// clang `DefaultedDefaultConstructorIsConstexpr` as `addedMember`
    /// computes it, with `defaultedDefaultConstructorIsConstexpr`'s union
    /// rule). Only a direct virtual base forbids it before C++26, as
    /// clang's `setBases` clears the flag for one: from C++23 a class whose
    /// virtual bases are all indirect has a constexpr defaulted default
    /// constructor (its construction then fails in the Oven).
    fn defaulted_default_ctor_constexpr(
        &self,
        union: bool,
        bases: &[(DeclId, bool)],
        fields: &[DeclId],
    ) -> bool {
        use kiln_lex::Std;
        if self.std >= Std::Cxx26 {
            return true;
        }
        if self.std >= Std::Cxx23 {
            return self.vbase_flag_cxx23(bases, fields, false);
        }
        if bases.iter().any(|&(_, v)| v) {
            return false;
        }
        if bases.iter().any(|&(b, _)| !self.has_constexpr_default_ctor(b)) {
            return false;
        }
        let mut any_init = false;
        for &f in fields {
            if self.field_has_nsdmi(f) {
                any_init = true;
                continue;
            }
            if union || self.is_invalid_decl(f) {
                continue;
            }
            let Some(t) = self.ast.decls.get(f).ty() else { continue };
            let class = if self.ast.types.is_reference(t) { None } else { self.element_class(t) };
            match class {
                Some(c) => {
                    if !self.has_constexpr_default_ctor(c) {
                        return false;
                    }
                }
                // [dcl.constexpr]/4 before C++20 (P1331R2): every
                // subobject is initialized.
                None => {
                    if self.std < Std::Cxx20 {
                        return false;
                    }
                }
            }
        }
        !union || any_init || fields.is_empty() || self.std >= Std::Cxx20
    }

    /// Whether a class with `bits` has a virtual base that keeps its
    /// constructors and destructor from being constexpr-suitable
    /// ([dcl.constexpr]/3 before C++26; P3533R2 lifts it).
    /// Clang's C++23 `DefaultedDefaultConstructorIsConstexpr` (`dtor`:
    /// `DefaultedDestructorIsConstexpr`) for a class with direct bases
    /// `bases` (class, virtual) and fields `fields`: a running flag,
    /// cleared by a virtual base and set again by a later non-virtual base
    /// or (non-static, class-type, for the constructor
    /// non-default-initialized) member whose default constructor
    /// (destructor) is not constexpr; every other subobject leaves it.
    /// So `struct A : virtual V, NB {}` with a non-constexpr `NB()` has a
    /// constexpr defaulted default constructor where `struct A : NB,
    /// virtual V {}` does not (clang 19 and 22 alike).
    fn vbase_flag_cxx23(&self, bases: &[(DeclId, bool)], fields: &[DeclId], dtor: bool) -> bool {
        let lacks = |c: DeclId| {
            if dtor { !self.has_constexpr_dtor(c) } else { !self.has_constexpr_default_ctor(c) }
        };
        let mut flag = true;
        for &(b, virt) in bases {
            if virt {
                flag = false;
            } else if lacks(b) {
                flag = true;
            }
        }
        for &f in fields {
            if !dtor && self.field_has_nsdmi(f) || self.is_invalid_decl(f) {
                continue;
            }
            let Some(t) = self.ast.decls.get(f).ty() else { continue };
            if self.ast.types.is_reference(t) {
                continue;
            }
            if let Some(c) = self.element_class(t)
                && !self.is_union(c)
                && lacks(c)
            {
                flag = true;
            }
        }
        flag
    }

    /// Whether complete class `c` has a constexpr destructor, declared or
    /// still to be implicitly declared (clang `hasConstexprDestructor`).
    pub(crate) fn has_constexpr_dtor(&self, c: DeclId) -> bool {
        let d = self.class_def(c);
        match self.ast.destructor(d) {
            Some(f) => self.fn_spec(f).constexpr_kind() != ConstexprKind::None,
            None => {
                !self.vbases_forbid_constexpr(self.defn_bits(d))
                    || self.special.vbase_dtor_constexpr.contains(&d)
            }
        }
    }

    pub(crate) fn vbases_forbid_constexpr(&self, bits: DefinitionBits) -> bool {
        self.std < kiln_lex::Std::Cxx26 && bits.contains(DefinitionBits::HAS_VIRTUAL_BASES)
    }

    /// Whether `m`, a defaulted or deleted `k` of a class whose subobjects
    /// allow a trivial `k` (`class_trivial`), is trivial
    /// ([class.default.ctor]/3, [class.copy.ctor]/11, [class.copy.assign]/9,
    /// [class.dtor]/8; clang `SpecialMemberIsTrivial`'s signature checks).
    fn defaulted_is_trivial(&self, m: DeclId, k: SpecialKind, class_trivial: bool) -> bool {
        if !class_trivial {
            return false;
        }
        // A variadic special member (`T(...) = delete`, a default
        // constructor by [class.default.ctor]/1; `T(const T&, ...)`) has
        // a parameter-type-list unlike the implicit one's, so it is never
        // trivial (clang `SpecialMemberIsTrivial`'s
        // `note_nontrivial_variadic`; WP-1090).
        if self.fn_info(m).variadic() {
            return false;
        }
        let params = self.special_params(m);
        match k {
            SpecialKind::DefaultCtor => {
                if !params.is_empty() {
                    return false;
                }
            }
            SpecialKind::Dtor => {
                if is_virtual(&self.ast, m) {
                    return false;
                }
            }
            _ => {
                if params.len() != 1 {
                    return false;
                }
                let ty = self.ast.decls.get(params[0]).ty().unwrap_or(QualType::ERROR);
                let ok = match self.ast.types.canonical_view(ty) {
                    TypeView::LValueRef(_) => !k.is_move(),
                    TypeView::RValueRef(t) => {
                        k.is_move() && self.ast.types.effective_quals(t).is_empty()
                    }
                    _ => false,
                };
                if !ok {
                    return false;
                }
            }
        }
        !params.iter().any(|&p| self.param_has_default(p))
    }

    /// Whether constructor-using shadow `m` names a constexpr constructor
    /// or constructor template that is not a copy or move constructor.
    fn inherited_constexpr_ctor(&self, m: DeclId) -> bool {
        let Some(t) = self.ast.using_target(m) else { return false };
        let f = if self.kind_of(t) == DeclKind::FunctionTemplate {
            match self.pattern_of(t) {
                Some(p) => p,
                None => return false,
            }
        } else {
            t
        };
        self.kind_of(f) == DeclKind::CXXConstructor
            && !matches!(
                self.special_kind_of(f),
                Some(SpecialKind::CopyCtor | SpecialKind::MoveCtor)
            )
            && self
                .ast
                .decls
                .function(f)
                .is_some_and(|x| x.spec.constexpr_kind() != ConstexprKind::None)
    }

    /// The scan of class `r`'s own special members at its `}`.
    fn scan_declared(&mut self, r: DeclId, class_trivial: DefinitionBits) -> Declared {
        let mut out = Declared::default();
        let members: Vec<DeclId> = self.ast.members(r).collect();
        let vbases = class_trivial.contains(DefinitionBits::HAS_VIRTUAL_BASES);
        // An in-class defaulted destructor is constexpr unless virtual
        // bases forbid it (before C++26, P3533R2); an in-class defaulted
        // default constructor when its subobjects allow it
        // (`DEFAULTED_DEFAULT_CTOR_CONSTEXPR`), or always in an
        // instantiation (CWG1358, clang `CheckExplicitlyDefaultedSpecialMember`).
        let vb_forbid = self.vbases_forbid_constexpr(class_trivial);
        let ddc = class_trivial.contains(DefinitionBits::DEFAULTED_DEFAULT_CTOR_CONSTEXPR);
        for m in members {
            if self.kind_of(m) == DeclKind::ConstructorUsingShadow {
                // [basic.types.general]/10.5.2: a constexpr constructor
                // (or template) inherited from a base counts, as clang's
                // `addedMember` counts the shadow's underlying constructor.
                if self.inherited_constexpr_ctor(m) {
                    out.constexpr_ctor = true;
                }
                continue;
            }
            let f = if self.kind_of(m) == DeclKind::FunctionTemplate {
                match self.pattern_of(m) {
                    Some(p) => p,
                    None => continue,
                }
            } else {
                m
            };
            let fkind = self.kind_of(f);
            if !matches!(
                fkind,
                DeclKind::CXXConstructor | DeclKind::CXXDestructor | DeclKind::CXXMethod
            ) || !self.is_user_declared(f)
            {
                continue;
            }
            // clang's `addedMember` ignores an invalid declaration: a
            // constructor with a parameter naming no type leaves the class
            // an aggregate with its implicit default constructor (WP-318).
            if self.is_invalid_decl(m) || self.is_invalid_decl(f) {
                continue;
            }
            let spec = self.ast.decls.function(f).map(|x| x.spec).unwrap_or_default();
            // [class.dtor]/4: a prospective destructor that was not
            // selected is not the destructor (clang counts only the
            // selected one, `addedSelectedDestructor`).
            if spec.not_selected() {
                continue;
            }
            if fkind == DeclKind::CXXConstructor {
                out.any_ctor = true;
                if self.is_user_provided(f) || spec.is_explicit() {
                    out.provided_or_explicit_ctor = true;
                }
            }
            let kind = self.special_kind_of(f);
            let in_class_default = spec.defaulted() == Defaulted::Explicit;
            if fkind == DeclKind::CXXConstructor
                && !matches!(kind, Some(SpecialKind::CopyCtor | SpecialKind::MoveCtor))
                && (spec.constexpr_kind() != ConstexprKind::None
                    || in_class_default
                        && (ddc || !vb_forbid && self.ast.instantiated_from(f).is_some()))
            {
                out.constexpr_ctor = true;
            }
            let Some(k) = kind else { continue };
            out.user |= k.bit();
            match k {
                SpecialKind::DefaultCtor => {
                    if in_class_default {
                        out.defaulted_default_ctor = true;
                    }
                    if self.is_user_provided(f) {
                        out.provided_default_ctor = true;
                    }
                }
                SpecialKind::Dtor => {
                    out.dtor_constexpr = Some(
                        spec.constexpr_kind() != ConstexprKind::None
                            || in_class_default && !vb_forbid,
                    );
                }
                _ => {}
            }
            // A copy/move whose subobject members need overload resolution
            // is trivial when every selected member is (clang
            // `SpecialMemberIsTrivial` from `CheckCompletedCXXClass`).
            let ovl = k != SpecialKind::Dtor
                && k.needs_ovl_bit().is_some_and(|n| class_trivial.contains(n))
                && !class_trivial.contains(DefinitionBits::POLYMORPHIC)
                && !vbases;
            if self.is_user_provided(f) {
                out.non_trivial |= k.bit();
            } else if ovl {
                let const_arg = self.special_params(f).first().is_some_and(|&p| {
                    let t = self.ast.decls.get(p).ty().unwrap_or(QualType::ERROR);
                    match self.ast.types.canonical_view(t) {
                        TypeView::LValueRef(t) => self.ast.types.effective_quals(t).is_const(),
                        _ => false,
                    }
                });
                if self.defaulted_is_trivial(f, k, true)
                    && self.selected_all_trivial(r, k, const_arg)
                {
                    self.ast.decls.mark_function(f, FnSpec::new().with_trivial(true));
                } else {
                    out.non_trivial |= k.bit();
                }
            } else if self.defaulted_is_trivial(f, k, class_trivial.contains(k.trivial_bit())) {
                self.ast.decls.mark_function(f, FnSpec::new().with_trivial(true));
            } else {
                out.non_trivial |= k.bit();
            }
        }
        out
    }

    /// The special-member definition bits of class `r` at its `}`, on top
    /// of `bits` from [`Sema::class_bits`] and
    /// [`Sema::virtual_class_bits`] (spec/sema-class.md §4.1–4.3, §4.6;
    /// clang `addedMember` / `addedClassSubobject`).
    #[allow(clippy::too_many_lines, reason = "one pass, as clang's addedMember")]
    pub(crate) fn special_member_bits(
        &mut self,
        r: DeclId,
        mut bits: DefinitionBits,
    ) -> DefinitionBits {
        use SpecialKind as K;
        let union = self.is_union(r);
        // The trivial bits so far say whether every subobject's
        // corresponding member is trivial (clang `HasTrivialSpecialMembers`);
        // a subobject without a move constructor / assignment has no
        // trivial one.
        let bases: Vec<BaseSpec> = self.ast.decls.bases(r, &self.ast.lists).copied().collect();
        let base_classes: Vec<(DeclId, bool)> =
            bases.iter().filter_map(|b| self.base_class(b).map(|d| (d, b.is_virtual))).collect();
        let fields = self.special_fields(r);
        let field_classes: Vec<(DeclId, DeclId)> = fields
            .iter()
            .filter_map(|&f| {
                let t = self.ast.decls.get(f).ty()?;
                if self.ast.types.is_reference(t) {
                    return None;
                }
                self.element_class(t).map(|c| (f, c))
            })
            .collect();
        let subobjects: Vec<DeclId> =
            base_classes.iter().map(|x| x.0).chain(field_classes.iter().map(|x| x.1)).collect();
        for &s in &subobjects {
            for k in [K::MoveCtor, K::MoveAssign] {
                if !self.has_trivial(s, k) {
                    bits = bits.without(k.trivial_bit());
                }
            }
        }
        // Subobjects whose members need overload resolution
        // (`addedClassSubobject`): direct bases, virtual bases, fields.
        let vbases = self.virtual_bases(r);
        let ovl_subobjects: Vec<DeclId> = base_classes
            .iter()
            .map(|x| x.0)
            .chain(vbases.iter().map(|x| x.0))
            .chain(field_classes.iter().map(|x| x.1))
            .collect();
        for s in ovl_subobjects {
            for k in [K::CopyCtor, K::MoveCtor, K::CopyAssign, K::MoveAssign] {
                if let Some(b) = k.needs_ovl_bit()
                    && !self.has_simple(s, k)
                {
                    bits = bits.with(b);
                }
            }
            if !self.has_simple(s, K::Dtor) {
                bits = bits
                    .with(DefinitionBits::NEEDS_OVL_COPY_CTOR)
                    .with(DefinitionBits::NEEDS_OVL_MOVE_CTOR)
                    .with(DefinitionBits::NEEDS_OVL_DTOR);
            }
        }
        // Fields ([class.copy.ctor]/10, [class.copy.assign]/7,
        // [class.union.general]/3).
        let d = self.class_def(r);
        let mut assign_deleted = 0u8;
        let mut uninit_fields = false;
        for &f in &fields {
            let Some(fty) = self.ast.decls.get(f).ty() else { continue };
            let has_init = self.field_has_nsdmi(f);
            let mutable = self.field_is_mutable(f);
            match self.ast.types.canonical_view(fty) {
                TypeView::RValueRef(_) => {
                    bits = bits.with(DefinitionBits::DEFAULTED_COPY_CTOR_DELETED);
                    assign_deleted |= K::CopyAssign.bit() | K::MoveAssign.bit();
                }
                TypeView::LValueRef(_) => {
                    assign_deleted |= K::CopyAssign.bit() | K::MoveAssign.bit();
                }
                _ => {}
            }
            let is_ref = self.ast.types.is_reference(fty);
            let quals = self.ast.types.effective_quals(fty);
            let elem = self.base_element(fty);
            let class = if is_ref { None } else { self.element_class(fty) };
            if is_ref && !has_init {
                bits = bits.with(DefinitionBits::HAS_UNINIT_REF_OR_CONST);
            }
            if !is_ref
                && quals.is_const()
                && !has_init
                && class.is_none_or(|c| !self.can_const_default_init(c))
            {
                bits = bits.with(DefinitionBits::HAS_UNINIT_REF_OR_CONST);
            }
            if !has_init && !mutable && !is_ref {
                match class {
                    Some(c) => {
                        if !self.can_const_default_init(c) {
                            uninit_fields = true;
                        }
                    }
                    None => {
                        if self.ast.types.as_record(elem).is_none() {
                            uninit_fields = true;
                        }
                    }
                }
            } else if is_ref && !has_init {
                uninit_fields = true;
            }
            let Some(c) = class else {
                if !is_ref && quals.is_const() {
                    assign_deleted |= K::CopyAssign.bit() | K::MoveAssign.bit();
                }
                continue;
            };
            if mutable {
                // A mutable member is copied from a non-const lvalue, which
                // may select another constructor or assignment operator
                // than its copy one (clang `addedMember`).
                bits = bits
                    .with(DefinitionBits::NEEDS_OVL_COPY_CTOR)
                    .with(DefinitionBits::NEEDS_OVL_COPY_ASSIGN);
            }
            if quals.contains(Quals::CONST) || quals.contains(Quals::VOLATILE) {
                bits = bits
                    .with(DefinitionBits::NEEDS_OVL_COPY_CTOR)
                    .with(DefinitionBits::NEEDS_OVL_MOVE_CTOR)
                    .with(DefinitionBits::NEEDS_OVL_COPY_ASSIGN)
                    .with(DefinitionBits::NEEDS_OVL_MOVE_ASSIGN);
            }
            let cb = self.defn_bits(c);
            if union {
                if !cb.contains(DefinitionBits::TRIVIAL_COPY_CTOR) {
                    bits = bits.with(DefinitionBits::DEFAULTED_COPY_CTOR_DELETED);
                }
                if !cb.contains(DefinitionBits::TRIVIAL_MOVE_CTOR) {
                    bits = bits.with(DefinitionBits::DEFAULTED_MOVE_CTOR_DELETED);
                }
                if !cb.contains(DefinitionBits::TRIVIAL_MOVE_ASSIGN) {
                    assign_deleted |= K::MoveAssign.bit();
                }
                // P3074R7 [class.dtor]/7.2.2: from C++26 only a variant
                // member with a default member initializer deletes the
                // destructor here; /7.2.1 (CWG3041) is decided when the
                // destructor is declared.
                if !cb.contains(DefinitionBits::TRIVIAL_DTOR)
                    && (!self.trivial_unions() || has_init)
                {
                    bits = bits.with(DefinitionBits::DEFAULTED_DTOR_DELETED);
                }
            }
            if self.is_anonymous_member(f) {
                for k in [K::CopyCtor, K::MoveCtor, K::CopyAssign, K::MoveAssign, K::Dtor] {
                    if let Some(b) = k.needs_ovl_bit()
                        && cb.contains(b)
                    {
                        bits = bits.with(b);
                    }
                    if let Some(b) = k.defaulted_deleted_bit()
                        && cb.contains(b)
                    {
                        bits = bits.with(b);
                    }
                }
                if let Some(m) = self.special.deleted_implicit.get(&c) {
                    assign_deleted |= m & (K::CopyAssign.bit() | K::MoveAssign.bit());
                }
            }
        }
        if assign_deleted != 0 {
            *self.special.deleted_implicit.entry(d).or_default() |= assign_deleted;
        }
        // Bases: const-default-constructibility.
        for &(b, _) in &base_classes {
            if !self.can_const_default_init(b) {
                uninit_fields = true;
            }
        }
        // `constexpr` default construction (§4.6), which the scan of the
        // in-class defaulted one reads.
        if self.std >= kiln_lex::Std::Cxx23
            && self.vbases_forbid_constexpr(bits)
            && self.vbase_flag_cxx23(&base_classes, &fields, true)
        {
            self.special.vbase_dtor_constexpr.insert(d);
        }
        if self.defaulted_default_ctor_constexpr(union, &base_classes, &fields) {
            bits = bits.with(DefinitionBits::DEFAULTED_DEFAULT_CTOR_CONSTEXPR);
        } else {
            bits = bits.without(DefinitionBits::DEFAULTED_DEFAULT_CTOR_CONSTEXPR);
        }
        // Own members.
        let decl = self.scan_declared(r, bits);
        let ud_bits = [
            (K::CopyCtor, DefinitionBits::USER_DECLARED_COPY_CTOR),
            (K::MoveCtor, DefinitionBits::USER_DECLARED_MOVE_CTOR),
            (K::CopyAssign, DefinitionBits::USER_DECLARED_COPY_ASSIGN),
            (K::MoveAssign, DefinitionBits::USER_DECLARED_MOVE_ASSIGN),
            (K::Dtor, DefinitionBits::USER_DECLARED_DTOR),
        ];
        for (k, b) in ud_bits {
            if decl.user & k.bit() != 0 {
                bits = bits.with(b);
            }
        }
        if decl.any_ctor {
            bits = bits.with(DefinitionBits::USER_DECLARED_CTOR);
        }
        // [dcl.init.aggr]/1: from C++20 any user-declared constructor
        // makes the class a non-aggregate; before, only a user-provided or
        // `explicit` one does (C++11-17 and DR1518; clang `addedMember`), so
        // `struct E { E() = delete; };` is an aggregate there (WP-1090).
        let non_aggregate_ctor = if self.std >= kiln_lex::Std::Cxx20 {
            decl.any_ctor
        } else {
            decl.provided_or_explicit_ctor
        };
        if non_aggregate_ctor {
            bits = bits.without(DefinitionBits::AGGREGATE);
        }
        if bits.contains(DefinitionBits::HAS_INHERITED_CTOR) {
            bits = bits.without(DefinitionBits::AGGREGATE);
        }
        if decl.provided_default_ctor {
            bits = bits.with(DefinitionBits::USER_PROVIDED_DEFAULT_CTOR);
        }
        let implicit = self.implicit_bits(d);
        // Triviality (clang `hasNonTrivial*` for copy/move/dtor,
        // `hasTrivialDefaultConstructor` for the default ctor).
        for k in K::ALL {
            let ht = bits.contains(k.trivial_bit());
            let trivial = if decl.user & k.bit() != 0 {
                decl.non_trivial & k.bit() == 0
            } else if needs_from(bits, implicit, k) {
                ht
            } else {
                // Not declared and never will be: a move (no non-trivial
                // one) or the default ctor (none at all).
                k != K::DefaultCtor
            };
            bits = if trivial { bits.with(k.trivial_bit()) } else { bits.without(k.trivial_bit()) };
        }
        let copyable = [K::CopyCtor, K::MoveCtor, K::CopyAssign, K::MoveAssign, K::Dtor]
            .iter()
            .all(|k| bits.contains(k.trivial_bit()));
        bits = if copyable {
            bits.with(DefinitionBits::TRIVIALLY_COPYABLE)
        } else {
            bits.without(DefinitionBits::TRIVIALLY_COPYABLE)
        };
        // Parameter forms of the implicit copy members
        // ([class.copy.ctor]/7, [class.copy.assign]/2).
        let mut ctor_const_direct = true;
        let mut ctor_const_vbase = true;
        let mut assign_const = true;
        for &(b, virt) in &base_classes {
            if !virt && !self.copy_has_const_param(b, K::CopyCtor) {
                ctor_const_direct = false;
            }
            if !self.copy_has_const_param(b, K::CopyAssign) {
                assign_const = false;
            }
        }
        for (v, _) in &vbases {
            if !self.copy_has_const_param(*v, K::CopyCtor) {
                ctor_const_vbase = false;
            }
        }
        for &(_, c) in &field_classes {
            if !self.copy_has_const_param(c, K::CopyCtor) {
                ctor_const_direct = false;
            }
            if !self.copy_has_const_param(c, K::CopyAssign) {
                assign_const = false;
            }
        }
        if ctor_const_direct && (bits.contains(DefinitionBits::ABSTRACT) || ctor_const_vbase) {
            bits = bits.with(DefinitionBits::IMPLICIT_COPY_CTOR_CONST_PARAM);
        }
        if assign_const {
            bits = bits.with(DefinitionBits::IMPLICIT_COPY_ASSIGN_CONST_PARAM);
        }
        // `constexpr` and literal types (§4.6, [basic.types.general]/10).
        let no_vbases = !self.vbases_forbid_constexpr(bits);
        let needs_default = needs_from(bits, implicit, K::DefaultCtor);
        if decl.constexpr_ctor
            || needs_default && bits.contains(DefinitionBits::DEFAULTED_DEFAULT_CTOR_CONSTEXPR)
        {
            bits = bits.with(DefinitionBits::HAS_CONSTEXPR_CTOR);
        }
        let constexpr_dtor = decl.dtor_constexpr.unwrap_or(no_vbases);
        let literal = constexpr_dtor
            && (bits.contains(DefinitionBits::AGGREGATE)
                || bits.contains(DefinitionBits::HAS_CONSTEXPR_CTOR)
                || bits.contains(DefinitionBits::TRIVIAL_DEFAULT_CTOR));
        if !literal {
            bits = bits.without(DefinitionBits::LITERAL).without(DefinitionBits::STRUCTURAL);
        }
        // [dcl.init.general]/8: const-default-constructible.
        if !uninit_fields || !(decl.defaulted_default_ctor || needs_default) {
            bits = bits.with(DefinitionBits::CAN_CONST_DEFAULT_INIT);
        }
        bits
    }

    /// The implicit declarations made at class `r`'s `}` (clang
    /// `AddImplicitlyDeclaredMembersToClass`; spec/sema-class.md §4.1),
    /// with its definition bits `bits` not yet stored. Returns the bits,
    /// updated by the declarations.
    pub(crate) fn finish_special_members(
        &mut self,
        r: DeclId,
        bits: DefinitionBits,
    ) -> DefinitionBits {
        use SpecialKind as K;
        self.special.finishing.push((r, bits));
        let flags = self.ast.decls.get(r).flags();
        if !flags.contains(DeclFlags::DEPENDENT_CTX) && !flags.contains(DeclFlags::INVALID) {
            let dynamic = bits.contains(DefinitionBits::POLYMORPHIC)
                || bits.contains(DefinitionBits::HAS_VIRTUAL_BASES);
            let inh_ctor = bits.contains(DefinitionBits::HAS_INHERITED_CTOR);
            let inh_assign = bits.contains(DefinitionBits::HAS_INHERITED_ASSIGN);
            // clang `HasInheritedDefaultConstructor`: an inherited
            // default constructor gives the class an implicit one.
            if inh_ctor
                && self.implicit_bits(r) & K::DefaultCtor.bit() == 0
                && !self.has_default_ctor_decl(r)
                && self.inherits_default_ctor(r)
            {
                self.declare_implicit(r, K::DefaultCtor);
            }
            for k in K::ALL {
                if !self.needs_implicit(r, k) {
                    continue;
                }
                let eager = match k {
                    K::DefaultCtor => inh_ctor,
                    K::CopyCtor | K::MoveCtor => needs_ovl_from(bits, k) || inh_ctor,
                    K::CopyAssign | K::MoveAssign => {
                        dynamic || needs_ovl_from(bits, k) || inh_assign
                    }
                    K::Dtor => dynamic || needs_ovl_from(bits, k),
                };
                if eager {
                    self.declare_implicit(r, k);
                }
            }
        }
        self.special.finishing.pop().map_or(bits, |x| x.1)
    }

    /// Declares the implicit special member `k` of class `r` if it needs
    /// one and it can be declared now (complete, not dependent; clang
    /// `LookupSpecialMember` / `ForceDeclarationOfImplicitMembers`).
    pub(crate) fn ensure_implicit(&mut self, class: DeclId, k: SpecialKind) {
        let Some(d) = self.ast.definition(class) else { return };
        let stored = self.ast.decls.record(d).map(|x| x.defn).unwrap_or_default();
        if !stored.contains(DefinitionBits::COMPLETE) {
            return;
        }
        // A class made invalid by its members or bases ([class.mem])
        // still has its special members (clang
        // `CanDeclareSpecialMemberFunction` asks only for a complete,
        // non-dependent class).
        if self.ast.decls.get(d).flags().contains(DeclFlags::DEPENDENT_CTX) {
            return;
        }
        if self.needs_implicit(d, k) {
            self.declare_implicit(d, k);
        }
    }

    /// Declares the implicit special members a lookup of `name` in class
    /// `class` can find (clang `DeclareImplicitMemberFunctionsWithName`,
    /// run by every class-scope lookup): `operator=` declares the copy and
    /// move assignment operators, a destructor name the destructor. Name
    /// lookup is `&self`, so the `&mut` entry points of a lookup that may
    /// name one (qualified-ids, member access, using-declarators) call
    /// this first. Hiding stops at `class`, which always declares
    /// `operator=` and a destructor, so its bases need nothing.
    pub(crate) fn declare_implicit_named(&mut self, class: DeclId, name: DeclName) {
        use kiln_ast::names::DeclNameKind;
        if self.kind_of(class) != DeclKind::CXXRecord {
            return;
        }
        match name.kind() {
            DeclNameKind::Operator if name.as_operator() == Some(OverloadedOperator::Equal) => {
                self.ensure_implicit(class, SpecialKind::CopyAssign);
                self.ensure_implicit(class, SpecialKind::MoveAssign);
            }
            DeclNameKind::Destructor => self.ensure_implicit(class, SpecialKind::Dtor),
            _ => {}
        }
    }

    /// [`Sema::declare_implicit_named`] for an unqualified lookup of
    /// `name` from the current scope: every enclosing class scope.
    pub(crate) fn declare_implicit_named_in_scope(&mut self, name: DeclName) {
        if name.as_operator() != Some(OverloadedOperator::Equal) {
            return;
        }
        let classes: Vec<DeclId> = self
            .scopes
            .iter()
            .filter(|s| s.kind == crate::scope::ScopeKind::Record)
            .filter_map(|s| s.entity)
            .collect();
        for c in classes {
            self.declare_implicit_named(c, name);
        }
    }

    /// Whether the implicit copy/move `k` of class `d`, which needs
    /// overload resolution, is trivial ([class.copy.ctor]/11,
    /// [class.copy.assign]/9; clang `SpecialMemberIsTrivial`): no virtual
    /// functions or bases, and the member selected for each direct base
    /// and class-type field is trivial. A failed selection does not make
    /// it non-trivial (the member is deleted then, as clang).
    fn selected_all_trivial(&mut self, d: DeclId, k: SpecialKind, const_arg: bool) -> bool {
        let bits = self.defn_bits(d);
        if bits.contains(DefinitionBits::POLYMORPHIC)
            || bits.contains(DefinitionBits::HAS_VIRTUAL_BASES)
        {
            return false;
        }
        let arg = if const_arg { Quals::CONST } else { Quals::NONE };
        let bases: Vec<BaseSpec> = self.ast.decls.bases(d, &self.ast.lists).copied().collect();
        // (class, argument quals, object quals), as `walk_subobjects`.
        let mut subs: Vec<(DeclId, Quals, Quals)> = bases
            .iter()
            .filter_map(|b| self.base_class(b))
            .map(|c| (c, arg, Quals::NONE))
            .collect();
        for f in self.special_fields(d) {
            let Some(t) = self.ast.decls.get(f).ty() else { continue };
            if self.ast.types.is_reference(t) {
                continue;
            }
            let elem = self.base_element(t);
            let Some(c) = self.element_class(elem) else { continue };
            let q = self.ast.types.effective_quals(elem) & Quals::CV;
            let rhs = if const_arg && !self.field_is_mutable(f) { q | Quals::CONST } else { q };
            subs.push((c, rhs, if k.is_assign() { q } else { Quals::NONE }));
        }
        subs.into_iter().all(|(c, rhs, lhs)| match self.select_special(c, k, rhs, lhs) {
            Selected::Found(m) | Selected::Deleted(m) => {
                self.ast.decls.function(m).is_some_and(|x| x.spec.is_trivial())
            }
            Selected::Unsupported => self.has_trivial(c, k),
            _ => true,
        })
    }

    /// Declares the implicit special member `k` of class `r` (clang
    /// `DeclareImplicit*`): public, inline, `Defaulted::Implicit`,
    /// `constexpr` per §4.6, trivial per the class bits, deleted per
    /// [`Sema::should_delete`], and the deferred exception specification.
    #[allow(clippy::too_many_lines, reason = "one step per property of the declaration")]
    pub(crate) fn declare_implicit(&mut self, r: DeclId, k: SpecialKind) -> DeclId {
        use SpecialKind as K;
        let d = self.class_def(r);
        let bits = self.defn_bits(d);
        *self.implicit_declared.entry(d).or_default() |= k.bit();
        let class_ty = self.class_type(d);
        let canon = self.ast.types.canonical(class_ty).ty();
        let copy_param = |s: &mut Self, konst: bool| {
            let t = if konst { s.ast.types.qualify(class_ty, Quals::CONST) } else { class_ty };
            s.ast.types.lvalue_ref_to(t)
        };
        let (kind, name, ret, params): (DeclKind, DeclName, QualType, Vec<QualType>) = match k {
            K::DefaultCtor => (
                DeclKind::CXXConstructor,
                self.ast.names.constructor(canon),
                QualType::VOID,
                vec![],
            ),
            K::CopyCtor => {
                let p =
                    copy_param(self, bits.contains(DefinitionBits::IMPLICIT_COPY_CTOR_CONST_PARAM));
                (
                    DeclKind::CXXConstructor,
                    self.ast.names.constructor(canon),
                    QualType::VOID,
                    vec![p],
                )
            }
            K::MoveCtor => {
                let p = self.ast.types.rvalue_ref_to(class_ty);
                (
                    DeclKind::CXXConstructor,
                    self.ast.names.constructor(canon),
                    QualType::VOID,
                    vec![p],
                )
            }
            K::CopyAssign | K::MoveAssign => {
                let p = if k == K::CopyAssign {
                    copy_param(
                        self,
                        bits.contains(DefinitionBits::IMPLICIT_COPY_ASSIGN_CONST_PARAM),
                    )
                } else {
                    self.ast.types.rvalue_ref_to(class_ty)
                };
                let ret = self.ast.types.lvalue_ref_to(class_ty);
                (DeclKind::CXXMethod, DeclName::operator(OverloadedOperator::Equal), ret, vec![p])
            }
            K::Dtor => {
                (DeclKind::CXXDestructor, self.ast.names.destructor(canon), QualType::VOID, vec![])
            }
        };
        let fty = self.ast.types.function(ret, &params, FuncInfo::new(), ExcSpec::None);
        let mut spec = FnSpec::new().with_inline(true).with_defaulted(Defaulted::Implicit);
        let const_arg = k == K::CopyCtor
            && bits.contains(DefinitionBits::IMPLICIT_COPY_CTOR_CONST_PARAM)
            || k == K::CopyAssign
                && bits.contains(DefinitionBits::IMPLICIT_COPY_ASSIGN_CONST_PARAM);
        if self.defaulted_is_constexpr(d, k, const_arg) {
            spec = spec.with_constexpr_kind(ConstexprKind::Constexpr);
        }
        let trivial = match k.needs_ovl_bit() {
            Some(n) if k != K::Dtor && bits.contains(n) => {
                let const_arg = k == K::CopyCtor
                    && bits.contains(DefinitionBits::IMPLICIT_COPY_CTOR_CONST_PARAM)
                    || k == K::CopyAssign
                        && bits.contains(DefinitionBits::IMPLICIT_COPY_ASSIGN_CONST_PARAM);
                let t = self.selected_all_trivial(d, k, const_arg);
                // The class's trivial bit now follows its declared member
                // (clang `addedMember` / `addedEligibleSpecialMember`), so a
                // class deriving from or containing `d` sees it trivial.
                let b = k.trivial_bit();
                self.set_finishing_bits(d, |x| if t { x.with(b) } else { x.without(b) });
                t
            }
            _ => bits.contains(k.trivial_bit()),
        };
        if trivial {
            spec = spec.with_trivial(true);
        }
        // The triviality bit just set above for a copy/move member declared
        // at `}` (clang `addedMember`: `HasTrivialSpecialMembers` or
        // `DeclaredNonTrivialSpecialMembers`) also decides the finishing
        // class's trivial copyability: a member of class type with only a
        // trivial copy constructor leaves the implicit move constructor
        // trivial.
        if k != K::Dtor
            && k != K::DefaultCtor
            && k.needs_ovl_bit().is_some_and(|n| bits.contains(n))
            && self.special.finishing.iter().any(|x| x.0 == d)
        {
            self.set_finishing_bits(d, |x| {
                let copyable = [K::CopyCtor, K::MoveCtor, K::CopyAssign, K::MoveAssign, K::Dtor]
                    .iter()
                    .all(|k| x.contains(k.trivial_bit()));
                if copyable {
                    x.with(DefinitionBits::TRIVIALLY_COPYABLE)
                } else {
                    x.without(DefinitionBits::TRIVIALLY_COPYABLE)
                }
            });
        }
        // [class.virtual]/2, /7: an implicit destructor overrides the
        // bases' virtual destructors, an implicit assignment operator a
        // base's virtual `operator=` with its parameter-type-list (clang
        // `DeclareImplicitCopyAssignment` → `AddOverriddenMethods`; such
        // a class is dynamic, so the operator is declared at `}`).
        let overridden = match k {
            K::Dtor => self.overridden_destructors(d),
            K::CopyAssign | K::MoveAssign => self.overridden_methods(d, name, fty),
            _ => Vec::new(),
        };
        if !overridden.is_empty() {
            spec = spec.with_virtual(true);
        }
        let loc = self.ast.decls.get(d).loc();
        let h = DeclHead::new(d, name, loc)
            .flags(DeclFlags::IMPLICIT)
            .access(kiln_ast::decl::Access::Public);
        // An implicit member is in a dependent context exactly when its
        // class is ([temp.dep]/1): the first use of a non-template class
        // may sit in a template definition (`S x;` in a function template
        // body declares `S::~S()`), and that must not make every later
        // `v.~S()` instantiation-dependent.
        let dep = self.ast.decls.get(d).has(DeclFlags::DEPENDENT_CTX);
        let saved = self.ast.templates.dependent_depth;
        self.ast.templates.dependent_depth = if dep { saved.max(1) } else { 0 };
        let f = self.builder().append_implicit_member(h, kind, fty, spec);
        let ps: Vec<DeclId> = params
            .iter()
            .enumerate()
            .map(|(i, &t)| {
                let i = u16::try_from(i).unwrap_or(u16::MAX);
                self.builder().param(f, DeclName::EMPTY, loc, t, i, 0)
            })
            .collect();
        self.ast.templates.dependent_depth = saved;
        if !ps.is_empty() {
            let _ = self.builder().set_params(f, &ps);
        }
        let _ = self.builder().set_overrides(f, &overridden);
        self.check_implicit_overrider(f, k, fty, loc, &overridden);
        let t = self.with_exc_spec(fty, ExcSpec::Deferred(f));
        self.ast.decls.set_ty(f, t);
        if self.should_delete(f, k, false) {
            self.ast.decls.mark_function(f, FnSpec::new().with_deleted(true));
            if let Some(b) = k.defaulted_deleted_bit() {
                if self.special.finishing.iter().any(|x| x.0 == d) {
                    self.set_finishing_bits(d, |x| x.with(b));
                } else {
                    *self.special.deleted_implicit.entry(d).or_default() |= k.bit();
                }
            }
        }
        f
    }

    /// The explicitly defaulted members of class `r` declared in it,
    /// checked once `r` is complete (clang `CheckCompletedCXXClass` →
    /// `CheckExplicitlyDefaultedSpecialMember`).
    pub(crate) fn check_pending_defaulted(&mut self, r: DeclId) {
        let (mut mine, rest): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.special.defaulted).into_iter().partition(|&(f, _)| {
                let ctx = if self.cmp_kind(f).is_some() {
                    self.ast.decls.lexical_ctx(f)
                } else {
                    self.ast.decls.get(f).ctx()
                };
                ctx.map(|c| self.class_def(c)) == Some(r)
            });
        self.special.defaulted = rest;
        // The destructor first: whether it is constexpr decides whether
        // the class is literal, which the other members' checks read
        // (clang `CheckCompletedCXXClass`).
        mine.sort_by_key(|&(f, _)| self.kind_of(f) != DeclKind::CXXDestructor);
        // [class.compare.default]: the secondary comparisons after the
        // primary ones (and the implicit `operator==`s, declared last),
        // whose deletedness they may depend on (clang
        // `CheckCompletedCXXClass`).
        let mut secondary = Vec::new();
        for (f, loc) in mine {
            if let Some(k) = self.cmp_kind(f) {
                if self.ast.decls.get(f).has(DeclFlags::INVALID) {
                    continue;
                }
                if k.is_primary() {
                    if self.check_defaulted_comparison(f, k, r) {
                        let _ = self.special.cmp.invalid.insert(self.ast.first(f));
                    }
                } else {
                    secondary.push((f, k));
                }
            } else if let Some(k) = self.special_kind_of(f) {
                self.check_defaulted(f, k, loc);
            }
        }
        for (f, k) in secondary {
            if self.check_defaulted_comparison(f, k, r) {
                let _ = self.special.cmp.invalid.insert(self.ast.first(f));
            }
        }
    }
}
