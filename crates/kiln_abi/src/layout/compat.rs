//! Layout compatibility and the common initial sequence
//! ([basic.types.general]/11, [class.mem.general]/23-25), and the two
//! member queries built on them: corresponding members
//! (`std::is_corresponding_member`, [meta.member]/4) and members
//! pointer-interconvertible with their class
//! (`std::is_pointer_interconvertible_with_class`, [meta.member]/2,
//! [basic.compound]/5). spec/abi.md "Layout compatibility".
//!
//! This is the one implementation: sema's `__is_layout_compatible`, the
//! Oven's evaluation of `__builtin_is_corresponding_member` and
//! `__builtin_is_pointer_interconvertible_with_class`, and codegen's
//! run-time lowering of both call it (WP-1472).
//!
//! The common initial sequence follows the text ([class.mem.general]/23:
//! layout-compatible types, the same alignment requirement, both or
//! neither `[[no_unique_address]]`, both bit-fields of the same width or
//! neither) and, as GCC 16.1 does, also requires the two members to sit
//! at the same offset: the text's conditions alone would pair members at
//! different offsets (an empty `[[no_unique_address]]` member that may
//! or may not share an address with its neighbour; layout-compatible
//! class types of different sizes), which no union read could honour.
//! clang 19/22 instead exclude every `[[no_unique_address]]` member.

use kiln_ast::decl::{DeclKind, DefinitionBits, EnumData, TagKind};
use kiln_ast::types::TypeView;
use kiln_ast::{Ast, DeclId, QualType};

use super::props::{FieldDesc, fields};
use crate::target::Target;

/// The recursion bound of the type and class walks.
const MAX_DEPTH: usize = 256;

/// The two classes' fields and how many leading pairs form the common
/// initial sequence.
struct Cis {
    x: Vec<FieldDesc>,
    y: Vec<FieldDesc>,
    /// Bit offsets of `x`'s fields in their class.
    ox: Box<[u64]>,
    len: usize,
}

struct Cx<'a> {
    ast: &'a Ast,
    target: &'a Target,
}

/// Whether `a` and `b` are layout-compatible types
/// ([basic.types.general]/11): the same type ignoring cv-qualification,
/// enumerations with the same underlying type, or layout-compatible
/// standard-layout classes. Incomplete classes are not.
pub fn layout_compatible(ast: &Ast, target: &Target, a: QualType, b: QualType) -> bool {
    Cx { ast, target }.types(a, b, 0)
}

/// Whether `m1` and `m2` (data members, any declaration) are
/// corresponding members of the common initial sequence of `s1` and `s2`
/// ([meta.member]/4): both standard-layout structs (not unions). The
/// members of a corresponding pair of GNU anonymous structs correspond as
/// in the anonymous structs' own sequence (GCC); members of anonymous
/// unions never do (they are not members of the sequence; GCC gives
/// "sorry, unimplemented").
pub fn corresponding_member(
    ast: &Ast,
    target: &Target,
    s1: QualType,
    s2: QualType,
    m1: DeclId,
    m2: DeclId,
) -> bool {
    let cx = Cx { ast, target };
    let (Some(x), Some(y)) = (cx.sl_struct(s1), cx.sl_struct(s2)) else { return false };
    cx.corresponding_in(x, y, cx.field(m1), cx.field(m2), 0)
}

/// The byte offsets in `s1` of the members that correspond to a member
/// of `s2` ([`corresponding_member`]) where the first has type `t1` and
/// the second `t2` (cv-qualification ignored): what a run-time
/// `__builtin_is_corresponding_member(m1, m2)` compares `m1 == m2`
/// against (corresponding members have equal offsets).
pub fn corresponding_offsets(
    ast: &Ast,
    target: &Target,
    s1: QualType,
    s2: QualType,
    t1: QualType,
    t2: QualType,
) -> Vec<u64> {
    let cx = Cx { ast, target };
    let mut out = Vec::new();
    if let (Some(x), Some(y)) = (cx.sl_struct(s1), cx.sl_struct(s2)) {
        cx.offsets_in(x, y, (t1, t2), 0, 0, &mut out);
    }
    out
}

/// Whether an object of class `s` is pointer-interconvertible with its
/// data member `m` ([basic.compound]/5, [meta.member]/2): every member
/// of a union is; the first non-static data member of a standard-layout
/// class is, and so is (transitively) the first member of a
/// standard-layout anonymous struct that is itself pointer-
/// interconvertible with the enclosing object; a member of a base class
/// is when `s` is standard-layout and the base declares `s`'s members.
pub fn pointer_interconvertible_member(ast: &Ast, target: &Target, s: QualType, m: DeclId) -> bool {
    let cx = Cx { ast, target };
    let Some(r) = cx.complete_record(s) else { return false };
    cx.interconvertible(r, cx.field(m))
}

/// Whether some data member of `s` of type `t` (cv-qualification
/// ignored) that a `t s::*` can designate is pointer-interconvertible
/// with `s` ([`pointer_interconvertible_member`]); every such member is
/// at offset 0, so at run time the builtin is `m == 0` when this holds
/// and false otherwise.
pub fn interconvertible_member_of_type(
    ast: &Ast,
    target: &Target,
    s: QualType,
    t: QualType,
) -> bool {
    let cx = Cx { ast, target };
    let Some(r) = cx.complete_record(s) else { return false };
    let mut found = false;
    cx.each_member(r, 0, &mut |f| {
        found = found
            || (f.bit_width.is_none()
                && cx.same_unqual(f.ty, t)
                && cx.interconvertible(r, ast.first(f.decl)));
    });
    found
}

impl Cx<'_> {
    /// The field member `m` designates (first declaration): a member of
    /// an anonymous class named in the enclosing one is an
    /// `IndirectField`, whose chain ends in the field.
    fn field(&self, m: DeclId) -> DeclId {
        let m = self.ast.first(m);
        if self.ast.decls.get(m).kind() != DeclKind::IndirectField {
            return m;
        }
        let chain = self.ast.indirect_field_chain(m);
        self.ast.lists.iter::<DeclId>(chain).last().map_or(m, |f| self.ast.first(f))
    }

    fn def(&self, d: DeclId) -> DeclId {
        self.ast.definition(d).unwrap_or(d)
    }

    fn bits(&self, d: DeclId) -> DefinitionBits {
        self.ast.decls.record(self.def(d)).map(|r| r.defn).unwrap_or_default()
    }

    fn is_union(&self, d: DeclId) -> bool {
        self.ast.decls.record(self.def(d)).is_some_and(|r| r.tag == TagKind::Union)
    }

    fn standard_layout(&self, d: DeclId) -> bool {
        let b = self.bits(d);
        b.contains(DefinitionBits::COMPLETE) && b.contains(DefinitionBits::STANDARD_LAYOUT)
    }

    fn complete_record(&self, q: QualType) -> Option<DeclId> {
        let r = self.ast.types.as_record(q)?;
        self.bits(r).contains(DefinitionBits::COMPLETE).then(|| self.def(r))
    }

    /// The definition of `q`'s class when it is a standard-layout struct.
    fn sl_struct(&self, q: QualType) -> Option<DeclId> {
        let r = self.complete_record(q)?;
        (!self.is_union(r) && self.standard_layout(r)).then_some(r)
    }

    /// The anonymous struct or union field `f`'s class.
    fn anonymous(&self, f: &FieldDesc) -> Option<DeclId> {
        let r = self.ast.types.as_record(f.ty)?;
        self.bits(r).contains(DefinitionBits::ANONYMOUS).then(|| self.def(r))
    }

    fn same_unqual(&self, a: QualType, b: QualType) -> bool {
        let t = &self.ast.types;
        t.same_type(t.canonical(a).without_quals(), t.canonical(b).without_quals())
    }

    /// `a` and `b` are the same type once every qualifier, including
    /// those an array carries on its element, is dropped.
    fn same_unqualified(&self, lhs: QualType, rhs: QualType, depth: usize) -> bool {
        let tys = &self.ast.types;
        let elem = |q: QualType| match tys.canonical_view(q) {
            TypeView::ConstantArray(arr) => Some(arr.elem),
            TypeView::IncompleteArray(el) => Some(el),
            _ => None,
        };
        match (tys.canonical_view(lhs), tys.canonical_view(rhs)) {
            (TypeView::ConstantArray(_), TypeView::ConstantArray(_))
            | (TypeView::IncompleteArray(_), TypeView::IncompleteArray(_)) => {
                depth <= MAX_DEPTH
                    && tys.array_size(lhs) == tys.array_size(rhs)
                    && match (elem(lhs), elem(rhs)) {
                        (Some(l), Some(r)) => self.same_unqualified(l, r, depth + 1),
                        _ => false,
                    }
            }
            _ => self.same_unqual(lhs, rhs),
        }
    }

    fn types(&self, lhs: QualType, rhs: QualType, depth: usize) -> bool {
        if depth > MAX_DEPTH {
            return false;
        }
        if self.same_unqualified(lhs, rhs, depth) {
            return true;
        }
        let tys = &self.ast.types;
        match (tys.canonical_view(lhs), tys.canonical_view(rhs)) {
            (TypeView::Enum(l), TypeView::Enum(r)) => {
                let int_ty = |e: DeclId| {
                    let first = self.ast.first(e);
                    let data = self.ast.decls.enumeration(self.def(first))?;
                    let complete = data.flags & EnumData::COMPLETE != 0 || data.is_fixed();
                    complete.then_some(data.int_type).flatten()
                };
                match (int_ty(l), int_ty(r)) {
                    (Some(li), Some(ri)) => tys.same_type(li, ri),
                    _ => false,
                }
            }
            (TypeView::Record(l), TypeView::Record(r)) => {
                self.standard_layout(l)
                    && self.standard_layout(r)
                    && self.records(self.def(l), self.def(r), depth + 1)
            }
            _ => false,
        }
    }

    /// Two structs whose common initial sequence is all of both, or two
    /// unions whose members pair up in any order with layout-compatible
    /// types ([class.mem.general]/24-25).
    fn records(&self, x: DeclId, y: DeclId, depth: usize) -> bool {
        if self.is_union(x) != self.is_union(y) {
            return false;
        }
        if self.is_union(x) {
            let mut unmatched = fields(self.ast, y);
            for f in fields(self.ast, x) {
                let Some(i) = unmatched.iter().position(|g| self.union_members(&f, g, depth))
                else {
                    return false;
                };
                unmatched.remove(i);
            }
            return unmatched.is_empty();
        }
        let c = self.cis(x, y, depth);
        c.len == c.x.len() && c.len == c.y.len()
    }

    /// Union members pair as clang 19 pairs them: layout-compatible
    /// types, the same bit-field width or none, no
    /// `[[no_unique_address]]`.
    fn union_members(&self, f: &FieldDesc, g: &FieldDesc, depth: usize) -> bool {
        self.types(f.ty, g.ty, depth) && f.bit_width == g.bit_width && !f.nua && !g.nua
    }

    /// clang's `CXXRecordDecl::getStandardLayoutBaseWithFields`: the
    /// class among `d` and its bases that declares the non-static data
    /// members (`d` itself when none does).
    fn base_with_fields(&self, d: DeclId, depth: usize) -> DeclId {
        let has_fields = |r: DeclId| {
            self.ast.members(r).any(|m| self.ast.decls.get(m).kind() == DeclKind::Field)
        };
        if has_fields(d) || depth > MAX_DEPTH {
            return d;
        }
        for b in self.ast.decls.bases(d, &self.ast.lists).map(|b| b.ty).collect::<Vec<_>>() {
            if let Some(r) = self.ast.types.as_record(b) {
                let w = self.base_with_fields(self.def(r), depth + 1);
                if has_fields(w) {
                    return w;
                }
            }
        }
        d
    }

    /// The bit offsets of the fields of class `r`, in declaration order.
    fn field_offsets(&self, r: DeclId) -> Box<[u64]> {
        match self.ast.layouts.get(self.ast.first(r)) {
            Some(l) => l.fields.clone(),
            None => super::record_layout(self.ast, r, self.target).fields,
        }
    }

    /// A member's alignment requirement in bits: its type's, raised by
    /// `alignas` on it.
    fn align(&self, f: &FieldDesc) -> u64 {
        let ty = super::type_size_align(self.ast, f.ty, self.target).align.saturating_mul(8);
        ty.max(super::decl_max_align(self.ast, f.decl, self.target))
    }

    /// The common initial sequence of the standard-layout structs `x`
    /// and `y` (definitions): the longest prefix of the fields (unnamed
    /// bit-fields included) of the classes that declare them whose pairs
    /// correspond ([class.mem.general]/23, plus GCC's equal offsets).
    fn cis(&self, x: DeclId, y: DeclId, depth: usize) -> Cis {
        let wx = self.base_with_fields(x, 0);
        let wy = self.base_with_fields(y, 0);
        let (fx, fy) = (fields(self.ast, wx), fields(self.ast, wy));
        let (ox, oy) = (self.field_offsets(wx), self.field_offsets(wy));
        let len = fx
            .iter()
            .zip(&fy)
            .enumerate()
            .take_while(|&(i, (f, g))| {
                self.types(f.ty, g.ty, depth)
                    && f.bit_width == g.bit_width
                    && f.nua == g.nua
                    && self.align(f) == self.align(g)
                    && ox.get(i).is_some()
                    && ox.get(i) == oy.get(i)
            })
            .count();
        Cis { x: fx, y: fy, ox, len }
    }

    /// Whether `m` (a first declaration) is declared in the anonymous
    /// class `a` or in an anonymous class nested in it.
    fn within(&self, m: DeclId, a: DeclId) -> bool {
        let mut cur = self.ast.decls.get(m).ctx();
        for _ in 0..MAX_DEPTH {
            let Some(c) = cur else { return false };
            let c = self.def(c);
            if self.ast.first(c) == self.ast.first(a) {
                return true;
            }
            if !self.bits(c).contains(DefinitionBits::ANONYMOUS) {
                return false;
            }
            cur = self.ast.decls.get(c).ctx();
        }
        false
    }

    /// A pair of GNU anonymous structs (not unions) in the sequence.
    fn anonymous_structs(&self, f: &FieldDesc, g: &FieldDesc) -> Option<(DeclId, DeclId)> {
        let (a, b) = (self.anonymous(f)?, self.anonymous(g)?);
        (!self.is_union(a) && !self.is_union(b)).then_some((a, b))
    }

    fn corresponding_in(&self, x: DeclId, y: DeclId, m1: DeclId, m2: DeclId, depth: usize) -> bool {
        if depth > MAX_DEPTH {
            return false;
        }
        let c = self.cis(x, y, depth);
        for (f, g) in c.x.iter().zip(&c.y).take(c.len) {
            if self.ast.first(f.decl) == m1 && self.ast.first(g.decl) == m2 {
                return true;
            }
            if let Some((a, b)) = self.anonymous_structs(f, g)
                && self.within(m1, a)
                && self.within(m2, b)
            {
                return self.corresponding_in(a, b, m1, m2, depth + 1);
            }
        }
        false
    }

    fn offsets_in(
        &self,
        x: DeclId,
        y: DeclId,
        t: (QualType, QualType),
        base: u64,
        depth: usize,
        out: &mut Vec<u64>,
    ) {
        if depth > MAX_DEPTH {
            return;
        }
        let c = self.cis(x, y, depth);
        for (i, (f, g)) in c.x.iter().zip(&c.y).take(c.len).enumerate() {
            let at = base + c.ox.get(i).copied().unwrap_or(0) / 8;
            if f.bit_width.is_none() && self.same_unqual(f.ty, t.0) && self.same_unqual(g.ty, t.1) {
                out.push(at);
            }
            if let Some((a, b)) = self.anonymous_structs(f, g) {
                self.offsets_in(a, b, t, at, depth + 1, out);
            }
        }
    }

    /// The field of class `r` (a definition) that holds the anonymous
    /// class `a`.
    fn anonymous_field(&self, r: DeclId, a: DeclId) -> Option<DeclId> {
        fields(self.ast, r)
            .into_iter()
            .find(|f| self.anonymous(f).is_some_and(|b| self.ast.first(b) == self.ast.first(a)))
            .map(|f| f.decl)
    }

    /// Whether `f` (a first declaration) is the first non-static data
    /// member of class `p` (unnamed bit-fields are not members,
    /// [class.bit]/2).
    fn first_member(&self, p: DeclId, f: DeclId) -> bool {
        fields(self.ast, p)
            .into_iter()
            .find(|g| g.named || self.anonymous(g).is_some())
            .is_some_and(|g| self.ast.first(g.decl) == f)
    }

    fn interconvertible(&self, r: DeclId, m: DeclId) -> bool {
        let mut f = m;
        for _ in 0..MAX_DEPTH {
            let Some(p) = self.ast.decls.get(f).ctx() else { return false };
            if self.ast.decls.record(p).is_none() {
                return false;
            }
            let p = self.def(p);
            if !(self.is_union(p) || (self.standard_layout(p) && self.first_member(p, f))) {
                return false;
            }
            if self.ast.first(p) == self.ast.first(r) {
                return true;
            }
            if self.bits(p).contains(DefinitionBits::ANONYMOUS) {
                let Some(outer) = self.ast.decls.get(p).ctx() else { return false };
                let Some(af) = self.anonymous_field(self.def(outer), p) else { return false };
                f = self.ast.first(af);
                continue;
            }
            // A member of a base class: `r` is a standard-layout struct
            // whose members `p` declares, so `r` is pointer-
            // interconvertible with its `p` subobject.
            return !self.is_union(r)
                && self.standard_layout(r)
                && self.ast.first(self.base_with_fields(r, 0)) == self.ast.first(p);
        }
        false
    }

    /// Calls `each` on every data member a `T r::*` can designate: the
    /// fields of `r` and of its bases, and those of anonymous classes
    /// among them.
    fn each_member(&self, r: DeclId, depth: usize, each: &mut dyn FnMut(&FieldDesc)) {
        if depth > MAX_DEPTH {
            return;
        }
        for f in fields(self.ast, r) {
            if let Some(a) = self.anonymous(&f) {
                self.each_member(a, depth + 1, each);
            } else if f.named {
                each(&f);
            }
        }
        for b in self.ast.decls.bases(r, &self.ast.lists).map(|b| b.ty).collect::<Vec<_>>() {
            if let Some(c) = self.ast.types.as_record(b) {
                self.each_member(self.def(c), depth + 1, each);
            }
        }
    }
}
