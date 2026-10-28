//! Nested-name-specifiers (spec/types.md §8.2).
//!
//! An [`Nns`] is a linked prefix chain (`a::b::C<T>::`), interned in a
//! four-word keyed table with a canonical link like types: namespaces
//! canonicalize to the namespace without its written prefix, types to
//! their canonical type, identifiers keep their canonical prefix.

use super::context::TypeContext;
use super::kinds::TypeFlags;
use crate::ids::{DeclId, ExprId, NnsId, QualType, RawId};
use kiln_source::symbol::Symbol;

/// One component of a nested-name-specifier.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Nns {
    /// `::`
    Global,
    /// `prefix::ns::` (`ns` is the original namespace declaration).
    Namespace {
        /// The written prefix.
        prefix: Option<NnsId>,
        /// The namespace.
        ns: DeclId,
    },
    /// `prefix::alias::` naming namespace `ns`.
    NamespaceAlias {
        /// The written prefix.
        prefix: Option<NnsId>,
        /// The namespace alias declaration.
        alias: DeclId,
        /// The aliased (original) namespace.
        ns: DeclId,
    },
    /// `prefix::T::` or `prefix::template X<T>::`.
    Type {
        /// The written prefix.
        prefix: Option<NnsId>,
        /// The type as written.
        ty: QualType,
        /// The `template` keyword was written.
        template_kw: bool,
    },
    /// `prefix::name::` where `name` is unresolved (dependent).
    Identifier {
        /// The (dependent) prefix.
        prefix: Option<NnsId>,
        /// The identifier.
        name: Symbol,
    },
    /// `[: expr :]::` (P2996; canonical as written, M6).
    Splice(ExprId),
}

/// The kind of an [`Nns`] component.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum NnsKind {
    Global = 1,
    Namespace,
    NamespaceAlias,
    Type,
    Identifier,
    Splice,
}

impl NnsKind {
    fn from_u32(v: u32) -> NnsKind {
        match v & 0xff {
            2 => NnsKind::Namespace,
            3 => NnsKind::NamespaceAlias,
            4 => NnsKind::Type,
            5 => NnsKind::Identifier,
            6 => NnsKind::Splice,
            _ => NnsKind::Global,
        }
    }
}

fn encode(n: Nns) -> [u32; 4] {
    let p = <NnsId as RawId>::raw_opt;
    match n {
        Nns::Global => [NnsKind::Global as u32, 0, 0, 0],
        Nns::Namespace { prefix, ns } => [NnsKind::Namespace as u32, p(prefix), ns.raw(), 0],
        Nns::NamespaceAlias { prefix, alias, ns } => {
            [NnsKind::NamespaceAlias as u32, p(prefix), alias.raw(), ns.raw()]
        }
        Nns::Type { prefix, ty, template_kw } => {
            [NnsKind::Type as u32 | (u32::from(template_kw) << 8), p(prefix), ty.to_raw(), 0]
        }
        Nns::Identifier { prefix, name } => {
            [NnsKind::Identifier as u32, p(prefix), name.index(), 0]
        }
        Nns::Splice(e) => [NnsKind::Splice as u32, 0, e.raw(), 0],
    }
}

fn decode(k: [u32; 4]) -> Nns {
    let prefix = NnsId::try_new(k[1]);
    let decl = |v: u32| DeclId::try_new(v).unwrap_or(DeclId::new(1));
    match NnsKind::from_u32(k[0]) {
        NnsKind::Global => Nns::Global,
        NnsKind::Namespace => Nns::Namespace { prefix, ns: decl(k[2]) },
        NnsKind::NamespaceAlias => {
            Nns::NamespaceAlias { prefix, alias: decl(k[2]), ns: decl(k[3]) }
        }
        NnsKind::Type => Nns::Type {
            prefix,
            ty: QualType::from_raw(k[2]).unwrap_or(QualType::ERROR),
            template_kw: k[0] >> 8 != 0,
        },
        NnsKind::Identifier => Nns::Identifier { prefix, name: Symbol::from_index(k[2]) },
        NnsKind::Splice => Nns::Splice(ExprId::try_new(k[2]).unwrap_or(ExprId::new(1))),
    }
}

impl TypeContext {
    /// Interns a nested-name-specifier component. Returns the `::`
    /// sentinel (id 1) and sets [`TypeContext::exhausted`] when the table
    /// is full.
    pub fn nns(&mut self, n: Nns) -> NnsId {
        let key = encode(n);
        let hash = match self.nns.find(&key) {
            Ok(id) => return NnsId::new(id),
            Err(h) => h,
        };
        let prefix_flags =
            |ctx: &Self, p: Option<NnsId>| p.map_or(TypeFlags::NONE, |p| ctx.nns_flags(p));
        let (canon, flags) = match n {
            Nns::Global => (n, TypeFlags::NONE),
            Nns::Namespace { prefix, ns } | Nns::NamespaceAlias { prefix, ns, .. } => {
                (Nns::Namespace { prefix: None, ns }, prefix_flags(self, prefix))
            }
            Nns::Type { prefix, ty, .. } => (
                Nns::Type { prefix: None, ty: self.canonical(ty), template_kw: false },
                prefix_flags(self, prefix) | self.qflags(ty),
            ),
            Nns::Identifier { prefix, name } => (
                Nns::Identifier { prefix: prefix.map(|p| self.canonical_nns(p)), name },
                prefix_flags(self, prefix) | TypeFlags::DEPENDENT_INSTANTIATION,
            ),
            Nns::Splice(_) => (n, TypeFlags::DEPENDENT_INSTANTIATION),
        };
        let canon = if canon == n { None } else { Some(self.nns(canon).index()) };
        if let Some(id) = self.nns.push(key, hash, flags.bits(), canon) {
            NnsId::new(id)
        } else {
            self.exhausted = true;
            NnsId::new(1)
        }
    }

    /// The component `n`.
    pub fn nns_view(&self, n: NnsId) -> Nns {
        decode(self.nns.get(n.index()).key)
    }

    /// The kind of component `n`.
    pub fn nns_kind(&self, n: NnsId) -> NnsKind {
        NnsKind::from_u32(self.nns.get(n.index()).key[0])
    }

    /// The written prefix of `n`.
    pub fn nns_prefix(&self, n: NnsId) -> Option<NnsId> {
        NnsId::try_new(self.nns.get(n.index()).key[1])
    }

    /// The canonical form of `n`.
    pub fn canonical_nns(&self, n: NnsId) -> NnsId {
        NnsId::new(self.nns.get(n.index()).canon)
    }

    /// True if `a` and `b` denote the same entity.
    pub fn same_nns(&self, a: NnsId, b: NnsId) -> bool {
        self.canonical_nns(a) == self.canonical_nns(b)
    }

    /// The dependence flags of `n` (and its prefixes).
    pub fn nns_flags(&self, n: NnsId) -> TypeFlags {
        TypeFlags::from_bits(self.nns.get(n.index()).flags)
    }
}
