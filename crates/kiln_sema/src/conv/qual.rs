//! Qualification conversions ([conv.qual]) and function pointer
//! conversions ([conv.fctptr]).

use kiln_ast::types::{ExcSpec, Quals, TypeView};
use kiln_ast::{QualType, TypeId};

use crate::Sema;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Level {
    Pointer,
    MemberPointer(TypeId),
    Array(Option<u64>),
}

/// The relation between two types under [conv.qual].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum QualRelation {
    /// The types are not similar ([conv.qual]/2).
    NotSimilar,
    /// The types are similar.
    Similar {
        /// The types are identical up to top-level cv-qualifiers.
        identical: bool,
        /// A prvalue of the first converts to the second by a
        /// qualification conversion ([conv.qual]/3).
        convertible: bool,
        /// The conversion would drop a qualifier of the first pointee
        /// level (clang: "discards qualifiers").
        drops_outer: bool,
        /// The conversion would drop a qualifier at a deeper level, or
        /// add one without `const` on every level above (clang:
        /// "discards qualifiers in nested pointer types").
        drops_nested: bool,
    },
}

/// The cv-qualifiers of `q` for [conv.qual]: an array type is
/// cv-qualified exactly as its elements are ([basic.type.qualifier]/3).
fn effective_quals(s: &Sema<'_, '_>, q: QualType) -> Quals {
    let mut quals = q.quals();
    let mut cur = q;
    for _ in 0..4096 {
        match s.ast.types.view(cur.ty()) {
            TypeView::ConstantArray(a) => cur = s.ast.types.canonical(a.elem),
            TypeView::IncompleteArray(e) => cur = s.ast.types.canonical(e),
            _ => break,
        }
        quals |= cur.quals();
    }
    quals
}

/// The qualification-decomposition ([conv.qual]/1) of `q`, as deep as
/// it goes.
fn step(s: &Sema<'_, '_>, q: QualType) -> Option<(Level, QualType)> {
    let c = s.ast.types.canonical(q);
    match s.ast.types.view(c.ty()) {
        TypeView::Pointer(p) => Some((Level::Pointer, s.ast.types.canonical(p))),
        TypeView::MemberPointer { pointee, class } => {
            let cls = s.ast.types.canonical(QualType::unqual(class)).ty();
            Some((Level::MemberPointer(cls), s.ast.types.canonical(pointee)))
        }
        TypeView::ConstantArray(a) => {
            Some((Level::Array(Some(a.size)), s.ast.types.canonical(a.elem)))
        }
        TypeView::IncompleteArray(e) => Some((Level::Array(None), s.ast.types.canonical(e))),
        _ => None,
    }
}

fn levels_match(a: Level, b: Level) -> bool {
    match (a, b) {
        (Level::Pointer, Level::Pointer) => true,
        (Level::MemberPointer(x), Level::MemberPointer(y)) => x == y,
        (Level::Array(x), Level::Array(y)) => x.is_none() || y.is_none() || x == y,
        _ => false,
    }
}

/// Two similar types decomposed level by level ([conv.qual]/1, /3),
/// with their cv-combined decomposition.
struct Combined {
    /// cv1_{j+1}, cv2_{j+1} and cv3_{j+1} at index j.
    cv1: Vec<Quals>,
    cv2: Vec<Quals>,
    cv3: Vec<Quals>,
    /// `P1_j`, `P2_j` and `P3_j`.
    p1: Vec<Level>,
    p2: Vec<Level>,
    p3: Vec<Level>,
    /// The innermost type U of the first (canonical, its own cv kept).
    inner: QualType,
}

/// Decomposes `a` and `b` under [conv.qual] and forms their cv-combined
/// decomposition (/3), or `None` if they are not similar (/2).
fn combine(s: &Sema<'_, '_>, a: QualType, b: QualType) -> Option<Combined> {
    // lv[j] = (cv1_{j+1}, P1_j, cv2_{j+1}, P2_j) in the notation of
    // [conv.qual]/3: P_j sits between cv_j and cv_{j+1}.
    let mut lv: Vec<(Quals, Level, Quals, Level)> = Vec::new();
    let (mut a, mut b) = (s.ast.types.canonical(a), s.ast.types.canonical(b));
    loop {
        match (step(s, a), step(s, b)) {
            (Some((la, na)), Some((lb, nb))) if levels_match(la, lb) && lv.len() < 4096 => {
                lv.push((effective_quals(s, na), la, effective_quals(s, nb), lb));
                a = na;
                b = nb;
            }
            _ => break,
        }
    }
    if !s.ast.types.same_type(a.without_quals(), b.without_quals()) {
        return None;
    }
    let n = lv.len();
    let cv1: Vec<Quals> = lv.iter().map(|l| l.0).collect();
    let cv2: Vec<Quals> = lv.iter().map(|l| l.2).collect();
    let p1: Vec<Level> = lv.iter().map(|l| l.1).collect();
    let p2: Vec<Level> = lv.iter().map(|l| l.3).collect();
    // cv3[j] is cv3_{j+1}.
    let mut cv3: Vec<Quals> = (0..n).map(|j| cv1[j] | cv2[j]).collect();
    let p3: Vec<Level> = (0..n)
        .map(|j| match (p1[j], p2[j]) {
            (Level::Array(None), _) | (_, Level::Array(None)) => Level::Array(None),
            (x, _) => x,
        })
        .collect();
    for i in 0..=n {
        let cv_changed = i >= 1 && (cv3[i - 1] != cv1[i - 1] || cv3[i - 1] != cv2[i - 1]);
        let p_changed = i < n && (p3[i] != p1[i] || p3[i] != p2[i]);
        if cv_changed || p_changed {
            // const on every cv3_k for 0 < k < i. An array's cv_k is its
            // element's cv_{k+1} ([basic.type.qualifier]/3), not a level
            // of its own: `int (*)[]` converts to `volatile int (*)[]`
            // (clang unwraps an array with the level below it,
            // `UnwrapSimilarTypes`).
            for (k, c) in cv3.iter_mut().enumerate().take(i.saturating_sub(1)) {
                if !matches!(p3[k + 1], Level::Array(_)) {
                    *c |= Quals::CONST;
                }
            }
        }
    }
    Some(Combined { cv1, cv2, cv3, p1, p2, p3, inner: a })
}

/// The cv-combined type of `a` and `b` ([conv.qual]/3), unqualified at
/// the top level, or `None` if they are not similar. Composite pointer
/// types of similar pointers are built from it ([expr.type]/4.6):
/// `int **` and `const int **` give `const int *const *`.
pub(crate) fn cv_combined_type(s: &mut Sema<'_, '_>, a: QualType, b: QualType) -> Option<QualType> {
    let c = combine(s, a, b)?;
    let n = c.p3.len();
    let ty = &mut s.ast.types;
    let mut t = ty.unqualified(c.inner);
    if n > 0 {
        t = ty.qualify(t, c.cv3[n - 1]);
    }
    for j in (0..n).rev() {
        t = match c.p3[j] {
            Level::Pointer => ty.pointer_to(t),
            Level::MemberPointer(cls) => ty.member_pointer(t, cls),
            Level::Array(Some(size)) => ty.constant_array(t, size, None),
            Level::Array(None) => ty.incomplete_array(t),
        };
        if j > 0 {
            t = ty.qualify(t, c.cv3[j - 1]);
        }
    }
    Some(t)
}

/// Classifies `from` and `to` under [conv.qual] (top-level cv-qualifiers
/// are ignored).
pub fn qualification_relation(s: &Sema<'_, '_>, from: QualType, to: QualType) -> QualRelation {
    let Some(Combined { cv1, cv2, cv3, p1, p2, p3, .. }) = combine(s, from, to) else {
        return QualRelation::NotSimilar;
    };
    let n = cv1.len();
    let mut drops_outer = false;
    let mut drops_nested = false;
    for j in 0..n {
        if !cv2[j].contains(cv1[j]) {
            if j == 0 {
                drops_outer = true;
            } else {
                drops_nested = true;
            }
        }
    }
    let identical = (0..n).all(|j| cv1[j] == cv2[j] && p1[j] == p2[j]);
    let convertible = (0..n).all(|j| cv3[j] == cv2[j] && p3[j] == p2[j]);
    if !convertible && !drops_outer && !drops_nested {
        drops_nested = true;
    }
    QualRelation::Similar { identical, convertible, drops_outer, drops_nested }
}

/// True if `from` and `to` are similar ([conv.qual]/2).
pub fn similar(s: &Sema<'_, '_>, from: QualType, to: QualType) -> bool {
    !matches!(qualification_relation(s, from, to), QualRelation::NotSimilar)
}

/// True if `a` and `b` are similar with the same `P_i` at every level
/// (no array of unknown bound against one of known bound): the
/// similarity [expr.const.cast]/3 needs (clang `UnwrapSimilarTypes`
/// without `AllowPiMismatch`).
pub(crate) fn similar_same_levels(s: &Sema<'_, '_>, a: QualType, b: QualType) -> bool {
    combine(s, a, b).is_some_and(|c| c.p1 == c.p2)
}

/// True if a prvalue of `from` converts to `to` by a (possibly empty)
/// qualification conversion.
pub fn is_qualification_convertible(s: &Sema<'_, '_>, from: QualType, to: QualType) -> bool {
    matches!(qualification_relation(s, from, to), QualRelation::Similar { convertible: true, .. })
}

/// The function type `f` without its non-throwing exception
/// specification, or `None` if `f` is not a non-throwing function type.
pub(crate) fn drop_noexcept(s: &mut Sema<'_, '_>, f: QualType) -> Option<QualType> {
    let p = *s.ast.types.proto(f)?;
    if !matches!(s.ast.types.exc_spec(&p), ExcSpec::Nothrow) {
        return None;
    }
    let params = s.ast.types.params(&p).to_vec();
    Some(s.ast.types.function(p.ret(), &params, p.info(), ExcSpec::None))
}

/// True if function type `from` converts to function type `to` by
/// dropping its non-throwing exception specification ([conv.fctptr]),
/// its GNU `noreturn` (a clang extension, spec/types.md §5.4), or both
/// (clang `IsFunctionConversion`).
pub(crate) fn is_function_conversion(s: &mut Sema<'_, '_>, from: QualType, to: QualType) -> bool {
    let (Some(pf), Some(pt)) = (s.ast.types.proto(from).copied(), s.ast.types.proto(to).copied())
    else {
        return false;
    };
    let mut info = pf.info();
    let mut exc = s.ast.types.exc_spec(&pf);
    let mut changed = false;
    if info.noreturn() && !pt.info().noreturn() {
        info = info.with_noreturn(false);
        changed = true;
    }
    if matches!(exc, ExcSpec::Nothrow) && !matches!(s.ast.types.exc_spec(&pt), ExcSpec::Nothrow) {
        exc = ExcSpec::None;
        changed = true;
    }
    if !changed {
        return false;
    }
    let params = s.ast.types.params(&pf).to_vec();
    let f = s.ast.types.function(pf.ret(), &params, info, exc);
    s.ast.types.same_type(f, to)
}

/// True if function type `referee` is reference-compatible with
/// function type `from` only through a function pointer conversion
/// ([dcl.init.ref]/4: "pointer to `void (int) noexcept`" converts to
/// "pointer to `void (int)`"), both canonical and unqualified. Such a
/// reference binds directly, and its conversion sequence is the
/// identity ([over.ics.ref]/1), like that of an exact match (clang
/// `ReferenceConversions::Function`).
pub(crate) fn function_ref_compatible(
    s: &mut Sema<'_, '_>,
    referee: QualType,
    from: QualType,
) -> bool {
    s.ast.types.is_function(referee) && is_function_conversion(s, from, referee)
}

/// CWG2352 ([dcl.init.ref]/4): referee type `t1` against `t2`, the
/// initializer's type, both with their own cv-qualifiers, when the two are
/// similar ([conv.qual]/2) but not the same type up to top-level cv:
/// `None` if they are not similar; `Some(true)` if `cv1 T1` is
/// reference-compatible with `cv2 T2` (a prvalue "pointer to cv2 T2"
/// converts to "pointer to cv1 T1" by a qualification conversion, so
/// `const int *const &` binds an `int *` lvalue directly); `Some(false)`
/// if they are only reference-related (`const int *&` and `int *`).
/// clang `CompareReferenceRelationship`'s qualification loop, where the
/// top level takes part in the "const at every level above" rule.
pub(crate) fn similar_ref_relation(s: &mut Sema<'_, '_>, t1: QualType, t2: QualType) -> Option<bool> {
    let (c1, c2) = (s.ast.types.canonical(t1), s.ast.types.canonical(t2));
    if !similar(s, c1, c2) {
        return None;
    }
    let (p1, p2) = (s.ast.types.pointer_to(c1), s.ast.types.pointer_to(c2));
    Some(is_qualification_convertible(s, p2, p1))
}

/// True if a prvalue of `from` converts to `to` by a function pointer
/// conversion ([conv.fctptr]): "pointer to `noexcept` function" to
/// "pointer to function", and likewise for pointers to member
/// functions.
pub fn is_function_pointer_conversion(s: &mut Sema<'_, '_>, from: QualType, to: QualType) -> bool {
    let (cf, ct) = (s.ast.types.canonical(from), s.ast.types.canonical(to));
    let (pf, pt) = match (s.ast.types.view(cf.ty()), s.ast.types.view(ct.ty())) {
        (TypeView::Pointer(a), TypeView::Pointer(b)) => (a, b),
        (
            TypeView::MemberPointer { pointee: a, class: x },
            TypeView::MemberPointer { pointee: b, class: y },
        ) if s.ast.types.same_type(QualType::unqual(x), QualType::unqual(y)) => (a, b),
        _ => return false,
    };
    if !s.ast.types.is_function(pf) || !s.ast.types.is_function(pt) {
        return false;
    }
    if !s.ast.types.canonical(pt).quals().is_empty() {
        return false;
    }
    let pf = s.ast.types.canonical(pf);
    is_function_conversion(s, pf, pt)
}

/// How a pointer conversion casts away constness (clang
/// `CastAwayConstnessKind`), ordered from harmless to worst.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum CastAway {
    /// It does not ([expr.const.cast]/8).
    None,
    /// It does, at a level where both types are similar.
    Similar,
    /// It does, below levels of the same kind but not similar (pointers
    /// to members of different classes, arrays of different bounds).
    SimilarKind,
    /// It does, below levels of different kinds (a pointer against an
    /// array): clang accepts the cast as an extension.
    Incoherent,
}

/// The kind of one level of a type for [`casts_away_constness`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CastLevel {
    None,
    Ptr,
    MemPtr,
    Array,
}

fn cast_level(s: &Sema<'_, '_>, t: QualType) -> (CastLevel, QualType) {
    match s.ast.types.canonical_view(t) {
        TypeView::Pointer(p) => (CastLevel::Ptr, p),
        TypeView::MemberPointer { pointee, .. } => (CastLevel::MemPtr, pointee),
        TypeView::ConstantArray(a) => (CastLevel::Array, a.elem),
        TypeView::IncompleteArray(e) => (CastLevel::Array, e),
        _ => (CastLevel::None, t),
    }
}

/// clang `UnwrapSimilarArrayTypes` (allowing an array of unknown bound
/// against one of known bound).
fn unwrap_similar_arrays(s: &Sema<'_, '_>, t1: &mut QualType, t2: &mut QualType) {
    loop {
        let bound = |t: QualType| match s.ast.types.canonical_view(t) {
            TypeView::ConstantArray(a) => Some((Some(a.size), a.elem)),
            TypeView::IncompleteArray(e) => Some((None, e)),
            _ => None,
        };
        match (bound(*t1), bound(*t2)) {
            (Some((b1, e1)), Some((b2, e2))) if b1 == b2 || b1.is_none() || b2.is_none() => {
                *t1 = e1;
                *t2 = e2;
            }
            _ => return,
        }
    }
}

/// clang `unwrapCastAwayConstnessLevel`: strips one level from both
/// types; `None` when either has no further level.
fn unwrap_cast_level(s: &Sema<'_, '_>, t1: &mut QualType, t2: &mut QualType) -> Option<CastAway> {
    unwrap_similar_arrays(s, t1, t2);
    let (c1, p1) = cast_level(s, *t1);
    let (c2, p2) = cast_level(s, *t2);
    let similar = match (s.ast.types.canonical_view(*t1), s.ast.types.canonical_view(*t2)) {
        (TypeView::Pointer(_), TypeView::Pointer(_)) => true,
        (TypeView::MemberPointer { class: a, .. }, TypeView::MemberPointer { class: b, .. }) => {
            s.ast.types.same_unqualified(QualType::unqual(a), QualType::unqual(b))
        }
        _ => false,
    };
    if c1 == CastLevel::None || c2 == CastLevel::None {
        return None;
    }
    let mut kind = if similar {
        CastAway::Similar
    } else if c1 == c2 {
        CastAway::SimilarKind
    } else {
        CastAway::Incoherent
    };
    *t1 = p1;
    *t2 = p2;
    // An array at this level: its element type's qualifiers are the
    // level's (any qualifier of a matching level of `t2` corresponds).
    loop {
        unwrap_similar_arrays(s, t1, t2);
        let (a1, e1) = cast_level(s, *t1);
        if a1 != CastLevel::Array {
            break;
        }
        let (a2, e2) = cast_level(s, *t2);
        if a2 == CastLevel::None {
            break;
        }
        if a2 != CastLevel::Array {
            kind = CastAway::Incoherent;
        } else if kind != CastAway::Incoherent {
            kind = CastAway::SimilarKind;
        }
        // An array's qualifiers are its element type's (spec/types.md
        // §2.1), so `t2`'s carry over.
        *t1 = e1;
        *t2 = e2;
    }
    Some(kind)
}

/// clang `CastsAwayConstness` for the pointer (or pointer to member)
/// types `src` and `dest` ([expr.const.cast]/8): a level whose
/// cv-qualifiers `dest` does not include, or a level that adds one below
/// a level of `dest` that is not `const`.
pub(crate) fn casts_away_constness(s: &Sema<'_, '_>, src: QualType, dest: QualType) -> CastAway {
    let mut t1 = s.ast.types.canonical(src);
    let mut t2 = s.ast.types.canonical(dest);
    let mut worst = CastAway::Similar;
    let mut all_const = true;
    while let Some(kind) = unwrap_cast_level(s, &mut t1, &mut t2) {
        worst = worst.max(kind);
        let cvr = Quals::CV | Quals::RESTRICT;
        let q1 = s.ast.types.effective_quals(t1) & cvr;
        let q2 = s.ast.types.effective_quals(t2) & cvr;
        if q1 != q2 && (!q2.contains(q1) || !all_const) {
            return worst;
        }
        if all_const && !q2.is_const() {
            all_const = false;
        }
    }
    CastAway::None
}
