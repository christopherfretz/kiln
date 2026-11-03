//! Reflections (spec/ast.md §10).
//!
//! WP-267 spike (`-freflection`): the table interns the entity a
//! `std::meta::info` value designates. A [`ReflectionId`] is a dense index
//! into it, so a value the Oven hands back (a scalar of shadow kind
//! `Reflection`, frozen as `Scalar::Reflection`) is looked up again with
//! [`ReflectionTable::get`]. Ids are 1-based like every arena id; the
//! Oven's 8-byte value is `index - 1` ([`ReflectionId::bits`]), so id 1 is
//! the null reflection and an all-zero (value-initialized)
//! `std::meta::info` is null.
//!
//! Mapping (spec/reflection-repr.md; P2996R13 [expr.reflect],
//! [meta.reflection]): the id *is* the identity — two reflections are
//! equal iff their ids are. Every variant is keyed so that equal entities
//! intern equal:
//! - a type keeps a top-level alias (`Typedef` of the alias's first
//!   declaration, or an alias template specialization with canonical
//!   arguments); everything else is canonical ([`Ast::reflection_type`]);
//! - a declaration, namespace or namespace alias is its first declaration;
//! - a template is the canonical `TemplateName` of its first declaration;
//! - a value or object is a content-interned `ConstValueId`.

use crate::ast::Ast;
use crate::ids::{
    AnnId, ConstValueId, DeclId, DmsId, QualType, RawId, ReflectionId, TemplateNameId,
};
use crate::mem::{HeapSize, MemReport, MemRow};
use crate::types::TypeView;
use kiln_source::fx::FxHashMap;

/// The entity a reflection designates (spec/ast.md §10,
/// spec/reflection-repr.md §1).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Reflectee {
    /// The null reflection (a value-initialized `std::meta::info`).
    Null,
    /// A type: canonical, except that a top-level alias is kept
    /// ([`Ast::reflection_type`]).
    Type(QualType),
    /// A declaration that is not a namespace, namespace alias or template:
    /// variable, function, data member, enumerator, structured binding,
    /// parameter (first declaration).
    Decl(DeclId),
    /// A namespace, including the global namespace (first declaration).
    Namespace(DeclId),
    /// A namespace alias itself (`dealias` unwraps it).
    NamespaceAlias(DeclId),
    /// A class, variable, function, alias or concept template (canonical
    /// name of the first declaration).
    Template(TemplateNameId),
    /// A value of structural type (`reflect_constant`; the store value
    /// carries its type).
    Value(ConstValueId),
    /// An object of static storage duration (`reflect_object`): the
    /// interned pointer to it.
    Object(ConstValueId),
    /// The `index`-th direct base-specifier of class `derived` (first
    /// declaration).
    BaseSpec(DeclId, u32),
    /// A data member description (reserved: M6-08).
    DataMemberSpec(DmsId),
    /// An annotation (reserved).
    Annotation(AnnId),
}

const _: () = assert!(size_of::<Reflectee>() <= 12);

impl Reflectee {
    /// The declaration a `Decl`/`Namespace`/`NamespaceAlias` reflection
    /// keys on (the dense index's slot).
    fn decl_key(self) -> Option<DeclId> {
        match self {
            Reflectee::Decl(d) | Reflectee::Namespace(d) | Reflectee::NamespaceAlias(d) => Some(d),
            _ => None,
        }
    }

    /// The kind tag stored in a `CXXReflectExpr`'s `aux` (and printed by
    /// the dump).
    pub fn tag(self) -> u8 {
        match self {
            Reflectee::Type(_) => 0,
            Reflectee::Decl(_) => 1,
            Reflectee::Null => 2,
            Reflectee::Namespace(_) => 3,
            Reflectee::NamespaceAlias(_) => 4,
            Reflectee::Template(_) => 5,
            Reflectee::Value(_) => 6,
            Reflectee::Object(_) => 7,
            Reflectee::BaseSpec(..) => 8,
            Reflectee::DataMemberSpec(_) => 9,
            Reflectee::Annotation(_) => 10,
        }
    }

    /// The kind's name (`-ast-dump`, notes).
    pub fn kind_name(self) -> &'static str {
        match self {
            Reflectee::Null => "null",
            Reflectee::Type(_) => "type",
            Reflectee::Decl(_) => "decl",
            Reflectee::Namespace(_) => "namespace",
            Reflectee::NamespaceAlias(_) => "namespace-alias",
            Reflectee::Template(_) => "template",
            Reflectee::Value(_) => "value",
            Reflectee::Object(_) => "object",
            Reflectee::BaseSpec(..) => "base",
            Reflectee::DataMemberSpec(_) => "data-member-spec",
            Reflectee::Annotation(_) => "annotation",
        }
    }

    /// The one-word payload stored in a `CXXReflectExpr`'s `a` (a
    /// `BaseSpec` stores its class; the index lives in the table).
    pub fn payload(self) -> u32 {
        match self {
            Reflectee::Null => 0,
            Reflectee::Type(t) => t.raw(),
            Reflectee::Decl(d)
            | Reflectee::Namespace(d)
            | Reflectee::NamespaceAlias(d)
            | Reflectee::BaseSpec(d, _) => d.raw(),
            Reflectee::Template(n) => n.raw(),
            Reflectee::Value(v) | Reflectee::Object(v) => v.raw(),
            Reflectee::DataMemberSpec(m) => m.raw(),
            Reflectee::Annotation(a) => a.raw(),
        }
    }

    /// The reflectee a `(tag, payload)` pair encodes, for the one-word
    /// kinds (`None` for `BaseSpec` and unknown tags: those are read from
    /// the table).
    pub fn decode(tag: u8, a: u32) -> Option<Reflectee> {
        Some(match tag {
            0 => Reflectee::Type(QualType::from_raw(a).unwrap_or(QualType::ERROR)),
            1 => Reflectee::Decl(DeclId::try_new(a)?),
            2 => Reflectee::Null,
            3 => Reflectee::Namespace(DeclId::try_new(a)?),
            4 => Reflectee::NamespaceAlias(DeclId::try_new(a)?),
            5 => Reflectee::Template(TemplateNameId::try_new(a)?),
            6 => Reflectee::Value(ConstValueId::try_new(a)?),
            7 => Reflectee::Object(ConstValueId::try_new(a)?),
            9 => Reflectee::DataMemberSpec(DmsId::try_new(a)?),
            10 => Reflectee::Annotation(AnnId::try_new(a)?),
            _ => return None,
        })
    }
}

/// Interned reflections (spec/ast.md §10).
#[derive(Debug)]
pub struct ReflectionTable {
    items: Vec<Reflectee>,
    /// Reflections not found through `by_decl`.
    map: FxHashMap<Reflectee, ReflectionId>,
    /// Dense `DeclId index -> ReflectionId index` (0 = none) for the
    /// declaration kinds, the common case (append-only; 4 B per declaration
    /// up to the highest one reflected; spec/reflection-repr.md §3).
    by_decl: Vec<u32>,
}

impl Default for ReflectionTable {
    fn default() -> Self {
        let mut map = FxHashMap::default();
        map.insert(Reflectee::Null, ReflectionId::new(1));
        ReflectionTable { items: vec![Reflectee::Null], map, by_decl: Vec::new() }
    }
}

impl ReflectionTable {
    /// The id of `r`, interning it.
    ///
    /// # Panics
    ///
    /// If more than `u32::MAX - 1` reflections are interned.
    pub fn intern(&mut self, r: Reflectee) -> ReflectionId {
        if let Some(id) = self.find(r) {
            return id;
        }
        let id =
            ReflectionId::new(u32::try_from(self.items.len() + 1).expect("reflection table full"));
        self.items.push(r);
        let slot = r.decl_key().map(|d| d.index() as usize);
        match slot {
            Some(i) if self.by_decl.get(i).is_none_or(|&v| v == 0) => {
                if self.by_decl.len() <= i {
                    self.by_decl.resize(i + 1, 0);
                }
                self.by_decl[i] = id.index();
            }
            _ => {
                self.map.insert(r, id);
            }
        }
        id
    }

    /// The id of `r` if it is interned.
    pub fn find(&self, r: Reflectee) -> Option<ReflectionId> {
        if let Some(d) = r.decl_key() {
            let hit = self.by_decl.get(d.index() as usize).and_then(|&v| ReflectionId::try_new(v));
            if let Some(id) = hit.filter(|&id| self.get(id) == r) {
                return Some(id);
            }
        }
        self.map.get(&r).copied()
    }

    /// The entity `id` designates (`Null` for an id this table did not
    /// hand out, e.g. a raw value forged by a bit cast).
    pub fn get(&self, id: ReflectionId) -> Reflectee {
        self.items.get(id.index() as usize - 1).copied().unwrap_or(Reflectee::Null)
    }

    /// The number of interned reflections (including null).
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Always false: the null reflection is always present.
    pub fn is_empty(&self) -> bool {
        false
    }
}

impl HeapSize for ReflectionTable {
    fn heap_bytes(&self) -> MemReport {
        let mut r = MemReport::new();
        r.push(MemRow::of_vec("items", &self.items));
        r.push(MemRow::of_map("map", &self.map));
        r.push(MemRow::of_vec("by_decl", &self.by_decl));
        r
    }
}

impl ReflectionId {
    /// The null reflection (a value-initialized `std::meta::info`).
    pub const NULL: ReflectionId = ReflectionId::new(1);

    /// The Oven's 8-byte value of this reflection (`index - 1`, so null
    /// is all zeros).
    pub fn bits(self) -> u64 {
        u64::from(self.index() - 1)
    }

    /// The reflection whose Oven value is `v` (`None` past the id space:
    /// a value no table handed out).
    pub fn from_bits(v: u128) -> Option<ReflectionId> {
        u32::try_from(v).ok().and_then(|v| v.checked_add(1)).and_then(ReflectionId::try_new)
    }
}

impl Ast {
    /// The type `^^t` reflects ([expr.reflect]/6, [meta.reflection]
    /// `dealias`): a top-level alias is kept — a typedef or alias
    /// declaration (keyed on its first declaration, over the canonical
    /// underlying type) or an alias template specialization (canonical
    /// name and arguments) — when it carries no qualifiers of its own;
    /// `Elaborated`, `Using` and paren sugar above it are looked through.
    /// Everything else is canonical, so `^^const I == ^^const int` and
    /// `^^decltype(1) == ^^int`, but `^^I != ^^int`.
    pub fn reflection_type(&mut self, t: QualType) -> QualType {
        let mut cur = t;
        while cur.quals_bits() == 0 && !self.types.is_dependent(cur) {
            match self.types.view(cur.ty()) {
                TypeView::Elaborated { named, .. } => cur = named,
                TypeView::Using { underlying, .. } => cur = underlying,
                TypeView::Paren(inner) => cur = inner,
                TypeView::Typedef { decl, underlying } => {
                    let first = self.first(decl);
                    let u = self.types.canonical(underlying);
                    return self.types.typedef_type(first, u).into();
                }
                TypeView::TemplateSpecialization {
                    name,
                    args,
                    underlying: Some(u),
                    alias: true,
                } => {
                    let n = self.types.canonical_template_name(name);
                    let a = self.types.canonical_args(args);
                    let u = self.types.canonical(u);
                    return self.types.template_specialization(n, a, Some(u), true);
                }
                _ => break,
            }
        }
        self.types.canonical(t)
    }
}

impl Ast {
    /// The entity a `CXXReflectExpr` designates: the interned reflectee
    /// once it is not dependent, else the operand as written.
    pub fn reflect_operand(&self, e: crate::ids::ExprId) -> Reflectee {
        match self.exprs.reflect_value(e) {
            Some(id) => self.reflections.get(id),
            None => self.exprs.reflect_operand(e),
        }
    }
}
