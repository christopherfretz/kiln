//! Type and record layout (spec/abi.md §3–§5): `sizeof`/`alignof` of
//! every M2 type and the Itanium C++ ABI §2.4 layout of classes without
//! bases, matching clang 19 bit for bit.
//!
//! All internal arithmetic is in bits and saturating; the API is total
//! (incomplete, dependent or invalid types get a degenerate layout
//! instead of a panic). Record dependencies are resolved with an
//! explicit stack, so the nesting depth of the input cannot overflow the
//! native stack.

mod base_info;
mod empty;
pub mod padding;
pub(crate) mod props;
mod record;
mod subobject;

use kiln_ast::decl::DeclFlags;
use kiln_ast::types::{BuiltinType, RecordLayout, TypeView, VectorKind};
use kiln_ast::{Ast, DeclId, QualType};
use kiln_source::fx::{FxHashMap, FxHashSet};

use crate::target::Target;
use props::max_align_attr;
pub use subobject::{MAX_SUBOBJECTS, subobjects};

/// Size and alignment of a type, in bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
pub struct TypeLayout {
    /// `sizeof` (0 for incomplete arrays, `void` and functions).
    pub size: u64,
    /// `alignof` (at least 1).
    pub align: u64,
}

/// Width and alignment in bits (clang `TypeInfo`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Info {
    pub width: u64,
    pub align: u64,
}

impl Info {
    const DEGENERATE: Info = Info { width: 0, align: 8 };
}

/// What a field contributes to the empty-subobject map.
#[derive(Clone, Copy, PartialEq, Eq, Debug, kiln_snap::Snap)]
pub(crate) enum FieldTarget {
    /// Not a class or array of class (or a bit-field).
    None,
    /// A class (first declaration).
    Record(DeclId),
    /// A constant array of `n` (flattened) elements of a class.
    Array(DeclId, u64),
}

/// Everything computed for one class.
#[derive(Clone, Debug, kiln_snap::Snap)]
#[allow(clippy::struct_excessive_bools, reason = "independent class facts")]
pub(crate) struct RecordFacts {
    pub layout: RecordLayout,
    pub is_pod: bool,
    pub is_empty: bool,
    pub packed: bool,
    /// clang `SizeOfLargestEmptySubobject`, bytes.
    pub largest_empty: u64,
    /// Per field, in declaration order.
    pub field_targets: Box<[FieldTarget]>,
    /// Dynamic: polymorphic or with a virtual base (clang
    /// `isDynamicClass`).
    pub dynamic: bool,
    /// The virtual primary bases of this class and of every base with
    /// virtual bases, transitively (clang `AddIndirectPrimaryBases`).
    pub vprimaries: Box<[DeclId]>,
}

impl RecordFacts {
    /// An invalid class `def`: size, data size and every alignment 1
    /// (clang `getASTRecordLayout`'s invalid-decl path). clang records
    /// no offsets; every field and direct base gets offset 0 here so a
    /// consumer indexing them stays total.
    fn invalid(ast: &Ast, def: DeclId) -> RecordFacts {
        let mut f = RecordFacts::degenerate();
        let n = props::fields(ast, def).len();
        f.layout.size = 1;
        f.layout.data_size = 1;
        f.layout.nv_size = 1;
        f.layout.fields = vec![0; n].into_boxed_slice();
        let (mut bases, mut vbases) = (Vec::new(), Vec::new());
        for b in ast.decls.bases(def, &ast.lists) {
            if let Some(r) = props::as_record(ast, b.ty) {
                if b.is_virtual { vbases.push((r, 0)) } else { bases.push((r, 0)) }
            }
        }
        f.layout.bases = bases.into_boxed_slice();
        f.layout.vbases = vbases.into_boxed_slice();
        f.field_targets = vec![FieldTarget::None; n].into_boxed_slice();
        f.is_pod = false;
        f
    }

    fn degenerate() -> RecordFacts {
        RecordFacts {
            layout: RecordLayout {
                align: 1,
                preferred_align: 1,
                nv_align: 1,
                unadjusted_align: 1,
                unadjusted_field_align: 1,
                ..RecordLayout::default()
            },
            is_pod: true,
            is_empty: false,
            packed: false,
            largest_empty: 0,
            field_targets: Box::new([]),
            dynamic: false,
            vprimaries: Box::new([]),
        }
    }
}

/// The class layouts of a finished [`Layouter`] session, detached from
/// its AST borrow so a caller can resume them in a later session
/// ([`Layouter::with_cache`]; spec/abi.md §1 "Memoized"). Valid
/// only while every class in it keeps its definition: the caller resumes
/// it for complete classes only (a complete class's layout, and that of
/// every class it contains by value, is final for the rest of the
/// translation unit).
#[derive(Debug, Default, kiln_snap::Snap)]
pub struct LayoutCache {
    records: FxHashMap<DeclId, RecordFacts>,
}

impl LayoutCache {
    /// The number of classes laid out.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether no class is laid out.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// A layout session over one AST and target: caches every class it lays
/// out (keyed by the class's first declaration).
#[derive(Debug)]
pub struct Layouter<'a> {
    ast: &'a Ast,
    target: &'a Target,
    records: FxHashMap<DeclId, RecordFacts>,
    in_progress: FxHashSet<DeclId>,
    /// The classes this session laid out, in order (for
    /// [`Layouter::into_complete_cache`]).
    fresh: Vec<DeclId>,
}

impl<'a> Layouter<'a> {
    /// A session with an empty cache.
    pub fn new(ast: &'a Ast, target: &'a Target) -> Self {
        Layouter {
            ast,
            target,
            records: FxHashMap::default(),
            in_progress: FxHashSet::default(),
            fresh: Vec::new(),
        }
    }

    /// A session resuming the layouts of an earlier one.
    pub fn with_cache(ast: &'a Ast, target: &'a Target, cache: LayoutCache) -> Self {
        Layouter {
            ast,
            target,
            records: cache.records,
            in_progress: FxHashSet::default(),
            fresh: Vec::new(),
        }
    }

    /// Ends the session, keeping its layouts for a later one.
    pub fn into_cache(self) -> LayoutCache {
        LayoutCache { records: self.records }
    }

    /// Ends the session, keeping its layouts of complete classes only
    /// ([basic.types.general]/5: the definition reached `}`) for a later
    /// one. A class laid out while incomplete (still being defined, or
    /// without a definition) is dropped: its layout may change, and a
    /// [`LayoutCache`] is never invalidated (spec/abi.md §1). The complete
    /// classes it contains are kept.
    pub fn into_complete_cache(self) -> LayoutCache {
        let ast = self.ast;
        let mut records = self.records;
        for d in self.fresh {
            if !is_complete_record(ast, d) {
                records.remove(&d);
            }
        }
        LayoutCache { records }
    }

    /// The AST.
    pub fn ast(&self) -> &'a Ast {
        self.ast
    }

    /// The target.
    pub fn target(&self) -> &'a Target {
        self.target
    }

    /// `sizeof` and `alignof` of `q` ([expr.sizeof], [expr.alignof]).
    pub fn type_layout(&mut self, q: QualType) -> TypeLayout {
        let i = self.type_info(q);
        TypeLayout { size: i.width / 8, align: (i.align / 8).max(1) }
    }

    /// The unadjusted (natural) alignment of `q` in bytes (clang
    /// `ASTContext::getTypeUnadjustedAlign`; spec/abi.md §5): for a class
    /// type (through any sugar, so an `aligned` typedef of it is ignored)
    /// its [`RecordLayout::unadjusted_align`], or with `bases` false
    /// (clang 19/20) its [`RecordLayout::unadjusted_field_align`];
    /// otherwise the alignment of the canonical type. AAPCS64 classifies
    /// composites by it.
    pub fn type_unadjusted_align(&mut self, q: QualType, bases: bool) -> u64 {
        let c = self.ast.types.canonical(q);
        if let TypeView::Record(r) = self.ast.types.canonical_view(c) {
            let l = self.record(r);
            let a = if bases { l.unadjusted_align } else { l.unadjusted_field_align };
            return u64::from(a).max(1);
        }
        self.type_layout(c).align
    }

    /// The layout of class `record` (any of its declarations). An
    /// incomplete class gets size 0, alignment 1.
    pub fn record(&mut self, record: DeclId) -> &RecordLayout {
        let r = self.ast.first(record);
        self.ensure(r);
        &self.records.get(&r).expect("ensure inserts").layout
    }

    /// Is `record` POD for the purpose of layout (its tail padding is
    /// never reused)?
    pub fn is_pod(&mut self, record: DeclId) -> bool {
        let r = self.ast.first(record);
        self.ensure(r);
        self.records.get(&r).is_some_and(|f| f.is_pod)
    }

    /// Is `record` an empty class (clang `isEmpty`)?
    pub fn is_empty(&mut self, record: DeclId) -> bool {
        let r = self.ast.first(record);
        self.ensure(r);
        self.records.get(&r).is_some_and(|f| f.is_empty)
    }

    /// Every layout computed so far, keyed by first declaration.
    pub fn layouts(&self) -> impl Iterator<Item = (DeclId, &RecordLayout)> + '_ {
        self.records.iter().map(|(&d, f)| (d, &f.layout))
    }

    /// Consumes the session, yielding every computed layout.
    pub fn into_layouts(self) -> impl Iterator<Item = (DeclId, RecordLayout)> {
        self.records.into_iter().map(|(d, f)| (d, f.layout))
    }

    pub(crate) fn facts(&self, first: DeclId) -> Option<&RecordFacts> {
        self.records.get(&first)
    }

    pub(crate) fn record_size(&self, first: DeclId) -> u64 {
        self.records.get(&first).map_or(0, |f| f.layout.size)
    }

    /// Lays out `first` and every class it contains by value, children
    /// first (explicit post-order; a containment cycle, which only an
    /// invalid program has, sees the degenerate layout).
    fn ensure(&mut self, first: DeclId) {
        if self.records.contains_key(&first) || self.in_progress.contains(&first) {
            return;
        }
        let mut stack = vec![(first, false)];
        while let Some((d, expanded)) = stack.pop() {
            if self.records.contains_key(&d) {
                continue;
            }
            if expanded {
                let facts = self.compute(d);
                self.in_progress.remove(&d);
                self.records.insert(d, facts);
                self.fresh.push(d);
                continue;
            }
            if !self.in_progress.insert(d) {
                continue;
            }
            stack.push((d, true));
            for dep in self.deps(d) {
                if !self.records.contains_key(&dep) && !self.in_progress.contains(&dep) {
                    stack.push((dep, false));
                }
            }
        }
    }

    /// Classes `first` contains by value (fields through arrays and
    /// `_Atomic`, and bases).
    fn deps(&self, first: DeclId) -> Vec<DeclId> {
        let ast = self.ast;
        let Some(def) = ast.definition(first) else { return Vec::new() };
        let mut out = Vec::new();
        for f in props::fields(ast, def) {
            let mut v = props::strip_arrays(ast, f.ty);
            if let TypeView::Atomic(inner) = v {
                v = props::strip_arrays(ast, inner);
            }
            if let TypeView::Record(r) = v {
                out.push(ast.first(r));
            }
        }
        for b in ast.decls.bases(def, &ast.lists) {
            if let Some(r) = props::as_record(ast, b.ty) {
                out.push(r);
            }
        }
        out
    }

    fn compute(&mut self, first: DeclId) -> RecordFacts {
        let Some(def) = self.ast.definition(first) else { return RecordFacts::degenerate() };
        if self.ast.decls.record(def).is_none() {
            return RecordFacts::degenerate();
        }
        // clang `getASTRecordLayout`: an invalid class (an invalid field,
        // base or member subobject; spec/abi.md §1) has size, data size
        // and alignment 1, so no follow-on layout diagnostic fires.
        if self.ast.decls.get(def).has(DeclFlags::INVALID) {
            return RecordFacts::invalid(self.ast, def);
        }
        record::layout_record(self, def)
    }

    /// The facts of a by-value class member, laid out if needed.
    fn record_info(&mut self, d: DeclId) -> Info {
        let r = self.ast.first(d);
        if !self.records.contains_key(&r) {
            if self.in_progress.contains(&r) {
                return Info::DEGENERATE;
            }
            self.ensure(r);
        }
        self.records.get(&r).map_or(Info::DEGENERATE, |f| Info {
            width: f.layout.size.saturating_mul(8),
            align: u64::from(f.layout.align).saturating_mul(8).max(8),
        })
    }

    /// clang `ASTContext::getTypeInfo`: width and alignment in bits.
    pub(crate) fn type_info(&mut self, q: QualType) -> Info {
        let ast = self.ast;
        let t = self.target;
        match ast.types.view(q.ty()) {
            TypeView::Builtin(builtin) => {
                let (size, align) = t.builtin_size_align(builtin);
                let align = if builtin == BuiltinType::Void { 1 } else { align.max(1) };
                Info { width: size * 8, align: align * 8 }
            }
            TypeView::BitInt { width, .. } => {
                let (w, a) = t.bit_int_size_align_bits(width);
                Info { width: w, align: a }
            }
            TypeView::Pointer(_) | TypeView::LValueRef(_) | TypeView::RValueRef(_) => {
                Info { width: t.pointer_size * 8, align: t.pointer_size * 8 }
            }
            TypeView::MemberPointer { pointee, .. } => {
                let size = if ast.types.is_function(pointee) {
                    t.function_member_pointer_size
                } else {
                    t.data_member_pointer_size
                };
                Info { width: size * 8, align: t.pointer_size * 8 }
            }
            TypeView::ConstantArray(a) => {
                let e = self.type_info(a.elem);
                let width = e.width.saturating_mul(a.size);
                Info { width: align_to(width, e.align), align: e.align }
            }
            TypeView::IncompleteArray(elem) => Info { width: 0, align: self.type_info(elem).align },
            TypeView::VariableArray { elem, .. } | TypeView::DependentSizedArray { elem, .. } => {
                Info { width: 0, align: self.type_info(elem).align }
            }
            TypeView::QualifiedArray { base, .. } => self.type_info(QualType::unqual(base)),
            TypeView::Record(d) => self.record_info(d),
            TypeView::Enum(d) => self.enum_info(d),
            TypeView::Complex(e) => {
                let e = self.type_info(e);
                Info { width: e.width.saturating_mul(2), align: e.align }
            }
            TypeView::Atomic(v) => {
                let mut i = self.type_info(v);
                if i.width == 0 {
                    i.width = 8;
                } else if i.width <= t.max_atomic_promote_bits {
                    i.width = i.width.next_power_of_two();
                    i.align = i.width;
                }
                i
            }
            TypeView::Vector { elem, count, kind } => {
                // clang `getTypeInfo`: an ext_vector of `bool` packs one
                // bit per element (`isExtVectorBoolType`).
                let bool_ext = kind == VectorKind::Ext
                    && matches!(
                        ast.types.canonical_view(elem),
                        TypeView::Builtin(BuiltinType::Bool)
                    );
                let e = self.type_info(elem);
                let bits = if bool_ext { 1 } else { e.width };
                let mut width = bits.saturating_mul(u64::from(count)).max(8);
                let mut align = width;
                if !align.is_power_of_two() {
                    align = align.checked_next_power_of_two().unwrap_or(1 << 63);
                    width = align_to(width, align);
                }
                if t.max_vector_align_bits != 0 {
                    align = align.min(t.max_vector_align_bits);
                }
                // clang: a fixed-length SVE predicate is 2-byte aligned, a
                // fixed-length SVE vector 16-byte.
                match kind {
                    VectorKind::SveFixedPredicate => align = 16,
                    VectorKind::SveFixedData => align = 128,
                    _ => {}
                }
                Info { width, align }
            }
            TypeView::Typedef { decl, underlying } => {
                let mut i = self.type_info(underlying);
                let attr = max_align_attr(ast, decl, t, &mut |q| self.type_info(q).align);
                if attr != 0 {
                    i.align = attr;
                }
                i
            }
            TypeView::ExtQual { base, .. } => self.type_info(base),
            TypeView::Function(_) | TypeView::Invalid => Info::DEGENERATE,
            _ => {
                let d = ast.types.desugar_once(q);
                if d.ty() != q.ty() {
                    return self.type_info(d);
                }
                let c = ast.types.canonical(q);
                if c.ty() == q.ty() { Info::DEGENERATE } else { self.type_info(c) }
            }
        }
    }

    fn enum_info(&mut self, d: DeclId) -> Info {
        let ast = self.ast;
        let first = ast.first(d);
        let def = ast.definition(first).unwrap_or(first);
        let underlying = [def, first]
            .into_iter()
            .find_map(|e| ast.decls.enumeration(e).and_then(|x| x.int_type))
            .unwrap_or(QualType::INT);
        let mut i = self.type_info(underlying);
        let t = self.target;
        let attr = max_align_attr(ast, first, t, &mut |q| self.type_info(q).align);
        if attr != 0 {
            i.align = attr;
        }
        i
    }
}

/// Rounds `v` up to a multiple of `a` (`a` = 0 or 1 leaves it).
pub(crate) fn align_to(v: u64, a: u64) -> u64 {
    if a <= 1 { v } else { v.div_ceil(a).saturating_mul(a) }
}

/// `sizeof`/`alignof` of `q` on `target` (spec/types.md §11
/// `size_align`).
pub fn type_size_align(ast: &Ast, q: QualType, target: &Target) -> TypeLayout {
    Layouter::new(ast, target).type_layout(q)
}

/// The largest alignment, in bits, that `alignas` /
/// `__attribute__((aligned))` requests for declaration `d` on any of its
/// declarations (clang `Decl::getMaxAlignment`); 0 if none.
pub fn decl_max_align(ast: &Ast, d: DeclId, target: &Target) -> u64 {
    let mut lay = Layouter::new(ast, target);
    max_align_attr(ast, d, target, &mut |q| lay.type_info(q).align)
}

/// The alignment in bytes of the object declared by `d` (clang
/// `ASTContext::getDeclAlign(d, for_alignof)`; spec/abi.md §3.2):
///
/// - a variable with an `alignas` / `aligned` attribute: the attribute's
///   alignment alone ([dcl.align]/5; `aligned` may lower it);
/// - otherwise its type's (a reference's referee for `alignof`, a pointer
///   for storage), and for storage (`!for_alignof`) an x86-64 array of at
///   least 16 bytes is raised to 16 (`LargeArrayMinWidth`);
/// - a data member: the strictest of its attribute and type, capped by
///   what its offset guarantees inside the class (the class's alignment
///   or the low bit of the offset, whichever is smaller); a packed member
///   or a member of a packed class has its attribute's alignment only.
///
/// At least 1. No minimum global alignment applies on the M2 targets.
pub fn decl_align(ast: &Ast, d: DeclId, target: &Target, for_alignof: bool) -> u64 {
    use kiln_ast::attr::AttrKind;
    use kiln_ast::decl::DeclKind;
    let mut lay = Layouter::new(ast, target);
    let attr = max_align_attr(ast, d, target, &mut |q| lay.type_info(q).align);
    let node = ast.decls.get(d);
    let parent = (node.kind() == DeclKind::Field)
        .then(|| node.ctx().and_then(|p| ast.definition(p)))
        .flatten();
    let attr_only = match parent {
        Some(def) => {
            props::has_attr(ast, def, AttrKind::PACKED)
                || props::attrs(ast, d).any(|a| a.kind == AttrKind::PACKED)
        }
        None => attr != 0,
    };
    let mut align = if attr != 0 { attr } else { 8 };
    if !attr_only && let Some(ty) = node.ty() {
        let (t, info) = match ast.types.canonical_view(ty) {
            TypeView::LValueRef(r) | TypeView::RValueRef(r) if for_alignof => (r, lay.type_info(r)),
            TypeView::LValueRef(_) | TypeView::RValueRef(_) => {
                let p = target.pointer_size * 8;
                (ty, Info { width: p, align: p })
            }
            _ => (ty, lay.type_info(ty)),
        };
        align = align.max(info.align);
        let x86 = target.triple.arch == crate::target::Arch::X86_64;
        if x86 && !for_alignof && ast.types.is_array(t) && info.width >= 128 {
            align = align.max(128);
        }
        if let Some(def) = parent {
            let first = ast.first(def);
            let index = props::fields(ast, def).iter().position(|f| f.decl == d);
            let rl = lay.record(first);
            let mut field_align = u64::from(rl.align) * 8;
            if let Some(&offset) = index.and_then(|i| rl.fields.get(i))
                && offset > 0
            {
                field_align = field_align.min(offset & offset.wrapping_neg());
            }
            align = align.min(field_align);
        }
    }
    (align / 8).max(1)
}

/// The layout of `count` elements of `elem` ([dcl.array]/1: contiguous,
/// with the element's size as stride).
pub fn array_layout(ast: &Ast, elem: QualType, count: u64, target: &Target) -> TypeLayout {
    let e = Layouter::new(ast, target).type_layout(elem);
    TypeLayout { size: e.size.saturating_mul(count), align: e.align }
}

/// The Itanium layout of class `record` on `target` (spec/types.md §11).
pub fn record_layout(ast: &Ast, record: DeclId, target: &Target) -> RecordLayout {
    Layouter::new(ast, target).record(record).clone()
}

/// Whether class `first`'s definition has reached `}`
/// ([basic.types.general]/5).
fn is_complete_record(ast: &Ast, first: DeclId) -> bool {
    ast.definition(first)
        .and_then(|def| ast.decls.record(def))
        .is_some_and(|r| r.defn.contains(kiln_ast::decl::DefinitionBits::COMPLETE))
}

/// Lays out `record` and every class it contains by value, storing each
/// layout not yet cached in `ast.layouts` (keyed by first declaration),
/// and returns `record`'s.
///
/// `None` while `record`'s class is incomplete ([basic.types.general]/5:
/// no definition yet, or still being defined): its layout is not known,
/// and a cached one would never be invalidated (spec/abi.md §1).
pub fn ensure_layout<'a>(
    ast: &'a mut Ast,
    record: DeclId,
    target: &Target,
) -> Option<&'a RecordLayout> {
    let first = ast.first(record);
    if !is_complete_record(ast, first) {
        return None;
    }
    if ast.layouts.get(first).is_none() {
        let computed: Vec<(DeclId, RecordLayout)> = {
            let mut l = Layouter::new(ast, target);
            l.record(first);
            l.into_layouts().filter(|(d, _)| ast.layouts.get(*d).is_none()).collect()
        };
        for (d, layout) in computed {
            ast.layouts.insert(d, layout);
        }
    }
    ast.layouts.get(first)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kiln_ast::decl::{DefinitionBits, TagKind};
    use kiln_ast::names::DeclName;
    use kiln_ast::types::{BuiltinType, TypeContext};
    use kiln_ast::{Ast, DeclHead, QualType};
    use kiln_source::Loc;
    use kiln_source::symbol::Interner;

    use super::{Layouter, ensure_layout};
    use crate::target::target;

    /// [basic.types.general]/5: an incomplete class has no layout, so
    /// `ensure_layout` refuses it, before its definition and inside its
    /// member-specification, and caches nothing (spec/abi.md §1).
    #[test]
    fn ensure_layout_refuses_an_incomplete_class() {
        let tgt = target("x86_64-unknown-linux-gnu").expect("target");
        let mut ast = Ast::new(Arc::new(Interner::new()), TypeContext::new(tgt.types.clone()));
        let name = DeclName::ident(ast.symbols().intern("S"));
        let tu = ast.tu;
        let fwd =
            ast.builder().record(DeclHead::new(tu, name, Loc::INVALID), TagKind::Struct, None);
        assert!(ensure_layout(&mut ast, fwd, &tgt).is_none(), "declared, not defined");
        let def =
            ast.builder().record(DeclHead::new(tu, name, Loc::INVALID), TagKind::Struct, Some(fwd));
        ast.builder().begin_record_definition(def).expect("def");
        let x = DeclName::ident(ast.symbols().intern("x"));
        let _ = ast.builder().field(
            DeclHead::new(def, x, Loc::INVALID),
            BuiltinType::Int.qual_type(),
            None,
            0,
        );
        assert!(ensure_layout(&mut ast, fwd, &tgt).is_none(), "being defined");
        assert!(ensure_layout(&mut ast, def, &tgt).is_none(), "being defined");
        assert!(ast.layouts.get(fwd).is_none(), "nothing cached");
        ast.builder().complete_record_definition(def, DefinitionBits::empty());
        let l = ensure_layout(&mut ast, fwd, &tgt).expect("complete").clone();
        assert_eq!((l.size, l.align), (4, 4));
        assert_eq!(ensure_layout(&mut ast, def, &tgt).map(|l| l.size), Some(4));
    }

    /// [basic.types.general]/5: a session that laid out a class still
    /// being defined keeps the complete classes it contains, not that class
    /// (spec/abi.md §1, `into_complete_cache`; WP-428).
    #[test]
    fn into_complete_cache_drops_an_incomplete_class() {
        let tgt = target("x86_64-unknown-linux-gnu").expect("target");
        let mut ast = Ast::new(Arc::new(Interner::new()), TypeContext::new(tgt.types.clone()));
        let tu = ast.tu;
        let field = |ast: &mut Ast, rec, n: &str, ty| {
            let n = DeclName::ident(ast.symbols().intern(n));
            let _ = ast.builder().field(DeclHead::new(rec, n, Loc::INVALID), ty, None, 0);
        };
        let b_name = DeclName::ident(ast.symbols().intern("B"));
        let b =
            ast.builder().record(DeclHead::new(tu, b_name, Loc::INVALID), TagKind::Struct, None);
        ast.builder().begin_record_definition(b).expect("B");
        field(&mut ast, b, "x", BuiltinType::Long.qual_type());
        ast.builder().complete_record_definition(b, DefinitionBits::empty());
        let s_name = DeclName::ident(ast.symbols().intern("S"));
        let s =
            ast.builder().record(DeclHead::new(tu, s_name, Loc::INVALID), TagKind::Struct, None);
        ast.builder().begin_record_definition(s).expect("S");
        let b_ty = QualType::unqual(ast.types.record_type(b));
        field(&mut ast, s, "b", b_ty);
        field(&mut ast, s, "y", BuiltinType::Int.qual_type());
        let mut lay = Layouter::new(&ast, &tgt);
        assert_eq!(lay.record(s).size, 16, "laid out while being defined");
        let cache = lay.into_complete_cache();
        assert_eq!(cache.len(), 1, "B kept, S dropped");
        let mut lay = Layouter::with_cache(&ast, &tgt, cache);
        assert_eq!(lay.record(b).size, 8);
        assert!(lay.fresh.is_empty(), "B is not laid out again");
        ast.builder().complete_record_definition(s, DefinitionBits::empty());
        let mut lay = Layouter::new(&ast, &tgt);
        let _ = lay.record(s);
        assert_eq!(lay.into_complete_cache().len(), 2, "both complete");
    }
}
