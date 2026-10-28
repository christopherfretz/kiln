//! Base-class paths for derived-to-base conversions ([class.derived],
//! [conv.ptr]/3, [conv.mem]/2, [dcl.init.ref]).
//!
//! A cast path is the list of base-specifiers walked from the derived
//! class to the base, each named by its raw index into the record's
//! base list (the `ListId` entries of `RecordData::bases`): the same
//! numbering `DeclArena::bases` iterates in.

use kiln_ast::decl::DefinitionBits;
use kiln_ast::{DeclId, ListId, QualType};

use crate::Sema;

/// One way from a derived class to a base class subobject.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BasePath {
    /// The base-specifier indices walked, derived first.
    pub specs: Vec<u32>,
    /// The classes visited after the derived class, ending with the base.
    pub classes: Vec<DeclId>,
    /// The base-specifier types as written, parallel to `classes`.
    pub written: Vec<QualType>,
    /// The last class reached through a `virtual` base-specifier, if any.
    pub last_virtual: Option<DeclId>,
    /// The first class reached through a `virtual` base-specifier, if
    /// any (clang's "detected virtual" base, named in diagnostics).
    pub first_virtual: Option<DeclId>,
}

/// What a search for a base class found.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BaseSearch {
    /// The class is not derived from the base (or either is incomplete).
    NotDerived,
    /// Exactly one base subobject: its path.
    Unique(BasePath),
    /// More than one distinct base subobject ([class.member.lookup]):
    /// every path found.
    Ambiguous(Vec<BasePath>),
}

impl BaseSearch {
    /// True unless [`BaseSearch::NotDerived`].
    pub fn is_derived(&self) -> bool {
        !matches!(self, BaseSearch::NotDerived)
    }
}

/// The complete definition of class `d`, if any.
pub(crate) fn complete_class(s: &Sema<'_, '_>, d: DeclId) -> Option<DeclId> {
    let def = s.ast.definition(d)?;
    let r = s.ast.decls.record(def)?;
    r.defn.contains(DefinitionBits::COMPLETE).then_some(def)
}

fn same_class(s: &Sema<'_, '_>, a: DeclId, b: DeclId) -> bool {
    a == b || s.ast.first(a) == s.ast.first(b)
}

const MAX_DEPTH: usize = 1024;

fn walk(s: &Sema<'_, '_>, cur: DeclId, base: DeclId, path: &mut BasePath, out: &mut Vec<BasePath>) {
    if path.specs.len() > MAX_DEPTH || out.len() > 64 {
        return;
    }
    let Some(def) = complete_class(s, cur) else { return };
    let Some(rec) = s.ast.decls.record(def) else { return };
    let idx = s.ast.lists.get(rec.bases);
    let specs: Vec<_> = s.ast.decls.bases(def, &s.ast.lists).copied().collect();
    for (&i, spec) in idx.iter().zip(specs) {
        let Some(c) = s.ast.types.as_record(spec.ty) else { continue };
        let saved_virtual = (path.last_virtual, path.first_virtual);
        path.specs.push(i);
        path.classes.push(c);
        path.written.push(spec.ty);
        if spec.is_virtual {
            path.last_virtual = Some(c);
            path.first_virtual = path.first_virtual.or(Some(c));
        }
        if same_class(s, c, base) {
            out.push(path.clone());
        } else {
            walk(s, c, base, path, out);
        }
        path.specs.pop();
        path.classes.pop();
        path.written.pop();
        (path.last_virtual, path.first_virtual) = saved_virtual;
    }
}

/// The subobject a path designates: paths through the same virtual base
/// share their prefix up to it.
fn subobject_key(s: &Sema<'_, '_>, p: &BasePath) -> (Option<DeclId>, Vec<u32>) {
    match p.last_virtual {
        Some(v) => {
            let at = p.classes.iter().rposition(|&c| same_class(s, c, v)).unwrap_or(0);
            (Some(s.ast.first(v)), p.specs[at + 1..].to_vec())
        }
        None => (None, p.specs.clone()),
    }
}

/// Searches for `base` among the (direct and indirect) bases of
/// `derived` ([class.derived.general]/2). A class is not its own base.
pub fn find_base(s: &Sema<'_, '_>, derived: DeclId, base: DeclId) -> BaseSearch {
    if same_class(s, derived, base) {
        return BaseSearch::NotDerived;
    }
    let mut out = Vec::new();
    let mut path = BasePath {
        specs: Vec::new(),
        classes: Vec::new(),
        written: Vec::new(),
        last_virtual: None,
        first_virtual: None,
    };
    walk(s, derived, base, &mut path, &mut out);
    if out.is_empty() {
        return BaseSearch::NotDerived;
    }
    let first = subobject_key(s, &out[0]);
    if out.iter().skip(1).all(|p| subobject_key(s, p) == first) {
        BaseSearch::Unique(out.swap_remove(0))
    } else {
        BaseSearch::Ambiguous(out)
    }
}

/// True if `derived` is derived (unambiguously or not) from `base`.
pub fn is_derived_from(s: &Sema<'_, '_>, derived: DeclId, base: DeclId) -> bool {
    find_base(s, derived, base).is_derived()
}

/// Interns `path` as a cast path list.
pub fn path_list(s: &mut Sema<'_, '_>, path: &BasePath) -> ListId {
    s.ast.lists.push_raw(&path.specs)
}

/// clang's display of ambiguous paths: one line per path,
/// `\n    struct D -> B -> A` (the derived class with its class-key, then
/// the base-specifiers as written).
pub(crate) fn ambiguous_paths_string(
    s: &Sema<'_, '_>,
    derived_ty: QualType,
    paths: &[BasePath],
) -> String {
    let derived_ty = s.ast.types.canonical(derived_ty).without_quals();
    let Some(derived) = s.ast.types.as_record(derived_ty) else { return String::new() };
    let def = s.ast.definition(derived).unwrap_or(derived);
    let key = s.ast.decls.record(def).map_or("struct", |r| r.tag.keyword());
    let dname = s.ty_str(derived_ty);
    let mut out = String::new();
    for p in paths {
        out.push_str("\n    ");
        out.push_str(key);
        out.push(' ');
        out.push_str(&dname);
        for &w in &p.written {
            out.push_str(" -> ");
            out.push_str(&s.ty_str(w));
        }
    }
    out
}
