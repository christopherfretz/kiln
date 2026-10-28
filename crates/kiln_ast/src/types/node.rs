//! Node storage: the 8-byte [`TypeNode`], the side arenas, and the
//! structural keys the intern table hashes (spec/types.md §4, §7.1).

use super::func::{ExcSpec, FuncInfo, FuncProto};
use super::kinds::{TypeFlags, TypeKind};
use crate::ids::{ExprId, QualType, TypeId};

/// One type node: 8 bytes, align 4 (spec/types.md §4).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TypeNode {
    pub(crate) kind: u8,
    pub(crate) flags: u8,
    pub(crate) aux: u16,
    pub(crate) data: u32,
}

impl TypeNode {
    /// The node's own kind.
    #[inline]
    pub fn kind(&self) -> TypeKind {
        TypeKind::from_u8(self.kind)
    }

    /// The node's flags.
    #[inline]
    pub const fn flags(&self) -> TypeFlags {
        TypeFlags::from_bits(self.flags)
    }

    /// The small per-kind operand.
    #[inline]
    pub const fn aux(&self) -> u16 {
        self.aux
    }

    /// The inline operand or side-arena index.
    #[inline]
    pub const fn data(&self) -> u32 {
        self.data
    }
}

/// The side record of a `ConstantArray`: 16 bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ConstArray {
    /// The element type (carries the array's cv-qualifiers, §2.1).
    pub elem: QualType,
    /// The number of elements.
    pub size: u64,
    /// The bound as written, when it was an expression (sugar; `None` in
    /// the canonical type).
    pub size_expr: Option<ExprId>,
}

/// How a kind stores its payload.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Shape {
    Inline,
    Pair,
    Triple,
    Array,
    Func,
}

pub(crate) fn shape(kind: TypeKind) -> Shape {
    use TypeKind as K;
    match kind {
        K::Invalid
        | K::Builtin
        | K::BitInt
        | K::DependentBitInt
        | K::Pointer
        | K::LValueRef
        | K::RValueRef
        | K::IncompleteArray
        | K::Record
        | K::Enum
        | K::Complex
        | K::Atomic
        | K::QualifiedArray
        | K::TemplateTypeParm
        | K::CanonTemplateTypeParm
        | K::DependentDecltype => Shape::Inline,
        K::MemberPointer
        | K::VariableArray
        | K::DependentSizedArray
        | K::Vector
        | K::DependentVector
        | K::ExtQual
        | K::Typedef
        | K::Using
        | K::Elaborated
        | K::InjectedClassName
        | K::DependentName
        | K::PackExpansion
        | K::Decltype
        | K::TypeOf
        | K::UnaryTransform
        | K::DeducedTemplateSpecialization
        | K::Attributed
        | K::Splice => Shape::Pair,
        K::SubstTemplateTypeParm
        | K::SubstTemplateTypeParmPack
        | K::TemplateSpecialization
        | K::DependentTemplateSpecialization
        | K::PackIndexing
        | K::Auto => Shape::Triple,
        K::ConstantArray => Shape::Array,
        K::Function => Shape::Func,
    }
}

/// The exception-specification part of a function key.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum ExcKey<'a> {
    /// Tag plus inline operand (`tag | operand << 3`).
    Simple(u32),
    /// `noexcept(expr)`: `[expr, value tag, profile or 0]`.
    Expr([u32; 3]),
    /// `throw(A, B)`.
    Dynamic(&'a [QualType]),
}

/// The structural key of a node: what hash-consing compares.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Key<'a> {
    Inline { kind: u8, aux: u16, data: u32 },
    Pair { kind: u8, aux: u16, w: [u32; 2] },
    Triple { kind: u8, aux: u16, w: [u32; 3] },
    Array(ConstArray),
    Func { ret: QualType, params: &'a [QualType], info: FuncInfo, exc: ExcKey<'a> },
}

impl Key<'_> {
    pub(crate) fn inline(kind: TypeKind, aux: u16, data: u32) -> Key<'static> {
        Key::Inline { kind: kind as u8, aux, data }
    }

    pub(crate) fn pair(kind: TypeKind, aux: u16, a: u32, b: u32) -> Key<'static> {
        Key::Pair { kind: kind as u8, aux, w: [a, b] }
    }

    pub(crate) fn triple(kind: TypeKind, aux: u16, a: u32, b: u32, c: u32) -> Key<'static> {
        Key::Triple { kind: kind as u8, aux, w: [a, b, c] }
    }
}

/// The node arena and its side arenas. A separate struct from the intern
/// table so that probing (which reads keys back from here) and inserting
/// borrow disjoint fields.
#[derive(Debug)]
pub(crate) struct Store {
    pub(crate) nodes: Vec<TypeNode>,
    pub(crate) canon: Vec<QualType>,
    pub(crate) protos: Vec<FuncProto>,
    pub(crate) type_lists: Vec<QualType>,
    pub(crate) pairs: Vec<[u32; 2]>,
    pub(crate) triples: Vec<[u32; 3]>,
    pub(crate) arrays: Vec<ConstArray>,
}

impl Store {
    pub(crate) fn new() -> Store {
        let dummy_q = QualType::unqual(TypeId::new(1));
        Store {
            nodes: vec![TypeNode { kind: 0, flags: 0, aux: 0, data: 0 }],
            canon: vec![dummy_q],
            protos: Vec::new(),
            type_lists: Vec::new(),
            pairs: vec![[0, 0]],
            triples: vec![[0, 0, 0]],
            arrays: Vec::new(),
        }
    }

    #[inline]
    pub(crate) fn node(&self, t: TypeId) -> TypeNode {
        self.nodes[t.index() as usize]
    }

    #[inline]
    pub(crate) fn pair(&self, i: u32) -> [u32; 2] {
        self.pairs[i as usize]
    }

    #[inline]
    pub(crate) fn triple(&self, i: u32) -> [u32; 3] {
        self.triples[i as usize]
    }

    #[inline]
    pub(crate) fn list(&self, start: u32, len: u32) -> &[QualType] {
        &self.type_lists[start as usize..start as usize + len as usize]
    }

    /// The structural key of stored node `t`.
    pub(crate) fn key_of(&self, t: u32) -> Key<'_> {
        let n = self.nodes[t as usize];
        match shape(n.kind()) {
            Shape::Inline => Key::Inline { kind: n.kind, aux: n.aux, data: n.data },
            Shape::Pair => Key::Pair { kind: n.kind, aux: n.aux, w: self.pair(n.data) },
            Shape::Triple => Key::Triple { kind: n.kind, aux: n.aux, w: self.triple(n.data) },
            Shape::Array => Key::Array(self.arrays[n.data as usize]),
            Shape::Func => {
                let p = &self.protos[n.data as usize];
                let params = self.list(p.params, u32::from(p.n_params));
                let exc = match p.exc_tag() {
                    ExcSpec::TAG_NOEXCEPT_EXPR => ExcKey::Expr(self.triple(p.exc_operand())),
                    ExcSpec::TAG_DYNAMIC => {
                        let [s, l] = self.pair(p.exc_operand());
                        ExcKey::Dynamic(self.list(s, l))
                    }
                    _ => ExcKey::Simple(p.exc),
                };
                Key::Func { ret: p.ret, params, info: p.info, exc }
            }
        }
    }

    /// Room for `extra` more entries in a `u32`-indexed arena of length
    /// `len`.
    #[inline]
    pub(crate) fn fits(len: usize, extra: usize) -> bool {
        len.checked_add(extra).is_some_and(|n| u32::try_from(n).is_ok())
    }
}

/// Appends the payload of `key` to the side arenas and returns
/// `(kind, aux, data)` for the node, or `None` if an arena is full.
pub(crate) fn push_payload(store: &mut Store, key: &Key<'_>) -> Option<(u8, u16, u32)> {
    match *key {
        Key::Inline { kind, aux, data } => Some((kind, aux, data)),
        Key::Pair { kind, aux, w } => {
            let i = u32::try_from(store.pairs.len()).ok()?;
            store.pairs.push(w);
            Some((kind, aux, i))
        }
        Key::Triple { kind, aux, w } => {
            let i = u32::try_from(store.triples.len()).ok()?;
            store.triples.push(w);
            Some((kind, aux, i))
        }
        Key::Array(a) => {
            let i = u32::try_from(store.arrays.len()).ok()?;
            store.arrays.push(a);
            Some((TypeKind::ConstantArray as u8, 0, i))
        }
        Key::Func { ret, params, info, exc } => {
            let n_params = u16::try_from(params.len()).ok()?;
            let extra = params.len() + if let ExcKey::Dynamic(l) = exc { l.len() } else { 0 };
            if !Store::fits(store.type_lists.len(), extra)
                || !Store::fits(store.pairs.len().max(store.triples.len()), 1)
            {
                return None;
            }
            let start = store.type_lists.len() as u32;
            store.type_lists.extend_from_slice(params);
            let exc_word = match exc {
                ExcKey::Simple(w) => w,
                ExcKey::Expr(w) => {
                    let i = store.triples.len() as u32;
                    if i >> 29 != 0 {
                        return None;
                    }
                    store.triples.push(w);
                    ExcSpec::TAG_NOEXCEPT_EXPR | (i << 3)
                }
                ExcKey::Dynamic(l) => {
                    let s = store.type_lists.len() as u32;
                    store.type_lists.extend_from_slice(l);
                    let i = store.pairs.len() as u32;
                    if i >> 29 != 0 {
                        return None;
                    }
                    store.pairs.push([s, l.len() as u32]);
                    ExcSpec::TAG_DYNAMIC | (i << 3)
                }
            };
            let i = u32::try_from(store.protos.len()).ok()?;
            store.protos.push(FuncProto { ret, params: start, n_params, info, exc: exc_word });
            Some((TypeKind::Function as u8, 0, i))
        }
    }
}
