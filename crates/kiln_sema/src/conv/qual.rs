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

/// Classifies `from` and `to` under [conv.qual] (top-level cv-qualifiers
/// are ignored).
pub fn qualification_relation(s: &Sema<'_, '_>, from: QualType, to: QualType) -> QualRelation {
    // lv[j] = (cv1_{j+1}, P1_j, cv2_{j+1}, P2_j) in the notation of
    // [conv.qual]/3: P_j sits between cv_j and cv_{j+1}.
    let mut lv: Vec<(Quals, Level, Quals, Level)> = Vec::new();
    let (mut a, mut b) = (s.ast.types.canonical(from), s.ast.types.canonical(to));
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
        return QualRelation::NotSimilar;
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
            // const on every cv3_k for 0 < k < i.
            for c in cv3.iter_mut().take(i.saturating_sub(1)) {
                *c |= Quals::CONST;
            }
        }
    }
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
    match drop_noexcept(s, s.ast.types.canonical(pf)) {
        Some(f) => s.ast.types.same_type(f, pt),
        None => false,
    }
}
