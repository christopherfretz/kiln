//! Deduction guides from inherited constructors ([over.match.class.deduct]/1
//! last paragraph, P2582; spec/sema-templates.md §6.6).
//!
//! When class template `C` inherits constructors from a direct base denoted
//! by `B` (a simple-template-id), the guides of the hypothetical alias
//! template `template<C's parameters> using A = B;` are guides of `C` too,
//! provided `A` is deducible; each one's return type `R` is replaced by
//! `CC<R>::type`, the specialization of `C` whose arguments are deduced
//! from `R` against `B`. A guide is built with the alias-template
//! machinery ([`Sema::build_alias_guide`], `templ.ctad_alias` holding
//! `(C, B)` so that [`Sema::deduce_call`](crate::Sema) checks that `CC<R>`
//! matches); the selected guide's return type is then mapped to `C<...>`
//! by [`Sema::inherited_ctad_result`]. A non-template guide's result is
//! decided when the guide is formed: it is cloned with return type
//! `C<...>`.

use kiln_ast::decl::{DeclFlags, DeclKind, DefinitionBits};
use kiln_ast::names::DeclNameKind;
use kiln_ast::types::{ArgView, Nns, TemplateArg, TypeView};
use kiln_ast::{DeclId, QualType};
use kiln_source::Loc;

use super::GuideShape;
use crate::Sema;
use crate::templ::args::{LevelMode, MultiLevelArgs};
use crate::templ::subst::Substituter;

impl Sema<'_, '_> {
    /// The guides class template `tmpl` gets from the constructors it
    /// inherits ([over.match.class.deduct]/1, P2582): for each
    /// inheriting-constructor using-declarator of its definition whose
    /// nested-name-specifier denotes a direct base `B` that is a
    /// specialization of a class template, the guides of that template
    /// rewritten for the alias template `A` (§6.6). `A` must be deducible:
    /// every template parameter of `tmpl` is deducible from `B`.
    pub(super) fn inherited_guides(
        &mut self,
        tmpl: DeclId,
        level: u32,
        name_loc: Loc,
    ) -> Vec<DeclId> {
        let mut out = Vec::new();
        if level > 64 {
            return out;
        }
        let Some(def) = self.pattern_of(tmpl).and_then(|p| self.ast.definition(p)) else {
            return out;
        };
        let bits = self.ast.decls.record(def).map(|r| r.defn).unwrap_or_default();
        if !bits.contains(DefinitionBits::HAS_INHERITED_CTOR) {
            return out;
        }
        for rhs in self.inherited_ctor_bases(def) {
            let TypeView::TemplateSpecialization { name, .. } = self.ast.types.view(rhs.ty())
            else {
                continue;
            };
            let Some(base) = self.ast.types.template_decl(name) else { continue };
            if self.kind_of(base) != DeclKind::ClassTemplate || !self.alias_deducible(tmpl, rhs, rhs)
            {
                continue;
            }
            let Some(inner) = self.ctad_guides(base, None, level + 1, name_loc) else { continue };
            for g in inner {
                // A guide the base itself inherited: its return type is
                // not a specialization of the base (follow-up).
                if self.templ.ctad_inherited.contains_key(&g) {
                    continue;
                }
                if let Some(ng) = self.inherited_guide(tmpl, rhs, g) {
                    out.push(ng);
                }
            }
        }
        out
    }

    /// The direct bases of class definition `def` (each as written, sugar
    /// stripped) that an inheriting-constructor using-declarator of it
    /// nominates, in declaration order, without duplicates.
    fn inherited_ctor_bases(&self, def: DeclId) -> Vec<QualType> {
        let mut out: Vec<QualType> = Vec::new();
        let bases: Vec<QualType> = self
            .ast
            .decls
            .bases(def, &self.ast.lists)
            .filter(|b| !b.pack_expansion)
            .map(|b| b.ty)
            .collect();
        for m in self.ast.decls.members(def) {
            if !matches!(self.kind_of(m), DeclKind::Using | DeclKind::UnresolvedUsingValue)
                || self.ast.decls.get(m).name().kind() != DeclNameKind::Constructor
            {
                continue;
            }
            let Some(nns) = self.ast.using_qualifier(m) else { continue };
            let Nns::Type { ty, .. } = self.ast.types.nns_view(nns) else { continue };
            let want = self.ast.types.canonical(ty);
            let Some(&b) = bases.iter().find(|&&b| self.ast.types.canonical(b) == want) else {
                continue;
            };
            let mut b = b;
            while let TypeView::Elaborated { named, .. } = self.ast.types.view(b.ty()) {
                b = named;
            }
            if b.quals().is_empty() && !out.iter().any(|&o| self.ast.types.canonical(o) == want) {
                out.push(b);
            }
        }
        out
    }

    /// The guide of class template `c` formed from guide `g` of the
    /// template its direct base `rhs` names, cached with the alias guides;
    /// `None` when `g`'s return type does not match `rhs`.
    fn inherited_guide(&mut self, c: DeclId, rhs: QualType, g: DeclId) -> Option<DeclId> {
        if let Some(&r) = self.templ.ctad_alias_cache.get(&(c, g)) {
            return r;
        }
        let r = if self.kind_of(g) == DeclKind::FunctionTemplate {
            self.build_alias_guide(c, rhs, g)
        } else {
            self.inherited_plain_guide(c, rhs, g)
        };
        if let Some(ng) = r {
            let _ = self.templ.ctad_inherited.insert(ng, (c, rhs));
        }
        let _ = self.templ.ctad_alias_cache.insert((c, g), r);
        r
    }

    /// A non-template guide `g` of the base: its return type `R` is not
    /// dependent, so `CC<R>::type` is decided here and `g` is cloned with
    /// it as the return type; `None` when `CC<R>` does not match.
    fn inherited_plain_guide(&mut self, c: DeclId, rhs: QualType, g: DeclId) -> Option<DeclId> {
        let ty = self.ast.decls.get(g).ty()?;
        let proto = self.ast.types.proto(ty)?;
        let r = proto.ret();
        let variadic = proto.variadic();
        let ptys: Vec<QualType> = self.ast.types.params(proto).to_vec();
        let ret = self.inherited_ctad_result(c, rhs, r)?;
        let cparams = self.param_decls(g);
        let first = self.ast.first(c);
        let ctx = self.ast.decls.get(first).ctx().unwrap_or(self.ast.tu);
        let name = self.ast.names.deduction_guide(first);
        let n = *self.ast.decls.get(g);
        let shape = GuideShape {
            ret,
            cparams: &cparams,
            ptys: &ptys,
            variadic,
            explicit: self.fn_spec(g).is_explicit(),
            depth: 0,
            implicit: n.has(DeclFlags::IMPLICIT),
        };
        Some(self.guide_function(ctx, name, n.loc(), &shape))
    }

    /// `CC<ret>::type` for class template `c` inheriting constructors from
    /// base `rhs` (P2582): `c`'s specialization whose template arguments
    /// are deduced from `ret` against `rhs`; `None` when they are not all
    /// deduced (an undeduced pack is empty).
    pub(crate) fn inherited_ctad_result(
        &mut self,
        c: DeclId,
        rhs: QualType,
        ret: QualType,
    ) -> Option<QualType> {
        use crate::templ::deduce::{DeducedArgs, TdfFlags, Tdk};
        let info = self.ast.template_info(c).copied()?;
        let list = *self.ast.templates.param_list(info.params);
        let params: Vec<DeclId> = self.ast.lists.iter::<DeclId>(list.params).collect();
        let mut d = DeducedArgs::new(list.depth, params.len());
        d.loc = self.ast.decls.get(c).loc();
        if self.deduce(rhs, ret, TdfFlags::NONE, &mut d) != Tdk::Success {
            return None;
        }
        let mut args = Vec::with_capacity(params.len());
        for (a, &p) in d.args.iter().zip(&params) {
            match a.view() {
                ArgView::Null if self.ast.template_parm(p).is_some_and(|i| i.pack) => {
                    args.push(TemplateArg::pack(self.ast.types.arg_list(&[])));
                }
                ArgView::Null => return None,
                _ => args.push(*a),
            }
        }
        let injected = self.injected_type_of(c)?;
        let mut levels = MultiLevelArgs::new();
        for _ in 0..list.depth {
            levels.push(kiln_ast::ids::ArgListId::EMPTY, c, LevelMode::Retain);
        }
        levels.push(self.ast.types.arg_list(&args), c, LevelMode::Substitute);
        Substituter::new(self, &levels).ty(injected, Loc::INVALID).ok()
    }

    /// Whether the candidate guide `g` (template or function, as in the
    /// set) was formed from an inherited constructor's guide
    /// ([over.match.best.general]/2.10).
    pub(crate) fn is_inherited_guide(&self, g: DeclId) -> bool {
        self.templ.ctad_inherited.contains_key(&g)
    }

    /// Whether functions `f1` and `f2` have parameters of the same types
    /// for the first `n` arguments, or are both variadic past their
    /// parameters ([over.match.best.general]/2.10).
    pub(crate) fn same_guide_params(&self, f1: DeclId, f2: DeclId, n: usize) -> bool {
        let t = &self.ast.types;
        let proto = |f: DeclId| self.ast.decls.get(f).ty().and_then(|ty| t.proto(ty));
        let (Some(p1), Some(p2)) = (proto(f1), proto(f2)) else { return false };
        let (a, b) = (t.params(p1), t.params(p2));
        (0..n).all(|i| match (a.get(i), b.get(i)) {
            (Some(&x), Some(&y)) => t.same_type(x, y),
            (None, None) => p1.variadic() && p2.variadic(),
            _ => false,
        })
    }

    /// The deduced type for guide `g` selected with return type `ret`:
    /// `CC<ret>::type` for a template guide formed from an inherited
    /// constructor's ([`Sema::inherited_ctad_result`]), else `ret`.
    pub(super) fn ctad_result_type(&mut self, g: DeclId, ret: QualType) -> QualType {
        match self.templ.ctad_inherited.get(&g).copied() {
            Some((c, rhs)) if self.kind_of(g) == DeclKind::FunctionTemplate => {
                self.inherited_ctad_result(c, rhs, ret).unwrap_or(QualType::ERROR)
            }
            _ => ret,
        }
    }
}

