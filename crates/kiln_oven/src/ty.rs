//! The Oven's type table: every type the evaluator touches, flattened to
//! what the VM needs (size, alignment, scalar kind, array extent, field
//! offsets). Built lazily from the AST through the bridge and kept for the
//! whole translation unit; the VM never looks at the AST's type graph.

use kiln_ast::consts::FloatKind;
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;
use kiln_ast::types::{BuiltinType, FloatFormat, TypeView};
use kiln_ast::{Ast, DeclId, ExprId, QualType};
use kiln_source::fx::{FxHashMap, FxHashSet};

use crate::SemaBridge;

mod path;

pub(crate) use path::{Hop, Walk, alt_members};

/// A type in the Oven's table.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub(crate) struct OTy(pub u32);

/// The shape of a type.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum TyKind {
    Void,
    Bool,
    Int {
        width: u8,
        signed: bool,
    },
    Float(FloatKind),
    /// Object or function pointer; the pointee.
    Ptr(OTy),
    NullPtr,
    Array {
        elem: OTy,
        n: u64,
    },
    /// A vector of `n` `bool` lanes (`elem`, the `bool` type): packed
    /// bits, lane `i` at bit `i % 8` of byte `i / 8`, in `ceil(n / 8)`
    /// bytes padded to the vector's size (clang's layout; spec/oven.md
    /// "Vectors", WP-450). The bytes hold no pointer and no union.
    BoolVec {
        elem: OTy,
        n: u64,
    },
    Record {
        decl: DeclId,
        union: bool,
        first: u32,
        count: u32,
    },
    Func,
    /// A pointer to member: an interned value index ([`crate::memptr`]),
    /// 0 for null; `func` for a pointer to member function.
    MemPtr {
        func: bool,
    },
    /// `std::meta::info` (WP-267 spike): a `ReflectionId` index stored as
    /// 8 plain bytes, 0 for the null reflection; frozen as
    /// `Scalar::Reflection`.
    Reflection,
    /// `_BitInt(N)` with `N > 128` (spec/oven.md "`_BitInt`"): a
    /// memory-resident integer of `size / 8` little-endian words, kept
    /// canonical (sign- or zero-extended from bit `width`, `crate::bigint`);
    /// registers hold its address, as for an aggregate.
    WideInt {
        width: u32,
        signed: bool,
    },
    /// Incomplete or not representable in M2 (member pointers,
    /// vectors, ...): it has no storage the VM can touch.
    Opaque,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TyInfo {
    /// Canonical, cv-unqualified.
    pub q: QualType,
    pub size: u64,
    pub align: u32,
    pub kind: TyKind,
    /// Contains a union subobject (loads and stores take the slow path).
    pub has_union: bool,
    /// Contains a pointer subobject.
    pub has_ptr: bool,
    /// Contains a subobject of polymorphic class type (whose vptr slots
    /// hold the Oven's dynamic-type words, WP-077).
    pub has_vptr: bool,
    /// Contains a `mutable` member subobject (reads of it from an object
    /// whose lifetime began outside the evaluation are diagnosed,
    /// [expr.const]/5.9).
    pub has_mutable: bool,
    /// Union subobjects at fixed offsets: `union_pos[upos.0..upos.1]`.
    pub upos: (u32, u32),
    /// The bytes `[lo, hi)` spanning every union subobject in `upos`
    /// (`(0, 0)` without one; [`Types::union_hull`]): an access outside
    /// them crosses no union (WP-760).
    pub uhull: (u32, u32),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FieldInfo {
    pub decl: DeclId,
    pub ty: OTy,
    /// Byte offset (for a bit-field: of the byte holding its first bit).
    pub offset: u64,
    /// Bit offset within that byte (bit-fields only).
    pub bit: u32,
    /// Bit-field width (`None` for an ordinary member).
    pub width: Option<u32>,
    /// A direct non-virtual base class subobject (`decl` is the base
    /// class's definition); bases precede the members in
    /// [`Types::fields`] (WP-076, [class.derived]).
    pub base: bool,
    /// A `mutable` member ([dcl.stc]/10).
    pub mutable: bool,
}

/// The value class of a scalar type (how many registers, which ops).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Scal {
    Int { width: u8, signed: bool },
    Bool,
    Float(FloatKind),
    Ptr,
}

#[derive(Debug, Default)]
pub(crate) struct Types {
    pub infos: Vec<TyInfo>,
    pub fields: Vec<FieldInfo>,
    pub union_pos: Vec<(u64, OTy)>,
    map: FxHashMap<QualType, OTy>,
    arrays: FxHashMap<(OTy, u64), OTy>,
    /// Declared constant arrays by (element, bound): arrays of
    /// differently qualified elements (`const E[3]`, `E[3]`) are one
    /// type, as clang's `getUnqualifiedArrayType` makes them.
    const_arrays: FxHashMap<(OTy, u64), OTy>,
    /// The `dsize` of each class whose data ends before its size (its
    /// tail padding may hold a derived class's members).
    data_sizes: FxHashMap<OTy, u64>,
    /// The direct non-virtual empty bases (type, offset) of each class
    /// with one: not in [`Types::fields`] (they hold no storage), but
    /// subobjects a pointer may designate ([`Types::has_subobject`]).
    empty_bases: FxHashMap<OTy, Vec<(OTy, u64)>>,
    /// Class slots interned while the class had no definition or was
    /// still being defined (`S*` in a constant expression inside `S`'s
    /// member-specification): kept `Opaque` and rebuilt in place once
    /// the class is complete ([`Types::refresh`]), so pointers already
    /// naming the slot stay valid and no layout of an incomplete class
    /// is computed or cached (WP-267). Keyed by the class's first
    /// declaration, so a completion finds its slot in O(1).
    provisional: FxHashMap<DeclId, OTy>,
    /// The provisional slots, for [`Types::intern`]'s membership test.
    provisional_slots: FxHashSet<OTy>,
    /// How many entries of `DeclArena::completed_records` (the AST's
    /// completion log: its length is the completion epoch) refresh has
    /// already applied: a refresh with no class completed since is O(1)
    /// (WP-480).
    completion_epoch: usize,
    /// The base-subobject view of each class with virtual bases: the
    /// class without its virtual bases (`nv_size`, Itanium C++ ABI 2.4),
    /// the type of a base field naming that class (P3533R2; spec/oven.md
    /// "Object model").
    nv_views: FxHashMap<OTy, OTy>,
    /// The class each such view belongs to.
    nv_class: FxHashMap<OTy, OTy>,
    /// The offset index of each record with at least [`INDEX_MIN`]
    /// fields ([`Types::near`]): the start of its rows in `idx_order` and
    /// `idx_reach`, one row per field.
    idx_start: FxHashMap<OTy, u32>,
    /// A record's field indices (into [`Types::fields`] of the record)
    /// sorted by (offset, index).
    idx_order: Vec<u32>,
    /// The greatest end (`offset + max(size, 1)`) of the fields up to and
    /// including each row of `idx_order`: once it is at most a byte, no
    /// earlier row holds the byte.
    idx_reach: Vec<u64>,
    /// The index in its record's fields of each member (not base) of a
    /// record with an offset index ([`Types::field_index`]).
    member_idx: FxHashMap<DeclId, u32>,
    /// The subobject path cache (WP-644, [`path`]): memoized
    /// memory-independent walk steps, bounded by [`PATH_CACHE_MAX`].
    paths: path::PathCache,
    /// How many provisional slots were rebuilt in place: a walk result
    /// remembered across a rebuild is stale (the union-path memo,
    /// `vm::umemo`, stamps its entries with it; WP-760).
    rebuilt: u32,
}

/// Records with fewer fields are scanned linearly ([`Types::near`]).
const INDEX_MIN: usize = 16;
/// The most fields [`Types::near`] narrows a lookup to, and the most rows
/// it walks to find them; past either it yields every field.
const NEAR_MAX: usize = 8;
const NEAR_WALK: usize = 32;

/// The fields of a record that may hold one byte, in declaration order
/// ([`Types::near`]).
pub(crate) enum Near<'a> {
    All(std::slice::Iter<'a, FieldInfo>),
    Few { fs: &'a [FieldInfo], idx: [u32; NEAR_MAX], n: u8, i: u8 },
}

impl<'a> Iterator for Near<'a> {
    type Item = &'a FieldInfo;

    fn next(&mut self) -> Option<&'a FieldInfo> {
        match self {
            Near::All(it) => it.next(),
            Near::Few { fs, idx, n, i } => {
                if *i >= *n {
                    return None;
                }
                let f = &fs[idx[usize::from(*i)] as usize];
                *i += 1;
                Some(f)
            }
        }
    }
}

/// Evaluates an integral constant already checked by sema (bit-field
/// widths, array bounds): a `ConstantExpr` with its value, or a literal
/// under parentheses and casts.
pub(crate) fn eval_int(ast: &Ast, mut e: ExprId) -> Option<u128> {
    for _ in 0..64 {
        match ast.exprs.get(e).kind() {
            ExprKind::ConstantExpr => {
                if let Some(v) = ast.exprs.constant_value(e) {
                    return const_int(ast, v);
                }
                e = ast.exprs.operand(e);
            }
            ExprKind::IntegerLiteral => {
                return match ast.exprs.int_value(e) {
                    Some(v) => Some(u128::from(v)),
                    None => ast.exprs.literal_const(e).and_then(|v| const_int(ast, v)),
                };
            }
            ExprKind::ParenExpr => e = ast.exprs.operand(e),
            ExprKind::SubstNonTypeTemplateParmExpr => e = ast.exprs.subst_replacement(e),
            k if k.is_cast() => e = ast.exprs.operand(e),
            _ => return None,
        }
    }
    None
}

fn const_int(ast: &Ast, v: kiln_ast::ConstValueId) -> Option<u128> {
    match ast.consts.scalar(v)? {
        kiln_ast::consts::Scalar::Int { bits, .. } => bits.to_u128(),
        kiln_ast::consts::Scalar::Bool { value, .. } => Some(u128::from(value)),
        _ => None,
    }
}

/// The float kind of a builtin floating type on the target.
pub(crate) fn float_kind(b: BuiltinType, long_double: FloatFormat) -> Option<FloatKind> {
    Some(match b {
        BuiltinType::Float16 | BuiltinType::Half => FloatKind::Half,
        BuiltinType::BFloat16 => FloatKind::BFloat16,
        BuiltinType::Float | BuiltinType::Float32 => FloatKind::Single,
        BuiltinType::Double | BuiltinType::Float64 | BuiltinType::Float32x => FloatKind::Double,
        BuiltinType::Float128 | BuiltinType::StdFloat128 => FloatKind::Quad,
        BuiltinType::LongDouble | BuiltinType::Float64x => match long_double {
            FloatFormat::IeeeSingle => FloatKind::Single,
            FloatFormat::X87Extended => FloatKind::X87Extended,
            FloatFormat::IeeeQuad => FloatKind::Quad,
            FloatFormat::IeeeHalf => FloatKind::Half,
            FloatFormat::BFloat => FloatKind::BFloat16,
            FloatFormat::IeeeDouble | FloatFormat::PpcDoubleDouble => FloatKind::Double,
        },
        _ => return None,
    })
}

impl Types {
    pub fn info(&self, t: OTy) -> &TyInfo {
        &self.infos[t.0 as usize]
    }

    /// The type canonical `c` is interned as, if it is.
    pub fn lookup(&self, c: QualType) -> Option<OTy> {
        self.map.get(&c).copied()
    }

    pub fn kind(&self, t: OTy) -> TyKind {
        self.infos[t.0 as usize].kind
    }

    pub fn size(&self, t: OTy) -> u64 {
        self.infos[t.0 as usize].size
    }

    /// The bytes a trivial copy of an object of type `t` writes: a
    /// class's `dsize`, without the tail padding a derived class may
    /// reuse ([class.copy.ctor]/14 copies the base subobject, not the
    /// derived members laid out in its padding; clang's
    /// `handleTrivialCopy` copies the value); any other type's size.
    pub fn copy_size(&self, t: OTy) -> u64 {
        self.data_sizes.get(&t).copied().unwrap_or_else(|| self.size(t))
    }

    pub fn fields(&self, t: OTy) -> &[FieldInfo] {
        match self.kind(t) {
            TyKind::Record { first, count, .. } => {
                &self.fields[first as usize..(first + count) as usize]
            }
            _ => &[],
        }
    }

    /// The fields of record `t` that may hold byte `off`, in declaration
    /// order: a superset of those with `offset <= off < offset + max(size,
    /// 1)` (bases, members and bit-fields alike, whose callers test the
    /// exact rule they need). A record with an offset index yields just
    /// the fields holding the byte, found by binary search, unless more
    /// than [`NEAR_MAX`] do (a union's members) or the search would walk
    /// past [`NEAR_WALK`] rows; then, like a small record, every field.
    pub fn near(&self, t: OTy, off: u64) -> Near<'_> {
        let fs = self.fields(t);
        if fs.len() < INDEX_MIN {
            return Near::All(fs.iter());
        }
        let Some(&s) = self.idx_start.get(&t) else { return Near::All(fs.iter()) };
        let rows = s as usize..s as usize + fs.len();
        let (order, reach) = (&self.idx_order[rows.clone()], &self.idx_reach[rows]);
        let mut j = order.partition_point(|&i| fs[i as usize].offset <= off);
        let (mut idx, mut n) = ([0u32; NEAR_MAX], 0usize);
        let mut walked = 0;
        while j > 0 && reach[j - 1] > off {
            j -= 1;
            walked += 1;
            let f = &fs[order[j] as usize];
            if f.offset + self.size(f.ty).max(1) > off {
                if n == NEAR_MAX {
                    return Near::All(fs.iter());
                }
                idx[n] = order[j];
                n += 1;
            }
            if walked == NEAR_WALK && j > 0 && reach[j - 1] > off {
                return Near::All(fs.iter());
            }
        }
        idx[..n].sort_unstable();
        Near::Few { fs, idx, n: n as u8, i: 0 }
    }

    /// The index in [`Types::fields`] of record `t` of the first field
    /// whose declaration is `decl` (a non-static data member): O(1) in a
    /// record with an offset index.
    pub fn field_index(&self, t: OTy, decl: DeclId) -> Option<usize> {
        let fs = self.fields(t);
        if fs.len() >= INDEX_MIN
            && let Some(&i) = self.member_idx.get(&decl)
            && fs.get(i as usize).is_some_and(|f| f.decl == decl)
        {
            return Some(i as usize);
        }
        fs.iter().position(|f| f.decl == decl)
    }

    /// Builds the offset index of record `t` ([`Types::near`]) if it has
    /// enough fields.
    fn index_fields(&mut self, t: OTy) {
        let fs = self.fields(t);
        if fs.len() < INDEX_MIN {
            return;
        }
        let mut order: Vec<u32> = (0..fs.len() as u32).collect();
        order.sort_by_key(|&i| (fs[i as usize].offset, i));
        let mut top = 0;
        let reach: Vec<u64> = order
            .iter()
            .map(|&i| {
                let f = &fs[i as usize];
                top = top.max(f.offset + self.size(f.ty).max(1));
                top
            })
            .collect();
        let members: Vec<(DeclId, u32)> = fs
            .iter()
            .enumerate()
            .filter(|(_, f)| !f.base)
            .map(|(i, f)| (f.decl, i as u32))
            .collect();
        self.member_idx.extend(members);
        let start = self.idx_order.len() as u32;
        self.idx_order.extend(order);
        self.idx_reach.extend(reach);
        let _ = self.idx_start.insert(t, start);
    }

    /// Bytes of heap the type table holds.
    pub fn footprint(&self) -> usize {
        fn cap<T>(v: &Vec<T>) -> usize {
            v.capacity() * size_of::<T>()
        }
        fn map<K, V>(m: &FxHashMap<K, V>) -> usize {
            m.capacity() * (size_of::<(K, V)>() + 1)
        }
        cap(&self.infos)
            + cap(&self.fields)
            + cap(&self.union_pos)
            + map(&self.map)
            + map(&self.arrays)
            + map(&self.const_arrays)
            + map(&self.data_sizes)
            + map(&self.empty_bases)
            + self.empty_bases.values().map(cap).sum::<usize>()
            + map(&self.provisional)
            + self.provisional_slots.capacity() * (size_of::<OTy>() + 1)
            + map(&self.nv_views)
            + map(&self.nv_class)
            + map(&self.idx_start)
            + cap(&self.idx_order)
            + cap(&self.idx_reach)
            + map(&self.member_idx)
            + self.paths.footprint()
    }

    /// The class type `t` is a view of: the complete class of a
    /// base-subobject view ([`Types::nv_view`]), else `t`.
    pub fn class_of(&self, t: OTy) -> OTy {
        if self.nv_class.is_empty() {
            return t;
        }
        self.nv_class.get(&t).copied().unwrap_or(t)
    }

    /// The base-subobject view of class `t`: its non-virtual part, the
    /// type of a base field naming it (`t` itself for a class without
    /// virtual bases).
    pub fn nv_view(&self, t: OTy) -> OTy {
        self.nv_views.get(&t).copied().unwrap_or(t)
    }

    /// Whether `a` and `b` are the same class (or the same type), either
    /// possibly a base-subobject view.
    pub fn same_class(&self, a: OTy, b: OTy) -> bool {
        a == b || self.class_of(a) == self.class_of(b)
    }

    /// The index in [`Types::fields`] of the first virtual base field of
    /// the complete class `t` (its field count if it has none): its
    /// non-virtual bases and members come first.
    pub fn vbase_start(&self, t: OTy) -> usize {
        match self.nv_views.get(&t) {
            Some(&v) => self.fields(v).len(),
            None => self.fields(t).len(),
        }
    }

    /// A class holding no data: no vptr, and every base and member is
    /// itself such a class (an empty class, [class.prop]/1, or one of
    /// only empty subobjects). Its storage may overlap another
    /// subobject's (an empty base at offset 0, `[[no_unique_address]]`),
    /// so zero-initializing it writes no byte ([dcl.init.general]/6.2).
    pub fn holds_no_data(&self, t: OTy) -> bool {
        let i = self.info(t);
        // A pointer or union subobject holds data: the common class
        // answers without the walk over its members (WP-760).
        matches!(i.kind, TyKind::Record { union: false, .. })
            && !(i.has_vptr || i.has_ptr || i.has_union)
            && self.fields(t).iter().all(|f| f.width.is_none() && self.holds_no_data(f.ty))
    }

    /// The non-bit-field member (or base) of the record `t` that holds
    /// byte `off`. A member of non-zero size wins over a zero-size one at
    /// the same offset (a zero-length array, GNU `T x[0]`, holds no byte:
    /// libc++'s `char __padding_[0]` before `__data_` in `basic_string`),
    /// which is found only when nothing else is there. Of two members
    /// holding it, the one at the greater offset wins: it was placed in
    /// the other's tail padding (a `[[no_unique_address]]` member or a
    /// non-POD base, Itanium C++ ABI 2.4 dsize), where the other holds
    /// no object.
    pub fn field_at(&self, t: OTy, off: u64) -> Option<FieldInfo> {
        let sized = |f: &&FieldInfo| {
            f.width.is_none() && off >= f.offset && off < f.offset + self.size(f.ty)
        };
        let union = matches!(self.kind(t), TyKind::Record { union: true, .. });
        let mut near = self.near(t, off);
        let found = near.find(sized).map(|first| {
            if union {
                return first;
            }
            near.fold(first, |best, f| if f.offset > best.offset && sized(&f) { f } else { best })
        });
        found
            .or_else(|| {
                self.near(t, off)
                    .find(|f| f.width.is_none() && self.size(f.ty) == 0 && off == f.offset)
            })
            .copied()
    }

    /// The member of the non-union record `t` on the path to an object of
    /// type `p` at byte `off`: [`Types::field_at`]'s member when the object
    /// is a subobject of it, else the first member that holds it, else
    /// `field_at`'s. Two members may share an offset when one holds no
    /// data there ([intro.object]/9: a `[[no_unique_address]]` member whose
    /// data size is zero, such as a union of empty members, with the next
    /// member placed at its offset; libc++'s `expected<void, E>` flag), so
    /// the walk follows the object, not the byte (WP-334).
    pub fn member_toward(&self, t: OTy, off: u64, p: OTy) -> Option<FieldInfo> {
        let at = self.field_at(t, off);
        let reach = |f: &FieldInfo| {
            f.width.is_none()
                && off >= f.offset
                && off < f.offset + self.size(f.ty).max(1)
                && self.subobject_in(f.ty, off - f.offset, p)
        };
        if at.as_ref().is_some_and(reach) {
            return at;
        }
        self.near(t, off).find(|f| reach(f)).copied().or(at)
    }

    /// How many provisional slots were rebuilt in place so far
    /// ([`Types::refresh`]).
    pub(crate) fn rebuilt(&self) -> u32 {
        self.rebuilt
    }

    /// The bytes spanning the union subobjects `union_pos[a..b]`
    /// ([`TyInfo::uhull`]), clamped to `u32` offsets.
    fn union_hull(&self, a: u32, b: u32) -> (u32, u32) {
        let us = &self.union_pos[a as usize..b as usize];
        let lo = us.iter().map(|&(o, _)| o).min();
        let hi = us.iter().map(|&(o, u)| o.saturating_add(self.size(u).max(1))).max();
        match (lo, hi) {
            (Some(lo), Some(hi)) => {
                let c = |v: u64| u32::try_from(v).unwrap_or(u32::MAX);
                (c(lo), c(hi))
            }
            _ => (0, 0),
        }
    }

    pub fn unions(&self, t: OTy) -> &[(u64, OTy)] {
        let (a, b) = self.info(t).upos;
        &self.union_pos[a as usize..b as usize]
    }

    /// The scalar class of `t`, if it is a scalar.
    pub fn scal(&self, t: OTy) -> Option<Scal> {
        Some(match self.kind(t) {
            TyKind::Bool => Scal::Bool,
            TyKind::Int { width, signed } => Scal::Int { width, signed },
            TyKind::Float(k) => Scal::Float(k),
            TyKind::Ptr(_) | TyKind::NullPtr => Scal::Ptr,
            _ => return None,
        })
    }

    /// Whether objects of `t` live in memory only (an array, a class, a
    /// bool vector or a `WideInt`): their prvalues initialize a result
    /// object.
    pub fn is_aggregate(&self, t: OTy) -> bool {
        matches!(
            self.kind(t),
            TyKind::Array { .. }
                | TyKind::Record { .. }
                | TyKind::BoolVec { .. }
                | TyKind::WideInt { .. }
        )
    }

    /// `(width, signed)` of a `WideInt` type.
    pub fn wide(&self, t: OTy) -> Option<(u32, bool)> {
        match self.kind(t) {
            TyKind::WideInt { width, signed } => Some((width, signed)),
            _ => None,
        }
    }

    /// Whether `t` is a GNU/clang vector type (stored as an array of its
    /// lanes, WP-173, or as packed bits, `TyKind::BoolVec`).
    pub fn is_vector(&self, ast: &Ast, t: OTy) -> bool {
        if let TyKind::BoolVec { .. } = self.kind(t) {
            return true;
        }
        matches!(self.kind(t), TyKind::Array { .. })
            && !self.is_synthetic_array(t)
            && matches!(ast.types.canonical_view(self.info(t).q), TypeView::Vector { .. })
    }

    /// Whether `t` is a GNU `_Complex` type (stored as `T[2]`).
    pub fn is_complex(&self, ast: &Ast, t: OTy) -> bool {
        matches!(self.kind(t), TyKind::Array { n: 2, .. })
            && matches!(ast.types.canonical_view(self.info(t).q), TypeView::Complex(_))
    }

    /// Interns `q` (any qualifiers and sugar are dropped).
    pub fn intern(&mut self, bridge: &mut dyn SemaBridge, q: QualType) -> OTy {
        let c = bridge.ast().types.canonical(q).without_quals();
        if let Some(&t) = self.map.get(&c) {
            if !self.provisional_slots.is_empty() && self.provisional_slots.contains(&t) {
                self.upgrade(bridge, t);
            }
            return t;
        }
        let t = self.build(bridge, c);
        self.map.insert(c, t);
        t
    }

    /// Whether `a` and `b` are the same type but for the qualifiers of
    /// array elements (`const char[2]` and `char[2]` intern apart; every
    /// other type is interned unqualified).
    pub fn same_unqualified(&self, mut a: OTy, mut b: OTy) -> bool {
        loop {
            if self.same_class(a, b) {
                return true;
            }
            match (self.kind(a), self.kind(b)) {
                (TyKind::Array { elem: x, n }, TyKind::Array { elem: y, n: m }) if n == m => {
                    (a, b) = (x, y);
                }
                _ => return false,
            }
        }
    }

    /// [`Types::same_unqualified`] or [`Types::similar`], level by level:
    /// whether an array of `a` holds objects that a pointer to `b` may
    /// step over ([expr.add]/6 allows similar types: a `const int *const
    /// (&)[2]` bound directly to an `int *[2]`, CWG2352).
    pub fn alike(&self, mut a: OTy, mut b: OTy) -> bool {
        loop {
            if self.same_class(a, b) {
                return true;
            }
            match (self.kind(a), self.kind(b)) {
                (TyKind::Ptr(x), TyKind::Ptr(y)) => (a, b) = (x, y),
                (TyKind::Array { elem: x, n }, TyKind::Array { elem: y, n: m }) if n == m => {
                    (a, b) = (x, y);
                }
                _ => return false,
            }
        }
    }

    /// [conv.qual]/2: whether `a` and `b` are similar types (alike but
    /// for the cv-qualifiers at each level; [`Types::intern`] drops the
    /// top-level ones).
    pub fn similar(&self, mut a: OTy, mut b: OTy) -> bool {
        loop {
            if a == b {
                return true;
            }
            match (self.kind(a), self.kind(b)) {
                (TyKind::Ptr(x), TyKind::Ptr(y)) => (a, b) = (x, y),
                (TyKind::Array { elem: x, n }, TyKind::Array { elem: y, n: m }) if n == m => {
                    (a, b) = (x, y);
                }
                _ => return false,
            }
        }
    }

    /// A synthetic array of `n` elements of `elem` (string literal objects
    /// thawed from the store, array `new`); its `q` is the element's, so
    /// notes name it with [`Types::array_name`].
    pub fn array_of(&mut self, elem: OTy, n: u64) -> OTy {
        if let Some(&t) = self.arrays.get(&(elem, n)) {
            return t;
        }
        let e = *self.info(elem);
        let start = self.union_pos.len() as u32;
        if e.has_union {
            for i in 0..n {
                for k in e.upos.0..e.upos.1 {
                    let (o, u) = self.union_pos[k as usize];
                    self.union_pos.push((i * e.size + o, u));
                }
            }
        }
        let end = self.union_pos.len() as u32;
        let t = self.push(TyInfo {
            q: e.q,
            size: e.size.saturating_mul(n),
            align: e.align,
            kind: TyKind::Array { elem, n },
            has_union: e.has_union,
            has_ptr: e.has_ptr || matches!(e.kind, TyKind::Ptr(_) | TyKind::NullPtr),
            has_vptr: e.has_vptr,
            has_mutable: e.has_mutable,
            upos: (start, end),
            uhull: self.union_hull(start, end),
        });
        self.arrays.insert((elem, n), t);
        t
    }

    /// Whether `t` is a synthetic array (made by [`Types::array_of`]), whose
    /// `q` names its element type.
    pub fn is_synthetic_array(&self, t: OTy) -> bool {
        match self.kind(t) {
            TyKind::Array { elem, n } => self.arrays.get(&(elem, n)) == Some(&t),
            _ => false,
        }
    }

    /// A scalar type of `size` bytes for unit tests.
    #[cfg(test)]
    pub fn test_scalar(&mut self, size: u64) -> OTy {
        self.simple(QualType::INT, size, 1, TyKind::Int { width: 8, signed: false })
    }

    fn push(&mut self, info: TyInfo) -> OTy {
        let t = OTy(self.infos.len() as u32);
        self.infos.push(info);
        t
    }

    fn simple(&mut self, q: QualType, size: u64, align: u32, kind: TyKind) -> OTy {
        self.push(TyInfo {
            q,
            size,
            align,
            kind,
            has_union: false,
            has_ptr: false,
            has_vptr: false,
            has_mutable: false,
            upos: (0, 0),
            uhull: (0, 0),
        })
    }

    fn build(&mut self, bridge: &mut dyn SemaBridge, c: QualType) -> OTy {
        let ast = bridge.ast();
        let target = ast.types.target();
        let view = ast.types.canonical_view(c);
        let pbytes = u64::from(target.pointer_bytes.max(8));
        match view {
            TypeView::Builtin(b) => {
                let (size, align) = target.builtin_size_align(b);
                let (size, align) = (u64::from(size), u32::from(align));
                let kind = match b {
                    BuiltinType::Void => TyKind::Void,
                    BuiltinType::Bool => TyKind::Bool,
                    BuiltinType::NullPtr => TyKind::NullPtr,
                    BuiltinType::MetaInfo => TyKind::Reflection,
                    BuiltinType::Char => TyKind::Int { width: 8, signed: target.char_signed },
                    BuiltinType::WChar => {
                        TyKind::Int { width: (size * 8) as u8, signed: target.wchar_signed }
                    }
                    BuiltinType::SChar
                    | BuiltinType::Short
                    | BuiltinType::Int
                    | BuiltinType::Long
                    | BuiltinType::LongLong
                    | BuiltinType::Int128 => TyKind::Int { width: (size * 8) as u8, signed: true },
                    BuiltinType::UChar
                    | BuiltinType::Char8
                    | BuiltinType::Char16
                    | BuiltinType::Char32
                    | BuiltinType::UShort
                    | BuiltinType::UInt
                    | BuiltinType::ULong
                    | BuiltinType::ULongLong
                    | BuiltinType::UInt128 => {
                        TyKind::Int { width: (size * 8) as u8, signed: false }
                    }
                    _ => match float_kind(b, target.long_double) {
                        Some(k) => TyKind::Float(k),
                        None => TyKind::Opaque,
                    },
                };
                self.simple(c, size, align.max(1), kind)
            }
            // `_BitInt(N)` (spec/oven.md "`_BitInt`"): N <= 128 is an
            // ordinary integer of width N in its ABI-sized slot (the KBC
            // integer opcodes wrap to the width in `aux`); a wider one is a
            // memory-resident `WideInt` (`crate::bigint`).
            TypeView::BitInt { signed, width } => {
                let (size, align) = bridge.opaque_size_align(c);
                let kind = match u8::try_from(width) {
                    Ok(w) if w <= 128 => TyKind::Int { width: w, signed },
                    _ if size >= u64::from(width).div_ceil(64) * 8 => {
                        TyKind::WideInt { width, signed }
                    }
                    _ => TyKind::Opaque,
                };
                self.simple(c, size, align.max(1), kind)
            }
            // A reference is stored as the address of its referent (only
            // a declaration has reference type: a variable, parameter,
            // field or return type; expressions never do).
            TypeView::Pointer(p) | TypeView::LValueRef(p) | TypeView::RValueRef(p) => {
                let pointee = self.intern(bridge, p);
                self.simple(c, pbytes, pbytes as u32, TyKind::Ptr(pointee))
            }
            TypeView::Enum(d) => {
                let def = ast.definition(d).unwrap_or(d);
                let int =
                    ast.decls.enumeration(def).and_then(|e| e.int_type).unwrap_or(QualType::INT);
                let it = self.intern(bridge, int);
                let info = *self.info(it);
                self.simple(c, info.size, info.align, info.kind)
            }
            TypeView::ConstantArray(a) => {
                let elem = self.intern(bridge, a.elem);
                if let Some(&t) = self.const_arrays.get(&(elem, a.size)) {
                    return t;
                }
                let e = *self.info(elem);
                let start = self.union_pos.len() as u32;
                if e.has_union {
                    for i in 0..a.size {
                        for k in e.upos.0..e.upos.1 {
                            let (o, u) = self.union_pos[k as usize];
                            self.union_pos.push((i * e.size + o, u));
                        }
                    }
                }
                let end = self.union_pos.len() as u32;
                let t = self.push(TyInfo {
                    q: c,
                    size: e.size.saturating_mul(a.size),
                    align: e.align,
                    kind: TyKind::Array { elem, n: a.size },
                    has_union: e.has_union,
                    has_ptr: e.has_ptr || matches!(e.kind, TyKind::Ptr(_) | TyKind::NullPtr),
                    has_vptr: e.has_vptr,
                    has_mutable: e.has_mutable,
                    upos: (start, end),
                    uhull: self.union_hull(start, end),
                });
                let _ = self.const_arrays.insert((elem, a.size), t);
                t
            }
            TypeView::Function(_) => self.simple(c, 1, 1, TyKind::Func),
            TypeView::Record(d) => self.record(bridge, c, d),
            // A member pointer has the Itanium size (one pointer-sized
            // offset for data, pointer + adjustment for functions) so
            // `sizeof`/`alignof` and record layout agree with kiln_abi;
            // its value is an interned index ([`crate::memptr`]).
            TypeView::MemberPointer { pointee, .. } => {
                let func = ast.types.is_function(pointee);
                let n = if func { 2 } else { 1 };
                self.simple(c, pbytes * n, pbytes as u32, TyKind::MemPtr { func })
            }
            // `_Atomic(T)` holds a `T` (constant evaluation is
            // single-threaded): the value's representation, with clang's
            // size promotion (`kiln_abi`'s: a power of two up to 16 bytes,
            // aligned to its size; both targets' MaxAtomicPromoteWidth is
            // 128).
            TypeView::Atomic(v) => {
                let inner = self.intern(bridge, v);
                let mut info = *self.info(inner);
                info.q = c;
                if info.size == 0 {
                    info.size = 1;
                } else if info.size <= 16 {
                    info.size = info.size.next_power_of_two();
                    info.align = info.size as u32;
                }
                self.push(info)
            }
            // A GNU/clang vector (WP-173) is stored as an array of its
            // elements, padded to the vector's own size and alignment (an
            // `ext_vector_type(3)` occupies 4 lanes); `q` keeps the vector
            // type, so notes name it. A vector of `bool` packs its lanes
            // into bits (`TyKind::BoolVec`).
            TypeView::Vector { elem, count, .. } => {
                let (size, align) = bridge.opaque_size_align(c);
                let elem = self.intern(bridge, elem);
                let e = *self.info(elem);
                let n = u64::from(count);
                if e.kind == TyKind::Bool && n.div_ceil(8) <= size {
                    return self.simple(c, size, align.max(1), TyKind::BoolVec { elem, n });
                }
                if !matches!(e.kind, TyKind::Int { .. } | TyKind::Float(_))
                    || e.size.saturating_mul(n) > size
                {
                    return self.simple(c, size, align.max(1), TyKind::Opaque);
                }
                self.simple(c, size, align.max(1), TyKind::Array { elem, n })
            }
            // GNU `_Complex T` is stored as `T[2]` (real, imaginary;
            // clang's `{T, T}`); `q` keeps the complex type.
            TypeView::Complex(elem) => {
                let elem = self.intern(bridge, elem);
                let e = *self.info(elem);
                if !matches!(e.kind, TyKind::Int { .. } | TyKind::Float(_)) {
                    return self.simple(c, 0, 1, TyKind::Opaque);
                }
                self.simple(c, e.size * 2, e.align, TyKind::Array { elem, n: 2 })
            }
            _ => self.simple(c, 0, 1, TyKind::Opaque),
        }
    }

    /// Rebuilds every provisional class slot whose class completed since
    /// the last refresh; called before each evaluation and after each
    /// `Mutating` intrinsic (a class completes between evaluations, inside
    /// one through a lent Oven, or by `define_aggregate`). O(classes
    /// completed since), not O(provisional slots): the slots are keyed by
    /// class and the AST's completion log is read from a cursor.
    pub fn refresh(&mut self, bridge: &mut dyn SemaBridge) {
        let done = bridge.ast().decls.completed_records().len();
        let mut i = self.completion_epoch;
        self.completion_epoch = done;
        if self.provisional.is_empty() {
            return;
        }
        while i < done {
            let ast = bridge.ast();
            let r = ast.decls.completed_records()[i];
            if let Some(&t) = self.provisional.get(&ast.first(r)) {
                self.upgrade(bridge, t);
            }
            i += 1;
        }
    }

    /// Rebuilds provisional slot `t` in place if its class is complete
    /// now; whether it did (it is then no longer provisional).
    fn upgrade(&mut self, bridge: &mut dyn SemaBridge, t: OTy) -> bool {
        let c = self.info(t).q;
        let ast = bridge.ast();
        let Some(d) = ast.types.as_record(c) else { return false };
        if Self::incomplete(ast, d) {
            return false;
        }
        let first = ast.first(d);
        if self.provisional.get(&first) == Some(&t) {
            self.provisional.remove(&first);
        }
        self.provisional_slots.remove(&t);
        self.record_into(bridge, c, d, Some(t));
        true
    }

    /// Whether class `d` has no definition or is still being defined
    /// (`kiln_abi::ensure_layout` refuses it).
    pub(crate) fn incomplete(ast: &Ast, d: DeclId) -> bool {
        ast.definition(d).is_none_or(|def| {
            ast.decls
                .record(def)
                .is_none_or(|r| !r.defn.contains(kiln_ast::decl::DefinitionBits::COMPLETE))
        })
    }

    /// An `Opaque` slot for incomplete class type `c` (`slot` when one
    /// is reserved), kept provisional until the class completes.
    /// `first` is the class's first declaration, the slot's key.
    fn provisional_slot(&mut self, c: QualType, first: DeclId, slot: Option<OTy>) -> OTy {
        let t = slot.unwrap_or_else(|| self.simple(c, 0, 1, TyKind::Opaque));
        self.provisional.insert(first, t);
        self.provisional_slots.insert(t);
        t
    }

    fn record(&mut self, bridge: &mut dyn SemaBridge, c: QualType, d: DeclId) -> OTy {
        self.record_into(bridge, c, d, None)
    }

    /// Builds class type `c` (declaration `d`), into the reserved slot
    /// `slot` when one is given (an upgraded provisional slot).
    fn record_into(
        &mut self,
        bridge: &mut dyn SemaBridge,
        c: QualType,
        d: DeclId,
        slot: Option<OTy>,
    ) -> OTy {
        let ast = bridge.ast();
        if Self::incomplete(ast, d) {
            return self.provisional_slot(c, ast.first(d), slot);
        }
        let Some(def) = ast.definition(d) else {
            return self.simple(c, 0, 1, TyKind::Opaque);
        };
        let union = ast.decls.record(def).is_some_and(|r| r.tag == kiln_ast::decl::TagKind::Union);
        let poly = ast
            .decls
            .record(def)
            .is_some_and(|r| r.defn.contains(kiln_ast::decl::DefinitionBits::POLYMORPHIC));
        // Direct non-virtual bases as written; empty bases have no storage
        // the evaluator touches (they would shadow the member at offset 0)
        // and are kept apart (`empty_bases`).
        let mut ebases: Vec<(QualType, DeclId)> = Vec::new();
        let bdecls: Vec<(DeclId, QualType, DeclId)> = ast
            .decls
            .bases(def, &ast.lists)
            .filter(|b| !b.is_virtual)
            .filter_map(|b| {
                let rd = ast.types.as_record(b.ty)?;
                let rdef = ast.definition(rd)?;
                let empty = ast
                    .decls
                    .record(rdef)
                    .is_some_and(|r| r.defn.contains(kiln_ast::decl::DefinitionBits::EMPTY));
                if empty {
                    ebases.push((b.ty, ast.first(rd)));
                }
                (!empty).then_some((rdef, b.ty, ast.first(rd)))
            })
            .collect();
        let fdecls: Vec<(DeclId, QualType, Option<u32>, bool)> = ast
            .members(def)
            .filter(|&m| ast.decls.get(m).kind() == DeclKind::Field)
            .map(|m| {
                let w = ast
                    .decls
                    .field(m)
                    .and_then(|f| f.bit_width)
                    .map(|e| eval_int(ast, e).map_or(0, |v| v.min(1 << 20) as u32));
                let mutable = ast.decls.field(m).is_some_and(kiln_ast::decl::FieldData::is_mutable);
                (m, ast.decls.get(m).ty().unwrap_or(QualType::INT), w, mutable)
            })
            .collect();
        let first = ast.first(d);
        let Some(layout) = bridge.ensure_layout(def) else {
            // Unreachable: `incomplete` is `ensure_layout`'s rule.
            return self.provisional_slot(c, first, slot);
        };
        let (size, align) = (layout.size, layout.align);
        let dsize = layout.data_size;
        let offs: Vec<u64> = layout.fields.to_vec();
        let boffs: Vec<u64> = bdecls
            .iter()
            .map(|&(_, _, first)| {
                layout.bases.iter().find(|&&(b, _)| b == first).map_or(0, |&(_, o)| o)
            })
            .collect();
        let eoffs: Vec<(QualType, u64)> = ebases
            .iter()
            .map(|&(bq, first)| {
                (bq, layout.bases.iter().find(|&&(b, _)| b == first).map_or(0, |&(_, o)| o))
            })
            .collect();
        let (nv_size, nv_align) = (layout.nv_size, layout.nv_align);
        let vlist: Vec<(DeclId, u64)> = layout.vbases.to_vec();
        // Every virtual base (direct or indirect) at its offset in the
        // complete object, as a base field after the members (P3533R2;
        // empty ones, like empty non-virtual bases, are not laid out).
        let ast = bridge.ast();
        let vdecls: Vec<(DeclId, QualType, u64)> = vlist
            .iter()
            .filter_map(|&(v, off)| {
                let vdef = ast.definition(v)?;
                let r = ast.decls.record(vdef)?;
                if r.defn.contains(kiln_ast::decl::DefinitionBits::EMPTY) {
                    return None;
                }
                Some((vdef, ast.decls.get(vdef).ty()?, off))
            })
            .collect();
        // Reserve the slot first so self-referential pointers terminate.
        let me = match slot {
            Some(t) => {
                // The slot's layout changes from `Opaque`: walks cached
                // through it are stale.
                self.paths.clear();
                self.rebuilt = self.rebuilt.wrapping_add(1);
                let i = &mut self.infos[t.0 as usize];
                (i.size, i.align) = (size, align.max(1));
                t
            }
            None => self.simple(c, size, align.max(1), TyKind::Opaque),
        };
        self.map.insert(c, me);
        if dsize < size && !union {
            let _ = self.data_sizes.insert(me, dsize);
        }
        if !eoffs.is_empty() {
            let eb = eoffs.iter().map(|&(bq, off)| (self.intern(bridge, bq), off)).collect();
            let _ = self.empty_bases.insert(me, eb);
        }
        let mut fields = Vec::with_capacity(fdecls.len());
        let (mut has_union, mut has_ptr) = (union, false);
        let mut has_vptr = poly;
        let mut has_mutable = false;
        let mut upos = Vec::new();
        if union {
            upos.push((0, me));
        }
        for (&(rd, bq, _), &off) in bdecls.iter().zip(&boffs) {
            let bty = self.intern(bridge, bq);
            let bty = self.nv_view(bty);
            let bi = *self.info(bty);
            has_union |= bi.has_union;
            has_ptr |= bi.has_ptr;
            has_vptr |= bi.has_vptr;
            has_mutable |= bi.has_mutable;
            for k in bi.upos.0..bi.upos.1 {
                let (o, u) = self.union_pos[k as usize];
                upos.push((off + o, u));
            }
            fields.push(FieldInfo {
                decl: rd,
                ty: bty,
                offset: off,
                bit: 0,
                width: None,
                base: true,
                mutable: false,
            });
        }
        for (i, &(m, fq, w, mutable)) in fdecls.iter().enumerate() {
            let fty = self.intern(bridge, fq);
            let bits = offs.get(i).copied().unwrap_or(0);
            let fi = *self.info(fty);
            has_union |= fi.has_union;
            has_vptr |= fi.has_vptr;
            has_mutable |= mutable || fi.has_mutable;
            has_ptr |= fi.has_ptr || matches!(fi.kind, TyKind::Ptr(_) | TyKind::NullPtr);
            if fi.has_union {
                for k in fi.upos.0..fi.upos.1 {
                    let (o, u) = self.union_pos[k as usize];
                    upos.push((bits / 8 + o, u));
                }
            }
            match w {
                Some(w) => fields.push(FieldInfo {
                    decl: m,
                    ty: fty,
                    offset: bits / 8,
                    bit: (bits % 8) as u32,
                    width: Some(w),
                    base: false,
                    mutable,
                }),
                None => fields.push(FieldInfo {
                    decl: m,
                    ty: fty,
                    offset: bits / 8,
                    bit: 0,
                    width: None,
                    base: false,
                    mutable,
                }),
            }
        }
        if !vdecls.is_empty() && !union {
            // Interned first: interning a class appends its own fields.
            let vtys: Vec<OTy> = vdecls
                .iter()
                .map(|&(_, vq, _)| {
                    let t = self.intern(bridge, vq);
                    self.nv_view(t)
                })
                .collect();
            // The base-subobject view: the same fields up to the virtual
            // bases (a prefix of the complete class's field list).
            let mut nv_upos = upos.clone();
            nv_upos.sort_by_key(|&(o, _)| o);
            let ustart = self.union_pos.len() as u32;
            self.union_pos.extend(nv_upos);
            let uend = self.union_pos.len() as u32;
            let first = self.fields.len() as u32;
            let view = self.push(TyInfo {
                q: c,
                size: nv_size,
                align: nv_align.max(1),
                kind: TyKind::Record { decl: def, union, first, count: fields.len() as u32 },
                has_union,
                has_ptr,
                has_vptr,
                has_mutable,
                upos: (ustart, uend),
                uhull: self.union_hull(ustart, uend),
            });
            self.nv_views.insert(me, view);
            // The view holds the same non-virtual bases, empty ones included.
            if let Some(eb) = self.empty_bases.get(&me).cloned() {
                let _ = self.empty_bases.insert(view, eb);
            }
            self.nv_class.insert(view, me);
            for (&(vd, _, off), &vty) in vdecls.iter().zip(&vtys) {
                let vi = *self.info(vty);
                has_union |= vi.has_union;
                has_ptr |= vi.has_ptr;
                has_vptr |= vi.has_vptr;
                has_mutable |= vi.has_mutable;
                for k in vi.upos.0..vi.upos.1 {
                    let (o, u) = self.union_pos[k as usize];
                    upos.push((off + o, u));
                }
                fields.push(FieldInfo {
                    decl: vd,
                    ty: vty,
                    offset: off,
                    bit: 0,
                    width: None,
                    base: true,
                    mutable: false,
                });
            }
        }
        let first = self.fields.len() as u32;
        let count = fields.len() as u32;
        self.fields.extend(fields);
        upos.sort_by_key(|&(o, _)| o);
        let ustart = self.union_pos.len() as u32;
        self.union_pos.extend(upos);
        let uend = self.union_pos.len() as u32;
        let uhull = self.union_hull(ustart, uend);
        let info = &mut self.infos[me.0 as usize];
        info.kind = TyKind::Record { decl: def, union, first, count };
        info.has_union = has_union;
        info.has_ptr = has_ptr;
        info.has_vptr = has_vptr;
        info.has_mutable = has_mutable;
        info.upos = (ustart, uend);
        info.uhull = uhull;
        self.index_fields(me);
        if let Some(&view) = self.nv_views.get(&me) {
            self.index_fields(view);
        }
        me
    }

    /// The array extent containing the object of type `p` that a pointer
    /// at byte `off` of an object of type `root` designates: `(start, n)`
    /// in units of `p`. With `past`, the pointer is one past the end of
    /// that array. The flag is false for an object that is not an array
    /// element (treated as an array of one, [expr.add]/4).
    pub fn extent(&self, root: OTy, off: u64, past: bool, p: OTy) -> Option<(u64, u64, bool)> {
        let o = if past { off.checked_sub(1)? } else { off };
        self.extent_front(root, o, p)
    }

    /// [`Types::extent`] of the object at byte `o` below byte `base`
    /// (of type `t`); with `cache`, the first record on the way is looked
    /// up in the path cache (its own walk below is not).
    fn extent_in(
        &self,
        mut t: OTy,
        mut base: u64,
        o: u64,
        p: OTy,
        cache: bool,
    ) -> Option<(u64, u64, bool)> {
        loop {
            match self.kind(t) {
                TyKind::Array { elem, n } => {
                    if self.alike(elem, p) {
                        return Some((base, n, true));
                    }
                    // The array object itself (`&arr`), not an element.
                    if self.alike(t, p) {
                        return Some((base, 1, false));
                    }
                    let es = self.size(elem);
                    if es == 0 || n == 0 {
                        return None;
                    }
                    let i = ((o - base) / es).min(n - 1);
                    base += i * es;
                    t = elem;
                }
                _ if self.alike(t, p) => return Some((base, 1, false)),
                // A union's members overlap, and so may a class's: an
                // empty `[[no_unique_address]]` member shares its offset
                // with the next one ([intro.object]/9), so each member
                // containing the byte is tried in turn. A zero-size
                // member (`char pad[0]`) only when no sized one holds
                // the object: it overlaps the next member's first byte.
                TyKind::Record { .. } if cache => {
                    return self.extent_cached(t, o - base, p).map(|(s, n, e)| (s + base, n, e));
                }
                TyKind::Record { union, .. } => {
                    let rel = o - base;
                    // In a union, an array member holding the object wins
                    // over a member that is the object itself (`union {
                    // T t; T a[N]; }`: `&a[0] + 1` is `&a[1]`, WP-743);
                    // the access through the result checks the active
                    // member.
                    let mut single = None;
                    for sized in [true, false] {
                        for f in self.near(t, rel) {
                            let sz = self.size(f.ty);
                            if f.width.is_some() || (sz > 0) != sized {
                                continue;
                            }
                            if rel >= f.offset
                                && rel < f.offset + sz.max(1)
                                && let Some(r) = self.extent_in(f.ty, base + f.offset, o, p, false)
                            {
                                if !union || r.2 {
                                    return Some(r);
                                }
                                single = single.or(Some(r));
                            }
                        }
                    }
                    return single;
                }
                _ => return None,
            }
        }
    }

    /// Whether a base class subobject of type `p` lives at byte `off` of
    /// an object of class type `root` (through base classes only).
    pub fn has_base_subobject(&self, root: OTy, off: u64, p: OTy) -> bool {
        if off == 0 && self.same_class(p, root) {
            return true;
        }
        matches!(self.kind(root), TyKind::Record { .. })
            && self.near(root, off).any(|f| {
                f.base
                    && off >= f.offset
                    && off < f.offset + self.size(f.ty).max(1)
                    && self.has_base_subobject(f.ty, off - f.offset, p)
            })
    }

    /// Whether an object of type `p` lives at byte `off` of an object of
    /// type `root` (C++26 [expr.static.cast]/14 for `void*` casts).
    /// The direct non-virtual empty bases (type, offset) of class `t`,
    /// which are not in [`Types::fields`].
    pub fn empty_bases(&self, t: OTy) -> &[(OTy, u64)] {
        self.empty_bases.get(&t).map_or(&[], Vec::as_slice)
    }

    pub fn has_subobject(&self, mut root: OTy, mut off: u64, p: OTy) -> bool {
        // The leading arrays by index arithmetic, then the first record
        // through the path cache (WP-644).
        loop {
            if off == 0 && self.same_class(p, root) {
                return true;
            }
            match self.kind(root) {
                TyKind::Array { elem, n } => {
                    let es = self.size(elem);
                    if es == 0 || off / es >= n {
                        return false;
                    }
                    off %= es;
                    root = elem;
                }
                TyKind::Record { .. } => {
                    // An element of an array of `p` the whole-array table
                    // holds (an `inplace_vector`'s `construct_at(&data[n])`,
                    // WP-760).
                    if let Some((lo, _)) = self.extent_hit(root, off, false, p)
                        && (off - lo) % self.size(p).max(1) == 0
                    {
                        return true;
                    }
                    return self.subobject_cached(root, off, p);
                }
                _ => return false,
            }
        }
    }

    /// [`Types::has_subobject`], uncached.
    pub(crate) fn subobject_in(&self, root: OTy, off: u64, p: OTy) -> bool {
        if off == 0 && self.same_class(p, root) {
            return true;
        }
        match self.kind(root) {
            TyKind::Array { elem, n } => {
                let es = self.size(elem);
                es != 0 && off / es < n && self.subobject_in(elem, off % es, p)
            }
            TyKind::Record { .. } => {
                self.near(root, off).any(|f| {
                    f.width.is_none()
                        && off >= f.offset
                        && off < f.offset + self.size(f.ty).max(1)
                        && self.subobject_in(f.ty, off - f.offset, p)
                }) || self
                    .empty_bases
                    .get(&root)
                    .is_some_and(|v| v.iter().any(|&(b, o)| o == off && self.subobject_in(b, 0, p)))
            }
            _ => false,
        }
    }

    /// Whether the storage unit of a bit-field of type `p` lies at `off`
    /// inside `root` (a bit-field is not a subobject that a pointer can
    /// designate, but its accesses still go through the unions above it).
    /// A lane of a `bool` ext vector is the same: one bit, reached through
    /// the vector (WP-438).
    pub fn has_bit_unit(&self, root: OTy, off: u64, p: OTy) -> bool {
        match self.kind(root) {
            TyKind::BoolVec { elem, .. } => elem == p && off < self.size(root),
            TyKind::Array { elem, n } => {
                let es = self.size(elem);
                es != 0 && off / es < n && self.has_bit_unit(elem, off % es, p)
            }
            TyKind::Record { .. } => self.near(root, off).any(|f| match f.width {
                Some(_) => off == f.offset && f.ty == p,
                None => {
                    off >= f.offset
                        && off < f.offset + self.size(f.ty).max(1)
                        && self.has_bit_unit(f.ty, off - f.offset, p)
                }
            }),
            _ => false,
        }
    }
}
