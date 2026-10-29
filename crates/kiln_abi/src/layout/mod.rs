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
pub(crate) mod props;
mod record;
mod subobject;

use kiln_ast::types::{BuiltinType, RecordLayout, TypeView};
use kiln_ast::{Ast, DeclId, QualType};
use kiln_source::fx::{FxHashMap, FxHashSet};

use crate::target::Target;
use props::max_align_attr;
pub use subobject::{MAX_SUBOBJECTS, subobjects};

/// Size and alignment of a type, in bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
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
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FieldTarget {
    /// Not a class or array of class (or a bit-field).
    None,
    /// A class (first declaration).
    Record(DeclId),
    /// A constant array of `n` (flattened) elements of a class.
    Array(DeclId, u64),
}

/// Everything computed for one class.
#[derive(Clone, Debug)]
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
    fn degenerate() -> RecordFacts {
        RecordFacts {
            layout: RecordLayout {
                align: 1,
                preferred_align: 1,
                nv_align: 1,
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

/// A layout session over one AST and target: caches every class it lays
/// out (keyed by the class's first declaration).
#[derive(Debug)]
pub struct Layouter<'a> {
    ast: &'a Ast,
    target: &'a Target,
    records: FxHashMap<DeclId, RecordFacts>,
    in_progress: FxHashSet<DeclId>,
}

impl<'a> Layouter<'a> {
    /// A session with an empty cache.
    pub fn new(ast: &'a Ast, target: &'a Target) -> Self {
        Layouter { ast, target, records: FxHashMap::default(), in_progress: FxHashSet::default() }
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
            TypeView::Vector { elem, count, .. } => {
                let e = self.type_info(elem);
                let mut width = e.width.saturating_mul(u64::from(count)).max(8);
                let mut align = width;
                if !align.is_power_of_two() {
                    align = align.checked_next_power_of_two().unwrap_or(1 << 63);
                    width = align_to(width, align);
                }
                if t.max_vector_align_bits != 0 {
                    align = align.min(t.max_vector_align_bits);
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

/// Lays out `record` and every class it contains by value, storing each
/// layout not yet cached in `ast.layouts` (keyed by first declaration),
/// and returns `record`'s.
pub fn ensure_layout<'a>(ast: &'a mut Ast, record: DeclId, target: &Target) -> &'a RecordLayout {
    let first = ast.first(record);
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
    ast.layouts.get(first).expect("inserted above")
}
