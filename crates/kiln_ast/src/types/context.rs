//! `TypeContext`: storage, queries and the interning core (spec/types.md
//! §3, §4, §7, §13). The per-kind constructors are in `canon.rs`.

use super::args::ArgTable;
use super::func::{ExcSpec, FuncProto, NoexceptValue, TypeList};
use super::intern::{InternTable, KeyedTable, fx_hash};
use super::kinds::{
    AutoKeyword, BuiltinType, Quals, TagKeyword, TypeAttrKind, TypeFlags, TypeKind,
    UnaryTransformKind, VectorKind,
};
use super::layout::TargetTypes;
use super::node::{ConstArray, Key, Store, TypeNode, push_payload};
use super::profile::ProfileTable;
use crate::ids::{ArgListId, DeclId, ExprId, NnsId, ProfileId, QualType, TemplateNameId, TypeId};
use crate::mem::{HeapSize, MemReport, MemRow};
use kiln_source::symbol::Symbol;

/// Language options that change which types are distinct.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TypeOptions {
    /// The exception specification is part of the function type (C++17
    /// and later, [except.spec]/13 history). Before C++17 every canonical
    /// function type has `ExcSpec::None`.
    pub noexcept_in_type: bool,
}

impl Default for TypeOptions {
    fn default() -> Self {
        TypeOptions { noexcept_in_type: true }
    }
}

/// An expression operand of a type: the expression as written (sugar) or
/// the canonical profile (spec/types.md §7.2).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ExprOperand {
    /// The expression as written.
    Expr(ExprId),
    /// The canonical profile; the first expression seen is
    /// `TypeContext::profile_expr`.
    Profile(ProfileId),
}

/// The operand of a `typeof`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TypeOfOperand {
    /// `typeof(expr)`.
    Expr(ExprId),
    /// `typeof(type)`.
    Type(QualType),
    /// The canonical form of a dependent `typeof(expr)`.
    Profile(ProfileId),
}

/// A type node decoded into its operands (spec/types.md §15 R1). Obtained
/// with [`TypeContext::view`] (the node itself) or
/// [`TypeContext::canonical_view`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[allow(missing_docs)] // field names are the spec's operand names (§5)
pub enum TypeView {
    /// The dummy node 0 and the reserved builtin ids 34..=63.
    Invalid,
    Builtin(BuiltinType),
    BitInt {
        signed: bool,
        width: u32,
    },
    DependentBitInt {
        signed: bool,
        width: ProfileId,
    },
    Pointer(QualType),
    LValueRef(QualType),
    RValueRef(QualType),
    MemberPointer {
        pointee: QualType,
        class: TypeId,
    },
    ConstantArray(ConstArray),
    IncompleteArray(QualType),
    VariableArray {
        elem: QualType,
        size: ExprId,
    },
    DependentSizedArray {
        elem: QualType,
        size: ExprOperand,
    },
    Function(FuncProto),
    Record(DeclId),
    Enum(DeclId),
    Complex(QualType),
    Atomic(QualType),
    Vector {
        elem: QualType,
        count: u32,
        kind: VectorKind,
    },
    DependentVector {
        elem: QualType,
        size: ExprOperand,
        kind: VectorKind,
    },
    ExtQual {
        base: QualType,
        word: u32,
    },
    Typedef {
        decl: DeclId,
        underlying: QualType,
    },
    Using {
        shadow: DeclId,
        underlying: QualType,
    },
    Elaborated {
        keyword: TagKeyword,
        nns: Option<NnsId>,
        named: QualType,
    },
    QualifiedArray {
        quals: Quals,
        base: TypeId,
    },
    TemplateTypeParm(DeclId),
    CanonTemplateTypeParm {
        depth: u32,
        index: u32,
        pack: bool,
    },
    SubstTemplateTypeParm {
        replacement: QualType,
        associated: DeclId,
        index: u32,
        pack_index: Option<u32>,
    },
    SubstTemplateTypeParmPack {
        associated: DeclId,
        index: u32,
        pack: ArgListId,
    },
    TemplateSpecialization {
        name: TemplateNameId,
        args: ArgListId,
        underlying: Option<QualType>,
        alias: bool,
    },
    InjectedClassName {
        pattern: DeclId,
        injected: QualType,
    },
    DependentName {
        keyword: TagKeyword,
        nns: NnsId,
        name: Symbol,
    },
    DependentTemplateSpecialization {
        keyword: TagKeyword,
        nns: NnsId,
        name: Symbol,
        args: ArgListId,
    },
    PackExpansion {
        pattern: QualType,
        num_expansions: Option<u32>,
    },
    PackIndexing {
        pattern: QualType,
        index: ExprOperand,
        selected: Option<QualType>,
    },
    Decltype {
        expr: ExprId,
        underlying: QualType,
    },
    DependentDecltype(ProfileId),
    TypeOf {
        operand: TypeOfOperand,
        underlying: Option<QualType>,
        unqual: bool,
    },
    UnaryTransform {
        kind: UnaryTransformKind,
        base: QualType,
        result: Option<QualType>,
    },
    Auto {
        keyword: AutoKeyword,
        deduced: Option<QualType>,
        concept: Option<DeclId>,
        args: Option<ArgListId>,
        pack: bool,
    },
    DeducedTemplateSpecialization {
        name: TemplateNameId,
        deduced: Option<QualType>,
    },
    Attributed {
        kind: TypeAttrKind,
        modified: QualType,
        equivalent: QualType,
    },
    Splice {
        operand: ExprOperand,
        designated: Option<QualType>,
    },
}

/// `aux` bit marking the canonical, profile-keyed form of kinds whose
/// sugar form holds an `ExprId` (`DependentSizedArray`, `PackIndexing`,
/// `TypeOf`, Splice).
pub(crate) const AUX_PROFILE: u16 = 0x100;
/// `aux` bits of `TypeOf`.
pub(crate) const TYPEOF_UNQUAL: u16 = 1;
pub(crate) const TYPEOF_TYPE: u16 = 2;

/// The type arena of one translation unit (spec/types.md §4).
#[derive(Debug)]
pub struct TypeContext {
    pub(crate) store: Store,
    pub(crate) intern: InternTable,
    pub(crate) names: KeyedTable,
    pub(crate) name_decls: Vec<DeclId>,
    /// Templates declared in a dependent context (members of class
    /// templates): their names are dependent ([temp.dep.type]/9).
    pub(crate) dependent_templates: kiln_source::fx::FxHashSet<DeclId>,
    pub(crate) nns: KeyedTable,
    pub(crate) args: ArgTable,
    pub(crate) profiles: ProfileTable,
    pub(crate) target: TargetTypes,
    pub(crate) opts: TypeOptions,
    pub(crate) exhausted: bool,
}

impl Default for TypeContext {
    /// A context for an unknown target (all builtin sizes 0) with C++17+
    /// options; real compilations use [`TypeContext::with_options`].
    fn default() -> Self {
        TypeContext::new(TargetTypes::default())
    }
}

impl From<TypeId> for QualType {
    /// The unqualified type `t`.
    #[inline]
    fn from(t: TypeId) -> QualType {
        QualType::unqual(t)
    }
}

/// First id after the builtins and their reserved range (34..=63).
pub const FIRST_USER_TYPE: u32 = 64;

#[inline]
fn q_opt(raw: u32) -> Option<QualType> {
    QualType::from_raw(raw)
}

impl TypeContext {
    /// A context for `target` with default (C++17+) options, with the
    /// builtins at ids 1..=33 (§5.1).
    pub fn new(target: TargetTypes) -> Self {
        Self::with_options(target, TypeOptions::default())
    }

    /// A context for `target` with options `opts`.
    pub fn with_options(target: TargetTypes, opts: TypeOptions) -> Self {
        let mut ctx = TypeContext {
            store: Store::new(),
            intern: InternTable::default(),
            names: KeyedTable::new(),
            name_decls: Vec::new(),
            dependent_templates: kiln_source::fx::FxHashSet::default(),
            nns: KeyedTable::new(),
            args: ArgTable::new(),
            profiles: ProfileTable::new(),
            target,
            opts,
            exhausted: false,
        };
        for b in BuiltinType::ALL {
            let flags = match b {
                BuiltinType::Dependent => TypeFlags::DEPENDENT_INSTANTIATION,
                BuiltinType::Error => TypeFlags::CONTAINS_ERROR,
                _ => TypeFlags::NONE,
            };
            let key = Key::inline(TypeKind::Builtin, 0, b as u32);
            let hash = fx_hash(&key);
            let t = ctx.create(&key, flags, None, Some(hash));
            debug_assert_eq!(t, b.type_id());
        }
        while ctx.store.nodes.len() < FIRST_USER_TYPE as usize {
            let id = ctx.store.nodes.len() as u32;
            ctx.store.nodes.push(TypeNode { kind: 0, flags: 0, aux: 0, data: 0 });
            ctx.store.canon.push(QualType::unqual(TypeId::new(id)));
        }
        ctx.init_sentinels();
        ctx
    }

    /// The options this context was created with.
    pub fn options(&self) -> TypeOptions {
        self.opts
    }

    /// The target description (sizes of builtins etc., §11).
    pub fn target(&self) -> &TargetTypes {
        &self.target
    }

    /// True once an id space or arena ran out: the driver must emit a
    /// fatal "too many types" diagnostic. Constructors then return
    /// `QualType::ERROR` instead of panicking.
    pub fn exhausted(&self) -> bool {
        self.exhausted
    }

    /// Number of type nodes, including the builtins and reserved ids.
    pub fn len(&self) -> usize {
        self.store.nodes.len() - 1
    }

    /// Always false: the builtins exist from construction.
    pub fn is_empty(&self) -> bool {
        false
    }

    // ----- interning core -------------------------------------------------

    pub(crate) fn exhaust(&mut self) -> TypeId {
        self.exhausted = true;
        TypeId::ERROR
    }

    pub(crate) fn lookup(&self, key: &Key<'_>) -> Result<TypeId, u64> {
        let hash = fx_hash(key);
        let store = &self.store;
        match self.intern.find(hash, |id| store.key_of(id) == *key) {
            Some(id) => Ok(TypeId::new(id)),
            None => Err(hash),
        }
    }

    /// Creates a node for `key`. `canon == None` makes it canonical;
    /// `hash` (from [`Self::lookup`]) interns it, `None` creates a D/O
    /// node that is never found by lookup.
    pub(crate) fn create(
        &mut self,
        key: &Key<'_>,
        flags: TypeFlags,
        canon: Option<QualType>,
        hash: Option<u64>,
    ) -> TypeId {
        let id = self.store.nodes.len();
        if id > TypeId::MAX_INDEX as usize {
            return self.exhaust();
        }
        let Some((kind, aux, data)) = push_payload(&mut self.store, key) else {
            return self.exhaust();
        };
        let id = id as u32;
        let t = TypeId::new(id);
        let me = QualType::unqual(t);
        let canon = canon.unwrap_or(me);
        let mut flags = flags.without(TypeFlags::SUGAR);
        if canon != me {
            flags |= TypeFlags::SUGAR | self.qflags(canon);
        }
        self.store.nodes.push(TypeNode { kind, flags: flags.bits(), aux, data });
        self.store.canon.push(canon);
        if let Some(h) = hash {
            let store = &self.store;
            self.intern.insert(h, id, |i| fx_hash(&store.key_of(i)));
        }
        t
    }

    /// Looks `key` up; on a miss computes the canonical type with `canon`
    /// (which returns `None` when the node is its own canonical form) and
    /// creates an interned node.
    pub(crate) fn intern_with(
        &mut self,
        key: Key<'_>,
        flags: TypeFlags,
        canon: impl FnOnce(&mut Self) -> Option<QualType>,
    ) -> TypeId {
        match self.lookup(&key) {
            Ok(t) => t,
            Err(hash) => {
                let c = canon(self);
                self.create(&key, flags, c, Some(hash))
            }
        }
    }

    // ----- O(1) queries -----------------------------------------------------

    /// The raw node of `t`.
    #[inline]
    pub fn node(&self, t: TypeId) -> TypeNode {
        self.store.node(t)
    }

    /// The node's own kind (sugar kinds included).
    #[inline]
    pub fn kind(&self, t: TypeId) -> TypeKind {
        self.store.node(t).kind()
    }

    /// The flags of node `t` (§6).
    #[inline]
    pub fn flags(&self, t: TypeId) -> TypeFlags {
        self.store.node(t).flags()
    }

    /// The propagating flags of `q` (without `SUGAR`).
    #[inline]
    pub fn qflags(&self, q: QualType) -> TypeFlags {
        self.flags(q.ty()).without(TypeFlags::SUGAR)
    }

    /// The canonical type: one load and an OR (§3).
    #[inline]
    pub fn canonical(&self, q: QualType) -> QualType {
        let c = self.store.canon[q.ty().index() as usize];
        QualType::new(c.ty(), c.quals_bits() | q.quals_bits())
    }

    /// Alias of [`Self::canonical`] for readability at call sites.
    #[inline]
    pub fn desugar(&self, q: QualType) -> QualType {
        self.canonical(q)
    }

    /// True if `t` is its own canonical type.
    #[inline]
    pub fn is_canonical(&self, t: TypeId) -> bool {
        !self.flags(t).contains(TypeFlags::SUGAR)
    }

    /// Language type identity: `canonical(a) == canonical(b)`.
    #[inline]
    pub fn same_type(&self, a: QualType, b: QualType) -> bool {
        self.canonical(a) == self.canonical(b)
    }

    /// Identity ignoring top-level cv-qualifiers, including those of array
    /// elements (§2.1).
    pub fn same_unqualified(&self, a: QualType, b: QualType) -> bool {
        let (ca, cb) = (self.canonical(a), self.canonical(b));
        if ca.without_quals() == cb.without_quals() {
            return true;
        }
        self.same_unqualified_array(ca, cb)
    }

    fn same_unqualified_array(&self, a: QualType, b: QualType) -> bool {
        // Both canonical. Arrays are equal up to element cv iff their
        // shapes match and the elements are equal up to cv.
        let (na, nb) = (self.node(a.ty()), self.node(b.ty()));
        if na.kind != nb.kind || !na.kind().is_array() {
            return false;
        }
        match (self.view(a.ty()), self.view(b.ty())) {
            (TypeView::ConstantArray(x), TypeView::ConstantArray(y)) => {
                x.size == y.size && self.same_unqualified(x.elem, y.elem)
            }
            (TypeView::IncompleteArray(x), TypeView::IncompleteArray(y)) => {
                self.same_unqualified(x, y)
            }
            (
                TypeView::DependentSizedArray { elem: x, size: sx },
                TypeView::DependentSizedArray { elem: y, size: sy },
            ) => sx == sy && self.same_unqualified(x, y),
            _ => false,
        }
    }

    /// The kind of the canonical node of `q`.
    #[inline]
    pub fn canonical_kind(&self, q: QualType) -> TypeKind {
        self.kind(self.canonical(q).ty())
    }

    /// The canonical node of `q`, decoded.
    pub fn canonical_view(&self, q: QualType) -> TypeView {
        self.view(self.canonical(q).ty())
    }

    /// The function prototype of node `t` if it is a `Function` node.
    pub fn proto_of(&self, t: TypeId) -> Option<&FuncProto> {
        let n = self.node(t);
        (n.kind() == TypeKind::Function).then(|| &self.store.protos[n.data as usize])
    }

    /// The canonical function prototype of `q`, if `q` is a function type.
    pub fn proto(&self, q: QualType) -> Option<&FuncProto> {
        self.proto_of(self.canonical(q).ty())
    }

    /// The parameter types of `p` (adjusted, [dcl.fct]/5).
    pub fn params(&self, p: &FuncProto) -> &[QualType] {
        self.store.list(p.params, u32::from(p.n_params))
    }

    /// The decoded exception specification of `p`.
    pub fn exc_spec(&self, p: &FuncProto) -> ExcSpec {
        let op = p.exc_operand();
        match p.exc_tag() {
            ExcSpec::TAG_NOTHROW => ExcSpec::Nothrow,
            ExcSpec::TAG_DYNAMIC_NONE => ExcSpec::DynamicNone,
            ExcSpec::TAG_BASIC_NOEXCEPT => ExcSpec::BasicNoexcept,
            ExcSpec::TAG_NOEXCEPT_EXPR => {
                let [e, tag, p] = self.store.triple(op);
                let value = match (tag, ProfileId::try_new(p)) {
                    (2, Some(p)) => NoexceptValue::Dependent(p),
                    (1, _) => NoexceptValue::True,
                    _ => NoexceptValue::False,
                };
                match ExprId::try_new(e) {
                    Some(expr) => ExcSpec::NoexceptExpr { expr, value },
                    None => ExcSpec::None,
                }
            }
            ExcSpec::TAG_DYNAMIC => {
                let [start, len] = self.store.pair(op);
                ExcSpec::Dynamic(TypeList { start, len })
            }
            ExcSpec::TAG_DEPENDENT => {
                ProfileId::try_new(op).map_or(ExcSpec::None, ExcSpec::DependentNoexcept)
            }
            ExcSpec::TAG_DEFERRED => DeclId::try_new(op).map_or(ExcSpec::None, ExcSpec::Deferred),
            _ => ExcSpec::None,
        }
    }

    /// The types of a dynamic exception specification.
    pub fn type_list_types(&self, l: TypeList) -> &[QualType] {
        self.store.list(l.start, l.len)
    }

    /// Stores a list of types (for `ExcSpec::Dynamic`).
    pub fn type_list(&mut self, types: &[QualType]) -> TypeList {
        if types.is_empty() {
            return TypeList::EMPTY;
        }
        if !Store::fits(self.store.type_lists.len(), types.len()) {
            self.exhausted = true;
            return TypeList::EMPTY;
        }
        let start = self.store.type_lists.len() as u32;
        self.store.type_lists.extend_from_slice(types);
        TypeList { start, len: types.len() as u32 }
    }

    /// Decodes node `t`.
    #[allow(clippy::too_many_lines, reason = "one arm per type kind")]
    pub fn view(&self, t: TypeId) -> TypeView {
        use TypeKind as K;
        let n = self.node(t);
        let s = &self.store;
        let q = |raw: u32| QualType::from_raw(raw).unwrap_or(QualType::ERROR);
        let d = |raw: u32| DeclId::try_new(raw).unwrap_or(DeclId::new(1));
        let p = |raw: u32| ProfileId::try_new(raw).unwrap_or(ProfileId::new(1));
        let operand = |aux: u16, raw: u32| {
            if aux & AUX_PROFILE != 0 {
                ExprOperand::Profile(p(raw))
            } else {
                ExprOperand::Expr(ExprId::try_new(raw).unwrap_or(ExprId::new(1)))
            }
        };
        match n.kind() {
            K::Invalid => TypeView::Invalid,
            K::Builtin => {
                BuiltinType::from_index(n.data).map_or(TypeView::Invalid, TypeView::Builtin)
            }
            K::BitInt => TypeView::BitInt { signed: n.aux != 0, width: n.data },
            K::DependentBitInt => {
                TypeView::DependentBitInt { signed: n.aux != 0, width: p(n.data) }
            }
            K::Pointer => TypeView::Pointer(q(n.data)),
            K::LValueRef => TypeView::LValueRef(q(n.data)),
            K::RValueRef => TypeView::RValueRef(q(n.data)),
            K::MemberPointer => {
                let [a, b] = s.pair(n.data);
                TypeView::MemberPointer {
                    pointee: q(a),
                    class: TypeId::try_new(b).unwrap_or(TypeId::ERROR),
                }
            }
            K::ConstantArray => TypeView::ConstantArray(s.arrays[n.data as usize]),
            K::IncompleteArray => TypeView::IncompleteArray(q(n.data)),
            K::VariableArray => {
                let [a, b] = s.pair(n.data);
                TypeView::VariableArray {
                    elem: q(a),
                    size: ExprId::try_new(b).unwrap_or(ExprId::new(1)),
                }
            }
            K::DependentSizedArray => {
                let [a, b] = s.pair(n.data);
                TypeView::DependentSizedArray { elem: q(a), size: operand(n.aux, b) }
            }
            K::Function => TypeView::Function(s.protos[n.data as usize]),
            K::Record => TypeView::Record(d(n.data)),
            K::Enum => TypeView::Enum(d(n.data)),
            K::Complex => TypeView::Complex(q(n.data)),
            K::Atomic => TypeView::Atomic(q(n.data)),
            K::Vector => {
                let [a, b] = s.pair(n.data);
                TypeView::Vector { elem: q(a), count: b, kind: VectorKind::from_u8(n.aux as u8) }
            }
            K::DependentVector => {
                let [a, b] = s.pair(n.data);
                TypeView::DependentVector {
                    elem: q(a),
                    size: operand(n.aux, b),
                    kind: VectorKind::from_u8(n.aux as u8),
                }
            }
            K::ExtQual => {
                let [a, b] = s.pair(n.data);
                TypeView::ExtQual { base: q(a), word: b }
            }
            K::Typedef => {
                let [a, b] = s.pair(n.data);
                TypeView::Typedef { decl: d(a), underlying: q(b) }
            }
            K::Using => {
                let [a, b] = s.pair(n.data);
                TypeView::Using { shadow: d(a), underlying: q(b) }
            }
            K::Elaborated => {
                let [a, b] = s.pair(n.data);
                TypeView::Elaborated {
                    keyword: TagKeyword::from_u8(n.aux as u8),
                    nns: NnsId::try_new(a),
                    named: q(b),
                }
            }
            K::QualifiedArray => TypeView::QualifiedArray {
                quals: Quals::from_bits(n.aux as u8),
                base: TypeId::try_new(n.data).unwrap_or(TypeId::ERROR),
            },
            K::TemplateTypeParm => TypeView::TemplateTypeParm(d(n.data)),
            K::CanonTemplateTypeParm => TypeView::CanonTemplateTypeParm {
                depth: u32::from(n.aux & 0x7fff),
                index: n.data,
                pack: n.aux & 0x8000 != 0,
            },
            K::SubstTemplateTypeParm => {
                let [a, b, c] = s.triple(n.data);
                TypeView::SubstTemplateTypeParm {
                    replacement: q(a),
                    associated: d(b),
                    index: c,
                    pack_index: u32::from(n.aux).checked_sub(1),
                }
            }
            K::SubstTemplateTypeParmPack => {
                let [a, b, c] = s.triple(n.data);
                TypeView::SubstTemplateTypeParmPack {
                    associated: d(a),
                    index: b,
                    pack: ArgListId::try_new(c).unwrap_or(ArgListId::EMPTY),
                }
            }
            K::TemplateSpecialization => {
                let [a, b, c] = s.triple(n.data);
                TypeView::TemplateSpecialization {
                    name: TemplateNameId::try_new(a).unwrap_or(TemplateNameId::new(1)),
                    args: ArgListId::try_new(b).unwrap_or(ArgListId::EMPTY),
                    underlying: q_opt(c),
                    alias: n.aux & 1 != 0,
                }
            }
            K::InjectedClassName => {
                let [a, b] = s.pair(n.data);
                TypeView::InjectedClassName { pattern: d(a), injected: q(b) }
            }
            K::DependentName => {
                let [a, b] = s.pair(n.data);
                TypeView::DependentName {
                    keyword: TagKeyword::from_u8(n.aux as u8),
                    nns: NnsId::try_new(a).unwrap_or(NnsId::new(1)),
                    name: Symbol::from_index(b),
                }
            }
            K::DependentTemplateSpecialization => {
                let [a, b, c] = s.triple(n.data);
                TypeView::DependentTemplateSpecialization {
                    keyword: TagKeyword::from_u8(n.aux as u8),
                    nns: NnsId::try_new(a).unwrap_or(NnsId::new(1)),
                    name: Symbol::from_index(b),
                    args: ArgListId::try_new(c).unwrap_or(ArgListId::EMPTY),
                }
            }
            K::PackExpansion => {
                let [a, b] = s.pair(n.data);
                TypeView::PackExpansion { pattern: q(a), num_expansions: b.checked_sub(1) }
            }
            K::PackIndexing => {
                let [a, b, c] = s.triple(n.data);
                TypeView::PackIndexing {
                    pattern: q(a),
                    index: operand(n.aux, b),
                    selected: q_opt(c),
                }
            }
            K::Decltype => {
                let [a, b] = s.pair(n.data);
                TypeView::Decltype {
                    expr: ExprId::try_new(a).unwrap_or(ExprId::new(1)),
                    underlying: q(b),
                }
            }
            K::DependentDecltype => TypeView::DependentDecltype(p(n.data)),
            K::TypeOf => {
                let [a, b] = s.pair(n.data);
                let operand = if n.aux & AUX_PROFILE != 0 {
                    TypeOfOperand::Profile(p(a))
                } else if n.aux & TYPEOF_TYPE != 0 {
                    TypeOfOperand::Type(q(a))
                } else {
                    TypeOfOperand::Expr(ExprId::try_new(a).unwrap_or(ExprId::new(1)))
                };
                TypeView::TypeOf {
                    operand,
                    underlying: q_opt(b),
                    unqual: n.aux & TYPEOF_UNQUAL != 0,
                }
            }
            K::UnaryTransform => {
                let [a, b] = s.pair(n.data);
                TypeView::UnaryTransform {
                    kind: UnaryTransformKind::from_u8(n.aux as u8)
                        .unwrap_or(UnaryTransformKind::UnderlyingType),
                    base: q(a),
                    result: q_opt(b),
                }
            }
            K::Auto => {
                let [a, b, c] = s.triple(n.data);
                TypeView::Auto {
                    keyword: AutoKeyword::from_u8((n.aux & 3) as u8),
                    deduced: q_opt(a),
                    concept: DeclId::try_new(b),
                    args: ArgListId::try_new(c),
                    pack: n.aux & 4 != 0,
                }
            }
            K::DeducedTemplateSpecialization => {
                let [a, b] = s.pair(n.data);
                TypeView::DeducedTemplateSpecialization {
                    name: TemplateNameId::try_new(a).unwrap_or(TemplateNameId::new(1)),
                    deduced: q_opt(b),
                }
            }
            K::Attributed => {
                let [a, b] = s.pair(n.data);
                TypeView::Attributed {
                    kind: TypeAttrKind::from_u16(n.aux),
                    modified: q(a),
                    equivalent: q(b),
                }
            }
            K::Splice => {
                let [a, b] = s.pair(n.data);
                TypeView::Splice { operand: operand(n.aux, a), designated: q_opt(b) }
            }
        }
    }

    /// The builtin `q` canonically is, if any.
    pub fn builtin(&self, q: QualType) -> Option<BuiltinType> {
        match self.canonical_view(q) {
            TypeView::Builtin(b) => Some(b),
            _ => None,
        }
    }

    /// The pointee of a (canonical) pointer, reference or member pointer.
    pub fn pointee(&self, q: QualType) -> Option<QualType> {
        match self.canonical_view(q) {
            TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => Some(p),
            TypeView::MemberPointer { pointee, .. } => Some(pointee),
            _ => None,
        }
    }

    /// The element type of a (canonical) array, vector or complex type.
    pub fn element(&self, q: QualType) -> Option<QualType> {
        match self.canonical_view(q) {
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

    /// The bound of a (canonical) constant array.
    pub fn array_size(&self, q: QualType) -> Option<u64> {
        match self.canonical_view(q) {
            TypeView::ConstantArray(a) => Some(a.size),
            _ => None,
        }
    }

    /// The class of a (canonical) record type.
    pub fn as_record(&self, q: QualType) -> Option<DeclId> {
        match self.canonical_view(q) {
            TypeView::Record(d) => Some(d),
            // The injected-class-name type `X<P...>` (the type of `*this`
            // in the pattern of class template `X`) names the pattern
            // ([temp.dep.type]/1; clang's `InjectedClassNameType`). A
            // specialization written out is not looked through: its
            // canonical type is shared with every `X<P...>` at the same
            // depths, inside `X` or not.
            TypeView::TemplateSpecialization { .. } => {
                let mut t = q;
                loop {
                    if let TypeView::InjectedClassName { pattern, .. } = self.view(t.ty()) {
                        return Some(pattern);
                    }
                    let n = self.desugar_once(t);
                    if n == t {
                        return None;
                    }
                    t = n;
                }
            }
            _ => None,
        }
    }

    /// The enumeration of a (canonical) enum type.
    pub fn as_enum(&self, q: QualType) -> Option<DeclId> {
        match self.canonical_view(q) {
            TypeView::Enum(d) => Some(d),
            _ => None,
        }
    }

    /// True for `void` (any cv).
    pub fn is_void(&self, q: QualType) -> bool {
        self.canonical(q).ty() == TypeId::VOID
    }

    /// True for the integral builtins and `_BitInt` ([basic.fundamental]).
    pub fn is_integral(&self, q: QualType) -> bool {
        match self.canonical_view(q) {
            TypeView::Builtin(b) => b.is_integral(),
            TypeView::BitInt { .. } => true,
            _ => false,
        }
    }

    /// True for the floating-point builtins.
    pub fn is_floating(&self, q: QualType) -> bool {
        self.builtin(q).is_some_and(BuiltinType::is_floating)
    }

    /// True for integral and floating types ([basic.fundamental]/14).
    pub fn is_arithmetic(&self, q: QualType) -> bool {
        self.is_integral(q) || self.is_floating(q)
    }

    /// True if the canonical type is a pointer.
    pub fn is_pointer(&self, q: QualType) -> bool {
        self.canonical_kind(q) == TypeKind::Pointer
    }

    /// True if the canonical type is a reference.
    pub fn is_reference(&self, q: QualType) -> bool {
        self.canonical_kind(q).is_reference()
    }

    /// True if the canonical type is an array.
    pub fn is_array(&self, q: QualType) -> bool {
        self.canonical_kind(q).is_array()
    }

    /// True if the canonical type is a function type.
    pub fn is_function(&self, q: QualType) -> bool {
        self.canonical_kind(q) == TypeKind::Function
    }

    /// True for a dependent type.
    pub fn is_dependent(&self, q: QualType) -> bool {
        self.flags(q.ty()).is_dependent()
    }

    /// The qualifiers that apply to an object of type `q`: `q`'s own,
    /// plus those hidden in sugar, plus an array's element qualifiers
    /// (§2.1).
    pub fn effective_quals(&self, q: QualType) -> Quals {
        let mut c = self.canonical(q);
        loop {
            match self.view(c.ty()) {
                TypeView::ConstantArray(a) => c = a.elem,
                TypeView::IncompleteArray(e)
                | TypeView::VariableArray { elem: e, .. }
                | TypeView::DependentSizedArray { elem: e, .. } => c = e,
                _ => return c.quals(),
            }
            c = self.canonical(c);
        }
    }

    /// Removes one level of sugar (clang's single-step desugaring:
    /// `isSugared` kinds only). Returns `q` for non-sugar nodes and for
    /// the dependent kinds clang does not desugar (template type
    /// parameters, injected-class-names, dependent specializations).
    pub fn desugar_once(&self, q: QualType) -> QualType {
        let add = |t: QualType| QualType::new(t.ty(), t.quals_bits() | q.quals_bits());
        match self.view(q.ty()) {
            TypeView::Typedef { underlying, .. } | TypeView::Using { underlying, .. } => {
                add(underlying)
            }
            TypeView::Elaborated { named, .. } => add(named),
            TypeView::QualifiedArray { .. } => self.canonical(q),
            TypeView::SubstTemplateTypeParm { replacement, .. } => add(replacement),
            TypeView::TemplateSpecialization { underlying: Some(u), .. } => add(u),
            TypeView::Decltype { underlying, .. } if self.is_canonical_decltype_sugar(q) => {
                add(underlying)
            }
            TypeView::TypeOf { operand: TypeOfOperand::Expr(_), underlying: Some(u), unqual } => {
                if unqual {
                    self.canonical(q)
                } else if self.kind(self.canonical(q).ty()) == TypeKind::TypeOf {
                    q
                } else {
                    add(u)
                }
            }
            TypeView::TypeOf { operand: TypeOfOperand::Type(t), unqual, .. } => {
                if unqual {
                    self.canonical(q)
                } else {
                    add(t)
                }
            }
            TypeView::UnaryTransform { result: Some(r), .. } => add(r),
            TypeView::Auto { deduced: Some(d), .. }
            | TypeView::DeducedTemplateSpecialization { deduced: Some(d), .. }
            | TypeView::Splice { designated: Some(d), .. } => add(d),
            TypeView::Attributed { equivalent, .. } => add(equivalent),
            TypeView::PackIndexing { selected: Some(s), .. } => add(s),
            _ => q,
        }
    }

    fn is_canonical_decltype_sugar(&self, q: QualType) -> bool {
        self.kind(self.canonical(q).ty()) != TypeKind::DependentDecltype
    }

    /// Removes all top-level sugar ([`Self::desugar_once`] to a fixed
    /// point): what clang's AST dump prints as the second type string.
    pub fn desugar_top(&self, mut q: QualType) -> QualType {
        loop {
            let n = self.desugar_once(q);
            if n == q {
                return q;
            }
            q = n;
        }
    }

    // ----- names, nns, profiles -------------------------------------------

    /// Interns an expression profile (§7.2): the words of
    /// `profile_expr`, the first expression seen with that profile (for
    /// printing), and the expression's dependence flags.
    pub fn profile(&mut self, words: &[u32], first_expr: ExprId, flags: TypeFlags) -> ProfileId {
        if let Some(p) = self.profiles.intern(words, Some(first_expr), flags) {
            p
        } else {
            self.exhausted = true;
            ProfileId::new(1)
        }
    }

    /// The words of profile `p`.
    pub fn profile_words(&self, p: ProfileId) -> &[u32] {
        self.profiles.words(p)
    }

    /// The first expression interned with profile `p` (used to print
    /// canonical dependent types).
    pub fn profile_expr(&self, p: ProfileId) -> Option<ExprId> {
        self.profiles.first_expr(p)
    }

    /// The dependence flags recorded with profile `p`.
    pub fn profile_flags(&self, p: ProfileId) -> TypeFlags {
        self.profiles.flags(p)
    }

    fn init_sentinels(&mut self) {
        // Profile 1: the empty profile (exhaustion fallback).
        let _ = self.profiles.intern(&[], None, TypeFlags::NONE);
        // Argument list 1: the empty list (ArgListId::EMPTY).
        let empty = self.arg_list(&[]);
        debug_assert_eq!(empty, ArgListId::EMPTY);
        // Nns 1: `::` (exhaustion fallback).
        let g = self.nns(super::nns::Nns::Global);
        debug_assert_eq!(g.index(), 1);
        // Template name 1: the empty overload set (exhaustion fallback).
        let n = self
            .template_name(super::names::TemplateName::Overloaded(super::names::DeclSet::EMPTY));
        debug_assert_eq!(n.index(), 1);
    }

    // ----- memory -----------------------------------------------------------

    /// One row per arena (§9).
    pub fn memory(&self) -> MemReport {
        let s = &self.store;
        let mut r = MemReport::new();
        r.push(MemRow::of_vec("nodes", &s.nodes));
        r.push(MemRow::of_vec("canon", &s.canon));
        r.push(MemRow::new(
            "intern",
            self.intern.len() as u64,
            self.intern.live_bytes(),
            self.intern.heap_bytes(),
        ));
        r.push(MemRow::of_vec("protos", &s.protos));
        r.push(MemRow::of_vec("type_lists", &s.type_lists));
        r.push(MemRow::of_vec("pairs", &s.pairs));
        r.push(MemRow::of_vec("triples", &s.triples));
        r.push(MemRow::of_vec("arrays", &s.arrays));
        self.names.memory("names", &mut r);
        r.push(MemRow::of_vec("names.decls", &self.name_decls));
        r.push(MemRow::of_set("names.dependent_templates", &self.dependent_templates));
        self.nns.memory("nns", &mut r);
        self.args.memory(&mut r);
        self.profiles.memory(&mut r);
        r
    }
}

impl HeapSize for TypeContext {
    fn heap_bytes(&self) -> MemReport {
        self.memory()
    }
}
