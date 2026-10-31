//! Built-in operator candidates ([over.built], [over.match.oper]/3.3;
//! spec/sema-overload.md §5.3): clang's `AddBuiltinOperatorCandidates`,
//! builder for builder and in its order, so candidate notes list the
//! same functions in the same order.
//!
//! The types an operand converts to (clang `BuiltinCandidateTypeSet`)
//! come from its type and, for a class, from its non-template
//! conversion functions; a complete class's conversion targets are
//! cached in [`OverloadScratch::conv_types`] (`sema.ovl.conv_types`).
//! Within one resolution an operand's conversion sequence to a
//! parameter type is computed once ([`OverloadScratch::builtin_memo`]).

use kiln_ast::names::OverloadedOperator as OO;
use kiln_ast::types::{BuiltinType, FloatFormat, Quals, TypeView};
use kiln_ast::{DeclId, ExprId, QualType};
use kiln_diag::Diag;
use kiln_source::Loc;

use super::oper::OperForm;
use super::{Candidate, CandidateFailure, Ics, IcsOptions, OverloadCandidateSet};
use crate::Sema;
use crate::conv::bases::{complete_class, is_derived_from};

/// The conversion targets of one complete class (clang
/// `getVisibleConversionFunctions`, templates skipped).
#[derive(Clone, Debug, Default)]
pub(crate) struct ConvTargets {
    /// Each conversion function's canonical result type, and whether it
    /// is `explicit`.
    pub targets: Vec<(QualType, bool)>,
    /// The `volatile`/`restrict` qualifiers seen along the result types'
    /// pointer chains (clang `CollectVRQualifiers`).
    pub vr: Quals,
}

/// The types one operand converts to (clang `BuiltinCandidateTypeSet`).
#[derive(Default)]
struct TypeSet {
    /// Pointer types (canonical), with their more-qualified variants.
    ptrs: Vec<QualType>,
    /// Pointer-to-member types (canonical), likewise.
    mptrs: Vec<QualType>,
    /// Enumeration types (canonical).
    enums: Vec<QualType>,
    /// `std::nullptr_t` is among them.
    nullptr: bool,
    /// A non-class type is among them.
    non_record: bool,
    /// An arithmetic or enumeration type is among them.
    arith_or_enum: bool,
}

/// clang's qualifier numbering (`Const = 1, Restrict = 2, Volatile = 4`),
/// whose numeric order the more-qualified variants follow, as kiln
/// qualifiers.
fn clang_cvr(cvr: u8) -> Quals {
    let mut q = 0;
    if cvr & 1 != 0 {
        q |= Quals::CONST.bits();
    }
    if cvr & 2 != 0 {
        q |= Quals::RESTRICT.bits();
    }
    if cvr & 4 != 0 {
        q |= Quals::VOLATILE.bits();
    }
    Quals::from_bits(q)
}

/// kiln qualifiers in clang's numbering.
fn to_clang_cvr(q: Quals) -> u8 {
    u8::from(q.is_const()) | (u8::from(q.is_restrict()) << 1) | (u8::from(q.is_volatile()) << 2)
}

/// The arithmetic types of [over.built] in clang's order, and the ends
/// of its ranges.
struct Arith {
    types: Vec<QualType>,
    /// The first integral type (= the first promoted integral type).
    first_integral: usize,
    /// The end of the promoted types.
    last_promoted: usize,
}

/// One candidate being added.
struct Params<'p> {
    tys: &'p [QualType],
    /// The first operand is a built-in assignment's left operand
    /// ([over.match.oper]/4).
    assign: bool,
    /// The leading operands contextually converted to `bool`.
    ctx_bool: usize,
}

impl Sema<'_, '_> {
    /// The arithmetic types (clang `InitArithmeticTypes`).
    fn arith_types(&self) -> Arith {
        let mut types = vec![QualType::FLOAT, QualType::DOUBLE, QualType::LONGDOUBLE];
        // `__float128` exists on the x86 targets (whose `long double`
        // is the x87 format).
        if self.target.long_double == FloatFormat::X87Extended {
            types.push(QualType::FLOAT128);
        }
        let first_integral = types.len();
        types.extend([
            QualType::INT,
            QualType::LONG,
            QualType::LONGLONG,
            QualType::INT128,
            QualType::UINT,
            QualType::ULONG,
            QualType::ULONGLONG,
            QualType::UINT128,
        ]);
        let last_promoted = types.len();
        types.extend([
            QualType::BOOL,
            QualType::CHAR,
            QualType::WCHAR,
            QualType::CHAR8,
            QualType::CHAR16,
            QualType::CHAR32,
            QualType::SCHAR,
            QualType::SHORT,
            QualType::UCHAR,
            QualType::USHORT,
        ]);
        Arith { types, first_integral, last_promoted }
    }

    /// The cached conversion targets of class `rec` (`None` when it is
    /// not complete).
    fn class_conv_targets(&mut self, rec: DeclId) -> Option<ConvTargets> {
        let def = complete_class(self, rec)?;
        if let Some(c) = self.ovl.conv_types.get(&def) {
            return Some(c.clone());
        }
        let ty = QualType::unqual(self.ast.types.record_type(def));
        let mut c = ConvTargets::default();
        for (_, f) in self.conversion_functions(ty) {
            let t = self.ast.types.canonical(self.conversion_result_type(f));
            c.targets.push((t, self.fn_spec(f).is_explicit()));
            // clang `CollectVRQualifiers`: down the pointer chain.
            let mut t = match self.ast.types.canonical_view(t) {
                TypeView::LValueRef(p) | TypeView::RValueRef(p) => self.ast.types.canonical(p),
                _ => t,
            };
            loop {
                if t.quals().is_restrict() {
                    c.vr = c.vr | Quals::RESTRICT;
                }
                let next = match self.ast.types.canonical_view(t) {
                    TypeView::Pointer(p) | TypeView::MemberPointer { pointee: p, .. } => Some(p),
                    _ => None,
                };
                if let Some(p) = next {
                    t = self.ast.types.canonical(p);
                }
                if t.quals().is_volatile() {
                    c.vr = c.vr | Quals::VOLATILE;
                }
                if next.is_none() || c.vr.contains(Quals::VOLATILE | Quals::RESTRICT) {
                    break;
                }
            }
        }
        self.ovl.conv_types.insert(def, c.clone());
        Some(c)
    }

    /// clang `CollectVRQualifiers` for an operand of type `ty`.
    fn visible_vr(&mut self, ty: QualType) -> Quals {
        let rec = match self.ast.types.canonical_view(ty) {
            TypeView::MemberPointer { class, .. } => {
                self.ast.types.as_record(QualType::unqual(class))
            }
            _ => self.ast.types.as_record(ty),
        };
        let Some(rec) = rec else { return Quals::VOLATILE | Quals::RESTRICT };
        self.class_conv_targets(rec).map_or(Quals::NONE, |c| c.vr)
    }

    /// clang `AddTypesConvertedFrom`.
    fn add_types_converted_from(
        &mut self,
        set: &mut TypeSet,
        ty: QualType,
        udc: bool,
        explicit: bool,
        vis: Quals,
    ) {
        let mut t = self.ast.types.canonical(ty);
        if let TypeView::LValueRef(p) | TypeView::RValueRef(p) = self.ast.types.canonical_view(t) {
            t = self.ast.types.canonical(p);
        }
        if self.ast.types.is_array(t) {
            let e = self.ast.types.element(t).unwrap_or(QualType::ERROR);
            let p = self.ast.types.pointer_to(e);
            t = self.ast.types.canonical(p);
        }
        let t = t.without_quals();
        let rec = self.ast.types.as_record(t);
        set.non_record |= rec.is_none();
        set.arith_or_enum |= self.ast.types.is_arithmetic(t);
        match self.ast.types.canonical_view(t) {
            TypeView::Pointer(p) => {
                if set.ptrs.contains(&t) {
                    return;
                }
                set.ptrs.push(t);
                let p = self.ast.types.canonical(p);
                if self.ast.types.is_array(p) {
                    return;
                }
                let base = to_clang_cvr(p.quals());
                let ptr_like = self.ast.types.is_pointer(p) || self.ast.types.is_reference(p);
                for cvr in base + 1..=7 {
                    if cvr | base != cvr
                        || (cvr & 4 != 0 && !vis.is_volatile())
                        || (cvr & 2 != 0 && (!vis.is_restrict() || !ptr_like))
                    {
                        continue;
                    }
                    let q = self.ast.types.qualify(p, clang_cvr(cvr));
                    let v = self.ast.types.pointer_to(q);
                    let v = self.ast.types.canonical(v);
                    if !set.ptrs.contains(&v) {
                        set.ptrs.push(v);
                    }
                }
            }
            TypeView::MemberPointer { pointee, class } => {
                if set.mptrs.contains(&t) {
                    return;
                }
                set.mptrs.push(t);
                let p = self.ast.types.canonical(pointee);
                if self.ast.types.is_array(p) {
                    return;
                }
                let base = to_clang_cvr(p.quals());
                for cvr in base + 1..=7 {
                    if cvr | base != cvr {
                        continue;
                    }
                    let q = self.ast.types.qualify(p, clang_cvr(cvr));
                    let v = self.ast.types.member_pointer(q, class);
                    let v = self.ast.types.canonical(v);
                    if !set.mptrs.contains(&v) {
                        set.mptrs.push(v);
                    }
                }
            }
            TypeView::Enum(_) => {
                set.arith_or_enum = true;
                if !set.enums.contains(&t) {
                    set.enums.push(t);
                }
            }
            TypeView::Builtin(BuiltinType::NullPtr) => set.nullptr = true,
            _ => {
                let Some(r) = rec.filter(|_| udc) else { return };
                if !self.require_complete_silent(t, Loc::INVALID) {
                    return;
                }
                let Some(c) = self.class_conv_targets(r) else { return };
                for (target, is_explicit) in c.targets {
                    if explicit || !is_explicit {
                        self.add_types_converted_from(set, target, false, false, vis);
                    }
                }
            }
        }
    }

    /// Adds the built-in candidates of `op` for operands `ops` of types
    /// `types` ([over.match.oper]/3.3; clang `AddBuiltinOperatorCandidates`).
    pub(crate) fn add_builtin_candidates(
        &mut self,
        set: &mut OverloadCandidateSet,
        op: OO,
        form: OperForm,
        ops: &[ExprId],
        types: &[QualType],
    ) {
        if form == OperForm::Call || ops.len() > 2 || matches!(op, OO::Comma | OO::Arrow) {
            return;
        }
        set.builtin_op = Some(op);
        let mut vis = Quals::CONST;
        for &t in types {
            vis = vis | self.visible_vr(t);
        }
        let logical = matches!(op, OO::Exclaim | OO::AmpAmp | OO::PipePipe);
        let mut sets: Vec<TypeSet> = Vec::with_capacity(types.len());
        for &t in types {
            let mut s = TypeSet::default();
            self.add_types_converted_from(&mut s, t, true, logical, vis);
            sets.push(s);
        }
        if !sets.iter().any(|s| s.non_record) && !logical {
            return;
        }
        let arith_or_enum = sets.iter().any(|s| s.arith_or_enum);
        let mut memo = std::mem::take(&mut self.ovl.builtin_memo);
        memo.clear();
        let mut b = Builder { set, memo: &mut memo, vis, arith_or_enum, sets: &sets };
        let n = types.len();
        let ar = self.arith_types();
        match op {
            OO::Plus | OO::Minus if n == 1 => {
                if op == OO::Plus {
                    for &p in &b.sets[0].ptrs {
                        self.add_builtin(&mut b, &[p]);
                    }
                }
                if b.arith_or_enum {
                    for &t in &ar.types[..ar.last_promoted] {
                        self.add_builtin(&mut b, &[t]);
                    }
                }
            }
            OO::Plus | OO::Minus => {
                self.binary_plus_minus_pointer(&mut b, op);
                self.arith_pairs(&mut b, &ar, 0);
            }
            OO::Star if n == 1 => {
                for &p in &b.sets[0].ptrs {
                    let pointee = self.ast.types.pointee(p).unwrap_or(QualType::ERROR);
                    if self.ast.types.is_function(pointee) {
                        let info = self.ast.types.proto(pointee).map(|f| f.info());
                        if info.is_some_and(|i| {
                            !i.this_quals().is_empty()
                                || i.ref_qual() != kiln_ast::types::RefQual::None
                        }) {
                            continue;
                        }
                    } else if !self.is_object_type(pointee) {
                        continue;
                    }
                    self.add_builtin(&mut b, &[p]);
                }
            }
            OO::Star | OO::Slash => self.arith_pairs(&mut b, &ar, 0),
            OO::PlusPlus | OO::MinusMinus => {
                if b.arith_or_enum {
                    for (i, &t) in ar.types.iter().enumerate() {
                        if i == ar.last_promoted {
                            // `bool`: not decremented, and not incremented
                            // since C++17.
                            continue;
                        }
                        let (v, r) = (vis.is_volatile(), vis.is_restrict());
                        self.inc_dec_style(&mut b, t, v, r, n);
                    }
                }
                let ptrs = b.sets[0].ptrs.clone();
                for p in ptrs {
                    let pointee = self.ast.types.pointee(p).unwrap_or(QualType::ERROR);
                    if !self.is_object_type(pointee) {
                        continue;
                    }
                    let v = !p.quals().is_volatile() && vis.is_volatile();
                    let r = !p.quals().is_restrict() && vis.is_restrict();
                    self.inc_dec_style(&mut b, p, v, r, n);
                }
            }
            OO::EqualEqual | OO::ExclaimEqual => {
                let mut added: Vec<QualType> = Vec::new();
                for i in 0..n {
                    for &m in &b.sets[i].mptrs {
                        if !added.contains(&m) {
                            added.push(m);
                            self.add_builtin(&mut b, &[m, m]);
                        }
                    }
                    if b.sets[i].nullptr && !added.contains(&QualType::NULLPTR) {
                        added.push(QualType::NULLPTR);
                        self.add_builtin(&mut b, &[QualType::NULLPTR, QualType::NULLPTR]);
                    }
                }
                self.pointer_or_enum_pairs(&mut b, false);
                self.arith_pairs(&mut b, &ar, 0);
            }
            OO::Less | OO::Greater | OO::LessEqual | OO::GreaterEqual => {
                self.pointer_or_enum_pairs(&mut b, false);
                self.arith_pairs(&mut b, &ar, 0);
            }
            OO::Spaceship => {
                self.pointer_or_enum_pairs(&mut b, true);
                self.arith_pairs(&mut b, &ar, 0);
            }
            OO::Percent | OO::Caret | OO::Pipe | OO::LessLess | OO::GreaterGreater => {
                self.arith_pairs(&mut b, &ar, ar.first_integral);
            }
            OO::Amp if n == 2 => self.arith_pairs(&mut b, &ar, ar.first_integral),
            OO::Tilde if b.arith_or_enum => {
                for &t in &ar.types[ar.first_integral..ar.last_promoted] {
                    self.add_builtin(&mut b, &[t]);
                }
            }
            OO::Equal | OO::PlusEqual | OO::MinusEqual | OO::StarEqual | OO::SlashEqual => {
                let is_eq = op == OO::Equal;
                if is_eq {
                    self.assign_mptr_or_enum(&mut b);
                }
                if matches!(op, OO::Equal | OO::PlusEqual | OO::MinusEqual) {
                    self.assign_pointer(&mut b, is_eq);
                }
                if b.arith_or_enum {
                    self.assign_arith(&mut b, &ar, 0, is_eq);
                }
            }
            OO::PercentEqual
            | OO::LessLessEqual
            | OO::GreaterGreaterEqual
            | OO::AmpEqual
            | OO::CaretEqual
            | OO::PipeEqual => {
                if b.arith_or_enum {
                    self.assign_arith(&mut b, &ar, ar.first_integral, false);
                }
            }
            OO::Exclaim => {
                self.add_builtin_with(&mut b, &Params {
                    tys: &[QualType::BOOL],
                    assign: false,
                    ctx_bool: 1,
                });
            }
            OO::AmpAmp | OO::PipePipe => {
                self.add_builtin_with(&mut b, &Params {
                    tys: &[QualType::BOOL, QualType::BOOL],
                    assign: false,
                    ctx_bool: 2,
                });
            }
            OO::Subscript if n == 2 => {
                let ptrdiff = QualType::unqual(self.target.ptrdiff_t);
                for i in 0..2 {
                    for &p in &b.sets[i].ptrs {
                        let pointee = self.ast.types.pointee(p).unwrap_or(QualType::ERROR);
                        if !self.is_object_type(pointee) {
                            continue;
                        }
                        let tys = if i == 0 { [p, ptrdiff] } else { [ptrdiff, p] };
                        self.add_builtin(&mut b, &tys);
                    }
                }
            }
            OO::ArrowStar if n == 2 => self.arrow_star(&mut b),
            _ => {}
        }
        self.ovl.builtin_memo = memo;
    }

    /// Promoted arithmetic (or, from `first`, promoted integral) `L × R`
    /// (clang `addGenericBinaryArithmeticOverloads`,
    /// `addBinaryBitwiseArithmeticOverloads`).
    fn arith_pairs(&mut self, b: &mut Builder<'_>, ar: &Arith, first: usize) {
        if !b.arith_or_enum {
            return;
        }
        for &l in &ar.types[first..ar.last_promoted] {
            for &r in &ar.types[first..ar.last_promoted] {
                self.add_builtin(b, &[l, r]);
            }
        }
    }

    /// clang `addPlusPlusMinusMinusStyleOverloads`.
    fn inc_dec_style(&mut self, b: &mut Builder<'_>, t: QualType, v: bool, r: bool, n: usize) {
        let one = |s: &mut Self, b: &mut Builder<'_>, q: Quals| {
            let qt = s.ast.types.qualify(t, q);
            let lr = s.ast.types.lvalue_ref_to(qt);
            let tys = [lr, QualType::INT];
            s.add_builtin(b, &tys[..n]);
        };
        one(self, b, Quals::NONE);
        if v {
            one(self, b, Quals::VOLATILE);
        }
        if r && self.ast.types.is_pointer(t) && !t.quals().is_restrict() {
            one(self, b, Quals::RESTRICT);
            if v {
                one(self, b, Quals::VOLATILE | Quals::RESTRICT);
            }
        }
    }

    /// clang `addBinaryPlusOrMinusPointerOverloads`.
    fn binary_plus_minus_pointer(&mut self, b: &mut Builder<'_>, op: OO) {
        let ptrdiff = QualType::unqual(self.target.ptrdiff_t);
        let mut added: Vec<QualType> = Vec::new();
        for arg in 0..2 {
            let mut tys = [ptrdiff, ptrdiff];
            let ptrs = b.sets[arg].ptrs.clone();
            for p in ptrs {
                let pointee = self.ast.types.pointee(p).unwrap_or(QualType::ERROR);
                if !self.is_object_type(pointee) {
                    continue;
                }
                tys[arg] = p;
                if arg == 0 || op == OO::Plus {
                    self.add_builtin(b, &tys);
                }
                if op == OO::Minus {
                    if added.contains(&p) {
                        continue;
                    }
                    added.push(p);
                    self.add_builtin(b, &[p, p]);
                }
            }
        }
    }

    /// clang `addGenericBinaryPointerOrEnumeralOverloads`, with the
    /// [over.match.oper]/3.3.4 rule: no `(E, E)` built-in where a
    /// non-template candidate takes `(E, E)`.
    fn pointer_or_enum_pairs(&mut self, b: &mut Builder<'_>, spaceship: bool) {
        let mut user: Vec<(QualType, QualType)> = Vec::new();
        if b.sets.iter().any(|s| !s.enums.is_empty()) {
            let mut params = Vec::new();
            for c in &b.set.cands {
                let Some(f) = c.decl.filter(|_| c.viable && c.template.is_none()) else {
                    continue;
                };
                self.fn_param_types(f, &mut params);
                if params.len() != 2 {
                    continue;
                }
                let (mut p0, mut p1) = (params[0], params[1]);
                if c.rewrite.is_reversed() {
                    std::mem::swap(&mut p0, &mut p1);
                }
                let (p0, p1) = (
                    self.ast.types.canonical(p0).without_quals(),
                    self.ast.types.canonical(p1).without_quals(),
                );
                if self.ast.types.as_enum(p0).is_some() && self.ast.types.as_enum(p1).is_some() {
                    user.push((p0, p1));
                }
            }
        }
        let mut added: Vec<QualType> = Vec::new();
        for i in 0..b.sets.len() {
            let ptrs = b.sets[i].ptrs.clone();
            for p in ptrs {
                if added.contains(&p) {
                    continue;
                }
                added.push(p);
                let pointee = self.ast.types.pointee(p).unwrap_or(QualType::ERROR);
                if spaceship && self.ast.types.is_function(pointee) {
                    continue;
                }
                self.add_builtin(b, &[p, p]);
            }
            let enums = b.sets[i].enums.clone();
            for e in enums {
                if added.contains(&e) {
                    continue;
                }
                added.push(e);
                if user.contains(&(e, e)) {
                    continue;
                }
                self.add_builtin(b, &[e, e]);
            }
        }
    }

    /// clang `AddBuiltinAssignmentOperatorCandidates`.
    fn assign_style(&mut self, b: &mut Builder<'_>, t: QualType) {
        let lr = self.ast.types.lvalue_ref_to(t);
        self.add_builtin_with(b, &Params { tys: &[lr, t], assign: true, ctx_bool: 0 });
        if !t.quals().is_volatile() {
            let v = self.ast.types.qualify(t, Quals::VOLATILE);
            let lr = self.ast.types.lvalue_ref_to(v);
            self.add_builtin_with(b, &Params { tys: &[lr, t], assign: true, ctx_bool: 0 });
        }
    }

    /// clang `addAssignmentMemberPointerOrEnumeralOverloads`.
    fn assign_mptr_or_enum(&mut self, b: &mut Builder<'_>) {
        let mut added: Vec<QualType> = Vec::new();
        for i in 0..2.min(b.sets.len()) {
            let tys: Vec<QualType> =
                b.sets[i].enums.iter().chain(&b.sets[i].mptrs).copied().collect();
            for t in tys {
                if added.contains(&t) {
                    continue;
                }
                added.push(t);
                self.assign_style(b, t);
            }
        }
    }

    /// clang `addAssignmentPointerOverloads`.
    fn assign_pointer(&mut self, b: &mut Builder<'_>, is_eq: bool) {
        let ptrdiff = QualType::unqual(self.target.ptrdiff_t);
        let mut added: Vec<QualType> = Vec::new();
        let vis = b.vis;
        let variants = |s: &mut Self, b: &mut Builder<'_>, p: QualType, rhs: QualType| {
            let add = |s: &mut Self, b: &mut Builder<'_>, q: Quals| {
                let qt = s.ast.types.qualify(p, q);
                let lr = s.ast.types.lvalue_ref_to(qt);
                s.add_builtin_with(b, &Params { tys: &[lr, rhs], assign: is_eq, ctx_bool: 0 });
            };
            add(s, b, Quals::NONE);
            let need_v = !p.quals().is_volatile() && vis.is_volatile();
            if need_v {
                add(s, b, Quals::VOLATILE);
            }
            if !p.quals().is_restrict() && vis.is_restrict() {
                add(s, b, Quals::RESTRICT);
                if need_v {
                    add(s, b, Quals::VOLATILE | Quals::RESTRICT);
                }
            }
        };
        let ptrs = b.sets[0].ptrs.clone();
        for p in ptrs {
            if is_eq {
                added.push(p);
            } else {
                let pointee = self.ast.types.pointee(p).unwrap_or(QualType::ERROR);
                if !self.is_object_type(pointee) {
                    continue;
                }
            }
            variants(self, b, p, if is_eq { p } else { ptrdiff });
        }
        if is_eq && b.sets.len() > 1 {
            let ptrs = b.sets[1].ptrs.clone();
            for p in ptrs {
                if added.contains(&p) {
                    continue;
                }
                added.push(p);
                variants(self, b, p, p);
            }
        }
    }

    /// clang `addAssignmentArithmeticOverloads` (`first` 0) and
    /// `addAssignmentIntegralOverloads`: every `L` from `first` ×
    /// promoted `R`, the `volatile` left operand first.
    fn assign_arith(&mut self, b: &mut Builder<'_>, ar: &Arith, first: usize, is_eq: bool) {
        let quals: &[Quals] =
            if b.vis.is_volatile() { &[Quals::VOLATILE, Quals::NONE] } else { &[Quals::NONE] };
        for &l in &ar.types[first..] {
            for &r in &ar.types[first..ar.last_promoted] {
                for &q in quals {
                    let lq = self.ast.types.qualify(l, q);
                    let lr = self.ast.types.lvalue_ref_to(lq);
                    self.add_builtin_with(b, &Params { tys: &[lr, r], assign: is_eq, ctx_bool: 0 });
                }
            }
        }
    }

    /// clang `addArrowStarOverloads`.
    fn arrow_star(&mut self, b: &mut Builder<'_>) {
        let vis = b.vis;
        let ptrs = b.sets[0].ptrs.clone();
        for p in ptrs {
            let pointee = self.ast.types.pointee(p).unwrap_or(QualType::ERROR);
            let pointee = self.ast.types.canonical(pointee);
            let q1 = pointee.quals();
            let TypeView::Record(c1) = self.ast.types.canonical_view(pointee.without_quals())
            else {
                continue;
            };
            if (!vis.is_volatile() && q1.is_volatile()) || (!vis.is_restrict() && q1.is_restrict())
            {
                continue;
            }
            let mptrs = b.sets[1].mptrs.clone();
            for m in mptrs {
                let TypeView::MemberPointer { pointee: t, class } = self.ast.types.canonical_view(m)
                else {
                    continue;
                };
                let Some(c2) = self.ast.types.as_record(QualType::unqual(class)) else { continue };
                if self.ast.first(c1) != self.ast.first(c2) && !is_derived_from(self, c1, c2) {
                    break;
                }
                let tq = self.ast.types.canonical(t).quals();
                if (!vis.is_volatile() && tq.is_volatile()) || (!vis.is_restrict() && tq.is_restrict())
                {
                    continue;
                }
                self.add_builtin(b, &[p, m]);
            }
        }
    }

    /// Adds the built-in candidate with parameter types `tys`.
    fn add_builtin(&mut self, b: &mut Builder<'_>, tys: &[QualType]) {
        self.add_builtin_with(b, &Params { tys, assign: false, ctx_bool: 0 });
    }

    /// clang `AddBuiltinCandidate`: the operands are copy-initialized
    /// (not in overload resolution: a deleted reference conversion is
    /// still a conversion).
    fn add_builtin_with(&mut self, b: &mut Builder<'_>, p: &Params<'_>) {
        self.ovl.stats.candidates += 1;
        self.ovl.stats.builtin_candidates += 1;
        let set = &mut *b.set;
        let start = u32::try_from(set.ics.len()).unwrap_or(u32::MAX);
        let mut cand = Candidate::builtin([
            p.tys[0],
            p.tys.get(1).copied().unwrap_or(QualType::ERROR),
        ]);
        for (i, &to) in p.tys.iter().enumerate() {
            let opts = if i < p.ctx_bool {
                IcsOptions::ALLOW_EXPLICIT
            } else if i == 0 && p.assign {
                IcsOptions::SUPPRESS_UDC.with(IcsOptions::NO_TEMPORARY)
            } else {
                IcsOptions::NONE
            };
            let key = (i as u8, to, opts.bits());
            let x = match b.memo.get(&key) {
                Some(&x) => x,
                None => {
                    let a = if i == 0 { b.set.object } else { b.set.args.get(i - 1).copied() };
                    let Some(a) = a else { break };
                    let x = self.try_implicit_conversion(&a, to, opts);
                    b.memo.insert(key, x);
                    x
                }
            };
            let bad = x.is_bad();
            b.set.ics.push(x);
            if bad {
                cand.viable = false;
                cand.failure = CandidateFailure::BadConversion { arg: i as u16 };
                break;
            }
        }
        let end = u32::try_from(b.set.ics.len()).unwrap_or(u32::MAX);
        cand.ics = start..end;
        b.set.cands.push(cand);
    }

    /// The converted operands of the selected built-in candidate `b`
    /// ([over.match.oper]/11): a class operand by its user-defined
    /// conversion (the built-in operator applies its own standard
    /// conversions); `None` after diagnosing an ambiguous one.
    pub(crate) fn builtin_operand_conversions(
        &mut self,
        set: &OverloadCandidateSet,
        b: u32,
        ops: &[ExprId],
    ) -> Option<Vec<ExprId>> {
        let cand = set.cand(b);
        let mut out = ops.to_vec();
        for (i, x) in set.ics_of(b).iter().enumerate() {
            let Some(&e) = ops.get(i) else { break };
            match *x {
                Ics::UserDefined(u) => {
                    if let (Some(f), Some(found)) = (u.func, u.found)
                        && u.via == super::ics::UdcVia::ConvFn
                    {
                        out[i] = self.call_conversion_function(e, f, found, set.loc);
                    }
                }
                Ics::Ambiguous { .. } => {
                    let a = self.arg_info(e);
                    let opts =
                        if i == 0 && set.builtin_op == Some(OO::Exclaim) || cand_ctx_bool(set, i) {
                            IcsOptions::ALLOW_EXPLICIT
                        } else {
                            IcsOptions::NONE
                        };
                    let err = self.ambiguous_conversion(&a, cand.builtin[i], opts);
                    let msg = err.diag_at(self, set.loc);
                    self.emit(msg);
                    return None;
                }
                _ => {}
            }
        }
        Some(out)
    }

    /// The notes clang gives a viable built-in candidate `c` (clang
    /// `NoteBuiltinOperatorCandidate`), after, when `ambiguities`, the
    /// ambiguous conversions of its operands
    /// (`NoteAmbiguousUserConversions`).
    pub(crate) fn builtin_notes(
        &mut self,
        set: &OverloadCandidateSet,
        c: u32,
        ambiguities: bool,
    ) -> Vec<Diag> {
        let mut out = Vec::new();
        let cand = set.cand(c);
        let n = cand.ics.len();
        if ambiguities {
            for (i, x) in set.ics_of(c).iter().enumerate() {
                if x.is_bad() {
                    break;
                }
                if let Ics::Ambiguous { .. } = x {
                    let a = if i == 0 { set.object } else { set.args.get(i - 1).copied() };
                    let Some(a) = a else { continue };
                    let to = cand.builtin[i];
                    let opts =
                        if cand_ctx_bool(set, i) { IcsOptions::ALLOW_EXPLICIT } else { IcsOptions::NONE };
                    let m = self
                        .diag(kiln_diag::DiagId::note_ambiguous_type_conversion, set.loc)
                        .arg(self.ty_arg(a.ty))
                        .arg(self.ty_arg(to));
                    out.push(m);
                    out.extend(self.udc_notes_of(&a, to, opts, true));
                }
            }
        }
        let spelling = set.builtin_op.map_or("", OO::spelling);
        let mut s = format!("operator{spelling}(");
        for (i, &t) in cand.builtin[..n].iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(&self.c_ty_str(t));
        }
        s.push(')');
        out.push(self.diag(kiln_diag::DiagId::note_ovl_builtin_candidate, set.loc).arg(s));
        out
    }

    /// `t` as clang's `QualType::getAsString` prints it (the C policy:
    /// `struct S *`, `_Bool`).
    fn c_ty_str(&self, t: QualType) -> String {
        kiln_ast::types::print_type(
            &self.ast.types,
            &crate::names::SemaNames::new(self),
            t,
            kiln_ast::types::PrintOptions::C_DEFAULT,
        )
    }
}

/// Operand `i` of a built-in candidate of `set` is contextually
/// converted to `bool` (`!`, `&&`, `||`).
fn cand_ctx_bool(set: &OverloadCandidateSet, _i: usize) -> bool {
    matches!(set.builtin_op, Some(OO::Exclaim | OO::AmpAmp | OO::PipePipe))
}

/// The state of one [`Sema::add_builtin_candidates`].
struct Builder<'b> {
    set: &'b mut OverloadCandidateSet,
    memo: &'b mut kiln_source::fx::FxHashMap<(u8, QualType, u8), Ics>,
    vis: Quals,
    arith_or_enum: bool,
    sets: &'b [TypeSet],
}
