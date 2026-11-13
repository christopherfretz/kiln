//! `TypeContext`: storage, queries and the interning core (spec/types.md
//! §3, §4, §7, §13). The per-kind constructors are in `canon.rs`.

use super::args::{ArgTable, ArgView};
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
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
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
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
pub enum ExprOperand {
    /// The expression as written.
    Expr(ExprId),
    /// The canonical profile; the first expression seen is
    /// `TypeContext::profile_expr`.
    Profile(ProfileId),
}

/// The operand of a `typeof`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
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
    /// The dummy node 0 and the reserved builtin ids 48..=63.
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
    /// The type of an `UnresolvedUsingTypename` declaration (WP-179).
    UnresolvedUsing(DeclId),
    /// Declarator parentheses around the inner type (clang's
    /// `ParenType`, WP-191).
    Paren(QualType),
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
    /// The template name of the first deduced class template
    /// specialization type formed for each canonical template name: clang
    /// uniques `DeducedTemplateSpecializationType` on the canonical
    /// template, so later undeduced spellings print as the first one.
    pub(crate) first_dtst_names: kiln_source::fx::FxHashMap<u32, TemplateNameId>,
    /// clang 19's first-spelling rule for undeduced placeholders
    /// ([`TypeContext::dtst_display_name`]); clang 22 prints each one as
    /// written (`-fclang-compat=22` clears it, WP-377).
    pub dtst_first_spelling: bool,
    /// Before C++17 ([`TypeOptions::noexcept_in_type`] false): for each
    /// non-canonical function node written with an exception
    /// specification, its canonical prototype carrying that written
    /// specification. The canonical type has none (the specification is
    /// not part of the type), but a declaration's own specification is
    /// still read through [`TypeContext::proto`] (clang's
    /// `getAs<FunctionProtoType>` stops at the first prototype in the
    /// sugar chain; spec/types.md §5.4).
    pub(crate) exc_protos: kiln_source::fx::FxHashMap<TypeId, FuncProto>,
    /// For each canonical dependent (template-)specialization type with
    /// a value-dependent expression argument: the argument list as
    /// written by the first specialization type that canonicalized to
    /// it. clang's canonical `TemplateSpecializationType` keeps the
    /// expressions of the spelling that created it, and mangles those
    /// (spec/abi.md §8.15, WP-790); [`TypeContext::arg_origin`].
    pub(crate) arg_origins: kiln_source::fx::FxHashMap<TypeId, ArgListId>,
    /// The same for a canonical dependent `decltype` or dependent-bound
    /// array type: the operand or bound written by the first `decltype`
    /// or array type that canonicalized to it
    /// ([`TypeContext::expr_origin`]).
    pub(crate) expr_origins: kiln_source::fx::FxHashMap<TypeId, ExprId>,
    /// For each non-dependent alias template specialization type whose
    /// written arguments differ from its converted ones (a non-type
    /// argument written as an expression, a defaulted argument, a pack):
    /// the canonical converted list, the specialization's identity
    /// ([temp.alias]/2, [temp.type]/1; [`TypeContext::alias_converted`]).
    /// The type itself keeps the written list for printing.
    pub(crate) alias_converted: kiln_source::fx::FxHashMap<TypeId, ArgListId>,
    pub(crate) nns: KeyedTable,
    pub(crate) args: ArgTable,
    pub(crate) profiles: ProfileTable,
    /// Reusable buffers of `Ast::profile_expr`.
    pub(crate) profile_scratch: super::profile_expr::ProfileScratch,
    pub(crate) target: TargetTypes,
    pub(crate) opts: TypeOptions,
    pub(crate) exhausted: bool,
    /// The bound every `[*]` variable length array of unspecified size
    /// (C17 6.7.6.2p4) shares, an integer literal `0` created by sema at
    /// the first `[*]`: the type prints as `T[*]`.
    pub vla_star_bound: Option<ExprId>,
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

/// First id after the builtins and their reserved range (48..=63).
pub const FIRST_USER_TYPE: u32 = 64;

#[inline]
fn q_opt(raw: u32) -> Option<QualType> {
    QualType::from_raw(raw)
}

impl TypeContext {
    /// The largest index of the template name, nested-name-specifier,
    /// argument list and profile tables (the snapshot range check).
    pub(crate) fn max_table_ids(&self) -> [usize; 4] {
        [
            self.names.entries.len().saturating_sub(1),
            self.nns.entries.len().saturating_sub(1),
            self.args.lists.len().saturating_sub(1),
            self.profiles.entries.len().saturating_sub(1),
        ]
    }

    /// A context for `target` with default (C++17+) options, with the
    /// builtins at ids 1..=90 (§5.1).
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
            first_dtst_names: kiln_source::fx::FxHashMap::default(),
            dtst_first_spelling: true,
            exc_protos: kiln_source::fx::FxHashMap::default(),
            arg_origins: kiln_source::fx::FxHashMap::default(),
            expr_origins: kiln_source::fx::FxHashMap::default(),
            alias_converted: kiln_source::fx::FxHashMap::default(),
            nns: KeyedTable::new(),
            args: ArgTable::new(),
            profiles: ProfileTable::new(),
            profile_scratch: super::profile_expr::ProfileScratch::default(),
            target,
            opts,
            exhausted: false,
            vla_star_bound: None,
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
    /// Before C++17 its exception specification is the one written on the
    /// first function type in `q`'s sugar chain (the canonical type has
    /// none; spec/types.md §5.4).
    pub fn proto(&self, q: QualType) -> Option<&FuncProto> {
        if !self.exc_protos.is_empty()
            && let Some(p) = self.written_exc_proto(q)
        {
            return Some(p);
        }
        self.proto_of(self.canonical(q).ty())
    }

    /// Before C++17: the prototype recorded in `exc_protos` for the first
    /// function node in `q`'s sugar chain.
    fn written_exc_proto(&self, mut q: QualType) -> Option<&FuncProto> {
        loop {
            if self.kind(q.ty()) == TypeKind::Function {
                return self.exc_protos.get(&q.ty());
            }
            if self.is_canonical(q.ty()) {
                return None;
            }
            let n = self.desugar_once(q);
            if n.ty() == q.ty() {
                return None;
            }
            q = n;
        }
    }

    /// Before C++17: `q` with no function type in it that keeps a
    /// written exception specification (its canonical type when it has
    /// one, else `q` itself). A type template argument is converted with
    /// it, so a template argument's identity, its substitution and what a
    /// call through it may throw follow the canonical type, as clang's
    /// do (spec/types.md §5.4).
    pub fn without_exc_sugar(&self, q: QualType) -> QualType {
        if self.exc_protos.is_empty() || !self.has_exc_sugar(q, 0) {
            return q;
        }
        self.canonical(q)
    }

    /// `q`'s structure holds a function node recorded in `exc_protos`.
    fn has_exc_sugar(&self, q: QualType, depth: u32) -> bool {
        // A canonical type has none; the depth guards a degenerate chain.
        if self.is_canonical(q.ty()) || depth > 256 {
            return false;
        }
        if self.exc_protos.contains_key(&q.ty()) {
            return true;
        }
        let d = depth + 1;
        match self.view(q.ty()) {
            TypeView::Pointer(p)
            | TypeView::LValueRef(p)
            | TypeView::RValueRef(p)
            | TypeView::IncompleteArray(p)
            | TypeView::Paren(p) => self.has_exc_sugar(p, d),
            TypeView::MemberPointer { pointee, .. } => self.has_exc_sugar(pointee, d),
            TypeView::ConstantArray(a) => self.has_exc_sugar(a.elem, d),
            TypeView::Function(p) => {
                self.has_exc_sugar(p.ret(), d)
                    || self.params(&p).iter().any(|&t| self.has_exc_sugar(t, d))
            }
            _ => {
                let n = self.desugar_once(q);
                n.ty() != q.ty() && self.has_exc_sugar(n, d)
            }
        }
    }

    /// The pointee of pointer, reference or member pointer `q` as written
    /// (sugar kept), for reading a pointee function type's exception
    /// specification before C++17 through [`TypeContext::proto`]; the
    /// canonical pointee otherwise.
    pub fn exc_pointee(&self, q: QualType) -> Option<QualType> {
        if !self.exc_protos.is_empty() {
            let mut w = q;
            loop {
                match self.view(w.ty()) {
                    TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => {
                        return Some(p);
                    }
                    TypeView::MemberPointer { pointee, .. } => return Some(pointee),
                    _ => {}
                }
                let n = self.desugar_once(w);
                if self.is_canonical(w.ty()) || n.ty() == w.ty() {
                    break;
                }
                w = n;
            }
        }
        self.pointee(q)
    }

    /// The parameter types of `p` (adjusted, [dcl.fct]/5).
    pub fn params(&self, p: &FuncProto) -> &[QualType] {
        self.store.list(p.params, u32::from(p.n_params))
    }

    /// The decoded exception specification of `p`.
    pub fn exc_spec(&self, p: &FuncProto) -> ExcSpec {
        let op = p.exc_operand();
        match p.exc_tag() {
            ExcSpec::TAG_NOTHROW if op == 1 => ExcSpec::NothrowAttr,
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
            K::UnresolvedUsing => TypeView::UnresolvedUsing(d(n.data)),
            K::Paren => TypeView::Paren(q(n.data)),
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

    /// The canonical node of `q` (no [`TypeView`] decoding).
    #[inline]
    pub fn canonical_node(&self, q: QualType) -> TypeNode {
        self.node(self.canonical(q).ty())
    }

    /// The node's `QualType` operand (as [`Self::view`] decodes it).
    #[inline]
    fn q_of(raw: u32) -> QualType {
        QualType::from_raw(raw).unwrap_or(QualType::ERROR)
    }

    /// The node's `DeclId` operand (as [`Self::view`] decodes it).
    #[inline]
    fn d_of(raw: u32) -> DeclId {
        DeclId::try_new(raw).unwrap_or(DeclId::new(1))
    }

    /// [`Self::view`] of `t` when its own kind is `kind`, else `None`:
    /// for callers that match one variant and would otherwise decode
    /// every node they test.
    #[inline]
    pub fn view_if(&self, t: TypeId, kind: TypeKind) -> Option<TypeView> {
        (self.kind(t) == kind).then(|| self.view(t))
    }

    /// The builtin `q` canonically is, if any.
    #[inline]
    pub fn builtin(&self, q: QualType) -> Option<BuiltinType> {
        let n = self.canonical_node(q);
        if n.kind() == TypeKind::Builtin { BuiltinType::from_index(n.data) } else { None }
    }

    /// True if `q` is canonically the error type (any cv).
    #[inline]
    pub fn is_error(&self, q: QualType) -> bool {
        let n = self.canonical_node(q);
        n.kind() == TypeKind::Builtin && n.data == BuiltinType::Error as u32
    }

    /// The pointee of a (canonical) pointer, reference or member pointer.
    #[inline]
    pub fn pointee(&self, q: QualType) -> Option<QualType> {
        let n = self.canonical_node(q);
        match n.kind() {
            TypeKind::Pointer | TypeKind::LValueRef | TypeKind::RValueRef => {
                Some(Self::q_of(n.data))
            }
            TypeKind::MemberPointer => Some(Self::q_of(self.store.pair(n.data)[0])),
            _ => None,
        }
    }

    /// The pointee of a pointer or reference type with its sugar (clang's
    /// `getPointeeType` on the written type: through the first pointer or
    /// reference node in `q`'s sugar); [`Self::pointee`] (canonical) for
    /// any other type.
    pub fn pointee_sugared(&self, q: QualType) -> Option<QualType> {
        let mut cur = q;
        loop {
            match self.view(cur.ty()) {
                TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => {
                    return Some(p);
                }
                _ => {
                    let next = self.desugar_once(cur);
                    if next == cur {
                        return self.pointee(q);
                    }
                    cur = next;
                }
            }
        }
    }

    /// The element type of a (canonical) array, vector or complex type.
    #[inline]
    pub fn element(&self, q: QualType) -> Option<QualType> {
        self.node_element(self.canonical_node(q))
    }

    /// The element of node `n` if it is an array, vector or complex node.
    #[inline]
    fn node_element(&self, n: TypeNode) -> Option<QualType> {
        match n.kind() {
            TypeKind::ConstantArray => Some(self.store.arrays[n.data as usize].elem),
            TypeKind::IncompleteArray | TypeKind::Complex => Some(Self::q_of(n.data)),
            TypeKind::VariableArray
            | TypeKind::DependentSizedArray
            | TypeKind::Vector
            | TypeKind::DependentVector => Some(Self::q_of(self.store.pair(n.data)[0])),
            _ => None,
        }
    }

    /// The bound of a (canonical) constant array.
    #[inline]
    pub fn array_size(&self, q: QualType) -> Option<u64> {
        let n = self.canonical_node(q);
        (n.kind() == TypeKind::ConstantArray).then(|| self.store.arrays[n.data as usize].size)
    }

    /// The value type `T` of an `_Atomic(T)` (canonically; the
    /// qualifiers on `_Atomic(T)` itself are not included).
    #[inline]
    pub fn as_atomic(&self, q: QualType) -> Option<QualType> {
        let n = self.canonical_node(q);
        (n.kind() == TypeKind::Atomic).then(|| Self::q_of(n.data))
    }

    /// The element type `T` of a GNU `_Complex T` (canonically).
    #[inline]
    pub fn as_complex(&self, q: QualType) -> Option<QualType> {
        let n = self.canonical_node(q);
        (n.kind() == TypeKind::Complex).then(|| Self::q_of(n.data))
    }

    /// The class of a (canonical) record type.
    #[inline]
    pub fn as_record(&self, q: QualType) -> Option<DeclId> {
        let n = self.canonical_node(q);
        match n.kind() {
            TypeKind::Record => Some(Self::d_of(n.data)),
            TypeKind::TemplateSpecialization => self.injected_record(q),
            _ => None,
        }
    }

    /// The injected-class-name type `X<P...>` (the type of `*this`
    /// in the pattern of class template `X`) names the pattern
    /// ([temp.dep.type]/1; clang's `InjectedClassNameType`). A
    /// specialization written out is not looked through: its
    /// canonical type is shared with every `X<P...>` at the same
    /// depths, inside `X` or not.
    #[cold]
    fn injected_record(&self, q: QualType) -> Option<DeclId> {
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

    /// The enumeration of a (canonical) enum type.
    #[inline]
    pub fn as_enum(&self, q: QualType) -> Option<DeclId> {
        let n = self.canonical_node(q);
        (n.kind() == TypeKind::Enum).then(|| Self::d_of(n.data))
    }

    /// True for `void` (any cv).
    pub fn is_void(&self, q: QualType) -> bool {
        self.canonical(q).ty() == TypeId::VOID
    }

    /// True for the integral builtins and `_BitInt` ([basic.fundamental]).
    #[inline]
    pub fn is_integral(&self, q: QualType) -> bool {
        let n = self.canonical_node(q);
        match n.kind() {
            TypeKind::Builtin => {
                BuiltinType::from_index(n.data).is_some_and(BuiltinType::is_integral)
            }
            TypeKind::BitInt => true,
            _ => false,
        }
    }

    /// True for the floating-point builtins.
    pub fn is_floating(&self, q: QualType) -> bool {
        self.builtin(q).is_some_and(BuiltinType::is_floating)
    }

    /// True for the sizeless `AArch64` SVE builtins (§5.1).
    pub fn is_sizeless(&self, q: QualType) -> bool {
        self.builtin(q).is_some_and(BuiltinType::is_sizeless)
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
            let n = self.node(c.ty());
            match n.kind() {
                TypeKind::ConstantArray => c = self.store.arrays[n.data as usize].elem,
                TypeKind::IncompleteArray => c = Self::q_of(n.data),
                TypeKind::VariableArray | TypeKind::DependentSizedArray => {
                    c = Self::q_of(self.store.pair(n.data)[0]);
                }
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
            TypeView::Elaborated { named, .. } | TypeView::Paren(named) => add(named),
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

    /// The written argument list of the specialization type that created
    /// the canonical dependent specialization type `canonical`, when its
    /// arguments include a value-dependent expression: the list whose
    /// expressions name the type in a mangled name (clang mangles the
    /// creating spelling's expressions, so `R<T>::value` and `R<T,
    /// void>::value` mangle as the first one written in that enclosing
    /// type). `None` when the canonical list is all there is.
    pub fn arg_origin(&self, canonical: TypeId) -> Option<ArgListId> {
        self.arg_origins.get(&canonical).copied()
    }

    /// Records `written` as the origin of canonical type `c` if it is
    /// the first spelling with an expression argument
    /// ([`TypeContext::arg_origin`]).
    pub(crate) fn note_arg_origin(&mut self, c: TypeId, written: ArgListId) {
        if !self.arg_origins.contains_key(&c) && self.has_expr_arg(written) {
            self.arg_origins.insert(c, written);
        }
    }

    /// The canonical converted argument list of alias template
    /// specialization type `spec` (one argument per parameter, non-type
    /// arguments as values, defaults filled in): its identity
    /// ([temp.alias]/2, [temp.type]/1), so `A<1 + 2>` and `A<3>` reflect
    /// the same specialization. Falls back to the canonical written list
    /// when no converted list was recorded (a dependent specialization).
    pub fn alias_converted(&self, spec: TypeId, written: ArgListId) -> ArgListId {
        self.alias_converted.get(&spec).copied().unwrap_or_else(|| self.canonical_args(written))
    }

    /// Records `converted` (canonical) as the identity of the alias
    /// template specialization type `spec` written with arguments
    /// `written` ([`TypeContext::alias_converted`]); nothing is stored
    /// when the written list canonicalizes to it already.
    pub fn note_alias_converted(&mut self, spec: TypeId, written: ArgListId, converted: ArgListId) {
        if self.canonical_args(written) != converted {
            self.alias_converted.insert(spec, converted);
        }
    }

    /// The expression written by the `decltype` (or dependent-bound
    /// array, or dependent-size vector) type that created the canonical type `canonical`
    /// ([`TypeContext::arg_origin`]); else the profile's first
    /// expression in the TU.
    pub fn expr_origin(&self, canonical: TypeId, p: ProfileId) -> Option<ExprId> {
        self.expr_origins.get(&canonical).copied().or_else(|| self.profile_expr(p))
    }

    /// Records `e` as the origin of canonical type `c` if it is the
    /// first ([`TypeContext::expr_origin`]).
    pub(crate) fn note_expr_origin(&mut self, c: TypeId, e: ExprId) {
        self.expr_origins.entry(c).or_insert(e);
    }

    fn has_expr_arg(&self, l: ArgListId) -> bool {
        self.args(l).iter().any(|a| match a.view() {
            ArgView::Expr { profile: Some(_), .. } => true,
            ArgView::Pack(p) => self.has_expr_arg(p),
            _ => false,
        })
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
        r.push(MemRow::of_map("names.first_dtst", &self.first_dtst_names));
        r.push(MemRow::of_map("exc_protos", &self.exc_protos));
        r.push(MemRow::of_map("arg_origins", &self.arg_origins));
        r.push(MemRow::of_map("expr_origins", &self.expr_origins));
        r.push(MemRow::of_map("alias_converted", &self.alias_converted));
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
