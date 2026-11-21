//! Where dependent names were written (spec/sema-templates.md §5.8):
//! kiln's types carry no source locations (they are interned), so a
//! diagnostic found while substituting a dependent name (`T::type`,
//! `T::N::`, `X<T>`) would otherwise sit at the declaration being
//! instantiated. Each dependent type or nested-name-specifier the parser
//! forms records its written location here; the substituter reports at
//! the occurrence nearest to its anchor, or, for a declaration's own type,
//! the one on the declared type's side of the declarator-id (clang keeps a
//! `TypeLoc` per occurrence and reports at the name).

use kiln_ast::ids::{NnsId, QualType};
use kiln_source::Loc;

use crate::Sema;

/// Which occurrence of a dependent name a declaration-level substitution
/// means, relative to a bound in the declaration (§5.8): the declared
/// type of a variable, field, typedef, named parameter or leading return
/// type is written before the declarator-id; that of an alias-declaration,
/// unnamed parameter (located where it starts), conversion function or
/// trailing return type after it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum WrittenSide {
    /// The nearest occurrence at or before the bound.
    Before,
    /// The nearest occurrence at or after the bound.
    After,
}

/// The declaration-level occurrence rule in force for one anchor
/// ([`crate::templ::subst::Substituter::written_scope`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct WrittenScope {
    /// The anchor the declaration's type is substituted with; a lookup
    /// with any other anchor (an expression inside the type) is nearest.
    pub anchor: Loc,
    /// Where the occurrence lies relative to `bound`.
    pub side: WrittenSide,
    /// The declarator-id (or the end of the parameters, for a trailing
    /// return type).
    pub bound: Loc,
}

/// A dependent parameter of a function type written inside another
/// declarator (§5.8.1): where it is declared, and what its type check at
/// substitution needs that the function type does not keep (clang keeps a
/// `ParmVarDecl` per parameter of every `FunctionProtoTypeLoc` and checks
/// it again in `SubstParmVarDecl`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, kiln_snap::Snap)]
pub(crate) struct NestedParam {
    /// The parameter's name, or where it would be (clang's `NameLoc`).
    pub loc: Loc,
    /// Its Swift parameter ABI attribute, when written (else invalid).
    pub abi_loc: Loc,
    /// Declared as an array (adjusted to the pointer the type keeps): its
    /// element type is the one an address space must not qualify.
    pub array: bool,
}

/// A dependent entity whose written locations are recorded.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, kiln_snap::Snap)]
pub(crate) enum WrittenKey {
    /// A dependent type (`typename T::type`, `X<T>`).
    Type(QualType),
    /// A dependent nested-name-specifier component (`T::N::`).
    Nns(NnsId),
}

impl Sema<'_, '_> {
    /// Records that `key` was written at `loc` (kept sorted, so the
    /// lookup is a binary search). Nothing while instantiating: only the
    /// pattern as written is recorded.
    pub(crate) fn note_written_loc(&mut self, key: WrittenKey, loc: Loc) {
        if !loc.is_valid() || !self.inst.stack.is_empty() {
            return;
        }
        let v = self.templ.written_locs.entry(key).or_default();
        let i = v.partition_point(|&l| l < loc);
        if v.get(i) != Some(&loc) {
            v.insert(i, loc);
        }
    }

    /// Records the dependent parameters `(adjusted type, written)` of a
    /// function type written inside another declarator (§5.8.1); nothing
    /// while instantiating.
    pub(crate) fn note_nested_params(&mut self, params: &[(QualType, NestedParam)]) {
        if !self.inst.stack.is_empty() {
            return;
        }
        for (i, &(ty, np)) in params.iter().enumerate() {
            let Ok(i) = u16::try_from(i) else { break };
            if !np.loc.is_valid() || !self.is_instantiation_dependent_type(ty) {
                continue;
            }
            let v = self.templ.nested_params.entry((ty, i)).or_default();
            let at = v.partition_point(|p| p.loc < np.loc);
            if v.get(at).is_none_or(|p| p.loc != np.loc) {
                v.insert(at, np);
            }
        }
    }

    /// [`Self::note_nested_params`] of a function declarator's `params`
    /// whose types in the function type are `ptys`.
    pub(crate) fn note_declarator_nested_params(
        &mut self,
        params: &[crate::declspec::ParamInfo],
        ptys: &[QualType],
    ) {
        if !params.iter().any(|p| self.is_instantiation_dependent_type(p.ty)) {
            return;
        }
        let v: Vec<(QualType, NestedParam)> = params
            .iter()
            .zip(ptys)
            .map(|(p, &ty)| {
                let loc = p.decl.map_or_else(
                    || p.name.map_or(p.loc, |n| n.1),
                    |pd| self.ast.decls.get(pd).loc(),
                );
                let abi_loc = p
                    .attrs
                    .iter()
                    .map(|&a| self.ast.attrs.get(a))
                    .find(|n| n.kind == kiln_ast::attr::AttrKind::SWIFT_PARAM_ABI)
                    .map_or(Loc::INVALID, |n| n.loc);
                let array = p.original_ty.is_some_and(|o| self.ast.types.is_array(o));
                (ty, NestedParam { loc, abi_loc, array })
            })
            .collect();
        self.note_nested_params(&v);
    }

    /// The written parameter `i` (of pattern type `ty`) of a function type
    /// inside another declarator nearest to `anchor`, if one was recorded
    /// ([`Self::note_nested_params`]).
    pub(crate) fn nested_param(&self, ty: QualType, i: usize, anchor: Loc) -> Option<NestedParam> {
        let v = self.templ.nested_params.get(&(ty, u16::try_from(i).ok()?))?;
        let at = v.partition_point(|p| p.loc <= anchor);
        let before = at.checked_sub(1).and_then(|j| v.get(j)).copied();
        let after = v.get(at).copied();
        let dist = |p: &NestedParam| p.loc.to_raw().abs_diff(anchor.to_raw());
        match (before, after) {
            (Some(b), Some(a)) => Some(if dist(&a) < dist(&b) { a } else { b }),
            (b, a) => b.or(a),
        }
    }

    /// Records the class-key `kw` of a dependent tag name written at
    /// `name` (`class T::X`); nothing while instantiating.
    pub fn note_tag_keyword_loc(&mut self, name: Loc, kw: Loc) {
        if name.is_valid() && kw.is_valid() && self.inst.stack.is_empty() {
            self.templ.tag_keyword_locs.insert(name, kw);
        }
    }

    /// The class-key written before the dependent tag name at `name`, or
    /// `name` when none was recorded.
    pub(crate) fn tag_keyword_loc(&self, name: Loc) -> Loc {
        self.templ.tag_keyword_locs.get(&name).copied().unwrap_or(name)
    }

    /// The written location of `key` on `scope`'s side of its bound, or
    /// nearest to `anchor` when there is none there (or no scope applies
    /// to `anchor`).
    pub(crate) fn written_loc_in(
        &self,
        key: WrittenKey,
        anchor: Loc,
        scope: Option<WrittenScope>,
    ) -> Loc {
        if let Some(sc) = scope.filter(|sc| sc.anchor == anchor)
            && let Some(v) = self.templ.written_locs.get(&key)
        {
            let found = match sc.side {
                WrittenSide::Before => {
                    v.partition_point(|&l| l <= sc.bound).checked_sub(1).and_then(|i| v.get(i))
                }
                WrittenSide::After => v.get(v.partition_point(|&l| l < sc.bound)),
            };
            if let Some(&l) = found {
                return l;
            }
        }
        self.written_loc(key, anchor)
    }

    /// The written location of `key` nearest to `anchor` (the location a
    /// substitution was given: the declarator, expression or use being
    /// instantiated), or `anchor` when none was recorded.
    pub(crate) fn written_loc(&self, key: WrittenKey, anchor: Loc) -> Loc {
        let Some(v) = self.templ.written_locs.get(&key) else { return anchor };
        let i = v.partition_point(|&l| l <= anchor);
        let before = i.checked_sub(1).and_then(|j| v.get(j)).copied();
        let after = v.get(i).copied();
        let dist = |l: Loc| l.to_raw().abs_diff(anchor.to_raw());
        match (before, after) {
            (Some(b), Some(a)) => {
                if dist(a) < dist(b) {
                    a
                } else {
                    b
                }
            }
            (Some(l), None) | (None, Some(l)) => l,
            (None, None) => anchor,
        }
    }
}
