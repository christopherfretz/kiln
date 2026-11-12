//! A function template specialization's parameter in a body
//! (spec/modules.md §11.11, "Forms carried by WP-1350").
//!
//! A default argument use of a specialization's parameter (`f(1.0)` of
//! `template <class T> int f(T, int = sizeof(T))`, a constructor
//! template's defaulted allocator) names a parameter that has no record:
//! the specialization is a [`Payload::Spec`] reference the importer
//! instantiates again. The word `PARAM | SPECP | spec << 8 | i` names the
//! `i`-th parameter of specialization record `spec`; the importer reads
//! it from the specialization it declared.

use kiln_ast::DeclId;
use kiln_ast::decl::DeclKind;
use kiln_ast::expr::ExprKind;

use super::body::PARAM;
use super::{DeclRec, Image, Locs, Payload, W, Writer};
use crate::Sema;

/// The specialization-parameter bit of a parameter word.
pub(super) const SPECP: u32 = 0x2000_0000;
/// Parameter positions a word holds.
const INDEX_BITS: u32 = 8;
/// Records a word can name.
const REC_LIMIT: u32 = 1 << (29 - INDEX_BITS);

/// Whether parameter word `w` names a specialization's parameter.
pub(super) fn is_spec_param(w: u32) -> bool {
    w & PARAM != 0 && w & super::requires::REQP == 0 && w & SPECP != 0
}

/// The specialization record and position of word `w`.
fn split(w: u32) -> (u32, u32) {
    let v = w & !(PARAM | SPECP);
    (v >> INDEX_BITS, v & ((1 << INDEX_BITS) - 1))
}

/// The record word `w` depends on.
pub(super) fn spec_param_rec(w: u32) -> u32 {
    split(w).0
}

/// Whether word `w` names an existing parameter of a function template
/// specialization record.
pub(super) fn spec_param_ok(img: &Image, w: u32) -> bool {
    let (r, i) = split(w);
    let Some(DeclRec { payload: Payload::Spec { template, .. }, .. }) = img.decls.get(r as usize)
    else {
        return false;
    };
    let Some(DeclRec { payload: Payload::Template { pattern, .. }, .. }) =
        img.decls.get(*template as usize)
    else {
        return false;
    };
    matches!(
        img.decls.get(*pattern as usize),
        Some(DeclRec { payload: Payload::Function { params, .. }, .. }) if (i as usize) < params.len()
    )
}

impl Writer<'_, '_, '_> {
    /// The word of parameter `d` of a function template specialization
    /// the image names by reference; `None` for any other declaration.
    pub(super) fn spec_param(&mut self, d: DeclId) -> W<Option<u32>> {
        let ast = &self.s.ast;
        if ast.decls.get(d).kind() != DeclKind::ParmVar {
            return Ok(None);
        }
        let Some(f) = ast.decls.get(d).ctx() else { return Ok(None) };
        if ast.spec_info(ast.first(f)).is_none() {
            return Ok(None);
        }
        let Some(fd) = ast.decls.function(f) else { return Ok(None) };
        let Some(i) = ast.lists.iter::<DeclId>(fd.params).position(|p| p == d) else {
            return Ok(None);
        };
        let Some(r) = self.spec_rec(f)? else { return Ok(None) };
        if r >= REC_LIMIT || i >= 1 << INDEX_BITS {
            return Err("too many records for a specialization's parameter");
        }
        Ok(Some(PARAM | SPECP | r << INDEX_BITS | i as u32))
    }
}

impl Sema<'_, '_> {
    /// The default arguments of the function template specializations
    /// the image names: the importer declared them before their
    /// patterns' defaults were built, so each is deferred again as
    /// substitution defers it (`defer_default_args`, [temp.inst]/12),
    /// and the ones a body uses are instantiated, as the replay's call
    /// did.
    pub(super) fn image_spec_defaults(&mut self, img: &Image, ids: &[DeclId], main: &Locs) {
        let params = |s: &Self, f: DeclId| -> Vec<DeclId> {
            s.ast
                .decls
                .function(f)
                .map_or_else(Vec::new, |fd| s.ast.lists.iter(fd.params).collect())
        };
        for (r, rec) in img.decls.iter().enumerate() {
            let Payload::Spec { .. } = rec.payload else { continue };
            let Some(&f) = ids.get(r) else { continue };
            let Some(pat) = self.ast.spec_info(f).map(|(_, i)| i.from) else { continue };
            let (pp, fp) = (params(self, pat), params(self, f));
            if self.ast.decls.function(f).is_none() || pp.len() != fp.len() {
                continue;
            }
            let mut any = false;
            for (a, b) in pp.into_iter().zip(fp) {
                let def = self.ast.decls.param(a).and_then(|x| x.default_arg);
                if def.is_none_or(|e| self.ast.exprs.get(e).kind() == ExprKind::CXXDefaultArgExpr)
                    || self.ast.decls.param(b).is_some_and(|x| x.default_arg.is_some())
                    || self.inst.lazy_defaults.contains_key(&b)
                {
                    continue;
                }
                self.ast.decls.mark_default_arg_pending(b);
                self.inst.lazy_defaults.insert(b, (pat, a));
                any = true;
            }
            if any {
                self.ast.refresh_min_args(f);
            }
        }
        let k = super::body::EXPR_KINDS.iter().position(|&k| k == ExprKind::CXXDefaultArgExpr);
        for e in &img.bodies.exprs {
            let Some(&w) = e.words.first() else { continue };
            if Some(usize::from(e.kind)) != k || !is_spec_param(w) {
                continue;
            }
            let Some(p) = self.image_spec_param(ids, w) else { continue };
            let f = ids[spec_param_rec(w) as usize];
            let _ = self.instantiate_lazy_default(f, p, main.get(e.loc));
        }
    }

    /// The parameter word `w` names, of the specialization the importer
    /// declared for its record.
    pub(super) fn image_spec_param(&self, ids: &[DeclId], w: u32) -> Option<DeclId> {
        let (r, i) = split(w);
        let f = *ids.get(r as usize)?;
        let fd = self.ast.decls.function(f)?;
        self.ast.lists.iter::<DeclId>(fd.params).nth(i as usize)
    }
}
