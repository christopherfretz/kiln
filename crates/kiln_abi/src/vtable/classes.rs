//! Per-class facts the vtable builders read (clang `ASTRecordLayout`
//! plus `CXXRecordDecl::bases()` / `methods()`), cached over one
//! [`Layouter`] session, and the method predicates of
//! `VTableBuilder.cpp`.

use std::rc::Rc;

use kiln_ast::decl::{ConstexprKind, DeclFlags, DeclKind};
use kiln_ast::names::{DeclNameView, OverloadedOperator};
use kiln_ast::types::TypeView;
use kiln_ast::{Ast, DeclId, ListId, QualType};
use kiln_source::fx::{FxHashMap, FxHashSet};

use super::VTableError;
use crate::layout::{Layouter, MAX_SUBOBJECTS, props};

/// The deepest chain of dynamic classes the builders accept (they
/// recurse once per level, as clang does).
pub(crate) const MAX_DYNAMIC_DEPTH: usize = 256;

/// The builder steps one [`Classes`] session may take.
const FUEL: u64 = 1 << 26;

/// What the vtable builders need of one class.
#[derive(Debug)]
pub(crate) struct ClassInfo {
    /// Direct bases (first declarations) in declaration order, with
    /// `virtual`; non-class bases are skipped.
    pub bases: Vec<(DeclId, bool)>,
    /// Offsets of the direct non-virtual bases (bytes).
    pub nv_offsets: Vec<(DeclId, i64)>,
    /// Offsets of every virtual base in the complete object (bytes).
    pub vbase_offsets: Vec<(DeclId, i64)>,
    /// The primary base and whether it is virtual.
    pub primary: Option<(DeclId, bool)>,
    /// clang `isDynamicClass`.
    pub dynamic: bool,
    /// clang `isPolymorphic`: declares or inherits a virtual function.
    pub polymorphic: bool,
    /// The class's own virtual member functions (first declarations,
    /// declaration order), including immediate ones.
    pub virtuals: Vec<DeclId>,
    /// The subset of `virtuals` with a vtable slot (clang
    /// `hasVtableSlot`: not immediate).
    pub slots: Vec<DeclId>,
}

impl ClassInfo {
    /// The offset of direct non-virtual base `b` (0 if it is not one).
    pub fn base_offset(&self, b: DeclId) -> i64 {
        self.nv_offsets.iter().find(|(d, _)| *d == b).map_or(0, |&(_, o)| o)
    }

    /// The offset of virtual base `v` in this class as a complete object
    /// (0 if it is not one).
    pub fn vbase_offset(&self, v: DeclId) -> i64 {
        self.vbase_offsets.iter().find(|(d, _)| *d == v).map_or(0, |&(_, o)| o)
    }

    /// clang `getNumVBases() != 0`.
    pub fn has_vbases(&self) -> bool {
        !self.vbase_offsets.is_empty()
    }

    /// clang `isVirtuallyDerivedFrom(v)`.
    pub fn has_vbase(&self, v: DeclId) -> bool {
        self.vbase_offsets.iter().any(|(d, _)| *d == v)
    }
}

/// One base-class subobject reachable from a class, as clang's
/// `CXXBasePaths` walk finds it: its class, the last virtual base on the
/// path and the non-virtual offset after it (clang `ComputeBaseOffset`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct PathEntry {
    pub class: DeclId,
    pub vbase: Option<DeclId>,
    pub nv: i64,
}

/// A class-fact cache over one layout session.
pub(crate) struct Classes<'a> {
    pub lay: Layouter<'a>,
    infos: FxHashMap<DeclId, Rc<ClassInfo>>,
    paths: FxHashMap<DeclId, Rc<Vec<PathEntry>>>,
    /// `getVirtualBaseOffsetOffset` per class.
    pub vbase_offset_offsets: FxHashMap<DeclId, Rc<Vec<(DeclId, i64)>>>,
    /// Remaining builder steps before [`VTableError::TooComplex`].
    fuel: u64,
}

impl<'a> Classes<'a> {
    pub fn new(lay: Layouter<'a>) -> Self {
        Classes {
            lay,
            infos: FxHashMap::default(),
            paths: FxHashMap::default(),
            vbase_offset_offsets: FxHashMap::default(),
            fuel: FUEL,
        }
    }

    /// Spends one builder step: the walks are quadratic in the number
    /// of subobjects in the worst case, so a session is bounded.
    pub fn tick(&mut self) -> Result<(), VTableError> {
        self.fuel = self.fuel.checked_sub(1).ok_or(VTableError::TooComplex)?;
        Ok(())
    }

    pub fn ast(&self) -> &'a Ast {
        self.lay.ast()
    }

    /// The pointer size in bytes.
    pub fn ptr_bytes(&self) -> i64 {
        i64::try_from(self.lay.target().pointer_size).unwrap_or(8)
    }

    /// The facts of class `c` (any declaration). Callers bound the
    /// recursion through polymorphic bases with [`Self::check_depth`].
    pub fn get(&mut self, c: DeclId) -> Rc<ClassInfo> {
        let c = self.ast().first(c);
        if let Some(i) = self.infos.get(&c) {
            return Rc::clone(i);
        }
        let info = Rc::new(self.compute(c));
        self.infos.insert(c, Rc::clone(&info));
        info
    }

    fn compute(&mut self, c: DeclId) -> ClassInfo {
        let ast = self.ast();
        let layout = self.lay.record(c).clone();
        let dynamic = self.lay.facts(c).is_some_and(|f| f.dynamic);
        let def = ast.definition(c);
        let bases: Vec<(DeclId, bool)> = def
            .map(|d| props::bases(ast, d))
            .unwrap_or_default()
            .into_iter()
            .filter_map(|b| Some((b.class?, b.is_virtual)))
            .collect();
        let mut virtuals = Vec::new();
        if let Some(def) = def {
            for m in ast.members(def) {
                if is_method_kind(ast, m) && ast.first(m) == m && is_virtual(ast, m) {
                    virtuals.push(m);
                }
            }
        }
        let slots: Vec<DeclId> = virtuals.iter().copied().filter(|&m| has_slot(ast, m)).collect();
        let polymorphic = dynamic
            && (!virtuals.is_empty()
                || bases.iter().any(|&(b, _)| {
                    self.lay.facts(b).is_some_and(|f| f.dynamic) && self.get(b).polymorphic
                }));
        let to_i = |o: u64| i64::try_from(o).unwrap_or(i64::MAX);
        ClassInfo {
            nv_offsets: layout.bases.iter().map(|&(d, o)| (d, to_i(o))).collect(),
            vbase_offsets: layout.vbases.iter().map(|&(d, o)| (d, to_i(o))).collect(),
            primary: layout.primary_base,
            bases,
            dynamic,
            polymorphic,
            virtuals,
            slots,
        }
    }

    /// Refuses hierarchies whose chain of dynamic classes is deeper than
    /// [`MAX_DYNAMIC_DEPTH`] (computed without recursion).
    pub fn check_depth(&mut self, root: DeclId) -> Result<(), VTableError> {
        let ast = self.ast();
        let root = ast.first(root);
        self.lay.record(root);
        let mut depth: FxHashMap<DeclId, usize> = FxHashMap::default();
        let mut on_stack: FxHashSet<DeclId> = FxHashSet::default();
        let mut stack: Vec<(DeclId, bool)> = vec![(root, false)];
        while let Some((c, done)) = stack.pop() {
            if done {
                on_stack.remove(&c);
                let mut d = 0;
                for (b, _) in direct_dynamic_bases(self, c) {
                    d = d.max(depth.get(&b).copied().unwrap_or(0) + 1);
                }
                if d > MAX_DYNAMIC_DEPTH {
                    return Err(VTableError::TooComplex);
                }
                depth.insert(c, d);
                continue;
            }
            if depth.contains_key(&c) || !on_stack.insert(c) {
                continue;
            }
            stack.push((c, true));
            for (b, _) in direct_dynamic_bases(self, c) {
                if !depth.contains_key(&b) && !on_stack.contains(&b) {
                    stack.push((b, false));
                }
            }
        }
        Ok(())
    }

    /// Every base-class subobject of `derived` in clang's
    /// `lookupInBases` order (depth-first over base-specifiers in
    /// declaration order, each virtual base expanded at its first
    /// encounter only), without `derived` itself.
    pub fn paths(&mut self, derived: DeclId) -> Result<Rc<Vec<PathEntry>>, VTableError> {
        let derived = self.ast().first(derived);
        if let Some(p) = self.paths.get(&derived) {
            return Ok(Rc::clone(p));
        }
        let mut out: Vec<PathEntry> = Vec::new();
        let mut seen_virtual: FxHashSet<DeclId> = FxHashSet::default();
        // Frames: (class info, vbase, nv, next base index).
        let root = self.get(derived);
        let mut stack: Vec<(Rc<ClassInfo>, Option<DeclId>, i64, usize)> = vec![(root, None, 0, 0)];
        while let Some(top) = stack.last_mut() {
            let Some(&(b, is_virtual)) = top.0.bases.get(top.3) else {
                stack.pop();
                continue;
            };
            top.3 += 1;
            let entry = if is_virtual {
                if !seen_virtual.insert(b) {
                    continue;
                }
                PathEntry { class: b, vbase: Some(b), nv: 0 }
            } else {
                PathEntry { class: b, vbase: top.1, nv: top.2 + top.0.base_offset(b) }
            };
            out.push(entry);
            if out.len() > MAX_SUBOBJECTS {
                return Err(VTableError::TooComplex);
            }
            let info = self.get(b);
            stack.push((info, entry.vbase, entry.nv, 0));
        }
        let out = Rc::new(out);
        self.paths.insert(derived, Rc::clone(&out));
        Ok(out)
    }
}

/// The direct dynamic bases of `c`.
fn direct_dynamic_bases(cx: &mut Classes<'_>, c: DeclId) -> Vec<(DeclId, bool)> {
    let ast = cx.ast();
    let Some(def) = ast.definition(c) else { return Vec::new() };
    let mut out = Vec::new();
    for b in props::bases(ast, def) {
        let Some(b2) = b.class else { continue };
        cx.lay.record(b2);
        if cx.lay.facts(b2).is_some_and(|f| f.dynamic) {
            out.push((b2, b.is_virtual));
        }
    }
    out
}

/// True for the member-function kinds that can be virtual.
pub(crate) fn is_method_kind(ast: &Ast, m: DeclId) -> bool {
    matches!(
        ast.decls.get(m).kind(),
        DeclKind::CXXMethod | DeclKind::CXXDestructor | DeclKind::CXXConversion
    )
}

/// `virtual`, written or by overriding ([class.virtual]/2).
pub(crate) fn is_virtual(ast: &Ast, m: DeclId) -> bool {
    ast.decls.function(ast.first(m)).is_some_and(|f| f.spec.is_virtual() || f.spec.is_pure())
}

/// clang `hasVtableSlot`: virtual and not an immediate function.
pub(crate) fn has_slot(ast: &Ast, m: DeclId) -> bool {
    ast.decls
        .function(ast.first(m))
        .is_some_and(|f| f.spec.constexpr_kind() != ConstexprKind::Consteval && !f.spec.escalated())
}

pub(crate) fn is_pure(ast: &Ast, m: DeclId) -> bool {
    ast.decls.function(ast.first(m)).is_some_and(|f| f.spec.is_pure())
}

pub(crate) fn is_deleted(ast: &Ast, m: DeclId) -> bool {
    ast.decls.function(ast.first(m)).is_some_and(|f| f.spec.is_deleted())
}

pub(crate) fn is_dtor(ast: &Ast, m: DeclId) -> bool {
    ast.decls.get(m).kind() == DeclKind::CXXDestructor
}

pub(crate) fn is_implicit(ast: &Ast, m: DeclId) -> bool {
    ast.decls.get(m).has(DeclFlags::IMPLICIT)
}

/// The class declaring method `m` (first declaration).
pub(crate) fn parent(ast: &Ast, m: DeclId) -> DeclId {
    let m = ast.first(m);
    ast.decls.get(m).ctx().map_or(m, |c| ast.first(c))
}

/// The methods `m` directly overrides (first declarations).
pub(crate) fn overridden(ast: &Ast, m: DeclId) -> Vec<DeclId> {
    let list = ast.decls.cold.overrides.get(ast.first(m)).copied().unwrap_or(ListId::EMPTY);
    ast.lists.iter::<DeclId>(list).map(|o| ast.first(o)).collect()
}

/// clang `ComputeAllOverriddenMethods`: every method `m` overrides,
/// directly or indirectly.
pub(crate) fn all_overridden(ast: &Ast, m: DeclId) -> Vec<DeclId> {
    let mut out: Vec<DeclId> = Vec::new();
    let mut stack = overridden(ast, m);
    stack.reverse();
    while let Some(o) = stack.pop() {
        if out.contains(&o) {
            continue;
        }
        out.push(o);
        let mut next = overridden(ast, o);
        next.reverse();
        stack.extend(next);
    }
    out
}

/// The canonical function type of `m`.
fn fn_type(ast: &Ast, m: DeclId) -> Option<QualType> {
    ast.decls.get(ast.first(m)).ty().map(|t| ast.types.canonical(t))
}

/// clang `VCallOffsetMap::MethodsCanShareVCallOffset`.
pub(crate) fn can_share_vcall_offset(ast: &Ast, a: DeclId, b: DeclId) -> bool {
    if is_dtor(ast, a) {
        return is_dtor(ast, b);
    }
    if ast.decls.get(a).name() != ast.decls.get(b).name() {
        return false;
    }
    let (Some(ta), Some(tb)) = (fn_type(ast, a), fn_type(ast, b)) else { return false };
    if ta == tb {
        return true;
    }
    let (Some(pa), Some(pb)) = (ast.types.proto(ta), ast.types.proto(tb)) else { return false };
    pa.info().this_quals() == pb.info().this_quals() && ast.types.params(pa) == ast.types.params(pb)
}

/// The return type of method `m` (canonical).
pub(crate) fn return_type(ast: &Ast, m: DeclId) -> Option<QualType> {
    let t = fn_type(ast, m)?;
    ast.types.proto(t).map(|p| ast.types.canonical(p.ret()))
}

/// The class `q` designates (first declaration).
pub(crate) fn record_of(ast: &Ast, q: QualType) -> Option<DeclId> {
    match ast.types.canonical_view(q) {
        TypeView::Record(d) => Some(ast.first(d)),
        _ => None,
    }
}

/// The sort key of an implicit virtual function (clang `AddMethods`):
/// copy assignment, move assignment, destructor, then the rest.
pub(crate) fn implicit_rank(ast: &Ast, m: DeclId) -> u8 {
    if let Some(kind) = assignment_kind(ast, m) {
        return kind;
    }
    if is_dtor(ast, m) { 2 } else { 3 }
}

/// 0 for a copy-assignment operator, 1 for a move-assignment operator
/// ([class.copy.assign]/1, /3).
fn assignment_kind(ast: &Ast, m: DeclId) -> Option<u8> {
    let node = ast.decls.get(m);
    if !matches!(ast.names.view(node.name()), DeclNameView::Operator(OverloadedOperator::Equal)) {
        return None;
    }
    let class = parent(ast, m);
    let proto = ast.types.proto(node.ty()?)?;
    let params = ast.types.params(proto);
    if params.len() != 1 || proto.variadic() {
        return None;
    }
    match ast.types.canonical_view(params[0]) {
        TypeView::LValueRef(p) => (record_of(ast, p) == Some(class)).then_some(0),
        TypeView::RValueRef(p) => (record_of(ast, p) == Some(class)).then_some(1),
        TypeView::Record(d) => (ast.first(d) == class).then_some(0),
        _ => None,
    }
}
