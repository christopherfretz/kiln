//! The subobject graph and final overriders of a most-derived class
//! ([class.virtual]/2–3, [class.mi]; spec/sema-class.md §3.5): pure
//! functions of the [`Ast`], shared by sema (abstractness and the "more
//! than one final overrider" check at `}`) and the Oven bridge (computed
//! on demand and cached in `Ast::virtuals`).
//!
//! Numbering matches `kiln_abi::layout::subobject`: 0 is the class, then
//! each direct non-virtual base's subtree in declaration order (preorder),
//! then each virtual base once, in Itanium inheritance-graph order (a
//! depth-first, left-to-right walk that numbers a virtual base at its
//! first encounter), with its own non-virtual subtree.

use kiln_ast::class::{ClassVirtuals, Subobject, SubobjectId};
use kiln_ast::decl::{DeclKind, DefinitionBits};
use kiln_ast::types::TypeView;
use kiln_ast::{Ast, DeclId, ListId, QualType};
use kiln_source::fx::{FxHashMap, FxHashSet};

/// The most subobjects a graph holds (`SubobjectId` is a `u16`; the same
/// cap as `kiln_abi::layout::subobject::MAX_SUBOBJECTS`).
pub const MAX_SUBOBJECTS: usize = u16::MAX as usize + 1;

/// Recursion bound for walks over the base-class graph.
const MAX_DEPTH: usize = 1024;

/// One base-specifier of a class: its position in the base list, the
/// base class (first declaration) and whether it is virtual.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DirectBase {
    pub index: u16,
    pub class: DeclId,
    pub is_virtual: bool,
}

/// The class a type designates (first declaration).
pub(crate) fn record_of(ast: &Ast, q: QualType) -> Option<DeclId> {
    match ast.types.canonical_view(q) {
        TypeView::Record(d) => Some(ast.first(d)),
        _ => None,
    }
}

/// The direct bases of class `c` (any declaration; none until defined).
pub(crate) fn direct_bases(ast: &Ast, c: DeclId) -> Vec<DirectBase> {
    let Some(def) = ast.definition(c) else { return Vec::new() };
    ast.decls
        .bases(def, &ast.lists)
        .enumerate()
        .filter_map(|(i, b)| {
            Some(DirectBase {
                index: u16::try_from(i).unwrap_or(u16::MAX),
                class: record_of(ast, b.ty)?,
                is_virtual: b.is_virtual,
            })
        })
        .collect()
}

/// True for the member-function kinds that can be virtual.
pub(crate) fn is_method_kind(k: DeclKind) -> bool {
    matches!(k, DeclKind::CXXMethod | DeclKind::CXXDestructor | DeclKind::CXXConversion)
}

/// Whether method `m` (any declaration) is virtual.
pub(crate) fn is_virtual(ast: &Ast, m: DeclId) -> bool {
    is_method_kind(ast.decls.get(m).kind())
        && ast.decls.function(ast.first(m)).is_some_and(|f| f.spec.is_virtual())
}

/// Whether method `m` (any declaration) is pure virtual.
pub(crate) fn is_pure(ast: &Ast, m: DeclId) -> bool {
    ast.decls.function(ast.first(m)).is_some_and(|f| f.spec.is_pure())
}

/// The methods `m` directly overrides (first declarations).
pub(crate) fn overridden(ast: &Ast, m: DeclId) -> impl Iterator<Item = DeclId> + '_ {
    let list = ast.decls.cold.overrides.get(ast.first(m)).copied().unwrap_or(ListId::EMPTY);
    ast.lists.iter::<DeclId>(list)
}

/// The virtual member functions declared in class `c` (first
/// declarations, in declaration order).
pub(crate) fn own_virtuals(ast: &Ast, c: DeclId) -> Vec<DeclId> {
    let Some(def) = ast.definition(c) else { return Vec::new() };
    let mut out = Vec::new();
    for m in ast.members(def) {
        if is_method_kind(ast.decls.get(m).kind()) && ast.first(m) == m && is_virtual(ast, m) {
            out.push(m);
        }
    }
    out
}

/// `m` and every method it overrides, directly or indirectly (first
/// declarations).
pub(crate) fn override_closure(ast: &Ast, m: DeclId) -> Vec<DeclId> {
    let mut out = vec![ast.first(m)];
    let mut i = 0;
    while i < out.len() && out.len() < 4096 {
        let cur = out[i];
        for o in overridden(ast, cur) {
            let o = ast.first(o);
            if !out.contains(&o) {
                out.push(o);
            }
        }
        i += 1;
    }
    out
}

/// The subobject graph of `record` (any declaration). Truncated at
/// [`MAX_SUBOBJECTS`].
pub fn subobject_graph(ast: &Ast, record: DeclId) -> Vec<Subobject> {
    let root = ast.first(record);
    let mut out: Vec<Subobject> = Vec::new();
    number_subtree(
        ast,
        Subobject { class: root, parent: SubobjectId::COMPLETE, base_index: 0, is_virtual: false },
        &mut out,
    );
    // Virtual bases in inheritance-graph order.
    let mut vbases: Vec<DeclId> = Vec::new();
    let mut visited: FxHashSet<DeclId> = FxHashSet::default();
    collect_vbases(ast, root, &mut vbases, &mut visited, 0);
    for v in vbases {
        if out.len() >= MAX_SUBOBJECTS {
            break;
        }
        // The parent: the first subobject whose class names `v` as a
        // direct virtual base.
        let (parent, base_index) = out
            .iter()
            .enumerate()
            .find_map(|(i, s)| {
                direct_bases(ast, s.class)
                    .into_iter()
                    .find(|b| b.is_virtual && b.class == v)
                    .map(|b| (SubobjectId(u16::try_from(i).unwrap_or(u16::MAX)), b.index))
            })
            .unwrap_or((SubobjectId::COMPLETE, 0));
        number_subtree(ast, Subobject { class: v, parent, base_index, is_virtual: true }, &mut out);
    }
    out
}

/// Numbers `top` and its non-virtual base subtree in preorder.
fn number_subtree(ast: &Ast, top: Subobject, out: &mut Vec<Subobject>) {
    let mut stack = vec![top];
    while let Some(sub) = stack.pop() {
        if out.len() >= MAX_SUBOBJECTS {
            break;
        }
        let id = SubobjectId(u16::try_from(out.len()).unwrap_or(u16::MAX));
        out.push(sub);
        let first_child = stack.len();
        for b in direct_bases(ast, sub.class) {
            if !b.is_virtual {
                stack.push(Subobject {
                    class: b.class,
                    parent: id,
                    base_index: b.index,
                    is_virtual: false,
                });
            }
        }
        stack[first_child..].reverse();
    }
}

/// The virtual bases of `c` in inheritance-graph order (a class once
/// walked has had all its virtual bases numbered).
fn collect_vbases(
    ast: &Ast,
    c: DeclId,
    out: &mut Vec<DeclId>,
    visited: &mut FxHashSet<DeclId>,
    depth: usize,
) {
    if depth > MAX_DEPTH || !visited.insert(c) {
        return;
    }
    for b in direct_bases(ast, c) {
        if b.is_virtual && !out.contains(&b.class) {
            out.push(b.class);
        }
        collect_vbases(ast, b.class, out, visited, depth + 1);
    }
}

/// True if class `c` has a virtual base (directly or indirectly).
pub(crate) fn has_virtual_bases(ast: &Ast, c: DeclId) -> bool {
    let mut out = Vec::new();
    let mut visited = FxHashSet::default();
    collect_vbases(ast, ast.first(c), &mut out, &mut visited, 0);
    !out.is_empty()
}

/// The final overriders of every virtual function in every subobject of
/// `record`: `(subobject, method, overrider, overrider's subobject)` for
/// each candidate that no other candidate dominates, sorted. A
/// `(subobject, method)` pair with more than one entry has no unique
/// final overrider ([class.virtual]/2; an error when the class is
/// complete, reported by sema).
pub(crate) fn all_final_overriders(
    ast: &Ast,
    subobjects: &[Subobject],
) -> Vec<(SubobjectId, DeclId, DeclId, SubobjectId)> {
    let n = subobjects.len();
    let sid = |i: usize| SubobjectId(u16::try_from(i).unwrap_or(u16::MAX));
    // The subobjects of each class, and each class's direct virtual
    // bases, for the containment edges.
    let mut by_class: FxHashMap<DeclId, Vec<usize>> = FxHashMap::default();
    for (i, s) in subobjects.iter().enumerate() {
        by_class.entry(s.class).or_default().push(i);
    }
    let mut vcontainers: FxHashMap<DeclId, Vec<usize>> = FxHashMap::default();
    for (i, s) in subobjects.iter().enumerate() {
        for b in direct_bases(ast, s.class) {
            if b.is_virtual {
                vcontainers.entry(b.class).or_default().push(i);
            }
        }
    }
    // Per class: overridden method → the class's own virtual that
    // overrides it (or is it).
    let mut inv: FxHashMap<DeclId, FxHashMap<DeclId, DeclId>> = FxHashMap::default();
    for &c in by_class.keys() {
        let mut map = FxHashMap::default();
        for f in own_virtuals(ast, c) {
            for m in override_closure(ast, f) {
                map.entry(m).or_insert(f);
            }
        }
        inv.insert(c, map);
    }
    let containers = |i: usize| -> Vec<usize> {
        let s = subobjects[i];
        if i == 0 {
            Vec::new()
        } else if s.is_virtual {
            vcontainers.get(&s.class).cloned().unwrap_or_default()
        } else {
            vec![s.parent.index()]
        }
    };
    // The subobjects containing each subobject (itself included), as a
    // sorted list of indices.
    let ancestors = |i: usize| -> Vec<usize> {
        let mut seen: FxHashSet<usize> = FxHashSet::default();
        let mut work = vec![i];
        seen.insert(i);
        while let Some(x) = work.pop() {
            for c in containers(x) {
                if c < n && seen.insert(c) {
                    work.push(c);
                }
            }
        }
        let mut v: Vec<usize> = seen.into_iter().collect();
        v.sort_unstable();
        v
    };
    let mut anc_cache: FxHashMap<usize, Vec<usize>> = FxHashMap::default();
    let mut out = Vec::new();
    for (x, sub) in subobjects.iter().enumerate() {
        let methods = own_virtuals(ast, sub.class);
        if methods.is_empty() {
            continue;
        }
        let anc_x = anc_cache.entry(x).or_insert_with(|| ancestors(x)).clone();
        for m in methods {
            let mut cands: Vec<(DeclId, usize)> = Vec::new();
            for &t in &anc_x {
                if let Some(&f) = inv.get(&subobjects[t].class).and_then(|mp| mp.get(&m)) {
                    cands.push((f, t));
                }
            }
            let mut finals: Vec<(DeclId, usize)> = Vec::new();
            for &(f, t) in &cands {
                let anc_t = anc_cache.entry(t).or_insert_with(|| ancestors(t));
                let dominated =
                    cands.iter().any(|&(_, t2)| t2 != t && anc_t.binary_search(&t2).is_ok());
                if !dominated {
                    finals.push((f, t));
                }
            }
            for (f, t) in finals {
                out.push((sid(x), m, f, sid(t)));
            }
        }
    }
    out.sort_by_key(|e| (e.0, e.1, e.3, e.2));
    out
}

/// The subobjects and final overriders of complete class `record` (any
/// declaration), or `None` if it is not complete. A `(subobject,
/// method)` without a unique final overrider records its first
/// candidate.
pub fn compute_virtuals(ast: &Ast, record: DeclId) -> Option<ClassVirtuals> {
    let def = ast.definition(record)?;
    let r = ast.decls.record(def)?;
    if !r.defn.contains(DefinitionBits::COMPLETE) {
        return None;
    }
    Some(virtuals_of(ast, record))
}

/// [`compute_virtuals`] without the completeness check (sema computes it
/// at `}`, before the class is marked complete).
pub(crate) fn virtuals_of(ast: &Ast, record: DeclId) -> ClassVirtuals {
    let subobjects = subobject_graph(ast, record);
    let mut overriders = all_final_overriders(ast, &subobjects);
    overriders.dedup_by_key(|e| (e.0, e.1));
    ClassVirtuals { subobjects, overriders }
}

/// The cached [`ClassVirtuals`] of complete class `record`, computing and
/// caching it on first use (`None` if the class is not complete).
pub fn ensure_virtuals(ast: &mut Ast, record: DeclId) -> Option<&ClassVirtuals> {
    let first = ast.first(record);
    if ast.virtuals.get(first).is_none() {
        let v = compute_virtuals(ast, first)?;
        ast.virtuals.insert(first, v);
    }
    ast.virtuals.get(first)
}

/// The virtual functions of `record` and of its bases in the order of
/// clang's `CXXFinalOverriderMap` (the order of its diagnostics): each
/// base first (depth-first, left to right, a class once), then the
/// class's own, in declaration order. First declarations.
pub(crate) fn method_order(ast: &Ast, record: DeclId) -> Vec<DeclId> {
    fn walk(
        ast: &Ast,
        c: DeclId,
        out: &mut Vec<DeclId>,
        seen: &mut FxHashSet<DeclId>,
        visited: &mut FxHashSet<DeclId>,
        depth: usize,
    ) {
        if depth > MAX_DEPTH || !visited.insert(c) {
            return;
        }
        for b in direct_bases(ast, c) {
            walk(ast, b.class, out, seen, visited, depth + 1);
        }
        for m in own_virtuals(ast, c) {
            if seen.insert(m) {
                out.push(m);
            }
        }
    }
    let mut out = Vec::new();
    walk(ast, ast.first(record), &mut out, &mut FxHashSet::default(), &mut FxHashSet::default(), 0);
    out
}
