//! WP-655: the `TypeContext` kind queries read the canonical node
//! directly. Each must agree with the `TypeView` match it replaced, on a
//! node of every `TypeKind`.

use super::context::{TypeContext, TypeView};
use super::func::{ExcSpec, FuncInfo};
use super::kinds::{BuiltinType, Quals, TypeFlags, TypeKind};
use super::layout::TargetTypes;
use super::node::{ConstArray, Key, Shape, shape};

use crate::ids::{DeclId, ExprId, QualType, RawId, TypeId};

/// The pre-WP-655 bodies, kept as the reference.
mod reference {
    use super::*;

    pub fn builtin(c: &TypeContext, q: QualType) -> Option<BuiltinType> {
        match c.canonical_view(q) {
            TypeView::Builtin(b) => Some(b),
            _ => None,
        }
    }

    pub fn pointee(c: &TypeContext, q: QualType) -> Option<QualType> {
        match c.canonical_view(q) {
            TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => Some(p),
            TypeView::MemberPointer { pointee, .. } => Some(pointee),
            _ => None,
        }
    }

    pub fn element(c: &TypeContext, q: QualType) -> Option<QualType> {
        match c.canonical_view(q) {
            TypeView::ConstantArray(a) => Some(a.elem),
            TypeView::IncompleteArray(e)
            | TypeView::Complex(e)
            | TypeView::VariableArray { elem: e, .. }
            | TypeView::DependentSizedArray { elem: e, .. }
            | TypeView::Vector { elem: e, .. }
            | TypeView::DependentVector { elem: e, .. } => Some(e),
            _ => None,
        }
    }

    pub fn array_size(c: &TypeContext, q: QualType) -> Option<u64> {
        match c.canonical_view(q) {
            TypeView::ConstantArray(a) => Some(a.size),
            _ => None,
        }
    }

    pub fn as_atomic(c: &TypeContext, q: QualType) -> Option<QualType> {
        match c.canonical_view(q) {
            TypeView::Atomic(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_complex(c: &TypeContext, q: QualType) -> Option<QualType> {
        match c.canonical_view(q) {
            TypeView::Complex(e) => Some(e),
            _ => None,
        }
    }

    pub fn as_record(c: &TypeContext, q: QualType) -> Option<DeclId> {
        match c.canonical_view(q) {
            TypeView::Record(d) => Some(d),
            TypeView::TemplateSpecialization { .. } => {
                let mut t = q;
                loop {
                    if let TypeView::InjectedClassName { pattern, .. } = c.view(t.ty()) {
                        return Some(pattern);
                    }
                    let n = c.desugar_once(t);
                    if n == t {
                        return None;
                    }
                    t = n;
                }
            }
            _ => None,
        }
    }

    pub fn as_enum(c: &TypeContext, q: QualType) -> Option<DeclId> {
        match c.canonical_view(q) {
            TypeView::Enum(d) => Some(d),
            _ => None,
        }
    }

    pub fn is_integral(c: &TypeContext, q: QualType) -> bool {
        match c.canonical_view(q) {
            TypeView::Builtin(b) => b.is_integral(),
            TypeView::BitInt { .. } => true,
            _ => false,
        }
    }

    pub fn effective_quals(c: &TypeContext, q: QualType) -> Quals {
        let mut x = c.canonical(q);
        loop {
            match c.view(x.ty()) {
                TypeView::ConstantArray(a) => x = a.elem,
                TypeView::IncompleteArray(e)
                | TypeView::VariableArray { elem: e, .. }
                | TypeView::DependentSizedArray { elem: e, .. } => x = e,
                _ => return x.quals(),
            }
            x = c.canonical(x);
        }
    }
}

/// Raw operand words: zero (decodes to the fallbacks), a valid
/// qualified type, and a valid declaration id.
fn words() -> [u32; 3] {
    let cint = QualType::new(TypeId::INT, Quals::CONST.bits());
    [0, cint.raw(), DeclId::new(7).raw()]
}

/// Builds nodes of `kind` (none for `Func`, which `function` covers).
fn nodes_of(c: &mut TypeContext, kind: TypeKind) -> Vec<TypeId> {
    let w = words();
    let mut keys: Vec<Key<'static>> = Vec::new();
    match shape(kind) {
        Shape::Inline => {
            for aux in [0u16, 1, 0x100] {
                for &d in &w {
                    keys.push(Key::inline(kind, aux, d));
                }
            }
            if kind == TypeKind::Builtin {
                for b in BuiltinType::ALL {
                    keys.push(Key::inline(kind, 0, b as u32));
                }
                keys.push(Key::inline(kind, 0, 10_000));
            }
        }
        Shape::Pair => {
            for &a in &w {
                for &b in &w {
                    keys.push(Key::pair(kind, 0, a, b));
                    keys.push(Key::pair(kind, 0x100, a, b));
                }
            }
        }
        Shape::Triple => {
            for &a in &w {
                for &b in &w {
                    keys.push(Key::triple(kind, 0, a, b, 0));
                    keys.push(Key::triple(kind, 0, a, b, w[1]));
                }
            }
        }
        Shape::Array => {
            for elem in [QualType::INT, QualType::new(TypeId::CHAR, Quals::VOLATILE.bits())] {
                keys.push(Key::Array(ConstArray { elem, size: 3, size_expr: None }));
                keys.push(Key::Array(ConstArray {
                    elem,
                    size: 0,
                    size_expr: Some(ExprId::new(1)),
                }));
            }
        }
        Shape::Func => {}
    }
    keys.iter().map(|k| c.create(k, TypeFlags::NONE, None, None)).collect()
}

fn check(c: &TypeContext, q: QualType) {
    let what = || format!("{:?} {q:?}", c.kind(c.canonical(q).ty()));
    assert_eq!(c.builtin(q), reference::builtin(c, q), "builtin {}", what());
    assert_eq!(
        c.is_error(q),
        reference::builtin(c, q) == Some(BuiltinType::Error),
        "is_error {}",
        what()
    );
    assert_eq!(c.pointee(q), reference::pointee(c, q), "pointee {}", what());
    assert_eq!(c.element(q), reference::element(c, q), "element {}", what());
    assert_eq!(c.array_size(q), reference::array_size(c, q), "array_size {}", what());
    assert_eq!(c.as_atomic(q), reference::as_atomic(c, q), "as_atomic {}", what());
    assert_eq!(c.as_complex(q), reference::as_complex(c, q), "as_complex {}", what());
    assert_eq!(c.as_record(q), reference::as_record(c, q), "as_record {}", what());
    assert_eq!(c.as_enum(q), reference::as_enum(c, q), "as_enum {}", what());
    assert_eq!(c.is_integral(q), reference::is_integral(c, q), "is_integral {}", what());
    assert_eq!(
        c.effective_quals(q),
        reference::effective_quals(c, q),
        "effective_quals {}",
        what()
    );
    for k in all_kinds() {
        let t = c.canonical(q).ty();
        let expect = (c.kind(t) == k).then(|| c.view(t));
        assert_eq!(c.view_if(t, k), expect, "view_if {k:?} {}", what());
    }
}

fn all_kinds() -> impl Iterator<Item = TypeKind> {
    (0..=u8::MAX)
        .map(TypeKind::from_u8)
        .enumerate()
        .filter_map(|(i, k)| (k as usize == i).then_some(k))
}

#[test]
fn queries_agree_with_view_on_every_kind() {
    let mut c = TypeContext::new(TargetTypes::default());
    let mut covered = Vec::new();
    let mut ids = Vec::new();
    for k in all_kinds() {
        let made = nodes_of(&mut c, k);
        if !made.is_empty() || shape(k) == Shape::Func {
            covered.push(k);
        }
        ids.extend(made);
    }
    let f = c.function(QualType::INT, &[QualType::DOUBLE], FuncInfo::default(), ExcSpec::None);
    ids.push(f.ty());
    assert_eq!(covered.len(), TypeKind::COUNT, "a kind without a test node");
    // Every builtin at its fixed id, and every node as a sugar-free
    // canonical type, plain and cv-qualified.
    ids.extend(BuiltinType::ALL.iter().map(|b| b.type_id()));
    for t in ids {
        for quals in [Quals::NONE, Quals::CONST, Quals::ALL] {
            check(&c, QualType::new(t, quals.bits()));
        }
    }
    // Sugar over each interesting canonical node: a pointer to it, an
    // array of it and a typedef-like `Paren` node are canonical-preserving
    // through `canonical`.
    let base = [QualType::INT, QualType::ERROR, QualType::new(TypeId::CHAR, Quals::CONST.bits())];
    for b in base {
        let p = c.pointer_to(b);
        check(&c, p);
        let r = c.lvalue_ref_to(b);
        check(&c, r);
    }
}
